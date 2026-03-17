/**
 * Bulkhead Pattern Implementation.
 * Isolates resources to prevent cascading failures from overwhelming the system.
 * @module aether/resilience/bulkhead
 */

import {
  BulkheadConfig,
  BulkheadStats,
  AsyncFunction,
} from './types';
import { withTracing } from './tracing';

/**
 * Error thrown when bulkhead is at capacity.
 */
export class BulkheadRejectedError extends Error {
  constructor(
    public readonly name: string,
    public readonly active: number,
    public readonly maxConcurrent: number,
    message?: string
  ) {
    super(
      message ||
        `Bulkhead '${name}' rejected: ${active}/${maxConcurrent} calls active`
    );
    this.name = 'BulkheadRejectedError';
  }
}

/**
 * Error thrown when bulkhead queue times out.
 */
export class BulkheadTimeoutError extends Error {
  constructor(
    public readonly name: string,
    public readonly queueTime: number,
    message?: string
  ) {
    super(
      message ||
        `Bulkhead '${name}' queue timeout after ${queueTime}ms`
    );
    this.name = 'BulkheadTimeoutError';
  }
}

/**
 * Default bulkhead configuration.
 */
const DEFAULT_CONFIG: BulkheadConfig = {
  maxConcurrent: 10,
  maxQueueSize: 0,
  queueTimeout: 0,
  name: 'default',
};

/**
 * Queued call representation.
 */
interface QueuedCall<T> {
  resolve: (value: T) => void;
  reject: (error: Error) => void;
  fn: AsyncFunction<T>;
  queuedAt: number;
}

/**
 * Bulkhead implementation.
 *
 * Limits the number of concurrent calls to a resource, optionally
 * queueing excess requests.
 *
 * @example
 * ```typescript
 * const bulkhead = new Bulkhead({
 *   maxConcurrent: 10,
 *   maxQueueSize: 5,
 *   queueTimeout: 5000,
 * });
 *
 * try {
 *   const result = await bulkhead.execute(() => fetchData());
 * } catch (e) {
 *   if (e instanceof BulkheadRejectedError) {
 *     // Too many concurrent calls
 *   }
 * }
 * ```
 */
export class Bulkhead {
  private readonly config: BulkheadConfig;
  private active = 0;
  private queue: QueuedCall<unknown>[] = [];
  private rejected = 0;
  private accepted = 0;

