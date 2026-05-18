"""
Tests for Aether SDK Event Delivery Module

Tests for delivery guarantees, retry policies, outbox pattern, and dead letter queue.
"""

import asyncio
from datetime import datetime, timedelta, timezone

import pytest

from aether_sdk.event.delivery import (
    DeliveryGuarantee,
    DeliveryStats,
    InMemoryDeadLetterQueue,
    InMemoryOutbox,
    OutboxEntry,
    RetryPolicy,
)

# ============================================
# DeliveryGuarantee Tests
# ============================================


class TestDeliveryGuarantee:
    """Tests for DeliveryGuarantee enum."""

    def test_at_most_once(self):
        """Test AT_MOST_ONCE guarantee."""
        assert DeliveryGuarantee.AT_MOST_ONCE.value == "at_most_once"

    def test_at_least_once(self):
        """Test AT_LEAST_ONCE guarantee."""
        assert DeliveryGuarantee.AT_LEAST_ONCE.value == "at_least_once"

    def test_exactly_once(self):
        """Test EXACTLY_ONCE guarantee."""
        assert DeliveryGuarantee.EXACTLY_ONCE.value == "exactly_once"

    def test_all_guarantees_defined(self):
        """Test that all expected guarantees are defined."""
        guarantees = list(DeliveryGuarantee)
        assert len(guarantees) == 3


# ============================================
# RetryPolicy Tests
# ============================================


class TestRetryPolicy:
    """Tests for RetryPolicy class."""

    def test_default_retry_policy(self):
        """Test default retry policy values."""
        policy = RetryPolicy()

        assert policy.max_retries == 3
        assert policy.initial_backoff_ms == 100
        assert policy.max_backoff_ms == 30000
        assert policy.backoff_multiplier == 2.0

    def test_custom_retry_policy(self):
        """Test custom retry policy values."""
        policy = RetryPolicy(
            max_retries=5,
            initial_backoff_ms=200,
            max_backoff_ms=60000,
            backoff_multiplier=1.5,
        )

        assert policy.max_retries == 5
        assert policy.initial_backoff_ms == 200
        assert policy.max_backoff_ms == 60000
        assert policy.backoff_multiplier == 1.5

    def test_get_backoff_first_attempt(self):
        """Test backoff calculation for first attempt."""
        policy = RetryPolicy(initial_backoff_ms=100, backoff_multiplier=2.0)

        backoff = policy.get_backoff_ms(0)
        assert backoff == 100  # 100 * 2^0 = 100

    def test_get_backoff_second_attempt(self):
        """Test backoff calculation for second attempt."""
        policy = RetryPolicy(initial_backoff_ms=100, backoff_multiplier=2.0)

        backoff = policy.get_backoff_ms(1)
        assert backoff == 200  # 100 * 2^1 = 200

    def test_get_backoff_third_attempt(self):
        """Test backoff calculation for third attempt."""
        policy = RetryPolicy(initial_backoff_ms=100, backoff_multiplier=2.0)

        backoff = policy.get_backoff_ms(2)
        assert backoff == 400  # 100 * 2^2 = 400

    def test_get_backoff_respects_max(self):
        """Test that backoff respects max_backoff_ms."""
        policy = RetryPolicy(
            initial_backoff_ms=1000, max_backoff_ms=5000, backoff_multiplier=3.0
        )

        # 1000 * 3^2 = 9000, but should cap at 5000
        backoff = policy.get_backoff_ms(2)
        assert backoff == 5000

    def test_get_backoff_high_attempt(self):
        """Test backoff calculation for high attempt number."""
        policy = RetryPolicy(
            initial_backoff_ms=100, max_backoff_ms=30000, backoff_multiplier=2.0
        )

        # 100 * 2^10 = 102400, should cap at 30000
        backoff = policy.get_backoff_ms(10)
        assert backoff == 30000

    def test_get_backoff_fractional_multiplier(self):
        """Test backoff with fractional multiplier."""
        policy = RetryPolicy(initial_backoff_ms=1000, backoff_multiplier=1.5)

        backoff = policy.get_backoff_ms(2)
        # 1000 * 1.5^2 = 2250
        assert backoff == 2250


# ============================================
# OutboxEntry Tests
# ============================================


