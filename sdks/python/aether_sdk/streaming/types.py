"""
Streaming Types and Enums

Core types for stream processing:
- StreamEvent: Individual event in a stream
- Watermark: Time marker for event progress
- StreamConfig: Configuration for stream actors
- Window types and configurations
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum, auto
from typing import Any, Dict, Generic, List, Optional, TypeVar, Callable
import time

T = TypeVar('T')


class WindowType(Enum):
    """Types of windowing strategies."""
    TUMBLING = auto()   # Fixed-size, non-overlapping
    SLIDING = auto()    # Fixed-size, overlapping
    SESSION = auto()    # Dynamic size based on activity gaps


class LateDataPolicy(Enum):
    """How to handle late-arriving data."""
    DROP = auto()           # Discard late events
    SIDE_OUTPUT = auto()    # Route to side output stream
    REPROCESS = auto()      # Reprocess affected windows


class WatermarkStrategy(Enum):
    """Watermark generation strategy."""
    EVENT_TIME = auto()     # Based on event timestamps
    PROCESSING_TIME = auto()  # Based on processing time
    BOUNDED_OUT_OF_ORDER = auto()  # Allow bounded lateness


class BackpressureStrategy(Enum):
    """Backpressure handling strategies."""
    BUFFER = auto()     # Buffer events up to limit
    DROP = auto()       # Drop events when overloaded
    FAIL = auto()       # Raise error when overloaded
    LATEST = auto()     # Keep only latest events


class DeliverySemantics(Enum):
    """Message delivery guarantees."""
    AT_MOST_ONCE = auto()    # Fire and forget
    AT_LEAST_ONCE = auto()   # May duplicate
    EXACTLY_ONCE = auto()    # No duplicates, no loss


class PaneInfo(Enum):
    """Window pane type."""
    EARLY = auto()       # Early firing before watermark
    ON_TIME = auto()     # On-time firing at watermark
    LATE = auto()        # Late firing after watermark


@dataclass
class Timestamp:
    """Event timestamp with millisecond precision."""
    milliseconds: int
    
    @classmethod
    def now(cls) -> 'Timestamp':
        """Create timestamp from current time."""
        return cls(int(time.time() * 1000))
    
    @classmethod
    def from_datetime(cls, dt: datetime) -> 'Timestamp':
        """Create timestamp from datetime."""
        return cls(int(dt.timestamp() * 1000))
    
    @classmethod
    def from_seconds(cls, seconds: float) -> 'Timestamp':
        """Create timestamp from seconds."""
        return cls(int(seconds * 1000))
    
    def to_datetime(self) -> datetime:
        """Convert to datetime."""
        return datetime.fromtimestamp(self.milliseconds / 1000)
    
    def to_seconds(self) -> float:
        """Convert to seconds."""
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
    """Duration with millisecond precision."""
    ms: int  # milliseconds stored here
    
    @property
    def milliseconds(self) -> int:
        """Get milliseconds."""
        return self.ms
    
    @classmethod
    def from_timedelta(cls, td: timedelta) -> 'Duration':
        """Create duration from timedelta."""
        return cls(int(td.total_seconds() * 1000))
    
    @classmethod
    def from_millis(cls, ms: int) -> 'Duration':
        """Create duration from milliseconds."""
        return cls(ms)
    
    @classmethod
    def from_seconds(cls, s: float) -> 'Duration':
        """Create duration from seconds."""
        return cls(int(s * 1000))
    
    @classmethod
    def from_minutes(cls, m: float) -> 'Duration':
        """Create duration from minutes."""
        return cls(int(m * 60 * 1000))
    
    @classmethod
    def from_hours(cls, h: float) -> 'Duration':
        """Create duration from hours."""
        return cls(int(h * 3600 * 1000))
    
    def to_timedelta(self) -> timedelta:
        """Convert to timedelta."""
        return timedelta(milliseconds=self.ms)
    
    def to_seconds(self) -> float:
        """Convert to seconds."""
        return self.ms / 1000
    
    def to_millis(self) -> int:
        """Convert to milliseconds."""
        return self.ms
    
    def __add__(self, other: 'Duration') -> 'Duration':
        return Duration(self.ms + other.ms)
    
    def __mul__(self, factor: int) -> 'Duration':
        return Duration(self.ms * factor)


@dataclass
class StreamEvent(Generic[T]):
    """Event in a stream with metadata."""
    key: str                           # Partition key
    value: T                           # Event payload
    timestamp: Timestamp               # Event timestamp
    headers: Dict[str, str] = field(default_factory=dict)
    partition: Optional[int] = None    # Partition number
    offset: Optional[int] = None       # Offset in partition
    event_type: Optional[str] = None   # Event type identifier
    
    @classmethod
    def create(
        cls,
        key: str,
        value: T,
        timestamp: Optional[Timestamp] = None,
        **kwargs
    ) -> 'StreamEvent[T]':
        """Create a new stream event."""
        return cls(
            key=key,
            value=value,
            timestamp=timestamp or Timestamp.now(),
            **kwargs
        )


@dataclass
class Watermark:
    """Watermark indicating event time progress."""
    timestamp: Timestamp
    stream_id: str
    partition: Optional[int] = None
    
    def is_late(self, event_timestamp: Timestamp) -> bool:
        """Check if an event timestamp is late relative to this watermark."""
        return event_timestamp < self.timestamp


@dataclass
class WindowSpec:
    """Window specification for stream processing."""
    type: WindowType
    size: Duration
    slide: Optional[Duration] = None   # For sliding windows
    gap: Optional[Duration] = None     # For session windows
    late_tolerance: Duration = field(default_factory=lambda: Duration.from_seconds(0))
    allowed_lateness: Duration = field(default_factory=lambda: Duration.from_seconds(0))
    
    def __post_init__(self):
        """Validate window specification."""
        if self.type == WindowType.SLIDING and self.slide is None:
            raise ValueError("Sliding window requires 'slide' parameter")
        if self.type == WindowType.SESSION and self.gap is None:
            raise ValueError("Session window requires 'gap' parameter")


@dataclass
class WindowInfo:
    """Information about an active window."""
    start: Timestamp
    end: Timestamp
    max_timestamp: Timestamp
    pane: PaneInfo
    window_id: Optional[str] = None
    
    def contains(self, timestamp: Timestamp) -> bool:
        """Check if timestamp falls within this window."""
        return self.start <= timestamp < self.end
    
    def is_late(self, timestamp: Timestamp) -> bool:
        """Check if timestamp is late for this window."""
        return timestamp < self.start


@dataclass
class StreamConfig:
    """Configuration for stream actors."""
    # Input/Output
    input_streams: List[str] = field(default_factory=list)
    output_streams: List[str] = field(default_factory=list)
    
    # Parallelism
    parallelism: int = 1
    partition_strategy: str = 'key'  # key, range, hash, random
    
    # Watermark
    watermark_strategy: WatermarkStrategy = WatermarkStrategy.PROCESSING_TIME
    watermark_interval: Duration = field(default_factory=lambda: Duration.from_seconds(1))
    out_of_orderness: Duration = field(default_factory=lambda: Duration.from_seconds(0))
    
    # Checkpointing
    checkpointing_enabled: bool = False
    checkpoint_interval: Duration = field(default_factory=lambda: Duration.from_minutes(1))
    
    # Late data
    late_data_policy: LateDataPolicy = LateDataPolicy.DROP
    late_data_output: Optional[str] = None  # Side output stream for late data
    
    # Buffering
    buffer_capacity: int = 10000
    buffer_timeout: Duration = field(default_factory=lambda: Duration.from_seconds(30))


@dataclass
class BackpressureConfig:
    """Configuration for backpressure handling."""
    strategy: BackpressureStrategy = BackpressureStrategy.BUFFER
    buffer_size: int = 10000
    high_watermark: float = 0.9   # 90% full
    low_watermark: float = 0.5    # 50% full
    
    # Callbacks
    on_overflow: Optional[Callable[[], None]] = None
    on_resume: Optional[Callable[[], None]] = None


@dataclass
class PartitionConfig:
    """Configuration for stream partitioning."""
    strategy: str = 'key'  # key, range, hash, random
    partitions: int = 1
    key_extractor: Optional[Callable[[Any], str]] = None


@dataclass
class DeliveryConfig:
    """Configuration for message delivery guarantees."""
    semantics: DeliverySemantics = DeliverySemantics.AT_LEAST_ONCE
    max_retries: int = 3
    retry_backoff: Duration = field(default_factory=lambda: Duration.from_seconds(1))
    dead_letter_topic: Optional[str] = None
    enable_idempotence: bool = False


# Type aliases
EventHandler = Callable[[StreamEvent], None]
BatchHandler = Callable[[List[StreamEvent]], None]
WindowHandler = Callable[[List[StreamEvent], WindowInfo], None]
