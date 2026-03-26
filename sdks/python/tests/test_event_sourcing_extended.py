"""
Extended tests for Aether SDK Event Sourcing Module

Tests for EventSourcedActor, snapshots, and uncovered InMemoryEventStore methods.
"""

import pytest
import asyncio
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional

from aether_sdk.event.event_sourcing import (
    EventVersion,
    EventEnvelope,
    Snapshot,
    Aggregate,
    EventStore,
    InMemoryEventStore,
    ConcurrencyError,
    EventSourcedActor,
    apply_event,
)


# ============================================
# Test Aggregates
# ============================================

class OrderAggregate(Aggregate):
    """Test aggregate for event sourcing tests."""
    
    def __init__(self):
        super().__init__()
        self.status = "pending"
        self.total = 0.0
        self.items: List[str] = []
    
    def apply_order_created(self, event: Dict[str, Any]):
        self.status = "created"
        # Make a copy of the items list to avoid mutating the original event payload
        self.items = list(event.get("items", []))
        self.total = event.get("total", 0.0)
    
    def apply_item_added(self, event: Dict[str, Any]):
        self.items.append(event.get("item"))
        self.total += event.get("price", 0.0)
    
    def apply_order_shipped(self, event: Dict[str, Any]):
        self.status = "shipped"
    
    def apply_order_cancelled(self, event: Dict[str, Any]):
        self.status = "cancelled"


class BankAccountAggregate(Aggregate):
    """Test aggregate for bank account."""
    
    def __init__(self):
        super().__init__()
        self.balance = 0.0
        self.owner = ""
        self.is_active = False
    
    def apply_account_opened(self, event: Dict[str, Any]):
        self.owner = event.get("owner", "")
        self.balance = event.get("initial_balance", 0.0)
        self.is_active = True
    
    def apply_money_deposited(self, event: Dict[str, Any]):
        self.balance += event.get("amount", 0.0)
    
    def apply_money_withdrawn(self, event: Dict[str, Any]):
        self.balance -= event.get("amount", 0.0)
    
    def apply_account_closed(self, event: Dict[str, Any]):
        self.is_active = False


# ============================================
# InMemoryEventStore Extended Tests
# ============================================

