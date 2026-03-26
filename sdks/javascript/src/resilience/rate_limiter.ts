/**
 * Rate Limiter Pattern Implementation.
 * Controls the rate of requests using various strategies.
 * @module aether/resilience/rate_limiter
 */

import {
  RateLimitStrategy,
  RateLimitConfig,
  RateLimitResult,
} from './types';
import { withTracing } from './tracing';

/**
 * Error thrown when the rate limit is exceeded and no wait is possible.
 *
 * @example
 * ```typescript
 * try {
 *   await limiter.acquire();
 * } catch (e) {
 *   if (e instanceof RateLimitExhaustedError) {
 *     console.log(`Retry after ${e.retryAfter}ms`);
 *   }
 * }
 * ```
 */
export class RateLimitExhaustedError extends Error {
  /**
   * @param retryAfter - Minimum time to wait before retrying (ms).
   * @param message    - Optional custom message.
   */
  constructor(
    public readonly retryAfter: number,
    message?: string
  ) {
    super(message || `Rate limit exceeded. Retry after ${retryAfter}ms`);
    this.name = 'RateLimitExhaustedError';
  }
}

/**
 * Default rate limiter configuration.
 */
const DEFAULT_CONFIG: RateLimitConfig = {
  maxRequests: 100,
  windowMs: 1000,
  strategy: RateLimitStrategy.SlidingWindow,
  refillRate: 100,
  name: 'default',
};

/**
 * Token Bucket rate limiter implementation.
 *
 * Maintains a pool of tokens that refill at a constant rate.
 * Each request consumes one or more tokens; if insufficient tokens
 * are available the request is rejected.
 *
 * @internal
 */
class TokenBucket {
  private tokens: number;
  private lastRefill: number;

  /**
   * @param maxTokens  - Maximum token capacity.
   * @param refillRate - Tokens added per second.
   */
  constructor(
    private readonly maxTokens: number,
    private readonly refillRate: number
  ) {
    this.tokens = maxTokens;
    this.lastRefill = Date.now();
  }

  /**
   * Try to acquire tokens from the bucket.
   *
   * @param tokens - Number of tokens to acquire (default: 1).
   * @returns A {@link RateLimitResult} indicating whether the request is allowed.
   */
  tryAcquire(tokens = 1): RateLimitResult {
    this.refill();

    if (this.tokens >= tokens) {
      this.tokens -= tokens;
      return {
        allowed: true,
        remaining: Math.floor(this.tokens),
        resetIn: 0,
        retryAfter: 0,
      };
    }

    // Calculate time until enough tokens
    const needed = tokens - this.tokens;
    const timeToWait = Math.ceil((needed / this.refillRate) * 1000);

    return {
      allowed: false,
      remaining: Math.floor(this.tokens),
      resetIn: timeToWait,
      retryAfter: timeToWait,
    };
  }

  /**
   * Refill tokens based on elapsed time.
   */
  private refill(): void {
    const now = Date.now();
    const elapsed = (now - this.lastRefill) / 1000;
    const tokensToAdd = elapsed * this.refillRate;

    this.tokens = Math.min(this.maxTokens, this.tokens + tokensToAdd);
    this.lastRefill = now;
  }
}

/**
 * Sliding Window rate limiter implementation.
 *
 * Tracks individual request timestamps within a sliding time window.
 * Allows requests while the count of timestamps within the window is
 * below the maximum.
 *
 * @internal
 */
class SlidingWindow {
  private requests: number[] = [];

  /**
   * @param maxRequests - Maximum requests per window.
   * @param windowMs    - Window duration in milliseconds.
   */
  constructor(
    private readonly maxRequests: number,
    private readonly windowMs: number
  ) {}

  /**
   * Try to acquire a permit.
   *
   * @returns A {@link RateLimitResult} indicating whether the request is allowed.
   */
  tryAcquire(): RateLimitResult {
    const now = Date.now();
    const windowStart = now - this.windowMs;

    // Remove expired requests
    this.requests = this.requests.filter((t) => t > windowStart);

    if (this.requests.length < this.maxRequests) {
      this.requests.push(now);
      return {
        allowed: true,
        remaining: this.maxRequests - this.requests.length,
        resetIn: this.requests.length > 0 ? this.requests[0] - windowStart : 0,
        retryAfter: 0,
      };
    }

    // Calculate time until oldest request expires
    const oldestRequest = this.requests[0];
    const resetIn = oldestRequest - windowStart;

    return {
      allowed: false,
      remaining: 0,
      resetIn,
      retryAfter: resetIn,
    };
  }
}

