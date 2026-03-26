/**
 * Streaming Types and Enums
 *
 * Core types for stream processing:
 * - StreamEvent: Individual event in a stream
 * - Watermark: Time marker for event progress
 * - StreamConfig: Configuration for stream actors
 * - Window types and configurations
 *
 * @module aether/streaming/types
 */

/**
 * Types of windowing strategies.
 */
export enum WindowType {
  /** Fixed-size, non-overlapping windows. */
  Tumbling = 'tumbling',
  /** Fixed-size, overlapping windows. */
  Sliding = 'sliding',
  /** Dynamic size based on activity gaps. */
  Session = 'session',
}

/**
 * How to handle late-arriving data.
 */
export enum LateDataPolicy {
  /** Discard late events silently. */
  Drop = 'drop',
  /** Route late events to a side output stream. */
  SideOutput = 'side-output',
  /** Reprocess affected windows with late data. */
  Reprocess = 'reprocess',
}

/**
 * Watermark generation strategy.
 */
export enum WatermarkStrategy {
  /** Watermark based on event timestamps. */
  EventTime = 'event-time',
  /** Watermark based on wall-clock processing time. */
  ProcessingTime = 'processing-time',
  /** Allow bounded out-of-orderness before advancing watermark. */
  BoundedOutOfOrder = 'bounded-out-of-order',
}

/**
 * Backpressure handling strategies.
 */
export enum BackpressureStrategy {
  /** Buffer events up to the configured limit. */
  Buffer = 'buffer',
  /** Drop events when the processor is overloaded. */
  Drop = 'drop',
  /** Raise an error when the processor is overloaded. */
  Fail = 'fail',
  /** Keep only the latest events, discarding older ones. */
  Latest = 'latest',
}

/**
 * Message delivery guarantees.
 */
export enum DeliverySemantics {
  /** Fire and forget; no delivery confirmation. */
  AtMostOnce = 'at-most-once',
  /** Guaranteed delivery but possible duplicates. */
  AtLeastOnce = 'at-least-once',
  /** Exactly-once delivery; no duplicates, no loss. */
  ExactlyOnce = 'exactly-once',
}

/**
 * Window pane firing classification.
 */
export enum PaneInfo {
  /** Early firing before the watermark has passed. */
  Early = 'early',
  /** On-time firing at the watermark. */
  OnTime = 'on-time',
  /** Late firing after the watermark has passed. */
  Late = 'late',
}

/**
 * Event timestamp with millisecond precision.
 *
 * Immutable value object for representing points in time in stream processing.
 *
 * @example
 * ```typescript
 * const now = Timestamp.now();
 * const fromSeconds = Timestamp.fromSeconds(1234567890);
 * const future = now.add(Duration.fromMinutes(5));
 * ```
 */
export class Timestamp {
  /**
   * Create a Timestamp from milliseconds.
   *
   * @param milliseconds - Time value in milliseconds since Unix epoch.
   */
  constructor(public readonly milliseconds: number) {}

  /**
   * Create a Timestamp representing the current time.
   *
   * @returns A new Timestamp for `Date.now()`.
   */
  static now(): Timestamp {
    return new Timestamp(Date.now());
  }

  /**
   * Create a Timestamp from a Date object.
   *
   * @param date - The Date to convert.
   * @returns A new Timestamp with the same millisecond value.
   */
  static fromDate(date: Date): Timestamp {
    return new Timestamp(date.getTime());
  }

  /**
   * Create a Timestamp from seconds since the Unix epoch.
   *
   * @param seconds - Time value in seconds (fractional values are floored).
   * @returns A new Timestamp.
   */
  static fromSeconds(seconds: number): Timestamp {
    return new Timestamp(Math.floor(seconds * 1000));
  }

  /**
   * Convert this Timestamp to a JavaScript Date object.
   *
   * @returns A new Date representing the same point in time.
   */
  toDate(): Date {
    return new Date(this.milliseconds);
  }

  /**
   * Convert this Timestamp to seconds.
   *
   * @returns The time value in seconds (may be fractional).
   */
  toSeconds(): number {
    return this.milliseconds / 1000;
  }

