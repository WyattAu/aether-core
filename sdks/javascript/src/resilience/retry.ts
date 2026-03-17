/**
 * Retry Pattern Implementation.
 * Handles transient failures with configurable backoff strategies.
 * @module aether/resilience/retry
 */

import {
  BackoffStrategy,
  RetryConfig,
  RetryResult,
  AsyncFunction,
} from './types';
import { withTracing } from './tracing';

/**
 * Error thrown when all retry attempts are exhausted.
 */
export class RetryExhaustedError extends Error {
  constructor(
    public readonly attempts: number,
    public readonly lastError: Error,
    message?: string
  ) {
    super(
      message || `All ${attempts} retry attempts exhausted: ${lastError.message}`
    );
    this.name = 'RetryExhaustedError';
  }
}

/**
 * Default retry configuration.
 */
const DEFAULT_CONFIG: RetryConfig = {
  maxAttempts: 3,
  initialDelay: 100,
  maxDelay: 30000,
  multiplier: 2,
  strategy: BackoffStrategy.ExponentialJitter,
  jitterFactor: 0.1,
  name: 'default',
};

/**
 * Calculate delay for a given attempt using the configured strategy.
 */
function calculateDelay(
  attempt: number,
  config: RetryConfig
): number {
  let delay: number;

  switch (config.strategy) {
    case BackoffStrategy.Fixed:
      delay = config.initialDelay;
      break;

    case BackoffStrategy.Linear:
      delay = config.initialDelay * attempt;
      break;

    case BackoffStrategy.Exponential:
      delay = config.initialDelay * Math.pow(config.multiplier, attempt - 1);
      break;

    case BackoffStrategy.ExponentialJitter:
    default:
      delay = config.initialDelay * Math.pow(config.multiplier, attempt - 1);
      // Add jitter: ±jitterFactor
      const jitter = delay * config.jitterFactor;
      delay = delay + (Math.random() * 2 - 1) * jitter;
      break;
  }

  // Cap at max delay
  return Math.min(delay, config.maxDelay);
}

/**
 * Sleep for a specified duration.
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Retry Policy implementation.
 *
 * @example
 * ```typescript
 * const retry = new RetryPolicy({
 *   maxAttempts: 3,
 *   strategy: BackoffStrategy.ExponentialJitter,
 * });
 *
 * const result = await retry.execute(() => fetchData());
 * ```
 */
export class RetryPolicy {
  private readonly config: RetryConfig;

  constructor(config: Partial<RetryConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Get the retry configuration.
   */
  getConfig(): RetryConfig {
    return { ...this.config };
  }

  /**
   * Execute a function with retry logic.
   *
   * @param fn - Async function to execute
   * @param shouldRetry - Optional predicate to determine if retry should occur
   * @returns Retry result with outcome
   */
  async execute<T>(
    fn: AsyncFunction<T>,
    shouldRetry?: (error: Error, attempt: number) => boolean
  ): Promise<RetryResult<T>> {
    const startTime = Date.now();
    let lastError: Error | null = null;
    let attempt = 0;

    while (attempt < this.config.maxAttempts) {
      attempt++;

      try {
        const result = await withTracing(
          `retry.${this.config.name}.attempt`,
          async (span) => {
            span.setAttribute('retry.name', this.config.name);
            span.setAttribute('retry.attempt', attempt);
            span.setAttribute('retry.max_attempts', this.config.maxAttempts);

            return fn();
          }
        );

        return {
          result,
          error: null,
          attempts: attempt,
          totalTime: Date.now() - startTime,
          success: true,
        };
      } catch (error) {
        lastError = error as Error;

        // Check if we should retry
        const shouldRetryResult =
          attempt < this.config.maxAttempts &&
          (shouldRetry ? shouldRetry(lastError, attempt) : this.defaultShouldRetry(lastError));

        if (!shouldRetryResult) {
          break;
        }

        // Calculate and apply delay
        const delay = calculateDelay(attempt, this.config);
        await sleep(delay);
      }
    }

    return {
      result: null,
      error: lastError,
      attempts: attempt,
      totalTime: Date.now() - startTime,
      success: false,
    };
  }

  /**
   * Execute and throw if all retries fail.
   *
   * @param fn - Async function to execute
   * @param shouldRetry - Optional predicate to determine if retry should occur
   * @returns Result of the function
   * @throws RetryExhaustedError if all retries fail
   */
  async executeOrThrow<T>(
    fn: AsyncFunction<T>,
    shouldRetry?: (error: Error, attempt: number) => boolean
  ): Promise<T> {
    const result = await this.execute(fn, shouldRetry);

    if (result.success) {
      return result.result!;
    }

    throw new RetryExhaustedError(result.attempts, result.error!);
  }

  /**
   * Default retry decision logic.
   * Retries on network errors, timeouts, and 5xx responses.
   */
  private defaultShouldRetry(error: Error): boolean {
    // Network errors
    if (
      error.message.includes('ECONNRESET') ||
      error.message.includes('ETIMEDOUT') ||
      error.message.includes('ENOTFOUND') ||
      error.message.includes('ECONNREFUSED')
    ) {
      return true;
    }

    // HTTP 5xx errors
    if (error.message.includes('500') || error.message.includes('502') || 
        error.message.includes('503') || error.message.includes('504')) {
      return true;
    }

    // Timeout errors
    if (error.message.includes('timeout') || error.message.includes('Timeout')) {
      return true;
    }

    return false;
  }
}

/**
 * Pre-configured network retry policy (3 attempts, exponential backoff).
 */
export function networkRetryPolicy(): RetryPolicy {
  return new RetryPolicy({
    maxAttempts: 3,
    initialDelay: 100,
    maxDelay: 10000,
    multiplier: 2,
    strategy: BackoffStrategy.ExponentialJitter,
    name: 'network',
  });
}

/**
 * Pre-configured database retry policy (5 attempts, longer delays).
 */
export function databaseRetryPolicy(): RetryPolicy {
  return new RetryPolicy({
    maxAttempts: 5,
    initialDelay: 200,
    maxDelay: 30000,
    multiplier: 2,
    strategy: BackoffStrategy.ExponentialJitter,
    name: 'database',
  });
}

/**
 * Aggressive retry policy (10 attempts, quick backoff).
 */
export function aggressiveRetryPolicy(): RetryPolicy {
  return new RetryPolicy({
    maxAttempts: 10,
    initialDelay: 50,
    maxDelay: 5000,
    multiplier: 1.5,
    strategy: BackoffStrategy.ExponentialJitter,
    name: 'aggressive',
  });
}

/**
 * Conservative retry policy (2 attempts, long delays).
 */
export function conservativeRetryPolicy(): RetryPolicy {
  return new RetryPolicy({
    maxAttempts: 2,
    initialDelay: 1000,
    maxDelay: 10000,
    multiplier: 2,
    strategy: BackoffStrategy.Fixed,
    name: 'conservative',
  });
}
