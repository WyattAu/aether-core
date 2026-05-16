"""
Tests for Aether SDK Backpressure Module

Tests for backpressure handling strategies and controls.
"""

import asyncio
from typing import Any
from unittest.mock import Mock

import pytest

from aether_sdk.streaming.backpressure import (
    BackpressureController,
    BackpressureError,
    BackpressureStats,
    BufferFullError,
    MultiLevelBackpressure,
    RateBasedBackpressure,
)
from aether_sdk.streaming.types import (
    BackpressureConfig,
    BackpressureStrategy,
    StreamEvent,
    Timestamp,
)

# ============================================
# Helper Functions
# ============================================


def create_event(key: str, value: Any = None) -> StreamEvent:
    """Create a test stream event."""
    return StreamEvent(
        key=key,
        value=value or {"data": key},
        timestamp=Timestamp.now(),
        event_type="test",
    )


# ============================================
# Fixtures
# ============================================


@pytest.fixture
def buffer_config():
    """Create a BUFFER strategy config."""
    return BackpressureConfig(
        strategy=BackpressureStrategy.BUFFER,
        buffer_size=100,
        high_watermark=0.8,
        low_watermark=0.2,
    )


@pytest.fixture
def drop_config():
    """Create a DROP strategy config."""
    return BackpressureConfig(
        strategy=BackpressureStrategy.DROP,
        buffer_size=100,
    )


@pytest.fixture
def fail_config():
    """Create a FAIL strategy config."""
    return BackpressureConfig(
        strategy=BackpressureStrategy.FAIL,
        buffer_size=100,
    )


@pytest.fixture
def latest_config():
    """Create a LATEST strategy config."""
    return BackpressureConfig(
        strategy=BackpressureStrategy.LATEST,
        buffer_size=100,
    )


@pytest.fixture
def buffer_controller(buffer_config):
    """Create a backpressure controller with BUFFER strategy."""
    return BackpressureController(buffer_config)


@pytest.fixture
def drop_controller(drop_config):
    """Create a backpressure controller with DROP strategy."""
    return BackpressureController(drop_config)


@pytest.fixture
def fail_controller(fail_config):
    """Create a backpressure controller with FAIL strategy."""
    return BackpressureController(fail_config)


@pytest.fixture
def latest_controller(latest_config):
    """Create a backpressure controller with LATEST strategy."""
    return BackpressureController(latest_config)


# ============================================
# BackpressureConfig Tests
# ============================================


class TestBackpressureConfig:
    """Tests for BackpressureConfig."""

    def test_default_config(self):
        """Test default configuration values."""
        config = BackpressureConfig()
        assert config.strategy == BackpressureStrategy.BUFFER
        assert config.buffer_size == 10000
        assert config.high_watermark == 0.9
        assert config.low_watermark == 0.5
        assert config.on_overflow is None
        assert config.on_resume is None

    def test_custom_config(self, buffer_config):
        """Test custom configuration values."""
        assert buffer_config.strategy == BackpressureStrategy.BUFFER
        assert buffer_config.buffer_size == 100
        assert buffer_config.high_watermark == 0.8
        assert buffer_config.low_watermark == 0.2

    def test_config_with_callbacks(self):
        """Test configuration with callbacks."""
        overflow_cb = Mock()
        resume_cb = Mock()
        config = BackpressureConfig(
            strategy=BackpressureStrategy.BUFFER,
            buffer_size=100,
            on_overflow=overflow_cb,
            on_resume=resume_cb,
        )
        assert config.on_overflow == overflow_cb
        assert config.on_resume == resume_cb


# ============================================
# BackpressureStats Tests
# ============================================


class TestBackpressureStats:
    """Tests for BackpressureStats."""

    def test_default_stats(self):
        """Test default statistics."""
        stats = BackpressureStats()
        assert stats.total_events == 0
        assert stats.buffered_events == 0
        assert stats.dropped_events == 0
        assert stats.rejected_events == 0
        assert stats.overflow_count == 0
        assert stats.resume_count == 0
        assert stats.current_buffer_size == 0
        assert stats.high_watermark_reached is False

    def test_stats_to_dict(self):
        """Test stats serialization to dictionary."""
        stats = BackpressureStats(
            total_events=100,
            buffered_events=50,
            dropped_events=10,
            rejected_events=5,
            overflow_count=2,
            resume_count=1,
            current_buffer_size=50,
            high_watermark_reached=True,
        )
        data = stats.to_dict()
        assert data["total_events"] == 100
        assert data["buffered_events"] == 50
        assert data["dropped_events"] == 10
        assert data["rejected_events"] == 5
        assert data["overflow_count"] == 2
        assert data["resume_count"] == 1
        assert data["current_buffer_size"] == 50
        assert data["high_watermark_reached"] is True


