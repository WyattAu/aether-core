/**
 * Rate Limiter Implementation
 * 
 * Provides rate limiting with multiple strategies:
 * - Token Bucket: Allows bursts up to bucket size
 * - Sliding Window: Smooth rate limiting over time
 * - Fixed Window: Simple window-based limiting
 * 
 * @module @aether/sdk/resilience
 */

// ============================================
// Types
// ============================================

export type RateLimitStrategy = 'token-bucket' | 'sliding-window' | 'fixed-window';

export interface RateLimitConfig {
    /** Maximum requests per second */
    requestsPerSecond: number;
    /** Maximum burst size (for token bucket) */
    burstSize?: number;
    /** Strategy to use */
    strategy?: RateLimitStrategy;
    /** Window size in ms (for window strategies) */
    windowSize?: number;
}

export interface RateLimitStats {
    allowedRequests: number;
    rejectedRequests: number;
    currentRate: number;
    waitTimeMs: number;
}

export interface RateLimitResult {
    allowed: boolean;
    waitTimeMs: number;
    remainingTokens?: number;
    resetIn?: number;
}

// ============================================
// Token Bucket Rate Limiter
// ============================================

class TokenBucket {
    private tokens: number;
    private lastRefill: number;
    private readonly maxTokens: number;
    private readonly refillRate: number; // tokens per ms

    constructor(requestsPerSecond: number, burstSize: number) {
        this.maxTokens = burstSize;
        this.tokens = burstSize;
        this.refillRate = requestsPerSecond / 1000; // per ms
        this.lastRefill = Date.now();
    }

    tryAcquire(tokens: number = 1): RateLimitResult {
        this.refill();

        if (this.tokens >= tokens) {
            this.tokens -= tokens;
            return {
                allowed: true,
                waitTimeMs: 0,
                remainingTokens: this.tokens,
            };
        }

        // Calculate wait time for required tokens
        const tokensNeeded = tokens - this.tokens;
        const waitTimeMs = Math.ceil(tokensNeeded / this.refillRate);

        return {
            allowed: false,
            waitTimeMs,
            remainingTokens: this.tokens,
        };
    }

    private refill(): void {
        const now = Date.now();
        const elapsed = now - this.lastRefill;
        const tokensToAdd = elapsed * this.refillRate;
        
        this.tokens = Math.min(this.maxTokens, this.tokens + tokensToAdd);
        this.lastRefill = now;
    }

    getTokens(): number {
        this.refill();
        return this.tokens;
    }
}

// ============================================
// Sliding Window Rate Limiter
// ============================================

class SlidingWindow {
    private requests: number[] = [];
    private readonly maxRequests: number;
    private readonly windowSizeMs: number;

    constructor(requestsPerSecond: number, windowSizeMs: number = 1000) {
        this.maxRequests = requestsPerSecond;
        this.windowSizeMs = windowSizeMs;
    }

    tryAcquire(): RateLimitResult {
        const now = Date.now();
        const windowStart = now - this.windowSizeMs;

        // Remove old requests
        this.requests = this.requests.filter(t => t > windowStart);

        if (this.requests.length < this.maxRequests) {
            this.requests.push(now);
            return {
                allowed: true,
                waitTimeMs: 0,
                remainingTokens: this.maxRequests - this.requests.length,
            };
        }

        // Calculate wait time until oldest request exits window
        const oldestRequest = this.requests[0];
        const waitTimeMs = oldestRequest + this.windowSizeMs - now;

        return {
            allowed: false,
            waitTimeMs: Math.max(1, waitTimeMs),
            resetIn: waitTimeMs,
        };
    }

    getCurrentCount(): number {
        const now = Date.now();
        const windowStart = now - this.windowSizeMs;
        return this.requests.filter(t => t > windowStart).length;
    }
}

// ============================================
// Fixed Window Rate Limiter
// ============================================

class FixedWindow {
    private count: number = 0;
    private windowStart: number;
    private readonly maxRequests: number;
    private readonly windowSizeMs: number;

    constructor(requestsPerSecond: number, windowSizeMs: number = 1000) {
        this.maxRequests = requestsPerSecond;
        this.windowSizeMs = windowSizeMs;
        this.windowStart = Date.now();
    }

    tryAcquire(): RateLimitResult {
        const now = Date.now();

        // Check if we need to reset the window
        if (now - this.windowStart >= this.windowSizeMs) {
            this.count = 0;
            this.windowStart = now;
        }

        if (this.count < this.maxRequests) {
            this.count++;
            return {
                allowed: true,
                waitTimeMs: 0,
                remainingTokens: this.maxRequests - this.count,
                resetIn: this.windowStart + this.windowSizeMs - now,
            };
        }

        return {
            allowed: false,
            waitTimeMs: this.windowStart + this.windowSizeMs - now,
            resetIn: this.windowStart + this.windowSizeMs - now,
        };
    }

    getCurrentCount(): number {
        return this.count;
    }
}

// ============================================
// Rate Limiter
// ============================================

export class RateLimiter {
    private impl: TokenBucket | SlidingWindow | FixedWindow;
    private allowedRequests: number = 0;
    private rejectedRequests: number = 0;
    private readonly config: Required<RateLimitConfig>;

