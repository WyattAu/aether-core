"""
Windowing Functions

Time-based windowing for stream processing:
- Tumbling windows: Fixed-size, non-overlapping
- Sliding windows: Fixed-size, overlapping  
- Session windows: Dynamic size based on activity gaps
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, Generic, List, Optional, TypeVar
from collections import defaultdict
import heapq

from .types import (
    Duration,
    Timestamp,
    StreamEvent,
    WindowSpec,
    WindowInfo,
    WindowType,
    PaneInfo,
    Watermark,
)

K = TypeVar('K')  # Key type
V = TypeVar('V')  # Value type
R = TypeVar('R')  # Result type


@dataclass
class WindowState(Generic[K, V]):
    """State for a single window."""
    window_id: str
    key: K
    start: Timestamp
    end: Timestamp
    events: List[StreamEvent[V]] = field(default_factory=list)
    max_timestamp: Optional[Timestamp] = None
    is_closed: bool = False
    early_fired: bool = False
    on_time_fired: bool = False
    
    def add_event(self, event: StreamEvent[V]) -> bool:
        """Add event to window. Returns True if event was added."""
        if self.is_closed:
            return False
        
        if event.timestamp < self.start or event.timestamp >= self.end:
            return False
        
        self.events.append(event)
        
        if self.max_timestamp is None or event.timestamp > self.max_timestamp:
            self.max_timestamp = event.timestamp
        
        return True
    
    def is_empty(self) -> bool:
        """Check if window has no events."""
        return len(self.events) == 0
    
    def clear(self) -> None:
        """Clear window events."""
        self.events.clear()
        self.is_closed = True


class WindowAssigner(Generic[K, V]):
    """Assigns events to windows."""
    
    def __init__(self, spec: WindowSpec):
        self.spec = spec
        self._windows: Dict[str, WindowState[K, V]] = {}
        self._key_windows: Dict[K, List[str]] = defaultdict(list)
    
    def assign(self, event: StreamEvent[V], key: K) -> List[WindowState[K, V]]:
        """Assign event to one or more windows."""
        windows = []
        
        if self.spec.type == WindowType.TUMBLING:
            window = self._assign_tumbling(event, key)
            if window:
                windows.append(window)
        
        elif self.spec.type == WindowType.SLIDING:
            windows = self._assign_sliding(event, key)
        
        elif self.spec.type == WindowType.SESSION:
            window = self._assign_session(event, key)
            if window:
                windows.append(window)
        
        return windows
    
    def _assign_tumbling(
        self,
        event: StreamEvent[V],
        key: K
    ) -> Optional[WindowState[K, V]]:
        """Assign to tumbling window."""
        # Calculate window start (aligned to size)
        size_ms = self.spec.size.ms
        start_ms = (event.timestamp.milliseconds // size_ms) * size_ms
        end_ms = start_ms + size_ms
        
        window_id = f"{key}_{start_ms}"
        
        if window_id not in self._windows:
            window = WindowState(
                window_id=window_id,
                key=key,
                start=Timestamp(start_ms),
                end=Timestamp(end_ms),
            )
            self._windows[window_id] = window
            self._key_windows[key].append(window_id)
        
        window = self._windows[window_id]
        window.add_event(event)
        return window
    
    def _assign_sliding(
        self,
        event: StreamEvent[V],
        key: K
    ) -> List[WindowState[K, V]]:
        """Assign to overlapping sliding windows."""
        windows = []
        size_ms = self.spec.size.milliseconds
        slide_ms = self.spec.slide.milliseconds
        
        # Find all windows that contain this event
        event_ts = event.timestamp.milliseconds
        
        # Start from the earliest window that could contain this event
        window_start = (event_ts // slide_ms) * slide_ms
        # Go back to include windows that started earlier but still contain event
        while window_start + size_ms > event_ts and window_start >= 0:
            window_start -= slide_ms
        window_start += slide_ms
        
        # Iterate through all overlapping windows
        current_start = window_start
        while current_start <= event_ts:
            window_id = f"{key}_{current_start}"
            
            if window_id not in self._windows:
                window = WindowState(
                    window_id=window_id,
                    key=key,
                    start=Timestamp(current_start),
                    end=Timestamp(current_start + size_ms),
                )
                self._windows[window_id] = window
                self._key_windows[key].append(window_id)
            
            window = self._windows[window_id]
            if window.add_event(event):
                windows.append(window)
            
            current_start += slide_ms
        
        return windows
    
    def _assign_session(
        self,
        event: StreamEvent[V],
        key: K
    ) -> Optional[WindowState[K, V]]:
        """Assign to session window (dynamic based on gap)."""
        gap_ms = self.spec.gap.ms if self.spec.gap else 0
        event_ts = event.timestamp.milliseconds
        
        # Find existing session to merge with
        key_window_ids = self._key_windows.get(key, [])
        merged_window: Optional[WindowState[K, V]] = None
        
        for window_id in list(key_window_ids):
            window = self._windows.get(window_id)
            if window is None or window.is_closed:
                continue
            
            # Check if event falls within session gap
            if window.max_timestamp:
                time_diff = abs(event_ts - window.max_timestamp.milliseconds)
                if time_diff <= gap_ms:
                    # Merge with existing session
                    if merged_window is None:
                        # Extend this window
                        window.add_event(event)
                        merged_window = window
                    else:
                        # Merge two windows
                        for evt in window.events:
                            merged_window.add_event(evt)
                        window.is_closed = True
        
        if merged_window:
            return merged_window
        
        # Create new session window
        window_id = f"{key}_session_{event_ts}"
        window = WindowState(
            window_id=window_id,
            key=key,
            start=Timestamp(event_ts),
            end=Timestamp(event_ts + gap_ms + 1),  # Will be extended
        )
        window.add_event(event)
        self._windows[window_id] = window
        self._key_windows[key].append(window_id)
        
        return window
    
    def get_triggered_windows(
        self,
        watermark: Timestamp
    ) -> List[WindowState[K, V]]:
        """Get windows ready to fire based on watermark."""
        triggered = []
        
        for window in self._windows.values():
            if window.is_closed:
                continue
            
            # Window end has passed the watermark
            if window.end <= watermark:
                window.on_time_fired = True
                triggered.append(window)
        
        return triggered
    
    def cleanup_closed(self) -> int:
        """Remove closed windows. Returns count removed."""
        to_remove = [
            wid for wid, w in self._windows.items() if w.is_closed
        ]
        
        for wid in to_remove:
            del self._windows[wid]
            # Remove from key index
            for key, window_ids in self._key_windows.items():
                if wid in window_ids:
                    window_ids.remove(wid)
        
        return len(to_remove)


class WindowTrigger(Generic[K, V, R]):
    """Triggers window firing with custom logic."""
    
    def __init__(
        self,
        assigner: WindowAssigner[K, V],
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R],
        early_firing: Optional[Duration] = None,
    ):
        self.assigner = assigner
        self.handler = handler
        self.early_firing = early_firing
        self._results: List[R] = []
    
    def process(self, event: StreamEvent[V], key: K) -> List[R]:
        """Process event and return any triggered results."""
        results = []
        windows = self.assigner.assign(event, key)
        
        # Check for early firing
        if self.early_firing:
            for window in windows:
                if not window.early_fired and not window.is_empty():
                    # Check if we should fire early
                    if window.max_timestamp:
                        elapsed = event.timestamp.milliseconds - window.start.milliseconds
                        if elapsed >= self.early_firing.milliseconds:
                            result = self._fire_window(window, PaneInfo.EARLY)
                            if result is not None:
                                results.append(result)
                            window.early_fired = True
        
        return results
    
    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance watermark and fire completed windows."""
        results = []
        triggered = self.assigner.get_triggered_windows(watermark)
        
        for window in triggered:
            if not window.is_empty():
                pane = PaneInfo.LATE if window.on_time_fired else PaneInfo.ON_TIME
                result = self._fire_window(window, pane)
                if result is not None:
                    results.append(result)
        
        return results
    
    def _fire_window(
        self,
        window: WindowState[K, V],
        pane: PaneInfo
    ) -> Optional[R]:
        """Fire window and return result."""
        if window.is_empty():
            return None
        
        info = WindowInfo(
            start=window.start,
            end=window.end,
            max_timestamp=window.max_timestamp or window.start,
            pane=pane,
            window_id=window.window_id,
        )
        
        result = self.handler(window.events.copy(), info)
        return result


