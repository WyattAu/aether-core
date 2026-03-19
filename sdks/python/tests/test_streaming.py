"""
Tests for the Streaming Module

"""

import asyncio
import pytest
from datetime import datetime, timedelta
from typing import Any, Dict, List, Optional
from dataclasses import dataclass

import sys
import os

# Add the SDK to the path
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from aether_sdk.streaming import (
    StreamActor,
    StreamEvent,
    Duration,
    Timestamp,
    WindowType,
    WindowSpec,
    WindowInfo,
    TumblingWindow,
    SlidingWindow,
    SessionWindow,
    BackpressureController,
    BackpressureConfig,
    BackpressureStrategy,
    Watermark,
    PaneInfo,
)


# ============================================
# Test Fixtures
# ============================================

@pytest.fixture
def event():
    """Create a test event."""
    return StreamEvent.create(
        key="test-key",
        value={"data": "test"},
        timestamp=Timestamp.now(),
    )


@pytest.fixture
def duration():
    """Create a test duration."""
    return Duration.from_seconds(1)


@pytest.fixture
def timestamp():
    """Create a test timestamp."""
    return Timestamp.now()


@pytest.fixture
def window_spec():
    """Create a test window spec."""
    return WindowSpec(
        type=WindowType.TUMBLING,
        size=Duration.from_seconds(60),
    )


# ============================================
# Duration Tests
# ============================================

class TestDuration:
    def test_duration_from_seconds(self):
        d = Duration.from_seconds(1)
        assert d.to_seconds() == 1.0
        assert d.ms == 1000
    
    def test_duration_from_minutes(self):
        d = Duration.from_minutes(1)
        assert d.to_seconds() == 60.0
        assert d.ms == 60000
    
    def test_duration_from_hours(self):
        d = Duration.from_hours(1)
        assert d.to_seconds() == 3600.0
        assert d.ms == 3600000
    
    def test_duration_from_millis(self):
        d = Duration.from_millis(100)
        assert d.to_seconds() == 0.1
        assert d.ms == 100
    
    def test_duration_from_timedelta(self):
        td = timedelta(seconds=1, milliseconds=500)  # 1.5 seconds
        d = Duration.from_timedelta(td)
        assert d.to_seconds() == 1.5
        assert d.ms == 1500
    
    def test_duration_addition(self):
        d1 = Duration.from_seconds(5)
        d2 = Duration.from_seconds(10)
        result = d1 + d2
        assert result.to_seconds() == 15.0
    
    def test_duration_multiplication(self):
        d = Duration.from_seconds(5)
        result = d * 3
        assert result.to_seconds() == 15.0


# ============================================
# Timestamp Tests
# ============================================
class TestTimestamp:
    def test_timestamp_now(self):
        ts = Timestamp.now()
        assert isinstance(ts, Timestamp)
        assert ts.milliseconds > 0
    
    def test_timestamp_from_datetime(self):
        dt = datetime.now()
        ts = Timestamp.from_datetime(dt)
        assert isinstance(ts, Timestamp)
    
    def test_timestamp_from_seconds(self):
        ts = Timestamp.from_seconds(100.5)
        assert ts.milliseconds == 100500
    
    def test_timestamp_comparison(self):
        ts1 = Timestamp.from_seconds(100)
        ts2 = Timestamp.from_seconds(200)
        assert ts1 < ts2
        assert ts2 > ts1
        assert ts1 <= ts2
        assert ts2 >= ts1
    
    def test_timestamp_subtraction(self):
        ts1 = Timestamp.from_seconds(200)
        ts2 = Timestamp.from_seconds(100)
        diff = ts1 - ts2
        assert diff.ms == 100000
    
    def test_timestamp_addition(self):
        ts = Timestamp.from_seconds(100)
        d = Duration.from_seconds(50)
        result = ts + d
        assert result.milliseconds == 100050


# ============================================
# StreamEvent Tests
# ============================================
class TestStreamEvent:
    def test_stream_event_create(self):
        ev = StreamEvent.create(
            key="test-key",
            value={"count": 1},
        )
        assert ev.key == "test-key"
        assert ev.value == {"count": 1}
        assert isinstance(ev.timestamp, Timestamp)
    
    def test_stream_event_with_headers(self):
        ev = StreamEvent.create(
            key="test-key",
            value="test-value",
            headers={"source": "test-source"},
        )
        assert ev.headers == {"source": "test-source"}


