"""Aether Server Client.

Provides a high-level async client for communicating with an Aether
reference server over HTTP.
"""

import asyncio
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Dict, List, Optional

import httpx


class AetherServerError(Exception):
    """Raised when the server returns an error."""

    def __init__(self, status_code: int, detail: str):
        self.status_code = status_code
        self.detail = detail
        super().__init__(f"HTTP {status_code}: {detail}")


@dataclass
class ActorInfo:
    actor_id: str
    actor_type: str = "default"
    capabilities: List[str] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)
    status: str = "active"
    created_at: Optional[datetime] = None
    last_heartbeat: Optional[datetime] = None


@dataclass
class MessageEnvelope:
    source_actor: str
    target_actor: str
    message_type: str = "default"
    payload: Any = None
    correlation_id: Optional[str] = None
    timestamp: Optional[datetime] = None
    priority: int = 0
    message_id: Optional[str] = None


@dataclass
class DeliveryReceipt:
    message_id: str
    status: str = "delivered"
    delivered_at: Optional[datetime] = None
    correlation_id: Optional[str] = None


@dataclass
class StateEntry:
    actor_id: str
    key: str
    value: Any = None
    version: int = 1
    updated_at: Optional[datetime] = None


@dataclass
class EventRecord:
    event_id: str
    aggregate_id: str
    event_type: str
    data: Any = None
    version: int = 1
    timestamp: Optional[datetime] = None


@dataclass
class ServerInfo:
    status: str = "ok"
    uptime: float = 0.0
    actor_count: int = 0
    message_count: int = 0


