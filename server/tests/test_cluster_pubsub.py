"""Unit tests for ClusterPubSub — cluster-aware pub/sub fan-out."""

import pytest
from unittest.mock import MagicMock, patch, call

from server.pubsub_service import PubSubService
from server.cluster.pubsub import ClusterPubSub
from server.cluster.node import ClusterNode, NodeStatus


# ============================================================
# Fixtures
# ============================================================

@pytest.fixture
def local_pubsub():
    return PubSubService(history_size=50)


@pytest.fixture
def mock_membership():
    membership = MagicMock()
    membership.is_running = True
    membership.alive_nodes = []
    membership.node_id = "local-node-1"
    return membership


@pytest.fixture
def mock_transport():
    transport = MagicMock()
    transport.forward_pubsub.return_value = {"local_subscriber_count": 0}
    return transport


@pytest.fixture
def cluster_pubsub(local_pubsub, mock_membership, mock_transport):
    return ClusterPubSub(
        local_pubsub=local_pubsub,
        membership=mock_membership,
        transport=mock_transport,
        node_id="local-node-1",
    )


def _make_peer(node_id, host="10.0.0.2", port=8080):
    return ClusterNode(
        node_id=node_id,
        host=host,
        gossip_port=port + 1000,
        api_port=port,
        status=NodeStatus.ALIVE,
        incarnation=0,
    )


# ============================================================
# Local delegation tests (subscribe, unsubscribe, list, history)
# ============================================================

class TestLocalDelegation:
    """Tests that ClusterPubSub delegates local operations to PubSubService."""

    def test_subscribe_returns_id(self, cluster_pubsub):
        sub_id = cluster_pubsub.subscribe("orders", "handler-1")
        assert sub_id.startswith("sub_")

    def test_subscribe_with_filter(self, cluster_pubsub):
        sub_id = cluster_pubsub.subscribe("orders.*", "handler-1", filter="type == 'created'")
        assert sub_id.startswith("sub_")

    def test_unsubscribe(self, cluster_pubsub):
        sub_id = cluster_pubsub.subscribe("topic-x", "h1")
        assert cluster_pubsub.unsubscribe(sub_id) is True
        assert cluster_pubsub.unsubscribe(sub_id) is False

    def test_list_topics(self, cluster_pubsub):
        cluster_pubsub.subscribe("topic-a", "h1")
        cluster_pubsub.subscribe("topic-b", "h2")
        topics = cluster_pubsub.list_topics()
        assert sorted(topics) == ["topic-a", "topic-b"]

    def test_list_subscribers(self, cluster_pubsub):
        cluster_pubsub.subscribe("orders", "h1")
        cluster_pubsub.subscribe("orders", "h2")
        subs = cluster_pubsub.list_subscribers("orders")
        assert sorted(subs) == ["h1", "h2"]

    def test_list_subscribers_empty(self, cluster_pubsub):
        assert cluster_pubsub.list_subscribers("nonexistent") == []

    def test_get_history(self, cluster_pubsub):
        cluster_pubsub.publish("hist-topic", payload="msg1")
        cluster_pubsub.publish("hist-topic", payload="msg2")
        history = cluster_pubsub.get_history("hist-topic")
        assert len(history) == 2
        assert history[0].payload == "msg1"
        assert history[1].payload == "msg2"

    def test_get_history_empty(self, cluster_pubsub):
        assert cluster_pubsub.get_history("nonexistent") == []

    def test_get_matching_subscribers_with_wildcard(self, cluster_pubsub):
        cluster_pubsub.subscribe("orders.*", "wildcard-handler")
        cluster_pubsub.subscribe("orders.created", "exact-handler")
        matches = cluster_pubsub.get_matching_subscribers("orders.created")
        ids = [s.subscriber_id for s in matches]
        assert "wildcard-handler" in ids
        assert "exact-handler" in ids

    def test_get_matching_subscribers_no_match(self, cluster_pubsub):
        cluster_pubsub.subscribe("orders.*", "h1")
        matches = cluster_pubsub.get_matching_subscribers("payments.done")
        assert len(matches) == 0


# ============================================================
# Publish tests (local delivery)
# ============================================================

