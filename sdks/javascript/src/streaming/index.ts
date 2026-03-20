/**
 * Aether SDK Streaming Module
 *
 * Provides stream processing capabilities for building event-driven applications:
 * - Event-time processing with watermarks
 * - Windowed aggregations (tumbling, sliding, session)
 * - Backpressure handling
 * - Stream actors
 *
 * @example
 * ```typescript
 * import {
 *   StreamActor,
 *   StreamEvent,
 *   Duration,
 *   Timestamp,
 *   TumblingWindow,
 *   BackpressureController,
 * } from 'aether-sdk/streaming';
 *
 * class MyProcessor extends StreamActor<string, Event> {
 *   async processEvent(event: StreamEvent<Event>): Promise<void> {
 *     const data = event.value;
 *     await this.emit('output', transform(data));
 *   }
 * }
 * ```
 *
 * @module aether/streaming
 */

// Re-export types
export {
  // Enums
  WindowType,
  LateDataPolicy,
  WatermarkStrategy,
  BackpressureStrategy,
  DeliverySemantics,
  PaneInfo,

  // Value types
  Timestamp,
  Duration,

  // Event types
  StreamEvent,
  Watermark,

  // Configuration types
  WindowSpec,
  WindowInfo,
  StreamConfig,
  BackpressureConfig,
  PartitionConfig,
  DeliveryConfig,

  // Handler types
  EventHandler,
  BatchHandler,
  WindowHandler,
} from './types';

// Re-export window
export {
  WindowState,
  WindowAssigner,
  WindowTrigger,
  TumblingWindow,
  SlidingWindow,
  SessionWindow,
  window,
  tumbling,
  sliding,
  session,
} from './window';

// Re-export backpressure
export {
  BackpressureStats,
  BackpressureError,
  BufferFullError,
  BackpressureController,
  MultiLevelBackpressure,
  RateBasedBackpressure,
  DEFAULT_BACKPRESSURE_CONFIG,
} from './backpressure';

// Re-export stream actor
export {
  StreamingStateHandle,
  StreamActor,
} from './stream_actor';
