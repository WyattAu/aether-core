/**
 * Backpressure Handling
 *
 * Strategies for handling flow control in stream processing:
 * - BUFFER: Buffer events up to capacity
 * - DROP: Drop events when overloaded
 * - FAIL: Raise error when overloaded
 * - LATEST: Keep only the latest events
 *
 * @module aether/streaming/backpressure
 */

import { Duration, Timestamp, StreamEvent, BackpressureStrategy } from './types';

/**
 * Statistics snapshot for backpressure handling.
 */
export interface BackpressureStats {
  /** Total events received. */
  totalEvents: number;
  /** Events currently buffered. */
  bufferedEvents: number;
  /** Events dropped due to overflow or strategy. */
  droppedEvents: number;
  /** Events rejected (e.g., buffer full with no queue). */
  rejectedEvents: number;
  /** Number of times the high watermark was reached. */
  overflowCount: number;
  /** Number of times the buffer recovered below the low watermark. */
  resumeCount: number;
  /** Current number of events in the buffer. */
  currentBufferSize: number;
  /** Whether the high watermark has been reached. */
  highWatermarkReached: boolean;
}

/**
 * Configuration for a {@link BackpressureController}.
 */
export interface BackpressureConfig {
  /** The backpressure strategy to use. */
  strategy: BackpressureStrategy;
  /** Maximum number of events to buffer. */
  bufferSize: number;
  /** High watermark threshold as a fraction (0.0 - 1.0). */
  highWatermark: number;
  /** Low watermark threshold as a fraction (0.0 - 1.0). */
  lowWatermark: number;
  /** Callback invoked when the high watermark is reached. */
  onOverflow?: () => void;
  /** Callback invoked when the buffer recovers below the low watermark. */
  onResume?: () => void;
}

/**
 * Error thrown when backpressure causes a processing failure.
 */
export class BackpressureError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BackpressureError';
  }
}

/**
 * Error thrown when the buffer is full and the strategy is {@link BackpressureStrategy.Fail}.
 *
 * @example
 * ```typescript
 * try {
 *   controller.tryPush(event);
 * } catch (e) {
 *   if (e instanceof BufferFullError) {
 *     console.log(`Buffer capacity: ${e.bufferSize}`);
 *   }
 * }
 * ```
 */
export class BufferFullError extends BackpressureError {
  /**
   * @param bufferSize - The configured buffer capacity.
   * @param event      - The event that was rejected.
   */
  constructor(
    public readonly bufferSize: number,
    public readonly event?: StreamEvent<unknown>
  ) {
    super(`Buffer full (size=${bufferSize}). Cannot accept more events.`);
    this.name = 'BufferFullError';
  }
}

/**
 * Default backpressure configuration.
 */
export const DEFAULT_BACKPRESSURE_CONFIG: BackpressureConfig = {
  strategy: BackpressureStrategy.Buffer,
  bufferSize: 10000,
  highWatermark: 0.9,
  lowWatermark: 0.5,
};

/**
 * Backpressure Controller
 *
 * Controls the flow of events through the stream processor using one of
 * several strategies. Monitors buffer fill levels against high/low
 * watermarks and invokes callbacks on state transitions.
 *
 * @typeParam T - The event payload type.
 *
 * @example
 * ```typescript
 * const controller = new BackpressureController({
 *   strategy: BackpressureStrategy.Buffer,
 *   bufferSize: 10000,
 * });
 *
 * if (controller.tryPush(event)) {
 *   // Event accepted
 * }
 *
 * if (controller.isOverloaded) {
 *   // Signal upstream to slow down
 * }
 * ```
 */
export class BackpressureController<T = unknown> {
  private buffer: StreamEvent<T>[] = [];
  private stats: BackpressureStats = {
    totalEvents: 0,
    bufferedEvents: 0,
    droppedEvents: 0,
    rejectedEvents: 0,
    overflowCount: 0,
    resumeCount: 0,
    currentBufferSize: 0,
    highWatermarkReached: false,
  };

  /**
   * Create a new BackpressureController.
   *
   * @param config - Backpressure configuration (uses defaults if omitted).
   */
  constructor(private config: BackpressureConfig = DEFAULT_BACKPRESSURE_CONFIG) {}

  /**
   * Get a snapshot of the current configuration.
   *
   * @returns A copy of the backpressure configuration.
   */
  get configSnapshot(): BackpressureConfig {
    return { ...this.config };
  }

