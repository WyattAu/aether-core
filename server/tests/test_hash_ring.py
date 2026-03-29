"""Tests for the consistent hash ring."""

import pytest

from server.cluster.hash_ring import HashRing
from server.cluster.node import ClusterNode, NodeStatus


def _make_node(node_id: str, host: str = "localhost", port: int = 8080) -> ClusterNode:
    return ClusterNode(node_id=node_id, host=host, api_port=port)


class TestHashRingEmpty:
    """Tests for an empty ring."""

    def test_empty_ring_has_no_nodes(self):
        ring = HashRing()
        assert ring.node_count == 0
        assert ring.virtual_node_count == 0

    def test_get_node_returns_none_on_empty(self):
        ring = HashRing()
        assert ring.get_node("any-key") is None

    def test_get_nodes_returns_empty_on_empty(self):
        ring = HashRing()
        assert ring.get_nodes("any-key", 3) == []

    def test_get_all_nodes_returns_empty(self):
        ring = HashRing()
        assert ring.get_all_nodes() == []

    def test_has_node_returns_false(self):
        ring = HashRing()
        assert ring.has_node("nonexistent") is False


class TestHashRingAddNode:
    """Tests for adding nodes to the ring."""

    def test_add_single_node(self):
        ring = HashRing(virtual_nodes=100)
        node = _make_node("node-1")
        added = ring.add_node(node)
        assert added == 100
        assert ring.node_count == 1
        assert ring.virtual_node_count == 100

    def test_add_multiple_nodes(self):
        ring = HashRing(virtual_nodes=50)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        ring.add_node(_make_node("node-3"))
        assert ring.node_count == 3
        assert ring.virtual_node_count == 150

    def test_add_duplicate_node_ignored(self):
        ring = HashRing(virtual_nodes=100)
        node = _make_node("node-1")
        ring.add_node(node)
        added = ring.add_node(node)
        assert added == 0
        assert ring.node_count == 1

    def test_add_different_nodes_same_host(self):
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-a", host="10.0.0.1"))
        ring.add_node(_make_node("node-b", host="10.0.0.1"))
        assert ring.node_count == 2


class TestHashRingRemoveNode:
    """Tests for removing nodes from the ring."""

    def test_remove_existing_node(self):
        ring = HashRing(virtual_nodes=100)
        node = _make_node("node-1")
        ring.add_node(node)
        removed = ring.remove_node("node-1")
        assert removed == 100
        assert ring.node_count == 0

    def test_remove_nonexistent_node(self):
        ring = HashRing()
        removed = ring.remove_node("ghost")
        assert removed == 0

    def test_remove_one_of_many(self):
        ring = HashRing(virtual_nodes=50)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        ring.add_node(_make_node("node-3"))
        removed = ring.remove_node("node-2")
        assert removed == 50
        assert ring.node_count == 2
        assert ring.has_node("node-1")
        assert ring.has_node("node-3")


class TestHashRingGetNode:
    """Tests for key-to-node lookup."""

    def test_single_node_gets_all_keys(self):
        ring = HashRing(virtual_nodes=100)
        node = _make_node("node-1")
        ring.add_node(node)
        for i in range(100):
            result = ring.get_node(f"actor-{i}")
            assert result is not None
            assert result.node_id == "node-1"

    def test_two_nodes_distribute_keys(self):
        ring = HashRing(virtual_nodes=200)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        
        counts = {"node-1": 0, "node-2": 0}
        for i in range(1000):
            node = ring.get_node(f"actor-{i}")
            counts[node.node_id] += 1
        
        # Both nodes should get some keys (probabilistic but very likely)
        assert counts["node-1"] > 0
        assert counts["node-2"] > 0

    def test_deterministic_lookup(self):
        """Same key always maps to same node (as long as ring doesn't change)."""
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        ring.add_node(_make_node("node-3"))
        
        for key in ["actor-1", "actor-2", "actor-3", "some-id", "another-id"]:
            first = ring.get_node(key)
            for _ in range(50):
                assert ring.get_node(key).node_id == first.node_id

    def test_empty_key_works(self):
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-1"))
        result = ring.get_node("")
        assert result is not None


