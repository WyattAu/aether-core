/**
 * Batch Processing Optimization
 * 
 * @module aether/streaming/batch
 */

import { StreamEvent, Duration } from './types';

/**
 * Batch configuration
 */
export interface BatchConfig {
  maxBatchSize: number;
  maxWaitTimeMs: number;
  maxBytes: number;
  timeoutOnFull: boolean;
  partialOnTimeout: boolean;
  partialOnShutdown: boolean;
  parallel: boolean;
  maxParallelBatches: number;
  batchTimeoutMs: number;
  retryOnFailure: boolean;
  retryDelayMs: number;
  retryBackoff: number;
  enableAsync: boolean;
  adaptiveBatching: boolean;
  batchTimeoutFactor: number;
  maxConcurrency: number;
}

/**
 * Default batch configuration
 */
export function defaultBatchConfig(): BatchConfig {
  return {
    maxBatchSize: 1000,
    maxWaitTimeMs: 100,
    maxBytes: 1024 * 1024, // 1MB
    timeoutOnFull: true,
    partialOnTimeout: true,
    partialOnShutdown: true,
    parallel: false,
    maxParallelBatches: 10,
    batchTimeoutMs: 1000,
    retryOnFailure: true,
    retryDelayMs: 100,
    retryBackoff: 2.0,
    enableAsync: true,
    adaptiveBatching: false,
    batchTimeoutFactor: 1.5,
    maxConcurrency: 4,
  };
}

/**
 * Batch result
 */
export interface BatchResult<T> {
  items: T[];
  sizeBytes: number;
  processingTimeMs: number;
  batchId: string;
  timestamp: Date;
  aggregated?: unknown;
  aggregationKey?: string;
  checksum?: string;
}

/**
 * Batch statistics
 */
export interface BatchStats {
  totalItems: number;
  totalBatches: number;
  totalBytes: number;
  totalProcessingTimeMs: number;
  minProcessingTimeMs: number;
  maxProcessingTimeMs: number;
  avgBatchSize: number;
  failedBatches: number;
  startTime?: Date;
  endTime?: Date;
}

/**
 * Batch collector for gathering items into batches
 */
export class BatchCollector<T> {
  private config: BatchConfig;
  private items: T[] = [];
  private currentBytes: number = 0;
  private batchStartTime?: number;
  private batchCount: number = 0;

  constructor(config: BatchConfig) {
    this.config = config;
  }

  /**
   * Add an item to the current batch
   */
  add(item: T, sizeBytes: number = 0): BatchResult<T> | null {
    // Initialize batch timing
    if (!this.batchStartTime) {
      this.batchStartTime = Date.now();
    }

    // Add item
    this.items.push(item);
    this.currentBytes += sizeBytes;

    // Check if batch should be flushed
    if (this.shouldFlush()) {
      return this.flush();
    }

    return null;
  }

  /**
   * Add multiple items at once
   */
  addMany(items: T[], sizeBytes: number = 0): BatchResult<T> | null {
    if (items.length === 0) {
      return null;
    }

    const itemSize = Math.floor(sizeBytes / items.length);
    for (const item of items) {
      const result = this.add(item, itemSize);
      if (result) {
        return result;
      }
    }
    return null;
  }

  private shouldFlush(): boolean {
    if (this.items.length >= this.config.maxBatchSize) {
      return true;
    }
    if (this.currentBytes >= this.config.maxBytes) {
      return true;
    }
    if (this.batchStartTime) {
      const elapsedMs = Date.now() - this.batchStartTime;
      if (elapsedMs >= this.config.maxWaitTimeMs) {
        return this.config.timeoutOnFull;
      }
    }
    return false;
  }

  /**
   * Flush the current batch
   */
  flush(): BatchResult<T> | null {
    if (this.items.length === 0) {
      return null;
    }

    let processingTimeMs = 0;
    if (this.batchStartTime) {
      processingTimeMs = Date.now() - this.batchStartTime;
    }

    this.batchCount++;
    const batchId = `batch-${Date.now()}-${this.batchCount}`;

    const result: BatchResult<T> = {
      items: this.items,
      sizeBytes: this.currentBytes,
      processingTimeMs,
      batchId,
      timestamp: new Date(),
    };

    // Reset
    this.items = [];
    this.currentBytes = 0;
    this.batchStartTime = undefined;

    return result;
  }

  /**
   * Get current batch size
   */
  get currentSize(): number {
    return this.items.length;
  }

  /**
   * Get current byte size
   */
  get currentBytes(): number {
    return this.currentBytes;
  }

  /**
   * Check if batch is empty
   */
  isEmpty(): boolean {
    return this.items.length === 0;
  }
}

/**
 * Batch aggregator function type
 */
