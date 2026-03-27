import asyncio

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
async def test_actor_lifecycle(client: AsyncClient):
    resp = await client.post(
        "/api/v1/actors",
        json={
            "actor_id": "sdk-lifecycle-1",
            "actor_type": "worker",
            "capabilities": ["compute"],
            "metadata": {"region": "us-east"},
        },
    )
    assert resp.status_code == 201
    data = resp.json()
    assert data["actor_id"] == "sdk-lifecycle-1"
    assert data["actor_type"] == "worker"
    assert data["status"] == "active"
    assert data["capabilities"] == ["compute"]
    assert data["metadata"] == {"region": "us-east"}
    assert "created_at" in data

    resp = await client.post("/api/v1/actors/sdk-lifecycle-1/heartbeat")
    assert resp.status_code == 204

    resp = await client.get("/api/v1/actors/sdk-lifecycle-1")
    assert resp.status_code == 200
    info = resp.json()
    assert info["actor_id"] == "sdk-lifecycle-1"
    assert info["last_heartbeat"] is not None

    resp = await client.delete("/api/v1/actors/sdk-lifecycle-1")
    assert resp.status_code == 204

    resp = await client.get("/api/v1/actors/sdk-lifecycle-1")
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_message_passing(client: AsyncClient):
    await client.post("/api/v1/actors", json={"actor_id": "sdk-msg-a"})
    await client.post("/api/v1/actors", json={"actor_id": "sdk-msg-b"})

    resp = await client.post(
        "/api/v1/actors/sdk-msg-b/messages",
        json={
            "source_actor": "sdk-msg-a",
            "target_actor": "sdk-msg-b",
            "message_type": "greeting",
            "payload": {"text": "hello from A"},
            "correlation_id": "corr-123",
            "priority": 1,
        },
    )
    assert resp.status_code == 202
    receipt = resp.json()
    assert receipt["status"] in ("delivered", "buffered")
    assert "message_id" in receipt
    assert "delivered_at" in receipt

    resp = await client.get("/api/v1/actors/sdk-msg-b/messages")
    assert resp.status_code == 200
    messages = resp.json()
    assert len(messages) >= 1
    msg = messages[-1]
    assert msg["source_actor"] == "sdk-msg-a"
    assert msg["target_actor"] == "sdk-msg-b"
    assert msg["message_type"] == "greeting"
    assert msg["payload"]["text"] == "hello from A"
    assert msg["correlation_id"] == "corr-123"
    assert msg["priority"] == 1


@pytest.mark.asyncio
async def test_state_management(client: AsyncClient):
    await client.post("/api/v1/actors", json={"actor_id": "sdk-state-1"})

    resp = await client.put(
        "/api/v1/state/sdk-state-1/count", json={"value": 42}
    )
    assert resp.status_code == 200
    entry = resp.json()
    assert entry["actor_id"] == "sdk-state-1"
    assert entry["key"] == "count"
    assert entry["value"] == 42
    assert entry["version"] == 1

    resp = await client.get("/api/v1/state/sdk-state-1/count")
    assert resp.status_code == 200
    assert resp.json()["value"] == 42

    resp = await client.put(
        "/api/v1/state/sdk-state-1/count", json={"value": 99, "version": 1}
    )
    assert resp.status_code == 200
    assert resp.json()["value"] == 99
    assert resp.json()["version"] == 2

    resp = await client.delete("/api/v1/state/sdk-state-1/count")
    assert resp.status_code == 204

    resp = await client.get("/api/v1/state/sdk-state-1/count")
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_pubsub(client: AsyncClient):
    sub_resp = await client.post(
        "/api/v1/events/subscribe",
        json={"topic": "sdk-pubsub-topic", "subscriber_id": "sdk-sub-1"},
    )
    assert sub_resp.status_code == 201
    sub_id = sub_resp.json()["subscription_id"]
    assert sub_id.startswith("sub_")

    pub_resp = await client.post(
        "/api/v1/events/publish",
        json={
            "topic": "sdk-pubsub-topic",
            "payload": {"event": "test"},
            "headers": {"x-test": "1"},
        },
    )
    assert pub_resp.status_code == 202
    assert pub_resp.json()["subscriber_count"] == 1
    assert pub_resp.json()["topic"] == "sdk-pubsub-topic"

    hist_resp = await client.get(
        "/api/v1/events/topics/sdk-pubsub-topic/history"
    )
    assert hist_resp.status_code == 200
    history = hist_resp.json()
    assert len(history) >= 1
    last = history[-1]
    assert last["topic"] == "sdk-pubsub-topic"
    assert last["payload"] == {"event": "test"}
    assert last["headers"] == {"x-test": "1"}
    assert "message_id" in last
    assert "timestamp" in last

    unsub_resp = await client.delete(f"/api/v1/events/subscribe/{sub_id}")
    assert unsub_resp.status_code == 204

    pub_resp2 = await client.post(
        "/api/v1/events/publish",
        json={"topic": "sdk-pubsub-topic", "payload": "after-unsub"},
    )
    assert pub_resp2.status_code == 202
    assert pub_resp2.json()["subscriber_count"] == 0


