"""Tests for cluster membership protocol."""

import asyncio
import time

import pytest

from server.cluster.config import ClusterConfig
from server.cluster.membership import ClusterMembership
from server.cluster.node import ClusterNode, NodeStatus


def _make_config(**kwargs) -> ClusterConfig:
    defaults = dict(
        enabled=True,
        gossip_interval_seconds=0.1,
        failure_timeout_seconds=0.5,
        dead_timeout_seconds=2.0,
        suspicion_max=2,
        virtual_nodes=100,
        cluster_secret="test-secret",
    )
    defaults.update(kwargs)
    return ClusterConfig(**defaults)


def _make_node(node_id: str, incarnation: int = 0, status: NodeStatus = NodeStatus.ALIVE) -> ClusterNode:
    return ClusterNode(
        node_id=node_id,
        host=f"host-{node_id}",
        api_port=8080,
        status=status,
        incarnation=incarnation,
    )


class TestMembershipInit:

    def test_create_membership(self):
        config = _make_config()
        membership = ClusterMembership(config)
        assert not membership.is_running
        assert membership.self_node is None
        assert membership.member_count == 0

    @pytest.mark.asyncio
    async def test_start_creates_self_node(self):
        config = _make_config(node_id="test-node")
        membership = ClusterMembership(config)
        node = await membership.start(host="10.0.0.1", api_port=8080)
        
        assert membership.is_running
        assert membership.node_id == "test-node"
        assert node.node_id == "test-node"
        assert node.status == NodeStatus.ALIVE
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_start_auto_generates_node_id(self):
        config = _make_config(node_id="")
        membership = ClusterMembership(config)
        node = await membership.start(host="10.0.0.1", api_port=8080)
        
        assert membership.node_id != ""
        assert len(membership.node_id) > 0
        
        await membership.stop()


