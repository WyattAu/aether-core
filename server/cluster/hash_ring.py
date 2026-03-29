"""Consistent hash ring for actor-to-node placement.

Uses virtual nodes (replicas) to ensure uniform distribution.
Supports addition and removal of nodes with minimal key migration.
"""

import bisect
import hashlib
from typing import Dict, List, Optional, Set, Tuple

from .node import ClusterNode


class HashRing:
    """Consistent hash ring for distributing actors across cluster nodes.
    
    Each physical node is mapped to ``virtual_nodes`` points on a 0..2^160
    ring (SHA-1 hash space). Actors are placed on the first node clockwise
    from their hash.
    
    When a node is added or removed, only keys in the affected arcs migrate.
    """
    
    def __init__(self, virtual_nodes: int = 150):
        """Initialize an empty hash ring.
        
        Args:
            virtual_nodes: Number of virtual nodes per physical node.
                Higher values give more uniform distribution at the cost
                of more memory and slightly slower lookups.
        """
        self._virtual_nodes = virtual_nodes
        self._ring: List[int] = []          # sorted list of hash values
        self._ring_nodes: Dict[int, ClusterNode] = {}  # hash -> node
        self._nodes: Dict[str, ClusterNode] = {}        # node_id -> node
    
    @property
    def node_count(self) -> int:
        """Number of physical nodes on the ring."""
        return len(self._nodes)
    
    @property
    def virtual_node_count(self) -> int:
        """Total number of virtual nodes on the ring."""
        return len(self._ring)
    
    def add_node(self, node: ClusterNode) -> int:
        """Add a physical node to the ring.
        
        Creates ``virtual_nodes`` virtual nodes for the given node,
        each placed at ``SHA-1(node_id:replica_index)``.
        
        Args:
            node: The cluster node to add.
            
        Returns:
            Number of virtual nodes added.
        """
        if node.node_id in self._nodes:
            return 0
        
        self._nodes[node.node_id] = node
        added = 0
        for i in range(self._virtual_nodes):
            key = self._hash(f"{node.node_id}:{i}")
            # Avoid hash collisions (extremely unlikely but safe)
            if key not in self._ring_nodes:
                self._ring_nodes[key] = node
                bisect.insort(self._ring, key)
                added += 1
        return added
    
    def remove_node(self, node_id: str) -> int:
        """Remove a physical node and all its virtual nodes from the ring.
        
        Args:
            node_id: The ID of the node to remove.
            
        Returns:
            Number of virtual nodes removed, or 0 if node not found.
        """
        node = self._nodes.pop(node_id, None)
        if node is None:
            return 0
        
        removed = 0
        keys_to_remove = [
            key for key, n in self._ring_nodes.items()
            if n.node_id == node_id
        ]
        for key in keys_to_remove:
            del self._ring_nodes[key]
            index = bisect.bisect_left(self._ring, key)
            if index < len(self._ring) and self._ring[index] == key:
                self._ring.pop(index)
            removed += 1
        return removed
    
    def get_node(self, key: str) -> Optional[ClusterNode]:
        """Find the node responsible for a given key.
        
        Uses binary search to find the first virtual node clockwise
        from the key's hash. Wraps around if necessary.
        
        Args:
            key: The key to look up (e.g., actor_id).
            
        Returns:
            The responsible ``ClusterNode``, or ``None`` if the ring is empty.
        """
        if not self._ring:
            return None
        
        h = self._hash(key)
        index = bisect.bisect_right(self._ring, h)
        if index >= len(self._ring):
            index = 0  # wrap around
        
        return self._ring_nodes[self._ring[index]]
    
    def get_nodes(self, key: str, count: int = 3) -> List[ClusterNode]:
        """Get the N distinct physical nodes responsible for a key.
        
        Useful for replication: place replicas on the next N nodes
        clockwise from the primary.
        
        Args:
            key: The key to look up.
            count: Number of distinct nodes to return.
            
        Returns:
            List of distinct ``ClusterNode`` instances (may be fewer
            than ``count`` if the ring has fewer physical nodes).
        """
        if not self._ring:
            return []
        
        count = min(count, len(self._nodes))
        result: List[ClusterNode] = []
        seen: Set[str] = set()
        
        h = self._hash(key)
        index = bisect.bisect_right(self._ring, h)
        
        while len(result) < count:
            if index >= len(self._ring):
                index = 0
            node = self._ring_nodes[self._ring[index]]
            if node.node_id not in seen:
                seen.add(node.node_id)
                result.append(node)
            index += 1
        
        return result
    
    def get_all_nodes(self) -> List[ClusterNode]:
        """Return all physical nodes on the ring."""
        return list(self._nodes.values())
    
    def has_node(self, node_id: str) -> bool:
        """Check if a node is on the ring."""
        return node_id in self._nodes
    
    def get_partition_stats(self, num_keys: int = 10000) -> Dict[str, int]:
        """Approximate key distribution across nodes.
        
        Hashes ``num_keys`` sequential integers and counts how many
        land on each physical node. Useful for testing distribution
        uniformity.
        
        Args:
            num_keys: Number of test keys to hash.
            
        Returns:
            Dict mapping ``node_id`` to key count.
        """
        counts: Dict[str, int] = {nid: 0 for nid in self._nodes}
        for i in range(num_keys):
            node = self.get_node(str(i))
            if node is not None:
                counts[node.node_id] += 1
        return counts
    
    @staticmethod
    def _hash(key: str) -> int:
        """Hash a key to a 160-bit integer using SHA-1.
        
        Args:
            key: The string to hash.
            
        Returns:
            Integer in range 0..2^160-1.
        """
        return int(hashlib.sha1(key.encode("utf-8")).hexdigest(), 16)
