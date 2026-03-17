/**
 * Circuit Breaker Pattern Implementation.
 * Prevents cascading failures by temporarily blocking requests to a failing service.
 * @module aether/resilience/circuit_breaker
 */

import {
  CircuitState,
  CircuitBreakerConfig,
  CircuitBreakerStats,
  AsyncFunction,
} from './types';
import { withTracing } from './tracing';

/**
 * Error thrown when circuit breaker is open.
 */
export class CircuitBreakerError extends Error {
  constructor(
    public readonly name: string,
    public readonly state: CircuitState,
    message?: string
  ) {
    super(message || `Circuit breaker '${name}' is ${state}`);
    this.name = 'CircuitBreakerError';
  }
}

/**
 * Default circuit breaker configuration.
 */
const DEFAULT_CONFIG: CircuitBreakerConfig = {
  failureThreshold: 5,
  successThreshold: 3,
  resetTimeout: 30000,
  failureWindow: 60000,
  name: 'default',
};

/**
 * Circuit Breaker implementation.
 *
 * States:
 * - Closed: Normal operation, requests pass through
 * - Open: Requests are blocked, waiting for reset timeout
 * - Half-Open: Limited requests allowed to test recovery
 *
 * @example
 * ```typescript
 * const breaker = new CircuitBreaker({
 *   failureThreshold: 5,
 *   resetTimeout: 30000,
 * });
 *
 * try {
 *   const result = await breaker.execute(() => fetchData());
 * } catch (e) {
 *   if (e instanceof CircuitBreakerError) {
 *     // Handle blocked request
 *   }
 * }
 * ```
 */
export class CircuitBreaker {
  private state: CircuitState = CircuitState.Closed;
  private failureCount = 0;
  private successCount = 0;
  private totalCalls = 0;
  private lastFailureTime: number | null = null;
  private lastStateChange: number | null = null;
  private failures: number[] = [];
  private readonly config: CircuitBreakerConfig;

