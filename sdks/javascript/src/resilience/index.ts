/**
 * Aether SDK Resilience Module
 *
 * Provides patterns for building robust, fault-tolerant actor systems:
 * - Circuit Breaker: Prevents cascading failures
 * - Retry: Handles transient failures with backoff
 * - Rate Limiter: Controls request rates
 * - Health Check: Kubernetes-compatible probes
 * - Bulkhead: Resource isolation
 * - Tracing: OpenTelemetry integration
 *
 * @example
 * ```typescript
 * import {
 *   CircuitBreaker,
 *   RetryPolicy,
 *   RateLimiter,
 *   Bulkhead,
 *   ResilientExecutor,
 * } from 'aether-sdk/resilience';
 *
 * // Create resilience patterns
 * const breaker = new CircuitBreaker({ failureThreshold: 5 });
 * const retry = new RetryPolicy({ maxAttempts: 3 });
 * const limiter = new RateLimiter({ maxRequests: 100 });
 * const bulkhead = new Bulkhead({ maxConcurrent: 10 });
 *
 * // Combine with ResilientExecutor
 * const executor = new ResilientExecutor({
 *   breaker,
 *   retry,
 *   rateLimiter: limiter,
 *   bulkhead,
 * });
 *
 * // Execute with all protections
 * const result = await executor.execute(() => fetchData());
 * ```
 *
 * @module aether/resilience
 */

// Types
export {
  CircuitState,
  BackoffStrategy,
  RateLimitStrategy,
  HealthStatus,
  CircuitBreakerConfig,
  CircuitBreakerStats,
  RetryConfig,
  RetryResult,
  RateLimitConfig,
  RateLimitResult,
  HealthCheckConfig,
  HealthCheckResult,
  HealthReport,
  BulkheadConfig,
  BulkheadStats,
  TracingConfig,
  TracingContext,
  HealthCheckFn,
  AsyncFunction,
} from './types';

// Circuit Breaker
export {
  CircuitBreakerError,
  CircuitBreaker,
  CircuitBreakerManager,
  apiCircuitBreaker,
  databaseCircuitBreaker,
} from './circuit_breaker';

// Retry
export {
  RetryExhaustedError,
  RetryPolicy,
  networkRetryPolicy,
  databaseRetryPolicy,
  aggressiveRetryPolicy,
  conservativeRetryPolicy,
} from './retry';

// Rate Limiter
export {
  RateLimitExhaustedError,
  RateLimiter,
  RateLimiterManager,
  apiRateLimiter,
  strictRateLimiter,
  burstyRateLimiter,
} from './rate_limiter';

// Health Check
export {
  HealthChecker,
  pingHealthCheck,
  memoryHealthCheck,
  stateHealthCheck,
  dependencyHealthCheck,
} from './health_check';

// Bulkhead
export {
  BulkheadRejectedError,
  BulkheadTimeoutError,
  Bulkhead,
  BulkheadManager,
  apiBulkhead,
  databaseBulkhead,
  strictBulkhead,
} from './bulkhead';

// Tracing
export {
  isTracingAvailable,
  getTracer,
  withTracing,
  tracedCircuitBreaker,
  tracedRetry,
  tracedRateLimiter,
  tracedBulkhead,
  tracedHealthCheck,
  ResilienceInstrumentation,
  TRACING_AVAILABLE,
} from './tracing';

// Executor
export {
  ResilientExecutor,
  ResilientExecutorBuilder,
  ResilientExecutorConfig,
} from './executor';
