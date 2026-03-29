"""gRPC server for the Aether reference server.

Implements all Aether services defined in ``proto/aether/v1/aether.proto``
by delegating to the existing server components (ActorManager, MessageRouter,
StateStore, PubSubService, EventStore).

Usage::

    from server.grpc_server import create_grpc_server, serve_grpc

    server = create_grpc_server()
    serve_grpc(server, host="0.0.0.0", port=50051)
"""

import json
import logging
import time
from concurrent import futures
from datetime import datetime, timezone
from typing import Optional

import grpc

from .actor_manager import ActorManager
from .message_router import MessageRouter
from .state_store import StateStore
from .pubsub_service import PubSubService
from .event_store import EventStore

logger = logging.getLogger("aether-server.grpc")

# Import generated stubs
try:
    from .proto.aether.v1 import aether_pb2, aether_pb2_grpc
except ImportError:
    aether_pb2 = None
    aether_pb2_grpc = None
    logger.warning("gRPC stubs not generated. Run: python -m grpc_tools.protoc -I. "
                   "--python_out=. --grpc_python_out=. proto/aether/v1/aether.proto")


def _dt_to_timestamp(dt: Optional[datetime]) -> "aether_pb2.Timestamp":
    """Convert a Python datetime to a protobuf Timestamp."""
    if aether_pb2 is None:
        raise RuntimeError("gRPC stubs not available")
    ts = aether_pb2.Timestamp()
    if dt is not None:
        ts.seconds = int(dt.timestamp())
        ts.nanos = int((dt.timestamp() % 1) * 1e9)
    return ts


def _now_timestamp() -> "aether_pb2.Timestamp":
    """Get a protobuf Timestamp for the current time."""
    return _dt_to_timestamp(datetime.now(timezone.utc))


# ============================================================
# Service Implementations
# ============================================================

class ActorServiceServicer(aether_pb2_grpc.ActorServiceServicer):
    """gRPC implementation of the Actor service."""

    def __init__(self, actor_manager: ActorManager):
        self._mgr = actor_manager

    def Register(self, request, context):
        try:
            info = self._mgr.register(
                actor_id=request.actor_id,
                actor_type=request.actor_type or "default",
                capabilities=list(request.capabilities),
                metadata=dict(request.metadata),
            )
            return _actor_info_to_proto(info)
        except ValueError as e:
            context.set_code(grpc.StatusCode.ALREADY_EXISTS)
            context.set_details(str(e))
            return aether_pb2.ActorInfo()
        except RuntimeError as e:
            context.set_code(grpc.StatusCode.RESOURCE_EXHAUSTED)
            context.set_details(str(e))
            return aether_pb2.ActorInfo()

    def Unregister(self, request, context):
        success = self._mgr.unregister(request.actor_id)
        return aether_pb2.UnregisterResponse(success=success)

    def GetActor(self, request, context):
        info = self._mgr.get_actor(request.actor_id)
        if info is None:
            context.set_code(grpc.StatusCode.NOT_FOUND)
            context.set_details(f"Actor {request.actor_id} not found")
            return aether_pb2.ActorInfo()
        return _actor_info_to_proto(info)

    def ListActors(self, request, context):
        actors = self._mgr.list_actors(
            actor_type=request.actor_type or None,
            status=request.status or None,
        )
        proto_actors = [_actor_info_to_proto(a) for a in actors]
        return aether_pb2.ListActorsResponse(
            actors=proto_actors,
            total=len(proto_actors),
        )

    def Heartbeat(self, request, context):
        success = self._mgr.update_heartbeat(request.actor_id)
        if not success:
            context.set_code(grpc.StatusCode.NOT_FOUND)
            context.set_details(f"Actor {request.actor_id} not found")
        return aether_pb2.HeartbeatResponse(success=success)


class MessageServiceServicer(aether_pb2_grpc.MessageServiceServicer):
    """gRPC implementation of the Message service."""

    def __init__(self, message_router: MessageRouter):
        self._router = message_router

    def Send(self, request, context):
        import asyncio
        from .models import MessageEnvelope
        envelope = MessageEnvelope(
            source_actor=request.source_actor,
            target_actor=request.target_actor,
            message_type=request.message_type or "default",
            payload=request.payload,
            correlation_id=request.correlation_id or None,
            priority=request.priority or 0,
        )
        # route() is async — run it in a new event loop from the gRPC thread
        receipt = asyncio.run(self._router.route(envelope))
        return aether_pb2.DeliveryReceipt(
            message_id=receipt.message_id,
            status=receipt.status,
            delivered_at=_dt_to_timestamp(receipt.delivered_at),
            correlation_id=receipt.correlation_id or "",
        )

    def GetPending(self, request, context):
        messages = self._router.get_pending_messages(request.actor_id)
        pending = []
        for msg in messages:
            pending.append(aether_pb2.PendingMessage(
                message_id=msg.message_id,
                source_actor=msg.source_actor,
                target_actor=msg.target_actor,
                message_type=msg.message_type,
                payload=json.dumps(msg.payload).encode() if not isinstance(msg.payload, bytes) else msg.payload,
                correlation_id=msg.correlation_id or "",
                priority=msg.priority,
                timestamp=_dt_to_timestamp(msg.timestamp),
            ))
        return aether_pb2.GetPendingMessagesResponse(messages=pending)


