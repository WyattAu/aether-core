/**
 * Circuit Breaker Pattern Implementation
 * 
 * Prevents cascading failures by stopping requests to a failing service.
 * 
 * States:
 * - CLOSED: Normal operation, requests pass through
 * - OPEN: Failing, requests are rejected immediately
 * - HALF_OPEN: Testing if service recovered
 * 
 * @module @aether/sdk/resilience
 */

// ============================================
// Types
// ============================================

export type CircuitState = 'closed' | 'open' | 'half-open';

export interface CircuitBreakerConfig {
    /** Number of failures before opening the circuit */
    failureThreshold: number;
    /** Number of successes in half-open to close the circuit */
    successThreshold: number;
    /** Time in ms before attempting to close from open state */
    timeout: number;
    /** Maximum calls allowed in half-open state */
    halfOpenMaxCalls: number;
    /** Time window in ms for counting failures (0 = no window) */
    failureWindow?: number;
    /** Callback when circuit opens */
    onOpen?: () => void;
    /** Callback when circuit closes */
    onClose?: () => void;
    /** Callback when circuit enters half-open */
    onHalfOpen?: () => void;
}

export interface CircuitBreakerStats {
    state: CircuitState;
    failures: number;
    successes: number;
    rejectedCalls: number;
    totalCalls: number;
    lastFailure?: Date;
    lastSuccess?: Date;
    lastStateChange?: Date;
}

interface FailureRecord {
    timestamp: number;
    error: Error;
}

// ============================================
// Circuit Breaker Implementation
// ============================================

export class CircuitBreaker {
    private state: CircuitState = 'closed';
    private failures: number = 0;
    private successes: number = 0;
    private rejectedCalls: number = 0;
    private totalCalls: number = 0;
    private halfOpenCalls: number = 0;
    private lastFailure?: Date;
    private lastSuccess?: Date;
    private lastStateChange?: Date;
    private failureHistory: FailureRecord[] = [];
    private readonly config: Required<CircuitBreakerConfig>;

    constructor(config: Partial<CircuitBreakerConfig> = {}) {
        this.config = {
            failureThreshold: config.failureThreshold ?? 5,
            successThreshold: config.successThreshold ?? 3,
            timeout: config.timeout ?? 30000,
            halfOpenMaxCalls: config.halfOpenMaxCalls ?? 3,
            failureWindow: config.failureWindow ?? 60000,
            onOpen: config.onOpen ?? (() => {}),
            onClose: config.onClose ?? (() => {}),
            onHalfOpen: config.onHalfOpen ?? (() => {}),
        };
    }

    /**
     * Execute a function through the circuit breaker.
     * If the circuit is open, throws an error immediately.
     */
    async execute<T>(fn: () => Promise<T>): Promise<T> {
        this.totalCalls++;

        // Check if we should transition from open to half-open
        if (this.state === 'open') {
            if (this.shouldAttemptReset()) {
                this.transitionTo('half-open');
            } else {
                this.rejectedCalls++;
                throw new CircuitBreakerError('Circuit breaker is open');
            }
        }

        // Check half-open call limit
        if (this.state === 'half-open' && this.halfOpenCalls >= this.config.halfOpenMaxCalls) {
            this.rejectedCalls++;
            throw new CircuitBreakerError('Circuit breaker is half-open and at max calls');
        }

        // Execute the function
        try {
            if (this.state === 'half-open') {
                this.halfOpenCalls++;
            }

            const result = await fn();
            this.onSuccess();
            return result;
        } catch (error) {
            this.onFailure(error as Error);
            throw error;
        }
    }

    /**
     * Check if circuit is currently closed (allowing requests)
     */
    isClosed(): boolean {
        return this.state === 'closed';
    }

    /**
     * Check if circuit is currently open (rejecting requests)
     */
    isOpen(): boolean {
        return this.state === 'open';
    }

    /**
     * Check if circuit is in half-open state (testing recovery)
     */
    isHalfOpen(): boolean {
        return this.state === 'half-open';
    }

    /**
     * Get current circuit state
     */
    getState(): CircuitState {
        return this.state;
    }

    /**
     * Get circuit breaker statistics
     */
    getStats(): CircuitBreakerStats {
        return {
            state: this.state,
            failures: this.failures,
            successes: this.successes,
            rejectedCalls: this.rejectedCalls,
            totalCalls: this.totalCalls,
            lastFailure: this.lastFailure,
            lastSuccess: this.lastSuccess,
            lastStateChange: this.lastStateChange,
        };
    }

    /**
     * Force the circuit to open state
     */
    forceOpen(): void {
        this.transitionTo('open');
    }