  /**
   * Get a snapshot of current statistics.
   *
   * @returns A copy of the backpressure stats.
   */
  getStats(): BackpressureStats {
    return { ...this.stats };
  }

  /**
   * Check if the buffer fill level is at or above the high watermark.
   *
   * @returns `true` if the buffer is overloaded.
   */
  get isOverloaded(): boolean {
    if (this.buffer.length === 0) return false;
    const fillRatio = this.buffer.length / this.config.bufferSize;
    return fillRatio >= this.config.highWatermark;
  }

  /**
   * Check if the buffer fill level is at or below the low watermark.
   *
   * @returns `true` if the buffer has recovered.
   */
  get isRecovered(): boolean {
    if (this.buffer.length === 0) return true;
    const fillRatio = this.buffer.length / this.config.bufferSize;
    return fillRatio <= this.config.lowWatermark;
  }

  /**
   * Try to push an event into the buffer.
   *
   * Behavior depends on the configured strategy:
   * - **Buffer**: Rejects (returns `false`) when full.
   * - **Drop**: Drops the event (returns `false`) when full.
   * - **Fail**: Throws {@link BufferFullError} when full.
   * - **Latest**: Evicts the oldest event to make room.
   *
   * @param event - The stream event to buffer.
   * @returns `true` if the event was accepted, `false` if dropped/rejected.
   * @throws BufferFullError If strategy is Fail and buffer is full.
   */
  tryPush(event: StreamEvent<T>): boolean {
    this.stats.totalEvents++;
    const bufferSize = this.buffer.length;

    // Check if buffer is full
    if (bufferSize >= this.config.bufferSize) {
      return this.handleFullBuffer(event);
    }

    // Buffer has room
    if (this.config.strategy === BackpressureStrategy.Latest) {
      if (bufferSize >= this.config.bufferSize) {
        this.buffer.shift();
        this.stats.droppedEvents++;
      }
    }

    this.buffer.push(event);
    this.stats.bufferedEvents++;
    this.stats.currentBufferSize = this.buffer.length;

    // Check if we hit high watermark
    if (this.isOverloaded && !this.stats.highWatermarkReached) {
      this.stats.highWatermarkReached = true;
      this.stats.overflowCount++;
      if (this.config.onOverflow) {
        this.config.onOverflow();
      }
    }

    return true;
  }

  /**
   * Handle a push when the buffer is at capacity.
   * @internal
   */
  private handleFullBuffer(event: StreamEvent<T>): boolean {
    switch (this.config.strategy) {
      case BackpressureStrategy.Fail:
        this.stats.rejectedEvents++;
        throw new BufferFullError(this.config.bufferSize, event);

      case BackpressureStrategy.Drop:
        this.stats.droppedEvents++;
        return false;

      case BackpressureStrategy.Latest:
        this.buffer.shift();
        this.stats.droppedEvents++;
        this.buffer.push(event);
        return true;

      default: // Buffer
        this.stats.rejectedEvents++;
        return false;
    }
  }

  /**
   * Pop the next (oldest) event from the buffer.
   *
   * Triggers the resume callback if the buffer transitions from
   * above the high watermark to below the low watermark.
   *
   * @returns The next event, or `undefined` if the buffer is empty.
   */
  pop(): StreamEvent<T> | undefined {
    if (this.buffer.length === 0) return undefined;

    const event = this.buffer.shift()!;
    this.stats.bufferedEvents--;
    this.stats.currentBufferSize = this.buffer.length;

    // Check if we recovered below low watermark
    const wasOverloaded = this.stats.highWatermarkReached;
    if (wasOverloaded && this.isRecovered) {
      this.stats.highWatermarkReached = false;
      this.stats.resumeCount++;
      if (this.config.onResume) {
        this.config.onResume();
      }
    }

    return event;
  }

  /**
   * Peek at the next event without removing it.
   *
   * @returns The next event, or `undefined` if the buffer is empty.
   */
  peek(): StreamEvent<T> | undefined {
    return this.buffer[0];
  }

  /**
   * Clear all events from the buffer.
   *
   * @returns The number of events that were cleared.
   */
  clear(): number {
    const count = this.buffer.length;
    this.stats.droppedEvents += count;
    this.buffer = [];
    this.stats.bufferedEvents = 0;
    this.stats.currentBufferSize = 0;
    this.stats.highWatermarkReached = false;
    return count;
  }