/**
 * Fixed Window rate limiter implementation.
 *
 * Divides time into fixed-size windows and resets the counter at
 * the start of each new window.
 *
 * @internal
 */
class FixedWindow {
  private count = 0;
  private windowStart: number;

  /**
   * @param maxRequests - Maximum requests per window.
   * @param windowMs    - Window duration in milliseconds.
   */
  constructor(
    private readonly maxRequests: number,
    private readonly windowMs: number
  ) {
    this.windowStart = Date.now();
  }

  /**
   * Try to acquire a permit.
   *
   * @returns A {@link RateLimitResult} indicating whether the request is allowed.
   */
  tryAcquire(): RateLimitResult {
    const now = Date.now();

    // Reset window if expired
    if (now - this.windowStart >= this.windowMs) {
      this.count = 0;
      this.windowStart = now;
    }

    if (this.count < this.maxRequests) {
      this.count++;
      return {
        allowed: true,
        remaining: this.maxRequests - this.count,
        resetIn: this.windowMs - (now - this.windowStart),
        retryAfter: 0,
      };
    }

    return {
      allowed: false,
      remaining: 0,
      resetIn: this.windowMs - (now - this.windowStart),
      retryAfter: this.windowMs - (now - this.windowStart),
    };
  }
}

/**
 * Rate Limiter implementation.
 *
 * Supports three strategies selected via {@link RateLimitConfig.strategy}:
 *
 * - **TokenBucket** — Tokens refill at a constant rate; bursty traffic is
 *   allowed up to the bucket capacity.
 * - **SlidingWindow** — Counts requests in a sliding time window for
 *   smoother rate control.
 * - **FixedWindow** — Resets a counter at fixed intervals; simpler but
 *   may allow brief bursts at window boundaries.
 *
 * @example
 * ```typescript
 * const limiter = new RateLimiter({
 *   maxRequests: 100,
 *   windowMs: 1000,
 *   strategy: RateLimitStrategy.SlidingWindow,
 * });
 *
 * const result = limiter.tryAcquire();
 * if (result.allowed) {
 *   // Process request
 * } else {
 *   // Reject or wait using result.retryAfter
 * }
 * ```
 */
export class RateLimiter {
  private readonly config: RateLimitConfig;
  private tokenBucket: TokenBucket | null = null;
  private slidingWindow: SlidingWindow | null = null;
  private fixedWindow: FixedWindow | null = null;

  /**
   * Create a new RateLimiter.
   *
   * @param config - Partial configuration; unspecified fields use defaults.
   */
  constructor(config: Partial<RateLimitConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };

    switch (this.config.strategy) {
      case RateLimitStrategy.TokenBucket:
        this.tokenBucket = new TokenBucket(
          this.config.maxRequests,
          this.config.refillRate
        );
        break;

      case RateLimitStrategy.SlidingWindow:
        this.slidingWindow = new SlidingWindow(
          this.config.maxRequests,
          this.config.windowMs
        );
        break;

      case RateLimitStrategy.FixedWindow:
        this.fixedWindow = new FixedWindow(
          this.config.maxRequests,
          this.config.windowMs
        );
        break;
    }
  }

  /**
   * Try to acquire a permit without blocking.
   *
   * @param tokens - Number of tokens to acquire (only meaningful for
   *                 TokenBucket strategy; default: 1).
   * @returns A {@link RateLimitResult} with `allowed`, `remaining`,
   *          `resetIn`, and `retryAfter` fields.
   */
  tryAcquire(tokens = 1): RateLimitResult {
    return withTracingSync(
      `rate_limiter.${this.config.name}.try_acquire`,
      (span) => {
        let result: RateLimitResult;

        if (this.tokenBucket) {
          result = this.tokenBucket.tryAcquire(tokens);
        } else if (this.slidingWindow) {
          result = this.slidingWindow.tryAcquire();
        } else if (this.fixedWindow) {
          result = this.fixedWindow.tryAcquire();
        } else {
          result = { allowed: true, remaining: 0, resetIn: 0, retryAfter: 0 };
        }

        span.setAttribute('rate_limiter.name', this.config.name);
        span.setAttribute('rate_limiter.allowed', result.allowed);
        span.setAttribute('rate_limiter.remaining', result.remaining);

        return result;
      }
    );
  }

  /**
   * Wait until a permit is available, then acquire it.
   *
   * This method polls using `retryAfter` from {@link tryAcquire} and
   * sleeps between attempts.
   *
   * @param tokens - Number of tokens to acquire (default: 1).
   * @returns A promise that resolves when the permit is acquired.
   * @throws RateLimitExhaustedError If the rate limit cannot be satisfied.
   */
  async acquire(tokens = 1): Promise<void> {
    const result = this.tryAcquire(tokens);

    if (result.allowed) {
      return;
    }

    if (result.retryAfter > 0) {
      await sleep(result.retryAfter);
      return this.acquire(tokens);
    }

    throw new RateLimitExhaustedError(result.retryAfter);
  }

  /**
   * Execute a function with rate limiting.
   *
   * Waits for a permit before invoking the function.
   *
   * @typeParam T - The return type of the function.
   * @param fn - Async function to execute.
   * @returns Result of the function.
   * @throws RateLimitExhaustedError If rate limit is exceeded.
   */
  async execute<T>(fn: () => Promise<T>): Promise<T> {
    await this.acquire();
    return fn();
  }
}

