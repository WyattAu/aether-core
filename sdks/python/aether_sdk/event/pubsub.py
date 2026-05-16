"""
Pub/Sub Messaging for Event-Driven Architecture

Provides topic-based publish/subscribe messaging with actor integration.

Example:
    >>> from aether_sdk.event.pubsub import PubSubClient, Topic, publish
    >>> client = PubSubClient()
    >>> await client.create_topic(Topic(name="orders"))
    >>> msg_id = await client.publish("orders", {"orderId": "123"})
"""

from __future__ import annotations

import asyncio
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Callable, Dict, List, Optional, Set

from ..actor import Actor
from ..exceptions import AetherError
from ..messaging import Message, MessageType

# ============================================
# Topic Configuration
# ============================================


@dataclass
class Topic:
    """Represents a pub/sub topic for message routing.

    Topics are hierarchical and use ``.`` as separator.

    Args:
        name: Topic name (alphanumeric, dots, hyphens, underscores).
        partitions: Number of partitions for the topic.
        retention_ms: Message retention time in milliseconds.
        compacted: Whether the topic is compacted (retains latest
            value per key only).

    Raises:
        ValueError: If *name* is empty or contains invalid characters.

    Example:
        >>> Topic(name="user.events", partitions=4)
    """

    name: str
    partitions: int = 1
    retention_ms: Optional[int] = None
    compacted: bool = False

    def __post_init__(self):
        """Validate the topic name format."""
        if not self.name:
            raise ValueError("Topic name cannot be empty")
        if not all(c.isalnum() or c in ".-_" for c in self.name):
            raise ValueError(f"Invalid topic name: {self.name}")


@dataclass
class Subscription:
    """Represents a subscription to a topic.

    Attributes:
        id: Unique subscription identifier.
        topic_pattern: Topic pattern to match (supports ``*`` wildcards).
        created_at: When the subscription was created.
        active: Whether the subscription is currently active.
        handler: Callback invoked when a matching message arrives.
    """

    id: str
    topic_pattern: str
    created_at: datetime = field(default_factory=datetime.utcnow)
    active: bool = True
    handler: Optional[Callable[[PubSubMessage], None]] = None


@dataclass
class PubSubMessage:
    """A message in the pub/sub system.

    Attributes:
        id: Unique message identifier (auto-generated UUID).
        topic: Topic the message was published to.
        value: Message payload.
        key: Optional partitioning key.
        timestamp: When the message was created.
        headers: Optional string key-value headers.
        partition: Partition number (set by the backend).
        offset: Offset within the partition (set by the backend).
    """

    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    topic: str = ""
    value: Any = None
    key: Optional[str] = None
    timestamp: datetime = field(default_factory=datetime.utcnow)
    headers: Dict[str, str] = field(default_factory=dict)
    partition: int = 0
    offset: int = 0

    def to_actor_message(self) -> Message:
        """Convert to an actor :class:`~aether_sdk.messaging.Message`.

        Returns:
            A ``Message`` with type ``STREAM_EVENT`` and the payload.
        """
        return Message(
            type=MessageType.STREAM_EVENT,
            payload=self.value,
        )


class PubSubBackend(ABC):
    """Abstract base class for pub/sub backends.

    Subclasses implement the storage and routing of messages to
    topic subscribers.
    """

    @abstractmethod
    async def create_topic(self, topic: Topic) -> Topic:
        """Create a new topic.

        Args:
            topic: The topic to create.

        Returns:
            The created topic.
        """
        pass

    @abstractmethod
    async def publish(self, topic: str, message: PubSubMessage) -> None:
        """Publish a message to a topic.

        Args:
            topic: Topic name.
            message: The message to publish.
        """
        pass

    @abstractmethod
    async def publish_batch(self, topic: str, messages: List[PubSubMessage]) -> None:
        """Publish multiple messages to a topic.

        Args:
            topic: Topic name.
            messages: Messages to publish.
        """
        pass

    @abstractmethod
    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None,
    ) -> Subscription:
        """Subscribe to a topic pattern.

        Args:
            topic_pattern: Pattern to match (supports ``*`` wildcards).
            handler: Callback for each matching message.
            subscription_id: Optional explicit subscription ID.

        Returns:
            The created :class:`Subscription`.
        """
        pass

    @abstractmethod
    async def unsubscribe(self, subscription_id: str) -> None:
        """Unsubscribe by subscription ID.

        Args:
            subscription_id: The subscription to cancel.
        """
        pass

    @abstractmethod
    async def acknowledge(self, message_id: str) -> None:
        """Acknowledge that a message has been processed.

        Args:
            message_id: The message to acknowledge.
        """
        pass


