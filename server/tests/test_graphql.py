import pytest

from server.api.graphql import GRAPHQL_AVAILABLE, graphql_app


skip_without_graphql = pytest.mark.skipif(
    not GRAPHQL_AVAILABLE,
    reason="strawberry-graphql not installed",
)


@skip_without_graphql
class TestGraphQLActors:
    def test_register_actor(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            resp = client.post("/graphql", json={
                "query": """
                mutation {
                    registerActor(actorId: "gql-actor-1", actorType: "worker") {
                        actorId
                        actorType
                        status
                    }
                }
                """
            })
            assert resp.status_code == 200
            data = resp.json()["data"]["registerActor"]
            assert data["actorId"] == "gql-actor-1"
            assert data["actorType"] == "worker"
            assert data["status"] == "active"

    def test_query_actors(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/graphql", json={
                "query": 'mutation { registerActor(actorId: "gql-list-1", actorType: "scheduler") { actorId } }'
            })
            resp = client.post("/graphql", json={
                "query": "{ actors { actorId actorType status } }"
            })
            assert resp.status_code == 200
            actors = resp.json()["data"]["actors"]
            assert len(actors) >= 1

    def test_query_actor(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/graphql", json={
                "query": 'mutation { registerActor(actorId: "gql-get-1", actorType: "worker") { actorId } }'
            })
            resp = client.post("/graphql", json={
                "query": '{ actor(actorId: "gql-get-1") { actorId actorType } }'
            })
            assert resp.status_code == 200
            actor = resp.json()["data"]["actor"]
            assert actor is not None
            assert actor["actorId"] == "gql-get-1"

    def test_query_actor_not_found(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            resp = client.post("/graphql", json={
                "query": '{ actor(actorId: "nonexistent-xyz") { actorId } }'
            })
            assert resp.status_code == 200
            assert resp.json()["data"]["actor"] is None


@skip_without_graphql
class TestGraphQLState:
    def test_set_and_query_state(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/graphql", json={
                "query": 'mutation { registerActor(actorId: "gql-state-1", actorType: "worker") { actorId } }'
            })
            resp = client.post("/graphql", json={
                "query": 'mutation { setState(actorId: "gql-state-1", key: "count", value: "42") { key value version } }'
            })
            assert resp.status_code == 200
            data = resp.json()["data"]["setState"]
            assert data["key"] == "count"
            assert data["value"] == "42"
            assert data["version"] == 1

    def test_actor_state(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/graphql", json={
                "query": 'mutation { registerActor(actorId: "gql-state-2", actorType: "worker") { actorId } }'
            })
            client.post("/graphql", json={
                "query": 'mutation { setState(actorId: "gql-state-2", key: "name", value: "test") { key } }'
            })
            resp = client.post("/graphql", json={
                "query": '{ actorState(actorId: "gql-state-2") { key value version } }'
            })
            assert resp.status_code == 200
            entries = resp.json()["data"]["actorState"]
            assert len(entries) >= 1
            assert any(e["key"] == "name" for e in entries)

    def test_actor_state_not_found(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            resp = client.post("/graphql", json={
                "query": '{ actorState(actorId: "nonexistent-xyz") { key value } }'
            })
            assert resp.status_code == 200
            errors = resp.json().get("errors")
            assert errors is not None


@skip_without_graphql
class TestGraphQLEvents:
    def test_query_events(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/api/v1/events/append", json={
                "aggregate_id": "gql-order-1",
                "event_type": "Created",
                "data": {"item": "test"},
            })
            resp = client.post("/graphql", json={
                "query": '{ events(aggregateId: "gql-order-1") { eventId eventType aggregateId version } }'
            })
            assert resp.status_code == 200
            events = resp.json()["data"]["events"]
            assert len(events) >= 1
            assert events[0]["aggregateId"] == "gql-order-1"


@skip_without_graphql
class TestGraphQLTopics:
    def test_query_topics(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/api/v1/events/subscribe", json={
                "topic": "gql-test-topic",
                "subscriber_id": "gql-sub-1",
            })
            resp = client.post("/graphql", json={
                "query": "{ topics { name subscriberCount messageCount } }"
            })
            assert resp.status_code == 200
            topics = resp.json()["data"]["topics"]
            assert any(t["name"] == "gql-test-topic" for t in topics)

    def test_topic_history(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/api/v1/events/publish", json={"topic": "gql-hist-topic", "payload": "msg1"})
            resp = client.post("/graphql", json={
                "query": '{ topicHistory(topic: "gql-hist-topic") { payload timestamp } }'
            })
            assert resp.status_code == 200
            msgs = resp.json()["data"]["topicHistory"]
            assert len(msgs) >= 1


@skip_without_graphql
class TestGraphQLMutations:
    def test_send_message(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            client.post("/graphql", json={
                "query": 'mutation { registerActor(actorId: "gql-msg-1", actorType: "worker") { actorId } }'
            })
            resp = client.post("/graphql", json={
                "query": 'mutation { sendMessage(target: "gql-msg-1", payload: "hello", messageType: "cmd") { messageId targetActor payload } }'
            })
            assert resp.status_code == 200
            data = resp.json()["data"]["sendMessage"]
            assert data["targetActor"] == "gql-msg-1"
            assert data["payload"] == "hello"
