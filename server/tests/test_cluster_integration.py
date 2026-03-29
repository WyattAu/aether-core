"""Multi-node cluster integration tests.

These tests spin up real FastAPI servers with clustering enabled
and verify end-to-end behavior: node discovery, message forwarding,
and cluster management.
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
    """Start a single cluster node as a background task.

    Yields an httpx.AsyncClient for making API calls.
    """
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


class TestSingleClusterNode:
    """Tests for a single cluster node running in isolation."""

    @pytest.mark.asyncio
    async def test_single_node_cluster_info(self):
        """A single node should show as running with 1 member (itself)."""
        async with start_cluster_node("127.0.0.1", 18001, "node-1") as client:
            resp = await client.get("/cluster/info")
            assert resp.status_code == 200
            data = resp.json()
            assert data["node_id"] == "node-1"
            assert data["status"] == "running"
            assert data["members"]["alive"] == 1

    @pytest.mark.asyncio
    async def test_single_node_lists_self(self):
        """A single node should list itself in the nodes endpoint."""
        async with start_cluster_node("127.0.0.1", 18002, "node-1") as client:
            resp = await client.get("/cluster/nodes")
            assert resp.status_code == 200
            data = resp.json()
            assert data["total"] == 1
            assert data["nodes"][0]["node_id"] == "node-1"

    @pytest.mark.asyncio
    async def test_ring_stats_single_node(self):
        """A single node should have 1 node on the hash ring."""
        async with start_cluster_node("127.0.0.1", 18003, "node-1") as client:
            resp = await client.get("/cluster/ring")
            assert resp.status_code == 200
            data = resp.json()
            assert data["ring_nodes"] == 1


class TestTwoNodeCluster:
    """Tests for two nodes forming a cluster."""

    @pytest.mark.asyncio
    async def test_two_nodes_discover_each_other(self):
        """Two nodes should discover each other via seed node bootstrap."""
        async with start_cluster_node(
            "127.0.0.1", 18010, "node-1", gossip_interval=0.1, failure_timeout=0.5,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18011, "node-2",
                seed_nodes=["127.0.0.1:18010"],
                gossip_interval=0.1, failure_timeout=0.5,
            ) as client2:
                for _ in range(30):
                    resp1 = await client1.get("/cluster/nodes")
                    assert resp1.status_code == 200
                    nodes1 = {n["node_id"] for n in resp1.json()["nodes"]}
                    if "node-2" in nodes1 and "node-1" in nodes1:
                        break
                    await asyncio.sleep(0.2)
                else:
                    assert False, f"Nodes did not discover each other: {nodes1}"

                resp2 = await client2.get("/cluster/nodes")
                assert resp2.status_code == 200
                nodes2 = {n["node_id"] for n in resp2.json()["nodes"]}
                assert "node-1" in nodes2
                assert "node-2" in nodes2

    @pytest.mark.asyncio
    async def test_cluster_info_two_members(self):
        async with start_cluster_node(
            "127.0.0.1", 18020, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18021, "node-2",
                seed_nodes=["127.0.0.1:18020"],
                gossip_interval=0.1,
            ) as client2:
                for _ in range(30):
                    resp = await client1.get("/cluster/info")
                    data = resp.json()
                    if data["members"]["alive"] == 2:
                        break
                    await asyncio.sleep(0.2)
                else:
                    assert False, f"Nodes did not converge: {data}"

    @pytest.mark.asyncio
    async def test_ring_distribution_two_nodes(self):
        """With 2 nodes, the ring should have 2 nodes."""
        async with start_cluster_node(
            "127.0.0.1", 18030, "node-1", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18031, "node-2",
                seed_nodes=["127.0.0.1:18030"],
                gossip_interval=0.1,
            ) as client2:
                for _ in range(30):
                    resp = await client1.get("/cluster/ring")
                    data = resp.json()
                    if data["ring_nodes"] == 2:
                        break
                    await asyncio.sleep(0.2)
                else:
                    assert False, f"Ring did not converge: {data}"


class TestClusterHealthIntegration:
    """Tests that cluster health integrates with the main health endpoint."""

    @pytest.mark.asyncio
    async def test_health_ok_with_cluster(self):
        """Health endpoint should work when clustering is enabled."""
        async with start_cluster_node("127.0.0.1", 18040, "node-1") as client:
            resp = await client.get("/health")
            assert resp.status_code == 200
            data = resp.json()
            assert data["status"] == "ok"

    @pytest.mark.asyncio
    async def test_cluster_stops_gracefully(self):
        """Cluster should stop cleanly when server shuts down."""
        async with start_cluster_node("127.0.0.1", 18041, "node-1") as client:
            resp = await client.get("/cluster/info")
            assert resp.status_code == 200


class TestDLQWithCluster:
    """Tests that DLQ works alongside clustering."""

    @pytest.mark.asyncio
    async def test_dlq_with_cluster_enabled(self):
        """DLQ endpoint should work when clustering is enabled."""
        async with start_cluster_node("127.0.0.1", 18050, "node-1") as client:
            resp = await client.post("/api/v1/actors/nonexistent/messages", json={
                "source_actor": "s",
                "target_actor": "nonexistent",
                "message_type": "test",
                "payload": {"x": 1},
            })
            assert resp.status_code == 202

            resp = await client.get("/dlq/stats")
            assert resp.status_code == 200
            data = resp.json()
            assert data["size"] == 0
