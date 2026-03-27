import pytest
from httpx import ASGITransport, AsyncClient

from server.app import app, lifespan


@pytest.fixture
async def client():
    async with lifespan(app):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as c:
            yield c


@pytest.mark.asyncio
async def test_actor_response_required_fields(client: AsyncClient):
    resp = await client.post(
        "/api/v1/actors",
        json={
            "actor_id": "contract-actor-1",
            "actor_type": "worker",
            "capabilities": ["compute", "state_read"],
            "metadata": {"env": "test"},
        },
    )
    assert resp.status_code == 201
    data = resp.json()
    required = ["actor_id", "actor_type", "capabilities", "metadata", "status", "created_at"]
    for field in required:
        assert field in data, f"ActorInfo missing required field: {field}"
    assert isinstance(data["actor_id"], str)
    assert isinstance(data["actor_type"], str)
    assert isinstance(data["capabilities"], list)
    assert isinstance(data["metadata"], dict)
    assert isinstance(data["status"], str)

    resp = await client.get("/api/v1/actors/contract-actor-1")
    assert resp.status_code == 200
    for field in required:
        assert field in resp.json(), f"ActorInfo GET missing field: {field}"
    assert "last_heartbeat" in resp.json()

    resp = await client.post("/api/v1/actors/contract-actor-1/heartbeat")
    assert resp.status_code == 204

    resp = await client.get("/api/v1/actors/contract-actor-1")
    assert resp.json()["last_heartbeat"] is not None


@pytest.mark.asyncio
async def test_message_format_contract(client: AsyncClient):
    await client.post("/api/v1/actors", json={"actor_id": "contract-msg-sender"})
    await client.post("/api/v1/actors", json={"actor_id": "contract-msg-target"})

    contract_message_types = [
        "start", "stop", "signal", "rpc_request", "rpc_response", "custom",
        "stream_event", "watermark", "checkpoint", "checkpoint_ack",
    ]

    for msg_type in contract_message_types:
        resp = await client.post(
            "/api/v1/actors/contract-msg-target/messages",
            json={
                "source_actor": "contract-msg-sender",
                "target_actor": "contract-msg-target",
                "message_type": msg_type,
                "payload": {"type": msg_type},
                "correlation_id": f"corr-{msg_type}",
            },
        )
        assert resp.status_code == 202, f"Failed for message_type={msg_type}"
        receipt = resp.json()
        assert "message_id" in receipt
        assert "status" in receipt
        assert receipt["status"] in ("delivered", "buffered")

    resp = await client.get("/api/v1/actors/contract-msg-target/messages")
    messages = resp.json()
    assert len(messages) >= len(contract_message_types)

    msg_fields = ["source_actor", "target_actor", "message_type", "payload", "message_id", "timestamp"]
    for msg in messages:
        for field in msg_fields:
            assert field in msg, f"MessageEnvelope missing field: {field}"
        assert msg["source_actor"] == "contract-msg-sender"
        assert msg["target_actor"] == "contract-msg-target"
        assert msg["message_type"] in contract_message_types


@pytest.mark.asyncio
async def test_error_response_structure(client: AsyncClient):
    resp = await client.get("/api/v1/actors/contract-nonexistent")
    assert resp.status_code == 404
    assert "detail" in resp.json()
    assert isinstance(resp.json()["detail"], str)

    await client.post("/api/v1/actors", json={"actor_id": "contract-dup-err"})
    resp = await client.post("/api/v1/actors", json={"actor_id": "contract-dup-err"})
    assert resp.status_code == 409
    assert "detail" in resp.json()

    resp = await client.delete("/api/v1/actors/contract-nonexistent")
    assert resp.status_code == 404
    assert "detail" in resp.json()

    resp = await client.post("/api/v1/actors/contract-nonexistent/heartbeat")
    assert resp.status_code == 404
    assert "detail" in resp.json()

    resp = await client.delete("/api/v1/events/subscribe/contract-nonexistent-sub")
    assert resp.status_code == 404
    assert "detail" in resp.json()


@pytest.mark.asyncio
async def test_timestamp_format_contract(client: AsyncClient):
    resp = await client.post(
        "/api/v1/actors", json={"actor_id": "contract-ts-actor"}
    )
    actor = resp.json()
    assert actor["created_at"] is not None
    assert isinstance(actor["created_at"], str)

    msg_resp = await client.post(
        "/api/v1/actors/contract-ts-actor/messages",
        json={
            "source_actor": "contract-ts-actor",
            "target_actor": "contract-ts-actor",
            "payload": "ts",
        },
    )
    receipt = msg_resp.json()
    assert isinstance(receipt["delivered_at"], str)

    messages = (await client.get("/api/v1/actors/contract-ts-actor/messages")).json()
    assert len(messages) >= 1
    assert isinstance(messages[0]["timestamp"], str)

    state_resp = await client.put(
        "/api/v1/state/contract-ts-actor/k", json={"value": "v"}
    )
    assert isinstance(state_resp.json()["updated_at"], str)

    event_resp = await client.post(
        "/api/v1/events/append",
        json={"aggregate_id": "contract-ts-agg", "event_type": "TSEvent", "data": {"x": 1}},
    )
    assert isinstance(event_resp.json()["timestamp"], str)


@pytest.mark.asyncio
async def test_capability_values_contract(client: AsyncClient):
    expected_capabilities = {
        1: "NETWORK_OUTBOUND",
        2: "NETWORK_INBOUND",
        4: "STATE_READ",
        8: "STATE_WRITE",
        16: "FS_READ",
        32: "FS_WRITE",
        64: "ACTOR_MESSAGING",
        128: "LOG",
        256: "TIME",
        512: "RANDOM",
        1024: "ENVIRONMENT",
        2048: "HTTP_CLIENT",
        4096: "HTTP_SERVER",
    }

    cap_names = list(expected_capabilities.values())
    resp = await client.post(
        "/api/v1/actors",
        json={"actor_id": "contract-caps", "capabilities": cap_names},
    )
    assert resp.status_code == 201
    assert resp.json()["capabilities"] == cap_names

    resp = await client.get("/api/v1/actors/contract-caps")
    assert resp.json()["capabilities"] == cap_names


@pytest.mark.asyncio
async def test_state_key_naming_contract(client: AsyncClient):
    snake_case_keys = ["user_count", "last_login", "is_active", "total_bytes"]

    for key in snake_case_keys:
        resp = await client.put(
            f"/api/v1/state/contract-keys/{key}",
            json={"value": f"val_{key}"},
        )
        assert resp.status_code == 200
        assert resp.json()["key"] == key

    resp = await client.get("/api/v1/state/contract-keys")
    state = resp.json()["state"]
    for key in snake_case_keys:
        assert key in state, f"Missing snake_case state key: {key}"

    resp = await client.put(
        "/api/v1/state/contract-keys/nested/config",
        json={"value": {"debug": True}},
    )
    assert resp.status_code == 200

    resp = await client.get("/api/v1/state/contract-keys/nested/config")
    assert resp.status_code == 200
    assert resp.json()["value"]["debug"] is True