  constructor(config: Partial<BulkheadConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Get bulkhead statistics.
   */
  getStats(): BulkheadStats {
    return {
      active: this.active,
      queueSize: this.queue.length,
      available: Math.max(0, this.config.maxConcurrent - this.active),
      rejected: this.rejected,
      accepted: this.accepted,
    };
  }

  /**
   * Execute a function with bulkhead protection.
   *
   * @param fn - Async function to execute
   * @returns Result of the function
   * @throws BulkheadRejectedError if at capacity and no queue
   * @throws BulkheadTimeoutError if queued and times out
   */
  async execute<T>(fn: AsyncFunction<T>): Promise<T> {
    return withTracing(`bulkhead.${this.config.name}.execute`, async (span) => {
      span.setAttribute('bulkhead.name', this.config.name);
      span.setAttribute('bulkhead.max_concurrent', this.config.maxConcurrent);
      span.setAttribute('bulkhead.active', this.active);

      // Try to acquire a permit
      if (this.active < this.config.maxConcurrent) {
        return this.executeImmediately(fn, span);
      }

      // Try to queue if enabled
      if (this.config.maxQueueSize > 0) {
        return this.executeQueued(fn, span);
      }

      // Reject
      this.rejected++;
      span.setAttribute('bulkhead.result', 'rejected');
      throw new BulkheadRejectedError(
        this.config.name,
        this.active,
        this.config.maxConcurrent
      );
    });
  }

  /**
   * Execute immediately (permit acquired).
   */
  private async executeImmediately<T>(
    fn: AsyncFunction<T>,
    span: TracingSpan
  ): Promise<T> {
    this.active++;
    this.accepted++;
    span.setAttribute('bulkhead.result', 'accepted');

    try {
      return await fn();
    } finally {
      this.active--;
      this.processQueue();
    }
  }

  /**
   * Execute with queuing.
   */
  private executeQueued<T>(
    fn: AsyncFunction<T>,
    span: TracingSpan
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      // Check queue capacity
      if (this.queue.length >= this.config.maxQueueSize) {
        this.rejected++;
        span.setAttribute('bulkhead.result', 'rejected');
        reject(
          new BulkheadRejectedError(
            this.config.name,
            this.active,
            this.config.maxConcurrent
          )
        );
        return;
      }

      // Add to queue
      const queuedAt = Date.now();
      this.queue.push({
        resolve: resolve as (value: unknown) => void,
        reject,
        fn: fn as AsyncFunction<unknown>,
        queuedAt,
      });

      span.setAttribute('bulkhead.result', 'queued');
      span.setAttribute('bulkhead.queue_position', this.queue.length);

      // Start queue timeout if configured
      if (this.config.queueTimeout > 0) {
        setTimeout(() => {
          // Find and remove from queue
          const index = this.queue.findIndex(
            (item) => item.queuedAt === queuedAt
          );
          if (index !== -1) {
            this.queue.splice(index, 1);
            reject(
              new BulkheadTimeoutError(
                this.config.name,
                Date.now() - queuedAt
              )
            );
          }
        }, this.config.queueTimeout);
      }
    });
  }

  /**
   * Process queued calls when capacity becomes available.
   */
  private processQueue(): void {
    while (
      this.queue.length > 0 &&
      this.active < this.config.maxConcurrent
    ) {
      const item = this.queue.shift();
      if (!item) {
        break;
      }

      // Check if queue timeout expired
      if (
        this.config.queueTimeout > 0 &&
        Date.now() - item.queuedAt >= this.config.queueTimeout
      ) {
        item.reject(
          new BulkheadTimeoutError(
            this.config.name,
            Date.now() - item.queuedAt
          )
        );
        continue;
      }

      // Execute
      this.active++;
      this.accepted++;
      item.fn()
        .then(item.resolve)
        .catch(item.reject)
        .finally(() => {
          this.active--;
          this.processQueue();
        });
    }
  }

  /**
   * Check if bulkhead has available capacity.
   */
  hasCapacity(): boolean {
    return this.active < this.config.maxConcurrent;
  }

  /**
   * Get number of available permits.
   */
  availablePermits(): number {
    return Math.max(0, this.config.maxConcurrent - this.active);
  }

  /**
   * Reset bulkhead state.
   */
  reset(): void {
    // Reject all queued items
    for (const item of this.queue) {
      item.reject(new Error('Bulkhead reset'));
    }
    this.queue = [];
    this.active = 0;
    this.rejected = 0;
    this.accepted = 0;
  }
}

/**
 * Tracing span interface (minimal).
 */
interface TracingSpan {
  setAttribute: (key: string, value: unknown) => void;
  addEvent: (name: string, attributes?: Record<string, unknown>) => void;
  end: (error?: Error) => void;
}

/**
 * Manager for multiple bulkheads.
 */
export class BulkheadManager {
  private bulkheads: Map<string, Bulkhead> = new Map();
  private defaultConfig: BulkheadConfig;

  constructor(defaultConfig: Partial<BulkheadConfig> = {}) {
    this.defaultConfig = { ...DEFAULT_CONFIG, ...defaultConfig };
  }

  /**
   * Get or create a bulkhead by name.
   */
  getBulkhead(name: string, config?: Partial<BulkheadConfig>): Bulkhead {
    if (!this.bulkheads.has(name)) {
      this.bulkheads.set(
        name,
        new Bulkhead({
          ...this.defaultConfig,
          ...config,
          name,
        })
      );
    }
    return this.bulkheads.get(name)!;
  }

  /**
   * Get all bulkhead names.
   */
  getNames(): string[] {
    return Array.from(this.bulkheads.keys());
  }

  /**
   * Clear all bulkheads.
   */
  clear(): void {
    for (const bulkhead of this.bulkheads.values()) {
      bulkhead.reset();
    }
    this.bulkheads.clear();
  }
}

// ============================================
// Pre-configured Bulkheads
// ============================================

/**
 * Pre-configured API bulkhead (25 concurrent).
 */
export function apiBulkhead(): Bulkhead {
  return new Bulkhead({
    maxConcurrent: 25,
    maxQueueSize: 10,
    queueTimeout: 5000,
    name: 'api',
  });
}

/**
 * Pre-configured database bulkhead (10 concurrent).
 */
export function databaseBulkhead(): Bulkhead {
  return new Bulkhead({
    maxConcurrent: 10,
    maxQueueSize: 20,
    queueTimeout: 10000,
    name: 'database',
  });
}

/**
 * Pre-configured strict bulkhead (5 concurrent, no queue).
 */
export function strictBulkhead(): Bulkhead {
  return new Bulkhead({
    maxConcurrent: 5,
    maxQueueSize: 0,
    name: 'strict',
  });
}