class TestOutboxEntry:
    """Tests for OutboxEntry class."""

    def test_default_entry(self):
        """Test default outbox entry creation."""
        entry = OutboxEntry()

        assert entry.id != ""  # Should have auto-generated ID
        assert entry.topic == ""
        assert entry.topic == ""
        assert entry.key is None
        assert entry.value is None
        assert entry.headers == {}
        assert entry.attempts == 0
        assert entry.last_error is None
        assert entry.next_retry_at is None

    def test_custom_entry(self):
        """Test custom outbox entry."""
        entry = OutboxEntry(
            id="custom-id",
            topic="orders",
            key="order-123",
            value={"order_id": "123", "status": "created"},
            headers={"source": "order-service", "version": "1.0"},
        )

        assert entry.id == "custom-id"
        assert entry.topic == "orders"
        assert entry.key == "order-123"
        assert entry.value == {"order_id": "123", "status": "created"}
        assert entry.headers == {"source": "order-service", "version": "1.0"}

    def test_entry_with_retry_info(self):
        """Test entry with retry information."""
        retry_time = datetime.utcnow() + timedelta(minutes=5)
        entry = OutboxEntry(
            topic="events",
            value={"data": "test"},
            attempts=2,
            last_error="Connection refused",
            next_retry_at=retry_time,
        )

        assert entry.attempts == 2
        assert entry.last_error == "Connection refused"
        assert entry.next_retry_at == retry_time

    def test_entry_timestamp_auto_generated(self):
        """Test that created_at is auto-generated."""
        before = datetime.now(timezone.utc)
        entry = OutboxEntry()
        after = datetime.now(timezone.utc)

        assert before <= entry.created_at <= after


# ============================================
# DeliveryStats Tests
# ============================================


class TestDeliveryStats:
    """Tests for DeliveryStats class."""

    def test_default_stats(self):
        """Test default delivery stats."""
        stats = DeliveryStats()

        assert stats.total_sent == 0
        assert stats.total_failed == 0
        assert stats.total_retries == 0
        assert stats.last_sent_at is None
        assert stats.last_failed_at is None

    def test_custom_stats(self):
        """Test custom delivery stats."""
        sent_time = datetime.utcnow()
        stats = DeliveryStats(
            total_sent=100, total_failed=5, total_retries=15, last_sent_at=sent_time
        )

        assert stats.total_sent == 100
        assert stats.total_failed == 5
        assert stats.total_retries == 15
        assert stats.last_sent_at == sent_time


# ============================================
# InMemoryDeadLetterQueue Tests
# ============================================


class TestInMemoryDeadLetterQueue:
    """Tests for InMemoryDeadLetterQueue class."""

    def setup_method(self):
        """Set up test fixtures."""
        self.dlq = InMemoryDeadLetterQueue()

    def test_initialization_default_max_size(self):
        """Test default max size."""
        assert self.dlq._max_size == 1000

    def test_initialization_custom_max_size(self):
        """Test custom max size."""
        dlq = InMemoryDeadLetterQueue(max_size=100)
        assert dlq._max_size == 100

    @pytest.mark.asyncio
    async def test_add_entry(self):
        """Test adding entry to DLQ."""
        entry = OutboxEntry(topic="orders", value={"id": 1})
        error = Exception("Delivery failed")

        await self.dlq.add(entry, error)

        pending = await self.dlq.get_pending()
        assert len(pending) == 1
        assert pending[0] == entry

    @pytest.mark.asyncio
    async def test_add_multiple_entries(self):
        """Test adding multiple entries."""
        for i in range(5):
            entry = OutboxEntry(topic="orders", value={"id": i})
            error = Exception(f"Error {i}")
            await self.dlq.add(entry, error)

        pending = await self.dlq.get_pending()
        assert len(pending) == 5

    @pytest.mark.asyncio
    async def test_get_pending_with_limit(self):
        """Test getting pending entries with limit."""
        for i in range(10):
            entry = OutboxEntry(topic="orders", value={"id": i})
            await self.dlq.add(entry, Exception())

        pending = await self.dlq.get_pending(limit=5)
        assert len(pending) == 5

    @pytest.mark.asyncio
    async def test_get_pending_empty(self):
        """Test getting pending from empty DLQ."""
        pending = await self.dlq.get_pending()
        assert pending == []

    @pytest.mark.asyncio
    async def test_remove_entry(self):
        """Test removing entry from DLQ."""
        entry = OutboxEntry(id="test-id", topic="orders", value={})
        await self.dlq.add(entry, Exception())

        result = await self.dlq.remove("test-id")
        assert result is True

        pending = await self.dlq.get_pending()
        assert len(pending) == 0

    @pytest.mark.asyncio
    async def test_remove_nonexistent_entry(self):
        """Test removing non-existent entry."""
        result = await self.dlq.remove("nonexistent")
        assert result is False

    @pytest.mark.asyncio
    async def test_max_size_eviction(self):
        """Test that oldest entry is evicted when max size reached."""
        dlq = InMemoryDeadLetterQueue(max_size=3)

        # Add 4 entries
        for i in range(4):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={"id": i})
            await dlq.add(entry, Exception())

        pending = await dlq.get_pending()

        # Should have 3 entries (oldest evicted)
        assert len(pending) == 3

        # First entry should be evicted
        ids = [e.id for e in pending]
        assert "entry-0" not in ids
        assert "entry-1" in ids
        assert "entry-2" in ids
        assert "entry-3" in ids