    /**
     * Force the circuit to closed state
     */
    forceClose(): void {
        this.transitionTo('closed');
    }

    /**
     * Reset all statistics
     */
    reset(): void {
        this.failures = 0;
        this.successes = 0;
        this.rejectedCalls = 0;
        this.totalCalls = 0;
        this.halfOpenCalls = 0;
        this.failureHistory = [];
        this.transitionTo('closed');
    }

    // ============================================
    // Private Methods
    // ============================================

    private shouldAttemptReset(): boolean {
        if (!this.lastFailure) {
            return true;
        }
        return Date.now() - this.lastFailure.getTime() >= this.config.timeout;
    }

    private onSuccess(): void {
        this.lastSuccess = new Date();
        this.failureHistory = [];

        if (this.state === 'half-open') {
            this.successes++;
            if (this.successes >= this.config.successThreshold) {
                this.transitionTo('closed');
            }
        } else if (this.state === 'closed') {
            this.failures = 0;
        }
    }

    private onFailure(error: Error): void {
        this.lastFailure = new Date();
        this.failures++;

        // Record failure for window-based counting
        this.failureHistory.push({
            timestamp: Date.now(),
            error,
        });

        // Clean old failures outside window
        if (this.config.failureWindow > 0) {
            const cutoff = Date.now() - this.config.failureWindow;
            this.failureHistory = this.failureHistory.filter(f => f.timestamp >= cutoff);
        }

        if (this.state === 'half-open') {
            // Any failure in half-open immediately opens
            this.transitionTo('open');
        } else if (this.state === 'closed') {
            // Check if we should open based on failure count
            const failureCount = this.config.failureWindow > 0
                ? this.failureHistory.length
                : this.failures;

            if (failureCount >= this.config.failureThreshold) {
                this.transitionTo('open');
            }
        }
    }

    private transitionTo(newState: CircuitState): void {
        if (this.state === newState) {
            return;
        }

        const oldState = this.state;
        this.state = newState;
        this.lastStateChange = new Date();

        // Reset counters on state change
        if (newState === 'closed') {
            this.failures = 0;
            this.successes = 0;
            this.halfOpenCalls = 0;
            this.failureHistory = [];
            this.config.onClose();
        } else if (newState === 'open') {
            this.successes = 0;
            this.halfOpenCalls = 0;
            this.config.onOpen();
        } else if (newState === 'half-open') {
            this.successes = 0;
            this.halfOpenCalls = 0;
            this.config.onHalfOpen();
        }
    }
}

// ============================================
// Circuit Breaker Error
// ============================================

export class CircuitBreakerError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'CircuitBreakerError';
    }
}

// ============================================
// Decorator for Method Protection
// ============================================

/**
 * Decorator to protect a method with a circuit breaker
 */
export function withCircuitBreaker(breaker: CircuitBreaker) {
    return function (
        _target: any,
        _propertyKey: string,
        descriptor: TypedPropertyDescriptor<(...args: any[]) => Promise<any>>
    ) {
        const originalMethod = descriptor.value!;

        descriptor.value = async function (...args: any[]) {
            return breaker.execute(() => originalMethod.apply(this, args));
        };

        return descriptor;
    };
}

// ============================================
// Circuit Breaker Manager
// ============================================

/**
 * Manages multiple circuit breakers by name
 */
export class CircuitBreakerManager {
    private breakers: Map<string, CircuitBreaker> = new Map();
    private defaultConfig: Partial<CircuitBreakerConfig>;

    constructor(defaultConfig: Partial<CircuitBreakerConfig> = {}) {
        this.defaultConfig = defaultConfig;
    }

    /**
     * Get or create a circuit breaker by name
     */
    get(name: string, config?: Partial<CircuitBreakerConfig>): CircuitBreaker {
        if (!this.breakers.has(name)) {
            this.breakers.set(name, new CircuitBreaker({
                ...this.defaultConfig,
                ...config,
            }));
        }
        return this.breakers.get(name)!;
    }

    /**
     * Get all circuit breaker stats
     */
    getAllStats(): Record<string, CircuitBreakerStats> {
        const stats: Record<string, CircuitBreakerStats> = {};
        for (const [name, breaker] of this.breakers) {
            stats[name] = breaker.getStats();
        }
        return stats;
    }

    /**
     * Reset all circuit breakers
     */
    resetAll(): void {
        for (const breaker of this.breakers.values()) {
            breaker.reset();
        }
    }

    /**
     * Get breaker names that are currently open
     */
    getOpenBreakers(): string[] {
        const open: string[] = [];
        for (const [name, breaker] of this.breakers) {
            if (breaker.isOpen()) {
                open.push(name);
            }
        }
        return open;
    }
}
