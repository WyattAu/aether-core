"""Aether gRPC Client.

Provides a high-level async client for communicating with an Aether
reference server over gRPC. Shares the same data model types as
``AetherClient`` (HTTP) so callers can swap transports.

Usage::

    from aether_sdk.grpc_client import AetherGrpcClient

    client = AetherGrpcClient("localhost:50051")
    await client.connect()
    await client.register_actor("my-actor", "worker")
    await client.set_state("my-actor", "counter", 0)
    value = await client.get_state("my-actor", "counter")
    await client.close()

Or as a context manager::

    async with AetherGrpcClient("localhost:50051") as client:
        await client.register_actor("my-actor", "worker")

Requires the ``grpcio`` package::

    pip install grpcio
"""

import json
import logging
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from .client import (
    ActorInfo,
    DeliveryReceipt,
    EventRecord,
    MessageEnvelope,
    ServerInfo,
    StateEntry,
)

logger = logging.getLogger("aether-sdk.grpc")

try:
    import grpc
    import grpc.aio
    from .proto import aether_pb2, aether_pb2_grpc
    _GRPC_AVAILABLE = True
except ImportError:
    _GRPC_AVAILABLE = False
    aether_pb2 = None
    aether_pb2_grpc = None


class AetherGrpcError(Exception):
    """Raised when a gRPC call fails."""
    def __init__(self, code: str, detail: str):
        self.code = code
        self.detail = detail
        super().__init__(f"gRPC {code}: {detail}")


def _ts_to_datetime(ts) -> Optional[datetime]:
    """Convert a protobuf Timestamp to a Python datetime."""
    if ts is None or (ts.seconds == 0 and ts.nanos == 0):
        return None
    return datetime.fromtimestamp(ts.seconds + ts.nanos / 1e9, tz=timezone.utc)


def _json_to_bytes(value: Any) -> bytes:
    """Serialize a value to JSON bytes for gRPC payloads."""
    if value is None:
        return b""
    return json.dumps(value, default=str).encode()


def _bytes_to_json(data: bytes) -> Any:
    """Deserialize JSON bytes from gRPC payloads."""
    if not data:
        return None
    return json.loads(data)


def _handle_rpc_error(exc: Exception):
    """Convert a gRPC RpcError to AetherGrpcError."""
    import grpc as _grpc
    if isinstance(exc, _grpc.RpcError):
        status = exc.code()
        details = exc.details() or status.name
        raise AetherGrpcError(status.name, details)
    raise


