"""Tests for the gRPC server services."""

import json
import time

import grpc
import pytest

from server.config import ServerConfig
from server.grpc_server import (
    ActorServiceServicer,
    EventServiceServicer,
    HealthServiceServicer,
    MessageServiceServicer,
    StateServiceServicer,
    create_grpc_server,
)
from server.proto.aether.v1 import aether_pb2, aether_pb2_grpc


# ============================================================
# Fixtures
# ============================================================

@pytest.fixture
def services():
    """Create server components and gRPC servicers."""
    from server.actor_manager import ActorManager
    from server.event_store import EventStore
    from server.message_router import MessageRouter
    from server.pubsub_service import PubSubService
    from server.state_store import MemoryStateStore

    config = ServerConfig()
    actors = ActorManager(config)
    messages = MessageRouter(message_ttl=300)
    state = MemoryStateStore()
    pubsub = PubSubService()
    events = EventStore()
    return actors, messages, state, pubsub, events


@pytest.fixture
def grpc_channel(services):
    """Create an in-process gRPC channel with all services registered."""
    actors, messages, state, pubsub, events = services
    server = create_grpc_server(actors, messages, state, pubsub, events, max_workers=4)
    port = server.add_insecure_port("localhost:0")
    server.start()
    channel = grpc.insecure_channel(f"localhost:{port}")
    yield channel
    channel.close()
    server.stop(1.0)


# ============================================================
# Actor Service Tests
# ============================================================

