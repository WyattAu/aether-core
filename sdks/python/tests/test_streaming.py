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
        ts = Timestamp.from_seconds(100)  # 100000 ms
        d = Duration.from_seconds(50)     # 50000 ms
        result = ts + d
        assert result.milliseconds == 150000  # 100000 + 50000


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
        
        # Fill to 50% (between low and high watermark)
        for i in range(50):
            event = StreamEvent.create(key=f"key-{i}", value=i)
            controller.try_push(event)
        
        assert not controller.is_overloaded  # 50% < 80% high watermark
        assert not controller.is_recovered    # 50% > 40% low watermark
        
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


# ============================================
# StreamingStateHandle Tests
# ============================================
class TestStreamingStateHandle:
    """Tests for StreamingStateHandle state operations."""
    
    @pytest.mark.asyncio
    async def test_get_set_value(self):
        """Test get/set value state."""
        class TestActor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_actor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestActor()
        
        # Get with default
        value = await actor.get_state("test_key", "default")
        assert value == "default"
        
        # Set and get
        await actor.set_state("test_key", "test_value")
        value = await actor.get_state("test_key")
        assert value == "test_value"
    
    @pytest.mark.asyncio
    async def test_list_state(self):
        """Test list state operations."""
        class TestActor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_actor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestActor()
        
        # Get empty list
        lst = await actor.get_list_state("my_list")
        assert lst == []
        
        # Append items
        await actor.update_list_state("my_list", "item1")
        await actor.update_list_state("my_list", "item2")
        
        lst = await actor.get_list_state("my_list")
        assert lst == ["item1", "item2"]
    
    @pytest.mark.asyncio
    async def test_map_state(self):
        """Test map state operations."""
        class TestActor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_actor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestActor()
        
        # Get empty map
        m = await actor.get_map_state("my_map")
        assert m == {}
        
        # Put values
        await actor.update_map_state("my_map", "key1", "value1")
        await actor.update_map_state("my_map", "key2", "value2")
        
        m = await actor.get_map_state("my_map")
        assert m == {"key1": "value1", "key2": "value2"}


# ============================================
# StreamActor Message Handling Tests
# ============================================
class TestStreamActorMessages:
    """Tests for StreamActor message handling."""
    
    @pytest.mark.asyncio
    async def test_handle_stream_event_message(self):
        """Test handling STREAM_EVENT messages."""
        from aether_sdk.messaging import Message, MessageType
        
        processed = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                processed.append(event.value)
        
        actor = TestProcessor()
        
        # Send stream event message
        event = StreamEvent.create(key="test", value=42, timestamp=Timestamp.now())
        message = Message(
            type=MessageType.STREAM_EVENT,
            payload=event,
        )
        
        await actor.handle_message("sender", message)
        
        # Event should have been processed
        assert 42 in processed
    
    @pytest.mark.asyncio
    async def test_handle_dict_event_message(self):
        """Test handling STREAM_EVENT messages with dict payload."""
        from aether_sdk.messaging import Message, MessageType
        
        processed = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                processed.append(event.value)
        
        actor = TestProcessor()
        
        # Send dict event message
        event = StreamEvent.create(key="test", value=123, timestamp=Timestamp.now())
        message = Message(
            type=MessageType.STREAM_EVENT,
            payload={
                "key": "test",
                "value": 123,
                "timestamp": event.timestamp.milliseconds,
                "headers": {},
            },
        )
        
        await actor.handle_message("sender", message)
        
        # Event should have been processed
        assert 123 in processed
    
    @pytest.mark.asyncio
    async def test_handle_watermark_message(self):
        """Test handling WATERMARK messages."""
        from aether_sdk.messaging import Message, MessageType
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Send watermark message
        watermark = Watermark(
            timestamp=Timestamp.from_seconds(100),
            stream_id="test-stream",
        )
        message = Message(
            type=MessageType.WATERMARK,
            payload=watermark,
        )
        
        await actor.handle_message("sender", message)
        
        # Watermark should be updated
        assert actor.get_watermark("test-stream") is not None
        assert actor.get_watermark("test-stream").milliseconds == 100000
    
    @pytest.mark.asyncio
    async def test_handle_dict_watermark_message(self):
        """Test handling WATERMARK messages with dict payload."""
        from aether_sdk.messaging import Message, MessageType
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Send dict watermark message
        message = Message(
            type=MessageType.WATERMARK,
            payload={
                "timestamp": 200000,
                "stream_id": "test-stream-2",
                "partition": 0,
            },
        )
        
        await actor.handle_message("sender", message)
        
        # Watermark should be updated
        wm = actor.get_watermark("test-stream-2")
        assert wm is not None
        assert wm.milliseconds == 200000