@pytest.mark.asyncio
async def test_event_sourcing(client: AsyncClient):
    r1 = await client.post(
        "/api/v1/events/append",
        json={
            "aggregate_id": "sdk-order-1",
            "event_type": "OrderCreated",
            "data": {"item": "widget", "qty": 5},
        },
    )
    assert r1.status_code == 201
    assert r1.json()["version"] == 1
    assert r1.json()["event_type"] == "OrderCreated"
    assert r1.json()["aggregate_id"] == "sdk-order-1"
    assert "event_id" in r1.json()
    assert "timestamp" in r1.json()

    r2 = await client.post(
        "/api/v1/events/append",
        json={
            "aggregate_id": "sdk-order-1",
            "event_type": "OrderShipped",
            "data": {"tracking": "ABC123"},
        },
    )
    assert r2.status_code == 201
    assert r2.json()["version"] == 2

    r3 = await client.post(
        "/api/v1/events/append",
        json={
            "aggregate_id": "sdk-order-1",
            "event_type": "OrderDelivered",
            "data": {"signed_by": "John"},
        },
    )
    assert r3.status_code == 201
    assert r3.json()["version"] == 3

    resp = await client.get("/api/v1/events/sdk-order-1")
    assert resp.status_code == 200
    events = resp.json()
    assert len(events) == 3
    assert events[0]["event_type"] == "OrderCreated"
    assert events[1]["event_type"] == "OrderShipped"
    assert events[2]["event_type"] == "OrderDelivered"
    assert [e["version"] for e in events] == [1, 2, 3]

    conflict = await client.post(
        "/api/v1/events/append",
        json={
            "aggregate_id": "sdk-order-1",
            "event_type": "Duplicate",
            "expected_version": 1,
        },
    )
    assert conflict.status_code == 409

    valid_opt = await client.post(
        "/api/v1/events/append",
        json={
            "aggregate_id": "sdk-order-1",
            "event_type": "Completed",
            "expected_version": 3,
        },
    )
    assert valid_opt.status_code == 201
    assert valid_opt.json()["version"] == 4


