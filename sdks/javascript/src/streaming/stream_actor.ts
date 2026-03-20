/**
 * Stream Actor Base Class
 *
 * Extends the base Actor with stream processing capabilities:
 * - Event-time processing with watermarks
 * - Windowed aggregation
 * - Backpressure handling
 * - State management for streaming
 *
 * @module aether/streaming/stream_actor
 */

import { Actor } from '../actor';
import { Message, MessageType } from '../messaging';
import { StateHandle } from '../state';
import {
  StreamEvent,
  Watermark,
  Timestamp,
  Duration,
  WindowSpec,
  WindowInfo,
  StreamConfig,
  BackpressureConfig,
  LateDataPolicy,
} from './types';
import { BackpressureController } from './backpressure';
import { WindowAssigner, WindowTrigger } from './window';

/**
 * Internal state for stream processing.
 */
interface StreamState {
  watermarks: Map<string, Timestamp>;
  processedCount: number;
  lateEventsCount: number;
  lastProcessedTimestamp?: Timestamp;
}

/**
 * Enhanced state handle for streaming operations.
 *
 * Provides typed state access methods commonly needed in stream processing:
 * - Value state: Single value per key
 * - List state: Accumulated values
 * - Map state: Key-value mappings
 */
export class StreamingStateHandle {
  constructor(private state: StateHandle) {}

  /**
   * Get a single value from state.
   */
  async getValue<T>(name: string, defaultValue?: T): Promise<T | undefined> {
    const value = await this.state.get(name);
    if (value === null || value === undefined) {
      return defaultValue;
    }
    return JSON.parse(value as string) as T;
  }

  /**
   * Set a single value in state.
   */
  async setValue<T>(name: string, value: T): Promise<void> {
    await this.state.set(name, JSON.stringify(value));
  }

  /**
   * Get a list from state.
   */
  async getList<T>(name: string): Promise<T[]> {
    const value = await this.state.get(name);
    if (value === null || value === undefined) {
      return [];
    }
    const parsed = JSON.parse(value as string);
    return Array.isArray(parsed) ? parsed : [parsed];
  }

  /**
   * Append an item to a list in state.
   */
  async appendToList<T>(name: string, item: T): Promise<void> {
    const list = await this.getList<T>(name);
    list.push(item);
    await this.state.set(name, JSON.stringify(list));
  }

  /**
   * Clear a list in state.
   */
  async clearList(name: string): Promise<void> {
    await this.state.set(name, JSON.stringify([]));
  }

  /**
   * Get a map/dict from state.
   */
  async getMap<K extends string, V>(name: string): Promise<Record<K, V>> {
    const value = await this.state.get(name);
    if (value === null || value === undefined) {
      return {} as Record<K, V>;
    }
    return JSON.parse(value as string) as Record<K, V>;
  }

  /**
   * Put a key-value pair in a map.
   */
  async putInMap<K extends string, V>(name: string, key: K, value: V): Promise<void> {
    const map = await this.getMap<K, V>(name);
    map[key] = value;
    await this.state.set(name, JSON.stringify(map));
  }

  /**
   * Remove a key from a map.
   */
  async removeFromMap<K extends string, V>(name: string, key: K): Promise<V | undefined> {
    const map = await this.getMap<K, V>(name);
    const value = map[key];
    delete map[key];
    await this.state.set(name, JSON.stringify(map));
    return value;
  }

  /**
   * Clear a map in state.
   */
  async clearMap(name: string): Promise<void> {
    await this.state.set(name, JSON.stringify({}));
  }
}

/**
 * Stream Actor
 *
 * Base class for stream processing actors.
 *
 * @example
 * ```typescript
 * class MyStreamProcessor extends StreamActor<string, Event> {
 *   static get name(): string {
 *     return 'my_stream_processor';
 *   }
 *
 *   async processEvent(event: StreamEvent<Event>): Promise<void> {
 *     const data = event.value;
 *     await this.emit('output', transform(data));
 *   }
 * }
 * ```
 */
export abstract class StreamActor<K = string, V = unknown> extends Actor {
  protected streamConfig: StreamConfig;
  protected streamState: StreamState;
  protected streamingState?: StreamingStateHandle;
  protected backpressure: BackpressureController<V>;

  private windowAssigner?: WindowAssigner<K, V>;
  private windowTrigger?: WindowTrigger<K, V, unknown>;
  private outputHandlers: Map<string, (event: StreamEvent) => void> = new Map();
  private lateDataHandler?: (event: StreamEvent<V>) => void;

