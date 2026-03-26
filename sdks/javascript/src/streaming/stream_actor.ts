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
  BackpressureStrategy,
  LateDataPolicy,
  createStreamConfig,
} from './types';
import { BackpressureController } from './backpressure';
import { WindowAssigner, WindowTrigger } from './window';

/**
 * Internal state for stream processing.
 * @internal
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
 * - **Value state**: Single value per key
 * - **List state**: Accumulated values
 * - **Map state**: Key-value mappings
 *
 * @example
 * ```typescript
 * const handle = new StreamingStateHandle(actorState);
 * await handle.setValue('counter', 0);
 * await handle.appendToList('events', { type: 'click' });
 * await handle.putInMap('sessions', 'abc123', { started: Date.now() });
 * ```
 */
export class StreamingStateHandle {
  /**
   * Create a new StreamingStateHandle wrapping a base StateHandle.
   *
   * @param state - The underlying state handle.
   */
  constructor(private state: StateHandle) {}

  /**
   * Get a single typed value from state.
   *
   * @typeParam T - The expected value type.
   * @param name         - The state key.
   * @param defaultValue - Optional default returned when the key is absent.
   * @returns The stored value, or `defaultValue` if not found.
   */
  async getValue<T>(name: string, defaultValue?: T): Promise<T | undefined> {
    const value = await this.state.getString(name);
    if (value === null || value === undefined) {
      return defaultValue;
    }
    return JSON.parse(value) as T;
  }

  /**
   * Set a single typed value in state.
   *
   * @typeParam T - The value type.
   * @param name  - The state key.
   * @param value - The value to store.
   */
  async setValue<T>(name: string, value: T): Promise<void> {
    await this.state.setString(name, JSON.stringify(value));
  }

  /**
   * Get a list from state.
   *
   * @typeParam T - The element type.
   * @param name - The state key.
   * @returns The stored list, or an empty array if not found.
   */
  async getList<T>(name: string): Promise<T[]> {
    const value = await this.state.getString(name);
    if (value === null || value === undefined) {
      return [];
    }
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [parsed];
  }

  /**
   * Append an item to a list in state.
   *
   * @typeParam T - The element type.
   * @param name - The state key.
   * @param item - The item to append.
   */
  async appendToList<T>(name: string, item: T): Promise<void> {
    const list = await this.getList<T>(name);
    list.push(item);
    await this.state.setString(name, JSON.stringify(list));
  }

  /**
   * Clear a list in state (sets it to an empty array).
   *
   * @param name - The state key.
   */
  async clearList(name: string): Promise<void> {
    await this.state.setString(name, JSON.stringify([]));
  }

  /**
   * Get a map/dictionary from state.
   *
   * @typeParam K - The key type (must extend string).
   * @typeParam V - The value type.
   * @param name - The state key.
   * @returns The stored map, or an empty object if not found.
   */
  async getMap<K extends string, V>(name: string): Promise<Record<K, V>> {
    const value = await this.state.getString(name);
    if (value === null || value === undefined) {
      return {} as Record<K, V>;
    }
    return JSON.parse(value) as Record<K, V>;
  }

  /**
   * Put a key-value pair into a map in state.
   *
   * @typeParam K - The key type (must extend string).
   * @typeParam V - The value type.
   * @param name  - The state key for the map.
   * @param key   - The key within the map.
   * @param value - The value to store.
   */
  async putInMap<K extends string, V>(name: string, key: K, value: V): Promise<void> {
    const map = await this.getMap<K, V>(name);
    map[key] = value;
    await this.state.setString(name, JSON.stringify(map));
  }

  /**
   * Remove a key from a map in state.
   *
   * @typeParam K - The key type (must extend string).
   * @typeParam V - The value type.
   * @param name - The state key for the map.
   * @param key  - The key to remove.
   * @returns The previous value, or `undefined` if the key did not exist.
   */
  async removeFromMap<K extends string, V>(name: string, key: K): Promise<V | undefined> {
    const map = await this.getMap<K, V>(name);
    const value = map[key];
    delete map[key];
    await this.state.setString(name, JSON.stringify(map));
    return value;
  }