class AetherClient:
    """Async client for the Aether reference server.

    Usage:
        async with AetherClient("http://localhost:8080") as client:
            await client.register_actor("my-actor", "worker")
            await client.set_state("my-actor", "counter", 0)
            value = await client.get_state("my-actor", "counter")

    Args:
        base_url: Server base URL (e.g. "http://localhost:8080")
        timeout: Request timeout in seconds (default 30)
        actor_id: Default actor_id for messages (optional)
        http_client: Pre-configured httpx.AsyncClient (optional, for testing)
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8080",
        timeout: float = 30.0,
        actor_id: Optional[str] = None,
        http_client: Optional[httpx.AsyncClient] = None,
    ):
        self._base_url = base_url.rstrip("/")
        self._timeout = timeout
        self._actor_id = actor_id
        self._client: Optional[httpx.AsyncClient] = http_client
        self._owns_client = http_client is None

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, *args):
        await self.close()

    async def connect(self):
        """Initialize the HTTP client."""
        if self._client is None:
            self._client = httpx.AsyncClient(
                base_url=self._base_url,
                timeout=self._timeout,
            )

    async def close(self):
        """Close the HTTP client."""
        if self._client and self._owns_client:
            await self._client.aclose()
            self._client = None

    def _ensure_connected(self):
        if self._client is None:
            raise RuntimeError(
                "Client not connected. Use 'async with' or call connect()"
            )

    def _handle_error(self, response: httpx.Response):
        if response.status_code >= 400:
            try:
                detail = response.json()["detail"]
            except (KeyError, ValueError):
                detail = response.text
            raise AetherServerError(response.status_code, detail)

    def _parse_datetime(self, value: Any) -> Optional[datetime]:
        if value is None:
            return None
        if isinstance(value, datetime):
            return value
        return datetime.fromisoformat(str(value))

    # === Health ===
    async def health(self) -> ServerInfo:
        """Check server health."""
        self._ensure_connected()
        resp = await self._client.get("/health")
        self._handle_error(resp)
        data = resp.json()
        return ServerInfo(**data)

    async def info(self) -> Dict[str, Any]:
        """Get server info including version."""
        self._ensure_connected()
        resp = await self._client.get("/api/v1/info")
        self._handle_error(resp)
        return resp.json()

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
        body: Dict[str, Any] = {
            "actor_id": actor_id,
            "actor_type": actor_type,
            "capabilities": capabilities or [],
            "metadata": metadata or {},
        }
        resp = await self._client.post("/api/v1/actors", json=body)
        self._handle_error(resp)
        return self._parse_actor(resp.json())

    async def unregister_actor(self, actor_id: str) -> None:
        """Unregister an actor."""
        self._ensure_connected()
        resp = await self._client.delete(f"/api/v1/actors/{actor_id}")
        self._handle_error(resp)

    async def get_actor(self, actor_id: str) -> ActorInfo:
        """Get actor info."""
        self._ensure_connected()
        resp = await self._client.get(f"/api/v1/actors/{actor_id}")
        self._handle_error(resp)
        return self._parse_actor(resp.json())

    async def list_actors(
        self, actor_type: Optional[str] = None, status: Optional[str] = None
    ) -> List[ActorInfo]:
        """List actors with optional filters."""
        self._ensure_connected()
        params = {}
        if actor_type is not None:
            params["type"] = actor_type
        if status is not None:
            params["status"] = status
        resp = await self._client.get("/api/v1/actors", params=params)
        self._handle_error(resp)
        return [self._parse_actor(a) for a in resp.json()]

    async def heartbeat(self, actor_id: str) -> None:
        """Send heartbeat for an actor."""
        self._ensure_connected()
        resp = await self._client.post(f"/api/v1/actors/{actor_id}/heartbeat")
        self._handle_error(resp)

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
        body: Dict[str, Any] = {
            "source_actor": source or self._actor_id or "unknown",
            "target_actor": target,
            "message_type": message_type,
            "payload": payload,
            "priority": priority,
        }
        if correlation_id is not None:
            body["correlation_id"] = correlation_id
        resp = await self._client.post(f"/api/v1/actors/{target}/messages", json=body)
        self._handle_error(resp)
        return self._parse_receipt(resp.json())

    async def get_pending_messages(self, actor_id: str) -> List[MessageEnvelope]:
        """Get pending messages for an actor."""
        self._ensure_connected()
        resp = await self._client.get(f"/api/v1/actors/{actor_id}/messages")
        self._handle_error(resp)
        return [self._parse_message(m) for m in resp.json()]

    async def send_and_receive(
        self,
        target: str,
        payload: Any,
        timeout_ms: float = 5000,
        poll_interval_ms: float = 100,
        **kwargs,
    ) -> List[MessageEnvelope]:
        """Send a message and wait for response messages."""
        me = self._actor_id or "unknown"
        cid = f"corr-{id(payload)}"
        await self.send_message(target, payload, correlation_id=cid, **kwargs)

        messages: List[MessageEnvelope] = []
        deadline = asyncio.get_event_loop().time() + timeout_ms / 1000
        while asyncio.get_event_loop().time() < deadline:
            pending = await self.get_pending_messages(me)
            new_msgs = [m for m in pending if m.correlation_id == cid]
            messages.extend(new_msgs)
            if new_msgs:
                break
            await asyncio.sleep(poll_interval_ms / 1000)
        return messages

    # === State ===
    async def get_state(self, actor_id: str, key: str) -> Any:
        """Get a state value. Returns None if not found."""
        self._ensure_connected()
        resp = await self._client.get(f"/api/v1/state/{actor_id}/{key}")
        if resp.status_code == 404:
            return None
        self._handle_error(resp)
        data = resp.json()
        return data.get("value")

    async def set_state(
        self, actor_id: str, key: str, value: Any, version: Optional[int] = None
    ) -> StateEntry:
        """Set a state value. Returns the StateEntry with new version."""
        self._ensure_connected()
        body: Dict[str, Any] = {"value": value}
        if version is not None:
            body["version"] = version
        resp = await self._client.put(f"/api/v1/state/{actor_id}/{key}", json=body)
        self._handle_error(resp)
        return self._parse_state_entry(resp.json())

    async def delete_state(self, actor_id: str, key: str) -> bool:
        """Delete a state value."""
        self._ensure_connected()
        resp = await self._client.delete(f"/api/v1/state/{actor_id}/{key}")
        if resp.status_code == 404:
            return False
        self._handle_error(resp)
        return True

    async def get_all_state(self, actor_id: str) -> Dict[str, Any]:
        """Get all state for an actor."""
        self._ensure_connected()
        resp = await self._client.get(f"/api/v1/state/{actor_id}")
        self._handle_error(resp)
        data = resp.json()
        return data.get("state", {})

    # === Pub/Sub ===
    async def publish(
        self, topic: str, payload: Any, headers: Optional[Dict[str, str]] = None
    ) -> int:
        """Publish a message to a topic. Returns subscriber count."""
        self._ensure_connected()
        body: Dict[str, Any] = {
            "topic": topic,
            "payload": payload,
            "headers": headers or {},
        }
        resp = await self._client.post("/api/v1/events/publish", json=body)
        self._handle_error(resp)
        return resp.json().get("subscriber_count", 0)

    async def subscribe(
        self, topic: str, subscriber_id: str, filter: Optional[str] = None
    ) -> str:
        """Subscribe to a topic. Returns subscription ID."""
        self._ensure_connected()
        body: Dict[str, Any] = {
            "topic": topic,
            "subscriber_id": subscriber_id,
        }
        if filter is not None:
            body["filter"] = filter
        resp = await self._client.post("/api/v1/events/subscribe", json=body)
        self._handle_error(resp)
        return resp.json()["subscription_id"]

    async def unsubscribe(self, subscription_id: str) -> bool:
        """Unsubscribe from a topic."""
        self._ensure_connected()
        resp = await self._client.delete(f"/api/v1/events/subscribe/{subscription_id}")
        if resp.status_code == 404:
            return False
        self._handle_error(resp)
        return True

    async def list_topics(self) -> List[str]:
        """List all active topics (topics with subscriptions)."""
        self._ensure_connected()
        resp = await self._client.get("/api/v1/events/topics")
        self._handle_error(resp)
        return resp.json()

    async def get_topic_history(self, topic: str, limit: int = 50) -> list:
        """Get recent messages for a topic."""
        self._ensure_connected()
        resp = await self._client.get(
            f"/api/v1/events/topics/{topic}/history",
            params={"limit": limit},
        )
        self._handle_error(resp)
        return resp.json()

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
        body: Dict[str, Any] = {
            "aggregate_id": aggregate_id,
            "event_type": event_type,
            "data": data,
        }
        if expected_version is not None:
            body["expected_version"] = expected_version
        resp = await self._client.post("/api/v1/events/append", json=body)
        self._handle_error(resp)
        return self._parse_event(resp.json())

    async def get_events(self, aggregate_id: str) -> List[EventRecord]:
        """Get all events for an aggregate."""
        self._ensure_connected()
        resp = await self._client.get(f"/api/v1/events/{aggregate_id}")
        self._handle_error(resp)
        return [self._parse_event(e) for e in resp.json()]

    # === Parsers ===
    @staticmethod
    def _parse_actor(data: dict) -> ActorInfo:
        return ActorInfo(
            actor_id=data["actor_id"],
            actor_type=data.get("actor_type", "default"),
            capabilities=data.get("capabilities", []),
            metadata=data.get("metadata", {}),
            status=data.get("status", "active"),
            created_at=data.get("created_at"),
            last_heartbeat=data.get("last_heartbeat"),
        )

    @staticmethod
    def _parse_message(data: dict) -> MessageEnvelope:
        return MessageEnvelope(
            source_actor=data["source_actor"],
            target_actor=data["target_actor"],
            message_type=data.get("message_type", "default"),
            payload=data.get("payload"),
            correlation_id=data.get("correlation_id"),
            timestamp=data.get("timestamp"),
            priority=data.get("priority", 0),
            message_id=data.get("message_id"),
        )

    @staticmethod
    def _parse_receipt(data: dict) -> DeliveryReceipt:
        return DeliveryReceipt(
            message_id=data["message_id"],
            status=data.get("status", "delivered"),
            delivered_at=data.get("delivered_at"),
            correlation_id=data.get("correlation_id"),
        )

    @staticmethod
    def _parse_state_entry(data: dict) -> StateEntry:
        return StateEntry(
            actor_id=data.get("actor_id", ""),
            key=data.get("key", ""),
            value=data.get("value"),
            version=data.get("version", 1),
            updated_at=data.get("updated_at"),
        )

    @staticmethod
    def _parse_event(data: dict) -> EventRecord:
        return EventRecord(
            event_id=data["event_id"],
            aggregate_id=data["aggregate_id"],
            event_type=data["event_type"],
            data=data.get("data"),
            version=data.get("version", 1),
            timestamp=data.get("timestamp"),
        )
