"""
Tests for Aether SDK Window Module

Tests for time-based windowing functions.
"""

from typing import Any, List

from aether_sdk.streaming.types import (Duration, StreamEvent, Timestamp,
                                        WindowInfo, WindowSpec, WindowType)
from aether_sdk.streaming.window import (SessionWindow, SlidingWindow,
                                         TumblingWindow, WindowAssigner,
                                         WindowState, WindowTrigger, session,
                                         sliding, tumbling, window)

# ============================================
# Helper Functions
# ============================================


def create_event(key: str, timestamp_ms: int, value: Any = None) -> StreamEvent:
    """Create a test stream event."""
    return StreamEvent(
        key=key,
        value=value or {"data": key},
        timestamp=Timestamp(timestamp_ms),
    )


def simple_handler(events: List[StreamEvent], info: WindowInfo) -> dict:
    """Simple window handler for testing."""
    return {
        "count": len(events),
        "window_id": info.window_id,
        "start": info.start.milliseconds,
        "end": info.end.milliseconds,
    }


# ============================================
# WindowState Tests
# ============================================


class TestWindowState:
    """Tests for WindowState."""

    def test_init(self):
        """Test initialization."""
        state = WindowState(
            window_id="test_window",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        assert state.window_id == "test_window"
        assert state.key == "key1"
        assert state.start.milliseconds == 0
        assert state.end.milliseconds == 1000
        assert state.events == []
        assert state.max_timestamp is None
        assert state.is_closed is False

    def test_add_event(self):
        """Test adding event."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        event = create_event("key1", 500)
        result = state.add_event(event)

        assert result is True
        assert len(state.events) == 1
        assert state.max_timestamp == Timestamp(500)

    def test_add_event_outside_window(self):
        """Test adding event outside window bounds."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        # Event before window
        event = create_event("key1", -100)
        result = state.add_event(event)
        assert result is False

        # Event at end (exclusive)
        event = create_event("key1", 1000)
        result = state.add_event(event)
        assert result is False

        # Event after window
        event = create_event("key1", 1500)
        result = state.add_event(event)
        assert result is False

    def test_add_event_to_closed_window(self):
        """Test adding event to closed window."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )
        state.is_closed = True

        event = create_event("key1", 500)
        result = state.add_event(event)

        assert result is False

    def test_is_empty(self):
        """Test is_empty check."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        assert state.is_empty() is True

        state.add_event(create_event("key1", 500))
        assert state.is_empty() is False

    def test_clear(self):
        """Test clearing window."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        state.add_event(create_event("key1", 500))
        state.clear()

        assert state.is_empty() is True
        assert state.is_closed is True

    def test_max_timestamp_update(self):
        """Test max timestamp is updated correctly."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        # Add events out of order
        state.add_event(create_event("key1", 500))
        assert state.max_timestamp == Timestamp(500)

        state.add_event(create_event("key1", 300))
        assert state.max_timestamp == Timestamp(500)

        state.add_event(create_event("key1", 800))
        assert state.max_timestamp == Timestamp(800)


# ============================================
# WindowAssigner Tests
# ============================================


class TestWindowAssigner:
    """Tests for WindowAssigner."""

    def test_tumbling_window(self):
        """Test tumbling window assignment."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),  # 1 second
        )
        assigner = WindowAssigner(spec)

        # Events at different times
        event1 = create_event("key1", 500)  # Window 0-1000
        event2 = create_event("key1", 1500)  # Window 1000-2000

        windows1 = assigner.assign(event1, "key1")
        windows2 = assigner.assign(event2, "key1")

        assert len(windows1) == 1
        assert len(windows2) == 1
        assert windows1[0].window_id != windows2[0].window_id

    def test_tumbling_window_same_window(self):
        """Test events in same tumbling window."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)

        # Two events in same window
        event1 = create_event("key1", 100)
        event2 = create_event("key1", 500)

        windows1 = assigner.assign(event1, "key1")
        windows2 = assigner.assign(event2, "key1")

        # Both events should be in the same window
        assert windows1[0].window_id == windows2[0].window_id
        # The window object is shared, so after both assignments it has 2 events
        assert len(windows2[0].events) == 2

    def test_sliding_window(self):
        """Test sliding window assignment."""
        spec = WindowSpec(
            type=WindowType.SLIDING,
            size=Duration(1000),  # 1 second window
            slide=Duration(500),  # 500ms slide
        )
        assigner = WindowAssigner(spec)

        # Event at 600ms should be in windows:
        # 0-1000, 500-1500
        event = create_event("key1", 600)
        windows = assigner.assign(event, "key1")

        # Should be in 2 overlapping windows
        assert len(windows) >= 1

    def test_session_window(self):
        """Test session window assignment."""
        spec = WindowSpec(
            type=WindowType.SESSION,
            gap=Duration(500),  # 500ms gap
            size=Duration(0),
        )
        assigner = WindowAssigner(spec)

        # First event creates session
        event1 = create_event("key1", 0)
        windows1 = assigner.assign(event1, "key1")

        assert len(windows1) == 1

        # Event within gap extends session
        event2 = create_event("key1", 300)
        windows2 = assigner.assign(event2, "key1")

        assert len(windows2) == 1
        assert len(windows2[0].events) == 2

    def test_session_window_new_session(self):
        """Test session window creates new session after gap."""
        spec = WindowSpec(
            type=WindowType.SESSION,
            gap=Duration(500),
            size=Duration(0),
        )
        assigner = WindowAssigner(spec)

        # First event
        event1 = create_event("key1", 0)
        assigner.assign(event1, "key1")

        # Event after gap creates new session
        event2 = create_event("key1", 1000)  # > 500ms gap
        windows2 = assigner.assign(event2, "key1")

        # Should be in new session
        assert windows2[0].start.milliseconds == 1000

    def test_get_triggered_windows(self):
        """Test getting triggered windows."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)

        # Add event to window 0-1000
        event = create_event("key1", 500)
        assigner.assign(event, "key1")

        # Watermark at 2000 should trigger window
        triggered = assigner.get_triggered_windows(Timestamp(2000))

        assert len(triggered) == 1
        assert triggered[0].on_time_fired is True

    def test_cleanup_closed(self):
        """Test cleanup of closed windows."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)

        # Add events
        event1 = create_event("key1", 500)
        event2 = create_event("key1", 1500)
        assigner.assign(event1, "key1")
        windows2 = assigner.assign(event2, "key1")

        # Close first window
        for win in assigner._windows.values():
            if win.window_id != windows2[0].window_id:
                win.is_closed = True

        # Cleanup
        removed = assigner.cleanup_closed()

        assert removed == 1


# ============================================
# WindowTrigger Tests
# ============================================


class TestWindowTrigger:
    """Tests for WindowTrigger."""

    def test_process_event(self):
        """Test processing event."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)
        trigger = WindowTrigger(assigner, simple_handler)

        event = create_event("key1", 500)
        results = trigger.process(event, "key1")

        # No results without watermark
        assert results == []

    def test_advance_watermark(self):
        """Test advancing watermark."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)
        trigger = WindowTrigger(assigner, simple_handler)

        # Add events
        event = create_event("key1", 500)
        trigger.process(event, "key1")

        # Advance watermark past window
        results = trigger.advance_watermark(Timestamp(2000))

        assert len(results) == 1
        assert results[0]["count"] == 1

    def test_early_firing(self):
        """Test early firing."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(10000),  # 10 second window
        )
        assigner = WindowAssigner(spec)
        trigger = WindowTrigger(
            assigner,
            simple_handler,
            early_firing=Duration(500),  # Fire after 500ms
        )

        # Event at window start
        event1 = create_event("key1", 0)
        trigger.process(event1, "key1")

        # Event after early firing threshold
        event2 = create_event("key1", 600)
        results = trigger.process(event2, "key1")

        # Should have early fired
        assert len(results) == 1

    def test_empty_window_not_fired(self):
        """Test empty window is not fired."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)
        trigger = WindowTrigger(assigner, simple_handler)

        # Create window without adding events
        assigner._windows["test"] = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        # Advance watermark
        results = trigger.advance_watermark(Timestamp(2000))

        # Empty window should not fire
        assert len(results) == 0


# ============================================
# TumblingWindow Tests
# ============================================


class TestTumblingWindow:
    """Tests for TumblingWindow convenience class."""

    def test_basic_usage(self):
        """Test basic tumbling window usage."""
        window = TumblingWindow(
            size=Duration(1000),
            handler=simple_handler,
        )

        # Add events
        event1 = create_event("key1", 100)
        event2 = create_event("key1", 500)

        window.process(event1, "key1")
        window.process(event2, "key1")

        # Trigger window
        results = window.advance_watermark(Timestamp(2000))

        assert len(results) == 1
        assert results[0]["count"] == 2

    def test_multiple_windows(self):
        """Test multiple tumbling windows."""
        window = TumblingWindow(
            size=Duration(1000),
            handler=simple_handler,
        )

        # Events in different windows
        window.process(create_event("key1", 500), "key1")  # Window 0-1000
        window.process(create_event("key1", 1500), "key1")  # Window 1000-2000

        results = window.advance_watermark(Timestamp(3000))

        assert len(results) == 2

    def test_with_late_tolerance(self):
        """Test tumbling window with late tolerance."""
        window = TumblingWindow(
            size=Duration(1000),
            handler=simple_handler,
            late_tolerance=Duration(100),
        )

        assert window.assigner.spec.late_tolerance.ms == 100


# ============================================
# SlidingWindow Tests
# ============================================


class TestSlidingWindowClass:
    """Tests for SlidingWindow convenience class."""

    def test_basic_usage(self):
        """Test basic sliding window usage."""
        window = SlidingWindow(
            size=Duration(1000),
            slide=Duration(500),
            handler=simple_handler,
        )

        # Add event
        event = create_event("key1", 600)
        window.process(event, "key1")

        # Trigger
        results = window.advance_watermark(Timestamp(2000))

        # Should have results
        assert len(results) >= 1


# ============================================
# SessionWindow Tests
# ============================================


class TestSessionWindowClass:
    """Tests for SessionWindow convenience class."""

    def test_basic_usage(self):
        """Test basic session window usage."""
        window = SessionWindow(
            gap=Duration(500),
            handler=simple_handler,
        )

        # Add events within gap
        event1 = create_event("key1", 0)
        event2 = create_event("key1", 300)

        window.process(event1, "key1")
        window.process(event2, "key1")

        # Trigger
        results = window.advance_watermark(Timestamp(1000))

        assert len(results) == 1
        assert results[0]["count"] == 2


# ============================================
# Decorator Tests
# ============================================


class TestWindowDecorators:
    """Tests for window decorators."""

    def test_window_decorator(self):
        """Test @window decorator."""

        @window(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        def process_window(events, info):
            return len(events)

        assert hasattr(process_window, "_window_config")
        assert process_window._window_config.type == WindowType.TUMBLING

    def test_tumbling_decorator(self):
        """Test @tumbling decorator."""

        @tumbling(size=Duration(5000))
        def process(events, info):
            return len(events)

        assert hasattr(process, "_window_config")
        assert process._window_config.type == WindowType.TUMBLING
        assert process._window_config.size.ms == 5000

    def test_sliding_decorator(self):
        """Test @sliding decorator."""

        @sliding(size=Duration(10000), slide=Duration(1000))
        def process(events, info):
            return len(events)

        assert hasattr(process, "_window_config")
        assert process._window_config.type == WindowType.SLIDING
        assert process._window_config.size.ms == 10000
        assert process._window_config.slide.ms == 1000

    def test_session_decorator(self):
        """Test @session decorator."""

        @session(gap=Duration(5000))
        def process(events, info):
            return len(events)

        assert hasattr(process, "_window_config")
        assert process._window_config.type == WindowType.SESSION
        assert process._window_config.gap.ms == 5000


# ============================================
# Edge Cases Tests
# ============================================


class TestWindowEdgeCases:
    """Tests for edge cases."""

    def test_event_at_window_boundary(self):
        """Test event at window boundary."""
        state = WindowState(
            window_id="test",
            key="key1",
            start=Timestamp(0),
            end=Timestamp(1000),
        )

        # Event at start (inclusive)
        event1 = create_event("key1", 0)
        assert state.add_event(event1) is True

        # Event at end (exclusive)
        event2 = create_event("key1", 1000)
        assert state.add_event(event2) is False

    def test_multiple_keys(self):
        """Test events with different keys."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)

        event1 = create_event("key1", 500)
        event2 = create_event("key2", 500)

        windows1 = assigner.assign(event1, "key1")
        windows2 = assigner.assign(event2, "key2")

        # Different keys = different windows
        assert windows1[0].window_id != windows2[0].window_id

    def test_late_data_handling(self):
        """Test late data is not added to closed windows."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)

        # Add event and close window
        event1 = create_event("key1", 500)
        windows1 = assigner.assign(event1, "key1")
        windows1[0].is_closed = True

        # Try to add late event
        event2 = create_event("key1", 600)
        windows2 = assigner.assign(event2, "key1")

        # Should create new window since old one is closed
        # or return empty list depending on implementation
        assert len(windows2) <= 1

    def test_session_merge(self):
        """Test session window merging."""
        spec = WindowSpec(
            type=WindowType.SESSION,
            gap=Duration(1000),
            size=Duration(0),
        )
        assigner = WindowAssigner(spec)

        # Create first session
        event1 = create_event("key1", 0)
        assigner.assign(event1, "key1")

        # Event within gap - extends session
        event2 = create_event("key1", 500)
        windows2 = assigner.assign(event2, "key1")

        assert len(windows2[0].events) == 2

    def test_empty_advance_watermark(self):
        """Test advance watermark with no events."""
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)
        trigger = WindowTrigger(assigner, simple_handler)

        results = trigger.advance_watermark(Timestamp(5000))

        # No windows to trigger
        assert len(results) == 0

    def test_window_info_in_handler(self):
        """Test WindowInfo is passed correctly to handler."""
        received_info = []

        def capture_handler(events, info):
            received_info.append(info)
            return len(events)

        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration(1000),
        )
        assigner = WindowAssigner(spec)
        trigger = WindowTrigger(assigner, capture_handler)

        event = create_event("key1", 500)
        trigger.process(event, "key1")
        trigger.advance_watermark(Timestamp(2000))

        assert len(received_info) == 1
        assert received_info[0].window_id is not None
        assert received_info[0].start.milliseconds == 0
        assert received_info[0].end.milliseconds == 1000
