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
 * Configuration for {@link ResilientExecutor}.
 */
export interface ResilientExecutorConfig {
  /** Optional circuit breaker instance. */
  breaker?: CircuitBreaker;
  /** Optional retry policy instance. */
  retry?: RetryPolicy;
  /** Optional rate limiter instance. */
  rateLimiter?: RateLimiter;
  /** Optional bulkhead instance. */
  bulkhead?: Bulkhead;
  /** Name for tracing spans (default: `'default'`). */
  name?: string;
}

/**
 * Combined resilience executor that applies all configured patterns.
 *
 * The executor chains resilience patterns in the following order:
 *
 * 1. **Rate limiting** — checks if the request is allowed.
 * 2. **Bulkhead** — checks concurrency capacity.
 * 3. **Circuit breaker** — checks if the downstream service is healthy.
 * 4. **Retry** — handles transient failures with backoff.
 *
 * Each pattern is optional; only the configured ones are applied.
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

  /**
   * Create a new ResilientExecutor.
   *
   * @param config - Configuration with optional resilience components.
   */
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
   * @typeParam T - The return type of the function.
   * @param fn - Async function to execute.
   * @returns Result of the function.
   * @throws CircuitBreakerError   If the circuit is open.
   * @throws RetryExhaustedError   If all retries fail.
   * @throws RateLimitExhaustedError If rate limit is exceeded.
   * @throws BulkheadRejectedError If bulkhead is at capacity.
   * @throws Error                 Any error thrown by the function itself.
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
   *
   * @typeParam T - The return type.
   * @param fn   - The async function.
   * @param span - The tracing span.
   * @returns The function result.
   * @internal
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
   *
   * @typeParam T - The return type.
   * @param fn   - The async function.
   * @param span - The tracing span.
   * @returns The function result.
   * @internal
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
   * Create a builder for fluently constructing a ResilientExecutor.
   *
   * @returns A new {@link ResilientExecutorBuilder}.
   */
  static builder(): ResilientExecutorBuilder {
    return new ResilientExecutorBuilder();
  }
}

/**
 * Fluent builder for constructing a {@link ResilientExecutor}.
 *
 * Each `with*` method returns `this` for chaining.
 *
 * @example
 * ```typescript
 * const executor = ResilientExecutor.builder()
 *   .withCircuitBreaker({ failureThreshold: 5 })
 *   .withRetry({ maxAttempts: 3 })
 *   .withRateLimiter({ maxRequests: 100 })
 *   .withBulkhead({ maxConcurrent: 10 })
 *   .withName('api-calls')
 *   .build();
 * ```
 */
export class ResilientExecutorBuilder {
  private config: ResilientExecutorConfig = {};

  /**
   * Add a circuit breaker with the given configuration.
   *
   * @param config - Partial circuit breaker configuration.
   * @returns This builder for chaining.
   */
  withCircuitBreaker(config?: ConstructorParameters<typeof CircuitBreaker>[0]): this {
    this.config.breaker = new CircuitBreaker(config ?? {});
    return this;
  }

  /**
   * Add an existing circuit breaker instance.
   *
   * @param breaker - The CircuitBreaker to use.
   * @returns This builder for chaining.
   */
  withExistingCircuitBreaker(breaker: CircuitBreaker): this {
    this.config.breaker = breaker;
    return this;
  }

  /**
   * Add a retry policy with the given configuration.
   *
   * @param config - Partial retry policy configuration.
   * @returns This builder for chaining.
   */
  withRetry(config?: ConstructorParameters<typeof RetryPolicy>[0]): this {
    this.config.retry = new RetryPolicy(config ?? {});
    return this;
  }

  /**
   * Add an existing retry policy instance.
   *
   * @param retry - The RetryPolicy to use.
   * @returns This builder for chaining.
   */
  withExistingRetry(retry: RetryPolicy): this {
    this.config.retry = retry;
    return this;
  }

  /**
   * Add a rate limiter with the given configuration.
   *
   * @param config - Partial rate limiter configuration.
   * @returns This builder for chaining.
   */
  withRateLimiter(config?: ConstructorParameters<typeof RateLimiter>[0]): this {
    this.config.rateLimiter = new RateLimiter(config ?? {});
    return this;
  }

  /**
   * Add an existing rate limiter instance.
   *
   * @param rateLimiter - The RateLimiter to use.
   * @returns This builder for chaining.
   */
  withExistingRateLimiter(rateLimiter: RateLimiter): this {
    this.config.rateLimiter = rateLimiter;
    return this;
  }

  /**
   * Add a bulkhead with the given configuration.
   *
   * @param config - Partial bulkhead configuration.
   * @returns This builder for chaining.
   */
  withBulkhead(config?: ConstructorParameters<typeof Bulkhead>[0]): this {
    this.config.bulkhead = new Bulkhead(config ?? {});
    return this;
  }

  /**
   * Add an existing bulkhead instance.
   *
   * @param bulkhead - The Bulkhead to use.
   * @returns This builder for chaining.
   */
  withExistingBulkhead(bulkhead: Bulkhead): this {
    this.config.bulkhead = bulkhead;
    return this;
  }

  /**
   * Set the name for tracing spans.
   *
   * @param name - The executor name.
   * @returns This builder for chaining.
   */
  withName(name: string): this {
    this.config.name = name;
    return this;
  }

  /**
   * Build and return the configured ResilientExecutor.
   *
   * @returns A new ResilientExecutor with the configured resilience patterns.
   */
  build(): ResilientExecutor {
    return new ResilientExecutor(this.config);
  }
}