    constructor(config: Partial<RateLimitConfig> = {}) {
        this.config = {
            requestsPerSecond: config.requestsPerSecond ?? 100,
            burstSize: config.burstSize ?? config.requestsPerSecond ?? 100,
            strategy: config.strategy ?? 'token-bucket',
            windowSize: config.windowSize ?? 1000,
        };

        switch (this.config.strategy) {
            case 'token-bucket':
                this.impl = new TokenBucket(
                    this.config.requestsPerSecond,
                    this.config.burstSize
                );
                break;
            case 'sliding-window':
                this.impl = new SlidingWindow(
                    this.config.requestsPerSecond,
                    this.config.windowSize
                );
                break;
            case 'fixed-window':
                this.impl = new FixedWindow(
                    this.config.requestsPerSecond,
                    this.config.windowSize
                );
                break;
            default:
                throw new Error(`Unknown rate limit strategy: ${this.config.strategy}`);
        }
    }

    /**
     * Try to acquire permission to proceed.
     * Returns immediately with result (non-blocking).
     */
    tryAcquire(tokens: number = 1): RateLimitResult {
        // Token bucket supports multiple tokens, others don't
        if (this.impl instanceof TokenBucket) {
            const result = this.impl.tryAcquire(tokens);
            if (result.allowed) {
                this.allowedRequests++;
            } else {
                this.rejectedRequests++;
            }
            return result;
        } else {
            const result = (this.impl as SlidingWindow | FixedWindow).tryAcquire();
            if (result.allowed) {
                this.allowedRequests++;
            } else {
                this.rejectedRequests++;
            }
            return result;
        }
    }

    /**
     * Acquire permission, waiting if necessary.
     * Throws RateLimitExhaustedError if wait time exceeds max.
     */
    async acquire(maxWaitMs: number = 5000): Promise<void> {
        const result = this.tryAcquire();

        if (result.allowed) {
            return;
        }

        if (result.waitTimeMs > maxWaitMs) {
            this.rejectedRequests++;
            throw new RateLimitExhaustedError(
                `Rate limit exceeded. Wait time ${result.waitTimeMs}ms exceeds max ${maxWaitMs}ms`
            );
        }

        await this.sleep(result.waitTimeMs);
        
        // Try again after waiting
        const retryResult = this.tryAcquire();
        if (!retryResult.allowed) {
            throw new RateLimitExhaustedError('Rate limit still exceeded after waiting');
        }
    }

    /**
     * Execute a function with rate limiting
     */
    async execute<T>(fn: () => Promise<T>, maxWaitMs?: number): Promise<T> {
        await this.acquire(maxWaitMs);
        return fn();
    }

    /**
     * Get current statistics
     */
    getStats(): RateLimitStats {
        let currentRate = 0;

        if (this.impl instanceof TokenBucket) {
            currentRate = this.config.requestsPerSecond * 
                (1 - this.impl.getTokens() / this.config.burstSize);
        } else if (this.impl instanceof SlidingWindow) {
            currentRate = this.impl.getCurrentCount();
        } else if (this.impl instanceof FixedWindow) {
            currentRate = this.impl.getCurrentCount();
        }

        const result = this.tryAcquire(0); // Peek without consuming
        this.allowedRequests--; // Undo the increment from peek

        return {
            allowedRequests: this.allowedRequests,
            rejectedRequests: this.rejectedRequests,
            currentRate,
            waitTimeMs: result.waitTimeMs,
        };
    }

    /**
     * Reset statistics
     */
    resetStats(): void {
        this.allowedRequests = 0;
        this.rejectedRequests = 0;
    }

    private sleep(ms: number): Promise<void> {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// ============================================
// Rate Limit Error
// ============================================

export class RateLimitExhaustedError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'RateLimitExhaustedError';
    }
}

// ============================================
// Rate Limiter Manager
// ============================================

/**
 * Manages multiple rate limiters by name
 */
export class RateLimiterManager {
    private limiters: Map<string, RateLimiter> = new Map();
    private defaultConfig: Partial<RateLimitConfig>;

    constructor(defaultConfig: Partial<RateLimitConfig> = {}) {
        this.defaultConfig = defaultConfig;
    }

    /**
     * Get or create a rate limiter by name
     */
    get(name: string, config?: Partial<RateLimitConfig>): RateLimiter {
        if (!this.limiters.has(name)) {
            this.limiters.set(name, new RateLimiter({
                ...this.defaultConfig,
                ...config,
            }));
        }
        return this.limiters.get(name)!;
    }

    /**
     * Get all rate limiter stats
     */
    getAllStats(): Record<string, RateLimitStats> {
        const stats: Record<string, RateLimitStats> = {};
        for (const [name, limiter] of this.limiters) {
            stats[name] = limiter.getStats();
        }
        return stats;
    }

    /**
     * Reset all statistics
     */
    resetAllStats(): void {
        for (const limiter of this.limiters.values()) {
            limiter.resetStats();
        }
    }
}

// ============================================
// Predefined Rate Limiters
// ============================================

/**
 * Create a rate limiter for API requests (100 req/s with bursts)
 */
export function apiRateLimiter(): RateLimiter {
    return new RateLimiter({
        requestsPerSecond: 100,
        burstSize: 200,
        strategy: 'token-bucket',
    });
}

/**
 * Create a rate limiter for strict limiting (no bursts)
 */
export function strictRateLimiter(requestsPerSecond: number): RateLimiter {
    return new RateLimiter({
        requestsPerSecond,
        strategy: 'sliding-window',
    });
}

/**
 * Create a rate limiter for bursty traffic
 */
export function burstyRateLimiter(burstSize: number, refillRate: number): RateLimiter {
    return new RateLimiter({
        requestsPerSecond: refillRate,
        burstSize,
        strategy: 'token-bucket',
    });
}