  /**
   * Clear a map in state (sets it to an empty object).
   *
   * @param name - The state key.
   */
  async clearMap(name: string): Promise<void> {
    await this.state.setString(name, JSON.stringify({}));
  }
}

/**
 * Stream Actor
 *
 * Base class for stream processing actors that extends the core {@link Actor}
 * with event-time processing, watermark tracking, windowed aggregation,
 * backpressure handling, and typed streaming state.
 *
 * @typeParam K - The key type for partitioning (default: `string`).
 * @typeParam V - The event payload type (default: `unknown`).
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
 *     await this.emitValue('output', transform(data));
 *   }
 * }
 * ```
 */
export abstract class StreamActor<K = string, V = unknown> extends Actor {
  /** The stream processing configuration. */
  protected streamConfig: StreamConfig;
  /** Internal stream processing state. */
  protected streamState: StreamState;
  /** Lazily initialized typed streaming state handle. */
  protected streamingState?: StreamingStateHandle;
  /** Backpressure controller for flow management. */
  protected backpressure: BackpressureController<V>;

  private windowAssigner?: WindowAssigner<K, V>;
  private windowTrigger?: WindowTrigger<K, V, unknown>;
  private outputHandlers: Map<string, (event: StreamEvent<unknown>) => void> = new Map();
  private lateDataHandler?: (event: StreamEvent<V>) => void;

  /**
   * Create a new StreamActor.
   *
   * @param config             - Stream configuration (uses defaults if omitted).
   * @param backpressureConfig - Backpressure configuration (uses defaults if omitted).
   */
  constructor(
    config?: StreamConfig,
    backpressureConfig?: BackpressureConfig
  ) {
    super({ name: 'stream-actor' });
    
    this.streamConfig = config || createStreamConfig();
    
    this.streamState = {
      watermarks: new Map(),
      processedCount: 0,
      lateEventsCount: 0,
    };
    
    this.backpressure = new BackpressureController<V>(
      backpressureConfig || {
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10000,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      }
    );
  }

  /**
   * Get the lazily initialized streaming state handle.
   *
   * @returns The {@link StreamingStateHandle} for typed state access.
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
   * Subclasses must implement this method with their event processing logic.
   *
   * @param event - The stream event to process.
   */
  abstract processEvent(event: StreamEvent<V>): Promise<void>;