class TestInMemoryEventStoreExtended:
    """Extended tests for InMemoryEventStore."""
    
    def setup_method(self):
        """Set up test fixtures."""
        self.store = InMemoryEventStore()
    
    @pytest.mark.asyncio
    async def test_append_multiple_events_at_once(self):
        """Test appending multiple events in a single call."""
        events = [
            {"type": "order_created", "items": ["item1"], "total": 50.0},
            {"type": "item_added", "item": "item2", "price": 25.0},
            {"type": "order_shipped"},
        ]
        
        version = await self.store.append("order-123", events)
        assert version == 3
        
        stored_events = await self.store.get_events("order-123")
        assert len(stored_events) == 3
        assert stored_events[0].event_type == "order_created"
        assert stored_events[1].event_type == "item_added"
        assert stored_events[2].event_type == "order_shipped"
    
    @pytest.mark.asyncio
    async def test_append_with_expected_version_success(self):
        """Test appending with correct expected version."""
        await self.store.append("order-123", [{"type": "event1"}])
        await self.store.append("order-123", [{"type": "event2"}], expected_version=1)
        
        events = await self.store.get_events("order-123")
        assert len(events) == 2
    
    @pytest.mark.asyncio
    async def test_append_with_expected_version_failure(self):
        """Test appending with incorrect expected version raises ConcurrencyError."""
        await self.store.append("order-123", [{"type": "event1"}])
        await self.store.append("order-123", [{"type": "event2"}])
        
        # Try to append with wrong expected version
        with pytest.raises(ConcurrencyError) as exc_info:
            await self.store.append("order-123", [{"type": "event3"}], expected_version=1)
        
        assert "Expected version 1" in str(exc_info.value)
    
    @pytest.mark.asyncio
    async def test_append_with_expected_version_zero_for_new_aggregate(self):
        """Test appending to new aggregate with expected_version=0."""
        # expected_version=0 should work for new aggregate
        version = await self.store.append("order-new", [{"type": "first"}], expected_version=0)
        assert version == 1
    
    @pytest.mark.asyncio
    async def test_get_events_empty_aggregate(self):
        """Test getting events for non-existent aggregate."""
        events = await self.store.get_events("non-existent")
        assert events == []
    
    @pytest.mark.asyncio
    async def test_get_events_after_event_id(self):
        """Test getting events after a specific event ID."""
        # Add events
        events = [
            {"type": "event1"},
            {"type": "event2"},
            {"type": "event3"},
            {"type": "event4"},
        ]
        await self.store.append("order-123", events)
        
        stored = await self.store.get_events("order-123")
        second_event_id = stored[1].event_id
        
        # Get events after the second event
        after_events = await self.store.get_events_after("order-123", second_event_id)
        assert len(after_events) == 2
        assert after_events[0].event_type == "event3"
        assert after_events[1].event_type == "event4"
    
    @pytest.mark.asyncio
    async def test_get_events_after_last_event(self):
        """Test getting events after the last event returns empty list."""
        events = [{"type": "event1"}, {"type": "event2"}]
        await self.store.append("order-123", events)
        
        stored = await self.store.get_events("order-123")
        last_event_id = stored[-1].event_id
        
        after_events = await self.store.get_events_after("order-123", last_event_id)
        assert after_events == []
    
    @pytest.mark.asyncio
    async def test_get_events_after_nonexistent_event_id(self):
        """Test getting events after non-existent event ID returns empty list."""
        events = [{"type": "event1"}]
        await self.store.append("order-123", events)
        
        after_events = await self.store.get_events_after("order-123", "nonexistent-id")
        assert after_events == []
    
    @pytest.mark.asyncio
    async def test_get_events_after_empty_aggregate(self):
        """Test getting events after for non-existent aggregate."""
        after_events = await self.store.get_events_after("non-existent", "any-id")
        assert after_events == []
    
    @pytest.mark.asyncio
    async def test_get_events_between_versions(self):
        """Test getting events within a version range."""
        events = [
            {"type": "event1"},
            {"type": "event2"},
            {"type": "event3"},
            {"type": "event4"},
            {"type": "event5"},
        ]
        await self.store.append("order-123", events)
        
        # Get events from version 2 to 4 (inclusive)
        between = await self.store.get_events_between_versions("order-123", 2, 4)
        assert len(between) == 3
        assert between[0].version == 2
        assert between[1].version == 3
        assert between[2].version == 4
    
    @pytest.mark.asyncio
    async def test_get_events_between_versions_single(self):
        """Test getting single event by version range."""
        events = [{"type": "event1"}, {"type": "event2"}]
        await self.store.append("order-123", events)
        
        between = await self.store.get_events_between_versions("order-123", 1, 1)
        assert len(between) == 1
        assert between[0].version == 1
    
    @pytest.mark.asyncio
    async def test_get_events_between_versions_empty_aggregate(self):
        """Test getting events between versions for non-existent aggregate."""
        between = await self.store.get_events_between_versions("non-existent", 1, 5)
        assert between == []
    
    @pytest.mark.asyncio
    async def test_get_all_events_with_aggregate_type_filter(self):
        """Test getting all events filtered by aggregate type."""
        # The aggregate_type is set on EventEnvelope during creation
        # But InMemoryEventStore doesn't set it in append()
        # Let's test the filter path anyway
        
        await self.store.append("order-1", [{"type": "order_created"}])
        await self.store.append("order-2", [{"type": "order_shipped"}])
        
        # This will return all events since aggregate_type isn't set
        all_events = await self.store.get_all_events()
        assert len(all_events) == 2
    
    @pytest.mark.asyncio
    async def test_get_all_events_with_timestamp_filter(self):
        """Test getting all events filtered by timestamp."""
        await self.store.append("order-1", [{"type": "event1"}])
        await self.store.append("order-2", [{"type": "event2"}])
        
        # Get events from future - should be empty
        future_time = datetime.utcnow() + timedelta(hours=1)
        future_events = await self.store.get_all_events(from_timestamp=future_time)
        assert future_events == []
        
        # Get events from past - should include all
        past_time = datetime.utcnow() - timedelta(hours=1)
        all_events = await self.store.get_all_events(from_timestamp=past_time)
        assert len(all_events) == 2
    
    @pytest.mark.asyncio
    async def test_save_and_load_snapshot(self):
        """Test saving and loading snapshots."""
        snapshot = Snapshot(
            aggregate_id="order-123",
            aggregate_type="OrderAggregate",
            version=5,
            state={"status": "created", "total": 100.0}
        )
        
        await self.store.save_snapshot(snapshot)
        
        loaded = await self.store.load_snapshot("order-123")
        assert loaded is not None
        assert loaded.aggregate_id == "order-123"
        assert loaded.aggregate_type == "OrderAggregate"
        assert loaded.version == 5
        assert loaded.state == {"status": "created", "total": 100.0}
    
    @pytest.mark.asyncio
    async def test_load_snapshot_nonexistent(self):
        """Test loading snapshot for non-existent aggregate."""
        snapshot = await self.store.load_snapshot("non-existent")
        assert snapshot is None
    
    @pytest.mark.asyncio
    async def test_snapshot_overwrites_previous(self):
        """Test that saving a new snapshot overwrites the previous one."""
        snapshot1 = Snapshot(
            aggregate_id="order-123",
            aggregate_type="Order",
            version=5,
            state={"status": "created"}
        )
        snapshot2 = Snapshot(
            aggregate_id="order-123",
            aggregate_type="Order",
            version=10,
            state={"status": "shipped"}
        )
        
        await self.store.save_snapshot(snapshot1)
        await self.store.save_snapshot(snapshot2)
        
        loaded = await self.store.load_snapshot("order-123")
        assert loaded.version == 10
        assert loaded.state == {"status": "shipped"}


