"""
Aether SDK Event Module

Provides event-driven architecture capabilities:
- Pub/Sub messaging with topic-based routing
- Event sourcing for state persistence
- Message delivery guarantees (at-least-once, exactly-once)
- Schema registry for event validation
- Dead letter queues and retry handling

Example:
    from aether_sdk.event import (
        PubSubClient,
        Topic,
        Subscription,
        EventStore,
        DeliveryGuarantee,
        SchemaRegistry,
    )

    # Create a pub/sub client
    client = PubSubClient()

    # Publish events
    await client.publish("user.events", {"userId": 123, "action": "login"})

    # Subscribe to topics
    async def handle_event(msg):
        print(f"Received: {msg.value}")

    await client.subscribe("user.*", handle_event)

    # Use event sourcing
    store = EventStore("orders")
    await store.append("order-123", {"status": "created"})
"""

from __future__ import annotations

# Delivery guarantee components
from .delivery import (DeadLetterQueue, DeliveryGuarantee, DeliveryStats,
                       InMemoryOutbox, OutboxEntry, RetryPolicy)
# Event sourcing components
from .event_sourcing import (Aggregate, EventEnvelope, EventSourcedActor,
                             EventStore, EventVersion, InMemoryEventStore,
                             Snapshot, apply_event)
# Pub/Sub components
from .pubsub import (Event, InMemoryPubSub, Publisher, PubSubClient,
                     PubSubMessage, Subscriber, Subscription, Topic, publish,
                     subscribe)
# Schema registry components
from .schema import (Compatibility, InMemorySchemaRegistry,
                     JsonSchemaValidator, Schema, SchemaError, SchemaRegistry,
                     SchemaValidator, SchemaVersion)

__all__ = [
    # Pub/Sub
    "PubSubClient",
    "Topic",
    "Subscription",
    "PubSubMessage",
    "Publisher",
    "Subscriber",
    "InMemoryPubSub",
    "Event",
    "subscribe",
    "publish",
    # Event Sourcing
    "EventStore",
    "EventSourcedActor",
    "EventEnvelope",
    "EventVersion",
    "Aggregate",
    "apply_event",
    "Snapshot",
    "InMemoryEventStore",
    # Delivery
    "DeliveryGuarantee",
    "InMemoryOutbox",
    "DeadLetterQueue",
    "DeliveryStats",
    "RetryPolicy",
    "OutboxEntry",
    # Schema
    "SchemaRegistry",
    "Schema",
    "SchemaVersion",
    "Compatibility",
    "SchemaValidator",
    "InMemorySchemaRegistry",
    "SchemaError",
    "JsonSchemaValidator",
]
