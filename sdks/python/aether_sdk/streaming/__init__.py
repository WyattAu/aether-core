"""
Aether SDK Streaming Module

Provides stream processing capabilities for building event-driven applications:
- Event-time processing with watermarks
- Windowed aggregations (tumbling, sliding, session)
- Backpressure handling
- Stream actors

Example:
    from aether_sdk.streaming import (
        StreamActor,
        StreamEvent,
        Duration,
        Timestamp,
        TumblingWindow,
        SlidingWindow,
        SessionWindow,
        @tumbling,
        @sliding,
        @session,
    )
    
    # Create a simple stream processor
    class MyProcessor(StreamActor[str, dict]):
        @classmethod
        def name(cls) -> str:
            return "my_processor"
        
        async def process_event(self, event: StreamEvent[dict]) -> None:
            data = event.value
            # Process the data
            result = transform(data)
            await self.emit("output", result)
    
    # Or use windowing decorators
    @tumbling(size=Duration.from_minutes(5))
    def process_window(events: List[StreamEvent], info: WindowInfo) -> Result:
        # Process batch of events in 5-minute windows
        return Result(aggregate=...)
"""

from __future__ import annotations

# Core types
from .types import (
    # Enums
    WindowType,
    LateDataPolicy,
    WatermarkStrategy,
    BackpressureStrategy,
    DeliverySemantics,
    PaneInfo,
    
    # Value types
    Timestamp,
    Duration,
    
    # Event types
    StreamEvent,
    Watermark,
    
    # Configuration types
    WindowSpec,
    WindowInfo,
    StreamConfig,
    BackpressureConfig,
    PartitionConfig,
    DeliveryConfig,
    
    # Handler types
    EventHandler,
    BatchHandler,
    WindowHandler,
)

# Windowing
from .window import (
    # Core classes
    WindowState,
    WindowAssigner,
    WindowTrigger,
    
    # Convenience classes
    TumblingWindow,
    SlidingWindow,
    SessionWindow,
    
    # Decorators
    window,
    tumbling,
    sliding,
    session,
)

# Backpressure
from .backpressure import (
    BackpressureStats,
    BackpressureError,
    BufferFullError,
    BackpressureController,
    MultiLevelBackpressure,
    RateBasedBackpressure,
)

# Stream Actor
from .stream_actor import (
    StreamState,
    StreamingStateHandle,
    StreamActor,
)

# Zero-Copy Messaging (M4 Performance)
from .zero_copy import (
    BufferStats,
    MemoryPool,
    PooledBuffer,
    ZeroCopyBuffer,
    RingBuffer,
    ZeroCopyEmitter,
)

# Batch Processing (M4 Performance)
from .batch import (
    BatchConfig,
    BatchResult,
    BatchStats,
    BatchCollector,
    BatchAggregator,
    BatchEmitter,
    BatchProcessor,
)

# Partitioning (M4 Performance)
from .partition import (
    PartitionStrategy,
    PartitionConfig,
    Partitioner,
    PartitionProcessor,
    CompositePartitioner,
    KeyExtractor,
)


# ============================================
# Module Exports
# ============================================

__all__ = [
    # Enums
    'WindowType',
    'LateDataPolicy',
    'WatermarkStrategy',
    'BackpressureStrategy',
    'DeliverySemantics',
    'PaneInfo',
    
    # Value types
    'Timestamp',
    'Duration',
    
    # Event types
    'StreamEvent',
    'Watermark',
    
    # Configuration types
    'WindowSpec',
    'WindowInfo',
    'StreamConfig',
    'BackpressureConfig',
    'PartitionConfig',
    'DeliveryConfig',
    
    # Handler types
    'EventHandler',
    'BatchHandler',
    'WindowHandler',
    
    # Windowing
    'WindowState',
    'WindowAssigner',
    'WindowTrigger',
    'TumblingWindow',
    'SlidingWindow',
    'SessionWindow',
    'window',
    'tumbling',
    'sliding',
    'session',
    
    # Backpressure
    'BackpressureStats',
    'BackpressureError',
    'BufferFullError',
    'BackpressureController',
    'MultiLevelBackpressure',
    'RateBasedBackpressure',
    
    # Stream Actor
    'StreamState',
    'StreamingStateHandle',
    'StreamActor',
    
    # Zero-Copy Messaging (M4 Performance)
    'BufferStats',
    'MemoryPool',
    'PooledBuffer',
    'ZeroCopyBuffer',
    'RingBuffer',
    'ZeroCopyEmitter',
    
    # Batch Processing (M4 Performance)
    'BatchConfig',
    'BatchResult',
    'BatchStats',
    'BatchCollector',
    'BatchAggregator',
    'BatchEmitter',
    'BatchProcessor',
    
    # Partitioning (M4 Performance)
    'PartitionStrategy',
    'Partitioner',
    'PartitionProcessor',
    'CompositePartitioner',
    'KeyExtractor',
]
