import pytest
from fastapi.testclient import TestClient

from server.app import app


@pytest.fixture
def client():
    with TestClient(app) as c:
        yield c


class TestActors:
    def test_register_actor(self, client):
        resp = client.post("/api/v1/actors", json={
            "actor_id": "actor-1",
            "actor_type": "worker",
            "capabilities": ["compute"],
        })
        assert resp.status_code == 201
        data = resp.json()
        assert data["actor_id"] == "actor-1"
        assert data["actor_type"] == "worker"
        assert data["status"] == "active"

    def test_register_duplicate_actor(self, client):
        client.post("/api/v1/actors", json={"actor_id": "dup-1"})
        resp = client.post("/api/v1/actors", json={"actor_id": "dup-1"})
        assert resp.status_code == 409

    def test_get_actor(self, client):
        client.post("/api/v1/actors", json={"actor_id": "get-1"})
        resp = client.get("/api/v1/actors/get-1")
        assert resp.status_code == 200
        assert resp.json()["actor_id"] == "get-1"

    def test_get_actor_not_found(self, client):
        resp = client.get("/api/v1/actors/nonexistent")
        assert resp.status_code == 404

    def test_list_actors(self, client):
        client.post("/api/v1/actors", json={"actor_id": "list-1", "actor_type": "worker"})
        client.post("/api/v1/actors", json={"actor_id": "list-2", "actor_type": "scheduler"})
        resp = client.get("/api/v1/actors")
        assert resp.status_code == 200
        actors = resp.json()
        assert len(actors) >= 2

    def test_list_actors_by_type(self, client):
        client.post("/api/v1/actors", json={"actor_id": "filter-1", "actor_type": "worker"})
        resp = client.get("/api/v1/actors", params={"type": "worker"})
        actors = resp.json()
        assert all(a["actor_type"] == "worker" for a in actors)

    def test_unregister_actor(self, client):
        client.post("/api/v1/actors", json={"actor_id": "del-1"})
        resp = client.delete("/api/v1/actors/del-1")
        assert resp.status_code == 204
        resp = client.get("/api/v1/actors/del-1")
        assert resp.status_code == 404

    def test_heartbeat(self, client):
        client.post("/api/v1/actors", json={"actor_id": "hb-1"})
        resp = client.post("/api/v1/actors/hb-1/heartbeat")
        assert resp.status_code == 204


class TestMessaging:
    def test_send_message(self, client):
        client.post("/api/v1/actors", json={"actor_id": "sender-1"})
        client.post("/api/v1/actors", json={"actor_id": "receiver-1"})
        resp = client.post("/api/v1/actors/receiver-1/messages", json={
            "source_actor": "sender-1",
            "target_actor": "receiver-1",
            "message_type": "greeting",
            "payload": {"msg": "hello"},
        })
        assert resp.status_code == 202
        data = resp.json()
        assert data["status"] in ("delivered", "buffered")

    def test_get_pending_messages(self, client):
        client.post("/api/v1/actors", json={"actor_id": "pending-sender"})
        client.post("/api/v1/actors", json={"actor_id": "pending-target"})
        client.post("/api/v1/actors/pending-target/messages", json={
            "source_actor": "pending-sender",
            "target_actor": "pending-target",
            "payload": "test",
        })
        resp = client.get("/api/v1/actors/pending-target/messages")
        assert resp.status_code == 200
        messages = resp.json()
        assert len(messages) >= 1


class TestState:
    def test_set_state(self, client):
        resp = client.put("/api/v1/state/actor-s1/count", json={"value": 42})
        assert resp.status_code == 200
        assert resp.json()["value"] == 42
        assert resp.json()["version"] == 1

    def test_get_state(self, client):
        client.put("/api/v1/state/actor-s2/name", json={"value": "test"})
        resp = client.get("/api/v1/state/actor-s2/name")
        assert resp.status_code == 200
        assert resp.json()["value"] == "test"

    def test_get_state_not_found(self, client):
        resp = client.get("/api/v1/state/noactor/nokey")
        assert resp.status_code == 404

    def test_update_state_version(self, client):
        r1 = client.put("/api/v1/state/actor-s3/x", json={"value": 1})
        v1 = r1.json()["version"]
        r2 = client.put("/api/v1/state/actor-s3/x", json={"value": 2, "version": v1})
        assert r2.status_code == 200
        assert r2.json()["version"] == v1 + 1

    def test_version_conflict(self, client):
        client.put("/api/v1/state/actor-s4/y", json={"value": 1})
        resp = client.put("/api/v1/state/actor-s4/y", json={"value": 2, "version": 999})
        assert resp.status_code == 409

    def test_delete_state(self, client):
        client.put("/api/v1/state/actor-s5/z", json={"value": "bye"})
        resp = client.delete("/api/v1/state/actor-s5/z")
        assert resp.status_code == 204
        resp = client.get("/api/v1/state/actor-s5/z")
        assert resp.status_code == 404

    def test_get_all_state(self, client):
        client.put("/api/v1/state/actor-s6/a", json={"value": 1})
        client.put("/api/v1/state/actor-s6/b", json={"value": 2})
        resp = client.get("/api/v1/state/actor-s6")
        assert resp.status_code == 200
        state = resp.json()["state"]
        assert state["a"] == 1
        assert state["b"] == 2


