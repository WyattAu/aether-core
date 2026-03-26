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
 * Error thrown when a circuit breaker is open and rejects a call.
 *
 * @example
 * ```typescript
 * try {
 *   await breaker.execute(() => fetchData());
 * } catch (e) {
 *   if (e instanceof CircuitBreakerError) {
 *     console.log(`Circuit '${e.name}' is ${e.state}`);
 *   }
 * }
 * ```
 */
export class CircuitBreakerError extends Error {
  /**
   * @param name    - The name of the circuit breaker.
   * @param state   - The state the breaker was in when the error was thrown.
   * @param message - Optional custom message.
   */
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
 * Protects services from cascading failures by monitoring call success/failure
 * rates and transitioning between three states:
 *
 * - **Closed** — Normal operation; requests pass through and failures are tracked.
 * - **Open** — Requests are blocked; the breaker waits for `resetTimeout` ms
 *   before transitioning to Half-Open.
 * - **Half-Open** — A limited number of trial requests are allowed to test
 *   whether the downstream service has recovered.
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

  /**
   * Create a new CircuitBreaker.
   *
   * @param config - Partial configuration; unspecified fields use defaults.
   */
  constructor(config: Partial<CircuitBreakerConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Get current circuit breaker state.
   *
   * Automatically checks for time-based state transitions
   * (Open → Half-Open) before returning.
   *
   * @returns The current {@link CircuitState}.
   */
  getState(): CircuitState {
    this.checkStateTransition();
    return this.state;
  }

  /**
   * Get circuit breaker statistics.
   *
   * @returns A snapshot of current breaker metrics.
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
   * @typeParam T - The return type of the function.
   * @param fn - Async function to execute.
   * @returns Result of the function.
   * @throws CircuitBreakerError If the circuit is open.
   * @throws Error If the function itself throws (also records a failure).
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
   * Execute a function, returning a fallback result when the circuit is open.
   *
   * @typeParam T - The return type of both functions.
   * @param fn       - Primary async function to execute.
   * @param fallback - Fallback async function called when the circuit is open.
   * @returns Result of `fn` or `fallback`.
   * @throws Error If `fn` fails for a reason other than an open circuit.
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
   * Manually record a successful operation.
   *
   * In Half-Open state, enough consecutive successes will transition
   * the breaker back to Closed.
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
   * Manually record a failed operation.
   *
   * Failures are tracked within a sliding time window. When the count of
   * recent failures exceeds `failureThreshold`, the breaker opens.
   * In Half-Open state, any failure immediately re-opens the breaker.
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
   *
   * Useful for testing or manual operational overrides.
   *
   * @param state - The desired circuit state.
   */
  forceState(state: CircuitState): void {
    this.transitionTo(state);
  }

  /**
   * Reset the circuit breaker to closed state and clear all counters.
   */
  reset(): void {
    this.transitionTo(CircuitState.Closed);
    this.failureCount = 0;
    this.successCount = 0;
    this.failures = [];
  }

  /**
   * Check and perform state transitions based on elapsed time.
   *
   * If the breaker has been Open for longer than `resetTimeout`,
   * it transitions to Half-Open.
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
   * Transition to a new state and reset relevant counters.
   *
   * @param newState - The state to transition to.
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
 *
 * Provides named access to circuit breaker instances, creating them
 * on first access with the provided or default configuration.
 *
 * @example
 * ```typescript
 * const manager = new CircuitBreakerManager({ failureThreshold: 5 });
 * const apiBreaker = manager.getBreaker('api');
 * const dbBreaker = manager.getBreaker('database', { failureThreshold: 3 });
 * ```
 */
export class CircuitBreakerManager {
  private breakers: Map<string, CircuitBreaker> = new Map();
  private defaultConfig: CircuitBreakerConfig;

  /**
   * Create a new CircuitBreakerManager.
   *
   * @param defaultConfig - Default configuration applied to all breakers
   *                        created through this manager.
   */
  constructor(defaultConfig: Partial<CircuitBreakerConfig> = {}) {
    this.defaultConfig = { ...DEFAULT_CONFIG, ...defaultConfig };
  }

  /**
   * Get or create a circuit breaker by name.
   *
   * If a breaker with the given name already exists, it is returned as-is.
   * Otherwise a new breaker is created with the manager's default config
   * overlaid by the optional per-breaker config.
   *
   * @param name   - Unique name for the circuit breaker.
   * @param config - Optional per-breaker configuration overrides.
   * @returns The existing or newly created CircuitBreaker.
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
   * Get all registered circuit breaker names.
   *
   * @returns An array of breaker names.
   */
  getNames(): string[] {
    return Array.from(this.breakers.keys());
  }

  /**
   * Get a shallow copy of all circuit breakers.
   *
   * @returns A new Map containing all breakers.
   */
  getAll(): Map<string, CircuitBreaker> {
    return new Map(this.breakers);
  }

  /**
   * Remove a circuit breaker by name.
   *
   * @param name - The name of the breaker to remove.
   * @returns `true` if the breaker was found and removed.
   */
  remove(name: string): boolean {
    return this.breakers.delete(name);
  }

  /**
   * Remove all circuit breakers.
   */
  clear(): void {
    this.breakers.clear();
  }
}

/**
 * Create a pre-configured API circuit breaker (5 failures, 30s reset).
 *
 * @param name - Breaker name (default: `'api'`).
 * @returns A CircuitBreaker tuned for typical API call patterns.
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
 * Create a pre-configured database circuit breaker (3 failures, 60s reset).
 *
 * @param name - Breaker name (default: `'database'`).
 * @returns A CircuitBreaker tuned for database query patterns.
 */
export function databaseCircuitBreaker(name = 'database'): CircuitBreaker {
  return new CircuitBreaker({
    name,
    failureThreshold: 3,
    resetTimeout: 60000,
    failureWindow: 120000,
  });
}