# ============================================
# Error Classes Tests
# ============================================


class TestBackpressureErrors:
    """Tests for backpressure error classes."""

    def test_backpressure_error(self):
        """Test base BackpressureError."""
        error = BackpressureError("Test error")
        assert str(error) == "Test error"
        assert isinstance(error, Exception)

    def test_buffer_full_error(self):
        """Test BufferFullError."""
        error = BufferFullError(buffer_size=100)
        assert error.buffer_size == 100
        assert error.event is None
        assert "Buffer full" in str(error)
        assert "100" in str(error)

    def test_buffer_full_error_with_event(self):
        """Test BufferFullError with event."""
        event = create_event("test-1")
        error = BufferFullError(buffer_size=100, event=event)
        assert error.buffer_size == 100
        assert error.event == event
        assert "Buffer full" in str(error)


# ============================================
# BackpressureController Tests - BUFFER Strategy
# ============================================


class TestBackpressureControllerBufferStrategy:
    """Tests for BackpressureController with BUFFER strategy."""

    def test_initial_state(self, buffer_controller):
        """Test initial controller state."""
        assert buffer_controller.size() == 0
        assert buffer_controller.is_empty()
        assert not buffer_controller.is_full()

        stats = buffer_controller.stats
        assert stats.total_events == 0
        assert stats.buffered_events == 0

    def test_push_and_pop(self, buffer_controller):
        """Test pushing and popping events."""
        event = create_event("event-1")

        # Push event
        result = buffer_controller.try_push(event)
        assert result is True
        assert buffer_controller.size() == 1
        assert not buffer_controller.is_empty()

        # Pop event
        popped = buffer_controller.pop()
        assert popped is not None
        assert popped.key == "event-1"
        assert buffer_controller.is_empty()

    def test_fill_buffer(self, buffer_controller):
        """Test filling buffer to capacity."""
        # Fill buffer to capacity
        for i in range(100):
            event = create_event(f"event-{i}")
            result = buffer_controller.try_push(event)
            assert result is True

        # Buffer should be full
        assert buffer_controller.is_full()
        assert buffer_controller.size() == 100

        # Next push should fail
        event = create_event("event-100")
        result = buffer_controller.try_push(event)
        assert result is False

        # Stats should reflect the rejection
        stats = buffer_controller.stats
        assert stats.total_events == 101
        assert stats.rejected_events == 1

    def test_peek(self, buffer_controller):
        """Test peeking at events."""
        event1 = create_event("event-1")
        event2 = create_event("event-2")

        buffer_controller.try_push(event1)
        buffer_controller.try_push(event2)

        # Peek should return first event without removing
        peeked = buffer_controller.peek()
        assert peeked is not None
        assert peeked.key == "event-1"
        assert buffer_controller.size() == 2

    def test_clear(self, buffer_controller):
        """Test clearing the buffer."""
        for i in range(50):
            buffer_controller.try_push(create_event(f"event-{i}"))

        assert buffer_controller.size() == 50

        cleared = buffer_controller.clear()
        assert cleared == 50
        assert buffer_controller.is_empty()

    def test_high_watermark_detection(self, buffer_controller):
        """Test high watermark detection."""
        # Fill to 80% (high watermark)
        for i in range(80):
            buffer_controller.try_push(create_event(f"event-{i}"))

        # Check if overloaded
        assert buffer_controller.is_overloaded is True

        # Stats should show high watermark reached
        stats = buffer_controller.stats
        assert stats.high_watermark_reached is True
        assert stats.overflow_count == 1

    def test_low_watermark_recovery(self, buffer_controller):
        """Test recovery when buffer drains below low watermark."""
        # Fill above high watermark
        for i in range(85):
            buffer_controller.try_push(create_event(f"event-{i}"))

        assert buffer_controller.is_overloaded is True
        assert buffer_controller.stats.high_watermark_reached is True

        # Drain below low watermark (20% of 100 = 20)
        for i in range(65):
            buffer_controller.pop()

        assert buffer_controller.is_recovered is True
        assert buffer_controller.stats.high_watermark_reached is False
        assert buffer_controller.stats.resume_count == 1

    def test_overflow_callback(self, buffer_config):
        """Test overflow callback is called."""
        callback_called = []

        def on_overflow():
            callback_called.append("overflow")

        buffer_config.on_overflow = on_overflow
        controller = BackpressureController(buffer_config)

        # Fill to high watermark
        for i in range(81):
            controller.try_push(create_event(f"event-{i}"))

        assert len(callback_called) == 1

    def test_resume_callback(self, buffer_config):
        """Test resume callback is called."""
        callback_called = []

        def on_resume():
            callback_called.append("resume")

        buffer_config.on_resume = on_resume
        controller = BackpressureController(buffer_config)

        # Fill above high watermark
        for i in range(85):
            controller.try_push(create_event(f"event-{i}"))

        # Drain below low watermark
        for i in range(65):
            controller.pop()

        assert len(callback_called) == 1

    def test_set_overflow_callback(self, buffer_controller):
        """Test setting overflow callback after creation."""
        callback_called = []

        def callback():
            callback_called.append(True)

        buffer_controller.set_overflow_callback(callback)

        # Fill to high watermark
        for i in range(81):
            buffer_controller.try_push(create_event(f"event-{i}"))

        assert len(callback_called) == 1

    def test_set_resume_callback(self, buffer_controller):
        """Test setting resume callback after creation."""
        callback_called = []

        def callback():
            callback_called.append(True)

        buffer_controller.set_resume_callback(callback)

        # Fill and drain
        for i in range(85):
            buffer_controller.try_push(create_event(f"event-{i}"))
        for i in range(65):
            buffer_controller.pop()

        assert len(callback_called) == 1

    def test_reset_stats(self, buffer_controller):
        """Test resetting statistics."""
        # Add some events
        for i in range(50):
            buffer_controller.try_push(create_event(f"event-{i}"))

        # Reset stats
        buffer_controller.reset_stats()

        stats = buffer_controller.stats
        assert stats.total_events == 0
        assert stats.buffered_events == 50  # Current buffer preserved
        assert stats.dropped_events == 0
        assert stats.overflow_count == 0


