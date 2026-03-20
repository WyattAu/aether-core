"""
Pub/Sub Messaging for Event-Driven Architecture

Provides topic-based publish/subscribe messaging with actor integration.
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Optional,
    Set,
    TypeVar,
    Union,
)
from abc import ABC, abstractmethod
import asyncio
import uuid

from ..actor import Actor
from ..messaging import Message, MessageType
from ..exceptions import AetherError


@dataclass
class Topic:
    """
    Represents a pub/sub topic for message routing.
    
    Topics are hierarchical and use '/' as separator.
    Examples: 'user.events', 'orders.created', 'system.metrics'
    """
    name: str
    partitions: int = 1
    retention_ms: Optional[int] = None  # Message retention time
    compacted: bool = False  # Whether topic is compacted
    
    def __post_init__(self):
        """Validate topic name format."""
        if not self.name:
            raise ValueError("Topic name cannot be empty")
        if not all(c.isalnum() or c in '.-_' for c in self.name):
            raise ValueError(f"Invalid topic name: {self.name}")
    
    @property
    def parts(self) -> List[str]:
        """Split topic into hierarchical parts."""
        return self.name.split('.')
    
    def matches(self, pattern: str) -> bool:
        """
        Check if this topic matches a subscription pattern.
        
        Supports wildcards:
        - '*' matches single level
        - '>' matches multiple levels
        
        Args:
            pattern: Subscription pattern (e.g., 'user.*', 'orders.>')
        """
        if pattern == self.name:
            return True
        
        pattern_parts = pattern.split('.')
        topic_parts = self.name.split('.')
        
        for i, p in enumerate(pattern_parts):
            if p == '>':
                return True
            if p == '*':
                if i < len(topic_parts) - 1:
                    continue
                return True
            if i >= len(topic_parts):
                return False
            if p != topic_parts[i]:
                return False
        
        return len(pattern_parts) == len(topic_parts)


@dataclass
class Subscription:
    """
    Represents a subscription to a topic or topic pattern.
    """
    topic_pattern: str  # Can include wildcards
    subscriber_id: str
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    created_at: datetime = field(default_factory=datetime.utcnow)
    active: bool = True
    delivery_semantics: str = "at_least_once"  # at_most_once, at_least_once, exactly_once
    start_from: str = "latest"  # earliest, latest, specific offset
    
    def __post_init__(self):
        if self.delivery_semantics not in ("at_most_once", "at_least_once", "exactly_once"):
            raise ValueError(f"Invalid delivery semantics: {self.delivery_semantics}")


@dataclass
class PubSubMessage:
    """
    A message in the pub/sub system.
    """
    topic: str
    value: Any
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    key: Optional[str] = None  # Partition key
    timestamp: datetime = field(default_factory=datetime.utcnow)
    headers: Dict[str, str] = field(default_factory=dict)
    partition: int = 0
    offset: int = 0
    
    def to_actor_message(self) -> Message:
        """Convert to actor message for processing."""
        return Message(
            type=MessageType.STREAM_EVENT,
            payload=self.value,
            sender=None,
            correlation_id=None,
        )


class Publisher(ABC):
    """Abstract base class for publishers."""
    
    @abstractmethod
    async def publish(
        self,
        topic: str,
        value: Any,
        key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None
    ) -> str:
        """Publish a message to a topic. Returns message ID."""
        pass
    
    @abstractmethod
    async def publish_batch(
        self,
        topic: str,
        messages: List[PubSubMessage]
    ) -> List[str]:
        """Publish multiple messages atomically."""
        pass


class Subscriber(ABC):
    """Abstract base class for subscribers."""
    
    @abstractmethod
    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None
    ) -> Subscription:
        """Subscribe to a topic pattern."""
        pass
    
    @abstractmethod
    async def unsubscribe(self, subscription_id: str) -> None:
        """Unsubscribe from a topic."""
        pass
    
    @abstractmethod
    async def acknowledge(self, message_id: str) -> None:
        """Acknowledge message processing (for at-least-once)."""
        pass


class InMemoryPubSub(Publisher, Subscriber):
    """
    In-memory implementation of pub/sub for testing and development.
    """
    
    def __init__(self):
        self._topics: Dict[str, List[PubSubMessage]] = {}
        self._subscriptions: Dict[str, Subscription] = {}
        self._handlers: Dict[str, Callable] = {}
        self._offsets: Dict[str, int] = {}
        self._pending_acks: Set[str] = set()
        self._lock = asyncio.Lock()
    
    async def publish(
        self,
        topic: str,
        value: Any,
        key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None
    ) -> str:
        """Publish a message to a topic."""
        message = PubSubMessage(
            topic=topic,
            key=key,
            value=value,
            headers=headers or {}
        )
        
        async with self._lock:
            if topic not in self._topics:
                self._topics[topic] = []
                self._offsets[topic] = 0
            
            message.offset = self._offsets[topic]
            self._offsets[topic] += 1
            self._topics[topic].append(message)
            
            # Deliver to matching subscriptions
            for sub_id, sub in self._subscriptions.items():
                if sub.active:
                    topic_obj = Topic(name=topic)
                    if topic_obj.matches(sub.topic_pattern):
                        handler = self._handlers.get(sub_id)
                        if handler:
                            try:
                                await handler(message)
                            except Exception as e:
                                # Log error but continue
                                print(f"Error in subscriber handler: {e}")
            
            return message.id
    
    async def publish_batch(
        self,
        topic: str,
        messages: List[PubSubMessage]
    ) -> List[str]:
        """Publish multiple messages atomically."""
        message_ids = []
        for msg in messages:
            msg.topic = topic
            msg_id = await self.publish(topic, msg.value, msg.key, msg.headers)
            message_ids.append(msg_id)
        return message_ids
    
    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None
    ) -> Subscription:
        """Subscribe to a topic pattern."""
        sub_id = subscription_id or str(uuid.uuid4())
        
        subscription = Subscription(
            id=sub_id,
            topic_pattern=topic_pattern,
            subscriber_id="in-memory"
        )
        
        async with self._lock:
            self._subscriptions[sub_id] = subscription
            self._handlers[sub_id] = handler
        
        return subscription
    
    async def unsubscribe(self, subscription_id: str) -> None:
        """Unsubscribe from a topic."""
        async with self._lock:
            if subscription_id in self._subscriptions:
                self._subscriptions[subscription_id].active = False
                del self._handlers[subscription_id]
    
    async def acknowledge(self, message_id: str) -> None:
        """Acknowledge message processing."""
        async with self._lock:
            self._pending_acks.discard(message_id)


class PubSubClient:
    """
    High-level client for pub/sub operations with actor integration.
    """
    
    def __init__(self, backend: Optional[InMemoryPubSub] = None):
        self._backend = backend or InMemoryPubSub()
        self._actor_handlers: Dict[str, Actor] = {}
    
    async def create_topic(
        self,
        name: str,
        partitions: int = 1,
        retention_ms: Optional[int] = None
    ) -> Topic:
        """Create a new topic."""
        return Topic(
            name=name,
            partitions=partitions,
            retention_ms=retention_ms
        )
    
    async def publish(
        self,
        topic: str,
        value: Any,
        key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None
    ) -> str:
        """Publish a message to a topic."""
        return await self._backend.publish(topic, value, key, headers)
    
    async def publish_batch(
        self,
        topic: str,
        messages: List[PubSubMessage]
    ) -> List[str]:
        """Publish multiple messages."""
        return await self._backend.publish_batch(topic, messages)
    
    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None,
        delivery_semantics: str = "at_least_once"
    ) -> Subscription:
        """Subscribe to a topic pattern with a handler function."""
        return await self._backend.subscribe(
            topic_pattern,
            handler,
            subscription_id
        )
    
    async def subscribe_actor(
        self,
        topic_pattern: str,
        actor: ActorRef,
        method_name: str = "handle_event"
    ) -> Subscription:
        """
        Subscribe an actor to a topic pattern.
        
        The actor will receive messages via the specified method.
        """
        async def actor_handler(msg: PubSubMessage) -> None:
            try:
                # Convert to actor message and invoke
                actor_msg = msg.to_actor_message()
                await actor.invoke(method_name, actor_msg)
            except Exception as e:
                # Could implement retry/dead letter here
                raise AetherError.internal(f"Actor handler failed: {e}")
        
        sub_id = f"actor-{actor.id}-{topic_pattern}"
        subscription = await self.subscribe(topic_pattern, actor_handler, sub_id)
        self._actor_handlers[sub_id] = actor
        return subscription
    
    async def unsubscribe(self, subscription_id: str) -> None:
        """Unsubscribe from a topic."""
        await self._backend.unsubscribe(subscription_id)
        self._actor_handlers.pop(subscription_id, None)
    
    async def acknowledge(self, message_id: str) -> None:
        """Acknowledge message processing."""
        await self._backend.acknowledge(message_id)


# Decorator for subscribing actors to topics
def subscribe(topic_pattern: str, delivery_semantics: str = "at_least_once"):
    """
    Decorator to mark an actor method as a topic subscriber.
    
    Usage:
        class MyActor(Actor):
            @subscribe("user.events")
            async def handle_user_event(self, event: PubSubMessage):
                # Process event
                pass
    """
    def decorator(method):
        method._aether_subscribe = {
            "topic_pattern": topic_pattern,
            "delivery_semantics": delivery_semantics
        }
        return method
    return decorator


# Convenience function for one-off publishing
async def publish(
    topic: str,
    value: Any,
    key: Optional[str] = None,
    headers: Optional[Dict[str, str]] = None,
    client: Optional[PubSubClient] = None
) -> str:
    """
    Convenience function for publishing a single message.
    
    Usage:
        await publish("user.events", {"userId": 123, "action": "login"})
    """
    if client is None:
        client = PubSubClient()
    return await client.publish(topic, value, key, headers)


# Event type alias for clarity
Event = PubSubMessage