class InMemoryPubSub(PubSubBackend):
    """In-memory implementation of :class:`PubSubBackend` for testing.

    Messages are routed synchronously to matching subscribers within
    the same process.
    """

    def __init__(self):
        """Initialize with empty topic and subscription registries."""
        self._topics: Dict[str, Topic] = {}
        self._subscriptions: Dict[str, Subscription] = {}
        self._pending_acks: Set[str] = set()
        self._lock = asyncio.Lock()

    async def create_topic(self, topic: Topic) -> Topic:
        """Create and register a topic.

        Args:
            topic: The topic to create.

        Returns:
            The registered topic.
        """
        self._topics[topic.name] = topic
        return topic

    async def publish(self, topic: str, message: PubSubMessage) -> None:
        """Publish a message and route it to matching subscribers.

        Args:
            topic: Topic name.

        Raises:
            ValueError: If the topic has not been created.
        """
        if topic not in self._topics:
            raise ValueError(f"Topic not found: {topic}")

        self._pending_acks.add(message.id)
        await self._route_message(topic, message)
        self._pending_acks.discard(message.id)

    async def publish_batch(self, topic: str, messages: List[PubSubMessage]) -> None:
        """Publish multiple messages sequentially.

        Args:
            topic: Topic name.
            messages: Messages to publish.
        """
        for msg in messages:
            await self.publish(topic, msg)

    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None,
    ) -> Subscription:
        """Subscribe to a topic pattern.

        Args:
            topic_pattern: Pattern to match.
            handler: Callback for matching messages.
            subscription_id: Optional explicit ID.

        Returns:
            The created :class:`Subscription`.
        """
        sub_id = subscription_id or str(uuid.uuid4())
        subscription = Subscription(
            id=sub_id,
            topic_pattern=topic_pattern,
            handler=handler,
        )
        self._subscriptions[sub_id] = subscription
        return subscription

    async def unsubscribe(self, subscription_id: str) -> None:
        """Remove a subscription.

        Args:
            subscription_id: The subscription to remove.
        """
        self._subscriptions.pop(subscription_id, None)

    async def acknowledge(self, message_id: str) -> None:
        """Acknowledge a message.

        Args:
            message_id: The message ID.
        """
        self._pending_acks.discard(message_id)

    async def _route_message(self, topic: str, message: PubSubMessage) -> None:
        """Route a message to all matching subscriptions.

        Args:
            topic: Topic name.
            message: The message to route.
        """
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
        """Check whether a topic matches a subscription pattern.

        Supports ``*`` as a wildcard within each dot-separated segment.

        Args:
            topic: The topic name.
            pattern: The subscription pattern.

        Returns:
            ``True`` if the topic matches the pattern.
        """
        if "*" in pattern:
            return True
        topic_parts = topic.split(".")
        pattern_parts = pattern.split(".")

        if len(topic_parts) != len(pattern_parts):
            return False

        for i in range(len(pattern_parts)):
            if pattern_parts[i] != "*" and pattern_parts[i] != topic_parts[i]:
                return False

        return True


