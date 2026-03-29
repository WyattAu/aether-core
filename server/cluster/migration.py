"""Actor migration coordinator.

Orchestrates the migration of actors between cluster nodes using
a three-phase handoff protocol:

Phase 1: Quiesce — Pause the actor, drain its mailbox.
Phase 2: Transfer — Serialize state and send to target node.
Phase 3: Activate — Unregister locally, target activates with restored state.

Only the cluster leader initiates migrations. Migrations are
triggered when nodes join or leave, causing the hash ring to
reassign actor ownership.
"""

import json
import logging
import threading
import time
from typing import Any, Callable, Dict, List, Optional, Set

from .config import ClusterConfig
from .membership import ClusterMembership
from .migration_state import MigrationStateTracker, MigrationStatus
from .transport import ClusterTransport

logger = logging.getLogger("aether-server.cluster.migration")


class MigrationCoordinator:
    """Coordinates actor migration between cluster nodes.

    The coordinator is owned by the leader node. When the cluster
    topology changes (node join/leave/failure), the coordinator
    computes which actors need to move and executes the migrations.

    Non-leader nodes run a passive coordinator that can receive
    incoming migrations but does not initiate them.

    Args:
        membership: The cluster membership instance.
        transport: The inter-node transport.
        config: The cluster configuration.
        actor_runtime: The local actor runtime (for snapshot/restore).
        state_store: The state store (for persistent state transfer).
    """

    def __init__(
        self,
        membership: ClusterMembership,
        transport: ClusterTransport,
        config: ClusterConfig,
        actor_runtime: Any = None,
        state_store: Any = None,
    ):
        self._membership = membership
        self._transport = transport
        self._config = config
        self._actor_runtime = actor_runtime
        self._state_store = state_store
        self._tracker = MigrationStateTracker()
        self._lock = threading.Lock()
        self._migrating_actors: Set[str] = set()  # Locally migrating
        self._handler_registry: Dict[str, Callable] = {}  # actor_type -> handler factory

        # Callbacks
        self._on_migration_complete: Optional[Callable] = None

        # Statistics
        self._total_migrated_in = 0
        self._total_migrated_out = 0

    # ============================================================
    # Handler Registry
    # ============================================================

    def register_handler_factory(
        self,
        actor_type: str,
        handler_factory: Callable,
    ) -> None:
        """Register a handler factory for an actor type.

        When an actor is migrated to this node, the coordinator
        uses the factory to create a handler for the restored actor.

        Args:
            actor_type: The actor type identifier.
            handler_factory: A callable that returns an async handler
                ``handler(ctx, envelope)``.
        """
        self._handler_registry[actor_type] = handler_factory
        logger.info("Migration handler registered for actor type: %s", actor_type)

    def _get_handler(self, actor_type: str) -> Optional[Callable]:
        """Get a handler for an actor type, or None if not registered."""
        factory = self._handler_registry.get(actor_type)
        if factory is None:
            return None
        try:
            return factory()
        except Exception as e:
            logger.error("Failed to create handler for type %s: %s", actor_type, e)
            return None

    # ============================================================
    # Outbound Migration (Source Node)
    # ============================================================

    def should_migrate(self, actor_id: str) -> Optional[str]:
        """Check if an actor should be migrated to a different node.

        Returns the target node_id if migration is needed, None otherwise.

        This checks the hash ring: if the ring says a different node
        owns this actor, migration is needed.
        """
        if not self._membership.is_running:
            return None

        if not self._membership.is_leader:
            return None

        target = self._membership.get_node_for_key(actor_id)
        if target is None:
            return None

        if target.node_id == self._membership.node_id:
            return None  # Already on the right node

        # Don't migrate to a non-alive node
        if not target.is_alive():
            return None

        return target.node_id

    def compute_rebalance(self, local_actor_ids: List[str]) -> List[Dict[str, str]]:
        """Compute which local actors need to be migrated.

        Called by the leader after a topology change. Returns a list
        of migration plans: [{actor_id, target_node_id}, ...].

        Args:
            local_actor_ids: IDs of actors on this node.
        """
        if not self._membership.is_leader:
            return []

        plans = []
        for actor_id in local_actor_ids:
            if self._tracker.is_migrating(actor_id):
                continue
            target = self.should_migrate(actor_id)
            if target is not None:
                plans.append({
                    "actor_id": actor_id,
                    "target_node_id": target,
                })
        return plans

    async def migrate_actor(
        self,
        actor_id: str,
        target_node_id: str,
    ) -> Dict[str, Any]:
        """Execute the full migration of an actor to a target node.

        Three-phase protocol:
        1. Quiesce: pause the actor, drain mailbox
        2. Transfer: serialize state, send to target
        3. Activate: unregister locally, confirm on target

        Returns:
            A dict with migration result.
        """
        # Register migration
        if not self._tracker.start_migration(
            actor_id, self._membership.node_id, target_node_id
        ):
            return {
                "actor_id": actor_id,
                "status": "skipped",
                "reason": "already_migrating",
            }

        source_node = self._membership.node_id

        try:
            # Phase 1: Quiesce
            self._tracker.update_status(actor_id, MigrationStatus.QUIESCING)

            if self._actor_runtime is None:
                raise RuntimeError("No actor runtime available")

            if not self._actor_runtime.quiesce_actor(actor_id):
                raise RuntimeError(
                    f"Actor {actor_id} not found or not active"
                )

            drain_count = self._actor_runtime.drain_actor(
                actor_id,
                timeout=self._config.migration_drain_timeout_seconds,
            )

            logger.info(
                "Actor %s quiesced (drain_remaining=%d)",
                actor_id, drain_count,
            )

            # Phase 2: Transfer
            self._tracker.update_status(actor_id, MigrationStatus.TRANSFERRING)

            snapshot = self._actor_runtime.snapshot_actor(actor_id)
            if snapshot is None:
                raise RuntimeError(f"Failed to snapshot actor {actor_id}")

            # Collect persistent state from StateStore
            persistent_state = {}
            if self._state_store is not None:
                try:
                    persistent_state = self._state_store.get_all(actor_id)
                except Exception as e:
                    logger.warning("Failed to get persistent state for %s: %s",
                                   actor_id, e)

            # Get pending messages from MessageRouter
            pending_messages = []
            if hasattr(self._actor_runtime, '_router'):
                try:
                    msgs = self._actor_runtime._router.get_pending_messages(actor_id)
                    if msgs:
                        pending_messages = [
                            m if isinstance(m, dict) else
                            (m.model_dump(mode="json") if hasattr(m, 'model_dump') else {})
                            for m in msgs
                        ]
                except Exception as e:
                    logger.warning("Failed to get pending messages for %s: %s",
                                   actor_id, e)

            # Build migration payload
            payload = {
                "actor_id": actor_id,
                "actor_type": snapshot["actor_type"],
                "state": snapshot["state"],
                "persistent_state": persistent_state,
                "supervision_strategy": snapshot["supervision_strategy"],
                "parent_id": snapshot.get("parent_id"),
                "children": snapshot.get("children", []),
                "pending_messages": pending_messages,
                "source_node": source_node,
                "migration_timeout": self._config.migration_timeout_seconds,
            }

            state_size = len(json.dumps(payload, default=str))
            self._tracker.set_state_size(actor_id, state_size)

            # Send to target node
            target_node = self._membership.get_member(target_node_id)
            if target_node is None:
                raise RuntimeError(f"Target node {target_node_id} not found")

            response = self._transport.migrate_actor(
                target_node.host, target_node.api_port, payload,
            )

            if response is None:
                raise RuntimeError(
                    f"Failed to transfer actor {actor_id} to {target_node_id}"
                )

            if response.get("status") != "accepted":
                raise RuntimeError(
                    f"Target node rejected migration: {response.get('error', 'unknown')}"
                )

            # Phase 3: Activate on target, deactivate locally
            self._actor_runtime.unregister_handler(actor_id)
            self._total_migrated_out += 1

            self._tracker.update_status(actor_id, MigrationStatus.COMPLETED)
            logger.info(
                "Actor %s migrated from %s to %s (state_size=%d bytes)",
                actor_id, source_node, target_node_id, state_size,
            )

            return {
                "actor_id": actor_id,
                "status": "completed",
                "source_node": source_node,
                "target_node": target_node_id,
                "state_size": state_size,
            }

        except Exception as e:
            self._tracker.update_status(
                actor_id, MigrationStatus.FAILED, error=str(e)
            )
            logger.error("Migration of actor %s failed: %s", actor_id, e)

            # Try to un-quiesce the actor (re-activate it)
            if self._actor_runtime is not None:
                try:
                    # Re-register won't work since we don't have the handler here,
                    # but we can at least log the situation
                    logger.warning(
                        "Actor %s is paused after failed migration — "
                        "manual intervention may be required", actor_id
                    )
                except Exception:
                    pass

            return {
                "actor_id": actor_id,
                "status": "failed",
                "error": str(e),
                "source_node": source_node,
                "target_node": target_node_id,
            }

    # ============================================================
    # Inbound Migration (Target Node)
    # ============================================================

    def receive_migration(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        """Handle an incoming actor migration from a remote node.

        This is called by the /cluster/internal/migrate/receive endpoint
        when a source node transfers an actor to this node.

        Args:
            payload: The migration payload containing actor state.

        Returns:
            A dict with acceptance status.
        """
        actor_id = payload.get("actor_id")
        actor_type = payload.get("actor_type", "default")
        source_node = payload.get("source_node", "unknown")

        if not actor_id:
            return {"status": "rejected", "error": "missing actor_id"}

        # Check if already have this actor locally
        if self._actor_runtime is not None:
            cell_info = self._actor_runtime.get_cell_info(actor_id)
            if cell_info is not None:
                return {
                    "status": "rejected",
                    "error": f"actor {actor_id} already exists locally",
                }

        # Get a handler for this actor type
        handler = self._get_handler(actor_type)
        if handler is None:
            return {
                "status": "rejected",
                "error": f"no handler registered for actor type {actor_type}",
            }

        try:
            # Restore actor from snapshot
            snapshot = {
                "actor_type": actor_type,
                "state": payload.get("state", {}),
                "supervision_strategy": payload.get("supervision_strategy", "restart"),
                "parent_id": payload.get("parent_id"),
                "children": payload.get("children", []),
                "message_count": 0,
                "error_count": 0,
            }

            context = self._actor_runtime.restore_actor(
                actor_id=actor_id,
                handler=handler,
                snapshot=snapshot,
            )

            if context is None:
                return {
                    "status": "rejected",
                    "error": "failed to restore actor",
                }

            # Restore persistent state
            persistent_state = payload.get("persistent_state", {})
            if self._state_store is not None and persistent_state:
                for key, value in persistent_state.items():
                    try:
                        self._state_store.set(actor_id, key, value)
                    except Exception as e:
                        logger.warning(
                            "Failed to restore persistent state %s:%s: %s",
                            actor_id, key, e
                        )

            # Re-deliver pending messages
            pending = payload.get("pending_messages", [])
            redelivered = 0
            if pending and hasattr(self._actor_runtime, '_router'):
                from ..models import MessageEnvelope
                for msg_data in pending:
                    try:
                        envelope = MessageEnvelope(**msg_data)
                        self._actor_runtime._router._pending[actor_id].append(envelope)
                        redelivered += 1
                    except Exception as e:
                        logger.warning(
                            "Failed to re-queue pending message for %s: %s",
                            actor_id, e
                        )

            self._total_migrated_in += 1
            logger.info(
                "Actor %s received from %s (state_keys=%d, pending_msgs=%d)",
                actor_id, source_node,
                len(payload.get("state", {})), redelivered,
            )

            return {
                "status": "accepted",
                "actor_id": actor_id,
                "redelivered_messages": redelivered,
            }

        except Exception as e:
            logger.error("Failed to receive actor %s: %s", actor_id, e)
            return {
                "status": "rejected",
                "error": str(e),
            }

    # ============================================================
    # Batch Migration
    # ============================================================

    async def rebalance(self, local_actor_ids: Optional[List[str]] = None) -> Dict[str, Any]:
        """Run a rebalance pass: compute and execute all needed migrations.

        Should be called by the leader after topology changes.

        Args:
            local_actor_ids: Optional list of actor IDs to check.
                If None, queries the actor runtime.

        Returns:
            A summary of migrations performed.
        """
        if not self._membership.is_leader:
            return {"status": "not_leader", "migrations": []}

        if local_actor_ids is None:
            if self._actor_runtime is not None:
                local_actor_ids = self._actor_runtime.get_registered_actor_ids()
            else:
                local_actor_ids = []

        plans = self.compute_rebalance(local_actor_ids)

        if not plans:
            return {"status": "balanced", "migrations": []}

        logger.info("Rebalance: %d actors to migrate", len(plans))

        # Limit batch size
        batch = plans[:self._config.migration_batch_size]
        results = []

        for plan in batch:
            result = await self.migrate_actor(
                plan["actor_id"],
                plan["target_node_id"],
            )
            results.append(result)

        completed = sum(1 for r in results if r["status"] == "completed")
        failed = sum(1 for r in results if r["status"] == "failed")
        skipped = sum(1 for r in results if r["status"] == "skipped")

        return {
            "status": "rebalanced",
            "planned": len(plans),
            "executed": len(batch),
            "completed": completed,
            "failed": failed,
            "skipped": skipped,
            "remaining": len(plans) - len(batch),
            "migrations": results,
        }

    # ============================================================
    # Query
    # ============================================================

    def get_tracker(self) -> MigrationStateTracker:
        """Get the migration state tracker."""
        return self._tracker

    def is_migrating(self, actor_id: str) -> bool:
        """Check if an actor is currently being migrated (inbound or outbound)."""
        return self._tracker.is_migrating(actor_id)

    def get_stats(self) -> dict:
        """Get migration statistics."""
        tracker_stats = self._tracker.get_stats()
        return {
            **tracker_stats,
            "migrated_in": self._total_migrated_in,
            "migrated_out": self._total_migrated_out,
        }

    def get_status(self) -> dict:
        """Get full migration status."""
        return {
            "is_leader": self._membership.is_leader,
            "node_id": self._membership.node_id,
            "stats": self.get_stats(),
            "active_migrations": self._tracker.get_active_migrations(),
            "recent_history": self._tracker.get_history(limit=20),
        }

    # ============================================================
    # Callbacks
    # ============================================================

    def on_migration_complete(self, callback: Callable) -> None:
        """Register a callback for completed migrations.

        Args:
            callback: Function(actor_id, source_node, target_node).
        """
        self._on_migration_complete = callback