export type AggregateFunc<T, R> = (batch: T[]) => R;

/**
 * Batch aggregator for combining items
 */
export class BatchAggregator<T, R> {
  private aggregateFunc: AggregateFunc<T, R>;
  private keyExtractor?: (item: T) => string;
  private batchCount: number = 0;
  private totalEvents: number = 0;
  private processingTimeMs: number = 0;

  constructor(aggregateFunc: AggregateFunc<T, R>, keyExtractor?: (item: T) => string) {
    this.aggregateFunc = aggregateFunc;
    this.keyExtractor = keyExtractor;
  }

  /**
   * Aggregate a batch of items
   */
  aggregate(batch: T[], key?: string): R {
    if (batch.length === 0) {
      throw new Error('Batch cannot be empty');
    }

    const startTime = Date.now();

    try {
      const result = this.aggregateFunc(batch);

      // Update stats
      this.batchCount++;
      this.totalEvents += batch.length;
      this.processingTimeMs += Date.now() - startTime;

      return result;
    } catch (e) {
      this.processingTimeMs += Date.now() - startTime;
      throw e;
    }
  }

  /**
   * Get aggregator statistics
   */
  getStats(): { batchCount: number; totalEvents: number; processingTimeMs: number } {
    return {
      batchCount: this.batchCount,
      totalEvents: this.totalEvents,
      processingTimeMs: this.processingTimeMs,
    };
  }
}

/**
 * Batch emitter for sending results downstream
 */
export class BatchEmitter<T> {
  private handlers: Array<(batch: BatchResult<T>) => Promise<void>> = [];

  /**
   * Add a handler for batch results
   */
  addHandler(handler: (batch: BatchResult<T>) => Promise<void>): void {
    this.handlers.push(handler);
  }

  /**
   * Emit batch to all handlers
   */
  async emit(batch: BatchResult<T>): Promise<void> {
    for (const handler of this.handlers) {
      await handler(batch);
    }
  }
}

/**
 * Batch processor for processing events in batches
 */
export class BatchProcessor<T> {
  private config: BatchConfig;
  private collector: BatchCollector<T>;
  private queue: BatchResult<T>[] = [];
  private running: boolean = false;
  private stats: BatchStats = {
    totalItems: 0,
    totalBatches: 0,
    totalBytes: 0,
    totalProcessingTimeMs: 0,
    minProcessingTimeMs: Infinity,
    maxProcessingTimeMs: 0,
    avgBatchSize: 0,
    failedBatches: 0,
  };

  constructor(config: BatchConfig) {
    this.config = config;
    this.collector = new BatchCollector<T>(config);
  }

  /**
   * Start the batch processor
   */
  async start(): Promise<void> {
    this.running = true;
    this.stats.startTime = new Date();
  }

  /**
   * Stop the batch processor
   */
  async stop(): Promise<void> {
    this.running = false;

    // Process remaining batches
    while (this.queue.length > 0) {
      const batch = this.queue.shift()!;
      this.processBatch(batch);
    }

    // Flush collector
    const remaining = this.collector.flush();
    if (remaining) {
      this.processBatch(remaining);
    }

    this.stats.endTime = new Date();
  }

  /**
   * Add an event to the batch processor
   */
  async add(event: StreamEvent<T>): Promise<boolean> {
    if (!this.running) {
      throw new Error('Batch processor not running');
    }

    // Get event size
    let sizeBytes = 0;
    if (typeof event.value === 'string') {
      sizeBytes = event.value.length;
    } else if (event.value instanceof Uint8Array) {
      sizeBytes = event.value.length;
    }

    // Try to add to collector
    const batchResult = this.collector.add(event.value, sizeBytes);

    if (batchResult) {
      this.processBatch(batchResult);
      return true;
    }

    return false;
  }

  private processBatch(batch: BatchResult<T>): void {
    const startTime = Date.now();

    this.stats.totalBatches++;
    this.stats.totalItems += batch.items.length;
    this.stats.totalBytes += batch.sizeBytes;

    const processingTimeMs = Date.now() - startTime;
    this.stats.totalProcessingTimeMs += processingTimeMs;

    if (processingTimeMs < this.stats.minProcessingTimeMs) {
      this.stats.minProcessingTimeMs = processingTimeMs;
    }
    if (processingTimeMs > this.stats.maxProcessingTimeMs) {
      this.stats.maxProcessingTimeMs = processingTimeMs;
    }
  }

  /**
   * Get current statistics
   */
  getStats(): BatchStats {
    const stats = { ...this.stats };
    if (stats.totalBatches > 0) {
      stats.avgBatchSize = stats.totalItems / stats.totalBatches;
    }
    return stats;
  }
}