class TumblingWindow(Generic[K, V]):
    """Convenience class for tumbling windows."""
    
    def __init__(
        self,
        size: Duration,
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R],
        late_tolerance: Duration = None,
    ):
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=size,
            late_tolerance=late_tolerance or Duration.from_seconds(0),
        )
        self.assigner = WindowAssigner(spec)
        self.trigger = WindowTrigger(self.assigner, handler)
    
    def process(self, event: StreamEvent[V], key: K) -> List[R]:
        """Process event."""
        return self.trigger.process(event, key)
    
    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance watermark."""
        return self.trigger.advance_watermark(watermark)


class SlidingWindow(Generic[K, V]):
    """Convenience class for sliding windows."""
    
    def __init__(
        self,
        size: Duration,
        slide: Duration,
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R],
        late_tolerance: Duration = None,
    ):
        spec = WindowSpec(
            type=WindowType.SLIDING,
            size=size,
            slide=slide,
            late_tolerance=late_tolerance or Duration.from_seconds(0),
        )
        self.assigner = WindowAssigner(spec)
        self.trigger = WindowTrigger(self.assigner, handler)
    
    def process(self, event: StreamEvent[V], key: K) -> List[R]:
        """Process event."""
        return self.trigger.process(event, key)
    
    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance watermark."""
        return self.trigger.advance_watermark(watermark)


