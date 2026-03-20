/**
 * Aether SDK Streaming Module
 * 
 * Provides stream processing capabilities for building event-driven applications:
 * - Event-time processing with watermarks
 * - Windowed aggregations (tumbling, sliding, session)
 * - Backpressure handling
 * - Zero-copy messaging for high throughput
 * - Batch processing optimization
 * - Partitioning for parallel processing
 * 
 * @module aether/streaming
 */

// Core types
export {
  Timestamp,
  Duration,
  StreamEvent,
  Watermark,
  WindowType,
  LateDataPolicy,
  WatermarkStrategy,
  BackpressureStrategy,
  DeliverySemantics,
  PaneInfo,
  WindowSpec,
  WindowInfo,
  StreamConfig,
  BackpressureConfig,
  PartitionConfig,
  DeliveryConfig,
  createStreamEvent,
  createStreamEventWithTimestamp,
  tumblingWindowSpec,
  slidingWindowSpec,
  sessionWindowSpec,
  defaultStreamConfig,
  defaultBackpressureConfig,
  defaultPartitionConfig,
  defaultDeliveryConfig,
  PartitionStrategy,
} from './types';

// Zero-copy messaging
export {
  BufferStats,
  MemoryPool,
  PooledBuffer,
  ZeroCopyBuffer,
  RingBuffer,
  ZeroCopyEmitter,
} from './zero_copy';

// Batch processing
export {
  BatchConfig,
  BatchResult,
  BatchStats,
  BatchCollector,
  BatchAggregator,
  BatchEmitter,
  BatchProcessor,
  defaultBatchConfig,
  AggregateFunc,
} from './batch';

// Partitioning
export {
  PartitionConfig as PartitionerConfig,
  PartitionStats,
  Partitioner,
  KeyExtractor,
  PartitionProcessor,
  CompositePartitioner,
  StrategyWeight,
  defaultPartitionConfig as defaultPartitionerConfig,
} from './partition';
