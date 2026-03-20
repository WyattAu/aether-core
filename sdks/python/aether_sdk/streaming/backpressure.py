"""
Backpressure Handling

Strategies for handling flow control in stream processing:
- BUFFER: Buffer events up to capacity
- DROP: Drop events when overloaded
- FAIL: Raise error when overloaded
- LATEST: Keep only the latest events
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
    """Statistics for backpressure handling."""
    total_events: int = 0
    buffered_events: int = 0
    dropped_events: int = 0
    rejected_events: int = 0
    overflow_count: int = 0
    resume_count: int = 0
    current_buffer_size: int = 0
    high_watermark_reached: bool = False
    
    def to_dict(self) -> dict:
        """Convert stats to dictionary."""
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
    """Raised when backpressure causes a failure."""
    pass


class BufferFullError(BackpressureError):
    """Raised when buffer is full and strategy is FAIL."""
    
    def __init__(self, buffer_size: int, event: Optional[StreamEvent] = None):
        self.buffer_size = buffer_size
        self.event = event
        super().__init__(
            f"Buffer full (size={buffer_size}). Cannot accept more events."
        )


class BackpressureController(Generic[T]):
    """Controls flow of events through the stream processor.
    
    Strategies:
    - BUFFER: Queue events up to buffer_size, then apply backpressure
    - DROP: Drop new events when buffer is full
    - FAIL: Raise BufferFullError when buffer is full
    - LATEST: Keep only the latest events (circular buffer behavior)
    
    Example:
        >>> config = BackpressureConfig(
        ...     strategy=BackpressureStrategy.BUFFER,
        ...     buffer_size=10000,
        ...     high_watermark=0.0,
        ...     low_watermark=0.0,
        ... )
        >>> controller = BackpressureController(config)
        >>> 
        >>> # Process events
        >>> accepted = controller.try_push(event)
        >>> if accepted:
        ...     # Process the event
        ...     pass
        >>> 
        >>> # Check if we should apply backpressure
        >>> if controller.is_overloaded():
        ...     # Signal upstream to slow down
        ...     pass
    """
    
    def __init__(self, config: Optional[BackpressureConfig] = None):
        self._config = config or BackpressureConfig()
        self._buffer: deque = deque(maxlen=self._config.buffer_size)
        self._stats = BackpressureStats()
        self._overflow_callback: Optional[Callable[[], None]] = None
        self._resume_callback: Optional[Callable[[], None]] = None
        self._lock = threading.Lock()
        
        # Set callbacks from config
        if self._config.on_overflow:
            self._overflow_callback = self._config.on_overflow
        if self._config.on_resume:
            self._resume_callback = self._config.on_resume
    
    @property
    def config(self) -> BackpressureConfig:
        """Get current configuration."""
        return self._config
    
    @property
    def stats(self) -> BackpressureStats:
        """Get current statistics."""
        return self._stats
    
    @property
    def is_overloaded(self) -> bool:
        """Check if buffer is above high watermark."""
        if len(self._buffer) == 0:
            return False
        fill_ratio = len(self._buffer) / self._config.buffer_size
        return fill_ratio >= self._config.high_watermark
    
    @property
    def is_recovered(self) -> bool:
        """Check if buffer is below low watermark."""
        if len(self._buffer) == 0:
            return True
        fill_ratio = len(self._buffer) / self._config.buffer_size
        return fill_ratio <= self._config.low_watermark
    
    def try_push(self, event: StreamEvent[T]) -> bool:
        """Try to push an event to the buffer.
        
        Returns True if the event was accepted, False if dropped.
        Raises BufferFullError if strategy is FAIL and buffer is full.
        
        Args:
            event: The event to push
            
        Returns:
            True if event was accepted, False if dropped
            
        Raises:
            BufferFullError: If strategy is FAIL and buffer is full
        """
        self._stats.total_events += 1
        
        with self._lock:
            buffer_size = len(self._buffer)
            
            # Check if buffer is full
            if buffer_size >= self._config.buffer_size:
                return self._handle_full_buffer(event)
            
            # Buffer has room, add the event
            if self._config.strategy == BackpressureStrategy.LATEST:
                # For LATEST, if buffer is full, remove oldest
                if buffer_size >= self._config.buffer_size:
                    self._buffer.popleft()
                    self._stats.dropped_events += 1
            
            self._buffer.append(event)
            self._stats.buffered_events += 1
            self._stats.current_buffer_size = len(self._buffer)
            
            # Check if we hit high watermark
            if self.is_overloaded and not self._stats.high_watermark_reached:
                self._stats.high_watermark_reached = True
                self._stats.overflow_count += 1
                if self._overflow_callback:
                    self._overflow_callback()
            
            return True
    
    def _handle_full_buffer(self, event: StreamEvent[T]) -> bool:
        """Handle when buffer is full based on strategy."""
        if self._config.strategy == BackpressureStrategy.FAIL:
            self._stats.rejected_events += 1
            raise BufferFullError(self._config.buffer_size, event)
        
        elif self._config.strategy == BackpressureStrategy.DROP:
            self._stats.dropped_events += 1
            return False
        
        elif self._config.strategy == BackpressureStrategy.LATEST:
            # Remove oldest and add new
            if self._buffer:
                self._buffer.popleft()
                self._stats.dropped_events += 1
            self._buffer.append(event)
            return True
        
        else:  # BUFFER
            # Block until space available (shouldn't reach here in try_push)
            self._stats.rejected_events += 1
            return False
    
    def pop(self) -> Optional[StreamEvent[T]]:
        """Pop the next event from the buffer.
        
        Returns:
            The next event, or None if buffer is empty
        """
        with self._lock:
            if not self._buffer:
                return None
            
            event = self._buffer.popleft()
            self._stats.buffered_events -= 1
            self._stats.current_buffer_size = len(self._buffer)
            
            # Check if we recovered below low watermark
            was_overloaded = self._stats.high_watermark_reached
            if was_overloaded and self.is_recovered:
                self._stats.high_watermark_reached = False
                self._stats.resume_count += 1
                if self._resume_callback:
                    self._resume_callback()
            
            return event
    
    def peek(self) -> Optional[StreamEvent[T]]:
        """Peek at the next event without removing it.
        
        Returns:
            The next event, or None if buffer is empty
        """
        with self._lock:
            if not self._buffer:
                return None
            return self._buffer[0]
    
    def clear(self) -> int:
        """Clear all events from buffer.
        
        Returns:
            Number of events that were cleared
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
        """Get current buffer size."""
        return len(self._buffer)
    
    def is_empty(self) -> bool:
        """Check if buffer is empty."""
        return len(self._buffer) == 0
    
    def is_full(self) -> bool:
        """Check if buffer is full."""
        return len(self._buffer) >= self._config.buffer_size
    
    def set_overflow_callback(self, callback: Callable[[], None]) -> None:
        """Set callback for overflow events."""
        self._overflow_callback = callback
    
    def set_resume_callback(self, callback: Callable[[], None]) -> None:
        """Set callback for resume events."""
        self._resume_callback = callback
    
    def reset_stats(self) -> None:
        """Reset statistics counters (except current state)."""
        self._stats.total_events = 0
        self._stats.buffered_events = len(self._buffer)
        self._stats.dropped_events = 0
        self._stats.rejected_events = 0
        self._stats.overflow_count = 0
        self._stats.resume_count = 0


