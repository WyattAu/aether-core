"""Tests for the Aether gRPC SDK client.

Tests spin up an in-process gRPC server using the SDK's own generated
stubs and a minimal server implementation, then verify the client works.
This avoids protobuf symbol conflicts between SDK and server packages.
"""

import asyncio
import json

import pytest

# The gRPC client is optional — skip all tests if grpcio not installed
pytest.importorskip("grpc")

import grpc
from aether_sdk.client import (
    ActorInfo,
    DeliveryReceipt,
    EventRecord,
    ServerInfo,
    StateEntry,
)
from aether_sdk.grpc_client import (
    AetherGrpcClient,
    AetherGrpcError,
    _bytes_to_json,
    _json_to_bytes,
)
from aether_sdk.proto import aether_pb2, aether_pb2_grpc

# ============================================================
# Minimal server implementation (using SDK stubs)
# ============================================================


class _MinimalActorService(aether_pb2_grpc.ActorServiceServicer):
    def __init__(self):
        self._actors = {}

    def Register(self, request, context):
        aid = request.actor_id
        if aid in self._actors:
            context.set_code(grpc.StatusCode.ALREADY_EXISTS)
            return aether_pb2.ActorInfo()
        self._actors[aid] = {
            "type": request.actor_type,
            "capabilities": list(request.capabilities),
            "metadata": dict(request.metadata),
            "status": "active",
        }
        return aether_pb2.ActorInfo(
            actor_id=aid,
            actor_type=request.actor_type,
            capabilities=request.capabilities,
            metadata=request.metadata,
            status="active",
        )

    def Unregister(self, request, context):
        self._actors.pop(request.actor_id, None)
        return aether_pb2.UnregisterResponse(success=True)

    def GetActor(self, request, context):
        info = self._actors.get(request.actor_id)
        if not info:
            context.set_code(grpc.StatusCode.NOT_FOUND)
            return aether_pb2.ActorInfo()
        return aether_pb2.ActorInfo(
            actor_id=request.actor_id,
            actor_type=info["type"],
            capabilities=info["capabilities"],
            metadata=info["metadata"],
            status=info["status"],
        )

    def ListActors(self, request, context):
        results = []
        for aid, info in self._actors.items():
            if request.actor_type and info["type"] != request.actor_type:
                continue
            if request.status and info["status"] != request.status:
                continue
            results.append(
                aether_pb2.ActorInfo(
                    actor_id=aid,
                    actor_type=info["type"],
                    capabilities=info["capabilities"],
                    metadata=info["metadata"],
                    status=info["status"],
                )
            )
        return aether_pb2.ListActorsResponse(actors=results, total=len(results))

    def Heartbeat(self, request, context):
        if request.actor_id not in self._actors:
            context.set_code(grpc.StatusCode.NOT_FOUND)
            return aether_pb2.HeartbeatResponse(success=False)
        return aether_pb2.HeartbeatResponse(success=True)


class _MinimalMessageService(aether_pb2_grpc.MessageServiceServicer):
    def __init__(self):
        self._pending = {}

    def Send(self, request, context):
        import time as _time

        mid = f"msg_{int(_time.time() * 1e6)}"
        receipt = aether_pb2.DeliveryReceipt(
            message_id=mid,
            status="buffered",
            correlation_id=request.correlation_id,
        )
        target = request.target_actor
        self._pending.setdefault(target, []).append(receipt)
        return receipt

    def GetPending(self, request, context):
        msgs = self._pending.get(request.actor_id, [])
        # Return as PendingMessage protos (minimal)
        result = []
        for r in msgs:
            result.append(
                aether_pb2.PendingMessage(
                    message_id=r.message_id,
                    correlation_id=r.correlation_id,
                )
            )
        return aether_pb2.GetPendingMessagesResponse(messages=result)


