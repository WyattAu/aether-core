/**
 * Resilient Executor - Combines all resilience patterns.
 * @module aether/resilience/executor
 */

import { CircuitBreaker } from './circuit_breaker';
import { RetryPolicy } from './retry';
import { RateLimiter } from './rate_limiter';
import { Bulkhead } from './bulkhead';
import { AsyncFunction } from './types';
import { withTracing } from './tracing';

/**
 * Configuration for ResilientExecutor.
 */
export interface ResilientExecutorConfig {
  /** Circuit breaker instance */
  breaker?: CircuitBreaker;
  /** Retry policy instance */
  retry?: RetryPolicy;
  /** Rate limiter instance */
  rateLimiter?: RateLimiter;
  /** Bulkhead instance */
  bulkhead?: Bulkhead;
  /** Name for tracing */
  name?: string;
}

/**
 * Combined resilience executor that applies all patterns.
 *
 * Order of operations:
 * 1. Rate limiting (check if request is allowed)
 * 2. Bulkhead (check capacity)
 * 3. Circuit breaker (check if service is healthy)
 * 4. Retry (handle transient failures)
 *
 * @example
 * ```typescript
 * const executor = new ResilientExecutor({
 *   breaker: new CircuitBreaker({ failureThreshold: 5 }),
 *   retry: new RetryPolicy({ maxAttempts: 3 }),
 *   rateLimiter: new RateLimiter({ maxRequests: 100 }),
 *   bulkhead: new Bulkhead({ maxConcurrent: 10 }),
 * });
 *
 * const result = await executor.execute(() => fetchData());
 * ```
 */
export class ResilientExecutor {
  private readonly breaker?: CircuitBreaker;
  private readonly retry?: RetryPolicy;
  private readonly rateLimiter?: RateLimiter;
  private readonly bulkhead?: Bulkhead;
  private readonly name: string;

  constructor(config: ResilientExecutorConfig = {}) {
    this.breaker = config.breaker;
    this.retry = config.retry;
    this.rateLimiter = config.rateLimiter;
    this.bulkhead = config.bulkhead;
    this.name = config.name ?? 'default';
  }

  /**
   * Execute a function with all configured resilience patterns.
   *
   * @param fn - Async function to execute
   * @returns Result of the function
   * @throws CircuitBreakerError if circuit is open
   * @throws RetryExhaustedError if all retries fail
   * @throws RateLimitExhaustedError if rate limit exceeded
   * @throws BulkheadRejectedError if bulkhead at capacity
   */
  async execute<T>(fn: AsyncFunction<T>): Promise<T> {
    return withTracing(`resilient_executor.${this.name}.execute`, async (span) => {
      span.setAttribute('resilient_executor.name', this.name);
      span.setAttribute('resilient_executor.has_circuit_breaker', !!this.breaker);
      span.setAttribute('resilient_executor.has_retry', !!this.retry);
      span.setAttribute('resilient_executor.has_rate_limiter', !!this.rateLimiter);
      span.setAttribute('resilient_executor.has_bulkhead', !!this.bulkhead);

      // 1. Apply rate limiting first
      if (this.rateLimiter) {
        await this.rateLimiter.acquire();
        span.addEvent('rate_limit.passed');
      }

      // 2. Apply bulkhead
      if (this.bulkhead) {
        return this.bulkhead.execute(() => this.executeWithRetry(fn, span));
      }

      return this.executeWithRetry(fn, span);
    });
  }

  /**
   * Execute with retry and circuit breaker.
   */
  private async executeWithRetry<T>(
    fn: AsyncFunction<T>,
    span: any
  ): Promise<T> {
    // 3. Apply circuit breaker
    if (this.breaker) {
      return this.breaker.execute(() => this.executeWithRetryInternal(fn, span));
    }

    return this.executeWithRetryInternal(fn, span);
  }

  /**
   * Execute with retry logic.
   */
  private async executeWithRetryInternal<T>(
    fn: AsyncFunction<T>,
    span: any
  ): Promise<T> {
    // 4. Apply retry
    if (this.retry) {
      const result = await this.retry.execute(fn);
      if (result.success) {
        span.setAttribute('resilient_executor.result', 'success');
        return result.result!;
      }
      span.setAttribute('resilient_executor.result', 'failed');
      span.setAttribute('resilient_executor.attempts', result.attempts);
      throw result.error!;
    }

    const result = await fn();
    span.setAttribute('resilient_executor.result', 'success');
    return result;
  }

  /**
   * Create a builder for constructing a ResilientExecutor.
   */
  static builder(): ResilientExecutorBuilder {
    return new ResilientExecutorBuilder();
  }
}

/**
 * Builder for ResilientExecutor.
 *
 * @example
 * ```typescript
 * const executor = ResilientExecutor.builder()
 *   .withCircuitBreaker({ failureThreshold: 5 })
 *   .withRetry({ maxAttempts: 3 })
 *   .withRateLimiter({ maxRequests: 100 })
 *   .withBulkhead({ maxConcurrent: 10 })
 *   .build();
 * ```
 */
export class ResilientExecutorBuilder {
  private config: ResilientExecutorConfig = {};

  /**
   * Add a circuit breaker.
   */
  withCircuitBreaker(config?: ConstructorParameters<typeof CircuitBreaker>[0]): this {
    this.config.breaker = new CircuitBreaker(config ?? {});
    return this;
  }

  /**
   * Add an existing circuit breaker.
   */
  withExistingCircuitBreaker(breaker: CircuitBreaker): this {
    this.config.breaker = breaker;
    return this;
  }

  /**
   * Add a retry policy.
   */
  withRetry(config?: ConstructorParameters<typeof RetryPolicy>[0]): this {
    this.config.retry = new RetryPolicy(config ?? {});
    return this;
  }

  /**
   * Add an existing retry policy.
   */
  withExistingRetry(retry: RetryPolicy): this {
    this.config.retry = retry;
    return this;
  }

  /**
   * Add a rate limiter.
   */
  withRateLimiter(config?: ConstructorParameters<typeof RateLimiter>[0]): this {
    this.config.rateLimiter = new RateLimiter(config ?? {});
    return this;
  }

  /**
   * Add an existing rate limiter.
   */
  withExistingRateLimiter(rateLimiter: RateLimiter): this {
    this.config.rateLimiter = rateLimiter;
    return this;
  }

  /**
   * Add a bulkhead.
   */
  withBulkhead(config?: ConstructorParameters<typeof Bulkhead>[0]): this {
    this.config.bulkhead = new Bulkhead(config ?? {});
    return this;
  }

  /**
   * Add an existing bulkhead.
   */
  withExistingBulkhead(bulkhead: Bulkhead): this {
    this.config.bulkhead = bulkhead;
    return this;
  }

  /**
   * Set the name for tracing.
   */
  withName(name: string): this {
    this.config.name = name;
    return this;
  }

  /**
   * Build the ResilientExecutor.
   */
  build(): ResilientExecutor {
    return new ResilientExecutor(this.config);
  }
}
