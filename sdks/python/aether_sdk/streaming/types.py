"""
Streaming Types and Enums

Core types for stream processing:
- StreamEvent: Individual event in a stream
- Watermark: Time marker for event progress
- StreamConfig: Configuration for stream actors
- Window types and configurations

Example:
    >>> from aether_sdk.streaming.types import Timestamp, Duration, StreamEvent
    >>> ts = Timestamp.now()
    >>> d = Duration.from_seconds(5)
    >>> event = StreamEvent.create(key="order-1", value={"total": 42.0})
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum, auto
from typing import Any, Dict, Generic, List, Optional, TypeVar, Callable
import time

T = TypeVar('T')


class WindowType(Enum):
    """Types of windowing strategies.

    Attributes:
        TUMBLING: Fixed-size, non-overlapping windows.
        SLIDING: Fixed-size, overlapping windows.
        SESSION: Dynamic windows based on activity gaps.
    """
    TUMBLING = auto()
    SLIDING = auto()
    SESSION = auto()


class LateDataPolicy(Enum):
    """Policy for handling late-arriving data.

    Attributes:
        DROP: Discard events that arrive after the watermark.
        SIDE_OUTPUT: Route late events to a side output stream.
        REPROCESS: Reprocess the affected windows.
    """
    DROP = auto()
    SIDE_OUTPUT = auto()
    REPROCESS = auto()


class WatermarkStrategy(Enum):
    """Strategy for generating watermarks.

    Attributes:
        EVENT_TIME: Watermarks based on event timestamps.
        PROCESSING_TIME: Watermarks based on wall-clock processing time.
        BOUNDED_OUT_OF_ORDER: Allows a bounded amount of lateness.
    """
    EVENT_TIME = auto()
    PROCESSING_TIME = auto()
    BOUNDED_OUT_OF_ORDER = auto()


class BackpressureStrategy(Enum):
    """Strategy for handling backpressure.

    Attributes:
        BUFFER: Buffer events up to a configured limit.
        DROP: Drop new events when overloaded.
        FAIL: Raise an error when overloaded.
        LATEST: Keep only the most recent events, discarding older ones.
    """
    BUFFER = auto()
    DROP = auto()
    FAIL = auto()
    LATEST = auto()


class DeliverySemantics(Enum):
    """Message delivery guarantees.

    Attributes:
        AT_MOST_ONCE: Fire-and-forget; no retries.
        AT_LEAST_ONCE: Retries may cause duplicates.
        EXACTLY_ONCE: No duplicates, no loss (requires checkpointing).
    """
    AT_MOST_ONCE = auto()
    AT_LEAST_ONCE = auto()
    EXACTLY_ONCE = auto()


class PaneInfo(Enum):
    """Window pane type indicating when a window was fired.

    Attributes:
        EARLY: Fired before the watermark reached the window end.
        ON_TIME: Fired when the watermark reached the window end.
        LATE: Fired after the watermark passed the window end.
    """
    EARLY = auto()
    ON_TIME = auto()
    LATE = auto()


@dataclass
class Timestamp:
    """Event timestamp with millisecond precision.

    Supports arithmetic with :class:`Duration` objects and comparison
    operators.

    Attributes:
        milliseconds: Unix epoch time in milliseconds.

    Example:
        >>> ts = Timestamp.now()
        >>> later = ts + Duration.from_seconds(10)
        >>> ts < later
        True
    """
    milliseconds: int

    @classmethod
    def now(cls) -> 'Timestamp':
        """Create a timestamp from the current wall-clock time.

        Returns:
            A :class:`Timestamp` set to the current time.
        """
        return cls(int(time.time() * 1000))

    @classmethod
    def from_datetime(cls, dt: datetime) -> 'Timestamp':
        """Create a timestamp from a :class:`datetime`.

        Args:
            dt: The datetime to convert.

        Returns:
            A :class:`Timestamp` representing the same instant.
        """
        return cls(int(dt.timestamp() * 1000))

    @classmethod
    def from_seconds(cls, seconds: float) -> 'Timestamp':
        """Create a timestamp from a number of seconds since epoch.

        Args:
            seconds: Seconds (may be fractional).

        Returns:
            A :class:`Timestamp`.
        """
        return cls(int(seconds * 1000))

    def to_datetime(self) -> datetime:
        """Convert to a local :class:`datetime`.

        Returns:
            A :class:`datetime` representing the same instant.
        """
        return datetime.fromtimestamp(self.milliseconds / 1000)

    def to_seconds(self) -> float:
        """Convert to seconds since epoch.

        Returns:
            A float representing seconds (may be fractional).
        """
        return self.milliseconds / 1000

    def __add__(self, other: 'Duration') -> 'Timestamp':
        return Timestamp(self.milliseconds + other.milliseconds)

    def __sub__(self, other: 'Timestamp') -> 'Duration':
        return Duration(self.milliseconds - other.milliseconds)

    def __lt__(self, other: 'Timestamp') -> bool:
        return self.milliseconds < other.milliseconds

    def __le__(self, other: 'Timestamp') -> bool:
        return self.milliseconds <= other.milliseconds

    def __gt__(self, other: 'Timestamp') -> bool:
        return self.milliseconds > other.milliseconds

    def __ge__(self, other: 'Timestamp') -> bool:
        return self.milliseconds >= other.milliseconds


@dataclass
class Duration:
    """Duration with millisecond precision.

    Supports addition and scalar multiplication.

    Attributes:
        ms: Duration in milliseconds.

    Example:
        >>> d = Duration.from_minutes(5)
        >>> d.to_seconds()
        300.0
        >>> (d + Duration.from_seconds(10)).to_millis()
        310000
    """
    ms: int

    @property
    def milliseconds(self) -> int:
        """Return the duration in milliseconds."""
        return self.ms

    @classmethod
    def from_timedelta(cls, td: timedelta) -> 'Duration':
        """Create from a :class:`datetime.timedelta`.

        Args:
            td: The timedelta to convert.

        Returns:
            A :class:`Duration`.
        """
        return cls(int(td.total_seconds() * 1000))

    @classmethod
    def from_millis(cls, ms: int) -> 'Duration':
        """Create from milliseconds.

        Args:
            ms: Milliseconds.

        Returns:
            A :class:`Duration`.
        """
        return cls(ms)

    @classmethod
    def from_seconds(cls, s: float) -> 'Duration':
        """Create from seconds.

        Args:
            s: Seconds (may be fractional).

        Returns:
            A :class:`Duration`.
        """
        return cls(int(s * 1000))

    @classmethod
    def from_minutes(cls, m: float) -> 'Duration':
        """Create from minutes.

        Args:
            m: Minutes (may be fractional).

        Returns:
            A :class:`Duration`.
        """
        return cls(int(m * 60 * 1000))

    @classmethod
    def from_hours(cls, h: float) -> 'Duration':
        """Create from hours.

        Args:
            h: Hours (may be fractional).

        Returns:
            A :class:`Duration`.
        """
        return cls(int(h * 3600 * 1000))

    def to_timedelta(self) -> timedelta:
        """Convert to a :class:`datetime.timedelta`.

        Returns:
            A :class:`timedelta` representing the same duration.
        """
        return timedelta(milliseconds=self.ms)

    def to_seconds(self) -> float:
        """Convert to seconds.

        Returns:
            A float representing seconds.
        """
        return self.ms / 1000

    def to_millis(self) -> int:
        """Return the duration in milliseconds.

        Returns:
            Integer milliseconds.
        """
        return self.ms

    def __add__(self, other: 'Duration') -> 'Duration':
        return Duration(self.ms + other.ms)

    def __mul__(self, factor: int) -> 'Duration':
        return Duration(self.ms * factor)


@dataclass
class StreamEvent(Generic[T]):
    """An event in a data stream with associated metadata.

    Attributes:
        key: Partition key for routing.
        value: Event payload.
        timestamp: Event timestamp.
        headers: Optional string key-value headers.
        partition: Source partition number.
        offset: Offset within the partition.
        event_type: Optional type identifier for the event.
    """
    key: str
    value: T
    timestamp: Timestamp
    headers: Dict[str, str] = field(default_factory=dict)
    partition: Optional[int] = None
    offset: Optional[int] = None
    event_type: Optional[str] = None

    @classmethod
    def create(
        cls,
        key: str,
        value: T,
        timestamp: Optional[Timestamp] = None,
        **kwargs
    ) -> 'StreamEvent[T]':
        """Create a new stream event with defaults for optional fields.

        Args:
            key: Partition key.
            value: Event payload.
            timestamp: Event timestamp (defaults to now).
            **kwargs: Additional fields (``headers``, ``partition``,
                ``offset``, ``event_type``).

        Returns:
            A new :class:`StreamEvent` instance.
        """
        return cls(
            key=key,
            value=value,
            timestamp=timestamp or Timestamp.now(),
            **kwargs
        )


@dataclass
class Watermark:
    """A watermark indicating event-time progress for a stream.

    Events with timestamps before the watermark are considered late.

    Attributes:
        timestamp: The watermark timestamp.
        stream_id: Identifier of the stream this watermark belongs to.
        partition: Optional partition number.
    """
    timestamp: Timestamp
    stream_id: str
    partition: Optional[int] = None

    def is_late(self, event_timestamp: Timestamp) -> bool:
        """Check whether an event timestamp is late relative to this watermark.

        Args:
            event_timestamp: The event's timestamp.

        Returns:
            ``True`` if the event is late.
        """
        return event_timestamp < self.timestamp


@dataclass
class WindowSpec:
    """Specification for a windowing strategy.

    Attributes:
        type: The window type (tumbling, sliding, or session).
        size: Window duration.
        slide: Slide interval (sliding windows only).
        gap: Activity gap (session windows only).
        late_tolerance: How late an event can arrive and still be
            included in the window.
        allowed_lateness: Maximum lateness allowed before side-output.
    """
    type: WindowType
    size: Duration
    slide: Optional[Duration] = None
    gap: Optional[Duration] = None
    late_tolerance: Duration = field(default_factory=lambda: Duration.from_seconds(0))
    allowed_lateness: Duration = field(default_factory=lambda: Duration.from_seconds(0))

    def __post_init__(self):
        """Validate the window specification.

        Raises:
            ValueError: If required fields are missing for the window
                type.
        """
        if self.type == WindowType.SLIDING and self.slide is None:
            raise ValueError("Sliding window requires 'slide' parameter")
        if self.type == WindowType.SESSION and self.gap is None:
            raise ValueError("Session window requires 'gap' parameter")


@dataclass
class WindowInfo:
    """Metadata about an active or fired window.

    Attributes:
        start: Window start timestamp.
        end: Window end timestamp.
        max_timestamp: Highest event timestamp in the window.
        pane: Which pane triggered (early, on-time, or late).
        window_id: Optional unique window identifier.
    """
    start: Timestamp
    end: Timestamp
    max_timestamp: Timestamp
    pane: PaneInfo
    window_id: Optional[str] = None

    def contains(self, timestamp: Timestamp) -> bool:
        """Check whether a timestamp falls within this window.

        Args:
            timestamp: The timestamp to check.

        Returns:
            ``True`` if ``start <= timestamp < end``.
        """
        return self.start <= timestamp < self.end

    def is_late(self, timestamp: Timestamp) -> bool:
        """Check whether a timestamp is before this window.

        Args:
            timestamp: The timestamp to check.

        Returns:
            ``True`` if the timestamp is before the window start.
        """
        return timestamp < self.start


@dataclass
class StreamConfig:
    """Configuration for a :class:`~aether_sdk.streaming.stream_actor.StreamActor`.

    Attributes:
        input_streams: Names of input streams to consume.
        output_streams: Names of output streams to produce.
        parallelism: Degree of parallelism for the actor.
        partition_strategy: Partitioning strategy (``"key"``,
            ``"range"``, ``"hash"``, ``"random"``).
        watermark_strategy: Strategy for generating watermarks.
        watermark_interval: Interval between automatic watermarks.
        out_of_orderness: Maximum allowed out-of-order timestamp skew.
        checkpointing_enabled: Whether exactly-once checkpointing
            is active.
        checkpoint_interval: Interval between checkpoints.
        late_data_policy: How to handle late-arriving events.
        late_data_output: Side output stream name for late events.
        buffer_capacity: Maximum buffered events before backpressure.
        buffer_timeout: Maximum time to buffer before flushing.
    """
    input_streams: List[str] = field(default_factory=list)
    output_streams: List[str] = field(default_factory=list)
    parallelism: int = 1
    partition_strategy: str = 'key'
    watermark_strategy: WatermarkStrategy = WatermarkStrategy.PROCESSING_TIME
    watermark_interval: Duration = field(default_factory=lambda: Duration.from_seconds(1))
    out_of_orderness: Duration = field(default_factory=lambda: Duration.from_seconds(0))
    checkpointing_enabled: bool = False
    checkpoint_interval: Duration = field(default_factory=lambda: Duration.from_minutes(1))
    late_data_policy: LateDataPolicy = LateDataPolicy.DROP
    late_data_output: Optional[str] = None
    buffer_capacity: int = 10000
    buffer_timeout: Duration = field(default_factory=lambda: Duration.from_seconds(30))


@dataclass
class BackpressureConfig:
    """Configuration for backpressure handling.

    Attributes:
        strategy: The backpressure strategy to use.
        buffer_size: Maximum number of events to buffer.
        high_watermark: Buffer fill ratio (0–1) at which backpressure
            is signaled.
        low_watermark: Buffer fill ratio (0–1) at which backpressure
            is relieved.
        on_overflow: Callback invoked when the buffer overflows.
        on_resume: Callback invoked when the buffer recovers.
    """
    strategy: BackpressureStrategy = BackpressureStrategy.BUFFER
    buffer_size: int = 10000
    high_watermark: float = 0.9
    low_watermark: float = 0.5
    on_overflow: Optional[Callable[[], None]] = None
    on_resume: Optional[Callable[[], None]] = None


@dataclass
class PartitionConfig:
    """Configuration for stream partitioning.

    Attributes:
        strategy: Partitioning method (``"key"``, ``"range"``,
            ``"hash"``, ``"random"``).
        partitions: Number of partitions.
        key_extractor: Optional callable to extract partition keys
            from events.
    """
    strategy: str = 'key'
    partitions: int = 1
    key_extractor: Optional[Callable[[Any], str]] = None


@dataclass
class DeliveryConfig:
    """Configuration for message delivery guarantees.

    Attributes:
        semantics: The delivery guarantee level.
        max_retries: Maximum delivery retry attempts.
        retry_backoff: Delay between delivery retries.
        dead_letter_topic: Topic for undeliverable messages.
        enable_idempotence: Whether to deduplicate on the producer
            side.
    """
    semantics: DeliverySemantics = DeliverySemantics.AT_LEAST_ONCE
    max_retries: int = 3
    retry_backoff: Duration = field(default_factory=lambda: Duration.from_seconds(1))
    dead_letter_topic: Optional[str] = None
    enable_idempotence: bool = False


EventHandler = Callable[[StreamEvent], None]
BatchHandler = Callable[[List[StreamEvent]], None]
WindowHandler = Callable[[List[StreamEvent], WindowInfo], None]
