/**
 * OpenTelemetry Tracing Integration for Resilience Patterns.
 * Provides tracing spans for all resilience patterns.
 * @module aether/resilience/tracing
 */

import { TracingConfig, TracingContext } from './types';

// Check if OpenTelemetry is available
let TRACING_AVAILABLE = false;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let apiModule: any = null;

try {
  // Dynamic import check - will be undefined if not installed
  // @ts-ignore - optional dependency
  apiModule = require('@opentelemetry/api');
  TRACING_AVAILABLE = apiModule !== null;
} catch {
  // OpenTelemetry not installed
}

/**
 * Check if OpenTelemetry tracing is available.
 *
 * @returns `true` if the `@opentelemetry/api` package is installed and loaded.
 */
export function isTracingAvailable(): boolean {
  return TRACING_AVAILABLE;
}

/**
 * Get a tracer instance for resilience patterns.
 *
 * @param serviceName - The service name to report in traces
 *                      (default: `'aether-resilience'`).
 * @returns A tracer instance, or `null` if tracing is not available.
 */
export function getTracer(serviceName = 'aether-resilience'): unknown {
  if (!TRACING_AVAILABLE || !apiModule) {
    return null;
  }

  try {
    const trace = apiModule.trace;
    return trace.getTracer(serviceName);
  } catch {
    return null;
  }
}

/**
 * Minimal span interface for tracing operations.
 *
 * Provides attribute-setting, event-adding, and status-setting capabilities
 * compatible with OpenTelemetry spans.
 */
interface Span {
  /** Set a key-value attribute on the span. */
  setAttribute(key: string, value: unknown): void;
  /** Record an event with optional attributes. */
  addEvent(name: string, attributes?: Record<string, unknown>): void;
  /** Set the span status. */
  setStatus(status: { code: number; message?: string }): void;
  /** End the span. */
  end(): void;
}

/**
 * No-op span implementation used when tracing is disabled.
 *
 * All methods are safe no-ops that return `this` for chaining.
 * @internal
 */
class NoOpSpan implements Span {
  setAttribute(_key: string, _value: unknown): this {
    return this;
  }
  addEvent(_name: string, _attributes?: Record<string, unknown>): this {
    return this;
  }
  setStatus(_status: { code: number; message?: string }): this {
    return this;
  }
  end(): void {}
}

const noOpSpan = new NoOpSpan();

/**
 * Execute a function within a tracing span.
 *
 * If OpenTelemetry is available, creates a real span; otherwise uses a
 * no-op span. The span status is set to OK on success or ERROR on failure.
 *
 * @typeParam T - The return type of the function.
 * @param spanName - Name for the tracing span.
 * @param fn       - Function receiving the span and returning a promise.
 * @returns The function result.
 *
 * @example
 * ```typescript
 * const result = await withTracing('my_operation', async (span) => {
 *   span.setAttribute('my.key', 'value');
 *   return doWork();
 * });
 * ```
 */
export async function withTracing<T>(
  spanName: string,
  fn: (span: Span) => Promise<T>
): Promise<T> {
  if (!TRACING_AVAILABLE || !apiModule) {
    return fn(noOpSpan);
  }

  const tracer = getTracer();
  if (!tracer) {
    return fn(noOpSpan);
  }

  try {
    const trace = apiModule.trace;
    const context = apiModule.context;

    // Start span
    const span = (tracer as any).startActiveSpan(
      spanName,
      (span: Span) => {
        // Execute function with span
        return fn(span).then(
          (result) => {
            span.setStatus({ code: 0 }); // OK
            span.end();
            return result;
          },
          (error) => {
            span.setStatus({ code: 2, message: error?.message }); // ERROR
            span.end();
            throw error;
          }
        );
      }
    );

    return span;
  } catch {
    // If tracing fails, just run the function
    return fn(noOpSpan);
  }
}

// ============================================
// Traced Wrappers
// ============================================

/**
 * Wrap a circuit breaker operation with tracing.
 *
 * @typeParam T - The return type.
 * @param name  - The circuit breaker name.
 * @param state - The current circuit state.
 * @param fn    - The async function to trace.
 * @returns The function result.
 */
export function tracedCircuitBreaker<T>(
  name: string,
  state: string,
  fn: () => Promise<T>
): Promise<T> {
  return withTracing(`circuit_breaker.${name}.execute`, async (span) => {
    span.setAttribute('circuit_breaker.name', name);
    span.setAttribute('circuit_breaker.state', state);

    try {
      const result = await fn();
      span.setAttribute('circuit_breaker.result', 'success');
      return result;
    } catch (error) {
      span.setAttribute('circuit_breaker.result', 'error');
      throw error;
    }
  });
}

/**
 * Wrap a retry operation with tracing.
 *
 * @typeParam T - The return type.
 * @param name       - The retry policy name.
 * @param attempt    - The current attempt number (1-based).
 * @param maxAttempts - The maximum number of attempts.
 * @param fn         - The async function to trace.
 * @returns The function result.
 */
export function tracedRetry<T>(
  name: string,
  attempt: number,
  maxAttempts: number,
  fn: () => Promise<T>
): Promise<T> {
  return withTracing(`retry.${name}.attempt`, async (span) => {
    span.setAttribute('retry.name', name);
    span.setAttribute('retry.attempt', attempt);
    span.setAttribute('retry.max_attempts', maxAttempts);

    try {
      const result = await fn();
      span.setAttribute('retry.result', 'success');
      return result;
    } catch (error) {
      span.setAttribute('retry.result', 'error');
      span.addEvent('retry.failed', {
        'error.message': error instanceof Error ? error.message : 'Unknown error',
      });
      throw error;
    }
  });
}

