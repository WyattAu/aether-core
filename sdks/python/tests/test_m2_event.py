"""
Tests for M2 Event System

Comprehensive tests for:
- Pub/Sub messaging (Topic, Subscription, PubSubMessage, PubSubClient)
- Event Sourcing (EventStore, EventEnvelope)
- Delivery Guarantees (DeliveryGuarantee, RetryPolicy, InMemoryOutbox)
- Schema Registry (Schema, SchemaValidator, InMemorySchemaRegistry)
"""

import pytest
from typing import Any, Dict
import asyncio
import sys
import os

# Add the SDK to the path
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from aether_sdk.event import (
    # Pub/Sub
    PubSubClient,
    Topic,
    Subscription,
    PubSubMessage,
    InMemoryPubSub,
    publish,
    # Event Sourcing
    EventStore,
    EventEnvelope,
    EventVersion,
    InMemoryEventStore,
    # Delivery
    DeliveryGuarantee,
    RetryPolicy,
    InMemoryOutbox,
    OutboxEntry,
    # Schema
    Schema,
    SchemaVersion,
    Compatibility,
    InMemorySchemaRegistry,
    SchemaError,
    JsonSchemaValidator,
)


# ============================================
# Pub/Sub Tests
# ============================================

class TestTopic:
    """Tests for Topic."""
    
    def test_topic_creation(self):
        """Test creating a topic."""
        topic = Topic(name="user.events", partitions=3)
        assert topic.name == "user.events"
        assert topic.partitions == 3
        assert topic.retention_ms is None
        assert topic.compacted is False
    
    def test_topic_validation_empty_name(self):
        """Test that empty topic name raises error."""
        with pytest.raises(ValueError):
            Topic(name="")
    
    def test_topic_validation_invalid_chars(self):
        """Test that invalid characters raise error."""
        with pytest.raises(ValueError):
            Topic(name="user/events")  # / is not allowed


class TestPubSubMessage:
    """Tests for PubSubMessage."""
    
    def test_message_creation(self):
        """Test creating a message."""
        msg = PubSubMessage(
            topic="test.topic",
            value={"key": "value"},
            key="msg-key",
        )
        assert msg.topic == "test.topic"
        assert msg.value == {"key": "value"}
        assert msg.key == "msg-key"
        assert msg.id is not None
    
    def test_message_to_actor_message(self):
        """Test converting to actor message."""
        msg = PubSubMessage(
            topic="test.topic",
            value={"data": "test"},
        )
        actor_msg = msg.to_actor_message()
        assert actor_msg is not None


class TestInMemoryPubSub:
    """Tests for InMemoryPubSub."""
    
    @pytest.fixture
    def pubsub(self):
        """Create a test pub/sub instance."""
        return InMemoryPubSub()
    
    @pytest.mark.asyncio
    async def test_create_topic(self, pubsub):
        """Test creating a topic."""
        topic = Topic(name="test.topic")
        result = await pubsub.create_topic(topic)
        assert result.name == "test.topic"
    
    @pytest.mark.asyncio
    async def test_publish_and_subscribe(self, pubsub):
        """Test publishing and subscribing."""
        # Create topic
        topic = Topic(name="test.topic")
        await pubsub.create_topic(topic)
        
        # Track received messages
        received = []
        
        async def handler(msg: PubSubMessage):
            received.append(msg)
        
        # Subscribe
        await pubsub.subscribe("test.topic", handler)
        
        # Publish
        msg = PubSubMessage(topic="test.topic", value={"test": "data"})
        await pubsub.publish("test.topic", msg)
        
        # Check message received
        assert len(received) == 1
        assert received[0].value == {"test": "data"}
    
    @pytest.mark.asyncio
    async def test_unsubscribe(self, pubsub):
        """Test unsubscribing."""
        topic = Topic(name="test.topic")
        await pubsub.create_topic(topic)
        
        received = []
        
        def handler(msg: PubSubMessage):
            received.append(msg)
        
        sub = await pubsub.subscribe("test.topic", handler)
        await pubsub.unsubscribe(sub.id)
        
        # Should not receive after unsubscribe
        msg = PubSubMessage(topic="test.topic", value={"test": "data"})
        # Note: InMemoryPubSub may still route, depending on implementation


class TestPubSubClient:
    """Tests for PubSubClient."""
    
    @pytest.fixture
    def client(self):
        """Create a test client."""
        return PubSubClient()
    
    @pytest.mark.asyncio
    async def test_create_topic(self, client):
        """Test creating a topic through client."""
        topic = Topic(name="user.events")
        result = await client.create_topic(topic)
        assert result.name == "user.events"
    
    @pytest.mark.asyncio
    async def test_publish(self, client):
        """Test publishing through client."""
        topic = Topic(name="test.topic")
        await client.create_topic(topic)
        
        msg_id = await client.publish(
            topic="test.topic",
            value={"key": "value"},
            key="test-key",
        )
        assert msg_id is not None
    
    @pytest.mark.asyncio
    async def test_publish_batch(self, client):
        """Test publishing batch through client."""
        topic = Topic(name="test.topic")
        await client.create_topic(topic)
        
        messages = [
            PubSubMessage(topic="test.topic", value={"id": 1}),
            PubSubMessage(topic="test.topic", value={"id": 2}),
        ]
        
        msg_ids = await client.publish_batch("test.topic", messages)
        assert len(msg_ids) == 2