  /**
   * Add a duration to this timestamp.
   *
   * @param duration - The duration to add.
   * @returns A new Timestamp representing the sum.
   */
  add(duration: Duration): Timestamp {
    return new Timestamp(this.milliseconds + duration.milliseconds);
  }

  /**
   * Subtract another timestamp to get the elapsed duration.
   *
   * @param other - The timestamp to subtract.
   * @returns A Duration representing `this - other`.
   */
  subtract(other: Timestamp): Duration {
    return new Duration(this.milliseconds - other.milliseconds);
  }

  /**
   * Subtract a duration from this timestamp.
   *
   * @param duration - The duration to subtract.
   * @returns A new Timestamp representing the difference.
   */
  subtractDuration(duration: Duration): Timestamp {
    return new Timestamp(this.milliseconds - duration.milliseconds);
  }

  /**
   * Compare this timestamp to another.
   *
   * @param other - The timestamp to compare against.
   * @returns Negative if `this < other`, zero if equal, positive if `this > other`.
   */
  compareTo(other: Timestamp): number {
    return this.milliseconds - other.milliseconds;
  }

  /**
   * Check equality with another timestamp.
   *
   * @param other - The timestamp to compare against.
   * @returns `true` if both represent the same millisecond.
   */
  equals(other: Timestamp): boolean {
    return this.milliseconds === other.milliseconds;
  }

  /**
   * Check if this timestamp is strictly before another.
   *
   * @param other - The timestamp to compare against.
   * @returns `true` if `this < other`.
   */
  isBefore(other: Timestamp): boolean {
    return this.milliseconds < other.milliseconds;
  }

  /**
   * Check if this timestamp is strictly after another.
   *
   * @param other - The timestamp to compare against.
   * @returns `true` if `this > other`.
   */
  isAfter(other: Timestamp): boolean {
    return this.milliseconds > other.milliseconds;
  }

  /**
   * Check if this timestamp is before or equal to another.
   *
   * @param other - The timestamp to compare against.
   * @returns `true` if `this <= other`.
   */
  isBeforeOrEqual(other: Timestamp): boolean {
    return this.milliseconds <= other.milliseconds;
  }

  /**
   * Check if this timestamp is after or equal to another.
   *
   * @param other - The timestamp to compare against.
   * @returns `true` if `this >= other`.
   */
  isAfterOrEqual(other: Timestamp): boolean {
    return this.milliseconds >= other.milliseconds;
  }

  /**
   * Serialize to a raw millisecond number.
   *
   * @returns The millisecond value.
   */
  toJSON(): number {
    return this.milliseconds;
  }

  /**
   * Deserialize from a raw millisecond number.
   *
   * @param ms - The millisecond value.
   * @returns A new Timestamp.
   */
  static fromJSON(ms: number): Timestamp {
    return new Timestamp(ms);
  }
}

/**
 * Duration with millisecond precision.
 *
 * Immutable value object for representing spans of time in stream processing.
 *
 * @example
 * ```typescript
 * const fiveMinutes = Duration.fromMinutes(5);
 * const oneSecond = Duration.fromSeconds(1);
 * const doubled = fiveMinutes.multiply(2);
 * ```
 */
export class Duration {
  /**
   * Create a Duration from milliseconds.
   *
   * @param milliseconds - The duration in milliseconds.
   */
  constructor(public readonly milliseconds: number) {}

  /**
   * Create a Duration from milliseconds.
   *
   * @param ms - The duration in milliseconds.
   * @returns A new Duration.
   */
  static fromMillis(ms: number): Duration {
    return new Duration(ms);
  }

  /**
   * Create a Duration from seconds.
   *
   * @param seconds - The duration in seconds.
   * @returns A new Duration.
   */
  static fromSeconds(seconds: number): Duration {
    return new Duration(Math.floor(seconds * 1000));
  }

  /**
   * Create a Duration from minutes.
   *
   * @param minutes - The duration in minutes.
   * @returns A new Duration.
   */
  static fromMinutes(minutes: number): Duration {
    return new Duration(Math.floor(minutes * 60 * 1000));
  }

  /**
   * Create a Duration from hours.
   *
   * @param hours - The duration in hours.
   * @returns A new Duration.
   */
  static fromHours(hours: number): Duration {
    return new Duration(Math.floor(hours * 3600 * 1000));
  }

