/**
 * Resilience Module Index
 * 
 * Exports all resilience patterns for building robust actor systems.
 */

export { CircuitBreaker } from './circuit-breaker';
export { CircuitBreakerError, CircuitBreakerError } from './circuit-breaker';
export { CircuitBreakerManager } from './circuit-breaker';
export { withCircuitBreaker } from './circuit-breaker';
export type { CircuitState, from './circuit-breaker';
export type { CircuitBreakerConfig, from './circuit-breaker';
export type { CircuitBreakerStats } from './circuit-breaker';

export { RetryPolicy } from './retry';
export { RetryExhaustedError } from './retry';
export { networkRetryPolicy, from './retry';
export { databaseRetryPolicy } from './retry';
export { aggressiveRetryPolicy } from './retry';
export { conservativeRetryPolicy } from './retry';
export { withRetry } from './retry';
export { ResiliencePolicyBuilder } from './retry';
export type { BackoffStrategy } from './retry';
export type { RetryConfig } from './retry';
export type { RetryStats } from './retry';
export type { RetryResult } from './retry';

export { RateLimiter } from './rate-limiter';
export { RateLimitExhaustedError } from './rate-limiter';
export { RateLimiterManager } from './rate-limiter';
export { apiRateLimiter } from './rate-limiter';
export { strictRateLimiter } from './rate-limiter';
export { burstyRateLimiter } from './rate-limiter';
export type { RateLimitStrategy } from './rate-limiter';
export type { RateLimitConfig } from './rate-limiter';
export type { RateLimitStats } from './rate-limiter';
export type { RateLimitResult } from './rate-limiter';

export { HealthChecker } from './health-check';
export type { HealthStatus } from './health-check';
export type { HealthCheckResult } from './health-check';
export type { HealthReport } from './health-check';
export type { HealthCheckFn } from './health-check';
export type { HealthCheckOptions } from './health-check';

/**
 * Bulkhead pattern for resource isolation
 */
export class Bulkhead {
    private readonly maxConcurrent: number;
    private readonly maxQueued: number;
    private active: number = 0;
    private queued: number = 0;
    private readonly queue: Array<{
        resolve: (result: any) => void;
        reject: (error: Error) => void;
    }> = [];

    constructor(config: { maxConcurrent?: number; maxQueued?: number } = {}) {
        this.maxConcurrent = config.maxConcurrent ?? 10;
        this.maxQueued = config.maxQueued ?? 100;
    }

    /**
     * Execute a function with bulkhead protection
     */
    async execute<T>(fn: () => Promise<T>): Promise<T> {
        // If at capacity, execute immediately
        if (this.active < this.maxConcurrent) {
            return this.executeNow(fn);
        }

        // If queue has space, queue the request
        if (this.queued < this.maxQueued) {
            return this.queueRequest(fn);
        }

        // Otherwise reject
        throw new BulkheadRejectedError(
            `Bulkhead at capacity: ${this.active}/${this.maxConcurrent} active, ${this.queued}/${this.maxQueued} queued`
        );
    }

    private async executeNow<T>(fn: () => Promise<T>): Promise<T> {
        this.active++;
        try {
            const result = await fn();
            return result;
        } finally {
            this.active--;
            this.processQueue();
        }
    }

    private async queueRequest<T>(fn: () => Promise<T>): Promise<T> {
        return new Promise((resolve, reject) => {
            this.queued++;
            this.queue.push({ fn, resolve, reject });
        });
    }

    private processQueue(): void {
        if (this.queue.length > 0 && this.active < this.maxConcurrent) {
            const item = this.queue.shift()!;
            this.queued--;
            this.executeNow(item.fn)
                .then(item.resolve)
                .catch(item.reject);
        }
    }

    /**
     * Get current bulkhead stats
     */
    getStats(): { active: number; queued: number; maxConcurrent: number; maxQueued: number } {
        return {
            active: this.active,
            queued: this.queued,
            maxConcurrent: this.maxConcurrent,
            maxQueued: this.maxQueued,
        };
    }
}

export class BulkheadRejectedError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'BulkheadRejectedError';
    }
}

/**
 * Combined resilience executor
 */
export class ResilientExecutor {
    private readonly breaker?: CircuitBreaker;
    private readonly retry?: RetryPolicy;
    private readonly rateLimiter?: RateLimiter;
    private readonly bulkhead?: Bulkhead;

    constructor(config: {
        breaker?: CircuitBreaker;
        retry?: RetryPolicy;
        rateLimiter?: RateLimiter;
        bulkhead?: Bulkhead;
    }) {
        this.breaker = config.breaker;
        this.retry = config.retry;
        this.rateLimiter = config.rateLimiter;
        this.bulkhead = config.bulkhead;
    }

    /**
     * Execute with all configured resilience patterns
     */
    async execute<T>(fn: () => Promise<T>): Promise<T> {
        // Apply rate limiting
        if (this.rateLimiter) {
            await this.rateLimiter.acquire();
        }

        // Apply bulkhead
        if (this.bulkhead) {
            return this.bulkhead.execute(() => this.executeWithRetry(fn));
        }

        return this.executeWithRetry(fn);
    }

    private async executeWithRetry<T>(fn: () => Promise<T>): Promise<T> {
        // Apply circuit breaker
        if (this.breaker) {
            const result = await this.breaker.execute(async () => {
                // Apply retry
                if (this.retry) {
                    const retryResult = await this.retry.execute(fn);
                    return retryResult.result;
                }
                return fn();
            });
            return result;
        }

        // Apply retry without circuit breaker
        if (this.retry) {
            const result = await this.retry.execute(fn);
            return result.result;
        }

        return fn();
    }
}