class StateServiceServicer(aether_pb2_grpc.StateServiceServicer):
    """gRPC implementation of the State service."""

    def __init__(self, state_store: StateStore):
        self._store = state_store

    def GetState(self, request, context):
        value = self._store.get(request.actor_id, request.key)
        if value is None:
            return aether_pb2.GetStateResponse(
                key=request.key,
                found=False,
            )
        # Get the full entry for version info
        all_state = self._store.get_all(request.actor_id)
        # Try to get version from set
        return aether_pb2.GetStateResponse(
            key=request.key,
            value=json.dumps(value).encode() if not isinstance(value, bytes) else value,
            found=True,
        )

    def SetState(self, request, context):
        try:
            payload_bytes = request.value
            value = json.loads(payload_bytes) if payload_bytes else None
            expected = request.expected_version if request.expected_version > 0 else None
            entry = self._store.set(
                actor_id=request.actor_id,
                key=request.key,
                value=value,
                expected_version=expected,
            )
            return _state_entry_to_proto(entry)
        except ValueError as e:
            context.set_code(grpc.StatusCode.ABORTED)
            context.set_details(str(e))
            return aether_pb2.StateEntry()

    def DeleteState(self, request, context):
        deleted = self._store.delete(request.actor_id, request.key)
        return aether_pb2.DeleteStateResponse(deleted=deleted)

    def GetAllState(self, request, context):
        state = self._store.get_all(request.actor_id)
        proto_state = {}
        for key, value in state.items():
            proto_state[key] = json.dumps(value).encode() if not isinstance(value, bytes) else value
        return aether_pb2.GetAllStateResponse(state=proto_state)


class EventServiceServicer(aether_pb2_grpc.EventServiceServicer):
    """gRPC implementation of the Event and PubSub service."""

    def __init__(self, pubsub: PubSubService, event_store: EventStore):
        self._pubsub = pubsub
        self._events = event_store

    def Publish(self, request, context):
        count = self._pubsub.publish(
            topic=request.topic,
            payload=json.loads(request.payload) if request.payload else None,
            headers=dict(request.headers),
        )
        return aether_pb2.PublishResponse(subscribers_notified=count)

    def Subscribe(self, request, context):
        sub_id = self._pubsub.subscribe(
            topic=request.topic,
            subscriber_id=request.subscriber_id,
            filter=request.filter or None,
        )
        return aether_pb2.SubscribeResponse(subscription_id=sub_id)

    def Unsubscribe(self, request, context):
        success = self._pubsub.unsubscribe(request.subscription_id)
        return aether_pb2.UnsubscribeResponse(success=success)

    def ListTopics(self, request, context):
        topics = self._pubsub.list_topics()
        return aether_pb2.ListTopicsResponse(topics=topics)

    def AppendEvent(self, request, context):
        try:
            data = json.loads(request.data) if request.data else None
            expected = request.expected_version if request.expected_version > 0 else None
            record = self._events.append(
                aggregate_id=request.aggregate_id,
                event_type=request.event_type,
                data=data,
                expected_version=expected,
            )
            return _event_record_to_proto(record)
        except ValueError as e:
            context.set_code(grpc.StatusCode.ABORTED)
            context.set_details(str(e))
            return aether_pb2.EventRecord()

    def GetEvents(self, request, context):
        events = self._events.get_events(request.aggregate_id)
        proto_events = [_event_record_to_proto(e) for e in events]
        return aether_pb2.GetEventsResponse(events=proto_events)


class HealthServiceServicer(aether_pb2_grpc.HealthServiceServicer):
    """gRPC implementation of the Health service."""

    def __init__(self, actor_manager: ActorManager, message_router: MessageRouter,
                 start_time: float):
        self._actors = actor_manager
        self._messages = message_router
        self._start_time = start_time

    def Health(self, request, context):
        return aether_pb2.HealthResponse(
            status="ok",
            uptime=time.time() - self._start_time,
            actor_count=self._actors.count(),
            message_count=self._messages.total_message_count(),
        )

    def Ready(self, request, context):
        return self.Health(request, context)

    def Info(self, request, context):
        return aether_pb2.InfoResponse(
            version="0.1.0",
            status="ok",
            uptime=time.time() - self._start_time,
            actor_count=self._actors.count(),
            message_count=self._messages.total_message_count(),
        )


# ============================================================
# Helpers
# ============================================================

def _actor_info_to_proto(info) -> "aether_pb2.ActorInfo":
    return aether_pb2.ActorInfo(
        actor_id=info.actor_id,
        actor_type=info.actor_type,
        capabilities=info.capabilities,
        metadata=info.metadata,
        status=info.status,
        created_at=_dt_to_timestamp(info.created_at),
        last_heartbeat=_dt_to_timestamp(info.last_heartbeat),
    )