class TestPubSub:
    def test_publish(self, client):
        resp = client.post("/api/v1/events/publish", json={
            "topic": "test.topic",
            "payload": {"msg": "hello"},
        })
        assert resp.status_code == 202

    def test_subscribe(self, client):
        resp = client.post("/api/v1/events/subscribe", json={
            "topic": "orders.*",
            "subscriber_id": "handler-1",
        })
        assert resp.status_code == 201
        assert "subscription_id" in resp.json()

    def test_unsubscribe(self, client):
        sub = client.post("/api/v1/events/subscribe", json={
            "topic": "temp.topic",
            "subscriber_id": "temp-sub",
        }).json()
        resp = client.delete(f"/api/v1/events/subscribe/{sub['subscription_id']}")
        assert resp.status_code == 204

    def test_list_topics(self, client):
        client.post("/api/v1/events/subscribe", json={"topic": "topic-list-1", "subscriber_id": "s1"})
        resp = client.get("/api/v1/events/topics")
        assert resp.status_code == 200
        assert "topic-list-1" in resp.json()

    def test_list_subscribers(self, client):
        client.post("/api/v1/events/subscribe", json={"topic": "sub-list-topic", "subscriber_id": "sub-1"})
        resp = client.get("/api/v1/events/topics/sub-list-topic/subscribers")
        assert resp.status_code == 200
        assert "sub-1" in resp.json()

    def test_topic_history(self, client):
        client.post("/api/v1/events/publish", json={"topic": "hist-topic", "payload": 1})
        client.post("/api/v1/events/publish", json={"topic": "hist-topic", "payload": 2})
        resp = client.get("/api/v1/events/topics/hist-topic/history")
        assert resp.status_code == 200
        assert len(resp.json()) >= 2


class TestEventStore:
    def test_append_event(self, client):
        resp = client.post("/api/v1/events/append", json={
            "aggregate_id": "order-123",
            "event_type": "OrderCreated",
            "data": {"item": "widget"},
        })
        assert resp.status_code == 201
        data = resp.json()
        assert data["aggregate_id"] == "order-123"
        assert data["event_type"] == "OrderCreated"
        assert data["version"] == 1

    def test_append_sequential(self, client):
        client.post("/api/v1/events/append", json={
            "aggregate_id": "order-seq",
            "event_type": "Created",
        })
        resp = client.post("/api/v1/events/append", json={
            "aggregate_id": "order-seq",
            "event_type": "Updated",
        })
        assert resp.json()["version"] == 2

    def test_version_conflict(self, client):
        client.post("/api/v1/events/append", json={
            "aggregate_id": "order-conflict",
            "event_type": "Created",
        })
        resp = client.post("/api/v1/events/append", json={
            "aggregate_id": "order-conflict",
            "event_type": "Updated",
            "expected_version": 0,
        })
        assert resp.status_code == 409

    def test_get_events(self, client):
        client.post("/api/v1/events/append", json={
            "aggregate_id": "order-get",
            "event_type": "Created",
        })
        client.post("/api/v1/events/append", json={
            "aggregate_id": "order-get",
            "event_type": "Shipped",
        })
        resp = client.get("/api/v1/events/order-get")
        assert resp.status_code == 200
        events = resp.json()
        assert len(events) == 2


class TestHealth:
    def test_health(self, client):
        resp = client.get("/health")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "ok"
        assert "uptime" in data

    def test_ready(self, client):
        resp = client.get("/health/ready")
        assert resp.status_code == 200
        assert resp.json()["status"] == "ok"

    def test_info(self, client):
        resp = client.get("/api/v1/info")
        assert resp.status_code == 200
        data = resp.json()
        assert data["version"] == "0.1.0"
        assert "uptime" in data
