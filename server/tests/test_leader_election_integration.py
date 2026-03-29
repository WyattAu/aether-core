"""Multi-node leader election integration tests.

These tests spin up real FastAPI servers with clustering enabled
and verify that leader election works across nodes.
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


async def wait_for_leader(client, timeout: float = 6.0):
    """Wait until a leader is elected."""
    deadline = asyncio.get_event_loop().time() + timeout
    while asyncio.get_event_loop().time() < deadline:
        resp = await client.get("/cluster/leader")
        data = resp.json()
        if data.get("leader_id") is not None:
            return data["leader_id"]
        await asyncio.sleep(0.2)
    pytest.fail(f"No leader elected within timeout")


class TestSingleNodeElection:
    """Leader election with a single node."""

    @pytest.mark.asyncio
    async def test_single_node_is_leader(self):
        """A single node should elect itself as leader."""
        async with start_cluster_node("127.0.0.1", 18401, "node-a") as client:
            resp = await client.get("/cluster/leader")
            assert resp.status_code == 200
            data = resp.json()
            assert data["leader_id"] == "node-a"
            assert data["is_leader"] is True
            assert data["election_count"] >= 1

    @pytest.mark.asyncio
    async def test_leader_in_cluster_info(self):
        """Leader info should be in cluster info."""
        async with start_cluster_node("127.0.0.1", 18402, "node-a") as client:
            resp = await client.get("/cluster/info")
            data = resp.json()
            assert data["leader_id"] == "node-a"
            assert data["is_leader"] is True


class TestTwoNodeElection:
    """Leader election with two nodes."""

    @pytest.mark.asyncio
    async def test_lowest_id_is_leader(self):
        """With deterministic IDs, the lowest ID should be leader."""
        async with start_cluster_node(
            "127.0.0.1", 18410, "node-b", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18411, "node-a",
                seed_nodes=["127.0.0.1:18410"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                await asyncio.sleep(0.5)

                resp = await client1.get("/cluster/leader")
                data = resp.json()
                assert data["leader_id"] == "node-a"

    @pytest.mark.asyncio
    async def test_both_nodes_agree_on_leader(self):
        """Both nodes should see the same leader."""
        async with start_cluster_node(
            "127.0.0.1", 18420, "node-b", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18421, "node-a",
                seed_nodes=["127.0.0.1:18420"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                await asyncio.sleep(0.5)

                resp1 = await client1.get("/cluster/leader")
                resp2 = await client2.get("/cluster/leader")
                assert resp1.json()["leader_id"] == resp2.json()["leader_id"]

    @pytest.mark.asyncio
    async def test_leader_status_consistency(self):
        """is_leader should be correct on both nodes."""
        async with start_cluster_node(
            "127.0.0.1", 18430, "node-b", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18431, "node-a",
                seed_nodes=["127.0.0.1:18430"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                await asyncio.sleep(0.5)

                resp1 = await client1.get("/cluster/leader")
                resp2 = await client2.get("/cluster/leader")
                assert resp1.json()["is_leader"] is False
                assert resp2.json()["is_leader"] is True


class TestThreeNodeElection:
    """Leader election with three nodes."""

    @pytest.mark.asyncio
    async def test_lowest_of_three_is_leader(self):
        async with start_cluster_node(
            "127.0.0.1", 18440, "node-c", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18441, "node-b",
                seed_nodes=["127.0.0.1:18440"],
                gossip_interval=0.1,
            ) as client2:
                async with start_cluster_node(
                    "127.0.0.1", 18442, "node-a",
                    seed_nodes=["127.0.0.1:18440"],
                    gossip_interval=0.1,
                ) as client3:
                    await wait_for_cluster(client1, 3)
                    await asyncio.sleep(0.5)

                    resp = await client1.get("/cluster/leader")
                    assert resp.json()["leader_id"] == "node-a"


class TestLeaderStepDownAPI:
    """Tests for the step-down API endpoint."""

    @pytest.mark.asyncio
    async def test_step_down_elects_next(self):
        """Stepping down should elect the next lowest node."""
        async with start_cluster_node(
            "127.0.0.1", 18450, "node-b", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18451, "node-a",
                seed_nodes=["127.0.0.1:18450"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                # Wait for gossip to stabilize so neither node is SUSPECT
                await asyncio.sleep(2.0)

                # node-a is leader, step it down
                resp = await client2.post("/cluster/leader/step-down")
                assert resp.status_code == 200
                data = resp.json()
                assert data["previous_leader"] == "node-a"
                # new_leader may be None if other node is temporarily SUSPECT
                assert data["new_leader"] in ("node-b", None)

                # node-b should now be leader (after gossip propagates step_down)
                await asyncio.sleep(1.5)
                resp1 = await client1.get("/cluster/leader")
                resp2 = await client2.get("/cluster/leader")
                assert resp1.json()["leader_id"] == "node-b"
                assert resp2.json()["leader_id"] == "node-b"

    @pytest.mark.asyncio
    async def test_step_down_from_non_leader_is_noop(self):
        """Stepping down from a non-leader should be a no-op."""
        async with start_cluster_node(
            "127.0.0.1", 18460, "node-b", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18461, "node-a",
                seed_nodes=["127.0.0.1:18460"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                await asyncio.sleep(0.5)

                # node-b tries to step down (not leader)
                resp = await client1.post("/cluster/leader/step-down")
                assert resp.status_code == 200
                data = resp.json()
                assert data["previous_leader"] == "node-a"  # Still reports current leader


class TestLeaderForceAPI:
    """Tests for the force-leader API endpoint."""

    @pytest.mark.asyncio
    async def test_force_leader_success(self):
        async with start_cluster_node(
            "127.0.0.1", 18470, "node-a", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18471, "node-b",
                seed_nodes=["127.0.0.1:18470"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                await asyncio.sleep(0.5)

                # Force node-b to be leader
                resp = await client1.post("/cluster/leader/force", json={"node_id": "node-b"})
                assert resp.status_code == 200
                assert resp.json()["leader_id"] == "node-b"

    @pytest.mark.asyncio
    async def test_force_leader_invalid_rejected(self):
        async with start_cluster_node(
            "127.0.0.1", 18480, "node-a", gossip_interval=0.1,
        ) as client:
            resp = await client.post("/cluster/leader/force", json={"node_id": "nonexistent"})
            assert resp.status_code == 200
            assert resp.json()["error"] == "invalid_target"


class TestLeaderInClusterInfo:
    """Leader info should be included in cluster info response."""

    @pytest.mark.asyncio
    async def test_leader_in_info(self):
        async with start_cluster_node(
            "127.0.0.1", 18490, "node-b", gossip_interval=0.1,
        ) as client1:
            async with start_cluster_node(
                "127.0.0.1", 18491, "node-a",
                seed_nodes=["127.0.0.1:18490"],
                gossip_interval=0.1,
            ) as client2:
                await wait_for_cluster(client1, 2)
                await asyncio.sleep(0.5)

                resp = await client1.get("/cluster/info")
                data = resp.json()
                assert data["leader_id"] == "node-a"
                assert data["is_leader"] is False
                assert data["election_count"] >= 1