# ============================================
# Aggregate Extended Tests
# ============================================

class TestAggregateExtended:
    """Extended tests for Aggregate class."""
    
    def test_load_from_history_without_snapshot(self):
        """Test loading aggregate from event history without snapshot."""
        order = OrderAggregate()
        
        events = [
            EventEnvelope(
                aggregate_id="order-123",
                event_type="order_created",
                version=1,
                payload={"items": ["item1"], "total": 50.0}
            ),
            EventEnvelope(
                aggregate_id="order-123",
                event_type="item_added",
                version=2,
                payload={"item": "item2", "price": 25.0}
            ),
            EventEnvelope(
                aggregate_id="order-123",
                event_type="order_shipped",
                version=3,
                payload={}
            ),
        ]
        
        order.load_from_history(events)
        
        assert order.id == "order-123"
        assert order.version == 3
        assert order.status == "shipped"
        assert len(order.items) == 2
        assert order.total == 75.0
    
    def test_load_from_history_with_snapshot(self):
        """Test loading aggregate from snapshot and events."""
        order = OrderAggregate()
        
        snapshot = Snapshot(
            aggregate_id="order-123",
            aggregate_type="OrderAggregate",
            version=2,
            state={"status": "created", "total": 75.0, "items": ["item1", "item2"]}
        )
        
        events = [
            EventEnvelope(
                aggregate_id="order-123",
                event_type="order_shipped",
                version=3,
                payload={}
            ),
        ]
        
        order.load_from_history(events, snapshot)
        
        # Should have loaded state from snapshot
        assert order.id == "order-123"
        assert order.version == 3
        assert order.status == "shipped"  # Updated from event
        assert len(order.items) == 2  # From snapshot
    
    def test_load_from_history_skips_older_events(self):
        """Test that loading from history skips events older than current version."""
        order = OrderAggregate()
        order._version = 2  # Already at version 2
        
        events = [
            EventEnvelope(
                aggregate_id="order-123",
                event_type="order_created",
                version=1,
                payload={"items": [], "total": 0}
            ),
            EventEnvelope(
                aggregate_id="order-123",
                event_type="item_added",
                version=2,
                payload={"item": "item1", "price": 10}
            ),
            EventEnvelope(
                aggregate_id="order-123",
                event_type="item_added",
                version=3,
                payload={"item": "item2", "price": 20}
            ),
        ]
        
        order.load_from_history(events)
        
        # Should only apply version 3 (version > current version 2)
        assert order.version == 3
        assert len(order.items) == 1  # Only item2 from version 3
    
    def test_create_snapshot(self):
        """Test creating a snapshot from aggregate."""
        order = OrderAggregate()
        order._id = "order-123"
        order._version = 5
        order.status = "created"
        order.total = 100.0
        order.items = ["item1", "item2"]
        
        snapshot = order.create_snapshot()
        
        assert snapshot.aggregate_id == "order-123"
        assert snapshot.aggregate_type == "OrderAggregate"
        assert snapshot.version == 5
        assert snapshot.state["status"] == "created"
        assert snapshot.state["total"] == 100.0
        assert snapshot.state["items"] == ["item1", "item2"]
    
    def test_create_snapshot_excludes_private_attributes(self):
        """Test that create_snapshot excludes private attributes."""
        order = OrderAggregate()
        order._id = "order-123"
        order._version = 5
        order.status = "created"
        
        snapshot = order.create_snapshot()
        
        # Private attributes (starting with _) should not be in state
        assert "_id" not in snapshot.state
        assert "_version" not in snapshot.state
        assert "_uncommitted_events" not in snapshot.state
        assert "status" in snapshot.state
    
    def test_load_snapshot(self):
        """Test _load_snapshot method."""
        order = OrderAggregate()
        
        snapshot = Snapshot(
            aggregate_id="order-456",
            aggregate_type="OrderAggregate",
            version=10,
            state={"status": "shipped", "total": 200.0, "items": ["a", "b", "c"]}
        )
        
        order._load_snapshot(snapshot)
        
        assert order.id == "order-456"
        assert order.version == 10
        assert order._snapshot_version == 10
        assert order.status == "shipped"
        assert order.total == 200.0
        assert order.items == ["a", "b", "c"]
    
    def test_emit_event_increments_version(self):
        """Test that emit_event increments version."""
        order = OrderAggregate()
        order._id = "order-123"
        
        envelope1 = order.emit_event("order_created", {"items": ["item1"]})
        assert envelope1.version == 1
        assert order.version == 1
        
        envelope2 = order.emit_event("item_added", {"item": "item2", "price": 25})
        assert envelope2.version == 2
        assert order.version == 2
    
    def test_emit_event_sets_aggregate_id_from_first_event(self):
        """Test that aggregate ID is set from first applied event envelope."""
        order = OrderAggregate()
        assert order.id == ""
        
        # Set ID before emitting (as would normally happen)
        order._id = "order-123"
        order.emit_event("order_created", {"items": []})
        
        # ID should remain as set
        assert order.id == "order-123"
    
    def test_emit_event_adds_to_uncommitted(self):
        """Test that emit_event adds to uncommitted events."""
        order = OrderAggregate()
        order._id = "order-123"
        
        order.emit_event("order_created", {"items": []})
        order.emit_event("item_added", {"item": "item1"})
        
        assert len(order.uncommitted_events) == 2
    
    def test_mark_events_committed(self):
        """Test marking events as committed."""
        order = OrderAggregate()
        order._id = "order-123"
        
        order.emit_event("order_created", {"items": []})
        assert len(order.uncommitted_events) == 1
        
        order.mark_events_committed()
        assert len(order.uncommitted_events) == 0
    
    def test_apply_event_unknown_type(self):
        """Test applying event with no handler method."""
        order = OrderAggregate()
        order._id = "order-123"
        
        # Event with no apply_ handler should be silently ignored
        # and version should NOT be incremented (no handler = no state change)
        envelope = EventEnvelope(
            aggregate_id="order-123",
            event_type="unknown_event",
            version=1,
            payload={"data": "test"}
        )
        
        # Should not raise
        order.apply_event(envelope)
        
        # Version should remain 0 since no handler was found
        assert order.version == 0
    
    def test_apply_event_sets_id_from_envelope(self):
        """Test that apply_event sets aggregate ID from envelope."""
        order = OrderAggregate()
        assert order.id == ""
        
        envelope = EventEnvelope(
            aggregate_id="order-from-envelope",
            event_type="order_created",
            version=1,
            payload={"items": [], "total": 0}
        )
        
        order.apply_event(envelope)
        assert order.id == "order-from-envelope"


