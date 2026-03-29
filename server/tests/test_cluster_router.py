"""Tests for the cluster-aware message router."""

from unittest.mock import MagicMock

import pytest

from server.cluster.config import ClusterConfig
from server.cluster.membership import ClusterMembership
from server.cluster.node import ClusterNode, NodeStatus
from server.cluster.router import ClusterRouter
from server.cluster.transport import ClusterTransport
from server.message_router import MessageRouter
from server.models import DeliveryReceipt, MessageEnvelope


def _make_config(**kwargs) -> ClusterConfig:
    defaults = dict(
        enabled=True,
        gossip_interval_seconds=10.0,
        failure_timeout_seconds=5.0,
        dead_timeout_seconds=30.0,
        suspicion_max=3,
        virtual_nodes=100,
    )
    defaults.update(kwargs)
    return ClusterConfig(**defaults)


def _make_envelope(target: str = "actor-1", source: str = "sender") -> MessageEnvelope:
    return MessageEnvelope(
        source_actor=source,
        target_actor=target,
        message_type="test",
        payload={"data": 42},
    )


@pytest.fixture
async def cluster_setup():
    """Create a ClusterMembership and ClusterRouter for testing."""
    config = _make_config(node_id="local-node")
    membership = ClusterMembership(config)
    await membership.start(host="10.0.0.1", api_port=8080)

    local_router = MessageRouter()
    transport = ClusterTransport(timeout=1.0)
    cluster_router = ClusterRouter(local_router, membership, transport)

    yield membership, cluster_router, local_router, transport

    await membership.stop()
    transport.close()


class TestClusterRouterLocalDelivery:

    @pytest.mark.asyncio
    async def test_local_handler_receives_message(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        handler_called = []
        async def handler(env):
            handler_called.append(env)

        local.register_handler("local-actor", handler)

        envelope = _make_envelope(target="local-actor")
        receipt = await router.route(envelope)

        assert receipt.status == "delivered"
        assert len(handler_called) == 1

    @pytest.mark.asyncio
    async def test_no_handler_buffers_locally(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        envelope = _make_envelope(target="unknown")
        receipt = await router.route(envelope)

        assert receipt.status == "buffered"


class TestClusterRouterForwarding:

    @pytest.mark.asyncio
    async def test_message_forwarded_to_remote_node(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        remote = ClusterNode(
            node_id="remote-node",
            host="10.0.0.2",
            api_port=8080,
            status=NodeStatus.ALIVE,
        )
        membership.register_node(remote)

        transport.forward_message = MagicMock(return_value={"status": "ok"})
        membership.get_node_for_key = MagicMock(return_value=remote)

        envelope = _make_envelope(target="remote-actor")
        receipt = await router.route(envelope)

        assert receipt.status == "forwarded"
        assert router.forwarded_count == 1
        transport.forward_message.assert_called_once()

    @pytest.mark.asyncio
    async def test_forward_failure_returns_failed(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        remote = ClusterNode(
            node_id="remote-node",
            host="10.0.0.2",
            api_port=8080,
            status=NodeStatus.ALIVE,
        )
        membership.register_node(remote)

        transport.forward_message = MagicMock(return_value=None)
        membership.get_node_for_key = MagicMock(return_value=remote)

        envelope = _make_envelope(target="remote-actor")
        receipt = await router.route(envelope)

        assert receipt.status == "failed"
        assert router.failed_forward_count == 1

    @pytest.mark.asyncio
    async def test_local_owner_buffers_when_no_handler(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        membership.get_node_for_key = MagicMock(return_value=membership.self_node)

        envelope = _make_envelope(target="orphan-actor")
        receipt = await router.route(envelope)

        assert receipt.status == "buffered"


class TestClusterRouterDelegation:

    @pytest.mark.asyncio
    async def test_register_handler_delegates(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        handler = lambda env: None
        router.register_handler("actor-1", handler)
        assert "actor-1" in local._handlers

    @pytest.mark.asyncio
    async def test_unregister_handler_delegates(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        router.register_handler("actor-1", lambda env: None)
        router.unregister_handler("actor-1")
        assert "actor-1" not in local._handlers

    @pytest.mark.asyncio
    async def test_get_pending_delegates(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        envelope = _make_envelope(target="buffered")
        await router.route(envelope)

        pending = router.get_pending_messages("buffered")
        assert len(pending) == 1

    @pytest.mark.asyncio
    async def test_total_message_count(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        envelope = _make_envelope(target="buffered")
        await router.route(envelope)

        assert router.total_message_count() == 1


class TestClusterRouterStats:

    @pytest.mark.asyncio
    async def test_get_stats(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        stats = router.get_stats()
        assert stats["total_messages"] == 0
        assert stats["forwarded"] == 0
        assert stats["failed_forwards"] == 0

    @pytest.mark.asyncio
    async def test_stats_after_forward(self, cluster_setup):
        membership, router, local, transport = cluster_setup

        remote = ClusterNode(node_id="r", host="10.0.0.2", api_port=8080, status=NodeStatus.ALIVE)
        membership.register_node(remote)
        transport.forward_message = MagicMock(return_value={"status": "ok"})
        membership.get_node_for_key = MagicMock(return_value=remote)

        await router.route(_make_envelope(target="remote"))

        stats = router.get_stats()
        assert stats["forwarded"] == 1
