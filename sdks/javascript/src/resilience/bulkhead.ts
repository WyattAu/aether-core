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
 * Error thrown when a bulkhead rejects a call because it is at capacity.
 *
 * @example
 * ```typescript
 * try {
 *   await bulkhead.execute(() => fetchData());
 * } catch (e) {
 *   if (e instanceof BulkheadRejectedError) {
 *     console.log(`${e.active}/${e.maxConcurrent} calls active`);
 *   }
 * }
 * ```
 */
export class BulkheadRejectedError extends Error {
  /**
   * @param name           - The bulkhead name.
   * @param active         - Number of currently active calls.
   * @param maxConcurrent  - Maximum allowed concurrent calls.
   * @param message        - Optional custom message.
   */
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
 * Error thrown when a queued call times out waiting for a permit.
 *
 * @example
 * ```typescript
 * if (e instanceof BulkheadTimeoutError) {
 *   console.log(`Waited ${e.queueTime}ms in queue`);
 * }
 * ```
 */
export class BulkheadTimeoutError extends Error {
  /**
   * @param name       - The bulkhead name.
   * @param queueTime  - Time spent in the queue before timeout (ms).
   * @param message    - Optional custom message.
   */
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
 *
 * @typeParam T - The result type of the queued function.
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
 * Limits the number of concurrent calls to a resource, optionally queueing
 * excess requests up to a configured limit. Calls that cannot be queued are
 * rejected with a {@link BulkheadRejectedError}.
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

  /**
   * Create a new Bulkhead.
   *
   * @param config - Partial configuration; unspecified fields use defaults.
   */
  constructor(config: Partial<BulkheadConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Get bulkhead statistics.
   *
   * @returns A snapshot of current bulkhead metrics.
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
   * If a permit is available, the function executes immediately. Otherwise,
   * if queuing is enabled, the call is queued until a permit frees up or the
   * queue times out. If no queue is configured and the bulkhead is at
   * capacity, a {@link BulkheadRejectedError} is thrown.
   *
   * @typeParam T - The return type of the function.
   * @param fn - Async function to execute.
   * @returns Result of the function.
   * @throws BulkheadRejectedError If at capacity and no queue available.
   * @throws BulkheadTimeoutError  If queued and times out.
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
   *
   * @typeParam T - The return type.
   * @param fn   - The async function.
   * @param span - The tracing span.
   * @returns The function result.
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
   *
   * @typeParam T - The return type.
   * @param fn   - The async function.
   * @param span - The tracing span.
   * @returns The function result.
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
   *
   * @returns `true` if at least one permit is available.
   */
  hasCapacity(): boolean {
    return this.active < this.config.maxConcurrent;
  }

  /**
   * Get number of available permits.
   *
   * @returns The number of permits not currently in use.
   */
  availablePermits(): number {
    return Math.max(0, this.config.maxConcurrent - this.active);
  }

  /**
   * Reset bulkhead state.
   *
   * Rejects all queued items and resets all counters.
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
 *
 * Provides named access to bulkhead instances, creating them on first access.
 *
 * @example
 * ```typescript
 * const manager = new BulkheadManager({ maxConcurrent: 10 });
 * const api = manager.getBulkhead('api', { maxConcurrent: 25 });
 * const db = manager.getBulkhead('database');
 * ```
 */
export class BulkheadManager {
  private bulkheads: Map<string, Bulkhead> = new Map();
  private defaultConfig: BulkheadConfig;

  /**
   * Create a new BulkheadManager.
   *
   * @param defaultConfig - Default configuration applied to new bulkheads.
   */
  constructor(defaultConfig: Partial<BulkheadConfig> = {}) {
    this.defaultConfig = { ...DEFAULT_CONFIG, ...defaultConfig };
  }

  /**
   * Get or create a bulkhead by name.
   *
   * @param name   - Unique name for the bulkhead.
   * @param config - Optional per-bulkhead configuration overrides.
   * @returns The existing or newly created Bulkhead.
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
   * Get all registered bulkhead names.
   *
   * @returns An array of bulkhead names.
   */
  getNames(): string[] {
    return Array.from(this.bulkheads.keys());
  }

  /**
   * Reset and remove all bulkheads.
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
 * Create a pre-configured API bulkhead (25 concurrent, 10 queued).
 *
 * @returns A Bulkhead tuned for typical API call concurrency.
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
 * Create a pre-configured database bulkhead (10 concurrent, 20 queued).
 *
 * @returns A Bulkhead tuned for database connection pooling.
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
 * Create a pre-configured strict bulkhead (5 concurrent, no queue).
 *
 * @returns A Bulkhead that immediately rejects excess calls.
 */
export function strictBulkhead(): Bulkhead {
  return new Bulkhead({
    maxConcurrent: 5,
    maxQueueSize: 0,
    name: 'strict',
  });
}
