"""Cluster membership protocol.

Implements a SWIM-inspired gossip protocol for node discovery,
failure detection, and membership state propagation.

Protocol overview:
1. Each node runs a periodic gossip loop (asyncio task).
2. Every gossip_interval, a random peer is pinged.
3. On ping failure, a ping-req is sent via a different peer.
4. If ping-req also fails, the suspect counter increments.
5. After suspicion_max failures, the node is marked DEAD.
6. Periodically, full membership state is synced to a random peer.
7. Incarnation numbers prevent stale state from overwriting fresh state.
8. Leader election runs on every membership change (deterministic bully).
"""

import asyncio
import logging
import random
import time
import uuid
from typing import Callable, Dict, List, Optional

from .config import ClusterConfig
from .election import LeaderElection
from .hash_ring import HashRing
from .node import ClusterNode, NodeStatus
from .transport import ClusterTransport

logger = logging.getLogger("aether-server.cluster.membership")


class ClusterMembership:
    """Manages cluster membership using a gossip protocol.
    
    Lifecycle:
        1. Create instance with ``ClusterConfig``.
        2. Call ``start()`` to begin the gossip loop.
        3. Use ``join()`` to connect to seed nodes.
        4. Use ``get_node(key)`` / ``get_nodes(key)`` for actor placement.
        5. Call ``stop()`` to leave the cluster gracefully.
    
    Args:
        config: Cluster configuration.
    """
    
    def __init__(self, config: ClusterConfig):
        self._config = config
        self._self: Optional[ClusterNode] = None
        self._members: Dict[str, ClusterNode] = {}
        self._suspect_counts: Dict[str, int] = {}
        self._ring = HashRing(virtual_nodes=config.virtual_nodes)
        self._transport: Optional[ClusterTransport] = None
        self._running = False
        self._task: Optional[asyncio.Task] = None
        
        self._on_node_join: Optional[Callable[[ClusterNode], None]] = None
        self._on_node_leave: Optional[Callable[[ClusterNode], None]] = None
        self._on_node_failure: Optional[Callable[[ClusterNode], None]] = None
        self._on_node_recover: Optional[Callable[[ClusterNode], None]] = None
        
        # Leader election (created on start when node_id is known)
        self._election: Optional[LeaderElection] = None
        
        self._lock = asyncio.Lock()
    
    # ============================================================
    # Properties
    # ============================================================
    
    @property
    def self_node(self) -> Optional[ClusterNode]:
        return self._self
    
    @property
    def node_id(self) -> str:
        return self._self.node_id if self._self else ""
    
    @property
    def is_running(self) -> bool:
        return self._running
    
    @property
    def member_count(self) -> int:
        return sum(1 for nid, n in self._members.items() if nid != self.node_id and n.is_alive())
    
    @property
    def alive_nodes(self) -> List[ClusterNode]:
        return [n for nid, n in self._members.items() if nid != self.node_id and n.is_alive()]
    
    @property
    def leader_id(self) -> Optional[str]:
        """The current cluster leader's node_id."""
        return self._election.leader_id if self._election else None
    
    @property
    def is_leader(self) -> bool:
        """Whether this node is the cluster leader."""
        return self._election.is_leader if self._election else False
    
    @property
    def election(self) -> Optional[LeaderElection]:
        """The leader election instance."""
        return self._election
    
    # ============================================================
    # Lifecycle
    # ============================================================
    
    async def start(self, host: str, api_port: int) -> ClusterNode:
        """Start the cluster membership protocol.
        
        Args:
            host: The host address this node advertises.
            api_port: The API port this node serves on.
            
        Returns:
            The local ``ClusterNode``.
        """
        node_id = self._config.node_id or str(uuid.uuid4())
        self._self = ClusterNode(
            node_id=node_id,
            host=host,
            gossip_port=self._config.gossip_port,
            api_port=api_port,
            status=NodeStatus.ALIVE,
            incarnation=0,
        )
        self._members[node_id] = self._self
        self._ring.add_node(self._self)
        
        self._transport = ClusterTransport(
            timeout=self._config.failure_timeout_seconds,
            cluster_secret=self._config.cluster_secret,
        )
        
        self._running = True
        self._task = asyncio.create_task(self._gossip_loop())
        
        # Initialize leader election
        self._election = LeaderElection(node_id=node_id)
        self._run_election()
        
        logger.info("Cluster membership started (node=%s, host=%s, port=%d)",
                     node_id, host, api_port)
        
        if self._config.seed_nodes:
            await self._join_seed_nodes()
        
        return self._self
    
    async def stop(self) -> None:
        """Leave the cluster gracefully."""
        if not self._running:
            return
        
        self._running = False
        
        if self._self:
            self._self.status = NodeStatus.LEAVING
            self._self.incarnation += 1
            await self._broadcast_leave()
        
        if self._task and not self._task.done():
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        
        if self._transport:
            self._transport.close()
            self._transport = None
        
        logger.info("Cluster membership stopped (node=%s)", self.node_id)
    
    # ============================================================
    # Node Registration
    # ============================================================
    
    def register_node(self, node: ClusterNode) -> bool:
        """Register or update a node in the membership table.
        
        Uses incarnation numbers to ensure newer information wins.
        
        Args:
            node: The node to register.
            
        Returns:
            True if the membership table was updated, False if ignored.
        """
        existing = self._members.get(node.node_id)
        
        if existing is None:
            self._members[node.node_id] = node
            if node.is_alive():
                self._ring.add_node(node)
                if self._on_node_join and node.node_id != self.node_id:
                    self._on_node_join(node)
            logger.info("Node registered: %s (%s)", node.node_id, node.status.value)
            self._run_election()
            return True
        
        if node.incarnation > existing.incarnation:
            old_status = existing.status
            self._members[node.node_id] = node
            
            if node.is_alive() and not old_status == NodeStatus.ALIVE:
                self._ring.add_node(node)
                self._suspect_counts.pop(node.node_id, None)
                if self._on_node_recover and node.node_id != self.node_id:
                    self._on_node_recover(node)
            elif not node.is_alive():
                self._ring.remove_node(node.node_id)
                if old_status == NodeStatus.ALIVE and self._on_node_failure:
                    self._on_node_failure(node)
            
            logger.info("Node updated: %s (%s -> %s, incarnation %d)",
                         node.node_id, old_status.value, node.status.value, node.incarnation)
            self._run_election()
            return True
        
        if node.incarnation == existing.incarnation:
            if node.is_alive() and not existing.is_alive():
                old_status = existing.status
                existing.status = node.status
                existing.last_heartbeat = node.last_heartbeat
                self._ring.add_node(existing)
                self._suspect_counts.pop(node.node_id, None)
                if self._on_node_recover and node.node_id != self.node_id:
                    self._on_node_recover(existing)
                logger.info("Node recovered: %s (%s -> %s)", node.node_id, old_status.value, node.status.value)
                self._run_election()
                return True
        
        return False
    
    def unregister_node(self, node_id: str) -> bool:
        """Remove a node from the cluster.
        
        Args:
            node_id: The node to remove.
            
        Returns:
            True if the node was found and removed.
        """
        node = self._members.pop(node_id, None)
        if node is None:
            return False
        self._ring.remove_node(node_id)
        self._suspect_counts.pop(node_id, None)
        if self._on_node_leave:
            self._on_node_leave(node)
        logger.info("Node removed: %s", node_id)
        self._run_election()
        return True
    
    # ============================================================
    # Hash Ring Delegation
    # ============================================================
    
    def get_node_for_key(self, key: str) -> Optional[ClusterNode]:
        """Find the node responsible for a key (e.g., actor_id)."""
        return self._ring.get_node(key)
    
    def get_nodes_for_key(self, key: str, count: int = 3) -> List[ClusterNode]:
        """Get N nodes for a key (for replication)."""
        return self._ring.get_nodes(key, count)
    
    def is_local_key(self, key: str) -> bool:
        """Check if a key should be handled by this node."""
        node = self._ring.get_node(key)
        return node is not None and node.node_id == self.node_id
    
    # ============================================================
    # Callbacks
    # ============================================================
    
    def on_node_join(self, callback: Callable[[ClusterNode], None]) -> None:
        self._on_node_join = callback
    
    def on_node_leave(self, callback: Callable[[ClusterNode], None]) -> None:
        self._on_node_leave = callback
    
    def on_node_failure(self, callback: Callable[[ClusterNode], None]) -> None:
        self._on_node_failure = callback
    
    def on_node_recover(self, callback: Callable[[ClusterNode], None]) -> None:
        self._on_node_recover = callback
    
    # ============================================================
    # Leader Election
    # ============================================================
    
    def _run_election(self) -> Optional[str]:
        """Run leader election based on current membership state.
        
        Should be called whenever the membership table changes.
        Returns the elected leader_id.
        """
        if self._election is None:
            return None
        return self._election.elect(self._members)
    
    # ============================================================
    # Gossip Loop
    # ============================================================
    
    async def _gossip_loop(self) -> None:
        """Main gossip loop — runs periodically while cluster is active."""
        while self._running:
            try:
                await asyncio.sleep(self._config.gossip_interval_seconds)
                if not self._running:
                    break
                
                alive_peers = [n for n in self._members.values()
                               if n.node_id != self.node_id and n.is_alive()]
                
                if not alive_peers:
                    continue
                
                target = random.choice(alive_peers)
                await self._probe_node(target)
                
                if random.random() < 0.1:
                    sync_target = random.choice(alive_peers)
                    await self._sync_with(sync_target)
                
                self._check_suspects()
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error("Gossip loop error: %s", e, exc_info=True)
    
    async def _probe_node(self, target: ClusterNode) -> None:
        """Probe a node with ping, and ping-req on failure."""
        if self._transport is None or self._self is None:
            return
        
        self._self.touch()
        response = self._transport.ping(
            target.host, target.api_port,
            self._self.to_dict(),
        )
        
        if response is not None:
            self._merge_remote_state(response.get("node"), response.get("nodes", {}))
            target.touch()
            return
        
        alive_peers = [n for n in self._members.values()
                       if n.node_id != self.node_id 
                       and n.node_id != target.node_id
                       and n.is_alive()]
        
        if alive_peers:
            intermediary = random.choice(alive_peers)
            response = self._transport.ping_request(
                intermediary.host, intermediary.api_port,
                target.host, target.api_port,
                self._self.to_dict(),
            )
            
            if response is not None:
                self._merge_remote_state(response.get("node"), response.get("nodes", {}))
                target.touch()
                return
        
        self._suspect_counts[target.node_id] = self._suspect_counts.get(target.node_id, 0) + 1
        count = self._suspect_counts[target.node_id]
        
        if count >= self._config.suspicion_max:
            target.status = NodeStatus.DEAD
            target.incarnation += 1
            self._ring.remove_node(target.node_id)
            self._suspect_counts.pop(target.node_id, None)
            if self._on_node_failure:
                self._on_node_failure(target)
            logger.warning("Node declared DEAD: %s (after %d failures)", target.node_id, count)
        else:
            target.status = NodeStatus.SUSPECT
            logger.info("Node SUSPECT: %s (failure %d/%d)", 
                         target.node_id, count, self._config.suspicion_max)
    
    async def _sync_with(self, target: ClusterNode) -> None:
        """Full membership state sync with a peer."""
        if self._transport is None or self._self is None:
            return
        
        nodes_dict = {nid: n.to_dict() for nid, n in self._members.items()}
        response = self._transport.sync(target.host, target.api_port, nodes_dict)
        
        if response is not None:
            remote_nodes = response.get("nodes", {})
            for node_id, node_data in remote_nodes.items():
                node = ClusterNode.from_dict(node_data)
                self.register_node(node)
            logger.debug("Synced %d nodes with %s", len(remote_nodes), target.node_id)
    
    async def _join_seed_nodes(self) -> None:
        """Contact seed nodes to join the cluster."""
        for seed in self._config.seed_nodes:
            parts = seed.rsplit(":", 1)
            host = parts[0]
            port = int(parts[1]) if len(parts) > 1 else 8080
            
            if self._transport is None or self._self is None:
                continue
            
            self._self.touch()
            response = self._transport.ping(host, port, self._self.to_dict())
            
            if response is not None:
                self._merge_remote_state(response.get("node"), response.get("nodes", {}))
                logger.info("Joined cluster via seed node %s:%d (received %d nodes)",
                             host, port, len(response.get("nodes", {})))
                self._run_election()
                break
            else:
                logger.warning("Failed to contact seed node %s:%d", host, port)
    
    async def _broadcast_leave(self) -> None:
        """Broadcast LEAVING status to all alive peers."""
        if self._transport is None or self._self is None:
            return
        
        peers = [n for n in self._members.values()
                 if n.node_id != self.node_id and n.is_alive()]
        
        for peer in peers:
            try:
                self._transport.ping(peer.host, peer.api_port, self._self.to_dict())
            except Exception:
                pass
    
    def _check_suspects(self) -> None:
        """Check if suspect nodes have exceeded the dead timeout."""
        now = time.time()
        to_mark_dead = []
        
        for node_id, node in self._members.items():
            if node.status == NodeStatus.SUSPECT:
                if now - node.last_heartbeat > self._config.dead_timeout_seconds:
                    to_mark_dead.append(node_id)
        
        for node_id in to_mark_dead:
            node = self._members[node_id]
            node.status = NodeStatus.DEAD
            node.incarnation += 1
            self._ring.remove_node(node_id)
            self._suspect_counts.pop(node_id, None)
            if self._on_node_failure:
                self._on_node_failure(node)
            logger.warning("Node DEAD (timeout): %s", node_id)
        
        # Re-run election if any nodes changed state
        if to_mark_dead:
            self._run_election()
    
    def _merge_remote_state(
        self,
        remote_node_data: Optional[Dict],
        remote_nodes: Dict[str, Dict],
    ) -> None:
        """Merge remote membership state into local state."""
        if remote_node_data:
            remote_node = ClusterNode.from_dict(remote_node_data)
            self.register_node(remote_node)
        
        for node_id, node_data in remote_nodes.items():
            if node_id != self.node_id:
                node = ClusterNode.from_dict(node_data)
                self.register_node(node)
    
    # ============================================================
    # Internal API Handlers
    # ============================================================
    
    def handle_ping(self, sender_data: Dict) -> Dict:
        """Handle an incoming ping from a remote node."""
        sender = ClusterNode.from_dict(sender_data)
        self.register_node(sender)
        
        if self._self:
            self._self.touch()
        
        nodes = {nid: n.to_dict() for nid, n in self._members.items()}
        return {
            "node": self._self.to_dict() if self._self else {},
            "nodes": nodes,
            "leader_id": self.leader_id,
        }
    
    def handle_ping_request(self, sender_data: Dict, target_data: Dict) -> Dict:
        """Handle a ping-req — probe a suspect on behalf of another node."""
        sender = ClusterNode.from_dict(sender_data)
        self.register_node(sender)
        
        if self._self:
            self._self.touch()
        
        target_host = target_data["host"]
        target_port = target_data["port"]
        probe_result = None
        
        if self._transport is not None and self._self is not None:
            probe_result = self._transport.ping(
                target_host, target_port,
                self._self.to_dict(),
            )
            if probe_result is not None:
                self._merge_remote_state(
                    probe_result.get("node"),
                    probe_result.get("nodes", {}),
                )
        
        nodes = {nid: n.to_dict() for nid, n in self._members.items()}
        return {
            "node": self._self.to_dict() if self._self else {},
            "nodes": nodes,
            "probe_ok": probe_result is not None,
            "leader_id": self.leader_id,
        }
    
    def handle_sync(self, remote_nodes: Dict[str, Dict]) -> Dict:
        """Handle a full membership sync from a remote node."""
        for node_id, node_data in remote_nodes.items():
            if node_id != self.node_id:
                node = ClusterNode.from_dict(node_data)
                self.register_node(node)
        
        if self._self:
            self._self.touch()
        
        nodes = {nid: n.to_dict() for nid, n in self._members.items()}
        return {
            "node": self._self.to_dict() if self._self else {},
            "nodes": nodes,
            "leader_id": self.leader_id,
        }
    
    # ============================================================
    # Query Methods
    # ============================================================
    
    def get_member(self, node_id: str) -> Optional[ClusterNode]:
        """Get a member node by ID."""
        return self._members.get(node_id)
    
    def get_members(self) -> Dict[str, ClusterNode]:
        """Get all members (including dead/suspect for debugging)."""
        return dict(self._members)
    
    def get_cluster_info(self) -> Dict:
        """Get cluster state summary."""
        alive = 0
        suspect = 0
        dead = 0
        leaving = 0
        for n in self._members.values():
            if n.status in (NodeStatus.ALIVE, NodeStatus.JOINING):
                alive += 1
            elif n.status == NodeStatus.SUSPECT:
                suspect += 1
            elif n.status == NodeStatus.DEAD:
                dead += 1
            elif n.status == NodeStatus.LEAVING:
                leaving += 1
        
        return {
            "node_id": self.node_id,
            "status": "running" if self._running else "stopped",
            "leader_id": self.leader_id,
            "is_leader": self.is_leader,
            "election_count": self._election.election_count if self._election else 0,
            "members": {
                "alive": alive,
                "suspect": suspect,
                "dead": dead,
                "leaving": leaving,
                "total": len(self._members),
            },
            "ring_nodes": self._ring.node_count,
        }
    
    # ============================================================
    # Leader Election Operations
    # ============================================================
    
    def leader_step_down(self) -> Dict:
        """Voluntarily step down if this node is the leader.
        
        Marks this node as stepped_down in the membership table so
        the flag propagates via gossip. Bumps incarnation to ensure
        the update wins conflicts. Returns a dict with previous_leader,
        new_leader, and was_leader.
        """
        previous_leader = self.leader_id
        was_leader = self.is_leader
        if self._election:
            self._election.step_down()
        # Mark self as stepped_down in membership so it propagates via gossip
        if self._self:
            self._self.stepped_down = True
            self._self.incarnation += 1  # Ensure gossip propagation
        new_leader = self._run_election()
        return {
            "previous_leader": previous_leader,
            "new_leader": new_leader,
            "was_leader": was_leader,
        }
    
    def leader_force(self, target_node_id: str) -> bool:
        """Force a specific node to be leader (admin operation).
        
        The target must be an alive member. This is local-only
        and does not propagate to other nodes.
        """
        if self._election:
            return self._election.force_leader(target_node_id, self._members)
        return False
    
    def get_leader_status(self) -> Dict:
        """Get leader election status."""
        if self._election:
            return self._election.get_status()
        return {
            "leader_id": None,
            "is_leader": False,
            "election_count": 0,
            "leader_incarnation": 0,
        }