  /**
   * Handle an incoming message from the actor framework.
   *
   * Routes custom messages that look like stream events to
   * {@link processEvent} (via backpressure), and watermark messages
   * to {@link advanceWatermark}.
   *
   * @param sender  - The sending actor identity.
   * @param message - The incoming message.
   * @returns An optional response message.
   */
  async handle(sender: string, message: Message): Promise<Message | void> {
    if (message.type === MessageType.CUSTOM) {
      const eventData = message.payload;
      // Check if this is a stream event
      if (eventData._type === 'stream_event' || this.isStreamEvent(eventData)) {
        if (this.isStreamEvent(eventData)) {
          await this.processWithBackpressure(eventData);
        } else if (typeof eventData === 'object' && eventData !== null) {
          const event = this.dictToEvent(eventData as Record<string, unknown>);
          if (event) {
            await this.processWithBackpressure(event);
          }
        }
      } 
      // Check if this is a watermark
      else if (eventData._type === 'watermark' || this.isWatermarkData(eventData)) {
        const watermark = this.toWatermark(eventData);
        if (watermark) {
          await this.advanceWatermark(watermark);
        }
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

  /**
   * Check if data has watermark-like structure (timestamp as number or Timestamp).
   * @internal
   */
  private isWatermarkData(obj: unknown): boolean {
    return (
      typeof obj === 'object' &&
      obj !== null &&
      'timestamp' in obj &&
      'streamId' in obj
    );
  }

  /**
   * Convert watermark data to a Watermark instance.
   * @internal
   */
  private toWatermark(obj: unknown): Watermark | null {
    if (!this.isWatermarkData(obj)) {
      return null;
    }

    const data = obj as Record<string, unknown>;
    
    // If already a Watermark instance
    if (obj instanceof Watermark) {
      return obj;
    }

    // If timestamp is a Timestamp instance
    if (data.timestamp instanceof Timestamp) {
      return new Watermark(
        data.timestamp,
        data.streamId as string,
        data.partition as number | undefined
      );
    }

    // If timestamp is a number, convert it
    if (typeof data.timestamp === 'number') {
      return Watermark.fromObject({
        timestamp: data.timestamp,
        streamId: data.streamId as string,
        partition: data.partition as number | undefined,
      });
    }

    return null;
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

  /**
   * Process an event through the backpressure controller.
   * @internal
   */
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

  /**
   * Internal event processing with late-data detection and windowing.
   * @internal
   */
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
        await this.emitToStream('window_output', result);
      }
    }

    // Call user's processEvent
    await this.processEvent(event);

    // Update last processed timestamp
    this.streamState.lastProcessedTimestamp = event.timestamp;
  }

  /**
   * Extract the partition key from an event.
   *
   * Override to customize key extraction logic.
   *
   * @param event - The stream event.
   * @returns The partition key.
   */
  protected extractKey(event: StreamEvent<V>): K {
    return event.key as unknown as K;
  }

  /**
   * Handle a late-arriving event according to the configured policy.
   *
   * @param event - The late stream event.
   * @internal
   */
  protected async handleLateEvent(event: StreamEvent<V>): Promise<void> {
    const policy = this.streamConfig.lateDataPolicy || LateDataPolicy.Drop;

    switch (policy) {
      case LateDataPolicy.Drop:
        return;

      case LateDataPolicy.SideOutput:
        if (this.lateDataHandler) {
          this.lateDataHandler(event);
        } else if (this.streamConfig.lateDataOutput) {
          await this.emitValue(this.streamConfig.lateDataOutput, event);
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
   * Advance the watermark for a stream.
   *
   * The watermark only advances forward. Any windows triggered by the
   * new watermark are fired and their results emitted.
   *
   * @param watermark - The new watermark.
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
          await this.emitToStream('window_output', result);
        }
      }
    }
  }

  /**
   * Get the current watermark for a stream.
   *
   * @param streamId - The stream identifier.
   * @returns The watermark timestamp, or `undefined` if none has been set.
   */
  getWatermark(streamId: string): Timestamp | undefined {
    return this.streamState.watermarks.get(streamId);
  }

  /**
   * Emit a value to an output stream.
   *
   * Creates a {@link StreamEvent} with the current timestamp and emits
   * it via the registered output handler.
   *
   * @param stream - The output stream name.
   * @param value  - The value to emit.
   */
  async emitValue(stream: string, value: unknown): Promise<void> {
    const event: StreamEvent<unknown> = {
      key: String(this.hashValue(value)),
      value,
      timestamp: Timestamp.now(),
      headers: {},
    };
    await this.doEmit(stream, event);
  }