class TestPublish:
    """Tests that publish delivers locally and returns correct counts."""

    def test_publish_no_subscribers(self, cluster_pubsub):
        count = cluster_pubsub.publish("empty-topic", payload="hello")
        assert count == 0

    def test_publish_with_subscribers(self, cluster_pubsub):
        cluster_pubsub.subscribe("topic-a", "h1")
        cluster_pubsub.subscribe("topic-a", "h2")
        count = cluster_pubsub.publish("topic-a", payload="hello")
        assert count == 2

    def test_publish_with_handler(self, cluster_pubsub):
        cluster_pubsub.subscribe("topic-h", "h1")
        results = []
        handler = lambda sid, msg: results.append((sid, msg.payload))
        count = cluster_pubsub.publish_with_handler(
            "topic-h", handler, payload="test-payload",
        )
        assert count == 1
        assert len(results) == 1
        assert results[0] == ("h1", "test-payload")

    def test_publish_with_handler_exception_ignored(self, cluster_pubsub):
        cluster_pubsub.subscribe("topic-err", "h1")
        cluster_pubsub.subscribe("topic-err", "h2")
        bad_handler = lambda sid, msg: (_ for _ in ()).throw(RuntimeError("boom"))
        count = cluster_pubsub.publish_with_handler(
            "topic-err", bad_handler, payload="x",
        )
        # Both subscribers are counted even if handler throws
        assert count == 2

    def test_publish_with_headers(self, cluster_pubsub):
        cluster_pubsub.subscribe("headers-topic", "h1")
        count = cluster_pubsub.publish(
            "headers-topic",
            payload="data",
            headers={"x-trace-id": "abc-123"},
        )
        assert count == 1
        history = cluster_pubsub.get_history("headers-topic")
        assert history[-1].headers == {"x-trace-id": "abc-123"}


# ============================================================
# Fan-out tests (cross-node delivery)
# ============================================================

class TestFanOut:
    """Tests that publish fans out to alive cluster peers."""

    def test_fan_out_to_single_peer(self, cluster_pubsub, mock_membership, mock_transport):
        peer = _make_peer("peer-1")
        mock_membership.alive_nodes = [peer]
        mock_transport.forward_pubsub.return_value = {"local_subscriber_count": 3}

        cluster_pubsub.publish("fanout-topic", payload="hello")

        mock_transport.forward_pubsub.assert_called_once()
        call_args = mock_transport.forward_pubsub.call_args
        assert call_args[0][0] == peer.host
        assert call_args[0][1] == peer.api_port
        assert call_args[0][2]["topic"] == "fanout-topic"
        assert call_args[0][2]["payload"] == "hello"
        assert call_args[0][2]["source_node_id"] == "local-node-1"

    def test_fan_out_to_multiple_peers(self, cluster_pubsub, mock_membership, mock_transport):
        peers = [_make_peer(f"peer-{i}") for i in range(3)]
        mock_membership.alive_nodes = peers

        cluster_pubsub.publish("multi-topic", payload="data")

        assert mock_transport.forward_pubsub.call_count == 3

    def test_fan_out_skips_self(self, cluster_pubsub, mock_membership, mock_transport):
        # Simulate a cluster where this node is in the alive list
        self_node = _make_peer("local-node-1", host="10.0.0.1", port=8080)
        peers = [_make_peer("peer-1"), self_node]
        mock_membership.alive_nodes = peers

        cluster_pubsub.publish("self-skip-topic", payload="data")

        # Should only call once (skipping self)
        assert mock_transport.forward_pubsub.call_count == 1

    def test_fan_out_not_running(self, cluster_pubsub, mock_membership, mock_transport):
        mock_membership.is_running = False
        mock_membership.alive_nodes = [_make_peer("peer-1")]

        cluster_pubsub.publish("stopped-topic", payload="data")

        mock_transport.forward_pubsub.assert_not_called()

    def test_fan_out_no_peers(self, cluster_pubsub, mock_membership, mock_transport):
        mock_membership.alive_nodes = []

        cluster_pubsub.publish("solo-topic", payload="data")

        mock_transport.forward_pubsub.assert_not_called()

    def test_fan_out_error_is_logged(self, cluster_pubsub, mock_membership, mock_transport):
        peer = _make_peer("peer-err")
        mock_membership.alive_nodes = [peer]
        mock_transport.forward_pubsub.return_value = None

        cluster_pubsub.publish("err-topic", payload="data")

        stats = cluster_pubsub.get_stats()
        assert stats["fan_out_errors"] == 1

    def test_fan_out_exception_is_caught(self, cluster_pubsub, mock_membership, mock_transport):
        peer = _make_peer("peer-exc")
        mock_membership.alive_nodes = [peer]
        mock_transport.forward_pubsub.side_effect = ConnectionError("network down")

        cluster_pubsub.publish("exc-topic", payload="data")

        stats = cluster_pubsub.get_stats()
        assert stats["fan_out_errors"] == 1

    def test_fan_out_tracks_remote_delivered(self, cluster_pubsub, mock_membership, mock_transport):
        peers = [_make_peer("peer-a"), _make_peer("peer-b")]
        mock_membership.alive_nodes = peers
        mock_transport.forward_pubsub.return_value = {"local_subscriber_count": 5}

        cluster_pubsub.publish("count-topic", payload="data")

        stats = cluster_pubsub.get_stats()
        assert stats["remote_delivered"] == 10  # 2 peers x 5 subscribers


