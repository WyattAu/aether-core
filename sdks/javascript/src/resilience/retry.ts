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
 * Error thrown when all retry attempts have been exhausted.
 *
 * Contains the total number of attempts made and the last error encountered.
 *
 * @example
 * ```typescript
 * try {
 *   await retry.executeOrThrow(() => fetchData());
 * } catch (e) {
 *   if (e instanceof RetryExhaustedError) {
 *     console.log(`Failed after ${e.attempts} attempts: ${e.lastError.message}`);
 *   }
 * }
 * ```
 */
export class RetryExhaustedError extends Error {
  /**
   * @param attempts  - Total number of attempts made.
   * @param lastError - The error from the final attempt.
   * @param message   - Optional custom message.
   */
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
 *
 * @param attempt - The 1-based attempt number.
 * @param config  - The retry configuration.
 * @returns The delay in milliseconds before the next attempt.
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
 *
 * @param ms - Duration in milliseconds.
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Retry Policy implementation.
 *
 * Wraps async functions with configurable retry logic and backoff
 * strategies. Supports custom retry predicates and provides both
 * result-returning and exception-throwing execution modes.
 *
 * @example
 * ```typescript
 * const retry = new RetryPolicy({
 *   maxAttempts: 3,
 *   strategy: BackoffStrategy.ExponentialJitter,
 * });
 *
 * const result = await retry.execute(() => fetchData());
 * // result.success indicates outcome; result.error holds the last error.
 * ```
 */
export class RetryPolicy {
  private readonly config: RetryConfig;

  /**
   * Create a new RetryPolicy.
   *
   * @param config - Partial configuration; unspecified fields use defaults.
   */
  constructor(config: Partial<RetryConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Get a copy of the current retry configuration.
   *
   * @returns The active {@link RetryConfig}.
   */
  getConfig(): RetryConfig {
    return { ...this.config };
  }

  /**
   * Execute a function with retry logic.
   *
   * On failure, the policy calculates a delay based on the configured
   * backoff strategy and retries up to `maxAttempts` times. An optional
   * `shouldRetry` predicate can override the default retry decision.
   *
   * @typeParam T - The return type of the function.
   * @param fn          - Async function to execute.
   * @param shouldRetry - Optional predicate; return `true` to retry,
   *                     `false` to stop. Receives the error and attempt number.
   * @returns A {@link RetryResult} indicating success or failure.
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
   * Execute a function with retry logic and throw on exhaustion.
   *
   * @typeParam T - The return type of the function.
   * @param fn          - Async function to execute.
   * @param shouldRetry - Optional predicate; return `true` to retry.
   * @returns The result of the function on success.
   * @throws RetryExhaustedError If all retry attempts fail.
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
   *
   * Retries on common network errors (ECONNRESET, ETIMEDOUT, ENOTFOUND,
   * ECONNREFUSED), HTTP 5xx responses, and timeout errors.
   *
   * @param error - The error to evaluate.
   * @returns `true` if the error is considered retryable.
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
 * Create a pre-configured network retry policy (3 attempts, exponential backoff).
 *
 * @returns A RetryPolicy tuned for network operations.
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
 * Create a pre-configured database retry policy (5 attempts, longer delays).
 *
 * @returns A RetryPolicy tuned for database operations.
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
 * Create an aggressive retry policy (10 attempts, quick backoff).
 *
 * @returns A RetryPolicy with high attempt count and low initial delay.
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
 * Create a conservative retry policy (2 attempts, long delays).
 *
 * @returns A RetryPolicy with low attempt count and high initial delay.
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
