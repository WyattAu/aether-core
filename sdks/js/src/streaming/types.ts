/**
 * Streaming types for Aether SDK
 * 
 * @module aether/streaming/types
 */

/**
 * Timestamp wrapper for event-time processing
 */
export class Timestamp {
  private readonly milliseconds: bigint;

  constructor(milliseconds: bigint | number) {
    this.milliseconds = typeof milliseconds === 'number' 
      ? BigInt(milliseconds) 
      : milliseconds;
  }

  static now(): Timestamp {
    return new Timestamp(BigInt(Date.now()));
  }

  static fromSeconds(seconds: number): Timestamp {
    return new Timestamp(BigInt(Math.floor(seconds * 1000)));
  }

  static fromDate(date: Date): Timestamp {
    return new Timestamp(BigInt(date.getTime()));
  }

  toDate(): Date {
    return new Date(Number(this.milliseconds));
  }

  toSeconds(): number {
    return Number(this.milliseconds) / 1000.0;
  }

  toMillis(): bigint {
    return this.milliseconds;
  }

  add(duration: Duration): Timestamp {
    return new Timestamp(this.milliseconds + duration.toMillis());
  }

  sub(other: Timestamp): Duration {
    return new Duration(this.milliseconds - other.milliseconds);
  }

  before(other: Timestamp): boolean {
    return this.milliseconds < other.milliseconds;
  }

  after(other: Timestamp): boolean {
    return this.milliseconds > other.milliseconds;
  }

  equals(other: Timestamp): boolean {
    return this.milliseconds === other.milliseconds;
  }
}

/**
 * Duration for time-based operations
 */
export class Duration {
  private readonly milliseconds: bigint;

  constructor(milliseconds: bigint | number) {
    this.milliseconds = typeof milliseconds === 'number' 
      ? BigInt(milliseconds) 
      : milliseconds;
  }

  static fromMillis(ms: number): Duration {
    return new Duration(ms);
  }

  static fromSeconds(s: number): Duration {
    return new Duration(s * 1000);
  }

  static fromMinutes(m: number): Duration {
    return new Duration(m * 60 * 1000);
  }

  static fromHours(h: number): Duration {
    return new Duration(h * 3600 * 1000);
  }

  toMillis(): bigint {
    return this.milliseconds;
  }

  toSeconds(): number {
    return Number(this.milliseconds) / 1000.0;
  }

  add(other: Duration): Duration {
    return new Duration(this.milliseconds + other.milliseconds);
  }

  mul(factor: number): Duration {
    return new Duration(Number(this.milliseconds) * factor);
  }
}

/**
 * Stream event with metadata
 */
export interface StreamEvent<T> {
  key: string;
  value: T;
  timestamp: Timestamp;
  headers: Map<string, string>;
  partition?: number;
  offset?: bigint;
  eventType?: string;
}

/**
 * Create a new stream event
 */
export function createStreamEvent<T>(key: string, value: T): StreamEvent<T> {
  return {
    key,
    value,
    timestamp: Timestamp.now(),
    headers: new Map(),
  };
}

/**
 * Create a stream event with timestamp
 */
export function createStreamEventWithTimestamp<T>(
  key: string, 
  value: T, 
  timestamp: Timestamp
): StreamEvent<T> {
  return {
    key,
    value,
    timestamp,
    headers: new Map(),
  };
}

/**
 * Watermark for event-time progress
 */
export interface Watermark {
  timestamp: Timestamp;
  streamId: string;
  partition?: number;
}

/**
 * Window types
 */
export enum WindowType {
  Tumbling = 'Tumbling',
  Sliding = 'Sliding',
  Session = 'Session',
}

/**
 * Late data handling policies
 */
export enum LateDataPolicy {
  Drop = 'Drop',
  SideOutput = 'SideOutput',
  Reprocess = 'Reprocess',
}

/**
 * Watermark strategies
 */
export enum WatermarkStrategy {
  EventTime = 'EventTime',
  ProcessingTime = 'ProcessingTime',
  BoundedOutOfOrder = 'BoundedOutOfOrder',
}

/**
 * Backpressure strategies
 */
export enum BackpressureStrategy {
  Buffer = 'Buffer',
  Drop = 'Drop',
  Fail = 'Fail',
  Latest = 'Latest',
}

/**
 * Delivery semantics
 */