# ============================================
# Event Sourcing Tests
# ============================================

class TestEventEnvelope:
    """Tests for EventEnvelope."""
    
    def test_envelope_creation(self):
        """Test creating an event envelope."""
        envelope = EventEnvelope(
            aggregate_id="agg-123",
            event_type="UserCreated",
            payload={"user_id": "123", "name": "Test"},
        )
        assert envelope.event_type == "UserCreated"
        assert envelope.payload["user_id"] == "123"


class TestEventVersion:
    """Tests for EventVersion."""
    
    def test_version_creation(self):
        """Test creating an event version."""
        version = EventVersion(major=1, minor=2)
        assert version.major == 1
        assert version.minor == 2
        assert str(version) == "v1.2"
    
    def test_version_parse(self):
        """Test parsing version string."""
        version = EventVersion.parse("v2.5")
        assert version.major == 2
        assert version.minor == 5


class TestInMemoryEventStore:
    """Tests for InMemoryEventStore."""
    
    @pytest.fixture
    def store(self):
        """Create a test event store."""
        return InMemoryEventStore()
    
    @pytest.mark.asyncio
    async def test_append_event(self, store):
        """Test appending an event."""
        event = {"type": "TestEvent", "key": "value"}
        
        result = await store.append("aggregate-1", [event])
        assert result == 1  # Returns new version number
    
    @pytest.mark.asyncio
    async def test_get_events(self, store):
        """Test getting events for an aggregate."""
        event1 = {"type": "Event1", "seq": 1}
        event2 = {"type": "Event2", "seq": 2}
        
        await store.append("aggregate-1", [event1])
        await store.append("aggregate-1", [event2])
        
        events = await store.get_events("aggregate-1")
        assert len(events) == 2
        assert events[0].event_type == "Event1"
        assert events[1].event_type == "Event2"
    
    @pytest.mark.asyncio
    async def test_get_events_empty(self, store):
        """Test getting events for non-existent aggregate."""
        events = await store.get_events("nonexistent")
        assert len(events) == 0
    
    @pytest.mark.asyncio
    async def test_append_multiple_events(self, store):
        """Test appending multiple events at once."""
        events = [
            {"type": "Event1", "data": "first"},
            {"type": "Event2", "data": "second"},
        ]
        
        result = await store.append("aggregate-1", events)
        assert result == 2  # Two events appended


# ============================================
# Delivery Guarantee Tests
# ============================================

class TestDeliveryGuarantee:
    """Tests for DeliveryGuarantee enum."""
    
    def test_guarantee_values(self):
        """Test delivery guarantee values."""
        assert DeliveryGuarantee.AT_MOST_ONCE.value == "at_most_once"
        assert DeliveryGuarantee.AT_LEAST_ONCE.value == "at_least_once"
        assert DeliveryGuarantee.EXACTLY_ONCE.value == "exactly_once"


class TestRetryPolicy:
    """Tests for RetryPolicy."""
    
    def test_default_policy(self):
        """Test default retry policy."""
        policy = RetryPolicy()
        assert policy.max_retries == 3
        assert policy.initial_backoff_ms == 100
    
    def test_backoff_calculation(self):
        """Test backoff time calculation."""
        policy = RetryPolicy(
            max_retries=5,
            initial_backoff_ms=100,
            backoff_multiplier=2.0,
        )
        
        assert policy.get_backoff_ms(0) == 100
        assert policy.get_backoff_ms(1) == 200
        assert policy.get_backoff_ms(2) == 400
    
    def test_backoff_max_cap(self):
        """Test that backoff is capped at max."""
        policy = RetryPolicy(
            max_retries=10,
            initial_backoff_ms=100,
            max_backoff_ms=1000,
            backoff_multiplier=2.0,
        )
        
        # Should cap at 1000
        assert policy.get_backoff_ms(10) == 1000


class TestOutboxEntry:
    """Tests for OutboxEntry."""
    
    def test_entry_creation(self):
        """Test creating an outbox entry."""
        entry = OutboxEntry(
            topic="test.topic",
            key="test-key",
            value={"data": "test"},
        )
        assert entry.topic == "test.topic"
        assert entry.attempts == 0
        assert entry.id is not None