  /**
   * Convert this Duration to seconds.
   *
   * @returns The duration in seconds (may be fractional).
   */
  toSeconds(): number {
    return this.milliseconds / 1000;
  }

  /**
   * Get the duration in milliseconds.
   *
   * @returns The millisecond value.
   */
  toMillis(): number {
    return this.milliseconds;
  }

  /**
   * Add another duration to this one.
   *
   * @param other - The duration to add.
   * @returns A new Duration representing the sum.
   */
  add(other: Duration): Duration {
    return new Duration(this.milliseconds + other.milliseconds);
  }

  /**
   * Subtract another duration from this one.
   *
   * @param other - The duration to subtract.
   * @returns A new Duration representing the difference.
   */
  subtract(other: Duration): Duration {
    return new Duration(this.milliseconds - other.milliseconds);
  }

  /**
   * Multiply this duration by a scalar factor.
   *
   * @param factor - The multiplication factor.
   * @returns A new Duration scaled by the factor.
   */
  multiply(factor: number): Duration {
    return new Duration(this.milliseconds * factor);
  }
}

/**
 * An event in a data stream with associated metadata.
 *
 * @typeParam T - The payload type.
 *
 * @example
 * ```typescript
 * const event: StreamEvent<User> = {
 *   key: 'user-123',
 *   value: { name: 'Alice' },
 *   timestamp: Timestamp.now(),
 *   headers: { 'trace-id': 'abc' },
 * };
 * ```
 */
export interface StreamEvent<T> {
  /** Partition key for routing and grouping. */
  key: string;
  /** Event payload. */
  value: T;
  /** Event timestamp (event time, not processing time). */
  timestamp: Timestamp;
  /** Optional headers for metadata. */
  headers?: Record<string, string>;
  /** Optional partition number. */
  partition?: number;
  /** Optional offset within the partition. */
  offset?: number;
  /** Optional event type identifier. */
  eventType?: string;
}

/**
 * Create a new stream event.
 *
 * @typeParam T - The payload type.
 * @param key       - Partition key.
 * @param value     - Event payload.
 * @param timestamp - Event timestamp (defaults to now).
 * @param options   - Optional additional event properties.
 * @returns A new {@link StreamEvent}.
 */
export function createStreamEvent<T>(
  key: string,
  value: T,
  timestamp?: Timestamp,
  options?: Partial<Omit<StreamEvent<T>, 'key' | 'value' | 'timestamp'>>
): StreamEvent<T> {
  const ts = timestamp ?? Timestamp.now();
  return {
    key,
    value,
    timestamp: ts,
    headers: options?.headers,
    partition: options?.partition,
    offset: options?.offset,
    eventType: options?.eventType,
  };
}

/**
 * Watermark indicating the progress of event-time processing.
 *
 * Events with timestamps before the watermark are considered late.
 *
 * @example
 * ```typescript
 * const watermark = new Watermark(Timestamp.now(), 'input-stream');
 * if (watermark.isLate(event.timestamp)) {
 *   // Handle late event
 * }
 * ```
 */
export class Watermark {
  /**
   * Create a new Watermark.
   *
   * @param timestamp - The watermark timestamp.
   * @param streamId  - The stream identifier.
   * @param partition - Optional partition number.
   */
  constructor(
    public readonly timestamp: Timestamp,
    public readonly streamId: string,
    public readonly partition?: number
  ) {}

  /**
   * Check if an event timestamp is late relative to this watermark.
   *
   * @param eventTimestamp - The event timestamp to check.
   * @returns `true` if the event is before the watermark (late).
   */
  isLate(eventTimestamp: Timestamp): boolean {
    return eventTimestamp.isBefore(this.timestamp);
  }

  /**
   * Serialize the watermark to a plain object.
   *
   * @returns A plain object representation.
   */
  toJSON(): object {
    return {
      timestamp: this.timestamp.milliseconds,
      streamId: this.streamId,
      partition: this.partition,
    };
  }