class PubSubClient:
    """High-level client for pub/sub messaging.

    Wraps a :class:`PubSubBackend` and provides convenient methods
    for publishing, subscribing, and actor integration.

    Example:
        >>> client = PubSubClient()
        >>> await client.create_topic(Topic(name="events"))
        >>> msg_id = await client.publish("events", {"type": "user_joined"})
    """

    def __init__(self, backend: Optional[PubSubBackend] = None):
        """Initialize the client.

        Args:
            backend: Optional backend. Defaults to
                :class:`InMemoryPubSub`.
        """
        self._backend = backend or InMemoryPubSub()
        self._actor_handlers: Dict[str, Actor] = {}

    async def create_topic(self, topic: Topic) -> Topic:
        """Create a topic.

        Args:
            topic: The topic to create.

        Returns:
            The created topic.
        """
        return await self._backend.create_topic(topic)

    async def publish(
        self,
        topic: str,
        value: Any,
        key: Optional[str] = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> str:
        """Publish a value to a topic.

        Args:
            topic: Topic name.
            value: Payload to publish.
            key: Optional partitioning key.
            headers: Optional headers.

        Returns:
            The message ID of the published message.
        """
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
        """Publish multiple messages to a topic.

        Args:
            topic: Topic name.
            messages: Messages to publish.

        Returns:
            List of message IDs.
        """
        await self._backend.publish_batch(topic, messages)
        return [msg.id for msg in messages]

    async def subscribe(
        self,
        topic_pattern: str,
        handler: Callable[[PubSubMessage], None],
        subscription_id: Optional[str] = None,
    ) -> Subscription:
        """Subscribe to a topic pattern with a handler function.

        Args:
            topic_pattern: Pattern to match.
            handler: Callback for matching messages.
            subscription_id: Optional explicit ID.

        Returns:
            The created :class:`Subscription`.
        """
        return await self._backend.subscribe(topic_pattern, handler, subscription_id)

    async def subscribe_actor(
        self, topic_pattern: str, actor: Actor, method_name: str = "handle_event"
    ) -> Subscription:
        """Subscribe an actor to a topic pattern.

        Messages are delivered by calling *method_name* on the actor.

        Args:
            topic_pattern: Pattern to match.
            actor: The actor to subscribe.
            method_name: Name of the method to invoke (default
                ``"handle_event"``).

        Returns:
            The created :class:`Subscription`.

        Raises:
            AetherError: If the actor handler raises an exception.
        """

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
        """Unsubscribe from a topic.

        Args:
            subscription_id: The subscription to cancel.
        """
        await self._backend.unsubscribe(subscription_id)
        self._actor_handlers.pop(subscription_id, None)

    async def acknowledge(self, message_id: str) -> None:
        """Acknowledge a message.

        Args:
            message_id: The message ID.
        """
        await self._backend.acknowledge(message_id)


def subscribe(topic_pattern: str, delivery_semantics: str = "at_least_once"):
    """Decorator to mark an actor method as a topic subscriber.

    Sets the ``_aether_subscribe`` attribute on the method with
    the topic pattern and delivery semantics.

    Args:
        topic_pattern: Pattern to subscribe to.
        delivery_semantics: Delivery guarantee (default
            ``"at_least_once"``).

    Returns:
        A decorator function.

    Example:
        >>> @subscribe("orders.*")
        ... def handle_order(self, msg):
        ...     pass
    """

    def decorator(method):
        method._aether_subscribe = {
            "topic_pattern": topic_pattern,
            "delivery_semantics": delivery_semantics,
        }
        return method

    return decorator


async def publish(
    topic: str,
    value: Any,
    key: Optional[str] = None,
    headers: Optional[Dict[str, str]] = None,
    client: Optional[PubSubClient] = None,
) -> str:
    """Convenience function for publishing a single message.

    Args:
        topic: Topic name.
        value: Payload to publish.
        key: Optional partitioning key.
        headers: Optional headers.
        client: Optional client (created if not provided).

    Returns:
        The message ID.
    """
    if client is None:
        client = PubSubClient()
    return await client.publish(topic, value, key, headers)


Event = PubSubMessage

__all__ = [
    "Topic",
    "Subscription",
    "PubSubMessage",
    "PubSubBackend",
    "InMemoryPubSub",
    "PubSubClient",
    "Publisher",
    "Subscriber",
    "Event",
    "subscribe",
    "publish",
]

Publisher = PubSubClient
Subscriber = PubSubClient
