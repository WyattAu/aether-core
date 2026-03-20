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
 * Statistics for backpressure handling.
 */
export interface BackpressureStats {
  totalEvents: number;
  bufferedEvents: number;
  droppedEvents: number;
  rejectedEvents: number;
  overflowCount: number;
  resumeCount: number;
  currentBufferSize: number;
  highWatermarkReached: boolean;
}

/**
 * Configuration for backpressure controller.
 */
export interface BackpressureConfig {
  strategy: BackpressureStrategy;
  bufferSize: number;
  highWatermark: number;  // 0.0 - 1.0
  lowWatermark: number;   // 0.0 - 1.0
  onOverflow?: () => void;
  onResume?: () => void;
}

/**
 * Error thrown when backpressure causes a failure.
 */
export class BackpressureError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BackpressureError';
  }
}

/**
 * Error thrown when buffer is full and strategy is FAIL.
 */
export class BufferFullError extends BackpressureError {
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
 * Controls flow of events through the stream processor.
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

  constructor(private config: BackpressureConfig = DEFAULT_BACKPRESSURE_CONFIG) {}

  /**
   * Get current configuration.
   */
  get configSnapshot(): BackpressureConfig {
    return { ...this.config };
  }

  /**
   * Get current statistics.
   */
  getStats(): BackpressureStats {
    return { ...this.stats };
  }

  /**
   * Check if buffer is above high watermark.
   */
  get isOverloaded(): boolean {
    if (this.buffer.length === 0) return false;
    const fillRatio = this.buffer.length / this.config.bufferSize;
    return fillRatio >= this.config.highWatermark;
  }

  /**
   * Check if buffer is below low watermark.
   */
  get isRecovered(): boolean {
    if (this.buffer.length === 0) return true;
    const fillRatio = this.buffer.length / this.config.bufferSize;
    return fillRatio <= this.config.lowWatermark;
  }

  /**
   * Try to push an event to the buffer.
   *
   * @returns True if accepted, false if dropped
   * @throws BufferFullError if strategy is FAIL and buffer is full
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
   * Pop the next event from the buffer.
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
   */
  peek(): StreamEvent<T> | undefined {
    return this.buffer[0];
  }

  /**
   * Clear all events from buffer.
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
   * Get current buffer size.
   */
  size(): number {
    return this.buffer.length;
  }

  /**
   * Check if buffer is empty.
   */
  isEmpty(): boolean {
    return this.buffer.length === 0;
  }

  /**
   * Check if buffer is full.
   */
  isFull(): boolean {
    return this.buffer.length >= this.config.bufferSize;
  }

  /**
   * Set overflow callback.
   */
  setOverflowCallback(callback: () => void): void {
    this.config.onOverflow = callback;
  }

  /**
   * Set resume callback.
   */
  setResumeCallback(callback: () => void): void {
    this.config.onResume = callback;
  }

  /**
   * Reset statistics counters.
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
 * Multi-level backpressure with priority queues.
 *
 * Provides different priority levels for events:
 * - HIGH: Critical events that should never be dropped
 * - NORMAL: Regular events
 * - LOW: Best-effort events that can be dropped first
 */
export class MultiLevelBackpressure<T = unknown> {
  static readonly HIGH = 0;
  static readonly NORMAL = 1;
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

  constructor(private bufferSize: number = 10000) {}

  /**
   * Push event with priority.
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
   * Pop highest priority event available.
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
   * Get total buffer size.
   */
  size(): number {
    return this.high.length + this.normal.length + this.low.length;
  }

  /**
   * Check if all queues are empty.
   */
  isEmpty(): boolean {
    return this.size() === 0;
  }

  /**
   * Get statistics.
   */
  getStats(): BackpressureStats {
    return { ...this.stats, currentBufferSize: this.size() };
  }
}

/**
 * Rate-based backpressure.
 *
 * Monitors the rate of events being processed and applies
 * backpressure when the rate exceeds the configured threshold.
 */
export class RateBasedBackpressure {
  private timestamps: number[] = [];
  private backpressureActive = false;
  private backpressureUntil = 0;

  constructor(
    private maxRate: number,
    private windowSize: number = 10.0,
    private cooldown: number = 1.0
  ) {}

  /**
   * Check if backpressure is currently active.
   */
  get isBackpressureActive(): boolean {
    return this.backpressureActive;
  }

  /**
   * Get current processing rate (events/second).
   */
  get currentRate(): number {
    const now = Date.now();
    const cutoff = now - this.windowSize * 1000;
    this.timestamps = this.timestamps.filter(ts => ts > cutoff);
    if (this.timestamps.length === 0) return 0.0;
    return this.timestamps.length / this.windowSize;
  }

  /**
   * Try to acquire permission to process.
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
   * Reset the rate tracker.
   */
  reset(): void {
    this.timestamps = [];
    this.backpressureActive = false;
    this.backpressureUntil = 0;
  }
}
