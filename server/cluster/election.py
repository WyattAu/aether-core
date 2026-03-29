"""Leader election for Aether cluster.

Implements a deterministic bully algorithm: the alive node with the
lowest lexicographic node_id is always the leader. All nodes
independently compute the same leader from their membership view,
ensuring consistency without additional protocol rounds.

Properties:
- Deterministic: same membership state always produces the same leader
- Split-brain safe: uses incarnation + alive status from SWIM gossip
- Fast convergence: election happens immediately on membership change
- No additional network traffic: piggybacks on existing gossip protocol

The leader is useful for:
- Coordinating cluster-wide operations (rebalancing, migrations)
- Serving as the single source of truth for cluster metadata
- Breaking ties in distributed decisions
"""

import logging
import threading
from typing import Callable, Optional

from .node import ClusterNode, NodeStatus

logger = logging.getLogger("aether-server.cluster.election")


class LeaderElection:
    """Deterministic leader election based on lowest node_id.

    Each node runs an independent instance. When the membership
    table changes (join/leave/failure/recover), ``elect()`` is
    called to recompute the leader. All nodes with consistent
    membership views will agree on the same leader.

    Args:
        node_id: This node's ID (to check if we are the leader).
    """

    def __init__(self, node_id: str):
        self._node_id = node_id
        self._leader_id: Optional[str] = None
        self._leader_incarnation: int = 0
        self._election_count: int = 0
        self._lock = threading.Lock()
        self._stepped_down: bool = False  # True if this node voluntarily stepped down

        # Callbacks
        self._on_leader_change: Optional[Callable[[Optional[str], Optional[str]], None]] = None

    # ============================================================
    # Properties
    # ============================================================

    @property
    def leader_id(self) -> Optional[str]:
        """The current leader's node_id, or None if no leader."""
        return self._leader_id

    @property
    def is_leader(self) -> bool:
        """Whether this node is the current leader."""
        return self._leader_id == self._node_id and not self._stepped_down

    @property
    def election_count(self) -> int:
        """Total number of elections that have been run."""
        return self._election_count

    # ============================================================
    # Election
    # ============================================================

    def elect(self, members: dict) -> Optional[str]:
        """Run a leader election from the current membership table.

        Selects the alive node with the lowest lexicographic node_id,
        excluding nodes that have voluntarily stepped down.
        If no eligible nodes exist, returns None.

        Triggers the ``on_leader_change`` callback if the leader changes.

        Args:
            members: Dict mapping node_id -> ClusterNode.

        Returns:
            The elected leader's node_id, or None.
        """
        # Build set of node_ids that have stepped down.
        # Check both the local flag (for self, set by step_down()) and
        # the node attribute (propagated via gossip for remote nodes).
        stepped_down = set()
        if self._stepped_down:
            stepped_down.add(self._node_id)
        for nid, node in members.items():
            if getattr(node, 'stepped_down', False):
                stepped_down.add(nid)

        alive_ids = sorted(
            nid for nid, node in members.items()
            if node.is_alive() and nid not in stepped_down
        )

        if not alive_ids:
            new_leader = None
        else:
            new_leader = alive_ids[0]

        old_leader = self._leader_id

        with self._lock:
            self._election_count += 1
            self._leader_id = new_leader

            # Track incarnation of the leader node
            if new_leader and new_leader in members:
                self._leader_incarnation = members[new_leader].incarnation
            else:
                self._leader_incarnation = 0

        if new_leader != old_leader:
            logger.info(
                "Leader changed: %s -> %s (election #%d)",
                old_leader or "None", new_leader or "None",
                self._election_count,
            )
            if self._on_leader_change:
                try:
                    self._on_leader_change(old_leader, new_leader)
                except Exception as e:
                    logger.error("Leader change callback error: %s", e)

        return new_leader

    def force_leader(self, leader_id: str, members: dict) -> bool:
        """Force a specific node to be leader (used for testing / admin).

        The leader_id must be an alive member. This is a local-only
        operation — it does NOT propagate to other nodes.

        Args:
            leader_id: The node_id to make leader.
            members: Current membership table.

        Returns:
            True if the forced leader was accepted.
        """
        if leader_id not in members or not members[leader_id].is_alive():
            logger.warning("Cannot force leader: %s is not alive", leader_id)
            return False

        old_leader = self._leader_id
        with self._lock:
            self._election_count += 1
            self._leader_id = leader_id
            self._leader_incarnation = members[leader_id].incarnation
            self._stepped_down = False

        if leader_id != old_leader:
            logger.info(
                "Leader forced: %s -> %s (election #%d)",
                old_leader or "None", leader_id,
                self._election_count,
            )
            if self._on_leader_change:
                try:
                    self._on_leader_change(old_leader, leader_id)
                except Exception as e:
                    logger.error("Leader change callback error: %s", e)

        return True

    def step_down(self) -> Optional[str]:
        """Voluntarily step down if this node is the current leader.

        After stepping down, this node is excluded from the next election
        until ``elect()`` is called with membership changes. Returns the
        new leader, or None if no other alive nodes exist.

        This only affects the local node's state. Other nodes will
        detect the change via their next gossip cycle.
        """
        if not self.is_leader:
            return self._leader_id

        logger.info("Leader stepping down: %s", self._node_id)
        with self._lock:
            self._leader_incarnation = 0
            self._stepped_down = True
        # Note: _leader_id is intentionally NOT cleared here so that
        # the next elect() can compute the correct old_leader for
        # the on_leader_change callback.
        return None

    # ============================================================
    # Callbacks
    # ============================================================

    def on_leader_change(self, callback: Callable[[Optional[str], Optional[str]], None]) -> None:
        """Register a callback invoked when the leader changes.

        Args:
            callback: Function(old_leader_id, new_leader_id).
        """
        self._on_leader_change = callback

    # ============================================================
    # Query
    # ============================================================

    def get_status(self) -> dict:
        """Get the current election status."""
        return {
            "leader_id": self._leader_id,
            "is_leader": self.is_leader,
            "election_count": self._election_count,
            "leader_incarnation": self._leader_incarnation,
        }