# ============================================
# StreamActor Watermark Tests
# ============================================
class TestStreamActorWatermarks:
    """Tests for StreamActor watermark management."""
    
    @pytest.mark.asyncio
    async def test_advance_watermark(self):
        """Test advancing watermarks."""
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Advance watermark
        watermark = Watermark(
            timestamp=Timestamp.from_seconds(100),
            stream_id="stream-1",
        )
        await actor.advance_watermark(watermark)
        
        assert actor.get_watermark("stream-1").milliseconds == 100000
        
        # Advance to later time
        watermark2 = Watermark(
            timestamp=Timestamp.from_seconds(200),
            stream_id="stream-1",
        )
        await actor.advance_watermark(watermark2)
        
        assert actor.get_watermark("stream-1").milliseconds == 200000
    
    @pytest.mark.asyncio
    async def test_watermark_does_not_go_backwards(self):
        """Test that watermark doesn't go backwards."""
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Advance to time 100
        await actor.advance_watermark(Watermark(
            timestamp=Timestamp.from_seconds(100),
            stream_id="stream-1",
        ))
        
        # Try to advance to earlier time
        await actor.advance_watermark(Watermark(
            timestamp=Timestamp.from_seconds(50),
            stream_id="stream-1",
        ))
        
        # Should still be at 100
        assert actor.get_watermark("stream-1").milliseconds == 100000


# ============================================
# StreamActor Emit Tests
# ============================================
class TestStreamActorEmit:
    """Tests for StreamActor emit operations."""
    
    @pytest.mark.asyncio
    async def test_emit(self):
        """Test emitting values."""
        emitted = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                await self.emit("output", event.value * 2)
        
        actor = TestProcessor()
        actor.register_output_handler("output", lambda e: emitted.append(e))
        
        event = StreamEvent.create(key="test", value=21, timestamp=Timestamp.now())
        await actor.process_event(event)
        
        assert len(emitted) == 1
        assert emitted[0].value == 42
    
    @pytest.mark.asyncio
    async def test_emit_with_timestamp(self):
        """Test emitting values with specific timestamp."""
        emitted = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                await self.emit_with_timestamp(
                    "output",
                    event.value,
                    Timestamp.from_seconds(1000),
                )
        
        actor = TestProcessor()
        actor.register_output_handler("output", lambda e: emitted.append(e))
        
        event = StreamEvent.create(key="test", value=42, timestamp=Timestamp.now())
        await actor.process_event(event)
        
        assert emitted[0].timestamp.milliseconds == 1000000
    
    @pytest.mark.asyncio
    async def test_emit_event(self):
        """Test emitting pre-constructed events."""
        emitted = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                new_event = StreamEvent.create(
                    key="new-key",
                    value=event.value * 3,
                )
                await self.emit_event("output", new_event)
        
        actor = TestProcessor()
        actor.register_output_handler("output", lambda e: emitted.append(e))
        
        event = StreamEvent.create(key="test", value=10, timestamp=Timestamp.now())
        await actor.process_event(event)
        
        assert emitted[0].key == "new-key"
        assert emitted[0].value == 30
    
    @pytest.mark.asyncio
    async def test_async_output_handler(self):
        """Test async output handlers."""
        emitted = []
        
        async def async_handler(event):
            await asyncio.sleep(0.01)
            emitted.append(event)
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                await self.emit("output", event.value)
        
        actor = TestProcessor()
        actor.register_output_handler("output", async_handler)
        
        event = StreamEvent.create(key="test", value=42, timestamp=Timestamp.now())
        await actor.process_event(event)
        
        assert len(emitted) == 1


