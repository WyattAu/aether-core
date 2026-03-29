"""Multi-node distributed pub/sub integration tests.

These tests spin up real FastAPI servers with clustering enabled
and verify that pub/sub messages are fanned out across nodes.
"""

import asyncio
import threading
from contextlib import asynccontextmanager

import httpx
import pytest
import uvicorn

from server.config import ServerConfig


@asynccontextmanager
async def start_cluster_node(
    host: str,
    port: int,
    node_id: str,
    seed_nodes: list | None = None,
    gossip_interval: float = 0.1,
    failure_timeout: float = 0.5,
    dead_timeout: float = 2.0,
):
    """Start a single cluster node as a background task."""
    from server.app import create_app

    config = ServerConfig(
        host=host,
        port=port,
        cluster_enabled=True,
        cluster_node_id=node_id,
        cluster_seed_nodes=seed_nodes or [],
        cluster_bind_host=host,
        cluster_gossip_port=port + 1000,
        cluster_gossip_interval=gossip_interval,
        cluster_failure_timeout=failure_timeout,
        cluster_dead_timeout=dead_timeout,
        cluster_suspicion_max=2,
        cluster_virtual_nodes=50,
        cluster_transport="http",
        cluster_secret="test-cluster-secret",
        dlq_max_size=1000,
    )

    app = create_app(config)
    app.state.server_config = config

    server = uvicorn.Server(
        uvicorn.Config(app, host=host, port=port, log_level="warning"),
    )

    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()

    client = httpx.AsyncClient(base_url=f"http://{host}:{port}", timeout=5.0)
    for _ in range(50):
        try:
            resp = await client.get("/health")
            if resp.status_code == 200:
                break
        except Exception:
            pass
        await asyncio.sleep(0.1)
    else:
        server.should_exit = True
        thread.join(timeout=5)
        pytest.fail(f"Node {node_id} failed to start on {host}:{port}")

    try:
        yield client
    finally:
        server.should_exit = True
        await client.aclose()
        thread.join(timeout=5)


async def wait_for_cluster(client, expected_members: int, timeout: float = 6.0):
    """Wait until the cluster has the expected number of alive members."""
    deadline = asyncio.get_event_loop().time() + timeout
    while asyncio.get_event_loop().time() < deadline:
        resp = await client.get("/cluster/info")
        data = resp.json()
        if data["members"]["alive"] >= expected_members:
            return
        await asyncio.sleep(0.2)
    pytest.fail(f"Cluster did not reach {expected_members} members: {data}")