  /**
   * Emit a value with a specific timestamp to an output stream.
   *
   * @param stream    - The output stream name.
   * @param value     - The value to emit.
   * @param timestamp - The event timestamp.
   */
  async emitValueWithTimestamp(
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
   * Emit a pre-constructed stream event to an output stream.
   *
   * @param stream - The output stream name.
   * @param event  - The stream event to emit.
   */
  async emitEvent(stream: string, event: StreamEvent<unknown>): Promise<void> {
    await this.doEmit(stream, event);
  }
 
  private async doEmit(stream: string, event: StreamEvent<unknown>): Promise<void> {
    if (this.outputHandlers.has(stream)) {
      const handler = this.outputHandlers.get(stream)!;
      handler(event);
    }
  }

  /**
   * Emit to stream (alias for doEmit for internal use).
   * @internal
   */
  private async emitToStream(stream: string, value: unknown): Promise<void> {
    await this.emit(stream, value);
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
   * Register a handler for an output stream.
   *
   * @param stream  - The output stream name.
   * @param handler - Callback invoked when an event is emitted to the stream.
   */
  registerOutputHandler(stream: string, handler: (event: StreamEvent<unknown>) => void): void {
    this.outputHandlers.set(stream, handler);
  }

  /**
   * Register a handler for late-arriving data.
   *
   * @param handler - Callback invoked for each late event when the
   *                 policy is {@link LateDataPolicy.SideOutput}.
   */
  registerLateDataHandler(handler: (event: StreamEvent<V>) => void): void {
    this.lateDataHandler = handler;
  }

  /**
   * Configure windowing for this stream actor.
   *
   * Sets up a {@link WindowAssigner} and {@link WindowTrigger} so that
   * events are automatically assigned to windows and fired when complete.
   *
   * @typeParam R - The result type of the window handler.
   * @param spec    - The window specification.
   * @param handler - Function called when a window fires, receiving the
   *                 accumulated events and window info.
   */
  configureWindow<R>(
    spec: WindowSpec,
    handler: (events: StreamEvent<V>[], info: WindowInfo) => R
  ): void {
    this.windowAssigner = new WindowAssigner<K, V>(spec);
    this.windowTrigger = new WindowTrigger<K, V, R>(this.windowAssigner, handler as any);
  }

  /**
   * Get a typed value from streaming state.
   *
   * @typeParam T - The expected value type.
   * @param name         - The state key.
   * @param defaultValue - Optional default.
   * @returns The stored value, or `defaultValue`.
   */
  async getState<T>(name: string, defaultValue?: T): Promise<T | undefined> {
    return this.streamingStateHandle.getValue(name, defaultValue);
  }

  /**
   * Set a typed value in streaming state.
   *
   * @typeParam T - The value type.
   * @param name  - The state key.
   * @param value - The value to store.
   */
  async setState<T>(name: string, value: T): Promise<void> {
    await this.streamingStateHandle.setValue(name, value);
  }

  /**
   * Get a list from streaming state.
   *
   * @typeParam T - The element type.
   * @param name - The state key.
   * @returns The stored list, or an empty array.
   */
  async getListState<T>(name: string): Promise<T[]> {
    return this.streamingStateHandle.getList<T>(name);
  }

  /**
   * Append an item to a list in streaming state.
   *
   * @typeParam T - The element type.
   * @param name - The state key.
   * @param item - The item to append.
   */
  async updateListState<T>(name: string, item: T): Promise<void> {
    await this.streamingStateHandle.appendToList(name, item);
  }

  /**
   * Get a map from streaming state.
   *
   * @typeParam K - The key type (must extend string).
   * @typeParam V - The value type.
   * @param name - The state key.
   * @returns The stored map, or an empty object.
   */
  async getMapState<K extends string, V>(name: string): Promise<Record<K, V>> {
    return this.streamingStateHandle.getMap<K, V>(name);
  }

  /**
   * Put a key-value pair into a map in streaming state.
   *
   * @typeParam K - The key type (must extend string).
   * @typeParam V - The value type.
   * @param name  - The state key.
   * @param key   - The key within the map.
   * @param value - The value to store.
   */
  async updateMapState<K extends string, V>(name: string, key: K, value: V): Promise<void> {
    await this.streamingStateHandle.putInMap(name, key, value);
  }

  /**
   * Get stream processing metrics.
   *
   * @returns An object containing processed count, late events count,
   *          watermarks, and backpressure stats.
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
