"""
Tests for Aether SDK Event Sourcing Module
"""

from typing import Any, Dict

import pytest

# Import what's actually available
from aether_sdk.event.event_sourcing import (Aggregate, ConcurrencyError,
                                             EventEnvelope, EventVersion,
                                             InMemoryEventStore, Snapshot,
                                             apply_event)

# ============================================
# EventVersion Tests
# ============================================


class TestEventVersion:
    """Tests for EventVersion class."""

    def test_default_version(self):
        """Test default version creation."""
        version = EventVersion()
        assert version.major == 1
        assert version.minor == 0

    def test_custom_version(self):
        """Test custom version creation."""
        version = EventVersion(major=2, minor=5)
        assert version.major == 2
        assert version.minor == 5

    def test_str_representation(self):
        """Test string representation."""
        version = EventVersion(major=1, minor=2)
        assert str(version) == "v1.2"

    def test_comparison_operators(self):
        """Test version comparison operators."""
        v1 = EventVersion(major=1, minor=0)
        v2 = EventVersion(major=1, minor=1)
        v3 = EventVersion(major=2, minor=0)

        assert v1 < v2
        assert v2 < v3
        assert v1 < v3
        assert v1 <= v2
        assert v2 <= v2
        assert v3 > v1
        assert v3 >= v2

    def test_parse_version_string(self):
        """Test parsing version strings."""
        v1 = EventVersion.parse("v1.2")
        assert v1.major == 1
        assert v1.minor == 2

        v2 = EventVersion.parse("2.5")
        assert v2.major == 2
        assert v2.minor == 5

        v3 = EventVersion.parse("v3.0")
        assert v3.major == 3
        assert v3.minor == 0

    def test_equality(self):
        """Test version equality."""
        v1 = EventVersion(major=1, minor=0)
        v2 = EventVersion(major=1, minor=0)
        v3 = EventVersion(major=1, minor=1)

        assert v1 == v2
        assert v1 != v3


# ============================================
# EventEnvelope Tests
# ============================================


class TestEventEnvelope:
    """Tests for EventEnvelope class."""

    def test_default_envelope(self):
        """Test default envelope creation."""
        envelope = EventEnvelope()
        assert envelope.event_id != ""
        assert envelope.aggregate_id == ""
        assert envelope.version == 1

    def test_custom_envelope(self):
        """Test custom envelope creation."""
        envelope = EventEnvelope(
            event_id="test-id",
            aggregate_id="agg-123",
            aggregate_type="Order",
            event_type="order_created",
            version=5,
            payload={"items": ["item1"]},
            metadata={"source": "test"},
        )
        assert envelope.event_id == "test-id"
        assert envelope.aggregate_id == "agg-123"
        assert envelope.aggregate_type == "Order"
        assert envelope.event_type == "order_created"
        assert envelope.version == 5
        assert envelope.payload == {"items": ["item1"]}
        assert envelope.metadata == {"source": "test"}

    def test_to_dict(self):
        """Test serialization to dictionary."""
        envelope = EventEnvelope(
            event_id="test-id",
            aggregate_id="agg-123",
            aggregate_type="Order",
            event_type="order_created",
            version=1,
            payload={"total": 100},
            metadata={"source": "test"},
            causation_id="cause-123",
            correlation_id="corr-123",
        )

        result = envelope.to_dict()

        assert result["event_id"] == "test-id"
        assert result["aggregate_id"] == "agg-123"
        assert result["aggregate_type"] == "Order"
        assert result["event_type"] == "order_created"
        assert result["version"] == 1
        assert result["payload"] == {"total": 100}
        assert result["metadata"] == {"source": "test"}
        assert result["causation_id"] == "cause-123"
        assert result["correlation_id"] == "corr-123"
        assert "timestamp" in result

    def test_from_dict(self):
        """Test deserialization from dictionary."""
        data = {
            "event_id": "test-id",
            "aggregate_id": "agg-123",
            "aggregate_type": "Order",
            "event_type": "order_created",
            "version": 1,
            "timestamp": "2024-01-15T10:30:00",
            "payload": {"total": 100},
            "metadata": {"source": "test"},
            "schema_version": "v1.0",
            "causation_id": "cause-123",
            "correlation_id": "corr-123",
        }

        envelope = EventEnvelope.from_dict(data)

        assert envelope.event_id == "test-id"
        assert envelope.aggregate_id == "agg-123"
        assert envelope.aggregate_type == "Order"
        assert envelope.event_type == "order_created"
        assert envelope.version == 1
        assert envelope.payload == {"total": 100}
        assert envelope.metadata == {"source": "test"}


# ============================================
# Snapshot Tests
# ============================================


class TestSnapshot:
    """Tests for Snapshot class."""

    def test_snapshot_creation(self):
        """Test snapshot creation."""
        snapshot = Snapshot(
            aggregate_id="order-123",
            aggregate_type="Order",
            version=10,
            state={"status": "created", "total": 100},
        )

        assert snapshot.aggregate_id == "order-123"
        assert snapshot.aggregate_type == "Order"
        assert snapshot.version == 10
        assert snapshot.state == {"status": "created", "total": 100}


# ============================================
# Aggregate Tests
# ============================================