# ============================================
# InMemoryOutbox Tests
# ============================================


class TestInMemoryOutbox:
    """Tests for InMemoryOutbox class."""

    def setup_method(self):
        """Set up test fixtures."""
        self.outbox = InMemoryOutbox()

    def test_initialization_default_policy(self):
        """Test default retry policy."""
        assert self.outbox._retry_policy is not None
        assert self.outbox._retry_policy.max_retries == 3

    def test_initialization_custom_policy(self):
        """Test custom retry policy."""
        policy = RetryPolicy(max_retries=5, initial_backoff_ms=200)
        outbox = InMemoryOutbox(retry_policy=policy)

        assert outbox._retry_policy.max_retries == 5
        assert outbox._retry_policy.initial_backoff_ms == 200

    @pytest.mark.asyncio
    async def test_add_entry(self):
        """Test adding entry to outbox."""
        entry = OutboxEntry(id="test-1", topic="orders", value={"id": 1})

        await self.outbox.add(entry)

        pending = await self.outbox.get_pending()
        assert len(pending) == 1
        assert pending[0].id == "test-1"

    @pytest.mark.asyncio
    async def test_add_multiple_entries(self):
        """Test adding multiple entries."""
        for i in range(5):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={"id": i})
            await self.outbox.add(entry)

        pending = await self.outbox.get_pending()
        assert len(pending) == 5

    @pytest.mark.asyncio
    async def test_get_pending_with_limit(self):
        """Test getting pending with limit."""
        for i in range(10):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={"id": i})
            await self.outbox.add(entry)

        pending = await self.outbox.get_pending(limit=5)
        assert len(pending) == 5

    @pytest.mark.asyncio
    async def test_get_pending_empty(self):
        """Test getting pending from empty outbox."""
        pending = await self.outbox.get_pending()
        assert pending == []

    @pytest.mark.asyncio
    async def test_mark_delivered(self):
        """Test marking entry as delivered."""
        entry = OutboxEntry(id="test-1", topic="orders", value={})
        await self.outbox.add(entry)

        await self.outbox.mark_delivered("test-1")

        # Entry should be removed
        pending = await self.outbox.get_pending()
        assert len(pending) == 0

        # Stats should be updated
        assert self.outbox._stats.total_sent == 1
        assert self.outbox._stats.last_sent_at is not None

    @pytest.mark.asyncio
    async def test_mark_delivered_nonexistent(self):
        """Test marking non-existent entry as delivered."""
        # Should not raise
        await self.outbox.mark_delivered("nonexistent")

        assert self.outbox._stats.total_sent == 0

    @pytest.mark.asyncio
    async def test_mark_failed_increments_attempts(self):
        """Test marking entry as failed increments attempts."""
        entry = OutboxEntry(id="test-1", topic="orders", value={})
        await self.outbox.add(entry)

        await self.outbox.mark_failed("test-1", Exception("Network error"))

        pending = await self.outbox.get_pending()
        assert len(pending) == 1
        assert pending[0].attempts == 1
        assert "Network error" in pending[0].last_error

    @pytest.mark.asyncio
    async def test_mark_failed_below_max_retries(self):
        """Test marking failed below max retries keeps entry."""
        entry = OutboxEntry(id="test-1", topic="orders", value={})
        await self.outbox.add(entry)

        # Mark failed twice (below default max of 3)
        await self.outbox.mark_failed("test-1", Exception("Error 1"))
        await self.outbox.mark_failed("test-1", Exception("Error 2"))

        pending = await self.outbox.get_pending()
        assert len(pending) == 1
        assert pending[0].attempts == 2

    @pytest.mark.asyncio
    async def test_mark_failed_exceeds_max_retries(self):
        """Test marking failed exceeding max retries removes entry."""
        entry = OutboxEntry(id="test-1", topic="orders", value={})
        await self.outbox.add(entry)

        # Mark failed 3 times (default max retries)
        await self.outbox.mark_failed("test-1", Exception("Error 1"))
        await self.outbox.mark_failed("test-1", Exception("Error 2"))
        await self.outbox.mark_failed("test-1", Exception("Error 3"))

        # Entry should be removed
        pending = await self.outbox.get_pending()
        assert len(pending) == 0

        # Stats should show failure
        assert self.outbox._stats.total_failed == 1
        assert self.outbox._stats.last_failed_at is not None

    @pytest.mark.asyncio
    async def test_mark_failed_nonexistent(self):
        """Test marking non-existent entry as failed."""
        # Should not raise
        await self.outbox.mark_failed("nonexistent", Exception())

        assert self.outbox._stats.total_failed == 0

    @pytest.mark.asyncio
    async def test_custom_max_retries(self):
        """Test custom max retries."""
        policy = RetryPolicy(max_retries=2)
        outbox = InMemoryOutbox(retry_policy=policy)

        entry = OutboxEntry(id="test-1", topic="orders", value={})
        await outbox.add(entry)

        # Mark failed twice (max is 2)
        await outbox.mark_failed("test-1", Exception("Error 1"))
        await outbox.mark_failed("test-1", Exception("Error 2"))

        # Entry should be removed
        pending = await outbox.get_pending()
        assert len(pending) == 0
        assert outbox._stats.total_failed == 1