  /**
   * Get the current number of buffered events.
   *
   * @returns The buffer size.
   */
  size(): number {
    return this.buffer.length;
  }

  /**
   * Check if the buffer is empty.
   *
   * @returns `true` if no events are buffered.
   */
  isEmpty(): boolean {
    return this.buffer.length === 0;
  }

  /**
   * Check if the buffer is at full capacity.
   *
   * @returns `true` if `size >= bufferSize`.
   */
  isFull(): boolean {
    return this.buffer.length >= this.config.bufferSize;
  }

  /**
   * Set the callback invoked when the high watermark is reached.
   *
   * @param callback - The overflow callback.
   */
  setOverflowCallback(callback: () => void): void {
    this.config.onOverflow = callback;
  }

  /**
   * Set the callback invoked when the buffer recovers.
   *
   * @param callback - The resume callback.
   */
  setResumeCallback(callback: () => void): void {
    this.config.onResume = callback;
  }

  /**
   * Reset all statistic counters (except current buffer size).
   */
  resetStats(): void {
    this.stats.totalEvents = 0;
    this.stats.bufferedEvents = this.buffer.length;
    this.stats.droppedEvents = 0;
    this.stats.rejectedEvents = 0;
    this.stats.overflowCount = 0;
    this.stats.resumeCount = 0;
  }
}

/**
 * Multi-level backpressure controller with priority queues.
 *
 * Provides different priority levels for events:
 * - **HIGH** (0) — Critical events that should never be dropped.
 * - **NORMAL** (1) — Regular events.
 * - **LOW** (2) — Best-effort events that can be dropped first.
 *
 * When the buffer is full, lower-priority events are dropped before
 * higher-priority ones.
 *
 * @typeParam T - The event payload type.
 *
 * @example
 * ```typescript
 * const bp = new MultiLevelBackpressure<Event>(10000);
 * bp.push(criticalEvent, MultiLevelBackpressure.HIGH);
 * bp.push(regularEvent, MultiLevelBackpressure.NORMAL);
 * bp.push(bestEffortEvent, MultiLevelBackpressure.LOW);
 * ```
 */
export class MultiLevelBackpressure<T = unknown> {
  /** High priority level — never dropped first. */
  static readonly HIGH = 0;
  /** Normal priority level. */
  static readonly NORMAL = 1;
  /** Low priority level — dropped first under pressure. */
  static readonly LOW = 2;

  private high: StreamEvent<T>[] = [];
  private normal: StreamEvent<T>[] = [];
  private low: StreamEvent<T>[] = [];
  private stats: BackpressureStats = {
    totalEvents: 0,
    bufferedEvents: 0,
    droppedEvents: 0,
    rejectedEvents: 0,
    overflowCount: 0,
    resumeCount: 0,
    currentBufferSize: 0,
    highWatermarkReached: false,
  };

  /**
   * Create a new MultiLevelBackpressure.
   *
   * @param bufferSize - Maximum total events across all priority queues.
   */
  constructor(private bufferSize: number = 10000) {}

  /**
   * Push an event with a specified priority level.
   *
   * When the buffer is full, lower-priority events are evicted first.
   * HIGH priority events are always accepted.
   *
   * @param event    - The stream event.
   * @param priority - The priority level (use `MultiLevelBackpressure.HIGH/NORMAL/LOW`).
   * @returns `true` if the event was accepted, `false` if dropped.
   */
  push(event: StreamEvent<T>, priority: number = MultiLevelBackpressure.NORMAL): boolean {
    const total = this.high.length + this.normal.length + this.low.length;

    if (total >= this.bufferSize) {
      // Try to drop from lowest priority first
      if (this.low.length > 0) {
        this.low.pop();
        this.stats.droppedEvents++;
      } else if (priority === MultiLevelBackpressure.LOW) {
        this.stats.droppedEvents++;
        return false;
      } else if (this.normal.length > 0) {
        this.normal.pop();
        this.stats.droppedEvents++;
      } else if (priority === MultiLevelBackpressure.NORMAL) {
        this.stats.droppedEvents++;
        return false;
      }
      // HIGH priority always accepted
    }

    // Add to appropriate queue
    switch (priority) {
      case MultiLevelBackpressure.HIGH:
        this.high.push(event);
        break;
      case MultiLevelBackpressure.NORMAL:
        this.normal.push(event);
        break;
      default:
        this.low.push(event);
    }

    this.stats.totalEvents++;
    this.stats.bufferedEvents = this.high.length + this.normal.length + this.low.length;
    return true;
  }