  constructor(
    config?: StreamConfig,
    backpressureConfig?: BackpressureConfig
  ) {
    super({ name: 'stream-actor' });
    
    this.streamConfig = config || {
      inputStreams: [],
      outputStreams: [],
      parallelism: 1,
      partitionStrategy: 'key',
    };
    
    this.streamState = {
      watermarks: new Map(),
      processedCount: 0,
      lateEventsCount: 0,
    };
    
    this.backpressure = new BackpressureController<V>(
      backpressureConfig || {
        strategy: 'buffer' as BackpressureStrategy,
        bufferSize: 10000,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      }
    );
  }

  /**
   * Get streaming state handle.
   */
  get streamingStateHandle(): StreamingStateHandle {
    if (!this.streamingState) {
      this.streamingState = new StreamingStateHandle(this.state);
    }
    return this.streamingState;
  }

  /**
   * Process a single stream event.
   *
   * Override this method to implement event processing logic.
   */
  abstract processEvent(event: StreamEvent<V>): Promise<void>;

  /**
   * Handle incoming message.
   */
  async handle(sender: string, message: Message): Promise<Message | void> {
    if (message.type === 'stream_event' || message.type === MessageType.Custom) {
      const eventData = message.payload;
      if (this.isStreamEvent(eventData)) {
        await this.processWithBackpressure(eventData);
      } else if (typeof eventData === 'object' && eventData !== null) {
        const event = this.dictToEvent(eventData as Record<string, unknown>);
        if (event) {
          await this.processWithBackpressure(event);
        }
      }
    } else if (message.type === 'watermark') {
      const watermarkData = message.payload;
      if (this.isWatermark(watermarkData)) {
        await this.advanceWatermark(watermarkData);
      } else if (typeof watermarkData === 'object' && watermarkData !== null) {
        const watermark: Watermark = {
          timestamp: new Timestamp((watermarkData as Record<string, unknown>).timestamp as number),
          streamId: (watermarkData as Record<string, unknown>).streamId as string,
          partition: (watermarkData as Record<string, unknown>).partition as number | undefined,
        };
        await this.advanceWatermark(watermark);
      }
    }
  }

  private isStreamEvent(obj: unknown): obj is StreamEvent<V> {
    return (
      typeof obj === 'object' &&
      obj !== null &&
      'key' in obj &&
      'value' in obj &&
      'timestamp' in obj
    );
  }

  private isWatermark(obj: unknown): obj is Watermark {
    return (
      typeof obj === 'object' &&
      obj !== null &&
      'timestamp' in obj &&
      'streamId' in obj
    );
  }

  private dictToEvent(data: Record<string, unknown>): StreamEvent<V> | undefined {
    try {
      return {
        key: data.key as string,
        value: data.value as V,
        timestamp: new Timestamp(data.timestamp as number),
        headers: (data.headers as Record<string, string>) || {},
        partition: data.partition as number | undefined,
        offset: data.offset as number | undefined,
        eventType: data.eventType as string | undefined,
      };
    } catch {
      return undefined;
    }
  }

  private async processWithBackpressure(event: StreamEvent<V>): Promise<void> {
    if (!this.backpressure.tryPush(event)) {
      return;
    }

    while (true) {
      const bufferedEvent = this.backpressure.pop();
      if (bufferedEvent === undefined) break;

      try {
        await this.processEventInternal(bufferedEvent);
      } catch (error) {
        console.error('Error processing event:', error);
      }
    }
  }

  private async processEventInternal(event: StreamEvent<V>): Promise<void> {
    this.streamState.processedCount++;

    // Check if event is late
    const currentWatermark = this.streamState.watermarks.get(
      event.eventType || 'default'
    ) || new Timestamp(0);

    if (event.timestamp.milliseconds < currentWatermark.milliseconds) {
      this.streamState.lateEventsCount++;
      await this.handleLateEvent(event);
      return;
    }

    // Process through windowing if configured
    if (this.windowTrigger) {
      const key = this.extractKey(event);
      const results = this.windowTrigger.process(event, key);
      
      for (const result of results) {
        await this.emit('window_output', result);
      }
    }

    // Call user's processEvent
    await this.processEvent(event);

    // Update last processed timestamp
    this.streamState.lastProcessedTimestamp = event.timestamp;
  }

  protected extractKey(event: StreamEvent<V>): K {
    return event.key as unknown as K;
  }

  protected async handleLateEvent(event: StreamEvent<V>): Promise<void> {
    const policy = this.streamConfig.lateDataPolicy || LateDataPolicy.Drop;

    switch (policy) {
      case LateDataPolicy.Drop:
        return;

      case LateDataPolicy.SideOutput:
        if (this.lateDataHandler) {
          this.lateDataHandler(event);
        } else if (this.streamConfig.lateDataOutput) {
          await this.emit(this.streamConfig.lateDataOutput, event);
        }
        break;

      case LateDataPolicy.Reprocess:
        if (this.windowAssigner) {
          const key = this.extractKey(event);
          this.windowAssigner.assign(event, key);
        }
        break;
    }
  }