class SessionWindow(Generic[K, V]):
    """Convenience class for session windows."""
    
    def __init__(
        self,
        gap: Duration,
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R],
        late_tolerance: Duration = None,
    ):
        spec = WindowSpec(
            type=WindowType.SESSION,
            gap=gap,
            size=Duration.from_seconds(0),  # Not used for session
            late_tolerance=late_tolerance or Duration.from_seconds(0),
        )
        self.assigner = WindowAssigner(spec)
        self.trigger = WindowTrigger(self.assigner, handler)
    
    def process(self, event: StreamEvent[V], key: K) -> List[R]:
        """Process event."""
        return self.trigger.process(event, key)
    
    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance watermark."""
        return self.trigger.advance_watermark(watermark)


# Decorator for window-based processing
def window(
    type: WindowType,
    size: Optional[Duration] = None,
    slide: Optional[Duration] = None,
    gap: Optional[Duration] = None,
) -> Callable:
    """Decorator for window-based stream processing.
    
    Example:
        @window(type=WindowType.TUMBLING, size=Duration.minutes(5))
        def process_batch(events: List[StreamEvent], info: WindowInfo):
            # Process events in 5-minute tumbling window
            pass
    """
    def decorator(func: Callable) -> Callable:
        func._window_config = WindowSpec(
            type=type,
            size=size if size is not None else Duration.from_seconds(0),
            slide=slide,
            gap=gap,
        )
        return func
    return decorator


def tumbling(size: Duration) -> Callable:
    """Decorator for tumbling window.
    
    Example:
        @tumbling(size=Duration.minutes(5))
        def process(events: List[StreamEvent], info: WindowInfo):
            pass
    """
    return window(type=WindowType.TUMBLING, size=size)


def sliding(size: Duration, slide: Duration) -> Callable:
    """Decorator for sliding window.
    
    Example:
        @sliding(size=Duration.minutes(10), slide=Duration.minutes(1))
        def process(events: List[StreamEvent], info: WindowInfo):
            pass
    """
    return window(type=WindowType.SLIDING, size=size, slide=slide)


def session(gap: Duration) -> Callable:
    """Decorator for session window.
    
    Example:
        @session(gap=Duration.minutes(5))
        def process(events: List[StreamEvent], info: WindowInfo):
            pass
    """
    return window(type=WindowType.SESSION, gap=gap)