# ============================================
# BackpressureController Tests - DROP Strategy
# ============================================


class TestBackpressureControllerDropStrategy:
    """Tests for BackpressureController with DROP strategy."""

    def test_drop_when_full(self, drop_controller):
        """Test that events are dropped when buffer is full."""
        # Fill buffer
        for i in range(100):
            result = drop_controller.try_push(create_event(f"event-{i}"))
            assert result is True

        # Next event should be dropped
        result = drop_controller.try_push(create_event("event-100"))
        assert result is False

        # Stats should show dropped event
        stats = drop_controller.stats
        assert stats.dropped_events == 1

    def test_drop_never_raises(self, drop_controller):
        """Test that DROP strategy never raises exception."""
        # Fill buffer
        for i in range(100):
            drop_controller.try_push(create_event(f"event-{i}"))

        # This should not raise
        result = drop_controller.try_push(create_event("event-100"))
        assert result is False

    def test_buffer_remains_consistent(self, drop_controller):
        """Test buffer remains at capacity when dropping."""
        # Fill buffer
        for i in range(100):
            drop_controller.try_push(create_event(f"event-{i}"))

        # Try to add more
        for i in range(50):
            drop_controller.try_push(create_event(f"event-{100 + i}"))

        # Buffer should still be at capacity with original events
        assert drop_controller.size() == 100


# ============================================
# BackpressureController Tests - FAIL Strategy
# ============================================


class TestBackpressureControllerFailStrategy:
    """Tests for BackpressureController with FAIL strategy."""

    def test_fail_raises_exception(self, fail_controller):
        """Test that FAIL strategy raises BufferFullError when full."""
        # Fill buffer
        for i in range(100):
            fail_controller.try_push(create_event(f"event-{i}"))

        # Next event should raise
        with pytest.raises(BufferFullError) as exc_info:
            fail_controller.try_push(create_event("event-100"))

        assert exc_info.value.buffer_size == 100

    def test_fail_includes_event_in_error(self, fail_controller):
        """Test that BufferFullError includes the rejected event."""
        # Fill buffer
        for i in range(100):
            fail_controller.try_push(create_event(f"event-{i}"))

        event = create_event("event-100")
        with pytest.raises(BufferFullError) as exc_info:
            fail_controller.try_push(event)

        assert exc_info.value.event == event

    def test_rejected_count_increments(self, fail_controller):
        """Test rejected events count increments on failure."""
        # Fill buffer
        for i in range(100):
            fail_controller.try_push(create_event(f"event-{i}"))

        # Try to push and catch exception
        try:
            fail_controller.try_push(create_event("event-100"))
        except BufferFullError:
            pass

        stats = fail_controller.stats
        assert stats.rejected_events == 1


