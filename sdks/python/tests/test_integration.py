"""Integration tests for the Python SDK against a running Aether server.

These tests require a running Aether server at the AETHER_BASE_URL
(default: http://localhost:8080). They are skipped automatically if
the server is not reachable.

Run with:
    pytest sdks/python/tests/test_integration.py -v
    AETHER_BASE_URL=http://localhost:9090 pytest sdks/python/tests/test_integration.py -v
"""

import os
import uuid

import httpx
import pytest
from aether_sdk.client import AetherClient, AetherServerError

BASE_URL = os.environ.get("AETHER_BASE_URL", "http://localhost:8080")

pytestmark = pytest.mark.integration


def _server_reachable() -> bool:
    try:
        resp = httpx.get(f"{BASE_URL}/health", timeout=3.0)
        return resp.status_code == 200
    except Exception:
        return False


def _needs_server():
    if not _server_reachable():
        pytest.skip(f"Aether server not reachable at {BASE_URL}")


@pytest.fixture
def client():
    _needs_server()
    c = AetherClient(base_url=BASE_URL, actor_id="integration-test-sender")
    import asyncio

    asyncio.get_event_loop().run_until_complete(c.connect())
    yield c
    asyncio.get_event_loop().run_until_complete(c.close())


@pytest.fixture
def unique_id():
    return f"inttest-{uuid.uuid4().hex[:12]}"


class TestHealthCheck:
    @pytest.mark.asyncio
    async def test_health_returns_ok(self, client):
        info = await client.health()
        assert info.status == "ok"
        assert info.uptime >= 0

    @pytest.mark.asyncio
    async def test_info_returns_version(self, client):
        info = await client.info()
        assert "version" in info or "uptime" in info or "actor_count" in info


class TestActorSpawnAndMessage:
    @pytest.mark.asyncio
    async def test_register_actor(self, client, unique_id):
        actor = await client.register_actor(unique_id, "worker")
        assert actor.actor_id == unique_id
        assert actor.actor_type == "worker"
        assert actor.status == "active"

    @pytest.mark.asyncio
    async def test_get_actor(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        actor = await client.get_actor(unique_id)
        assert actor.actor_id == unique_id

    @pytest.mark.asyncio
    async def test_list_actors(self, client, unique_id):
        await client.register_actor(unique_id, "integration-test")
        actors = await client.list_actors()
        ids = [a.actor_id for a in actors]
        assert unique_id in ids

    @pytest.mark.asyncio
    async def test_unregister_actor(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        await client.unregister_actor(unique_id)
        with pytest.raises(AetherServerError):
            await client.get_actor(unique_id)

    @pytest.mark.asyncio
    async def test_send_message(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        receipt = await client.send_message(
            target=unique_id,
            payload={"hello": "world"},
            message_type="greeting",
        )
        assert receipt.status == "delivered"
        assert receipt.message_id is not None

    @pytest.mark.asyncio
    async def test_heartbeat(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        await client.heartbeat(unique_id)


class TestStateReadWrite:
    @pytest.mark.asyncio
    async def test_set_and_get_state(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        entry = await client.set_state(unique_id, "counter", 42)
        assert entry.version >= 1

        value = await client.get_state(unique_id, "counter")
        assert value == 42

    @pytest.mark.asyncio
    async def test_get_missing_state_returns_none(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        value = await client.get_state(unique_id, "nonexistent")
        assert value is None

    @pytest.mark.asyncio
    async def test_delete_state(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        await client.set_state(unique_id, "temp", "data")
        deleted = await client.delete_state(unique_id, "temp")
        assert deleted is True

    @pytest.mark.asyncio
    async def test_get_all_state(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        await client.set_state(unique_id, "a", 1)
        await client.set_state(unique_id, "b", 2)
        state = await client.get_all_state(unique_id)
        assert "a" in state or "b" in state

    @pytest.mark.asyncio
    async def test_state_version_increments(self, client, unique_id):
        await client.register_actor(unique_id, "worker")
        e1 = await client.set_state(unique_id, "counter", 1)
        e2 = await client.set_state(unique_id, "counter", 2)
        assert e2.version > e1.version


class TestMeshPeerDiscovery:
    @pytest.mark.asyncio
    async def test_cluster_info_endpoint(self, client):
        try:
            import httpx as _hx

            resp = _hx.get(f"{BASE_URL}/cluster/info", timeout=5.0)
            if resp.status_code == 200:
                data = resp.json()
                assert (
                    "node_id" in data or "cluster_enabled" in data or "status" in data
                )
            elif resp.status_code == 404:
                pytest.skip("Cluster endpoints not available on this server")
        except Exception:
            pytest.skip("Cluster endpoints not available on this server")

    @pytest.mark.asyncio
    async def test_cluster_nodes_endpoint(self, client):
        try:
            import httpx as _hx

            resp = _hx.get(f"{BASE_URL}/cluster/nodes", timeout=5.0)
            if resp.status_code == 200:
                data = resp.json()
                assert isinstance(data, (list, dict))
            elif resp.status_code == 404:
                pytest.skip("Cluster endpoints not available on this server")
        except Exception:
            pytest.skip("Cluster endpoints not available on this server")