# ============================================
# Integration Tests
# ============================================


class TestDeliveryIntegration:
    """Integration tests for delivery components."""

    @pytest.mark.asyncio
    async def test_outbox_to_dlq_flow(self):
        """Test flow from outbox to dead letter queue."""
        policy = RetryPolicy(max_retries=2)
        outbox = InMemoryOutbox(retry_policy=policy)
        dlq = InMemoryDeadLetterQueue()

        # Add entry to outbox
        entry = OutboxEntry(id="msg-1", topic="orders", value={"order_id": 123})
        await outbox.add(entry)

        # Simulate failed delivery attempts
        await outbox.mark_failed("msg-1", Exception("Connection refused"))
        await outbox.mark_failed("msg-1", Exception("Connection refused"))

        # Entry should be removed from outbox
        pending = await outbox.get_pending()
        assert len(pending) == 0

        # Add to DLQ
        entry.last_error = "Connection refused"
        entry.attempts = 2
        await dlq.add(entry, Exception("Connection refused"))

        # Verify in DLQ
        dlq_pending = await dlq.get_pending()
        assert len(dlq_pending) == 1

    @pytest.mark.asyncio
    async def test_successful_delivery_flow(self):
        """Test successful delivery flow."""
        outbox = InMemoryOutbox()

        # Add entry
        entry = OutboxEntry(id="msg-1", topic="orders", value={"order_id": 123})
        await outbox.add(entry)

        # Simulate successful delivery
        await outbox.mark_delivered("msg-1")

        # Verify removed and stats updated
        pending = await outbox.get_pending()
        assert len(pending) == 0
        assert outbox._stats.total_sent == 1

    @pytest.mark.asyncio
    async def test_retry_then_success(self):
        """Test retry followed by success."""
        outbox = InMemoryOutbox()

        entry = OutboxEntry(id="msg-1", topic="orders", value={})
        await outbox.add(entry)

        # First attempt fails
        await outbox.mark_failed("msg-1", Exception("Temporary error"))

        # Second attempt succeeds
        await outbox.mark_delivered("msg-1")

        assert outbox._stats.total_sent == 1
        assert outbox._stats.total_failed == 0