# ============================================
# BackpressureController Tests
# ============================================
class TestBackpressureController:
    def test_backpressure_controller_buffer(self):
        config = BackpressureConfig(
            strategy=BackpressureStrategy.BUFFER,
            buffer_size=100,
        )
        controller = BackpressureController(config)
        
        # Push events
        for i in range(10):
            event = StreamEvent.create(key=f"key-{i}", value=i)
            assert controller.try_push(event)
        
        assert controller.size() == 10
        
        # Pop events
        for i in range(10):
            event = controller.pop()
            assert event is not None
            assert event.key == f"key-{i}"
        
        assert controller.is_empty()
    
    def test_backpressure_controller_drop(self):
        config = BackpressureConfig(
            strategy=BackpressureStrategy.DROP,
            buffer_size=5,
        )
        controller = BackpressureController(config)
        
        # Fill buffer
        for i in range(5):
            event = StreamEvent.create(key=f"key-{i}", value=i)
            controller.try_push(event)
        
        # Try to push when full - should be dropped
        event = StreamEvent.create(key="key-overflow", value="overflow")
        result = controller.try_push(event)
        assert result is False  # Dropped
        
        assert controller.stats.dropped_events == 1
    
    def test_backpressure_controller_fail(self):
        from aether_sdk.streaming.backpressure import BufferFullError
        
        config = BackpressureConfig(
            strategy=BackpressureStrategy.FAIL,
            buffer_size=5,
        )
        controller = BackpressureController(config)
        
        # Fill buffer
        for i in range(5):
            event = StreamEvent.create(key=f"key-{i}", value=i)
            controller.try_push(event)
        
        # Try to push when full - should raise error
        event = StreamEvent.create(key="key-overflow", value="overflow")
        with pytest.raises(BufferFullError):
            controller.try_push(event)
    
    def test_backpressure_watermarks(self):
        config = BackpressureConfig(
            strategy=BackpressureStrategy.BUFFER,
            buffer_size=100,
            high_watermark=0.8,
            low_watermark=0.4,
        )
        controller = BackpressureController(config)
        
        # Fill to 50% (below low watermark)
        for i in range(50):
            event = StreamEvent.create(key=f"key-{i}", value=i)
            controller.try_push(event)
        
        assert not controller.is_overloaded
        assert controller.is_recovered
        
        # Fill to 80% (above high watermark)
        for i in range(30):
            event = StreamEvent.create(key=f"key-{i}", value=i)
            controller.try_push(event)
        
        assert controller.is_overloaded
        
        # Drain to 40% (below low watermark)
        for i in range(40):
            controller.pop()
        
        assert controller.is_recovered
        assert not controller.is_overloaded


# ============================================
# Window Tests
# ============================================
class TestWindowing:
    def test_tumbling_window(self):
        results = []
        
        def handler(events: List[StreamEvent], info: WindowInfo) -> int:
            return sum(e.value for e in events)
        
        window = TumblingWindow(
            size=Duration.from_seconds(10),
            handler=handler,
        )
        
        # Process events within window
        for i in range(5):
            event = StreamEvent.create(
                key="key-a",
                value=i,
                timestamp=Timestamp.from_seconds(i * 2),  # 0, 2, 4, 6, 8 seconds
            )
            window.process(event, "key-a")
        
        # Advance watermark past window end
        watermark_ts = Timestamp.from_seconds(11)
        results = window.advance_watermark(watermark_ts)
        
        # Should have one result with sum 0+1+2+3+4 = 10
        assert len(results) == 1
        assert results[0] == 10
    
    def test_session_window(self):
        results = []
        
        def handler(events: List[StreamEvent], info: WindowInfo) -> int:
            return len(events)
        
        window = SessionWindow(
            gap=Duration.from_seconds(5),
            handler=handler,
        )
        
        # First event starts session
        event1 = StreamEvent.create(
            key="key-a",
            value=1,
            timestamp=Timestamp.from_seconds(0),
        )
        window.process(event1, "key-a")
        
        # Event within gap - extends session
        event2 = StreamEvent.create(
            key="key-a",
            value=1,
            timestamp=Timestamp.from_seconds(3),
        )
        window.process(event2, "key-a")
        
        # Event outside gap - new session
        event3 = StreamEvent.create(
            key="key-a",
            value=1,
            timestamp=Timestamp.from_seconds(10),
        )
        window.process(event3, "key-a")
        
        # Advance watermark
        results = window.advance_watermark(Timestamp.from_seconds(20))
        
        # Should have fired sessions
        assert len(results) >= 1


# ============================================
# StreamActor Tests
# ============================================
class TestStreamActor:
    @pytest.mark.asyncio
    async def test_stream_actor_basic(self):
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            def __init__(self):
                super().__init__()
                self.processed = []
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                self.processed.append(event)
        
        actor = TestProcessor()
        
        # Process an event
        event = StreamEvent.create(key="test", value=42, timestamp=Timestamp.now())
        await actor.process_event(event)
        
        assert len(actor.processed) == 1
        assert actor.processed[0].value == 42
    
    @pytest.mark.asyncio
    async def test_stream_actor_state(self):
        class StatefulProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "stateful_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                count = await self.get_state("count", 0)
                await self.set_state("count", count + 1)
        
        actor = StatefulProcessor()
        
        # Process events
        for i in range(5):
            event = StreamEvent.create(key="test", value=i, timestamp=Timestamp.now())
            await actor.process_event(event)
        
        # Check state
        count = await actor.get_state("count")
        assert count == 5


if __name__ == "__main__":
    unittest.main()