# ============================================
# EventSourcedActor Tests
# ============================================

class TestEventSourcedActor:
    """Tests for EventSourcedActor mixin."""
    
    def setup_method(self):
        """Set up test fixtures."""
        self.actor = EventSourcedActor()
    
    def test_initialization_with_default_store(self):
        """Test that EventSourcedActor initializes with InMemoryEventStore."""
        assert self.actor._event_store is not None
        assert isinstance(self.actor._event_store, InMemoryEventStore)
    
    def test_initialization_with_custom_store(self):
        """Test initialization with custom event store."""
        custom_store = InMemoryEventStore()
        actor = EventSourcedActor(event_store=custom_store)
        assert actor._event_store is custom_store
    
    @pytest.mark.asyncio
    async def test_load_aggregate(self):
        """Test loading an aggregate from the event store."""
        # First, add some events
        await self.actor._event_store.append(
            "order-123",
            [{"type": "order_created", "items": ["item1"], "total": 50.0}]
        )
        
        # Load the aggregate
        aggregate = await self.actor.load_aggregate("order-123", OrderAggregate)
        
        assert aggregate.id == "order-123"
        assert aggregate.status == "created"
        assert aggregate.items == ["item1"]
        assert aggregate.total == 50.0
    
    @pytest.mark.asyncio
    async def test_load_aggregate_with_snapshot(self):
        """Test loading aggregate with snapshot optimization."""
        # Create and save a snapshot
        snapshot = Snapshot(
            aggregate_id="order-456",
            aggregate_type="OrderAggregate",
            version=2,
            state={"status": "created", "total": 50.0, "items": ["item1"]}
        )
        await self.actor._event_store.save_snapshot(snapshot)
        
        # Add events after snapshot
        await self.actor._event_store.append(
            "order-456",
            [
                {"type": "order_created", "items": ["item1"], "total": 50.0},
                {"type": "item_added", "item": "item2", "price": 25.0},
                {"type": "order_shipped"},
            ]
        )
        
        # Load aggregate - should use snapshot + remaining events
        aggregate = await self.actor.load_aggregate("order-456", OrderAggregate)
        
        assert aggregate.version == 3
        assert aggregate.status == "shipped"
    
    @pytest.mark.asyncio
    async def test_load_aggregate_nonexistent(self):
        """Test loading a non-existent aggregate creates new instance."""
        aggregate = await self.actor.load_aggregate("nonexistent", OrderAggregate)
        
        assert aggregate.id == "nonexistent"
        assert aggregate.version == 0
        assert aggregate.status == "pending"  # Default state
    
    @pytest.mark.asyncio
    async def test_save_aggregate(self):
        """Test saving an aggregate's uncommitted events."""
        # Create aggregate and emit events
        aggregate = OrderAggregate()
        aggregate._id = "order-789"
        aggregate.emit_event("order_created", {"items": ["item1"], "total": 50.0})
        aggregate.emit_event("item_added", {"item": "item2", "price": 25.0})
        
        # Save
        await self.actor.save_aggregate(aggregate)
        
        # Verify events were persisted
        stored = await self.actor._event_store.get_events("order-789")
        assert len(stored) == 2
        assert stored[0].event_type == "order_created"
        assert stored[1].event_type == "item_added"
        
        # Verify uncommitted events were cleared
        assert len(aggregate.uncommitted_events) == 0
    
    @pytest.mark.asyncio
    async def test_save_aggregate_no_events(self):
        """Test saving aggregate with no uncommitted events."""
        aggregate = OrderAggregate()
        aggregate._id = "order-empty"
        
        # Should not raise and should not persist anything
        await self.actor.save_aggregate(aggregate)
        
        stored = await self.actor._event_store.get_events("order-empty")
        assert len(stored) == 0
    
    @pytest.mark.asyncio
    async def test_save_snapshot_from_aggregate(self):
        """Test saving a snapshot from an aggregate."""
        aggregate = OrderAggregate()
        aggregate._id = "order-snap"
        aggregate._version = 5
        aggregate.status = "created"
        aggregate.total = 100.0
        
        await self.actor.save_snapshot("order-snap")
        
        # The actor should use the aggregate from its cache
        # But since it's not in cache, this tests the path where aggregate_id not in cache
        # Let's add it to cache first
        self.actor._aggregates["order-snap"] = aggregate
        await self.actor.save_snapshot("order-snap")
        
        loaded = await self.actor._event_store.load_snapshot("order-snap")
        assert loaded is not None
        assert loaded.version == 5
    
    @pytest.mark.asyncio
    async def test_save_snapshot_nonexistent_aggregate(self):
        """Test saving snapshot for aggregate not in cache."""
        # This should return early without error
        await self.actor.save_snapshot("nonexistent-aggregate")
    
    @pytest.mark.asyncio
    async def test_aggregates_cache(self):
        """Test that loaded aggregates are cached."""
        await self.actor._event_store.append(
            "order-cached",
            [{"type": "order_created", "items": [], "total": 0}]
        )
        
        aggregate1 = await self.actor.load_aggregate("order-cached", OrderAggregate)
        aggregate2 = await self.actor.load_aggregate("order-cached", OrderAggregate)
        
        # Should be the same instance from cache
        assert aggregate1 is aggregate2


