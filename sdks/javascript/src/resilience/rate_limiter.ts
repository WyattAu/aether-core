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
 * Error thrown when rate limit is exceeded.
 */
export class RateLimitExhaustedError extends Error {
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
 * Token Bucket Rate Limiter.
 * Uses tokens that refill at a constant rate.
 */
class TokenBucket {
  private tokens: number;
  private lastRefill: number;

  constructor(
    private readonly maxTokens: number,
    private readonly refillRate: number
  ) {
    this.tokens = maxTokens;
    this.lastRefill = Date.now();
  }

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

  private refill(): void {
    const now = Date.now();
    const elapsed = (now - this.lastRefill) / 1000;
    const tokensToAdd = elapsed * this.refillRate;

    this.tokens = Math.min(this.maxTokens, this.tokens + tokensToAdd);
    this.lastRefill = now;
  }
}

/**
 * Sliding Window Rate Limiter.
 * Tracks requests in a sliding time window.
 */
class SlidingWindow {
  private requests: number[] = [];

  constructor(
    private readonly maxRequests: number,
    private readonly windowMs: number
  ) {}

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
 * Fixed Window Rate Limiter.
 * Tracks requests in fixed time windows.
 */
class FixedWindow {
  private count = 0;
  private windowStart: number;

  constructor(
    private readonly maxRequests: number,
    private readonly windowMs: number
  ) {
    this.windowStart = Date.now();
  }

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
 *   // Reject or wait
 * }
 * ```
 */
export class RateLimiter {
  private readonly config: RateLimitConfig;
  private tokenBucket: TokenBucket | null = null;
  private slidingWindow: SlidingWindow | null = null;
  private fixedWindow: FixedWindow | null = null;

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
   * Try to acquire a permit.
   *
   * @param tokens - Number of tokens to acquire (for token bucket)
   * @returns Rate limit result
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
   * @param tokens - Number of tokens to acquire
   * @returns Promise that resolves when permit is acquired
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
   * @param fn - Function to execute
   * @returns Result of the function
   * @throws RateLimitExhaustedError if rate limit exceeded
   */
  async execute<T>(fn: () => Promise<T>): Promise<T> {
    await this.acquire();
    return fn();
  }
}

/**
 * Helper for synchronous tracing (returns value directly).
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

interface TracingSpan {
  setAttribute: (key: string, value: unknown) => void;
  addEvent: (name: string, attributes?: Record<string, unknown>) => void;
  end: (error?: Error) => void;
}

/**
 * Sleep helper.
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Manager for multiple rate limiters.
 */
export class RateLimiterManager {
  private limiters: Map<string, RateLimiter> = new Map();
  private defaultConfig: RateLimitConfig;

  constructor(defaultConfig: Partial<RateLimitConfig> = {}) {
    this.defaultConfig = { ...DEFAULT_CONFIG, ...defaultConfig };
  }

  /**
   * Get or create a rate limiter by name.
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
   * Get all rate limiter names.
   */
  getNames(): string[] {
    return Array.from(this.limiters.keys());
  }

  /**
   * Clear all rate limiters.
   */
  clear(): void {
    this.limiters.clear();
  }
}

/**
 * Pre-configured API rate limiter (100 req/s).
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
 * Pre-configured strict rate limiter (10 req/s).
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
 * Pre-configured bursty rate limiter (1000 burst, 50 req/s sustained).
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