class _MinimalStateService(aether_pb2_grpc.StateServiceServicer):
    def __init__(self):
        self._state = {}
        self._versions = {}

    def GetState(self, request, context):
        key = (request.actor_id, request.key)
        entry = self._state.get(key)
        if entry is None:
            return aether_pb2.GetStateResponse(key=request.key, found=False)
        return aether_pb2.GetStateResponse(
            key=request.key,
            value=_json_to_bytes(entry["value"]),
            version=entry["version"],
            found=True,
        )

    def SetState(self, request, context):
        key = (request.actor_id, request.key)
        value = _bytes_to_json(request.value)
        expected = request.expected_version if request.expected_version > 0 else None

        current_ver = self._versions.get(key, 0)
        if expected is not None and expected != current_ver:
            context.set_code(grpc.StatusCode.ABORTED)
            context.set_details(
                f"Version conflict: expected {expected}, actual {current_ver}"
            )
            return aether_pb2.StateEntry()

        new_ver = current_ver + 1
        self._state[key] = {"value": value, "version": new_ver}
        self._versions[key] = new_ver
        return aether_pb2.StateEntry(
            actor_id=request.actor_id,
            key=request.key,
            value=request.value,
            version=new_ver,
        )

    def DeleteState(self, request, context):
        key = (request.actor_id, request.key)
        deleted = key in self._state
        self._state.pop(key, None)
        self._versions.pop(key, None)
        return aether_pb2.DeleteStateResponse(deleted=deleted)

    def GetAllState(self, request, context):
        prefix = request.actor_id
        result = {}
        for (aid, k), entry in self._state.items():
            if aid == prefix:
                result[k] = _json_to_bytes(entry["value"])
        return aether_pb2.GetAllStateResponse(state=result)


class _MinimalEventService(aether_pb2_grpc.EventServiceServicer):
    def __init__(self):
        self._subscriptions = {}
        self._events = {}

    def Publish(self, request, context):
        topic = request.topic
        subs = self._subscriptions.get(topic, [])
        return aether_pb2.PublishResponse(subscribers_notified=len(subs))

    def Subscribe(self, request, context):
        topic = request.topic
        sub_id = f"sub_{request.subscriber_id}"
        self._subscriptions.setdefault(topic, []).append(sub_id)
        return aether_pb2.SubscribeResponse(subscription_id=sub_id)

    def Unsubscribe(self, request, context):
        for topic, subs in self._subscriptions.items():
            if request.subscription_id in subs:
                subs.remove(request.subscription_id)
                return aether_pb2.UnsubscribeResponse(success=True)
        return aether_pb2.UnsubscribeResponse(success=False)

    def ListTopics(self, request, context):
        return aether_pb2.ListTopicsResponse(topics=list(self._subscriptions.keys()))

    def AppendEvent(self, request, context):
        agg = request.aggregate_id
        _data = _bytes_to_json(request.data)
        current_ver = self._events.get(agg, 0)
        expected = request.expected_version if request.expected_version > 0 else None
        if expected is not None and expected != current_ver:
            context.set_code(grpc.StatusCode.ABORTED)
            return aether_pb2.EventRecord()
        new_ver = current_ver + 1
        self._events[agg] = new_ver
        return aether_pb2.EventRecord(
            event_id=f"evt_{new_ver}",
            aggregate_id=agg,
            event_type=request.event_type,
            data=request.data,
            version=new_ver,
        )

    def GetEvents(self, request, context):
        # Minimal: just return count as empty list (events not stored individually)
        return aether_pb2.GetEventsResponse(events=[])


class _MinimalHealthService(aether_pb2_grpc.HealthServiceServicer):
    def __init__(self):
        self._start = (
            asyncio.get_event_loop().time()
            if asyncio.get_event_loop().is_running()
            else 0
        )

    def Health(self, request, context):
        import time as _time

        return aether_pb2.HealthResponse(
            status="ok",
            uptime=_time.time() - self._start if self._start else 0,
            actor_count=0,
            message_count=0,
        )

    def Ready(self, request, context):
        return self.Health(request, context)

    def Info(self, request, context):
        import time as _time

        return aether_pb2.InfoResponse(
            version="0.1.0",
            status="ok",
            uptime=_time.time() - self._start if self._start else 0,
            actor_count=0,
            message_count=0,
        )


def _start_grpc_server():
    """Start an in-process gRPC server with minimal service implementations."""
    from concurrent import futures

    actor_svc = _MinimalActorService()
    msg_svc = _MinimalMessageService()
    state_svc = _MinimalStateService()
    event_svc = _MinimalEventService()
    health_svc = _MinimalHealthService()

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=4))
    aether_pb2_grpc.add_ActorServiceServicer_to_server(actor_svc, server)
    aether_pb2_grpc.add_MessageServiceServicer_to_server(msg_svc, server)
    aether_pb2_grpc.add_StateServiceServicer_to_server(state_svc, server)
    aether_pb2_grpc.add_EventServiceServicer_to_server(event_svc, server)
    aether_pb2_grpc.add_HealthServiceServicer_to_server(health_svc, server)

    port = server.add_insecure_port("localhost:0")
    server.start()
    return port, server


