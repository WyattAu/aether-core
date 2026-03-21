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
    Coroutine,
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


# ============================================
# Topic Configuration
# ============================================

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


@dataclass
class Subscription:
    """Represents a subscription to a topic."""
    id: str
    topic_pattern: str
    created_at: datetime = field(default_factory=datetime.utcnow)
    active: bool = True
    handler: Optional[Callable[[PubSubMessage], None]] = None


@dataclass
class PubSubMessage:
    """Message in the pub/sub system."""
    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    topic: str = ""
    value: Any = None
    key: Optional[str] = None
    timestamp: datetime = field(default_factory=datetime.utcnow)
    headers: Dict[str, str] = field(default_factory=dict)
    partition: int = 0
    offset: int = 0
    
    def to_actor_message(self) -> Message:
        """Convert to actor message for processing."""
        return Message(
            type=MessageType.STREAM_EVENT,
            payload=self.value,
        )


class PubSubBackend(ABC):
    """Abstract base class for pub/sub backends."""
    
    @abstractmethod
    async def create_topic(self, topic: Topic) -> Topic:
        pass
    
    @abstractmethod
    async def publish(self, topic: str, message: PubSubMessage) -> None:
        pass
    
    @abstractmethod
    async def publish_batch(self, topic: str, messages: List[PubSubMessage]) -> None:
        pass
    
    @abstractmethod
    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None
    ) -> Subscription:
        pass
    
    @abstractmethod
    async def unsubscribe(self, subscription_id: str) -> None:
        pass
    
    @abstractmethod
    async def acknowledge(self, message_id: str) -> None:
        pass


class InMemoryPubSub(PubSubBackend):
    """In-memory implementation of PubSubBackend for testing."""
    
    def __init__(self):
        self._topics: Dict[str, Topic] = {}
        self._subscriptions: Dict[str, Subscription] = {}
        self._pending_acks: Set[str] = set()
        self._lock = asyncio.Lock()
    
    async def create_topic(self, topic: Topic) -> Topic:
        self._topics[topic.name] = topic
        return topic
    
    async def publish(self, topic: str, message: PubSubMessage) -> None:
        if topic not in self._topics:
            raise ValueError(f"Topic not found: {topic}")
        
        self._pending_acks.add(message.id)
        await self._route_message(topic, message)
        self._pending_acks.discard(message.id)
    
    async def publish_batch(self, topic: str, messages: List[PubSubMessage]) -> None:
        for msg in messages:
            await self.publish(topic, msg)

    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None
    ) -> Subscription:
        sub_id = subscription_id or str(uuid.uuid4())
        subscription = Subscription(
            id=sub_id,
            topic_pattern=topic_pattern,
            handler=handler,
        )
        self._subscriptions[sub_id] = subscription
        return subscription

    async def unsubscribe(self, subscription_id: str) -> None:
        self._subscriptions.pop(subscription_id, None)
        
    async def acknowledge(self, message_id: str) -> None:
        self._pending_acks.discard(message_id)

    async def _route_message(self, topic: str, message: PubSubMessage) -> None:
        """Route message to all matching subscriptions."""
        for sub in self._subscriptions.values():
            if self._matches_pattern(message.topic, sub.topic_pattern):
                handler = sub.handler
                if handler:
                    try:
                        result = handler(message)
                        if asyncio.iscoroutine(result):
                            await result
                    except Exception:
                        pass
        
    def _matches_pattern(self, topic: str, pattern: str) -> bool:
        """Check if topic matches pattern (supports wildcards)."""
        if '*' in pattern:
            return True
        topic_parts = topic.split('.')
        pattern_parts = pattern.split('.')
        
        if len(topic_parts) != len(pattern_parts):
            return False
        
        for i in range(len(pattern_parts)):
            if pattern_parts[i] != '*' and pattern_parts[i] != topic_parts[i]:
                return False
        
        return True


class PubSubClient:
    """Client for pub/sub messaging."""
    
    def __init__(self, backend: Optional[PubSubBackend] = None):
        self._backend = backend or InMemoryPubSub()
        self._actor_handlers: Dict[str, Actor] = {}
    
    async def create_topic(self, topic: Topic) -> Topic:
        """Create a topic."""
        return await self._backend.create_topic(topic)
    
    async def publish(
        self,
        topic: str,
        value: Any,
        key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> str:
        """Publish a message to a topic."""
        message = PubSubMessage(
            topic=topic,
            value=value,
            key=key,
            headers=headers or {},
        )
        await self._backend.publish(topic, message)
        return message.id
    
    async def publish_batch(
        self,
        topic: str,
        messages: List[PubSubMessage],
    ) -> List[str]:
        """Publish multiple messages."""
        await self._backend.publish_batch(topic, messages)
        return [msg.id for msg in messages]
    
    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None,
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
        actor: Actor,
        method_name: str = "handle_event"
    ) -> Subscription:
        """Subscribe an actor to a topic pattern."""
        async def actor_handler(msg: PubSubMessage) -> None:
            try:
                method = getattr(actor, method_name, None)
                if method is None:
                    raise ValueError(f"Actor method '{method_name}' not found")
                
                result = method(actor, msg)
                if asyncio.iscoroutine(result):
                    await result
            
            except Exception as e:
                raise AetherError(f"Actor handler failed: {e}")
        
        sub_id = f"actor-{id(actor)}-{topic_pattern}"
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
    """Decorator to mark an actor method as a topic subscriber."""
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
    """Convenience function for publishing a single message."""
    if client is None:
        client = PubSubClient()
    return await client.publish(topic, value, key, headers)


# Event type alias for clarity
Event = PubSubMessage

__all__ = [
    'Topic',
    'Subscription',
    'PubSubMessage',
    'PubSubBackend',
    'InMemoryPubSub',
    'PubSubClient',
    'Publisher',
    'Subscriber',
    'Event',
    'subscribe',
    'publish',
]

# Type aliases for clarity
Publisher = PubSubClient
Subscriber = PubSubClient