  constructor(config: Partial<CircuitBreakerConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Get current circuit breaker state.
   */
  getState(): CircuitState {
    this.checkStateTransition();
    return this.state;
  }

  /**
   * Get circuit breaker statistics.
   */
  getStats(): CircuitBreakerStats {
    return {
      state: this.getState(),
      failureCount: this.failureCount,
      successCount: this.successCount,
      totalCalls: this.totalCalls,
      lastFailureTime: this.lastFailureTime,
      lastStateChange: this.lastStateChange,
    };
  }

  /**
   * Execute a function with circuit breaker protection.
   *
   * @param fn - Async function to execute
   * @returns Result of the function
   * @throws CircuitBreakerError if circuit is open
   * @throws Original error if function fails
   */
  async execute<T>(fn: AsyncFunction<T>): Promise<T> {
    this.checkStateTransition();
    this.totalCalls++;

    if (this.state === CircuitState.Open) {
      throw new CircuitBreakerError(this.config.name, this.state);
    }

    return withTracing(
      `circuit_breaker.${this.config.name}.execute`,
      async (span) => {
        span.setAttribute('circuit_breaker.name', this.config.name);
        span.setAttribute('circuit_breaker.state', this.state);

        try {
          const result = await fn();
          this.recordSuccess();
          span.setAttribute('circuit_breaker.result', 'success');
          return result;
        } catch (error) {
          this.recordFailure();
          span.setAttribute('circuit_breaker.result', 'error');
          throw error;
        }
      }
    );
  }

  /**
   * Execute a function, returning fallback on circuit open.
   *
   * @param fn - Async function to execute
   * @param fallback - Fallback function to call if circuit is open
   * @returns Result of fn or fallback
   */
  async executeWithFallback<T>(
    fn: AsyncFunction<T>,
    fallback: AsyncFunction<T>
  ): Promise<T> {
    try {
      return await this.execute(fn);
    } catch (error) {
      if (error instanceof CircuitBreakerError) {
        return fallback();
      }
      throw error;
    }
  }

  /**
   * Record a successful operation.
   */
  recordSuccess(): void {
    this.successCount++;

    if (this.state === CircuitState.HalfOpen) {
      if (this.successCount >= this.config.successThreshold) {
        this.transitionTo(CircuitState.Closed);
      }
    }
  }

  /**
   * Record a failed operation.
   */
  recordFailure(): void {
    this.failureCount++;
    this.lastFailureTime = Date.now();

    // Track failures within the window
    const now = Date.now();
    this.failures.push(now);
    this.failures = this.failures.filter(
      (t) => now - t < this.config.failureWindow
    );

    if (this.state === CircuitState.HalfOpen) {
      // Any failure in half-open immediately opens
      this.transitionTo(CircuitState.Open);
    } else if (this.state === CircuitState.Closed) {
      if (this.failures.length >= this.config.failureThreshold) {
        this.transitionTo(CircuitState.Open);
      }
    }
  }

  /**
   * Force the circuit breaker to a specific state.
   */
  forceState(state: CircuitState): void {
    this.transitionTo(state);
  }

  /**
   * Reset the circuit breaker to closed state.
   */
  reset(): void {
    this.transitionTo(CircuitState.Closed);
    this.failureCount = 0;
    this.successCount = 0;
    this.failures = [];
  }

  /**
   * Check and perform state transitions based on time.
   */
  private checkStateTransition(): void {
    if (
      this.state === CircuitState.Open &&
      this.lastStateChange !== null &&
      Date.now() - this.lastStateChange >= this.config.resetTimeout
    ) {
      this.transitionTo(CircuitState.HalfOpen);
    }
  }

  /**
   * Transition to a new state.
   */
  private transitionTo(newState: CircuitState): void {
    const oldState = this.state;
    this.state = newState;
    this.lastStateChange = Date.now();

    if (newState === CircuitState.Closed) {
      this.failureCount = 0;
      this.successCount = 0;
      this.failures = [];
    } else if (newState === CircuitState.HalfOpen) {
      this.successCount = 0;
    } else if (newState === CircuitState.Open) {
      // Reset success count on open
      this.successCount = 0;
    }
  }
}

/**
 * Manager for multiple circuit breakers.
 */
export class CircuitBreakerManager {
  private breakers: Map<string, CircuitBreaker> = new Map();
  private defaultConfig: CircuitBreakerConfig;

  constructor(defaultConfig: Partial<CircuitBreakerConfig> = {}) {
    this.defaultConfig = { ...DEFAULT_CONFIG, ...defaultConfig };
  }

  /**
   * Get or create a circuit breaker by name.
   */
  getBreaker(name: string, config?: Partial<CircuitBreakerConfig>): CircuitBreaker {
    if (!this.breakers.has(name)) {
      this.breakers.set(
        name,
        new CircuitBreaker({
          ...this.defaultConfig,
          ...config,
          name,
        })
      );
    }
    return this.breakers.get(name)!;
  }

  /**
   * Get all circuit breaker names.
   */
  getNames(): string[] {
    return Array.from(this.breakers.keys());
  }

  /**
   * Get all circuit breakers.
   */
  getAll(): Map<string, CircuitBreaker> {
    return new Map(this.breakers);
  }

  /**
   * Remove a circuit breaker.
   */
  remove(name: string): boolean {
    return this.breakers.delete(name);
  }

  /**
   * Clear all circuit breakers.
   */
  clear(): void {
    this.breakers.clear();
  }
}

/**
 * Pre-configured API circuit breaker (5 failures, 30s reset).
 */
export function apiCircuitBreaker(name = 'api'): CircuitBreaker {
  return new CircuitBreaker({
    name,
    failureThreshold: 5,
    resetTimeout: 30000,
    failureWindow: 60000,
  });
}

/**
 * Pre-configured database circuit breaker (3 failures, 60s reset).
 */
export function databaseCircuitBreaker(name = 'database'): CircuitBreaker {
  return new CircuitBreaker({
    name,
    failureThreshold: 3,
    resetTimeout: 60000,
    failureWindow: 120000,
  });
}