class MultiLevelBackpressure(Generic[T]):
    """Multi-level backpressure with priority queues.
    
    Provides different priority levels for events:
    - HIGH: Critical events that should never be dropped
    - NORMAL: Regular events
    - LOW: Best-effort events that can be dropped first
    
    Example:
        >>> bp = MultiLevelBackpressure(buffer_size=1000)
        >>> bp.push(event, Priority.HIGH)
        >>> bp.push(event, Priority.NORMAL)
        >>> event = bp.pop()
    """
    
    class Priority:
        HIGH = 0
        NORMAL = 1
        LOW = 2
    
    def __init__(self, buffer_size: int = 10000):
        self._buffer_size = buffer_size
        self._high: deque = deque()
        self._normal: deque = deque()
        self._low: deque = deque()
        self._lock = threading.Lock()
        self._stats = BackpressureStats()
    
    def push(self, event: StreamEvent[T], priority: int = Priority.NORMAL) -> bool:
        """Push event with priority.
        
        Args:
            event: The event to push
            priority: Priority level (HIGH, NORMAL, LOW)
            
        Returns:
            True if accepted, False if dropped
        """
        with self._lock:
            total = len(self._high) + len(self._normal) + len(self._low)
            
            if total >= self._buffer_size:
                # Try to drop from lowest priority first
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
                    # HIGH priority, need to accept
                    pass
            
            # Add to appropriate queue
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
        """Pop highest priority event available.
        
        Returns:
            Next event (high priority first), or None if empty
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
        """Get total buffer size."""
        return len(self._high) + len(self._normal) + len(self._low)
    
    def is_empty(self) -> bool:
        """Check if all queues are empty."""
        return self.size() == 0


# ============================================
# Rate-Based Backpressure
# ============================================

class RateBasedBackpressure:
    """Backpressure based on processing rate.
    
    Monitors the rate of events being processed and applies
    backpressure when the rate exceeds the configured threshold.
    
    Example:
        >>> rbp = RateBasedBackpressure(
        ...     max_rate=1000,  # events per second
        ...     window_size=10,  # seconds
        ... )
        >>> if rbp.try_acquire():
        ...     process_event()
    """
    
    def __init__(
        self,
        max_rate: float,
        window_size: float = 10.0,
        cooldown: float = 1.0,
    ):
        self._max_rate = max_rate
        self._window_size = window_size
        self._cooldown = cooldown
        self._timestamps: List[float] = []
        self._lock = asyncio.Lock()
        self._backpressure_active = False
        self._backpressure_until = 0.0
    
    @property
    def is_backpressure_active(self) -> bool:
        """Check if backpressure is currently active."""
        return self._backpressure_active
    
    @property
    def current_rate(self) -> float:
        """Get current processing rate (events/second)."""
        now = time.time()
        cutoff = now - self._window_size
        recent = [ts for ts in self._timestamps if ts > cutoff]
        if not recent:
            return 0.0
        return len(recent) / self._window_size
    
    async def try_acquire(self) -> bool:
        """Try to acquire permission to process.
        
        Returns:
            True if allowed to process, False if should apply backpressure
        """
        async with self._lock:
            now = time.time()
            
            # Clean old timestamps
            cutoff = now - self._window_size
            self._timestamps = [ts for ts in self._timestamps if ts > cutoff]
            
            # Check if in cooldown
            if self._backpressure_active and now < self._backpressure_until:
                return False
            
            # Check if rate exceeded
            current_rate = len(self._timestamps) / self._window_size
            if current_rate >= self._max_rate:
                self._backpressure_active = True
                self._backpressure_until = now + self._cooldown
                return False
            
            # Allow and record
            self._timestamps.append(now)
            self._backpressure_active = False
            return True
    
    def reset(self) -> None:
        """Reset the rate tracker."""
        self._timestamps.clear()
        self._backpressure_active = False
        self._backpressure_until = 0.0