  /**
   * Pop the highest-priority event available.
   *
   * Prefers HIGH over NORMAL over LOW.
   *
   * @returns The next event, or `undefined` if all queues are empty.
   */
  pop(): StreamEvent<T> | undefined {
    let event: StreamEvent<T> | undefined;

    if (this.high.length > 0) {
      event = this.high.shift();
    } else if (this.normal.length > 0) {
      event = this.normal.shift();
    } else if (this.low.length > 0) {
      event = this.low.shift();
    }

    if (event) {
      this.stats.bufferedEvents = this.high.length + this.normal.length + this.low.length;
    }

    return event;
  }

  /**
   * Get the total number of buffered events across all priorities.
   *
   * @returns The total buffer size.
   */
  size(): number {
    return this.high.length + this.normal.length + this.low.length;
  }

  /**
   * Check if all priority queues are empty.
   *
   * @returns `true` if no events are buffered.
   */
  isEmpty(): boolean {
    return this.size() === 0;
  }

  /**
   * Get a snapshot of current statistics.
   *
   * @returns A copy of the backpressure stats.
   */
  getStats(): BackpressureStats {
    return { ...this.stats, currentBufferSize: this.size() };
  }
}

/**
 * Rate-based backpressure controller.
 *
 * Monitors the rate of events being processed and applies backpressure
 * when the rate exceeds a configured threshold. Includes a cooldown
 * period after backpressure is activated.
 *
 * @example
 * ```typescript
 * const rateBP = new RateBasedBackpressure(1000, 10.0, 1.0);
 * // Max 1000 events per 10-second window, 1-second cooldown
 *
 * if (await rateBP.tryAcquire()) {
 *   await processEvent(event);
 * }
 * ```
 */
export class RateBasedBackpressure {
  private timestamps: number[] = [];
  private backpressureActive = false;
  private backpressureUntil = 0;

  /**
   * Create a new RateBasedBackpressure.
   *
   * @param maxRate   - Maximum events per window before backpressure activates.
   * @param windowSize - Sliding window duration in seconds (default: 10).
   * @param cooldown  - Cooldown period in seconds after activation (default: 1).
   */
  constructor(
    private maxRate: number,
    private windowSize: number = 10.0,
    private cooldown: number = 1.0
  ) {}

  /**
   * Check if backpressure is currently active.
   *
   * @returns `true` if the rate threshold has been exceeded.
   */
  get isBackpressureActive(): boolean {
    return this.backpressureActive;
  }

  /**
   * Get the current processing rate (events per second).
   *
   * @returns The rate within the sliding window, or 0 if no events.
   */
  get currentRate(): number {
    const now = Date.now();
    const cutoff = now - this.windowSize * 1000;
    this.timestamps = this.timestamps.filter(ts => ts > cutoff);
    if (this.timestamps.length === 0) return 0.0;
    return this.timestamps.length / this.windowSize;
  }

  /**
   * Try to acquire permission to process an event.
   *
   * Records the current timestamp if the rate is within bounds;
   * otherwise activates backpressure for the cooldown period.
   *
   * @returns `true` if processing is allowed, `false` if backpressure is active.
   */
  async tryAcquire(): Promise<boolean> {
    const now = Date.now();

    // Clean old timestamps
    const cutoff = now - this.windowSize * 1000;
    this.timestamps = this.timestamps.filter(ts => ts > cutoff);

    // Check if in cooldown
    if (this.backpressureActive && now < this.backpressureUntil) {
      return false;
    }

    // Check if rate exceeded
    const currentRate = this.timestamps.length / this.windowSize;
    if (currentRate >= this.maxRate) {
      this.backpressureActive = true;
      this.backpressureUntil = now + this.cooldown * 1000;
      return false;
    }

    // Allow and record
    this.timestamps.push(now);
    this.backpressureActive = false;
    return true;
  }

  /**
   * Reset the rate tracker and clear backpressure state.
   */
  reset(): void {
    this.timestamps = [];
    this.backpressureActive = false;
    this.backpressureUntil = 0;
  }
}