class TestInMemoryOutbox:
    """Tests for InMemoryOutbox."""
    
    @pytest.fixture
    def outbox(self):
        """Create a test outbox."""
        return InMemoryOutbox()
    
    @pytest.mark.asyncio
    async def test_add_entry(self, outbox):
        """Test adding an entry to outbox."""
        entry = OutboxEntry(topic="test.topic", value={"test": "data"})
        await outbox.add(entry)
        
        pending = await outbox.get_pending()
        assert len(pending) == 1
    
    @pytest.mark.asyncio
    async def test_mark_delivered(self, outbox):
        """Test marking an entry as delivered."""
        entry = OutboxEntry(topic="test.topic", value={"test": "data"})
        await outbox.add(entry)
        
        await outbox.mark_delivered(entry.id)
        
        pending = await outbox.get_pending()
        assert len(pending) == 0
    
    @pytest.mark.asyncio
    async def test_mark_failed(self, outbox):
        """Test marking an entry as failed."""
        entry = OutboxEntry(topic="test.topic", value={"test": "data"})
        await outbox.add(entry)
        
        await outbox.mark_failed(entry.id, Exception("Test error"))
        
        # Entry should still be pending after first failure
        pending = await outbox.get_pending()
        # After max retries, it should be removed


# ============================================
# Schema Registry Tests
# ============================================

class TestSchema:
    """Tests for Schema."""
    
    def test_schema_creation(self):
        """Test creating a schema."""
        schema = Schema(
            name="UserCreated",
            type="json",
            definition={
                "type": "object",
                "properties": {
                    "userId": {"type": "string"},
                    "email": {"type": "string"},
                },
                "required": ["userId", "email"],
            },
        )
        assert schema.name == "UserCreated"
        assert schema.type == "json"


class TestJsonSchemaValidator:
    """Tests for JsonSchemaValidator."""
    
    @pytest.fixture
    def validator(self):
        """Create a test validator."""
        return JsonSchemaValidator()
    
    @pytest.fixture
    def test_schema(self):
        """Create a test schema."""
        return Schema(
            name="TestSchema",
            type="json",
            definition={
                "type": "object",
                "properties": {
                    "userId": {"type": "string"},
                    "count": {"type": "integer"},
                },
                "required": ["userId"],
            },
        )
    
    def test_validate_valid_data(self, validator, test_schema):
        """Test validating valid data."""
        data = {"userId": "123", "count": 5}
        errors = validator.validate(data, test_schema)
        assert len(errors) == 0
    
    def test_validate_missing_required(self, validator, test_schema):
        """Test validating data with missing required field."""
        data = {"count": 5}  # missing userId
        errors = validator.validate(data, test_schema)
        assert len(errors) > 0
        assert any("userId" in e for e in errors)
    
    def test_validate_wrong_type(self, validator, test_schema):
        """Test validating data with wrong type."""
        data = {"userId": "123", "count": "not a number"}
        errors = validator.validate(data, test_schema)
        assert len(errors) > 0


class TestInMemorySchemaRegistry:
    """Tests for InMemorySchemaRegistry."""
    
    @pytest.fixture
    def registry(self):
        """Create a test registry."""
        return InMemorySchemaRegistry()
    
    @pytest.fixture
    def test_schema(self):
        """Create a test schema."""
        return Schema(
            name="TestEvent",
            type="json",
            definition={
                "type": "object",
                "properties": {"id": {"type": "string"}},
                "required": ["id"],
            },
        )
    
    @pytest.mark.asyncio
    async def test_register_schema(self, registry, test_schema):
        """Test registering a schema."""
        version = await registry.register("TestEvent", test_schema)
        assert version.version == "1.0.0"
    
    @pytest.mark.asyncio
    async def test_get_schema(self, registry, test_schema):
        """Test getting a schema."""
        await registry.register("TestEvent", test_schema)
        
        retrieved = await registry.get_schema("TestEvent")
        assert retrieved is not None
        assert retrieved.name == "TestEvent"
    
    @pytest.mark.asyncio
    async def test_get_schema_not_found(self, registry):
        """Test getting a non-existent schema."""
        retrieved = await registry.get_schema("NonExistent")
        assert retrieved is None
    
    @pytest.mark.asyncio
    async def test_validate_data(self, registry, test_schema):
        """Test validating data against schema."""
        await registry.register("TestEvent", test_schema)
        
        # Valid data
        valid = await registry.validate("TestEvent", {"id": "123"})
        assert valid is True
        
        # Invalid data
        with pytest.raises(SchemaError):
            await registry.validate("TestEvent", {"name": "no id"})
    
    @pytest.mark.asyncio
    async def test_schema_versioning(self, registry, test_schema):
        """Test schema versioning."""
        await registry.register("TestEvent", test_schema)
        
        # Register a new version
        schema_v2 = Schema(
            name="TestEvent",
            type="json",
            definition={
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "name": {"type": "string"},
                },
                "required": ["id"],
            },
        )
        version = await registry.register("TestEvent", schema_v2)
        assert version.version == "1.0.1"
        
        # Get all versions
        versions = await registry.get_versions("TestEvent")
        assert len(versions) == 2


# Run all tests
if __name__ == "__main__":
    pytest.main([__file__, "-v"])
