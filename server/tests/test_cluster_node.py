"""Tests for ClusterNode data class."""

import time

from server.cluster.node import ClusterNode, NodeStatus


class TestClusterNodeCreation:

    def test_default_creation(self):
        node = ClusterNode()
        assert node.node_id != ""
        assert node.host == "localhost"
        assert node.gossip_port == 7946
        assert node.api_port == 8080
        assert node.status == NodeStatus.JOINING
        assert node.actor_count == 0
        assert node.incarnation == 0

    def test_custom_creation(self):
        node = ClusterNode(
            node_id="test-node",
            host="10.0.0.1",
            gossip_port=9000,
            api_port=8080,
            status=NodeStatus.ALIVE,
        )
        assert node.node_id == "test-node"
        assert node.host == "10.0.0.1"
        assert node.gossip_port == 9000
        assert node.status == NodeStatus.ALIVE

    def test_default_last_heartbeat_is_recent(self):
        node = ClusterNode()
        assert time.time() - node.last_heartbeat < 1.0


class TestClusterNodeProperties:

    def test_gossip_address(self):
        node = ClusterNode(host="10.0.0.1", gossip_port=9000)
        assert node.gossip_address == "10.0.0.1:9000"

    def test_api_address(self):
        node = ClusterNode(host="10.0.0.1", api_port=8080)
        assert node.api_address == "10.0.0.1:8080"


class TestClusterNodeTouch:

    def test_touch_updates_heartbeat(self):
        node = ClusterNode()
        old = node.last_heartbeat
        time.sleep(0.01)
        node.touch()
        assert node.last_heartbeat > old


class TestClusterNodeIsAlive:

    def test_alive_status(self):
        node = ClusterNode(status=NodeStatus.ALIVE)
        assert node.is_alive() is True

    def test_joining_status(self):
        node = ClusterNode(status=NodeStatus.JOINING)
        assert node.is_alive() is True

    def test_suspect_status(self):
        node = ClusterNode(status=NodeStatus.SUSPECT)
        assert node.is_alive() is False

    def test_dead_status(self):
        node = ClusterNode(status=NodeStatus.DEAD)
        assert node.is_alive() is False

    def test_leaving_status(self):
        node = ClusterNode(status=NodeStatus.LEAVING)
        assert node.is_alive() is False


class TestClusterNodeSerialization:

    def test_to_dict_roundtrip(self):
        node = ClusterNode(
            node_id="test-1",
            host="10.0.0.1",
            gossip_port=9000,
            api_port=8080,
            status=NodeStatus.ALIVE,
            metadata={"region": "us-east"},
            actor_count=42,
            incarnation=3,
        )
        data = node.to_dict()
        assert data["node_id"] == "test-1"
        assert data["status"] == "alive"
        assert data["metadata"] == {"region": "us-east"}
        assert data["actor_count"] == 42

    def test_from_dict(self):
        data = {
            "node_id": "test-1",
            "host": "10.0.0.1",
            "gossip_port": 9000,
            "api_port": 8080,
            "status": "alive",
            "metadata": {"region": "us-east"},
            "actor_count": 42,
            "last_heartbeat": 1234567890.0,
            "incarnation": 3,
            "joined_at": 1234567880.0,
        }
        node = ClusterNode.from_dict(data)
        assert node.node_id == "test-1"
        assert node.status == NodeStatus.ALIVE
        assert node.metadata == {"region": "us-east"}
        assert node.incarnation == 3

    def test_to_dict_from_dict_roundtrip(self):
        original = ClusterNode(
            node_id="test-1",
            host="10.0.0.1",
            status=NodeStatus.ALIVE,
            metadata={"zone": "a"},
        )
        restored = ClusterNode.from_dict(original.to_dict())
        assert restored.node_id == original.node_id
        assert restored.host == original.host
        assert restored.status == original.status
        assert restored.metadata == original.metadata


class TestClusterNodeEquality:

    def test_same_node_id_equal(self):
        n1 = ClusterNode(node_id="same")
        n2 = ClusterNode(node_id="same")
        assert n1 == n2

    def test_different_node_id_not_equal(self):
        n1 = ClusterNode(node_id="a")
        n2 = ClusterNode(node_id="b")
        assert n1 != n2

    def test_hash_matches_equality(self):
        n1 = ClusterNode(node_id="same")
        n2 = ClusterNode(node_id="same")
        assert hash(n1) == hash(n2)
        assert len({n1, n2}) == 1  # set deduplication

    def test_not_equal_to_non_node(self):
        node = ClusterNode(node_id="test")
        assert node != "test"
        assert node != None