export enum DeliverySemantics {
  AtMostOnce = 'AtMostOnce',
  AtLeastOnce = 'AtLeastOnce',
  ExactlyOnce = 'ExactlyOnce',
}

/**
 * Pane information
 */
export enum PaneInfo {
  Early = 'Early',
  OnTime = 'OnTime',
  Late = 'Late',
}

/**
 * Window specification
 */
export interface WindowSpec {
  type: WindowType;
  size: Duration;
  slide?: Duration;
  gap?: Duration;
  lateTolerance: Duration;
  allowedLateness: Duration;
}

/**
 * Create a tumbling window spec
 */
export function tumblingWindowSpec(size: Duration): WindowSpec {
  return {
    type: WindowType.Tumbling,
    size,
    lateTolerance: Duration.fromSeconds(0),
    allowedLateness: Duration.fromSeconds(0),
  };
}

/**
 * Create a sliding window spec
 */
export function slidingWindowSpec(size: Duration, slide: Duration): WindowSpec {
  return {
    type: WindowType.Sliding,
    size,
    slide,
    lateTolerance: Duration.fromSeconds(0),
    allowedLateness: Duration.fromSeconds(0),
  };
}

/**
 * Create a session window spec
 */
export function sessionWindowSpec(gap: Duration): WindowSpec {
  return {
    type: WindowType.Session,
    gap,
    lateTolerance: Duration.fromSeconds(0),
    allowedLateness: Duration.fromSeconds(0),
    size: Duration.fromMillis(0),
  };
}

/**
 * Window information
 */
export interface WindowInfo {
  start: Timestamp;
  end: Timestamp;
  maxTimestamp: Timestamp;
  pane: PaneInfo;
  windowId: string;
}

/**
 * Stream configuration
 */
export interface StreamConfig {
  inputStreams: string[];
  outputStreams: string[];
  parallelism: number;
  partitionStrategy: string;
  watermarkStrategy: WatermarkStrategy;
  watermarkInterval: Duration;
  outOfOrderness: Duration;
  checkpointingEnabled: boolean;
  checkpointInterval: Duration;
  lateDataPolicy: LateDataPolicy;
  lateDataOutput?: string;
  bufferCapacity: number;
  bufferTimeout: Duration;
}

/**
 * Default stream configuration
 */
export function defaultStreamConfig(): StreamConfig {
  return {
    inputStreams: [],
    outputStreams: [],
    parallelism: 1,
    partitionStrategy: 'key',
    watermarkStrategy: WatermarkStrategy.ProcessingTime,
    watermarkInterval: Duration.fromSeconds(1),
    outOfOrderness: Duration.fromMillis(0),
    checkpointingEnabled: false,
    checkpointInterval: Duration.fromMinutes(1),
    lateDataPolicy: LateDataPolicy.Drop,
    bufferCapacity: 10000,
    bufferTimeout: Duration.fromSeconds(30),
  };
}

/**
 * Backpressure configuration
 */
export interface BackpressureConfig {
  strategy: BackpressureStrategy;
  bufferSize: number;
  highWatermark: number;
  lowWatermark: number;
}

/**
 * Default backpressure configuration
 */
export function defaultBackpressureConfig(): BackpressureConfig {
  return {
    strategy: BackpressureStrategy.Buffer,
    bufferSize: 10000,
    highWatermark: 0.9,
    lowWatermark: 0.5,
  };
}

/**
 * Partition configuration
 */
export interface PartitionConfig {
  strategy: PartitionStrategy;
  partitions: number;
}

/**
 * Partition strategies
 */
export enum PartitionStrategy {
  RoundRobin = 'RoundRobin',
  Key = 'Key',
  Hash = 'Hash',
  Random = 'Random',
  Range = 'Range',
}

/**
 * Default partition configuration
 */
export function defaultPartitionConfig(): PartitionConfig {
  return {
    strategy: PartitionStrategy.Key,
    partitions: 1,
  };
}

/**
 * Delivery configuration
 */
export interface DeliveryConfig {
  semantics: DeliverySemantics;
  maxRetries: number;
  retryBackoff: Duration;
  deadLetterTopic?: string;
  enableIdempotence: boolean;
}

/**
 * Default delivery configuration
 */
export function defaultDeliveryConfig(): DeliveryConfig {
  return {
    semantics: DeliverySemantics.AtLeastOnce,
    maxRetries: 3,
    retryBackoff: Duration.fromSeconds(1),
    enableIdempotence: false,
  };
}