# ============================================
# StreamActor Late Event Tests
# ============================================
class TestStreamActorLateEvents:
    """Tests for StreamActor late event handling."""
    
    @pytest.mark.asyncio
    async def test_late_event_dropped(self):
        """Test that late events are dropped by default."""
        from aether_sdk.streaming.types import LateDataPolicy
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            def __init__(self):
                super().__init__()
                self.processed = []
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                self.processed.append(event.value)
        
        actor = TestProcessor()
        
        # Set watermark to time 100
        await actor.advance_watermark(Watermark(
            timestamp=Timestamp.from_seconds(100),
            stream_id="default",
        ))
        
        # Send late event (time 50 < watermark 100)
        late_event = StreamEvent.create(
            key="test",
            value=42,
            timestamp=Timestamp.from_seconds(50),
            event_type="default",
        )
        await actor._process_event_internal(late_event)
        
        # Event should not be processed
        assert 42 not in actor.processed
    
    @pytest.mark.asyncio
    async def test_late_event_side_output(self):
        """Test late events routed to side output via handler."""
        # Note: The emit() path fails because StreamEvent is unhashable.
        # This test uses the handler path instead.
        from aether_sdk.streaming.types import LateDataPolicy, StreamConfig
        
        late_events = []
        
        async def late_output_handler(e):
            late_events.append(e)
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            def __init__(self):
                config = StreamConfig(
                    late_data_policy=LateDataPolicy.SIDE_OUTPUT,
                )
                super().__init__(config=config)
                self.processed = []
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                self.processed.append(event.value)
        
        actor = TestProcessor()
        actor.register_late_data_handler(late_output_handler)
        
        # Set watermark
        await actor.advance_watermark(Watermark(
            timestamp=Timestamp.from_seconds(100),
            stream_id="default",
        ))
        
        # Send late event
        late_event = StreamEvent.create(
            key="test",
            value=42,
            timestamp=Timestamp.from_seconds(50),
            event_type="default",
        )
        
        await actor._process_event_internal(late_event)
        
        # Late event should be handled by custom handler
        assert len(late_events) == 1
        assert late_events[0].value == 42


# ============================================
# StreamActor Lifecycle Tests
# ============================================
class TestStreamActorLifecycle:
    """Tests for StreamActor lifecycle hooks."""
    
    @pytest.mark.asyncio
    async def test_on_start(self):
        """Test on_start lifecycle hook."""
        started = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def on_start(self) -> None:
                started.append(True)
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        await actor.on_start()
        
        assert started == [True]
    
    @pytest.mark.asyncio
    async def test_on_stop_flushes_buffer(self):
        """Test that on_stop flushes remaining events."""
        processed = []
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                processed.append(event.value)
        
        actor = TestProcessor()
        
        # Add event to backpressure buffer
        event = StreamEvent.create(key="test", value=42, timestamp=Timestamp.now())
        actor.backpressure.try_push(event)
        
        # Stop should flush
        await actor.on_stop()
        
        # Event should have been processed
        assert 42 in processed


# ============================================
# StreamActor Metrics Tests
# ============================================
class TestStreamActorMetrics:
    """Tests for StreamActor metrics."""
    
    @pytest.mark.asyncio
    async def test_get_metrics(self):
        """Test getting stream metrics."""
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Process some events
        for i in range(5):
            event = StreamEvent.create(key="test", value=i, timestamp=Timestamp.now())
            await actor._process_event_internal(event)
        
        metrics = actor.get_metrics()
        
        assert metrics['processed_count'] == 5
        assert metrics['late_events_count'] == 0
        assert 'watermarks' in metrics
        assert 'backpressure' in metrics
    
    @pytest.mark.asyncio
    async def test_metrics_include_watermarks(self):
        """Test that metrics include watermark info."""
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Set watermark
        await actor.advance_watermark(Watermark(
            timestamp=Timestamp.from_seconds(100),
            stream_id="stream-1",
        ))
        
        metrics = actor.get_metrics()
        
        assert "stream-1" in metrics['watermarks']
        assert metrics['watermarks']["stream-1"] == 100000


# ============================================
# StreamActor Window Configuration Tests
# ============================================
class TestStreamActorWindow:
    """Tests for StreamActor window configuration."""
    
    @pytest.mark.asyncio
    async def test_configure_window(self):
        """Test configuring windowing."""
        window_results = []
        
        def window_handler(events, info):
            window_results.append(len(events))
            return len(events)
        
        class TestProcessor(StreamActor[str, int]):
            @classmethod
            def name(cls) -> str:
                return "test_processor"
            
            async def process_event(self, event: StreamEvent[int]) -> None:
                pass
        
        actor = TestProcessor()
        
        # Configure window
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration.from_seconds(10),
        )
        actor.configure_window(spec, window_handler)
        
        # Window should be configured
        assert actor._window_assigner is not None
        assert actor._window_trigger is not None


if __name__ == "__main__":
    import unittest
    unittest.main()