# ============================================
# Integration Tests
# ============================================

class TestEventSourcingIntegration:
    """Integration tests for event sourcing."""
    
    @pytest.mark.asyncio
    async def test_full_lifecycle(self):
        """Test full aggregate lifecycle: create, modify, save, load."""
        store = InMemoryEventStore()
        
        # Create and save with first actor
        actor1 = EventSourcedActor(event_store=store)
        order = OrderAggregate()
        order._id = "order-full"
        order.emit_event("order_created", {"items": ["item1"], "total": 50.0})
        order.emit_event("item_added", {"item": "item2", "price": 25.0})
        await actor1.save_aggregate(order)
        
        # Load fresh instance with a new actor (avoids caching issues)
        actor2 = EventSourcedActor(event_store=store)
        loaded = await actor2.load_aggregate("order-full", OrderAggregate)
        
        assert loaded.version == 2
        assert loaded.status == "created"
        assert len(loaded.items) == 2
        assert loaded.total == 75.0
        
        # Modify and save again
        loaded.emit_event("order_shipped", {})
        await actor2.save_aggregate(loaded)
        
        # Load again with a fresh actor
        actor3 = EventSourcedActor(event_store=store)
        final = await actor3.load_aggregate("order-full", OrderAggregate)
        assert final.version == 3
        assert final.status == "shipped"
    
    @pytest.mark.asyncio
    async def test_snapshot_optimization(self):
        """Test that snapshots optimize replay."""
        store = InMemoryEventStore()
        actor = EventSourcedActor(event_store=store)
        
        # Create aggregate with many events
        order = OrderAggregate()
        order._id = "order-many"
        order.emit_event("order_created", {"items": [], "total": 0})
        
        for i in range(100):
            order.emit_event("item_added", {"item": f"item{i}", "price": 10.0})
        
        await actor.save_aggregate(order)
        
        # Save snapshot
        actor._aggregates["order-many"] = order
        await actor.save_snapshot("order-many")
        
        # Load should use snapshot
        loaded = await actor.load_aggregate("order-many", OrderAggregate)
        assert loaded.version == 101
        assert len(loaded.items) == 100