class TestActorService:

    def test_register_actor(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        resp = stub.Register(aether_pb2.RegisterActorRequest(
            actor_id="test-actor-1",
            actor_type="worker",
            capabilities=["process", "compute"],
        ))
        assert resp.actor_id == "test-actor-1"
        assert resp.actor_type == "worker"
        assert "process" in resp.capabilities
        assert resp.status == "active"

    def test_register_duplicate_actor(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="dup-1"))
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.Register(aether_pb2.RegisterActorRequest(actor_id="dup-1"))
        assert exc_info.value.code() == grpc.StatusCode.ALREADY_EXISTS

    def test_get_actor(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="get-1"))
        resp = stub.GetActor(aether_pb2.GetActorRequest(actor_id="get-1"))
        assert resp.actor_id == "get-1"

    def test_get_actor_not_found(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.GetActor(aether_pb2.GetActorRequest(actor_id="nonexistent"))
        assert exc_info.value.code() == grpc.StatusCode.NOT_FOUND

    def test_list_actors(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="list-1", actor_type="worker"))
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="list-2", actor_type="scheduler"))
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="list-3", actor_type="worker"))

        resp = stub.ListActors(aether_pb2.ListActorsRequest(actor_type="worker"))
        assert len(resp.actors) == 2
        assert resp.total == 2

    def test_list_actors_all(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="all-1"))
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="all-2"))
        resp = stub.ListActors(aether_pb2.ListActorsRequest())
        assert len(resp.actors) == 2

    def test_unregister_actor(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="unreg-1"))
        resp = stub.Unregister(aether_pb2.UnregisterActorRequest(actor_id="unreg-1"))
        assert resp.success

    def test_heartbeat(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        stub.Register(aether_pb2.RegisterActorRequest(actor_id="hb-1"))
        resp = stub.Heartbeat(aether_pb2.HeartbeatRequest(actor_id="hb-1"))
        assert resp.success

    def test_heartbeat_not_found(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.Heartbeat(aether_pb2.HeartbeatRequest(actor_id="ghost"))
        assert exc_info.value.code() == grpc.StatusCode.NOT_FOUND

    def test_register_with_metadata(self, grpc_channel):
        stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        resp = stub.Register(aether_pb2.RegisterActorRequest(
            actor_id="meta-1",
            metadata={"region": "us-east", "version": "2.0"},
        ))
        assert resp.metadata["region"] == "us-east"


# ============================================================
# Message Service Tests
# ============================================================

class TestMessageService:

    def test_send_message_buffered(self, grpc_channel):
        msg_stub = aether_pb2_grpc.MessageServiceStub(grpc_channel)
        actor_stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        actor_stub.Register(aether_pb2.RegisterActorRequest(actor_id="msg-src"))
        actor_stub.Register(aether_pb2.RegisterActorRequest(actor_id="msg-tgt"))

        resp = msg_stub.Send(aether_pb2.SendMessageRequest(
            source_actor="msg-src",
            target_actor="msg-tgt",
            message_type="greeting",
            payload=json.dumps({"text": "hello"}).encode(),
        ))
        assert resp.status in ("delivered", "buffered")
        assert resp.message_id != ""

    def test_send_message_with_correlation_id(self, grpc_channel):
        msg_stub = aether_pb2_grpc.MessageServiceStub(grpc_channel)
        resp = msg_stub.Send(aether_pb2.SendMessageRequest(
            source_actor="corr-src",
            target_actor="corr-tgt",
            correlation_id="corr-123",
        ))
        assert resp.correlation_id == "corr-123"

    def test_get_pending_messages(self, grpc_channel):
        msg_stub = aether_pb2_grpc.MessageServiceStub(grpc_channel)
        msg_stub.Send(aether_pb2.SendMessageRequest(
            source_actor="pend-src",
            target_actor="pend-tgt",
            message_type="task",
            payload=b'{"data": 42}',
        ))
        resp = msg_stub.GetPending(aether_pb2.GetPendingMessagesRequest(
            actor_id="pend-tgt",
        ))
        assert len(resp.messages) >= 1
        assert resp.messages[0].source_actor == "pend-src"


# ============================================================
# State Service Tests
# ============================================================

class TestStateService:

    def test_set_and_get_state(self, grpc_channel):
        stub = aether_pb2_grpc.StateServiceStub(grpc_channel)
        stub.SetState(aether_pb2.SetStateRequest(
            actor_id="state-1",
            key="counter",
            value=json.dumps(42).encode(),
        ))
        resp = stub.GetState(aether_pb2.GetStateRequest(
            actor_id="state-1",
            key="counter",
        ))
        assert resp.found
        assert json.loads(resp.value) == 42

    def test_get_state_not_found(self, grpc_channel):
        stub = aether_pb2_grpc.StateServiceStub(grpc_channel)
        resp = stub.GetState(aether_pb2.GetStateRequest(
            actor_id="ghost",
            key="missing",
        ))
        assert not resp.found

    def test_set_state_version_conflict(self, grpc_channel):
        stub = aether_pb2_grpc.StateServiceStub(grpc_channel)
        stub.SetState(aether_pb2.SetStateRequest(
            actor_id="vc-1",
            key="val",
            value=json.dumps("v1").encode(),
        ))
        stub.SetState(aether_pb2.SetStateRequest(
            actor_id="vc-1",
            key="val",
            value=json.dumps("v2").encode(),
        ))
        # Version is now 2 — try to set with stale expected_version=1
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.SetState(aether_pb2.SetStateRequest(
                actor_id="vc-1",
                key="val",
                value=json.dumps("v3").encode(),
                expected_version=1,  # Stale — current version is 2
            ))
        assert exc_info.value.code() == grpc.StatusCode.ABORTED

    def test_delete_state(self, grpc_channel):
        stub = aether_pb2_grpc.StateServiceStub(grpc_channel)
        stub.SetState(aether_pb2.SetStateRequest(
            actor_id="del-1",
            key="temp",
            value=b'"erase me"',
        ))
        resp = stub.DeleteState(aether_pb2.DeleteStateRequest(
            actor_id="del-1",
            key="temp",
        ))
        assert resp.deleted

    def test_delete_state_not_found(self, grpc_channel):
        stub = aether_pb2_grpc.StateServiceStub(grpc_channel)
        resp = stub.DeleteState(aether_pb2.DeleteStateRequest(
            actor_id="ghost",
            key="ghost-key",
        ))
        assert not resp.deleted

    def test_get_all_state(self, grpc_channel):
        stub = aether_pb2_grpc.StateServiceStub(grpc_channel)
        stub.SetState(aether_pb2.SetStateRequest(
            actor_id="all-1",
            key="a",
            value=json.dumps(1).encode(),
        ))
        stub.SetState(aether_pb2.SetStateRequest(
            actor_id="all-1",
            key="b",
            value=json.dumps(2).encode(),
        ))
        resp = stub.GetAllState(aether_pb2.GetAllStateRequest(actor_id="all-1"))
        assert len(resp.state) == 2
        assert json.loads(resp.state["a"]) == 1


# ============================================================
# Event Service Tests
# ============================================================

class TestEventService:

    def test_publish_and_subscribe(self, grpc_channel):
        stub = aether_pb2_grpc.EventServiceStub(grpc_channel)
        # Subscribe first
        sub_resp = stub.Subscribe(aether_pb2.SubscribeRequest(
            topic="orders",
            subscriber_id="sub-1",
        ))
        assert sub_resp.subscription_id != ""

        # Publish
        pub_resp = stub.Publish(aether_pb2.PublishRequest(
            topic="orders",
            payload=json.dumps({"order_id": 1}).encode(),
        ))
        assert pub_resp.subscribers_notified == 1

        # Unsubscribe
        unsub_resp = stub.Unsubscribe(aether_pb2.UnsubscribeRequest(
            subscription_id=sub_resp.subscription_id,
        ))
        assert unsub_resp.success

    def test_list_topics(self, grpc_channel):
        stub = aether_pb2_grpc.EventServiceStub(grpc_channel)
        stub.Subscribe(aether_pb2.SubscribeRequest(
            topic="topic-a", subscriber_id="s1",
        ))
        stub.Subscribe(aether_pb2.SubscribeRequest(
            topic="topic-b", subscriber_id="s2",
        ))
        resp = stub.ListTopics(aether_pb2.ListTopicsRequest())
        assert "topic-a" in resp.topics
        assert "topic-b" in resp.topics

    def test_append_event(self, grpc_channel):
        stub = aether_pb2_grpc.EventServiceStub(grpc_channel)
        resp = stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="order-123",
            event_type="OrderCreated",
            data=json.dumps({"item": "widget", "qty": 5}).encode(),
        ))
        assert resp.event_id != ""
        assert resp.aggregate_id == "order-123"
        assert resp.event_type == "OrderCreated"
        assert resp.version == 1

    def test_append_sequential_events(self, grpc_channel):
        stub = aether_pb2_grpc.EventServiceStub(grpc_channel)
        stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="seq-1", event_type="Created",
        ))
        stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="seq-1", event_type="Updated",
        ))
        resp = stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="seq-1", event_type="Completed",
        ))
        assert resp.version == 3

    def test_append_event_version_conflict(self, grpc_channel):
        stub = aether_pb2_grpc.EventServiceStub(grpc_channel)
        stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="conflict-1", event_type="E1",
        ))
        stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="conflict-1", event_type="E2",
        ))
        # Version is now 2 — try to append with stale expected_version=1
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.AppendEvent(aether_pb2.AppendEventRequest(
                aggregate_id="conflict-1",
                event_type="E3",
                expected_version=1,  # Stale — current version is 2
            ))
        assert exc_info.value.code() == grpc.StatusCode.ABORTED

    def test_get_events(self, grpc_channel):
        stub = aether_pb2_grpc.EventServiceStub(grpc_channel)
        stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="get-1", event_type="Created",
            data=b'{"x": 1}',
        ))
        stub.AppendEvent(aether_pb2.AppendEventRequest(
            aggregate_id="get-1", event_type="Updated",
            data=b'{"x": 2}',
        ))
        resp = stub.GetEvents(aether_pb2.GetEventsRequest(aggregate_id="get-1"))
        assert len(resp.events) == 2
        assert resp.events[0].event_type == "Created"
        assert resp.events[1].version == 2