class AetherGrpcClient:
    """Async gRPC client for the Aether reference server.

    Provides the same high-level API as ``AetherClient`` but communicates
    over gRPC instead of HTTP. All return types are identical.

    Args:
        target: gRPC server address (e.g. ``"localhost:50051"``).
        timeout: Default RPC timeout in seconds (default 30).
        actor_id: Default actor_id for messages (optional).
        token: Optional auth token for all calls.
        secure: Use TLS (default ``False``).
        metadata: Additional gRPC metadata (key-value pairs).
    """

    def __init__(
        self,
        target: str = "localhost:50051",
        timeout: float = 30.0,
        actor_id: Optional[str] = None,
        token: Optional[str] = None,
        secure: bool = False,
        metadata: Optional[List[tuple]] = None,
    ):
        if not _GRPC_AVAILABLE:
            raise ImportError(
                "gRPC client requires 'grpcio'. Install with: pip install grpcio"
            )
        self._target = target
        self._timeout = timeout
        self._actor_id = actor_id
        self._token = token
        self._secure = secure
        self._extra_metadata = metadata or []
        self._channel: Optional[grpc.aio.Channel] = None
        self._actors: Optional[aether_pb2_grpc.ActorServiceStub] = None
        self._messages: Optional[aether_pb2_grpc.MessageServiceStub] = None
        self._state: Optional[aether_pb2_grpc.StateServiceStub] = None
        self._events: Optional[aether_pb2_grpc.EventServiceStub] = None
        self._health: Optional[aether_pb2_grpc.HealthServiceStub] = None

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, *args):
        await self.close()

    async def connect(self):
        """Establish the gRPC channel and create stubs."""
        if self._secure:
            credentials = grpc.ssl_channel_credentials()
            self._channel = grpc.aio.secure_channel(self._target, credentials)
        else:
            self._channel = grpc.aio.insecure_channel(self._target)

        self._actors = aether_pb2_grpc.ActorServiceStub(self._channel)
        self._messages = aether_pb2_grpc.MessageServiceStub(self._channel)
        self._state = aether_pb2_grpc.StateServiceStub(self._channel)
        self._events = aether_pb2_grpc.EventServiceStub(self._channel)
        self._health = aether_pb2_grpc.HealthServiceStub(self._channel)
        logger.info("gRPC client connected to %s", self._target)

    async def close(self):
        """Close the gRPC channel."""
        if self._channel:
            await self._channel.close()
            self._channel = None
            logger.info("gRPC client disconnected")

    def _metadata(self) -> List[tuple]:
        """Build gRPC metadata with optional auth token."""
        md = list(self._extra_metadata)
        if self._token:
            md.append(("authorization", f"Bearer {self._token}"))
        return md if md else None

    def _ensure_connected(self):
        if self._channel is None:
            raise RuntimeError("Client not connected. Use 'async with' or call connect()")

    # === Health ===

    async def health(self) -> ServerInfo:
        """Check server health."""
        self._ensure_connected()
        try:
            resp = await self._health.Health(
                aether_pb2.Empty(),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return ServerInfo(
                status=resp.status,
                uptime=resp.uptime,
                actor_count=resp.actor_count,
                message_count=resp.message_count,
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def info(self) -> Dict[str, Any]:
        """Get server info including version."""
        self._ensure_connected()
        try:
            resp = await self._health.Info(
                aether_pb2.Empty(),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return {
                "version": resp.version,
                "status": resp.status,
                "uptime": resp.uptime,
                "actor_count": resp.actor_count,
                "message_count": resp.message_count,
            }
        except Exception as e:
            _handle_rpc_error(e)

    # === Actors ===

    async def register_actor(
        self,
        actor_id: str,
        actor_type: str = "default",
        capabilities: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> ActorInfo:
        """Register an actor with the server."""
        self._ensure_connected()
        try:
            resp = await self._actors.Register(
                aether_pb2.RegisterActorRequest(
                    actor_id=actor_id,
                    actor_type=actor_type,
                    capabilities=capabilities or [],
                    metadata=metadata or {},
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return ActorInfo(
                actor_id=resp.actor_id,
                actor_type=resp.actor_type,
                capabilities=list(resp.capabilities),
                metadata=dict(resp.metadata),
                status=resp.status,
                created_at=_ts_to_datetime(resp.created_at),
                last_heartbeat=_ts_to_datetime(resp.last_heartbeat),
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def unregister_actor(self, actor_id: str) -> None:
        """Unregister an actor."""
        self._ensure_connected()
        try:
            await self._actors.Unregister(
                aether_pb2.UnregisterActorRequest(actor_id=actor_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def get_actor(self, actor_id: str) -> ActorInfo:
        """Get actor info."""
        self._ensure_connected()
        try:
            resp = await self._actors.GetActor(
                aether_pb2.GetActorRequest(actor_id=actor_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return ActorInfo(
                actor_id=resp.actor_id,
                actor_type=resp.actor_type,
                capabilities=list(resp.capabilities),
                metadata=dict(resp.metadata),
                status=resp.status,
                created_at=_ts_to_datetime(resp.created_at),
                last_heartbeat=_ts_to_datetime(resp.last_heartbeat),
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def list_actors(
        self,
        actor_type: Optional[str] = None,
        status: Optional[str] = None,
    ) -> List[ActorInfo]:
        """List actors with optional filters."""
        self._ensure_connected()
        try:
            resp = await self._actors.ListActors(
                aether_pb2.ListActorsRequest(
                    actor_type=actor_type or "",
                    status=status or "",
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return [
                ActorInfo(
                    actor_id=a.actor_id,
                    actor_type=a.actor_type,
                    capabilities=list(a.capabilities),
                    metadata=dict(a.metadata),
                    status=a.status,
                    created_at=_ts_to_datetime(a.created_at),
                    last_heartbeat=_ts_to_datetime(a.last_heartbeat),
                )
                for a in resp.actors
            ]
        except Exception as e:
            _handle_rpc_error(e)

    async def heartbeat(self, actor_id: str) -> None:
        """Send heartbeat for an actor."""
        self._ensure_connected()
        try:
            await self._actors.Heartbeat(
                aether_pb2.HeartbeatRequest(actor_id=actor_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
        except Exception as e:
            _handle_rpc_error(e)

    # === Messaging ===

    async def send_message(
        self,
        target: str,
        payload: Any,
        source: Optional[str] = None,
        message_type: str = "default",
        correlation_id: Optional[str] = None,
        priority: int = 0,
    ) -> DeliveryReceipt:
        """Send a message to an actor."""
        self._ensure_connected()
        try:
            resp = await self._messages.Send(
                aether_pb2.SendMessageRequest(
                    source_actor=source or self._actor_id or "unknown",
                    target_actor=target,
                    message_type=message_type,
                    payload=_json_to_bytes(payload),
                    correlation_id=correlation_id or "",
                    priority=priority,
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return DeliveryReceipt(
                message_id=resp.message_id,
                status=resp.status,
                delivered_at=_ts_to_datetime(resp.delivered_at),
                correlation_id=resp.correlation_id or None,
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def get_pending_messages(self, actor_id: str) -> List[MessageEnvelope]:
        """Get pending messages for an actor."""
        self._ensure_connected()
        try:
            resp = await self._messages.GetPending(
                aether_pb2.GetPendingMessagesRequest(actor_id=actor_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return [
                MessageEnvelope(
                    source_actor=m.source_actor,
                    target_actor=m.target_actor,
                    message_type=m.message_type,
                    payload=_bytes_to_json(m.payload),
                    correlation_id=m.correlation_id or None,
                    timestamp=_ts_to_datetime(m.timestamp),
                    priority=m.priority,
                    message_id=m.message_id,
                )
                for m in resp.messages
            ]
        except Exception as e:
            _handle_rpc_error(e)

    # === State ===

    async def get_state(self, actor_id: str, key: str) -> Any:
        """Get a state value. Returns None if not found."""
        self._ensure_connected()
        try:
            resp = await self._state.GetState(
                aether_pb2.GetStateRequest(actor_id=actor_id, key=key),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            if not resp.found:
                return None
            return _bytes_to_json(resp.value)
        except Exception as e:
            _handle_rpc_error(e)

    async def set_state(
        self,
        actor_id: str,
        key: str,
        value: Any,
        version: Optional[int] = None,
    ) -> StateEntry:
        """Set a state value. Returns the StateEntry with new version."""
        self._ensure_connected()
        try:
            resp = await self._state.SetState(
                aether_pb2.SetStateRequest(
                    actor_id=actor_id,
                    key=key,
                    value=_json_to_bytes(value),
                    expected_version=version or 0,
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return StateEntry(
                actor_id=resp.actor_id,
                key=resp.key,
                value=_bytes_to_json(resp.value),
                version=resp.version,
                updated_at=_ts_to_datetime(resp.updated_at),
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def delete_state(self, actor_id: str, key: str) -> bool:
        """Delete a state value."""
        self._ensure_connected()
        try:
            resp = await self._state.DeleteState(
                aether_pb2.DeleteStateRequest(actor_id=actor_id, key=key),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return resp.deleted
        except Exception as e:
            _handle_rpc_error(e)

    async def get_all_state(self, actor_id: str) -> Dict[str, Any]:
        """Get all state for an actor."""
        self._ensure_connected()
        try:
            resp = await self._state.GetAllState(
                aether_pb2.GetAllStateRequest(actor_id=actor_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return {k: _bytes_to_json(v) for k, v in resp.state.items()}
        except Exception as e:
            _handle_rpc_error(e)

    # === Pub/Sub ===

    async def publish(
        self,
        topic: str,
        payload: Any,
        headers: Optional[Dict[str, str]] = None,
    ) -> int:
        """Publish a message to a topic. Returns subscriber count."""
        self._ensure_connected()
        try:
            resp = await self._events.Publish(
                aether_pb2.PublishRequest(
                    topic=topic,
                    payload=_json_to_bytes(payload),
                    headers=headers or {},
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return resp.subscribers_notified
        except Exception as e:
            _handle_rpc_error(e)

    async def subscribe(
        self,
        topic: str,
        subscriber_id: str,
        filter: Optional[str] = None,
    ) -> str:
        """Subscribe to a topic. Returns subscription ID."""
        self._ensure_connected()
        try:
            resp = await self._events.Subscribe(
                aether_pb2.SubscribeRequest(
                    topic=topic,
                    subscriber_id=subscriber_id,
                    filter=filter or "",
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return resp.subscription_id
        except Exception as e:
            _handle_rpc_error(e)

    async def unsubscribe(self, subscription_id: str) -> bool:
        """Unsubscribe from a topic."""
        self._ensure_connected()
        try:
            resp = await self._events.Unsubscribe(
                aether_pb2.UnsubscribeRequest(subscription_id=subscription_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return resp.success
        except Exception as e:
            _handle_rpc_error(e)

    async def list_topics(self) -> List[str]:
        """List all active topics."""
        self._ensure_connected()
        try:
            resp = await self._events.ListTopics(
                aether_pb2.ListTopicsRequest(),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return list(resp.topics)
        except Exception as e:
            _handle_rpc_error(e)

    # === Event Sourcing ===

    async def append_event(
        self,
        aggregate_id: str,
        event_type: str,
        data: Any = None,
        expected_version: Optional[int] = None,
    ) -> EventRecord:
        """Append an event to an aggregate."""
        self._ensure_connected()
        try:
            resp = await self._events.AppendEvent(
                aether_pb2.AppendEventRequest(
                    aggregate_id=aggregate_id,
                    event_type=event_type,
                    data=_json_to_bytes(data),
                    expected_version=expected_version or 0,
                ),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return EventRecord(
                event_id=resp.event_id,
                aggregate_id=resp.aggregate_id,
                event_type=resp.event_type,
                data=_bytes_to_json(resp.data),
                version=resp.version,
                timestamp=_ts_to_datetime(resp.timestamp),
            )
        except Exception as e:
            _handle_rpc_error(e)

    async def get_events(self, aggregate_id: str) -> List[EventRecord]:
        """Get all events for an aggregate."""
        self._ensure_connected()
        try:
            resp = await self._events.GetEvents(
                aether_pb2.GetEventsRequest(aggregate_id=aggregate_id),
                metadata=self._metadata(),
                timeout=self._timeout,
            )
            return [
                EventRecord(
                    event_id=e.event_id,
                    aggregate_id=e.aggregate_id,
                    event_type=e.event_type,
                    data=_bytes_to_json(e.data),
                    version=e.version,
                    timestamp=_ts_to_datetime(e.timestamp),
                )
                for e in resp.events
            ]
        except Exception as e:
            _handle_rpc_error(e)