class TestMembershipStop:

    @pytest.mark.asyncio
    async def test_stop_sets_not_running(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        await membership.stop()
        
        assert not membership.is_running

    @pytest.mark.asyncio
    async def test_double_stop_is_safe(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        await membership.stop()
        await membership.stop()


class TestNodeRegistration:

    @pytest.mark.asyncio
    async def test_register_new_node(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        node = _make_node("remote-1")
        result = membership.register_node(node)
        
        assert result is True
        assert membership.get_member("remote-1") is not None
        assert membership.member_count == 1
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_register_self_ignored_for_callback(self):
        config = _make_config(node_id="test-node")
        membership = ClusterMembership(config)
        joins = []
        membership.on_node_join(lambda n: joins.append(n.node_id))
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(membership.self_node)
        assert "test-node" not in joins
        
        await membership.stop()

    def test_higher_incarnation_wins(self):
        membership = ClusterMembership(_make_config())
        
        old = _make_node("n1", incarnation=1)
        new = _make_node("n1", incarnation=2)
        
        membership.register_node(old)
        result = membership.register_node(new)
        
        assert result is True
        assert membership.get_member("n1").incarnation == 2

    def test_lower_incarnation_ignored(self):
        membership = ClusterMembership(_make_config())
        
        new = _make_node("n1", incarnation=2)
        old = _make_node("n1", incarnation=1)
        
        membership.register_node(new)
        result = membership.register_node(old)
        
        assert result is False
        assert membership.get_member("n1").incarnation == 2

    def test_same_incarnation_status_recovery(self):
        membership = ClusterMembership(_make_config())
        
        suspect = _make_node("n1", incarnation=1, status=NodeStatus.SUSPECT)
        membership.register_node(suspect)
        assert membership.get_member("n1").status == NodeStatus.SUSPECT
        
        alive = _make_node("n1", incarnation=1, status=NodeStatus.ALIVE)
        result = membership.register_node(alive)
        
        assert result is True
        assert membership.get_member("n1").status == NodeStatus.ALIVE


class TestNodeRemoval:

    @pytest.mark.asyncio
    async def test_unregister_existing_node(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("remote-1"))
        result = membership.unregister_node("remote-1")
        
        assert result is True
        assert membership.get_member("remote-1") is None
        assert membership.member_count == 0
        
        await membership.stop()

    def test_unregister_nonexistent_returns_false(self):
        membership = ClusterMembership(_make_config())
        assert membership.unregister_node("ghost") is False


class TestHashRingIntegration:

    @pytest.mark.asyncio
    async def test_get_node_for_key(self):
        config = _make_config(node_id="test-node")
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("node-a"))
        membership.register_node(_make_node("node-b"))
        
        node = membership.get_node_for_key("actor-123")
        assert node is not None
        assert node.node_id in ("test-node", "node-a", "node-b")
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_is_local_key(self):
        config = _make_config(node_id="my-node")
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        assert membership.is_local_key("any-key") is True
        
        membership.register_node(_make_node("other"))
        node = membership.get_node_for_key("some-actor")
        assert membership.is_local_key("some-actor") == (node.node_id == "my-node")
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_dead_node_removed_from_ring(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("victim"))
        assert membership._ring.has_node("victim")
        
        dead = _make_node("victim", incarnation=1, status=NodeStatus.DEAD)
        membership.register_node(dead)
        assert not membership._ring.has_node("victim")
        
        await membership.stop()


class TestSuspectLifecycle:

    def test_check_suspects_marks_timed_out_as_dead(self):
        membership = ClusterMembership(_make_config(dead_timeout_seconds=0.1))
        
        suspect = _make_node("slow", incarnation=1, status=NodeStatus.SUSPECT)
        suspect.last_heartbeat = time.time() - 10.0
        membership.register_node(suspect)
        
        failures = []
        membership.on_node_failure(lambda n: failures.append(n.node_id))
        
        membership._check_suspects()
        
        assert membership.get_member("slow").status == NodeStatus.DEAD
        assert "slow" in failures

    def test_check_suspects_keeps_recent_suspects(self):
        membership = ClusterMembership(_make_config(dead_timeout_seconds=10.0))
        
        suspect = _make_node("recent", incarnation=1, status=NodeStatus.SUSPECT)
        suspect.last_heartbeat = time.time()
        membership.register_node(suspect)
        
        membership._check_suspects()
        
        assert membership.get_member("recent").status == NodeStatus.SUSPECT


class TestInternalHandlers:

    @pytest.mark.asyncio
    async def test_handle_ping(self):
        config = _make_config(node_id="test-node")
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        sender = _make_node("sender")
        response = membership.handle_ping(sender.to_dict())
        
        assert response["node"]["node_id"] == membership.node_id
        assert membership.node_id in response["nodes"]
        assert "sender" in response["nodes"]
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_handle_sync(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        remote_nodes = {
            "r1": _make_node("r1").to_dict(),
            "r2": _make_node("r2").to_dict(),
        }
        response = membership.handle_sync(remote_nodes)
        
        assert "r1" in response["nodes"]
        assert "r2" in response["nodes"]
        assert membership.get_member("r1") is not None
        assert membership.get_member("r2") is not None
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_handle_ping_request(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        sender = _make_node("requester")
        target = {"host": "10.0.0.5", "port": 8080}
        
        response = membership.handle_ping_request(sender.to_dict(), target)
        
        assert response["node"]["node_id"] == membership.node_id
        assert "probe_ok" in response
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_handle_sync_ignores_self_override(self):
        config = _make_config(node_id="my-node")
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        fake_self = ClusterNode(
            node_id="my-node",
            host="evil-host",
            api_port=9999,
            status=NodeStatus.DEAD,
            incarnation=999,
        ).to_dict()
        
        membership.handle_sync({"my-node": fake_self})
        
        assert membership.self_node.host == "10.0.0.1"
        assert membership.self_node.status == NodeStatus.ALIVE
        
        await membership.stop()


class TestCallbacks:

    @pytest.mark.asyncio
    async def test_on_node_join_callback(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        joins = []
        membership.on_node_join(lambda n: joins.append(n.node_id))
        
        membership.register_node(_make_node("new-node"))
        assert "new-node" in joins
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_on_node_failure_callback(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("victim"))
        
        failures = []
        membership.on_node_failure(lambda n: failures.append(n.node_id))
        
        dead = _make_node("victim", incarnation=1, status=NodeStatus.DEAD)
        membership.register_node(dead)
        assert "victim" in failures
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_on_node_recover_callback(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("phoenix", status=NodeStatus.SUSPECT))
        
        recoveries = []
        membership.on_node_recover(lambda n: recoveries.append(n.node_id))
        
        alive = _make_node("phoenix", status=NodeStatus.ALIVE)
        membership.register_node(alive)
        assert "phoenix" in recoveries
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_on_node_leave_callback(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("leaver"))
        
        leaves = []
        membership.on_node_leave(lambda n: leaves.append(n.node_id))
        
        membership.unregister_node("leaver")
        assert "leaver" in leaves
        
        await membership.stop()


class TestClusterInfo:

    @pytest.mark.asyncio
    async def test_cluster_info_empty(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        info = membership.get_cluster_info()
        assert info["node_id"] == membership.node_id
        assert info["status"] == "running"
        assert info["members"]["alive"] == 1
        assert info["members"]["total"] == 1
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_cluster_info_with_members(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("alive-1"))
        membership.register_node(_make_node("alive-2"))
        membership.register_node(_make_node("suspect-1", status=NodeStatus.SUSPECT))
        membership.register_node(_make_node("dead-1", incarnation=1, status=NodeStatus.DEAD))
        
        info = membership.get_cluster_info()
        assert info["members"]["alive"] == 3
        assert info["members"]["suspect"] == 1
        assert info["members"]["dead"] == 1
        assert info["members"]["total"] == 5
        assert info["ring_nodes"] == 3
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_cluster_info_stopped(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        await membership.stop()
        
        info = membership.get_cluster_info()
        assert info["status"] == "stopped"


class TestGetMembers:

    @pytest.mark.asyncio
    async def test_get_members_includes_all(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("a"))
        membership.register_node(_make_node("b"))
        
        members = membership.get_members()
        assert membership.node_id in members
        assert "a" in members
        assert "b" in members
        
        await membership.stop()

    @pytest.mark.asyncio
    async def test_alive_nodes_excludes_dead(self):
        config = _make_config()
        membership = ClusterMembership(config)
        await membership.start(host="10.0.0.1", api_port=8080)
        
        membership.register_node(_make_node("alive"))
        membership.register_node(_make_node("dead", incarnation=1, status=NodeStatus.DEAD))
        
        alive = membership.alive_nodes
        ids = [n.node_id for n in alive]
        assert "alive" in ids
        assert "dead" not in ids
        
        await membership.stop()