class TestHashRingGetNodes:
    """Tests for getting multiple replica nodes."""

    def test_single_node_returns_itself(self):
        ring = HashRing(virtual_nodes=100)
        node = _make_node("node-1")
        ring.add_node(node)
        nodes = ring.get_nodes("key", count=3)
        assert len(nodes) == 1
        assert nodes[0].node_id == "node-1"

    def test_two_nodes_distinct_replicas(self):
        ring = HashRing(virtual_nodes=200)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        nodes = ring.get_nodes("key", count=2)
        assert len(nodes) == 2
        assert nodes[0].node_id != nodes[1].node_id

    def test_count_clamped_to_node_count(self):
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        nodes = ring.get_nodes("key", count=10)
        assert len(nodes) == 2

    def test_replicas_are_consistent(self):
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        ring.add_node(_make_node("node-3"))
        
        first = ring.get_nodes("key", count=3)
        for _ in range(50):
            assert [n.node_id for n in ring.get_nodes("key", count=3)] == [n.node_id for n in first]


class TestHashRingDistribution:
    """Tests for key distribution uniformity."""

    def test_uniform_distribution_three_nodes(self):
        """With 3 nodes and 150 virtual nodes each, distribution should be roughly uniform."""
        ring = HashRing(virtual_nodes=150)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        ring.add_node(_make_node("node-3"))
        
        stats = ring.get_partition_stats(num_keys=10000)
        
        # Each node should get roughly 1/3 of keys (±20% tolerance)
        expected = 10000 / 3
        for node_id, count in stats.items():
            assert 0.6 * expected < count < 1.4 * expected, (
                f"Node {node_id} got {count} keys, expected ~{expected}"
            )

    def test_uniform_distribution_ten_nodes(self):
        ring = HashRing(virtual_nodes=150)
        for i in range(10):
            ring.add_node(_make_node(f"node-{i}"))
        
        stats = ring.get_partition_stats(num_keys=10000)
        
        expected = 1000
        for node_id, count in stats.items():
            assert 0.5 * expected < count < 1.5 * expected, (
                f"Node {node_id} got {count} keys, expected ~{expected}"
            )

    def test_minimal_relocation_on_removal(self):
        """Removing one node should only relocate ~1/N of keys."""
        ring = HashRing(virtual_nodes=150)
        for i in range(5):
            ring.add_node(_make_node(f"node-{i}"))
        
        # Record where 10000 keys land
        before = {}
        for i in range(10000):
            node = ring.get_node(f"key-{i}")
            before[i] = node.node_id
        
        # Remove node-2
        ring.remove_node("node-2")
        
        # Count how many keys moved
        moved = 0
        for i in range(10000):
            node = ring.get_node(f"key-{i}")
            if node.node_id != before[i]:
                moved += 1
        
        # Only ~1/5 of keys should have moved
        relocation_rate = moved / 10000
        assert relocation_rate < 0.35, f"Relocation rate {relocation_rate:.2%} too high, expected ~20%"

    def test_minimal_relocation_on_addition(self):
        """Adding a node should only relocate ~1/N of keys."""
        ring = HashRing(virtual_nodes=150)
        for i in range(4):
            ring.add_node(_make_node(f"node-{i}"))
        
        before = {}
        for i in range(10000):
            node = ring.get_node(f"key-{i}")
            before[i] = node.node_id
        
        ring.add_node(_make_node("node-4"))
        
        moved = 0
        for i in range(10000):
            node = ring.get_node(f"key-{i}")
            if node.node_id != before[i]:
                moved += 1
        
        relocation_rate = moved / 10000
        assert relocation_rate < 0.35, f"Relocation rate {relocation_rate:.2%} too high, expected ~20%"


class TestHashRingEdgeCases:
    """Edge case tests."""

    def test_many_virtual_nodes(self):
        ring = HashRing(virtual_nodes=1000)
        ring.add_node(_make_node("node-1"))
        assert ring.virtual_node_count == 1000

    def test_few_virtual_nodes(self):
        ring = HashRing(virtual_nodes=1)
        ring.add_node(_make_node("node-1"))
        assert ring.virtual_node_count == 1

    def test_has_node(self):
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-1"))
        assert ring.has_node("node-1") is True
        assert ring.has_node("node-2") is False

    def test_get_all_nodes(self):
        ring = HashRing(virtual_nodes=100)
        ring.add_node(_make_node("node-1"))
        ring.add_node(_make_node("node-2"))
        nodes = ring.get_all_nodes()
        assert len(nodes) == 2
        ids = {n.node_id for n in nodes}
        assert ids == {"node-1", "node-2"}
