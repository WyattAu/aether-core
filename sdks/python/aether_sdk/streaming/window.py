"""
Windowing Functions

Time-based windowing for stream processing:
- Tumbling windows: Fixed-size, non-overlapping
- Sliding windows: Fixed-size, overlapping
- Session windows: Dynamic size based on activity gaps

Example:
    >>> from aether_sdk.streaming.window import TumblingWindow
    >>> from aether_sdk.streaming.types import Duration, StreamEvent
    >>> tw = TumblingWindow(size=Duration.from_minutes(5), handler=my_handler)
    >>> results = tw.process(event, key="order-1")
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from typing import Callable, Dict, Generic, List, Optional, TypeVar

from .types import (
    Duration,
    PaneInfo,
    StreamEvent,
    Timestamp,
    WindowInfo,
    WindowSpec,
    WindowType,
)

K = TypeVar("K")
V = TypeVar("V")
R = TypeVar("R")


@dataclass
class WindowState(Generic[K, V]):
    """Mutable state for a single window.

    Attributes:
        window_id: Unique identifier for this window.
        key: Partition key associated with the window.
        start: Window start timestamp (inclusive).
        end: Window end timestamp (exclusive).
        events: Events assigned to this window.
        max_timestamp: Highest event timestamp seen so far.
        is_closed: Whether the window has been fired and closed.
        early_fired: Whether an early firing has occurred.
        on_time_fired: Whether the on-time firing has occurred.
    """

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
        """Add an event to the window if it falls within bounds.

        Args:
            event: The stream event to add.

        Returns:
            ``True`` if the event was added, ``False`` if the window
            is closed or the event is out of range.
        """
        if self.is_closed:
            return False

        if event.timestamp < self.start or event.timestamp >= self.end:
            return False

        self.events.append(event)

        if self.max_timestamp is None or event.timestamp > self.max_timestamp:
            self.max_timestamp = event.timestamp

        return True

    def is_empty(self) -> bool:
        """Check whether the window contains no events.

        Returns:
            ``True`` if no events have been added.
        """
        return len(self.events) == 0

    def clear(self) -> None:
        """Clear all events and mark the window as closed."""
        self.events.clear()
        self.is_closed = True


class WindowAssigner(Generic[K, V]):
    """Assigns stream events to windows based on a :class:`WindowSpec`.

    Supports tumbling, sliding, and session windowing strategies.
    Events are routed to the appropriate window(s) based on their
    timestamp and key.

    Example:
        >>> assigner = WindowAssigner(WindowSpec(
        ...     type=WindowType.TUMBLING,
        ...     size=Duration.from_minutes(5),
        ... ))
        >>> windows = assigner.assign(event, key="user-1")
    """

    def __init__(self, spec: WindowSpec):
        """Initialize the assigner.

        Args:
            spec: The window specification.
        """
        self.spec = spec
        self._windows: Dict[str, WindowState[K, V]] = {}
        self._key_windows: Dict[K, List[str]] = defaultdict(list)

    def assign(self, event: StreamEvent[V], key: K) -> List[WindowState[K, V]]:
        """Assign an event to one or more windows.

        Args:
            event: The stream event.
            key: The partition key.

        Returns:
            A list of :class:`WindowState` instances that the event
            was added to.
        """
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
        self, event: StreamEvent[V], key: K
    ) -> Optional[WindowState[K, V]]:
        """Assign an event to a tumbling window."""
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

    def _assign_sliding(self, event: StreamEvent[V], key: K) -> List[WindowState[K, V]]:
        """Assign an event to all overlapping sliding windows."""
        windows = []
        size_ms = self.spec.size.milliseconds
        slide_ms = self.spec.slide.milliseconds

        event_ts = event.timestamp.milliseconds

        window_start = (event_ts // slide_ms) * slide_ms
        while window_start + size_ms > event_ts and window_start >= 0:
            window_start -= slide_ms
        window_start += slide_ms

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
        self, event: StreamEvent[V], key: K
    ) -> Optional[WindowState[K, V]]:
        """Assign an event to a session window (merges if within gap)."""
        gap_ms = self.spec.gap.ms if self.spec.gap else 0
        event_ts = event.timestamp.milliseconds

        key_window_ids = self._key_windows.get(key, [])
        merged_window: Optional[WindowState[K, V]] = None

        for window_id in list(key_window_ids):
            window = self._windows.get(window_id)
            if window is None or window.is_closed:
                continue

            if window.max_timestamp:
                time_diff = abs(event_ts - window.max_timestamp.milliseconds)
                if time_diff <= gap_ms:
                    if merged_window is None:
                        window.add_event(event)
                        merged_window = window
                    else:
                        for evt in window.events:
                            merged_window.add_event(evt)
                        window.is_closed = True

        if merged_window:
            return merged_window

        window_id = f"{key}_session_{event_ts}"
        window = WindowState(
            window_id=window_id,
            key=key,
            start=Timestamp(event_ts),
            end=Timestamp(event_ts + gap_ms + 1),
        )
        window.add_event(event)
        self._windows[window_id] = window
        self._key_windows[key].append(window_id)

        return window

    def get_triggered_windows(self, watermark: Timestamp) -> List[WindowState[K, V]]:
        """Return windows that should fire based on the watermark.

        A window is triggered when its end timestamp is less than or
        equal to the watermark.

        Args:
            watermark: The current watermark timestamp.

        Returns:
            A list of :class:`WindowState` instances ready to fire.
        """
        triggered = []

        for window in self._windows.values():
            if window.is_closed:
                continue

            if window.end <= watermark:
                window.on_time_fired = True
                triggered.append(window)

        return triggered

    def cleanup_closed(self) -> int:
        """Remove all closed windows from memory.

        Returns:
            The number of windows that were removed.
        """
        to_remove = [wid for wid, w in self._windows.items() if w.is_closed]

        for wid in to_remove:
            del self._windows[wid]
            for key, window_ids in self._key_windows.items():
                if wid in window_ids:
                    window_ids.remove(wid)

        return len(to_remove)


class WindowTrigger(Generic[K, V, R]):
    """Triggers window firing based on watermark progress and early-firing rules.

    Delegates event assignment to a :class:`WindowAssigner` and invokes
    the configured handler when windows are ready to fire.

    Example:
        >>> trigger = WindowTrigger(assigner, my_handler, early_firing=Duration.from_seconds(30))
        >>> results = trigger.process(event, key="user-1")
    """

    def __init__(
        self,
        assigner: WindowAssigner[K, V],
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R],
        early_firing: Optional[Duration] = None,
    ):
        """Initialize the trigger.

        Args:
            assigner: The window assigner to use.
            handler: Callable invoked with ``(events, window_info)``
                when a window fires.
            early_firing: If set, windows fire early after this
                duration from the window start.
        """
        self.assigner = assigner
        self.handler = handler
        self.early_firing = early_firing
        self._results: List[R] = []

    def process(self, event: StreamEvent[V], key: K) -> List[R]:
        """Process an event and return any triggered results.

        Args:
            event: The stream event.
            key: The partition key.

        Returns:
            A (possibly empty) list of results from early firings.
        """
        results = []
        windows = self.assigner.assign(event, key)

        if self.early_firing:
            for window in windows:
                if not window.early_fired and not window.is_empty():
                    if window.max_timestamp:
                        elapsed = (
                            event.timestamp.milliseconds - window.start.milliseconds
                        )
                        if elapsed >= self.early_firing.milliseconds:
                            result = self._fire_window(window, PaneInfo.EARLY)
                            if result is not None:
                                results.append(result)
                            window.early_fired = True

        return results

    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance the watermark and fire completed windows.

        Args:
            watermark: The new watermark timestamp.

        Returns:
            A (possibly empty) list of results from fired windows.
        """
        results = []
        triggered = self.assigner.get_triggered_windows(watermark)

        for window in triggered:
            if not window.is_empty():
                pane = PaneInfo.LATE if window.on_time_fired else PaneInfo.ON_TIME
                result = self._fire_window(window, pane)
                if result is not None:
                    results.append(result)

        return results

    def _fire_window(self, window: WindowState[K, V], pane: PaneInfo) -> Optional[R]:
        """Fire a window by invoking the handler.

        Args:
            window: The window state to fire.
            pane: The pane type (EARLY, ON_TIME, or LATE).

        Returns:
            The handler's result, or ``None`` if the window is empty.
        """
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
    """Convenience class for tumbling (fixed-size, non-overlapping) windows.

    Args:
        size: Window duration.
        handler: Callable invoked with ``(events, window_info)`` when
            the window fires.
        late_tolerance: Maximum lateness allowed for events.

    Example:
        >>> tw = TumblingWindow(Duration.from_minutes(5), aggregate_fn)
        >>> results = tw.process(event, key="k1")
    """

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
        """Process an event and return any early-firing results.

        Args:
            event: The stream event.
            key: The partition key.

        Returns:
            A (possibly empty) list of results.
        """
        return self.trigger.process(event, key)

    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance the watermark and fire completed windows.

        Args:
            watermark: The new watermark.

        Returns:
            A (possibly empty) list of results.
        """
        return self.trigger.advance_watermark(watermark)


class SlidingWindow(Generic[K, V]):
    """Convenience class for sliding (fixed-size, overlapping) windows.

    Args:
        size: Window duration.
        slide: Slide interval.
        handler: Callable invoked when windows fire.
        late_tolerance: Maximum lateness allowed for events.

    Example:
        >>> sw = SlidingWindow(Duration.from_minutes(10), Duration.from_minutes(1), fn)
    """

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
        """Process an event and return any early-firing results."""
        return self.trigger.process(event, key)

    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance the watermark and fire completed windows."""
        return self.trigger.advance_watermark(watermark)