# ============================================================
# Remote publish handler
# ============================================================

class TestRemotePublish:
    """Tests handling of publishes forwarded from remote nodes."""

    def test_handle_remote_publish_delivers_locally(self, cluster_pubsub):
        cluster_pubsub.subscribe("remote-topic", "local-h1")
        cluster_pubsub.subscribe("remote-topic", "local-h2")

        count = cluster_pubsub.handle_remote_publish(
            topic="remote-topic",
            payload="from-peer",
            source_node_id="peer-1",
        )

        assert count == 2

    def test_handle_remote_publish_no_subscribers(self, cluster_pubsub):
        count = cluster_pubsub.handle_remote_publish(
            topic="nobody-home",
            payload="echo",
            source_node_id="peer-1",
        )
        assert count == 0

    def test_handle_remote_publish_with_headers(self, cluster_pubsub):
        cluster_pubsub.subscribe("hdr-topic", "h1")
        count = cluster_pubsub.handle_remote_publish(
            topic="hdr-topic",
            payload="data",
            headers={"x-source": "peer-2"},
        )
        assert count == 1
        history = cluster_pubsub.get_history("hdr-topic")
        assert history[-1].headers == {"x-source": "peer-2"}

    def test_remote_publish_records_in_history(self, cluster_pubsub):
        cluster_pubsub.handle_remote_publish(topic="hist-remote", payload="msg1")
        cluster_pubsub.handle_remote_publish(topic="hist-remote", payload="msg2")
        history = cluster_pubsub.get_history("hist-remote")
        assert len(history) == 2


# ============================================================
# Statistics
# ============================================================

class TestStats:
    """Tests for ClusterPubSub.get_stats()."""

    def test_initial_stats(self, cluster_pubsub):
        stats = cluster_pubsub.get_stats()
        assert stats["local_topics"] == 0
        assert stats["local_subscriptions"] == 0
        assert stats["fan_out_count"] == 0
        assert stats["fan_out_errors"] == 0
        assert stats["remote_delivered"] == 0
        assert stats["cluster_peers"] == 0

    def test_stats_after_subscribe(self, cluster_pubsub):
        cluster_pubsub.subscribe("t1", "h1")
        cluster_pubsub.subscribe("t2", "h2")
        cluster_pubsub.subscribe("t2", "h3")
        stats = cluster_pubsub.get_stats()
        assert stats["local_topics"] == 2
        assert stats["local_subscriptions"] == 3

    def test_stats_after_publish(self, cluster_pubsub, mock_membership, mock_transport):
        mock_membership.alive_nodes = [_make_peer("p1")]
        cluster_pubsub.publish("stats-topic", payload="data")
        stats = cluster_pubsub.get_stats()
        assert stats["fan_out_count"] == 1

    def test_stats_cluster_peers(self, cluster_pubsub, mock_membership):
        mock_membership.alive_nodes = [_make_peer("p1"), _make_peer("p2")]
        stats = cluster_pubsub.get_stats()
        assert stats["cluster_peers"] == 2

    def test_stats_peers_when_stopped(self, cluster_pubsub, mock_membership):
        mock_membership.is_running = False
        stats = cluster_pubsub.get_stats()
        assert stats["cluster_peers"] == 0


# ============================================================
# Thread safety
# ============================================================

class TestThreadSafety:
    """Basic thread safety tests for concurrent operations."""

    def test_concurrent_publish(self, cluster_pubsub, mock_membership, mock_transport):
        import threading
        mock_membership.alive_nodes = [_make_peer("p1")]
        errors = []

        def publish_n(n):
            try:
                for i in range(50):
                    cluster_pubsub.publish(f"topic-{n}", payload=i)
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=publish_n, args=(t,)) for t in range(4)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
        stats = cluster_pubsub.get_stats()
        assert stats["fan_out_count"] == 200  # 4 threads x 50 publishes