# ============================================================
# Health Service Tests
# ============================================================

class TestHealthService:

    def test_health(self, grpc_channel):
        stub = aether_pb2_grpc.HealthServiceStub(grpc_channel)
        resp = stub.Health(aether_pb2.Empty())
        assert resp.status == "ok"
        assert resp.uptime >= 0

    def test_ready(self, grpc_channel):
        stub = aether_pb2_grpc.HealthServiceStub(grpc_channel)
        resp = stub.Ready(aether_pb2.Empty())
        assert resp.status == "ok"

    def test_info(self, grpc_channel):
        stub = aether_pb2_grpc.HealthServiceStub(grpc_channel)
        resp = stub.Info(aether_pb2.Empty())
        assert resp.version == "0.1.0"
        assert resp.status == "ok"
        assert resp.uptime >= 0

    def test_actor_count_in_health(self, grpc_channel):
        actor_stub = aether_pb2_grpc.ActorServiceStub(grpc_channel)
        health_stub = aether_pb2_grpc.HealthServiceStub(grpc_channel)
        actor_stub.Register(aether_pb2.RegisterActorRequest(actor_id="hc-1"))
        actor_stub.Register(aether_pb2.RegisterActorRequest(actor_id="hc-2"))
        resp = health_stub.Info(aether_pb2.Empty())
        assert resp.actor_count >= 2