/**
 * Helper for synchronous tracing (returns value directly).
 *
 * @typeParam T - The return type.
 * @param spanName - Name of the tracing span.
 * @param fn       - Function receiving a span and returning a value.
 * @returns The function result.
 * @internal
 */
function withTracingSync<T>(
  spanName: string,
  fn: (span: TracingSpan) => T
): T {
  // Simple implementation without actual tracing
  const span: TracingSpan = {
    setAttribute: () => {},
    addEvent: () => {},
    end: () => {},
  };
  return fn(span);
}

/**
 * Minimal tracing span interface.
 * @internal
 */
interface TracingSpan {
  setAttribute: (key: string, value: unknown) => void;
  addEvent: (name: string, attributes?: Record<string, unknown>) => void;
  end: (error?: Error) => void;
}

/**
 * Sleep helper.
 *
 * @param ms - Duration in milliseconds.
 * @internal
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Manager for multiple rate limiters.
 *
 * @example
 * ```typescript
 * const manager = new RateLimiterManager({ maxRequests: 100 });
 * const api = manager.getLimiter('api');
 * const email = manager.getLimiter('email', { maxRequests: 10 });
 * ```
 */
export class RateLimiterManager {
  private limiters: Map<string, RateLimiter> = new Map();
  private defaultConfig: RateLimitConfig;

  /**
   * Create a new RateLimiterManager.
   *
   * @param defaultConfig - Default configuration for new limiters.
   */
  constructor(defaultConfig: Partial<RateLimitConfig> = {}) {
    this.defaultConfig = { ...DEFAULT_CONFIG, ...defaultConfig };
  }

  /**
   * Get or create a rate limiter by name.
   *
   * @param name   - Unique name for the rate limiter.
   * @param config - Optional per-limiter configuration overrides.
   * @returns The existing or newly created RateLimiter.
   */
  getLimiter(name: string, config?: Partial<RateLimitConfig>): RateLimiter {
    if (!this.limiters.has(name)) {
      this.limiters.set(
        name,
        new RateLimiter({
          ...this.defaultConfig,
          ...config,
          name,
        })
      );
    }
    return this.limiters.get(name)!;
  }

  /**
   * Get all registered rate limiter names.
   *
   * @returns An array of limiter names.
   */
  getNames(): string[] {
    return Array.from(this.limiters.keys());
  }

  /**
   * Remove all rate limiters.
   */
  clear(): void {
    this.limiters.clear();
  }
}

/**
 * Create a pre-configured API rate limiter (100 req/s, sliding window).
 *
 * @returns A RateLimiter tuned for typical API rate limits.
 */
export function apiRateLimiter(): RateLimiter {
  return new RateLimiter({
    maxRequests: 100,
    windowMs: 1000,
    strategy: RateLimitStrategy.SlidingWindow,
    name: 'api',
  });
}

/**
 * Create a pre-configured strict rate limiter (10 req/s, token bucket).
 *
 * @returns A RateLimiter suitable for strict per-second rate limiting.
 */
export function strictRateLimiter(): RateLimiter {
  return new RateLimiter({
    maxRequests: 10,
    windowMs: 1000,
    strategy: RateLimitStrategy.TokenBucket,
    refillRate: 10,
    name: 'strict',
  });
}

/**
 * Create a pre-configured bursty rate limiter (1000 burst, 50 req/s sustained).
 *
 * @returns A RateLimiter that allows initial bursts but sustains at 50 req/s.
 */
export function burstyRateLimiter(): RateLimiter {
  return new RateLimiter({
    maxRequests: 1000,
    windowMs: 1000,
    strategy: RateLimitStrategy.TokenBucket,
    refillRate: 50,
    name: 'bursty',
  });
}