  /**
   * Deserialize a plain object into a Watermark.
   *
   * @param obj - The serialized watermark object.
   * @returns A new Watermark instance.
   */
  static fromObject(obj: {
    timestamp: number;
    streamId: string;
    partition?: number;
  }): Watermark {
    return new Watermark(new Timestamp(obj.timestamp), obj.streamId, obj.partition);
  }
}

/**
 * Window specification for stream processing.
 */
export interface WindowSpec {
  /** Window type (tumbling, sliding, or session). */
  type: WindowType;
  /** Window size (or base size for sliding windows). */
  size: Duration;
  /** Slide interval for sliding windows. */
  slide?: Duration;
  /** Inactivity gap for session windows. */
  gap?: Duration;
  /** Tolerance for late-arriving data. */
  lateTolerance: Duration;
  /** Allowed lateness before events are dropped. */
  allowedLateness: Duration;
}

/**
 * Create a window specification.
 *
 * @param type    - The window type.
 * @param size    - The window size.
 * @param options - Optional overrides for slide, gap, and late data settings.
 * @returns A validated {@link WindowSpec}.
 * @throws Error If a sliding window is missing a `slide` parameter,
 *               or a session window is missing a `gap` parameter.
 */
export function createWindowSpec(
  type: WindowType,
  size: Duration,
  options?: {
    slide?: Duration;
    gap?: Duration;
    lateTolerance?: Duration;
    allowedLateness?: Duration;
  }
): WindowSpec {
  if (type === WindowType.Sliding && !options?.slide) {
    throw new Error("Sliding window requires 'slide' parameter");
  }
  if (type === WindowType.Session && !options?.gap) {
    throw new Error("Session window requires 'gap' parameter");
  }

  return {
    type,
    size,
    slide: options?.slide,
    gap: options?.gap,
    lateTolerance: options?.lateTolerance ?? Duration.fromMillis(0),
    allowedLateness: options?.allowedLateness ?? Duration.fromMillis(0),
  };
}

/**
 * Information about an active or completed window.
 */
export interface WindowInfo {
  /** Window start time. */
  start: Timestamp;
  /** Window end time. */
  end: Timestamp;
  /** Maximum event timestamp observed in the window. */
  maxTimestamp: Timestamp;
  /** Pane firing classification. */
  pane: PaneInfo;
  /** Optional unique window identifier. */
  windowId?: string;
}

/**
 * Create window info.
 *
 * @param start        - Window start timestamp.
 * @param end          - Window end timestamp.
 * @param maxTimestamp - Maximum event timestamp in the window.
 * @param pane         - The pane firing classification.
 * @param windowId     - Optional window identifier.
 * @returns A new {@link WindowInfo}.
 */
export function createWindowInfo(
  start: Timestamp,
  end: Timestamp,
  maxTimestamp: Timestamp,
  pane: PaneInfo,
  windowId?: string
): WindowInfo {
  return {
    start,
    end,
    maxTimestamp,
    pane,
    windowId,
  };
}

/**
 * Configuration for stream actors.
 */
export interface StreamConfig {
  /** Names of input streams. */
  inputStreams: string[];
  /** Names of output streams. */
  outputStreams: string[];
  /** Degree of parallelism for this actor. */
  parallelism: number;
  /** Partition assignment strategy. */
  partitionStrategy: 'key' | 'range' | 'hash' | 'random';
  /** Watermark generation strategy. */
  watermarkStrategy: WatermarkStrategy;
  /** Interval between watermark emissions. */
  watermarkInterval: Duration;
  /** Allowed out-of-orderness tolerance. */
  outOfOrderness: Duration;
  /** Whether periodic checkpointing is enabled. */
  checkpointingEnabled: boolean;
  /** Interval between checkpoints. */
  checkpointInterval: Duration;
  /** Policy for handling late-arriving data. */
  lateDataPolicy: LateDataPolicy;
  /** Side output stream name for late data. */
  lateDataOutput?: string;
  /** Maximum number of events to buffer. */
  bufferCapacity: number;
  /** Maximum time to wait before flushing buffered events. */
  bufferTimeout: Duration;
}

/**
 * Create a default stream configuration.
 *
 * @param options - Partial overrides for any config field.
 * @returns A complete {@link StreamConfig} with sensible defaults.
 */
