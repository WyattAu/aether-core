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
  /** Fixed-size, non-overlapping windows */
  Tumbling = 'tumbling',
  /** Fixed-size, overlapping windows */
  Sliding = 'sliding',
  /** Dynamic size based on activity gaps */
  Session = 'session',
}

/**
 * How to handle late-arriving data.
 */
export enum LateDataPolicy {
  /** Discard late events */
  Drop = 'drop',
  /** Route to side output stream */
  SideOutput = 'side-output',
  /** Reprocess affected windows */
  Reprocess = 'reprocess',
}

/**
 * Watermark generation strategy.
 */
export enum WatermarkStrategy {
  /** Based on event timestamps */
  EventTime = 'event-time',
  /** Based on processing time */
  ProcessingTime = 'processing-time',
  /** Allow bounded lateness */
  BoundedOutOfOrder = 'bounded-out-of-order',
}

/**
 * Backpressure handling strategies.
 */
export enum BackpressureStrategy {
  /** Buffer events up to limit */
  Buffer = 'buffer',
  /** Drop events when overloaded */
  Drop = 'drop',
  /** Raise error when overloaded */
  Fail = 'fail',
  /** Keep only latest events */
  Latest = 'latest',
}

/**
 * Message delivery guarantees.
 */
export enum DeliverySemantics {
  /** Fire and forget */
  AtMostOnce = 'at-most-once',
  /** May duplicate */
  AtLeastOnce = 'at-least-once',
  /** No duplicates, no loss */
  ExactlyOnce = 'exactly-once',
}

/**
 * Window pane type.
 */
export enum PaneInfo {
  /** Early firing before watermark */
  Early = 'early',
  /** On-time firing at watermark */
  OnTime = 'on-time',
  /** Late firing after watermark */
  Late = 'late',
}

/**
 * Event timestamp with millisecond precision.
 *
 * @example
 * ```typescript
 * const now = Timestamp.now();
 * const fromSeconds = Timestamp.fromSeconds(1234567890);
 * const future = now.add(Duration.fromMinutes(5));
 * ```
 */
export class Timestamp {
  constructor(public readonly milliseconds: number) {}

  /**
   * Create timestamp from current time.
   */
  static now(): Timestamp {
    return new Timestamp(Date.now());
  }

  /**
   * Create timestamp from Date object.
   */
  static fromDate(date: Date): Timestamp {
    return new Timestamp(date.getTime());
  }

  /**
   * Create timestamp from seconds.
   */
  static fromSeconds(seconds: number): Timestamp {
    return new Timestamp(Math.floor(seconds * 1000));
  }

  /**
   * Convert to Date object.
   */
  toDate(): Date {
    return new Date(this.milliseconds);
  }

  /**
   * Convert to seconds.
   */
  toSeconds(): number {
    return this.milliseconds / 1000;
  }

  /**
   * Add a duration to this timestamp.
   */
  add(duration: Duration): Timestamp {
    return new Timestamp(this.milliseconds + duration.milliseconds);
  }

  /**
   * Subtract another timestamp to get duration.
   */
  subtract(other: Timestamp): Duration {
    return new Duration(this.milliseconds - other.milliseconds);
  }

  /**
   * Subtract a duration from this timestamp.
   */
  subtractDuration(duration: Duration): Timestamp {
    return new Timestamp(this.milliseconds - duration.milliseconds);
  }

  compareTo(other: Timestamp): number {
    return this.milliseconds - other.milliseconds;
  }

  equals(other: Timestamp): boolean {
    return this.milliseconds === other.milliseconds;
  }

  isBefore(other: Timestamp): boolean {
    return this.milliseconds < other.milliseconds;
  }

  isAfter(other: Timestamp): boolean {
    return this.milliseconds > other.milliseconds;
  }

  isBeforeOrEqual(other: Timestamp): boolean {
    return this.milliseconds <= other.milliseconds;
  }

  isAfterOrEqual(other: Timestamp): boolean {
    return this.milliseconds >= other.milliseconds;
  }

  toJSON(): number {
    return this.milliseconds;
  }

  static fromJSON(ms: number): Timestamp {
    return new Timestamp(ms);
  }
}

/**
 * Duration with millisecond precision.
 *
 * @example
 * ```typescript
 * const fiveMinutes = Duration.fromMinutes(5);
 * const oneSecond = Duration.fromSeconds(1);
 * const doubled = fiveMinutes.multiply(2);
 * ```
 */
export class Duration {
  constructor(public readonly milliseconds: number) {}

  /**
   * Create duration from milliseconds.
   */
  static fromMillis(ms: number): Duration {
    return new Duration(ms);
  }

  /**
   * Create duration from seconds.
   */
  static fromSeconds(seconds: number): Duration {
    return new Duration(Math.floor(seconds * 1000));
  }

  /**
   * Create duration from minutes.
   */
  static fromMinutes(minutes: number): Duration {
    return new Duration(Math.floor(minutes * 60 * 1000));
  }