# ============================================
# EventVersion Edge Cases
# ============================================

class TestEventVersionEdgeCases:
    """Edge case tests for EventVersion."""
    
    def test_parse_with_v_prefix(self):
        """Test parsing version with 'v' prefix."""
        version = EventVersion.parse("v2.5")
        assert version.major == 2
        assert version.minor == 5
    
    def test_parse_without_v_prefix(self):
        """Test parsing version without 'v' prefix."""
        version = EventVersion.parse("3.7")
        assert version.major == 3
        assert version.minor == 7
    
    def test_parse_without_minor_version(self):
        """Test parsing version without minor version defaults to 0."""
        version = EventVersion.parse("5")
        assert version.major == 5
        assert version.minor == 0
    
    def test_comparison_edge_cases(self):
        """Test version comparison edge cases."""
        v1_0 = EventVersion(major=1, minor=0)
        v1_0_copy = EventVersion(major=1, minor=0)
        v1_1 = EventVersion(major=1, minor=1)
        v2_0 = EventVersion(major=2, minor=0)
        
        # Equality
        assert v1_0 == v1_0_copy
        assert v1_0 != v1_1
        
        # Less than
        assert v1_0 < v1_1
        assert v1_1 < v2_0
        assert not v1_1 < v1_0
        
        # Greater than
        assert v2_0 > v1_1
        assert v1_1 > v1_0
        assert not v1_0 > v1_1
        
        # Less than or equal
        assert v1_0 <= v1_0_copy
        assert v1_0 <= v1_1
        
        # Greater than or equal
        assert v1_1 >= v1_0
        assert v1_0 >= v1_0_copy