  /**
   * Advance watermark for a stream.
   */
  async advanceWatermark(watermark: Watermark): Promise<void> {
    const streamId = watermark.streamId;
    const oldWatermark = this.streamState.watermarks.get(streamId);

    // Only advance if new watermark is ahead
    if (!oldWatermark || watermark.timestamp.milliseconds > oldWatermark.milliseconds) {
      this.streamState.watermarks.set(streamId, watermark.timestamp);

      // Fire any windows triggered by this watermark
      if (this.windowTrigger) {
        const results = this.windowTrigger.advanceWatermark(watermark.timestamp);
        for (const result of results) {
          await this.emit('window_output', result);
        }
      }
    }
  }

  /**
   * Get current watermark for a stream.
   */
  getWatermark(streamId: string): Timestamp | undefined {
    return this.streamState.watermarks.get(streamId);
  }

  /**
   * Emit a value to an output stream.
   */
  async emit(stream: string, value: unknown): Promise<void> {
    const event: StreamEvent<unknown> = {
      key: String(this.hashValue(value)),
      value,
      timestamp: Timestamp.now(),
      headers: {},
    };
    await this.doEmit(stream, event);
  }

  /**
   * Emit a value with specific timestamp.
   */
  async emitWithTimestamp(
    stream: string,
    value: unknown,
    timestamp: Timestamp
  ): Promise<void> {
    const event: StreamEvent<unknown> = {
      key: String(this.hashValue(value)),
      value,
      timestamp,
      headers: {},
    };
    await this.doEmit(stream, event);
  }

  /**
   * Emit a pre-constructed stream event.
   */
  async emitEvent(stream: string, event: StreamEvent): Promise<void> {
    await this.doEmit(stream, event);
  }

  private async doEmit(stream: string, event: StreamEvent): Promise<void> {
    if (this.outputHandlers.has(stream)) {
      const handler = this.outputHandlers.get(stream)!;
      handler(event);
    }
  }

  private hashValue(value: unknown): number {
    const str = JSON.stringify(value);
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash;
    }
    return hash;
  }

  /**
   * Register a handler for output stream.
   */
  registerOutputHandler(stream: string, handler: (event: StreamEvent) => void): void {
    this.outputHandlers.set(stream, handler);
  }

  /**
   * Register handler for late-arriving data.
   */
  registerLateDataHandler(handler: (event: StreamEvent<V>) => void): void {
    this.lateDataHandler = handler;
  }

  /**
   * Configure windowing for this stream actor.
   */
  configureWindow<K2, R>(
    spec: WindowSpec,
    handler: (events: StreamEvent<V>[], info: WindowInfo) => R
  ): void {
    this.windowAssigner = new WindowAssigner<K2, V>(spec);
    this.windowTrigger = new WindowTrigger<K2, V, R>(this.windowAssigner, handler as any);
  }

  /**
   * Get value state.
   */
  async getState<T>(name: string, defaultValue?: T): Promise<T | undefined> {
    return this.streamingStateHandle.getValue(name, defaultValue);
  }

  /**
   * Set value state.
   */
  async setState<T>(name: string, value: T): Promise<void> {
    await this.streamingStateHandle.setValue(name, value);
  }

  /**
   * Get list state.
   */
  async getListState<T>(name: string): Promise<T[]> {
    return this.streamingStateHandle.getList<T>(name);
  }

  /**
   * Update list state.
   */
  async updateListState<T>(name: string, item: T): Promise<void> {
    await this.streamingStateHandle.appendToList(name, item);
  }

  /**
   * Get map state.
   */
  async getMapState<K extends string, V>(name: string): Promise<Record<K, V>> {
    return this.streamingStateHandle.getMap<K, V>(name);
  }

  /**
   * Update map state.
   */
  async updateMapState<K extends string, V>(name: string, key: K, value: V): Promise<void> {
    await this.streamingStateHandle.putInMap(name, key, value);
  }

  /**
   * Get stream processing metrics.
   */
  getMetrics(): Record<string, unknown> {
    return {
      processedCount: this.streamState.processedCount,
      lateEventsCount: this.streamState.lateEventsCount,
      lastProcessedTimestamp: this.streamState.lastProcessedTimestamp?.milliseconds ?? null,
      watermarks: Object.fromEntries(
        Array.from(this.streamState.watermarks.entries()).map(([k, v]) => [k, v.milliseconds])
      ),
      backpressure: this.backpressure.getStats(),
    };
  }
}