export function createStreamConfig(
  options?: Partial<StreamConfig>
): StreamConfig {
  return {
    inputStreams: options?.inputStreams ?? [],
    outputStreams: options?.outputStreams ?? [],
    parallelism: options?.parallelism ?? 1,
    partitionStrategy: options?.partitionStrategy ?? 'key',
    watermarkStrategy: options?.watermarkStrategy ?? WatermarkStrategy.ProcessingTime,
    watermarkInterval: options?.watermarkInterval ?? Duration.fromSeconds(1),
    outOfOrderness: options?.outOfOrderness ?? Duration.fromMillis(0),
    checkpointingEnabled: options?.checkpointingEnabled ?? false,
    checkpointInterval: options?.checkpointInterval ?? Duration.fromMinutes(1),
    lateDataPolicy: options?.lateDataPolicy ?? LateDataPolicy.Drop,
    lateDataOutput: options?.lateDataOutput,
    bufferCapacity: options?.bufferCapacity ?? 10000,
    bufferTimeout: options?.bufferTimeout ?? Duration.fromSeconds(30),
  };
}

/**
 * Configuration for backpressure handling.
 */
export interface BackpressureConfig {
  /** The backpressure strategy to use. */
  strategy: BackpressureStrategy;
  /** Maximum number of events to buffer. */
  bufferSize: number;
  /** High watermark as a fraction (e.g., 0.9 = 90%). */
  highWatermark: number;
  /** Low watermark as a fraction (e.g., 0.5 = 50%). */
  lowWatermark: number;
}

/**
 * Create a default backpressure configuration.
 *
 * @param options - Partial overrides for any config field.
 * @returns A complete {@link BackpressureConfig} with sensible defaults.
 */
export function createBackpressureConfig(
  options?: Partial<BackpressureConfig>
): BackpressureConfig {
  return {
    strategy: options?.strategy ?? BackpressureStrategy.Buffer,
    bufferSize: options?.bufferSize ?? 10000,
    highWatermark: options?.highWatermark ?? 0.9,
    lowWatermark: options?.lowWatermark ?? 0.5,
  };
}

/**
 * Configuration for stream partitioning.
 */
export interface PartitionConfig {
  /** Partition assignment strategy. */
  strategy: 'key' | 'range' | 'hash' | 'random';
  /** Total number of partitions. */
  partitions: number;
  /** Optional key extractor function. */
  keyExtractor?: (value: unknown) => string;
}

/**
 * Handler for processing individual stream events.
 *
 * @typeParam T - The event payload type.
 */
export type EventHandler<T> = (event: StreamEvent<T>) => Promise<void> | void;

/**
 * Handler for processing a batch of stream events.
 *
 * @typeParam T - The event payload type.
 */
export type BatchHandler<T> = (events: StreamEvent<T>[]) => Promise<void> | void;

/**
 * Handler for processing windowed stream events.
 *
 * @typeParam V - The event payload type.
 * @typeParam R - The result type.
 */
export type WindowHandler<V, R> = (events: StreamEvent<V>[], info: WindowInfo) => R;

/**
 * Configuration for message delivery guarantees.
 */
export interface DeliveryConfig {
  /** The delivery semantics to enforce. */
  semantics: DeliverySemantics;
  /** Maximum number of delivery retry attempts. */
  maxRetries: number;
  /** Backoff duration between retries. */
  retryBackoff: Duration;
  /** Optional dead-letter topic for failed deliveries. */
  deadLetterTopic?: string;
  /** Whether idempotent processing is enabled. */
  enableIdempotence: boolean;
}

/**
 * Create a default delivery configuration.
 *
 * @param options - Partial overrides for any config field.
 * @returns A complete {@link DeliveryConfig} with sensible defaults.
 */
export function createDeliveryConfig(
  options?: Partial<DeliveryConfig>
): DeliveryConfig {
  return {
    semantics: options?.semantics ?? DeliverySemantics.AtLeastOnce,
    maxRetries: options?.maxRetries ?? 3,
    retryBackoff: options?.retryBackoff ?? Duration.fromSeconds(1),
    deadLetterTopic: options?.deadLetterTopic,
    enableIdempotence: options?.enableIdempotence ?? false,
  };
}