# ============================================
# Edge Cases and Error Handling
# ============================================


class TestDeliveryEdgeCases:
    """Edge case tests for delivery components."""

    @pytest.mark.asyncio
    async def test_concurrent_add_operations(self):
        """Test concurrent add operations to outbox."""
        outbox = InMemoryOutbox()

        async def add_entry(i):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={"id": i})
            await outbox.add(entry)

        # Run 10 concurrent adds
        await asyncio.gather(*[add_entry(i) for i in range(10)])

        pending = await outbox.get_pending()
        assert len(pending) == 10

    @pytest.mark.asyncio
    async def test_concurrent_mark_operations(self):
        """Test concurrent mark operations."""
        outbox = InMemoryOutbox()

        # Add entries
        for i in range(5):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={})
            await outbox.add(entry)

        async def mark_delivered(i):
            await outbox.mark_delivered(f"entry-{i}")

        # Run concurrent mark_delivered
        await asyncio.gather(*[mark_delivered(i) for i in range(5)])

        pending = await outbox.get_pending()
        assert len(pending) == 0
        assert outbox._stats.total_sent == 5

    @pytest.mark.asyncio
    async def test_dlq_concurrent_operations(self):
        """Test concurrent DLQ operations."""
        dlq = InMemoryDeadLetterQueue()

        async def add_to_dlq(i):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={})
            await dlq.add(entry, Exception(f"Error {i}"))

        # Run concurrent adds
        await asyncio.gather(*[add_to_dlq(i) for i in range(10)])

        pending = await dlq.get_pending()
        assert len(pending) == 10

    def test_retry_policy_zero_backoff(self):
        """Test retry policy with zero initial backoff."""
        policy = RetryPolicy(initial_backoff_ms=0)

        backoff = policy.get_backoff_ms(0)
        assert backoff == 0

    def test_retry_policy_large_multiplier(self):
        """Test retry policy with large multiplier."""
        policy = RetryPolicy(
            initial_backoff_ms=100, max_backoff_ms=1000000, backoff_multiplier=10.0
        )

        backoff = policy.get_backoff_ms(3)
        # 100 * 10^3 = 100000
        assert backoff == 100000

    @pytest.mark.asyncio
    async def test_outbox_entry_with_complex_value(self):
        """Test outbox with complex nested value."""
        complex_value = {
            "order": {
                "id": 123,
                "items": [
                    {"product_id": 1, "quantity": 2, "price": 10.0},
                    {"product_id": 2, "quantity": 1, "price": 25.0},
                ],
                "customer": {
                    "id": 456,
                    "name": "John Doe",
                    "address": {"street": "123 Main St", "city": "Anytown"},
                },
            },
            "metadata": {"source": "web", "timestamp": "2024-01-15T10:30:00Z"},
        }

        entry = OutboxEntry(
            id="complex-1", topic="orders", key="order-123", value=complex_value
        )

        outbox = InMemoryOutbox()
        await outbox.add(entry)

        pending = await outbox.get_pending()
        assert len(pending) == 1
        assert pending[0].value == complex_value

    @pytest.mark.asyncio
    async def test_outbox_with_binary_value(self):
        """Test outbox with binary value."""
        binary_data = b"\x00\x01\x02\x03\x04\x05"

        entry = OutboxEntry(id="binary-1", topic="binary-events", value=binary_data)

        outbox = InMemoryOutbox()
        await outbox.add(entry)

        pending = await outbox.get_pending()
        assert pending[0].value == binary_data

    @pytest.mark.asyncio
    async def test_dlq_max_size_boundary(self):
        """Test DLQ behavior at max size boundary."""
        dlq = InMemoryDeadLetterQueue(max_size=5)

        # Fill to exactly max size
        for i in range(5):
            entry = OutboxEntry(id=f"entry-{i}", topic="orders", value={})
            await dlq.add(entry, Exception())

        pending = await dlq.get_pending()
        assert len(pending) == 5

        # Add one more - should evict oldest
        entry = OutboxEntry(id="entry-5", topic="orders", value={})
        await dlq.add(entry, Exception())

        pending = await dlq.get_pending()
        assert len(pending) == 5

        ids = [e.id for e in pending]
        assert "entry-0" not in ids
        assert "entry-5" in ids