/**
 * Wrap a rate limiter operation with tracing.
 *
 * @typeParam T - The return type.
 * @param name    - The rate limiter name.
 * @param allowed - Whether the request was allowed.
 * @param fn      - The async function to trace.
 * @returns The function result.
 */
export function tracedRateLimiter<T>(
  name: string,
  allowed: boolean,
  fn: () => Promise<T>
): Promise<T> {
  return withTracing(`rate_limiter.${name}.execute`, async (span) => {
    span.setAttribute('rate_limiter.name', name);
    span.setAttribute('rate_limiter.allowed', allowed);

    try {
      const result = await fn();
      span.setAttribute('rate_limiter.result', 'success');
      return result;
    } catch (error) {
      span.setAttribute('rate_limiter.result', 'error');
      throw error;
    }
  });
}

/**
 * Wrap a bulkhead operation with tracing.
 *
 * @typeParam T - The return type.
 * @param name           - The bulkhead name.
 * @param active         - Number of currently active calls.
 * @param maxConcurrent  - Maximum allowed concurrent calls.
 * @param fn             - The async function to trace.
 * @returns The function result.
 */
export function tracedBulkhead<T>(
  name: string,
  active: number,
  maxConcurrent: number,
  fn: () => Promise<T>
): Promise<T> {
  return withTracing(`bulkhead.${name}.execute`, async (span) => {
    span.setAttribute('bulkhead.name', name);
    span.setAttribute('bulkhead.active', active);
    span.setAttribute('bulkhead.max_concurrent', maxConcurrent);

    try {
      const result = await fn();
      span.setAttribute('bulkhead.result', 'success');
      return result;
    } catch (error) {
      span.setAttribute('bulkhead.result', 'error');
      throw error;
    }
  });
}

/**
 * Wrap a health check operation with tracing.
 *
 * @typeParam T - The return type.
 * @param name      - The health checker name.
 * @param checkName - The individual check name.
 * @param fn        - The async function to trace.
 * @returns The function result.
 */
export function tracedHealthCheck<T>(
  name: string,
  checkName: string,
  fn: () => Promise<T>
): Promise<T> {
  return withTracing(`health_check.${name}.${checkName}`, async (span) => {
    span.setAttribute('health_check.name', name);
    span.setAttribute('health_check.check', checkName);

    try {
      const result = await fn();
      span.setAttribute('health_check.result', 'success');
      return result;
    } catch (error) {
      span.setAttribute('health_check.result', 'error');
      throw error;
    }
  });
}

// ============================================
// Instrumentation Helper
// ============================================

/**
 * Resilience instrumentation configuration and span factory.
 *
 * Provides convenience methods for creating typed spans for each
 * resilience pattern without requiring direct OpenTelemetry usage.
 *
 * @example
 * ```typescript
 * const instrumentation = new ResilienceInstrumentation({
 *   serviceName: 'my-service',
 *   sampleRate: 0.5,
 * });
 *
 * const { span, end } = instrumentation.circuitBreaker('api', 'closed');
 * // ... do work ...
 * end(); // or end(error)
 * ```
 */
export class ResilienceInstrumentation {
  private tracer: unknown;
  private config: TracingConfig;

  /**
   * Create a new ResilienceInstrumentation.
   *
   * @param config - Partial tracing configuration.
   */
  constructor(config: Partial<TracingConfig> = {}) {
    this.config = {
      enabled: config.enabled ?? true,
      serviceName: config.serviceName ?? 'aether-resilience',
      sampleRate: config.sampleRate ?? 1.0,
    };

    if (this.config.enabled) {
      this.tracer = getTracer(this.config.serviceName);
    }
  }

  /**
   * Create a circuit breaker span and end callback.
   *
   * @param name  - The circuit breaker name.
   * @param state - The current circuit state.
   * @returns An object with the span and an `end` function.
   */
  circuitBreaker(name: string, state: string): {
    span: Span;
    end: (error?: Error) => void;
  } {
    const span = noOpSpan;
    return {
      span,
      end: (error?: Error) => {
        if (error) {
          span.setAttribute('error', error.message);
        }
        span.end();
      },
    };
  }

  /**
   * Create a retry span and end callback.
   *
   * @param name        - The retry policy name.
   * @param attempt     - The current attempt number.
   * @param maxAttempts - The maximum number of attempts.
   * @returns An object with the span and an `end` function.
   */
  retry(name: string, attempt: number, maxAttempts: number): {
    span: Span;
    end: (error?: Error) => void;
  } {
    const span = noOpSpan;
    return {
      span,
      end: (error?: Error) => {
        if (error) {
          span.setAttribute('error', error.message);
        }
        span.end();
      },
    };
  }

  /**
   * Create a rate limiter span and end callback.
   *
   * @param name    - The rate limiter name.
   * @param allowed - Whether the request was allowed.
   * @returns An object with the span and an `end` function.
   */
  rateLimiter(name: string, allowed: boolean): {
    span: Span;
    end: (error?: Error) => void;
  } {
    const span = noOpSpan;
    return {
      span,
      end: (error?: Error) => {
        if (error) {
          span.setAttribute('error', error.message);
        }
        span.end();
      },
    };
  }

  /**
   * Create a bulkhead span and end callback.
   *
   * @param name          - The bulkhead name.
   * @param active        - Number of currently active calls.
   * @param maxConcurrent - Maximum allowed concurrent calls.
   * @returns An object with the span and an `end` function.
   */
  bulkhead(name: string, active: number, maxConcurrent: number): {
    span: Span;
    end: (error?: Error) => void;
  } {
    const span = noOpSpan;
    return {
      span,
      end: (error?: Error) => {
        if (error) {
          span.setAttribute('error', error.message);
        }
        span.end();
      },
    };
  }
}

// ============================================
// Exports
// ============================================

export {
  TRACING_AVAILABLE,
};

export type { Span };
