/**
 * OpenTelemetry Tracing Integration for Resilience Patterns.
 * Provides tracing spans for all resilience patterns.
 * @module aether/resilience/tracing
 */

import { TracingConfig, TracingContext } from './types';

// Check if OpenTelemetry is available
let TRACING_AVAILABLE = false;
let apiModule: typeof import('@opentelemetry/api') | null = null;

try {
  // Dynamic import check - will be undefined if not installed
  // @ts-ignore - optional dependency
  apiModule = require('@opentelemetry/api');
  TRACING_AVAILABLE = apiModule !== null;
} catch {
  // OpenTelemetry not installed
}

/**
 * Check if tracing is available.
 */
export function isTracingAvailable(): boolean {
  return TRACING_AVAILABLE;
}

/**
 * Get a tracer for resilience patterns.
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
 * Minimal span interface.
 */
interface Span {
  setAttribute(key: string, value: unknown): void;
  addEvent(name: string, attributes?: Record<string, unknown>): void;
  setStatus(status: { code: number; message?: string }): void;
  end(): void;
}

/**
 * No-op span for when tracing is disabled.
 */
class NoOpSpan implements Span {
  setAttribute(): this {
    return this;
  }
  addEvent(): this {
    return this;
  }
  setStatus(): this {
    return this;
  }
  end(): void {}
}

const noOpSpan = new NoOpSpan();

/**
 * Execute a function with tracing.
 *
 * @param spanName - Name of the span
 * @param fn - Function to execute
 * @returns Result of the function
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
 * Resilience instrumentation configuration.
 */
export class ResilienceInstrumentation {
  private tracer: unknown;
  private config: TracingConfig;

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
   * Create a circuit breaker span.
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
   * Create a retry span.
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
   * Create a rate limiter span.
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
   * Create a bulkhead span.
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
