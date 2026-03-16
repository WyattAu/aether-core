/**
 * Retry Policy with Exponential Backoff Implementation
 * 
 * Provides configurable retry logic for transient failures.
 * 
 * @module @aether/sdk/resilience
 */

// ============================================
// Types
// ============================================

export type BackoffStrategy = 'fixed' | 'linear' | 'exponential' | 'exponential-jitter';

export interface RetryConfig {
    /** Maximum number of retry attempts */
    maxAttempts: number;
    /** Backoff strategy */
    backoff: BackoffStrategy;
    /** Base delay in ms */
    baseDelay: number;
    /** Maximum delay in ms */
    maxDelay: number;
    /** Multiplier for exponential backoff */
    multiplier?: number;
    /** Jitter factor (0-1) for jitter strategies */
    jitterFactor?: number;
    /** Predicate to determine if error is retryable */
    isRetryable?: (error: Error, attempt: number) => boolean;
    /** Callback before each retry */
    onRetry?: (error: Error, attempt: number, delay: number) => void;
    /** Callback when all retries exhausted */
    onExhausted?: (error: Error, attempts: number) => void;
}

export interface RetryStats {
    totalAttempts: number;
    successfulAttempts: number;
    failedAttempts: number;
    retriedCalls: number;
    exhaustedCalls: number;
    totalRetryDelayMs: number;
}

export interface RetryResult<T> {
    result: T;
    attempts: number;
    totalDelayMs: number;
}

// ============================================
// Retry Policy Implementation
// ============================================

export class RetryPolicy {
    private stats: RetryStats = {
        totalAttempts: 0,
        successfulAttempts: 0,
        failedAttempts: 0,
        retriedCalls: 0,
        exhaustedCalls: 0,
        totalRetryDelayMs: 0,
    };

    private readonly config: Required<RetryConfig>;

    constructor(config: Partial<RetryConfig> = {}) {
        this.config = {
            maxAttempts: config.maxAttempts ?? 3,
            backoff: config.backoff ?? 'exponential',
            baseDelay: config.baseDelay ?? 100,
            maxDelay: config.maxDelay ?? 30000,
            multiplier: config.multiplier ?? 2,
            jitterFactor: config.jitterFactor ?? 0.1,
            isRetryable: config.isRetryable ?? (() => true),
            onRetry: config.onRetry ?? (() => {}),
            onExhausted: config.onExhausted ?? (() => {}),
        };
    }

    /**
     * Execute a function with retry logic
     */
    async execute<T>(fn: () => Promise<T>): Promise<RetryResult<T>> {
        let attempt = 0;
        let totalDelayMs = 0;
        let lastError: Error | undefined;

        while (attempt < this.config.maxAttempts) {
            attempt++;
            this.stats.totalAttempts++;

            try {
                const result = await fn();
                this.stats.successfulAttempts++;
                
                if (attempt > 1) {
                    this.stats.retriedCalls++;
                }

                return {
                    result,
                    attempts: attempt,
                    totalDelayMs,
                };
            } catch (error) {
                lastError = error as Error;
                this.stats.failedAttempts++;

                // Check if we should retry
                if (attempt >= this.config.maxAttempts || !this.config.isRetryable(lastError, attempt)) {
                    break;
                }

                // Calculate delay
                const delay = this.calculateDelay(attempt);
                totalDelayMs += delay;
                this.stats.totalRetryDelayMs += delay;

                // Notify callback
                this.config.onRetry(lastError, attempt, delay);

                // Wait before retry
                await this.sleep(delay);
            }
        }

        // All retries exhausted
        this.stats.exhaustedCalls++;
        this.config.onExhausted(lastError!, attempt);

        throw new RetryExhaustedError(
            `All ${this.config.maxAttempts} retry attempts exhausted`,
            lastError!,
            attempt,
            totalDelayMs
        );
    }

    /**
     * Execute with a result wrapper (doesn't throw on exhaustion)
     */
    async executeSafe<T>(fn: () => Promise<T>): Promise<RetryResult<T> | null> {
        try {
            return await this.execute(fn);
        } catch (error) {
            if (error instanceof RetryExhaustedError) {
                return null;
            }
            throw error;
        }
    }

    /**
     * Get current statistics
     */
    getStats(): Readonly<RetryStats> {
        return { ...this.stats };
    }

    /**
     * Reset statistics
     */
    resetStats(): void {
        this.stats = {
            totalAttempts: 0,
            successfulAttempts: 0,
            failedAttempts: 0,
            retriedCalls: 0,
            exhaustedCalls: 0,
            totalRetryDelayMs: 0,
        };
    }

    // ============================================
    // Private Methods
    // ============================================

    private calculateDelay(attempt: number): number {
        let delay: number;

        switch (this.config.backoff) {
            case 'fixed':
                delay = this.config.baseDelay;
                break;

            case 'linear':
                delay = this.config.baseDelay * attempt;
                break;

            case 'exponential':
                delay = this.config.baseDelay * Math.pow(this.config.multiplier, attempt - 1);
                break;

            case 'exponential-jitter':
                delay = this.config.baseDelay * Math.pow(this.config.multiplier, attempt - 1);
                delay = this.addJitter(delay);
                break;

            default:
                delay = this.config.baseDelay;
        }

        return Math.min(delay, this.config.maxDelay);
    }