@pytest.mark.asyncio
async def test_health_checks(client: AsyncClient):
    resp = await client.get("/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    assert isinstance(data["uptime"], (int, float))
    assert isinstance(data["actor_count"], int)
    assert isinstance(data["message_count"], int)

    resp = await client.get("/health/ready")
    assert resp.status_code == 200
    assert resp.json()["status"] == "ok"

    resp = await client.get("/api/v1/info")
    assert resp.status_code == 200
    data = resp.json()
    assert data["version"] == "0.1.0"
    assert isinstance(data["uptime"], (int, float))
    assert isinstance(data["actor_count"], int)
    assert isinstance(data["message_count"], int)


@pytest.mark.asyncio
async def test_actor_listing(client: AsyncClient):
    await client.post(
        "/api/v1/actors",
        json={"actor_id": "sdk-list-w1", "actor_type": "worker"},
    )
    await client.post(
        "/api/v1/actors",
        json={"actor_id": "sdk-list-w2", "actor_type": "worker"},
    )
    await client.post(
        "/api/v1/actors",
        json={"actor_id": "sdk-list-s1", "actor_type": "scheduler"},
    )

    resp = await client.get("/api/v1/actors")
    assert resp.status_code == 200
    all_actors = resp.json()
    ids = [a["actor_id"] for a in all_actors]
    assert "sdk-list-w1" in ids
    assert "sdk-list-w2" in ids
    assert "sdk-list-s1" in ids

    resp = await client.get("/api/v1/actors", params={"type": "worker"})
    workers = resp.json()
    assert len(workers) >= 2
    assert all(a["actor_type"] == "worker" for a in workers)

    resp = await client.get("/api/v1/actors", params={"status": "active"})
    active = resp.json()
    assert all(a["status"] == "active" for a in active)

    resp = await client.get("/api/v1/actors", params={"type": "scheduler"})
    schedulers = resp.json()
    assert all(a["actor_type"] == "scheduler" for a in schedulers)

    resp = await client.get("/api/v1/actors", params={"type": "worker", "status": "active"})
    filtered = resp.json()
    assert all(
        a["actor_type"] == "worker" and a["status"] == "active" for a in filtered
    )


@pytest.mark.asyncio
async def test_message_buffering(client: AsyncClient):
    resp = await client.post(
        "/api/v1/actors/sdk-buf-target/messages",
        json={
            "source_actor": "sdk-buf-sender",
            "target_actor": "sdk-buf-target",
            "payload": {"msg": "before registration"},
        },
    )
    assert resp.status_code == 202
    assert resp.json()["status"] == "buffered"

    await client.post("/api/v1/actors", json={"actor_id": "sdk-buf-target"})

    resp = await client.get("/api/v1/actors/sdk-buf-target/messages")
    assert resp.status_code == 200
    messages = resp.json()
    assert len(messages) >= 1
    assert messages[0]["source_actor"] == "sdk-buf-sender"
    assert messages[0]["payload"]["msg"] == "before registration"

    await client.post(
        "/api/v1/actors/sdk-buf-target/messages",
        json={
            "source_actor": "sdk-buf-sender",
            "target_actor": "sdk-buf-target",
            "payload": {"msg": "after registration"},
        },
    )

    resp = await client.get("/api/v1/actors/sdk-buf-target/messages")
    messages = resp.json()
    assert len(messages) >= 2


@pytest.mark.asyncio
async def test_wildcard_subscriptions(client: AsyncClient):
    sub_resp = await client.post(
        "/api/v1/events/subscribe",
        json={"topic": "sdk-wild-orders.*", "subscriber_id": "sdk-wild-sub-1"},
    )
    assert sub_resp.status_code == 201
    sub_id = sub_resp.json()["subscription_id"]

    topics = (await client.get("/api/v1/events/topics")).json()
    assert "sdk-wild-orders.*" in topics

    pub_exact = await client.post(
        "/api/v1/events/publish",
        json={"topic": "sdk-wild-orders.*", "payload": "exact-match"},
    )
    assert pub_exact.status_code == 202
    assert pub_exact.json()["subscriber_count"] == 1

    pub_sub = await client.post(
        "/api/v1/events/publish",
        json={"topic": "sdk-wild-orders.created", "payload": "sub-topic-msg"},
    )
    assert pub_sub.status_code == 202
    assert pub_sub.json()["subscriber_count"] == 0

    hist = await client.get(
        "/api/v1/events/topics/sdk-wild-orders.created/history"
    )
    assert hist.status_code == 200
    assert len(hist.json()) >= 1
    assert hist.json()[0]["payload"] == "sub-topic-msg"

    unsub = await client.delete(f"/api/v1/events/subscribe/{sub_id}")
    assert unsub.status_code == 204

    topics_after = (await client.get("/api/v1/events/topics")).json()
    assert "sdk-wild-orders.*" not in topics_after


@pytest.mark.asyncio
async def test_concurrent_operations(client: AsyncClient):
    await client.post("/api/v1/actors", json={"actor_id": "sdk-conc-s1"})
    await client.post("/api/v1/actors", json={"actor_id": "sdk-conc-s2"})
    await client.post("/api/v1/actors", json={"actor_id": "sdk-conc-s3"})
    await client.post("/api/v1/actors", json={"actor_id": "sdk-conc-target"})

    async def send_msg(i: int):
        return await client.post(
            "/api/v1/actors/sdk-conc-target/messages",
            json={
                "source_actor": f"sdk-conc-s{i}",
                "target_actor": "sdk-conc-target",
                "payload": {"seq": i},
            },
        )

    results = await asyncio.gather(send_msg(1), send_msg(2), send_msg(3))
    for r in results:
        assert r.status_code == 202

    resp = await client.get("/api/v1/actors/sdk-conc-target/messages")
    messages = resp.json()
    seqs = sorted(
        m["payload"]["seq"]
        for m in messages
        if isinstance(m.get("payload"), dict) and "seq" in m["payload"]
    )
    assert seqs == [1, 2, 3]