class SessionWindow(Generic[K, V]):
    """Convenience class for session (activity-gap-based) windows.

    Args:
        gap: Maximum inactivity gap before a new session starts.
        handler: Callable invoked when windows fire.
        late_tolerance: Maximum lateness allowed for events.

    Example:
        >>> sw = SessionWindow(Duration.from_minutes(5), fn)
    """

    def __init__(
        self,
        gap: Duration,
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R],
        late_tolerance: Duration = None,
    ):
        spec = WindowSpec(
            type=WindowType.SESSION,
            gap=gap,
            size=Duration.from_seconds(0),
            late_tolerance=late_tolerance or Duration.from_seconds(0),
        )
        self.assigner = WindowAssigner(spec)
        self.trigger = WindowTrigger(self.assigner, handler)

    def process(self, event: StreamEvent[V], key: K) -> List[R]:
        """Process an event and return any early-firing results."""
        return self.trigger.process(event, key)

    def advance_watermark(self, watermark: Timestamp) -> List[R]:
        """Advance the watermark and fire completed windows."""
        return self.trigger.advance_watermark(watermark)


def window(
    type: WindowType,
    size: Optional[Duration] = None,
    slide: Optional[Duration] = None,
    gap: Optional[Duration] = None,
) -> Callable:
    """Decorator for marking a function as a window-based processor.

    The decorated function receives ``(events: List[StreamEvent],
    info: WindowInfo)`` and its ``_window_config`` attribute is set
    to the constructed :class:`WindowSpec`.

    Args:
        type: The window type.
        size: Window size (required for tumbling and sliding).
        slide: Slide interval (required for sliding).
        gap: Activity gap (required for session).

    Returns:
        A decorator function.

    Example:
        >>> @window(type=WindowType.TUMBLING, size=Duration.from_minutes(5))
        ... def process_batch(events, info):
        ...     return sum(e.value for e in events)
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
    """Decorator shorthand for a tumbling window.

    Args:
        size: Window size.

    Returns:
        A decorator function.

    Example:
        >>> @tumbling(size=Duration.from_minutes(5))
        ... def process(events, info):
        ...     pass
    """
    return window(type=WindowType.TUMBLING, size=size)


def sliding(size: Duration, slide: Duration) -> Callable:
    """Decorator shorthand for a sliding window.

    Args:
        size: Window size.
        slide: Slide interval.

    Returns:
        A decorator function.

    Example:
        >>> @sliding(size=Duration.from_minutes(10), slide=Duration.from_minutes(1))
        ... def process(events, info):
        ...     pass
    """
    return window(type=WindowType.SLIDING, size=size, slide=slide)


def session(gap: Duration) -> Callable:
    """Decorator shorthand for a session window.

    Args:
        gap: Maximum inactivity gap.

    Returns:
        A decorator function.

    Example:
        >>> @session(gap=Duration.from_minutes(5))
        ... def process(events, info):
        ...     pass
    """
    return window(type=WindowType.SESSION, gap=gap)