class TestOrder(Aggregate):
    """Test aggregate implementation."""

    def __init__(self):
        super().__init__()
        self.status = "pending"
        self.total = 0.0
        self.items = []

    def apply_order_created(self, event: Dict[str, Any]):
        self.status = "created"
        self.items = event.get("items", [])
        self.total = event.get("total", 0.0)

    def apply_item_added(self, event: Dict[str, Any]):
        self.items.append(event.get("item"))
        self.total += event.get("price", 0.0)

    def apply_order_shipped(self, event: Dict[str, Any]):
        self.status = "shipped"


class TestAggregate:
    """Tests for Aggregate base class."""

    def test_aggregate_initialization(self):
        """Test aggregate initialization."""
        order = TestOrder()
        assert order.id == ""
        assert order.version == 0
        assert order.uncommitted_events == []

    def test_apply_event(self):
        """Test applying events to aggregate."""
        order = TestOrder()

        envelope = EventEnvelope(
            aggregate_id="order-123",
            aggregate_type="TestOrder",
            event_type="order_created",
            version=1,
            payload={"items": ["item1"], "total": 50.0},
        )

        order.apply_event(envelope)

        assert order.id == "order-123"
        assert order.version == 1
        assert order.status == "created"
        assert order.items == ["item1"]
        assert order.total == 50.0

    def test_emit_event(self):
        """Test emitting events from aggregate."""
        order = TestOrder()
        order.id = "order-123"

        envelope = order.emit_event(
            "order_created", {"items": ["item1"], "total": 50.0}
        )

        assert envelope.event_type == "order_created"
        assert envelope.aggregate_id == "order-123"
        assert envelope.version == 1
        assert len(order.uncommitted_events) == 1

    def test_mark_events_committed(self):
        """Test marking events as committed."""
        order = TestOrder()
        order.id = "order-123"

        order.emit_event("order_created", {"items": []})
        order.emit_event("item_added", {"item": "item1"})

        assert len(order.uncommitted_events) == 2

        order.mark_events_committed()

        assert len(order.uncommitted_events) == 0

    def test_multiple_events(self):
        """Test applying multiple events."""
        order = TestOrder()
        order.id = "order-123"

        # Apply events
        events = [
            EventEnvelope(
                aggregate_id="order-123",
                event_type="order_created",
                version=1,
                payload={"items": ["item1"], "total": 50.0},
            ),
            EventEnvelope(
                aggregate_id="order-123",
                event_type="item_added",
                version=2,
                payload={"item": "item2", "price": 25.0},
            ),
            EventEnvelope(
                aggregate_id="order-123",
                event_type="order_shipped",
                version=3,
                payload={},
            ),
        ]

        for event in events:
            order.apply_event(event)

        assert order.version == 3
        assert order.status == "shipped"
        assert len(order.items) == 2
        assert order.total == 75.0


# ============================================
# InMemoryEventStore Tests
# ============================================


class TestInMemoryEventStore:
    """Tests for InMemoryEventStore."""

    def setup_method(self):
        """Set up test fixtures."""
        self.store = InMemoryEventStore()

    @pytest.mark.asyncio
    async def test_append_event(self):
        """Test appending events."""
        # append() expects a list of event dicts, not EventEnvelope objects
        event_dict = {"type": "order_created", "items": [], "total": 0.0}

        await self.store.append("order-123", [event_dict])

        events = await self.store.get_events("order-123")
        assert len(events) == 1
        assert events[0].event_type == "order_created"

    @pytest.mark.asyncio
    async def test_get_events_after_version(self):
        """Test getting events after a specific version."""
        for i in range(5):
            event_dict = {"type": f"event_{i}", "index": i}
            await self.store.append("order-123", [event_dict])

        events = await self.store.get_events("order-123", after_version=2)
        assert len(events) == 3
        assert events[0].version == 3

    @pytest.mark.asyncio
    async def test_get_all_events(self):
        """Test getting all events."""
        event_dict1 = {"type": "event_1", "data": "test1"}
        event_dict2 = {"type": "event_2", "data": "test2"}

        await self.store.append("order-1", [event_dict1])
        await self.store.append("order-2", [event_dict2])

        all_events = await self.store.get_all_events()
        assert len(all_events) == 2


# ============================================
# ConcurrencyError Tests
# ============================================


class TestConcurrencyError:
    """Tests for ConcurrencyError."""

    def test_error_creation(self):
        """Test creating concurrency error."""
        # ConcurrencyError is a simple exception that takes no keyword arguments
        error = ConcurrencyError(
            "Concurrency conflict: expected version 5, actual version 3"
        )

        assert "Concurrency" in str(error)
        assert "5" in str(error)
        assert "3" in str(error)


# ============================================
# apply_event Function Tests
# ============================================


class TestApplyEventFunction:
    """Tests for apply_event function."""

    def test_apply_event_to_aggregate(self):
        """Test applying events using helper function."""
        order = TestOrder()
        order.id = "order-123"

        envelope = EventEnvelope(
            aggregate_id="order-123",
            event_type="order_created",
            version=1,
            payload={"items": ["item1"], "total": 50.0},
        )

        apply_event(order, envelope)

        assert order.status == "created"
        assert order.items == ["item1"]
        assert order.total == 50.0