  /**
   * Create duration from hours.
   */
  static fromHours(hours: number): Duration {
    return new Duration(Math.floor(hours * 3600 * 1000));
  }

  /**
   * Convert to seconds.
   */
  toSeconds(): number {
    return this.milliseconds / 1000;
  }

  /**
   * Get milliseconds.
   */
  toMillis(): number {
    return this.milliseconds;
  }

  /**
   * Add another duration.
   */
  add(other: Duration): Duration {
    return new Duration(this.milliseconds + other.milliseconds);
  }

  /**
   * Subtract another duration.
   */
  subtract(other: Duration): Duration {
    return new Duration(this.milliseconds - other.milliseconds);
  }

  /**
   * Multiply by a factor.
   */
  multiply(factor: number): Duration {
    return new Duration(this.milliseconds * factor);
  }
}

/**
 * Event in a stream with metadata.
 */
export interface StreamEvent<T> {
  /** Partition key */
  key: string;
  /** Event payload */
  value: T;
  /** Event timestamp */
  timestamp: Timestamp;
  /** Optional headers */
  headers?: Record<string, string>;
  /** Optional partition number */
  partition?: number;
  /** Optional offset in partition */
  offset?: number;
  /** Optional event type identifier */
  eventType?: string;
}

/**
 * Create a new stream event.
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
}

/**
 * Watermark indicating event time progress.
 */
export class Watermark {
  constructor(
    public readonly timestamp: Timestamp,
    public readonly streamId: string,
    public readonly partition?: number
  ) {}

  /**
   * Check if an event timestamp is late relative to this watermark.
   */
  isLate(eventTimestamp: Timestamp): boolean {
    return eventTimestamp.isBefore(this.timestamp);
  }

  toJSON(): object {
    return {
      timestamp: this.timestamp.milliseconds,
      streamId: this.streamId,
      partition: this.partition,
    };
  }

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
  /** Window type */
  type: WindowType;
  /** Window size */
  size: Duration;
  /** Slide interval for sliding windows */
  slide?: Duration;
  /** Gap for session windows */
  gap?: Duration;
  /** Late data tolerance */
  lateTolerance: Duration;
  /** Allowed lateness */
  allowedLateness: Duration;
}

/**
 * Create a window specification.
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
 * Information about an active window.
 */
export interface WindowInfo {
  /** Window start time */
  start: Timestamp;
  /** Window end time */
  end: Timestamp;
  /** Maximum event timestamp in window */
  maxTimestamp: Timestamp;
  /** Pane type */
  pane: PaneInfo;
  /** Optional window ID */
  windowId?: string;
}

/**
 * Create window info.
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
}

/**
 * Check if timestamp falls within this window.
 */
  contains(timestamp: Timestamp): boolean {
    return timestamp.isBeforeOrEqual(this.start) && timestamp.isBefore(this.end);
  }
}

/**
 * Check if timestamp is late for this window.
 */
  isLate(timestamp: Timestamp): boolean {
    return timestamp.isBefore(this.start);
  }
}

/**
 * Configuration for stream actors.
 */
export interface StreamConfig {
  /** Input streams */
  inputStreams: string[];
  /** Output streams */
  outputStreams: string[];
  /** Parallelism */
  parallelism: number;
  /** Partition strategy */
  partitionStrategy: 'key' | 'range' | 'hash' | 'random';
  /** Watermark strategy */
  watermarkStrategy: WatermarkStrategy;
  /** Watermark interval */
  watermarkInterval: Duration;
  /** Out-of-orderness tolerance */
  outOfOrderness: Duration;
  /** Enable checkpointing */
  checkpointingEnabled: boolean;
  /** Checkpoint interval */
  checkpointInterval: Duration;
  /** Late data policy */
  lateDataPolicy: LateDataPolicy;
  /** Side output stream for late data */
  lateDataOutput?: string;
  /** Buffer capacity */
  bufferCapacity: number;
  /** Buffer timeout */
  bufferTimeout: Duration;
}

/**
 * Create a default stream configuration.
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
  /** Backpressure strategy */
  strategy: BackpressureStrategy;
  /** Buffer size */
  bufferSize: number;
  /** High watermark (90% = 0.9) */
  highWatermark: number;
  /** Low watermark (50% = 0.5) */
  lowWatermark: number;
}

/**
 * Create a default backpressure configuration.
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
  /** Partition strategy */
  strategy: 'key' | 'range' | 'hash' | 'random';
  /** Number of partitions */
  partitions: number;
  /** Key extractor function */
  keyExtractor?: (value: unknown) => string;
}

 /**
 * Configuration for message delivery guarantees.
 */
export interface DeliveryConfig {
  /** Delivery semantics */
  semantics: DeliverySemantics;
  /** Maximum retry attempts */
  maxRetries: number;
  /** Retry backoff duration */
  retryBackoff: Duration;
  /** Dead letter topic */
  deadLetterTopic?: string;
  /** Enable idempotence */
  enableIdempotence: boolean;
}

 /**
 * Create a default delivery configuration.
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