@pytest.fixture
async def grpc_client():
    """Create a connected gRPC client with a live server."""
    port, server = _start_grpc_server()
    client = AetherGrpcClient(f"localhost:{port}", timeout=5.0)
    await client.connect()
    yield client
    await client.close()
    server.stop(1.0)


# ============================================================
# Unit Tests
# ============================================================


class TestHelpers:
    def test_json_to_bytes_none(self):
        assert _json_to_bytes(None) == b""

    def test_json_to_bytes_value(self):
        result = _json_to_bytes({"key": "val"})
        assert json.loads(result) == {"key": "val"}

    def test_bytes_to_json_empty(self):
        assert _bytes_to_json(b"") is None

    def test_bytes_to_json_none(self):
        assert _bytes_to_json(None) is None

    def test_bytes_to_json_value(self):
        result = _bytes_to_json(b'{"key": "val"}')
        assert result == {"key": "val"}

    def test_roundtrip(self):
        data = {"numbers": [1, 2, 3], "nested": {"a": True}}
        assert _bytes_to_json(_json_to_bytes(data)) == data


# ============================================================
# Integration Tests
# ============================================================


class TestGrpcClientHealth:

    async def test_health(self, grpc_client):
        info = await grpc_client.health()
        assert isinstance(info, ServerInfo)
        assert info.status == "ok"

    async def test_info(self, grpc_client):
        info = await grpc_client.info()
        assert isinstance(info, dict)
        assert "version" in info
        assert info["status"] == "ok"


class TestGrpcClientActors:

    async def test_register_actor(self, grpc_client):
        info = await grpc_client.register_actor("grpc-1", "worker")
        assert isinstance(info, ActorInfo)
        assert info.actor_id == "grpc-1"
        assert info.actor_type == "worker"
        assert info.status == "active"

    async def test_register_with_capabilities(self, grpc_client):
        info = await grpc_client.register_actor(
            "grpc-2",
            "compute",
            capabilities=["gpu", "cuda"],
            metadata={"region": "us-east"},
        )
        assert "gpu" in info.capabilities
        assert info.metadata["region"] == "us-east"

    async def test_get_actor(self, grpc_client):
        await grpc_client.register_actor("grpc-3")
        info = await grpc_client.get_actor("grpc-3")
        assert info.actor_id == "grpc-3"

    async def test_get_actor_not_found(self, grpc_client):
        with pytest.raises(AetherGrpcError) as exc_info:
            await grpc_client.get_actor("nonexistent")
        assert exc_info.value.code == "NOT_FOUND"

    async def test_list_actors(self, grpc_client):
        await grpc_client.register_actor("list-1", "worker")
        await grpc_client.register_actor("list-2", "worker")
        actors = await grpc_client.list_actors(actor_type="worker")
        assert len(actors) == 2

    async def test_unregister_actor(self, grpc_client):
        await grpc_client.register_actor("unreg-1")
        await grpc_client.unregister_actor("unreg-1")
        with pytest.raises(AetherGrpcError):
            await grpc_client.get_actor("unreg-1")

    async def test_heartbeat(self, grpc_client):
        await grpc_client.register_actor("hb-1")
        await grpc_client.heartbeat("hb-1")  # Should not raise


class TestGrpcClientMessaging:

    async def test_send_message(self, grpc_client):
        receipt = await grpc_client.send_message(
            "msg-tgt",
            {"text": "hello"},
            source="msg-src",
        )
        assert isinstance(receipt, DeliveryReceipt)
        assert receipt.status == "buffered"
        assert receipt.message_id != ""

    async def test_send_message_with_correlation(self, grpc_client):
        receipt = await grpc_client.send_message(
            "corr-tgt",
            {"data": 42},
            correlation_id="corr-123",
        )
        assert receipt.correlation_id == "corr-123"

    async def test_get_pending_messages(self, grpc_client):
        await grpc_client.send_message("pend-tgt", {"x": 1}, source="pend-src")
        msgs = await grpc_client.get_pending_messages("pend-tgt")
        assert len(msgs) >= 1
        assert msgs[0].correlation_id is None