def _state_entry_to_proto(entry) -> "aether_pb2.StateEntry":
    value = entry.value
    if not isinstance(value, bytes):
        value = json.dumps(value).encode()
    return aether_pb2.StateEntry(
        actor_id=entry.actor_id,
        key=entry.key,
        value=value,
        version=entry.version,
        updated_at=_dt_to_timestamp(entry.updated_at),
    )


def _event_record_to_proto(record) -> "aether_pb2.EventRecord":
    data = record.data
    if not isinstance(data, bytes):
        data = json.dumps(data).encode() if data is not None else b""
    return aether_pb2.EventRecord(
        event_id=record.event_id,
        aggregate_id=record.aggregate_id,
        event_type=record.event_type,
        data=data,
        version=record.version,
        timestamp=_dt_to_timestamp(record.timestamp),
    )


# ============================================================
# Server Setup
# ============================================================

def create_grpc_server(
    actor_manager: ActorManager,
    message_router: MessageRouter,
    state_store: StateStore,
    pubsub: PubSubService,
    event_store: EventStore,
    max_workers: int = 10,
    auth_config: Optional[Any] = None,
    metrics: Any = None,
) -> grpc.Server:
    """Create a gRPC server with all Aether services registered.

    Args:
        actor_manager: The server's actor manager instance.
        message_router: The server's message router instance.
        state_store: The server's state store instance.
        pubsub: The server's pub/sub service instance.
        event_store: The server's event store instance.
        max_workers: Maximum thread pool workers for the gRPC server.
        auth_config: Optional ``AuthConfig`` for JWT authentication.
            When provided and ``auth_config.enabled`` is ``True``,
            all non-health calls require a valid Bearer token.
        metrics: Optional ``MetricsCollector`` for recording gRPC call
            metrics. When provided, a ``MetricsServerInterceptor`` is
            added to the server's interceptor chain.

    Returns:
        A configured ``grpc.Server`` instance (not yet started).
    """
    if aether_pb2_grpc is None:
        raise RuntimeError("gRPC stubs not generated. Cannot create gRPC server.")

    # Build interceptors list
    interceptors = []

    if auth_config is not None:
        from .grpc_auth import AuthServerInterceptor
        interceptors.append(AuthServerInterceptor(auth_config))
        logger.info("gRPC auth interceptor enabled" if auth_config.enabled else "gRPC auth configured (disabled)")

    if metrics is not None:
        from .grpc_metrics import MetricsServerInterceptor
        interceptors.append(MetricsServerInterceptor(metrics))
        logger.info("gRPC metrics interceptor enabled")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=max_workers), interceptors=interceptors)

    start_time = time.time()

    # Register all services
    aether_pb2_grpc.add_ActorServiceServicer_to_server(
        ActorServiceServicer(actor_manager), server)
    aether_pb2_grpc.add_MessageServiceServicer_to_server(
        MessageServiceServicer(message_router), server)
    aether_pb2_grpc.add_StateServiceServicer_to_server(
        StateServiceServicer(state_store), server)
    aether_pb2_grpc.add_EventServiceServicer_to_server(
        EventServiceServicer(pubsub, event_store), server)
    aether_pb2_grpc.add_HealthServiceServicer_to_server(
        HealthServiceServicer(actor_manager, message_router, start_time), server)

    # Enable reflection for debugging tools (best-effort)
    try:
        from grpc_reflection.v1alpha import reflection
        SERVICE_NAMES = (
            aether_pb2.DESCRIPTOR.services_by_name['ActorService'].full_name,
            aether_pb2.DESCRIPTOR.services_by_name['MessageService'].full_name,
            aether_pb2.DESCRIPTOR.services_by_name['StateService'].full_name,
            aether_pb2.DESCRIPTOR.services_by_name['EventService'].full_name,
            aether_pb2.DESCRIPTOR.services_by_name['HealthService'].full_name,
            reflection.SERVICE_NAME,
        )
        reflection.enable_server_reflection(SERVICE_NAMES, server)
        logger.info("gRPC reflection enabled")
    except ImportError:
        logger.info("gRPC reflection not available (grpc_reflection not installed). "
                     "Install with: pip install grpcio-reflection")
    except Exception as e:
        logger.warning("gRPC reflection failed to enable: %s", e)

    logger.info("gRPC server created (%d workers)", max_workers)
    return server


def serve_grpc(server: grpc.Server, host: str = "0.0.0.0", port: int = 50051):
    """Start the gRPC server (blocking).

    Args:
        server: A ``grpc.Server`` instance from ``create_grpc_server``.
        host: Bind address.
        port: Bind port.
    """
    bind_addr = f"{host}:{port}"
    server.add_insecure_port(bind_addr)
    logger.info("gRPC server listening on %s", bind_addr)
    server.start()
    server.wait_for_termination()