    private addJitter(delay: number): number {
        const jitter = delay * this.config.jitterFactor;
        return delay + (Math.random() * jitter * 2 - jitter);
    }

    private sleep(ms: number): Promise<void> {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// ============================================
// Retry Error
// ============================================

export class RetryExhaustedError extends Error {
    public readonly lastError: Error;
    public readonly attempts: number;
    public readonly totalDelayMs: number;

    constructor(message: string, lastError: Error, attempts: number, totalDelayMs: number) {
        super(message);
        this.name = 'RetryExhaustedError';
        this.lastError = lastError;
        this.attempts = attempts;
        this.totalDelayMs = totalDelayMs;
    }
}

// ============================================
// Predefined Retry Policies
// ============================================

/**
 * Create a retry policy for transient network errors
 */
export function networkRetryPolicy(overrides: Partial<RetryConfig> = {}): RetryPolicy {
    return new RetryPolicy({
        maxAttempts: 3,
        backoff: 'exponential-jitter',
        baseDelay: 100,
        maxDelay: 5000,
        isRetryable: (error) => {
            // Common transient error patterns
            const transientMessages = [
                'ECONNRESET',
                'ETIMEDOUT',
                'ENOTFOUND',
                'ECONNREFUSED',
                'network',
                'timeout',
                'unavailable',
                'temporary',
            ];
            const message = error.message.toLowerCase();
            return transientMessages.some(m => message.includes(m.toLowerCase()));
        },
        ...overrides,
    });
}

/**
 * Create a retry policy for database operations
 */
export function databaseRetryPolicy(overrides: Partial<RetryConfig> = {}): RetryPolicy {
    return new RetryPolicy({
        maxAttempts: 5,
        backoff: 'exponential',
        baseDelay: 50,
        maxDelay: 2000,
        multiplier: 2,
        isRetryable: (error) => {
            const retryableMessages = [
                'deadlock',
                'lock',
                'busy',
                'timeout',
                'connection',
            ];
            const message = error.message.toLowerCase();
            return retryableMessages.some(m => message.includes(m.toLowerCase()));
        },
        ...overrides,
    });
}

/**
 * Create an aggressive retry policy (many attempts, short delays)
 */
export function aggressiveRetryPolicy(overrides: Partial<RetryConfig> = {}): RetryPolicy {
    return new RetryPolicy({
        maxAttempts: 10,
        backoff: 'exponential-jitter',
        baseDelay: 10,
        maxDelay: 1000,
        multiplier: 1.5,
        jitterFactor: 0.2,
        ...overrides,
    });
}

/**
 * Create a conservative retry policy (few attempts, longer delays)
 */
export function conservativeRetryPolicy(overrides: Partial<RetryConfig> = {}): RetryPolicy {
    return new RetryPolicy({
        maxAttempts: 2,
        backoff: 'exponential',
        baseDelay: 1000,
        maxDelay: 10000,
        multiplier: 3,
        ...overrides,
    });
}

// ============================================
// Decorator for Method Retry
// ============================================

/**
 * Decorator to add retry logic to a method
 */
export function withRetry(policy: RetryPolicy) {
    return function (
        _target: any,
        _propertyKey: string,
        descriptor: TypedPropertyDescriptor<(...args: any[]) => Promise<any>>
    ) {
        const originalMethod = descriptor.value!;

        descriptor.value = async function (...args: any[]) {
            const result = await policy.execute(() => originalMethod.apply(this, args));
            return result.result;
        };

        return descriptor;
    };
}

// ============================================
// Combined Policy Builder
// ============================================

/**
 * Builder for creating combined retry + circuit breaker policies
 */
export class ResiliencePolicyBuilder {
    private retryConfig: Partial<RetryConfig> = {};
    private breakerConfig: Partial<import('./circuit-breaker').CircuitBreakerConfig> = {};

    withRetry(config: Partial<RetryConfig>): this {
        this.retryConfig = { ...this.retryConfig, ...config };
        return this;
    }

    withCircuitBreaker(config: Partial<import('./circuit-breaker').CircuitBreakerConfig>): this {
        this.breakerConfig = { ...this.breakerConfig, ...config };
        return this;
    }

    build(): { retry: RetryPolicy; breaker: import('./circuit-breaker').CircuitBreaker } {
        const { CircuitBreaker } = require('./circuit-breaker');
        
        return {
            retry: new RetryPolicy(this.retryConfig),
            breaker: new CircuitBreaker(this.breakerConfig),
        };
    }

    /**
     * Build a combined executor that uses both policies
     */
    buildExecutor(): <T>(fn: () => Promise<T>) => Promise<T> {
        const { CircuitBreaker } = require('./circuit-breaker');
        
        const retry = new RetryPolicy(this.retryConfig);
        const breaker = new CircuitBreaker(this.breakerConfig);

        return async <T>(fn: () => Promise<T>): Promise<T> => {
            return breaker.execute(async () => {
                const result = await retry.execute(fn);
                return result.result;
            });
        };
    }
}
