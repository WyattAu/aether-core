"""
Backpressure Handling

Strategies for handling flow control in stream processing:
- BUFFER: Buffer events up to capacity
- DROP: Drop events when overloaded
- FAIL: Raise error when overloaded
- LATEST: Keep only the latest events

Example:
    >>> from aether_sdk.streaming.backpressure import BackpressureController, BackpressureConfig
    >>> from aether_sdk.streaming.types import BackpressureStrategy
    >>> config = BackpressureConfig(strategy=BackpressureStrategy.BUFFER, buffer_size=1000)
    >>> ctrl = BackpressureController(config)
    >>> accepted = ctrl.try_push(event)
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Any, Callable, Generic, List, Optional, TypeVar
from collections import deque
import asyncio
import threading
import time

from .types import (
    BackpressureStrategy,
    BackpressureConfig,
    StreamEvent,
    Timestamp,
)

T = TypeVar('T')


@dataclass
class BackpressureStats:
    """Statistics for backpressure handling.

    Attributes:
        total_events: Total events received.
        buffered_events: Events currently in the buffer.
        dropped_events: Events dropped due to overflow or LATEST strategy.
        rejected_events: Events explicitly rejected (FAIL strategy).
        overflow_count: Number of times the high watermark was reached.
        resume_count: Number of times the buffer recovered below the
            low watermark.
        current_buffer_size: Current number of events in the buffer.
        high_watermark_reached: Whether the buffer is currently above
            the high watermark.
    """
    total_events: int = 0
    buffered_events: int = 0
    dropped_events: int = 0
    rejected_events: int = 0
    overflow_count: int = 0
    resume_count: int = 0
    current_buffer_size: int = 0
    high_watermark_reached: bool = False

    def to_dict(self) -> dict:
        """Convert statistics to a plain dictionary.

        Returns:
            A dict with all stat fields as key-value pairs.
        """
        return {
            'total_events': self.total_events,
            'buffered_events': self.buffered_events,
            'dropped_events': self.dropped_events,
            'rejected_events': self.rejected_events,
            'overflow_count': self.overflow_count,
            'resume_count': self.resume_count,
            'current_buffer_size': self.current_buffer_size,
            'high_watermark_reached': self.high_watermark_reached,
        }


class BackpressureError(Exception):
    """Base exception for backpressure-related failures."""
    pass


class BufferFullError(BackpressureError):
    """Raised when the buffer is full and the strategy is FAIL.

    Attributes:
        buffer_size: The configured buffer capacity.
        event: The event that was rejected (if available).
    """

    def __init__(self, buffer_size: int, event: Optional[StreamEvent] = None):
        self.buffer_size = buffer_size
        self.event = event
        super().__init__(
            f"Buffer full (size={buffer_size}). Cannot accept more events."
        )


class BackpressureController(Generic[T]):
    """Controls flow of events through the stream processor.

    Strategies:
    - BUFFER: Queue events up to *buffer_size*, then reject.
    - DROP: Silently drop new events when the buffer is full.
    - FAIL: Raise :class:`BufferFullError` when the buffer is full.
    - LATEST: Keep only the most recent events (circular buffer).

    Example:
        >>> ctrl = BackpressureController(BackpressureConfig(
        ...     strategy=BackpressureStrategy.BUFFER,
        ...     buffer_size=10000,
        ... ))
        >>> if ctrl.try_push(event):
        ...     process(ctrl.pop())
    """

    def __init__(self, config: Optional[BackpressureConfig] = None):
        """Initialize the controller.

        Args:
            config: Optional configuration. Defaults to
                :class:`BackpressureConfig`.
        """
        self._config = config or BackpressureConfig()
        self._buffer: deque = deque(maxlen=self._config.buffer_size)
        self._stats = BackpressureStats()
        self._overflow_callback: Optional[Callable[[], None]] = None
        self._resume_callback: Optional[Callable[[], None]] = None
        self._lock = threading.Lock()

        if self._config.on_overflow:
            self._overflow_callback = self._config.on_overflow
        if self._config.on_resume:
            self._resume_callback = self._config.on_resume

    @property
    def config(self) -> BackpressureConfig:
        """Return the current configuration."""
        return self._config

    @property
    def stats(self) -> BackpressureStats:
        """Return the current statistics."""
        return self._stats

    @property
    def is_overloaded(self) -> bool:
        """Check whether the buffer is at or above the high watermark.

        Returns:
            ``True`` if the buffer fill ratio >= *high_watermark*.
        """
        if len(self._buffer) == 0:
            return False
        fill_ratio = len(self._buffer) / self._config.buffer_size
        return fill_ratio >= self._config.high_watermark

    @property
    def is_recovered(self) -> bool:
        """Check whether the buffer has recovered below the low watermark.

        Returns:
            ``True`` if the buffer fill ratio <= *low_watermark* or
            the buffer is empty.
        """
        if len(self._buffer) == 0:
            return True
        fill_ratio = len(self._buffer) / self._config.buffer_size
        return fill_ratio <= self._config.low_watermark

    def try_push(self, event: StreamEvent[T]) -> bool:
        """Attempt to push an event into the buffer.

        Args:
            event: The event to buffer.

        Returns:
            ``True`` if the event was accepted, ``False`` if it was
            dropped.

        Raises:
            BufferFullError: If the strategy is FAIL and the buffer
                is full.
        """
        self._stats.total_events += 1

        with self._lock:
            buffer_size = len(self._buffer)

            if buffer_size >= self._config.buffer_size:
                return self._handle_full_buffer(event)

            if self._config.strategy == BackpressureStrategy.LATEST:
                if buffer_size >= self._config.buffer_size:
                    self._buffer.popleft()
                    self._stats.dropped_events += 1

            self._buffer.append(event)
            self._stats.buffered_events += 1
            self._stats.current_buffer_size = len(self._buffer)

            if self.is_overloaded and not self._stats.high_watermark_reached:
                self._stats.high_watermark_reached = True
                self._stats.overflow_count += 1
                if self._overflow_callback:
                    self._overflow_callback()

            return True

    def _handle_full_buffer(self, event: StreamEvent[T]) -> bool:
        """Handle a push when the buffer is full.

        Args:
            event: The event that could not be buffered.

        Returns:
            ``True`` if the event was accepted (LATEST strategy),
            ``False`` if dropped or rejected.
        """
        if self._config.strategy == BackpressureStrategy.FAIL:
            self._stats.rejected_events += 1
            raise BufferFullError(self._config.buffer_size, event)

        elif self._config.strategy == BackpressureStrategy.DROP:
            self._stats.dropped_events += 1
            return False

        elif self._config.strategy == BackpressureStrategy.LATEST:
            if self._buffer:
                self._buffer.popleft()
                self._stats.dropped_events += 1
            self._buffer.append(event)
            return True

        else:
            self._stats.rejected_events += 1
            return False

    def pop(self) -> Optional[StreamEvent[T]]:
        """Remove and return the next event from the buffer.

        Triggers the resume callback if the buffer transitions from
        above the high watermark to below the low watermark.

        Returns:
            The next :class:`StreamEvent`, or ``None`` if the buffer
            is empty.
        """
        with self._lock:
            if not self._buffer:
                return None

            event = self._buffer.popleft()
            self._stats.buffered_events -= 1
            self._stats.current_buffer_size = len(self._buffer)

            was_overloaded = self._stats.high_watermark_reached
            if was_overloaded and self.is_recovered:
                self._stats.high_watermark_reached = False
                self._stats.resume_count += 1
                if self._resume_callback:
                    self._resume_callback()

            return event

    def peek(self) -> Optional[StreamEvent[T]]:
        """Look at the next event without removing it.

        Returns:
            The next :class:`StreamEvent`, or ``None`` if the buffer
            is empty.
        """
        with self._lock:
            if not self._buffer:
                return None
            return self._buffer[0]

    def clear(self) -> int:
        """Remove all events from the buffer.

        Returns:
            The number of events that were cleared.
        """
        with self._lock:
            count = len(self._buffer)
            self._stats.dropped_events += count
            self._buffer.clear()
            self._stats.buffered_events = 0
            self._stats.current_buffer_size = 0
            self._stats.high_watermark_reached = False
            return count

    def size(self) -> int:
        """Return the current number of events in the buffer.

        Returns:
            Integer buffer size.
        """
        return len(self._buffer)

    def is_empty(self) -> bool:
        """Check whether the buffer is empty.

        Returns:
            ``True`` if there are no buffered events.
        """
        return len(self._buffer) == 0

    def is_full(self) -> bool:
        """Check whether the buffer is at capacity.

        Returns:
            ``True`` if ``len(buffer) >= buffer_size``.
        """
        return len(self._buffer) >= self._config.buffer_size

    def set_overflow_callback(self, callback: Callable[[], None]) -> None:
        """Set a callback invoked when the high watermark is reached.

        Args:
            callback: A no-argument callable.
        """
        self._overflow_callback = callback

    def set_resume_callback(self, callback: Callable[[], None]) -> None:
        """Set a callback invoked when the buffer recovers below the low watermark.

        Args:
            callback: A no-argument callable.
        """
        self._resume_callback = callback

    def reset_stats(self) -> None:
        """Reset statistics counters (except current buffer state)."""
        self._stats.total_events = 0
        self._stats.buffered_events = len(self._buffer)
        self._stats.dropped_events = 0
        self._stats.rejected_events = 0
        self._stats.overflow_count = 0
        self._stats.resume_count = 0


class MultiLevelBackpressure(Generic[T]):
    """Multi-level backpressure with priority queues.

    Events are classified into three priority levels:
    - **HIGH**: Critical events that should never be dropped.
    - **NORMAL**: Regular events.
    - **LOW**: Best-effort events dropped first under pressure.

    Example:
        >>> bp = MultiLevelBackpressure(buffer_size=1000)
        >>> bp.push(event, MultiLevelBackpressure.Priority.HIGH)
        >>> event = bp.pop()
    """

    class Priority:
        """Priority levels for event classification.

        Attributes:
            HIGH: Priority 0 — never dropped if possible.
            NORMAL: Priority 1 — dropped after LOW events.
            LOW: Priority 2 — dropped first under pressure.
        """
        HIGH = 0
        NORMAL = 1
        LOW = 2

    def __init__(self, buffer_size: int = 10000):
        """Initialize with a total buffer capacity across all queues.

        Args:
            buffer_size: Combined maximum events across all priority
                levels.
        """
        self._buffer_size = buffer_size
        self._high: deque = deque()
        self._normal: deque = deque()
        self._low: deque = deque()
        self._lock = threading.Lock()
        self._stats = BackpressureStats()

    def push(self, event: StreamEvent[T], priority: int = Priority.NORMAL) -> bool:
        """Push an event with a specified priority.

        When the total buffer is full, lower-priority events are
        evicted first to make room.

        Args:
            event: The event to push.
            priority: One of ``Priority.HIGH``, ``Priority.NORMAL``,
                or ``Priority.LOW``.

        Returns:
            ``True`` if the event was accepted, ``False`` if dropped.
        """
        with self._lock:
            total = len(self._high) + len(self._normal) + len(self._low)

            if total >= self._buffer_size:
                if self._low:
                    self._low.pop()
                    self._stats.dropped_events += 1
                elif priority == self.Priority.LOW:
                    self._stats.dropped_events += 1
                    return False
                elif self._normal:
                    self._normal.pop()
                    self._stats.dropped_events += 1
                elif priority == self.Priority.NORMAL:
                    self._stats.dropped_events += 1
                    return False
                else:
                    pass

            if priority == self.Priority.HIGH:
                self._high.append(event)
            elif priority == self.Priority.NORMAL:
                self._normal.append(event)
            else:
                self._low.append(event)

            self._stats.total_events += 1
            self._stats.buffered_events = (
                len(self._high) + len(self._normal) + len(self._low)
            )
            return True

    def pop(self) -> Optional[StreamEvent[T]]:
        """Pop the highest-priority event available.

        Events are consumed in priority order: HIGH → NORMAL → LOW.

        Returns:
            The next event, or ``None`` if all queues are empty.
        """
        with self._lock:
            if self._high:
                event = self._high.popleft()
            elif self._normal:
                event = self._normal.popleft()
            elif self._low:
                event = self._low.popleft()
            else:
                return None

            self._stats.buffered_events = (
                len(self._high) + len(self._normal) + len(self._low)
            )
            return event

    def size(self) -> int:
        """Return the total number of buffered events across all queues.

        Returns:
            Integer total buffer size.
        """
        return len(self._high) + len(self._normal) + len(self._low)

    def is_empty(self) -> bool:
        """Check whether all priority queues are empty.

        Returns:
            ``True`` if no events are buffered.
        """
        return self.size() == 0


# ============================================
# Rate-Based Backpressure
# ============================================

class RateBasedBackpressure:
    """Backpressure based on processing rate monitoring.

    Tracks the rate at which events are processed and applies
    backpressure when the rate exceeds a configurable threshold,
    followed by a cooldown period.

    Example:
        >>> rbp = RateBasedBackpressure(max_rate=1000, window_size=10)
        >>> if await rbp.try_acquire():
        ...     await process_event()
    """

    def __init__(
        self,
        max_rate: float,
        window_size: float = 10.0,
        cooldown: float = 1.0,
    ):
        """Initialize the rate-based backpressure controller.

        Args:
            max_rate: Maximum allowed events per second.
            window_size: Sliding window in seconds over which the
                rate is measured.
            cooldown: Minimum time (seconds) to suppress events
                after the rate limit is exceeded.
        """
        self._max_rate = max_rate
        self._window_size = window_size
        self._cooldown = cooldown
        self._timestamps: List[float] = []
        self._lock = asyncio.Lock()
        self._backpressure_active = False
        self._backpressure_until = 0.0

    @property
    def is_backpressure_active(self) -> bool:
        """Check whether backpressure is currently being applied.

        Returns:
            ``True`` if the rate limit has been exceeded and the
                cooldown has not elapsed.
        """
        return self._backpressure_active

    @property
    def current_rate(self) -> float:
        """Return the current processing rate in events/second.

        Returns:
            Float representing the measured rate over the window.
        """
        now = time.time()
        cutoff = now - self._window_size
        recent = [ts for ts in self._timestamps if ts > cutoff]
        if not recent:
            return 0.0
        return len(recent) / self._window_size

    async def try_acquire(self) -> bool:
        """Attempt to acquire permission to process an event.

        Returns:
            ``True`` if processing is allowed, ``False`` if
            backpressure should be applied.
        """
        async with self._lock:
            now = time.time()

            cutoff = now - self._window_size
            self._timestamps = [ts for ts in self._timestamps if ts > cutoff]

            if self._backpressure_active and now < self._backpressure_until:
                return False

            current_rate = len(self._timestamps) / self._window_size
            if current_rate >= self._max_rate:
                self._backpressure_active = True
                self._backpressure_until = now + self._cooldown
                return False

            self._timestamps.append(now)
            self._backpressure_active = False
            return True

    def reset(self) -> None:
        """Clear the rate tracker and deactivate backpressure."""
        self._timestamps.clear()
        self._backpressure_active = False
        self._backpressure_until = 0.0
