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
async def test_message_format_compatibility(client: AsyncClient):
    await client.post("/api/v1/actors", json={"actor_id": "js-msg-sender"})
    await client.post("/api/v1/actors", json={"actor_id": "js-msg-receiver"})

    resp = await client.post(
        "/api/v1/actors/js-msg-receiver/messages",
        json={
            "source_actor": "js-msg-sender",
            "target_actor": "js-msg-receiver",
            "message_type": "rpc_request",
            "payload": {"method": "ping", "args": []},
            "correlation_id": "js-corr-001",
            "priority": 2,
        },
    )
    assert resp.status_code == 202
    receipt = resp.json()
    assert "message_id" in receipt
    assert receipt["status"] in ("delivered", "buffered")
    assert "delivered_at" in receipt
    assert receipt["correlation_id"] == "js-corr-001"

    resp = await client.get("/api/v1/actors/js-msg-receiver/messages")
    messages = resp.json()
    assert len(messages) >= 1
    msg = messages[-1]

    js_sdk_fields = {
        "source_actor": "sender",
        "target_actor": "targetActor",
        "message_type": "type",
        "payload": "payload",
        "correlation_id": "correlationId",
        "priority": "priority",
        "message_id": "messageId",
        "timestamp": "timestamp",
    }
    for server_field, js_field in js_sdk_fields.items():
        assert server_field in msg, f"Missing field '{server_field}' (JS expects '{js_field}')"

    assert isinstance(msg["payload"], dict)
    assert isinstance(msg["priority"], int)
    assert isinstance(msg["message_type"], str)


@pytest.mark.asyncio
async def test_state_format_compatibility(client: AsyncClient):
    resp = await client.put(
        "/api/v1/state/js-state-1/config",
        json={"value": {"theme": "dark", "lang": "en"}},
    )
    assert resp.status_code == 200
    entry = resp.json()
    assert entry["actor_id"] == "js-state-1"
    assert entry["key"] == "config"
    assert isinstance(entry["value"], dict)
    assert entry["value"]["theme"] == "dark"
    assert isinstance(entry["version"], int)
    assert "updated_at" in entry

    resp = await client.get("/api/v1/state/js-state-1/config")
    assert resp.status_code == 200
    state = resp.json()
    assert state["actor_id"] == "js-state-1"
    assert state["key"] == "config"
    assert state["value"] == {"theme": "dark", "lang": "en"}

    await client.put(
        "/api/v1/state/js-state-1/count", json={"value": 0}
    )
    await client.put(
        "/api/v1/state/js-state-1/flag", json={"value": True}
    )
    await client.put(
        "/api/v1/state/js-state-1/empty", json={"value": None}
    )

    resp = await client.get("/api/v1/state/js-state-1")
    all_state = resp.json()["state"]
    assert all_state["count"] == 0
    assert all_state["flag"] is True
    assert all_state["empty"] is None
    assert isinstance(all_state["config"], dict)


@pytest.mark.asyncio
async def test_event_format_compatibility(client: AsyncClient):
    resp = await client.post(
        "/api/v1/events/append",
        json={
            "aggregate_id": "js-order-1",
            "event_type": "OrderCreated",
            "data": {"orderId": "ORD-001", "amount": 99.99, "items": ["item-a"]},
        },
    )
    assert resp.status_code == 201
    event = resp.json()

    required_fields = ["event_id", "aggregate_id", "event_type", "data", "version", "timestamp"]
    for field in required_fields:
        assert field in event, f"Missing field '{field}' in event record"

    assert event["event_type"] == "OrderCreated"
    assert event["aggregate_id"] == "js-order-1"
    assert isinstance(event["version"], int)
    assert isinstance(event["data"], dict)
    assert event["data"]["amount"] == 99.99
    assert isinstance(event["data"]["items"], list)

    resp = await client.get("/api/v1/events/js-order-1")
    events = resp.json()
    assert len(events) == 1
    assert events[0]["event_id"] == event["event_id"]
    assert events[0]["version"] == 1


@pytest.mark.asyncio
async def test_error_format_compatibility(client: AsyncClient):
    resp = await client.get("/api/v1/actors/js-nonexistent-actor")
    assert resp.status_code == 404
    body = resp.json()
    assert "detail" in body
    assert isinstance(body["detail"], str)
    assert "js-nonexistent-actor" in body["detail"]

    await client.post("/api/v1/actors", json={"actor_id": "js-err-dup"})
    resp = await client.post("/api/v1/actors", json={"actor_id": "js-err-dup"})
    assert resp.status_code == 409
    body = resp.json()
    assert "detail" in body
    assert "already registered" in body["detail"]

    resp = await client.get("/api/v1/state/js-no-actor/no-key")
    assert resp.status_code == 404
    assert "detail" in resp.json()

    await client.put("/api/v1/state/js-err-v1/x", json={"value": 1})
    resp = await client.put("/api/v1/state/js-err-v1/x", json={"value": 2, "version": 999})
    assert resp.status_code == 409
    assert "detail" in resp.json()

    await client.post("/api/v1/events/append", json={
        "aggregate_id": "js-err-evt", "event_type": "E1",
    })
    resp = await client.post("/api/v1/events/append", json={
        "aggregate_id": "js-err-evt", "event_type": "E2", "expected_version": 0,
    })
    assert resp.status_code == 409
    assert "detail" in resp.json()

    resp = await client.delete("/api/v1/events/subscribe/js-nonexistent-sub")
    assert resp.status_code == 404
    assert "detail" in resp.json()


@pytest.mark.asyncio
async def test_timestamp_format(client: AsyncClient):
    actor_resp = await client.post(
        "/api/v1/actors", json={"actor_id": "js-ts-actor"}
    )
    actor_data = actor_resp.json()
    created_at = actor_data["created_at"]
    assert created_at is not None
    assert isinstance(created_at, str)
    assert "T" in created_at

    msg_resp = await client.post(
        "/api/v1/actors/js-ts-actor/messages",
        json={
            "source_actor": "js-ts-actor",
            "target_actor": "js-ts-actor",
            "payload": "ts-test",
        },
    )
    receipt = msg_resp.json()
    assert "delivered_at" in receipt
    assert isinstance(receipt["delivered_at"], str)

    messages = (await client.get("/api/v1/actors/js-ts-actor/messages")).json()
    assert len(messages) >= 1
    assert isinstance(messages[0]["timestamp"], str)

    state_resp = await client.put(
        "/api/v1/state/js-ts-actor/k", json={"value": "v"}
    )
    assert isinstance(state_resp.json()["updated_at"], str)

    event_resp = await client.post(
        "/api/v1/events/append",
        json={"aggregate_id": "js-ts-agg", "event_type": "TestEvent"},
    )
    assert isinstance(event_resp.json()["timestamp"], str)

    pubsub_hist = await client.post(
        "/api/v1/events/publish",
        json={"topic": "js-ts-topic", "payload": "data"},
    )
    hist = (await client.get("/api/v1/events/topics/js-ts-topic/history")).json()
    assert len(hist) >= 1
    assert isinstance(hist[0]["timestamp"], str)