class TestDistributedPubSub:
    """Tests for pub/sub fan-out across cluster nodes."""

    @pytest.mark.asyncio
    async def test_pubsub_api_works_with_cluster(self):
        """Basic pub/sub API should work when clustering is enabled."""
        async with start_cluster_node("127.0.0.1", 18201, "node-1") as client:
            # Subscribe
            resp = await client.post("/api/v1/events/subscribe", json={
                "topic": "test-topic", "subscriber_id": "h1",
            })
            assert resp.status_code == 201
            sub_id = resp.json()["subscription_id"]

            # Publish
            resp = await client.post("/api/v1/events/publish", json={
                "topic": "test-topic", "payload": "hello",
            })
            assert resp.status_code == 202
            assert resp.json()["subscriber_count"] == 1

            # History
            resp = await client.get("/api/v1/events/topics/test-topic/history")
            assert resp.status_code == 200
            assert len(resp.json()) == 1

            # Unsubscribe
            resp = await client.delete(f"/api/v1/events/subscribe/{sub_id}")
            assert resp.status_code == 204

    @pytest.mark.asyncio
    async def test_cluster_pubsub_stats_endpoint(self):
        """The /cluster/pubsub-stats endpoint should return statistics."""
        async with start_cluster_node("127.0.0.1", 18202, "node-1") as client:
            resp = await client.get("/cluster/pubsub-stats")
            assert resp.status_code == 200
            data = resp.json()
            assert "local_topics" in data
            assert "fan_out_count" in data
            assert "cluster_peers" in data

    @pytest.mark.asyncio
    async def test_cluster_pubsub_stats_after_operations(self):
        """Stats should reflect pub/sub activity."""
        async with start_cluster_node("127.0.0.1", 18203, "node-1") as client:
            # Subscribe and publish
            await client.post("/api/v1/events/subscribe", json={
                "topic": "stats-topic", "subscriber_id": "h1",
            })
            await client.post("/api/v1/events/publish", json={
                "topic": "stats-topic", "payload": "data",
            })

            resp = await client.get("/cluster/pubsub-stats")
            data = resp.json()
            assert data["local_topics"] == 1
            assert data["local_subscriptions"] == 1

    @pytest.mark.asyncio
    async def test_two_node_pubsub_fan_out(self):
        """A publish on node-1 should fan out to node-2's subscribers."""
        async with start_cluster_node(
            "127.0.0.1", 18210, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18211, "node-2",
                seed_nodes=["127.0.0.1:18210"],
                gossip_interval=0.1,
            ) as client2:
                # Wait for cluster to form
                await wait_for_cluster(client1, 2)

                # Subscribe on node-2 only
                resp = await client2.post("/api/v1/events/subscribe", json={
                    "topic": "fanout-topic", "subscriber_id": "node2-handler",
                })
                assert resp.status_code == 201

                # Give gossip a moment to propagate membership
                await asyncio.sleep(0.5)

                # Publish on node-1
                resp = await client1.post("/api/v1/events/publish", json={
                    "topic": "fanout-topic", "payload": "cross-node-msg",
                })
                assert resp.status_code == 202
                # node-1 has 0 local subscribers for this topic
                assert resp.json()["subscriber_count"] == 0

                # Give fan-out time to propagate
                await asyncio.sleep(0.5)

                # Check history on node-2 — should have received the message
                resp = await client2.get("/api/v1/events/topics/fanout-topic/history")
                assert resp.status_code == 200
                history = resp.json()
                assert len(history) >= 1
                assert history[-1]["payload"] == "cross-node-msg"

    @pytest.mark.asyncio
    async def test_pubsub_fan_out_to_both_nodes(self):
        """A publish should reach subscribers on all nodes."""
        async with start_cluster_node(
            "127.0.0.1", 18220, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18221, "node-2",
                seed_nodes=["127.0.0.1:18220"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)

                # Subscribe on both nodes
                await client1.post("/api/v1/events/subscribe", json={
                    "topic": "both-topic", "subscriber_id": "node1-h",
                })
                await client2.post("/api/v1/events/subscribe", json={
                    "topic": "both-topic", "subscriber_id": "node2-h",
                })

                await asyncio.sleep(0.5)

                # Publish on node-1
                resp = await client1.post("/api/v1/events/publish", json={
                    "topic": "both-topic", "payload": "all-nodes",
                })
                assert resp.status_code == 202
                # node-1 has 1 local subscriber
                assert resp.json()["subscriber_count"] == 1

                await asyncio.sleep(0.5)

                # Both nodes should have the message in history
                resp1 = await client1.get("/api/v1/events/topics/both-topic/history")
                resp2 = await client2.get("/api/v1/events/topics/both-topic/history")
                assert len(resp1.json()) >= 1
                assert len(resp2.json()) >= 1
                assert resp1.json()[-1]["payload"] == "all-nodes"
                assert resp2.json()[-1]["payload"] == "all-nodes"

    @pytest.mark.asyncio
    async def test_multiple_publishes_fan_out(self):
        """Multiple publishes should all reach remote subscribers."""
        async with start_cluster_node(
            "127.0.0.1", 18230, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18231, "node-2",
                seed_nodes=["127.0.0.1:18230"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)

                # Subscribe on node-2
                await client2.post("/api/v1/events/subscribe", json={
                    "topic": "multi-topic", "subscriber_id": "node2-h",
                })
                await asyncio.sleep(0.5)

                # Publish 3 messages from node-1
                for i in range(3):
                    resp = await client1.post("/api/v1/events/publish", json={
                        "topic": "multi-topic", "payload": f"msg-{i}",
                    })
                    assert resp.status_code == 202

                await asyncio.sleep(0.5)

                # Node-2 should have all 3 in history
                resp = await client2.get("/api/v1/events/topics/multi-topic/history")
                assert resp.status_code == 200
                history = resp.json()
                payloads = [m["payload"] for m in history]
                assert "msg-0" in payloads
                assert "msg-1" in payloads
                assert "msg-2" in payloads

    @pytest.mark.asyncio
    async def test_cluster_pubsub_stats_show_fanout(self):
        """After cross-node publish, stats should show fan-out activity."""
        async with start_cluster_node(
            "127.0.0.1", 18240, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18241, "node-2",
                seed_nodes=["127.0.0.1:18240"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)

                # Subscribe on node-2
                await client2.post("/api/v1/events/subscribe", json={
                    "topic": "stats-fanout", "subscriber_id": "node2-h",
                })
                await asyncio.sleep(0.5)

                # Publish from node-1
                await client1.post("/api/v1/events/publish", json={
                    "topic": "stats-fanout", "payload": "test",
                })
                await asyncio.sleep(0.5)

                # Node-1 stats should show fan-out
                resp = await client1.get("/cluster/pubsub-stats")
                data = resp.json()
                assert data["fan_out_count"] >= 1
                assert data["cluster_peers"] >= 1

    @pytest.mark.asyncio
    async def test_internal_pubsub_endpoint_directly(self):
        """The internal pubsub/publish endpoint should work when called directly."""
        async with start_cluster_node("127.0.0.1", 18250, "node-1") as client:
            # Subscribe locally
            await client.post("/api/v1/events/subscribe", json={
                "topic": "internal-topic", "subscriber_id": "h1",
            })

            # Call the internal endpoint directly (simulating what a peer would do)
            resp = await client.post("/cluster/internal/pubsub/publish", json={
                "topic": "internal-topic",
                "payload": "from-internal-endpoint",
                "headers": {"x-source": "test"},
                "source_node_id": "some-peer",
            })
            assert resp.status_code == 200
            assert resp.json()["local_subscriber_count"] == 1

            # Verify message in history
            resp = await client.get("/api/v1/events/topics/internal-topic/history")
            history = resp.json()
            assert len(history) >= 1
            assert history[-1]["payload"] == "from-internal-endpoint"

    @pytest.mark.asyncio
    async def test_topics_list_is_local_only(self):
        """list_topics should only show topics with local subscribers."""
        async with start_cluster_node(
            "127.0.0.1", 18260, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18261, "node-2",
                seed_nodes=["127.0.0.1:18260"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)

                # Subscribe on node-1 only
                await client1.post("/api/v1/events/subscribe", json={
                    "topic": "local-only-topic", "subscriber_id": "h1",
                })
                await asyncio.sleep(0.5)

                # node-1 should list the topic
                resp = await client1.get("/api/v1/events/topics")
                assert "local-only-topic" in resp.json()

                # node-2 should NOT list the topic
                resp = await client2.get("/api/v1/events/topics")
                assert "local-only-topic" not in resp.json()