# ============================================
# BackpressureController Tests - LATEST Strategy
# ============================================


class TestBackpressureControllerLatestStrategy:
    """Tests for BackpressureController with LATEST strategy."""

    def test_latest_accepts_when_full(self, latest_controller):
        """Test that LATEST strategy accepts events when full."""
        # Fill buffer
        for i in range(100):
            latest_controller.try_push(create_event(f"event-{i}"))

        # Should still accept new event
        result = latest_controller.try_push(create_event("event-100"))
        assert result is True

    def test_latest_replaces_oldest(self, latest_controller):
        """Test that LATEST strategy replaces oldest event."""
        # Fill buffer
        for i in range(100):
            latest_controller.try_push(create_event(f"event-{i}"))

        # Add new event
        latest_controller.try_push(create_event("event-100"))

        # First event should now be event-1 (event-0 was replaced)
        peeked = latest_controller.peek()
        assert peeked.key == "event-1"

    def test_latest_maintains_capacity(self, latest_controller):
        """Test that LATEST strategy maintains buffer capacity."""
        # Fill buffer
        for i in range(100):
            latest_controller.try_push(create_event(f"event-{i}"))

        # Add more events
        for i in range(50):
            latest_controller.try_push(create_event(f"event-{100 + i}"))

        # Should still be at capacity
        assert latest_controller.size() == 100

    def test_latest_increments_dropped_count(self, latest_controller):
        """Test that replaced events count as dropped."""
        # Fill buffer
        for i in range(100):
            latest_controller.try_push(create_event(f"event-{i}"))

        # Add new event (should replace one)
        latest_controller.try_push(create_event("event-100"))

        stats = latest_controller.stats
        assert stats.dropped_events == 1


# ============================================
# MultiLevelBackpressure Tests
# ============================================


class TestMultiLevelBackpressure:
    """Tests for MultiLevelBackpressure with priority queues."""

    def test_initial_state(self):
        """Test initial state is empty."""
        bp = MultiLevelBackpressure(buffer_size=100)
        assert bp.is_empty()
        assert bp.size() == 0

    def test_push_and_pop_normal_priority(self):
        """Test pushing and popping with normal priority."""
        bp = MultiLevelBackpressure(buffer_size=100)

        event = create_event("event-1")
        result = bp.push(event, MultiLevelBackpressure.Priority.NORMAL)
        assert result is True
        assert bp.size() == 1

        popped = bp.pop()
        assert popped is not None
        assert popped.key == "event-1"

    def test_priority_ordering(self):
        """Test that high priority events are processed first."""
        bp = MultiLevelBackpressure(buffer_size=100)

        # Add events in different priority order
        bp.push(create_event("low"), MultiLevelBackpressure.Priority.LOW)
        bp.push(create_event("high"), MultiLevelBackpressure.Priority.HIGH)
        bp.push(create_event("normal"), MultiLevelBackpressure.Priority.NORMAL)

        # Should pop in priority order
        popped = bp.pop()
        assert popped is not None and popped.key == "high"
        popped = bp.pop()
        assert popped is not None and popped.key == "normal"
        popped = bp.pop()
        assert popped is not None and popped.key == "low"

    def test_drop_low_priority_when_full(self):
        """Test that low priority events are dropped first when full."""
        bp = MultiLevelBackpressure(buffer_size=3)

        # Fill with high and normal priority
        bp.push(create_event("high-1"), MultiLevelBackpressure.Priority.HIGH)
        bp.push(create_event("normal-1"), MultiLevelBackpressure.Priority.NORMAL)
        bp.push(create_event("low-1"), MultiLevelBackpressure.Priority.LOW)

        # Add another low priority - should drop the existing low
        result = bp.push(create_event("low-2"), MultiLevelBackpressure.Priority.LOW)
        assert result is True

        # Verify buffer size maintained at 3
        assert bp.size() == 3

        # Verify low-2 is present (replaced low-1)
        events = []
        while not bp.is_empty():
            evt = bp.pop()
            if evt:
                events.append(evt.key)
        assert "low-2" in events
        assert "low-1" not in events

    def test_reject_low_priority_when_no_lower_to_drop(self):
        """Test that new low priority is rejected if buffer is full of higher priority."""
        bp = MultiLevelBackpressure(buffer_size=2)

        # Fill with high priority
        bp.push(create_event("high-1"), MultiLevelBackpressure.Priority.HIGH)
        bp.push(create_event("high-2"), MultiLevelBackpressure.Priority.HIGH)

        # Try to add low priority - should be rejected
        result = bp.push(create_event("low-1"), MultiLevelBackpressure.Priority.LOW)
        assert result is False

        # Verify buffer still contains only high priority events
        assert bp.size() == 2
        events = []
        while not bp.is_empty():
            evt = bp.pop()
            if evt:
                events.append(evt.key)
        assert all(e.startswith("high") for e in events)


