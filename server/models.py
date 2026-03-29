import time
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field


class ActorRegistration(BaseModel):
    actor_id: str
    actor_type: str = "default"
    capabilities: List[str] = Field(default_factory=list)
    metadata: Dict[str, Any] = Field(default_factory=dict)


class ActorInfo(BaseModel):
    actor_id: str
    actor_type: str = "default"
    capabilities: List[str] = Field(default_factory=list)
    metadata: Dict[str, Any] = Field(default_factory=dict)
    status: str = "active"
    created_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))
    last_heartbeat: Optional[datetime] = None


class MessageEnvelope(BaseModel):
    source_actor: str
    target_actor: str
    message_type: str = "default"
    payload: Any = None
    correlation_id: Optional[str] = None
    timestamp: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))
    priority: int = 0
    message_id: str = Field(default_factory=lambda: f"msg_{int(time.time() * 1e6)}")


class DeliveryReceipt(BaseModel):
    message_id: str
    status: str = "delivered"
    delivered_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))
    correlation_id: Optional[str] = None


class StateEntry(BaseModel):
    actor_id: str
    key: str
    value: Any = None
    version: int = 1
    updated_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))


class PubSubMessage(BaseModel):
    topic: str
    payload: Any = None
    headers: Dict[str, str] = Field(default_factory=dict)
    timestamp: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))
    message_id: str = Field(default_factory=lambda: f"pub_{int(time.time() * 1e6)}")


class Subscription(BaseModel):
    subscription_id: str
    topic: str
    subscriber_id: str
    filter: Optional[str] = None


class EventRecord(BaseModel):
    event_id: str
    aggregate_id: str
    event_type: str
    data: Any = None
    version: int
    timestamp: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))


class HealthResponse(BaseModel):
    status: str = "ok"
    uptime: float = 0.0
    actor_count: int = 0
    message_count: int = 0


class SetStateRequest(BaseModel):
    value: Any = None
    version: Optional[int] = None


class PublishRequest(BaseModel):
    topic: str
    payload: Any = None
    headers: Dict[str, str] = Field(default_factory=dict)


class SubscribeRequest(BaseModel):
    topic: str
    subscriber_id: str
    filter: Optional[str] = None


class AppendEventRequest(BaseModel):
    aggregate_id: str
    event_type: str
    data: Any = None
    expected_version: Optional[int] = None


class GetStateResponse(BaseModel):
    """Response for GET /api/v1/state/{actor_id}/{key}."""
    actor_id: str
    key: str
    value: Any = None


class GetAllStateResponse(BaseModel):
    """Response for GET /api/v1/state/{actor_id}."""
    actor_id: str
    state: Dict[str, Any] = Field(default_factory=dict)


class PublishResponse(BaseModel):
    """Response for POST /api/v1/events/publish."""
    topic: str
    subscriber_count: int = 0


class SubscribeResponse(BaseModel):
    """Response for POST /api/v1/events/subscribe."""
    subscription_id: str
    topic: str


class InfoResponse(BaseModel):
    """Response for GET /api/v1/info."""
    version: str = "0.1.0"
    status: str = "ok"
    uptime: float = 0.0
    actor_count: int = 0
    message_count: int = 0