class TestGrpcClientState:

    async def test_set_and_get_state(self, grpc_client):
        entry = await grpc_client.set_state("st-1", "counter", 42)
        assert isinstance(entry, StateEntry)
        assert entry.actor_id == "st-1"
        assert entry.key == "counter"
        assert entry.value == 42
        assert entry.version == 1

        value = await grpc_client.get_state("st-1", "counter")
        assert value == 42

    async def test_get_state_not_found(self, grpc_client):
        value = await grpc_client.get_state("ghost", "missing")
        assert value is None

    async def test_state_version_increment(self, grpc_client):
        await grpc_client.set_state("ver-1", "k", "v1")
        e2 = await grpc_client.set_state("ver-1", "k", "v2")
        assert e2.version == 2

    async def test_delete_state(self, grpc_client):
        await grpc_client.set_state("del-1", "temp", "val")
        deleted = await grpc_client.delete_state("del-1", "temp")
        assert deleted is True
        value = await grpc_client.get_state("del-1", "temp")
        assert value is None

    async def test_delete_state_not_found(self, grpc_client):
        deleted = await grpc_client.delete_state("ghost", "key")
        assert deleted is False

    async def test_get_all_state(self, grpc_client):
        await grpc_client.set_state("all-1", "a", 1)
        await grpc_client.set_state("all-1", "b", 2)
        state = await grpc_client.get_all_state("all-1")
        assert state == {"a": 1, "b": 2}

    async def test_complex_state_value(self, grpc_client):
        complex_val = {"nested": {"deep": [1, 2, 3]}, "flag": True}
        await grpc_client.set_state("cx-1", "config", complex_val)
        result = await grpc_client.get_state("cx-1", "config")
        assert result == complex_val


class TestGrpcClientEvents:

    async def test_publish_and_subscribe(self, grpc_client):
        sub_id = await grpc_client.subscribe("grpc-topic", "sub-1")
        assert isinstance(sub_id, str)
        assert len(sub_id) > 0

        count = await grpc_client.publish("grpc-topic", {"msg": "hello"})
        assert count == 1

        unsub = await grpc_client.unsubscribe(sub_id)
        assert unsub is True

    async def test_list_topics(self, grpc_client):
        await grpc_client.subscribe("topic-a", "s1")
        await grpc_client.subscribe("topic-b", "s2")
        topics = await grpc_client.list_topics()
        assert "topic-a" in topics
        assert "topic-b" in topics

    async def test_append_event(self, grpc_client):
        record = await grpc_client.append_event(
            aggregate_id="order-1",
            event_type="OrderCreated",
            data={"item": "widget", "qty": 5},
        )
        assert isinstance(record, EventRecord)
        assert record.aggregate_id == "order-1"
        assert record.event_type == "OrderCreated"
        assert record.version == 1
        assert record.data == {"item": "widget", "qty": 5}

    async def test_append_sequential_events(self, grpc_client):
        await grpc_client.append_event("seq-1", "Created")
        await grpc_client.append_event("seq-1", "Updated")
        r3 = await grpc_client.append_event("seq-1", "Completed")
        assert r3.version == 3


class TestGrpcClientContextManager:

    async def test_context_manager(self):
        port, server = _start_grpc_server()
        async with AetherGrpcClient(f"localhost:{port}", timeout=5.0) as client:
            info = await client.health()
            assert info.status == "ok"
        server.stop(1.0)

    async def test_not_connected_raises(self):
        client = AetherGrpcClient("localhost:50051")
        with pytest.raises(RuntimeError, match="not connected"):
            await client.health()


class TestGrpcClientError:

    async def test_version_conflict(self, grpc_client):
        await grpc_client.set_state("vc-1", "val", "v1")
        await grpc_client.set_state("vc-1", "val", "v2")
        with pytest.raises(AetherGrpcError) as exc_info:
            await grpc_client.set_state("vc-1", "val", "v3", version=1)
        assert exc_info.value.code == "ABORTED"

    async def test_duplicate_actor(self, grpc_client):
        await grpc_client.register_actor("dup-1")
        with pytest.raises(AetherGrpcError) as exc_info:
            await grpc_client.register_actor("dup-1")
        assert exc_info.value.code == "ALREADY_EXISTS"