# ============================================
# RateBasedBackpressure Tests
# ============================================


class TestRateBasedBackpressure:
    """Tests for RateBasedBackpressure."""

    @pytest.mark.asyncio
    async def test_initial_state(self):
        """Test initial state allows processing."""
        rbp = RateBasedBackpressure(max_rate=100)
        assert rbp.is_backpressure_active is False
        assert rbp.current_rate == 0.0

    @pytest.mark.asyncio
    async def test_allows_within_rate(self):
        """Test that events within rate limit are allowed."""
        rbp = RateBasedBackpressure(max_rate=10, window_size=1.0)

        # Should allow several events
        for i in range(5):
            result = await rbp.try_acquire()
            assert result is True

    @pytest.mark.asyncio
    async def test_blocks_when_rate_exceeded(self):
        """Test that backpressure activates when rate exceeded."""
        rbp = RateBasedBackpressure(max_rate=5, window_size=1.0, cooldown=0.1)

        # Fill up to rate limit
        for i in range(6):
            await rbp.try_acquire()

        # Next should be blocked
        result = await rbp.try_acquire()
        assert result is False
        assert rbp.is_backpressure_active is True

    @pytest.mark.asyncio
    async def test_current_rate_calculation(self):
        """Test current rate calculation."""
        rbp = RateBasedBackpressure(max_rate=100, window_size=1.0)

        # Acquire 5 permits
        for i in range(5):
            await rbp.try_acquire()

        # Rate should be around 5 events per second
        rate = rbp.current_rate
        assert 4.0 <= rate <= 6.0

    @pytest.mark.asyncio
    async def test_reset(self):
        """Test resetting the rate tracker."""
        rbp = RateBasedBackpressure(max_rate=5, window_size=1.0)

        # Fill up
        for i in range(6):
            await rbp.try_acquire()

        # Should be blocked
        assert rbp.is_backpressure_active is True

        # Reset
        rbp.reset()

        assert rbp.is_backpressure_active is False
        assert rbp.current_rate == 0.0

    @pytest.mark.asyncio
    async def test_cooldown_expires(self):
        """Test that backpressure deactivates after cooldown."""
        rbp = RateBasedBackpressure(max_rate=5, window_size=0.5, cooldown=0.1)

        # Fill up to trigger backpressure
        for i in range(10):
            await rbp.try_acquire()

        assert rbp.is_backpressure_active is True

        # Wait for cooldown to expire and window to slide (need to wait longer than window_size)
        await asyncio.sleep(0.6)

        # Should be able to acquire again since window has completely reset
        result = await rbp.try_acquire()
        assert result is True
        assert rbp.is_backpressure_active is False


# ============================================
# Thread Safety Tests
# ============================================


class TestThreadSafety:
    """Tests for thread safety of backpressure controllers."""

    def test_concurrent_push_buffer(self, buffer_controller):
        """Test concurrent pushes with BUFFER strategy."""
        import threading

        errors = []

        def push_events(start, count):
            try:
                for i in range(start, start + count):
                    buffer_controller.try_push(create_event(f"event-{i}"))
            except Exception as e:
                errors.append(e)

        threads = [
            threading.Thread(target=push_events, args=(i * 100, 100)) for i in range(4)
        ]

        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
        assert buffer_controller.size() == 100  # Should be at capacity

    def test_concurrent_push_pop(self, buffer_controller):
        """Test concurrent push and pop operations."""
        import threading

        pushed = []
        popped = []
        errors = []

        def push_events():
            try:
                for i in range(50):
                    event = create_event(f"event-{i}")
                    if buffer_controller.try_push(event):
                        pushed.append(event.key)
            except Exception as e:
                errors.append(e)

        def pop_events():
            try:
                for i in range(50):
                    event = buffer_controller.pop()
                    if event:
                        popped.append(event.key)
            except Exception as e:
                errors.append(e)

        threads = [
            threading.Thread(target=push_events),
            threading.Thread(target=pop_events),
        ]

        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
