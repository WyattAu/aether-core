"""Cluster-aware message router.

Wraps the local MessageRouter and adds cross-node message forwarding
for actors that live on remote cluster nodes.
"""

import logging
from datetime import datetime, timezone
from typing import Optional

from ..message_router import MessageRouter
from ..models import DeliveryReceipt, MessageEnvelope
from .membership import ClusterMembership
from .transport import ClusterTransport

logger = logging.getLogger("aether-server.cluster.router")


class ClusterRouter:
    """Cluster-aware message router that forwards messages to remote nodes.

    Routing logic:
    1. Check if the target actor is being migrated -> buffer locally.
    2. Check if the target actor has a local handler -> deliver locally.
    3. If no local handler, check the hash ring for the owning node.
    4. If the owning node is remote, forward via HTTP transport.
    5. If no owning node found, buffer locally (same as single-node behavior).

    This router wraps a ``MessageRouter`` and delegates local delivery
    to it. It only intercepts messages that need cross-node forwarding.
    """

    def __init__(
        self,
        local_router: MessageRouter,
        membership: ClusterMembership,
        transport: ClusterTransport,
    ):
        self._local = local_router
        self._membership = membership
        self._transport = transport
        self._forwarded_count = 0
        self._failed_forwards = 0
        self._migration_coordinator = None  # Set by app.py after creation

    def set_migration_coordinator(self, coordinator) -> None:
        """Set the migration coordinator for migration-aware routing."""
        self._migration_coordinator = coordinator

    async def route(self, envelope: MessageEnvelope) -> DeliveryReceipt:
        """Route a message, forwarding to remote nodes if needed.

        Args:
            envelope: The message to route.

        Returns:
            A ``DeliveryReceipt`` indicating delivery status.
        """
        # If actor is being migrated, buffer the message locally
        if (self._migration_coordinator is not None
                and self._migration_coordinator.is_migrating(envelope.target_actor)):
            logger.debug("Buffering message for migrating actor %s",
                         envelope.target_actor)
            return await self._local.route(envelope)

        if self._has_local_handler(envelope.target_actor):
            return await self._local.route(envelope)

        if self._membership.is_running:
            target_node = self._membership.get_node_for_key(envelope.target_actor)

            if target_node is not None:
                if target_node.node_id == self._membership.node_id:
                    return await self._local.route(envelope)
                else:
                    return await self._forward_to_node(envelope, target_node)

        return await self._local.route(envelope)

    def _has_local_handler(self, actor_id: str) -> bool:
        """Check if the local router has a handler for this actor."""
        return actor_id in self._local._handlers

    async def _forward_to_node(
        self,
        envelope: MessageEnvelope,
        target_node,
    ) -> DeliveryReceipt:
        """Forward a message to a remote node via HTTP.

        Args:
            envelope: The message to forward.
            target_node: The remote node to send to.

        Returns:
            A ``DeliveryReceipt`` with forwarded/delivery status.
        """
        try:
            message_data = envelope.model_dump(mode="json")
            response = self._transport.forward_message(
                target_node.host, target_node.api_port, message_data,
            )

            if response is not None:
                self._forwarded_count += 1
                return DeliveryReceipt(
                    message_id=envelope.message_id,
                    status="forwarded",
                    delivered_at=datetime.now(timezone.utc),
                    correlation_id=envelope.correlation_id,
                )
            else:
                self._failed_forwards += 1
                return DeliveryReceipt(
                    message_id=envelope.message_id,
                    status="failed",
                    delivered_at=datetime.now(timezone.utc),
                    correlation_id=envelope.correlation_id,
                )
        except Exception as e:
            self._failed_forwards += 1
            logger.error("Failed to forward message %s to %s: %s",
                         envelope.message_id, target_node.node_id, e)
            return DeliveryReceipt(
                message_id=envelope.message_id,
                status="failed",
                delivered_at=datetime.now(timezone.utc),
                correlation_id=envelope.correlation_id,
            )

    def register_handler(self, actor_id: str, handler_fn) -> None:
        """Register a local handler (delegates to local router)."""
        self._local.register_handler(actor_id, handler_fn)

    def unregister_handler(self, actor_id: str) -> None:
        """Unregister a local handler (delegates to local router)."""
        self._local.unregister_handler(actor_id)

    def get_pending_messages(self, actor_id: str):
        """Get pending messages (delegates to local router)."""
        return self._local.get_pending_messages(actor_id)

    def clear_pending(self, actor_id: str) -> int:
        """Clear pending messages (delegates to local router)."""
        return self._local.clear_pending(actor_id)

    def get_receipt(self, message_id: str) -> Optional[DeliveryReceipt]:
        """Get delivery receipt (delegates to local router)."""
        return self._local.get_receipt(message_id)

    def total_message_count(self) -> int:
        """Get total message count (local + forwarded)."""
        return self._local.total_message_count()

    @property
    def forwarded_count(self) -> int:
        """Number of messages forwarded to remote nodes."""
        return self._forwarded_count

    @property
    def failed_forward_count(self) -> int:
        """Number of failed message forwards."""
        return self._failed_forwards

    def get_stats(self) -> dict:
        """Get router statistics."""
        return {
            "total_messages": self._local.total_message_count(),
            "forwarded": self._forwarded_count,
            "failed_forwards": self._failed_forwards,
        }
