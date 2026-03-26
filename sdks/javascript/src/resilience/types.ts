/**
 * Common types for the resilience module.
 * @module aether/resilience/types
 */

/**
 * Circuit breaker states.
 *
 * - **Closed** — Normal operation; requests pass through.
 * - **Open** — Requests are blocked; waiting for reset timeout.
 * - **HalfOpen** — Limited requests allowed to test recovery.
 */
export enum CircuitState {
  /** Normal operation; requests pass through. */
  Closed = 'closed',
  /** Requests are blocked; waiting for reset timeout. */
  Open = 'open',
  /** Limited requests allowed to test recovery. */
  HalfOpen = 'half-open',
}

/**
 * Backoff strategies for retry policies.
 */
export enum BackoffStrategy {
  /** Constant delay between attempts. */
  Fixed = 'fixed',
  /** Linearly increasing delay. */
  Linear = 'linear',
  /** Exponentially increasing delay. */
  Exponential = 'exponential',
  /** Exponential delay with random jitter to avoid thundering herd. */
  ExponentialJitter = 'exponential-jitter',
}

/**
 * Rate limiting strategies.
 */
export enum RateLimitStrategy {
  /** Tokens refill at a constant rate; bursty traffic allowed. */
  TokenBucket = 'token-bucket',
  /** Counts requests in a sliding time window. */
  SlidingWindow = 'sliding-window',
  /** Resets counter at fixed time intervals. */
  FixedWindow = 'fixed-window',
}

/**
 * Health status for health checks.
 */
export enum HealthStatus {
  /** The check is passing. */
  Healthy = 'healthy',
  /** The check is failing. */
  Unhealthy = 'unhealthy',
  /** The check is partially failing (below failure threshold). */
  Degraded = 'degraded',
  /** The check has not yet run. */
  Starting = 'starting',
}

/**
 * Circuit breaker configuration.
 */
export interface CircuitBreakerConfig {
  /** Number of failures before opening the circuit (default: 5). */
  failureThreshold: number;
  /** Number of consecutive successes in half-open to close (default: 3). */
  successThreshold: number;
  /** Time in open state before transitioning to half-open (ms, default: 30000). */
  resetTimeout: number;
  /** Sliding time window for counting failures (ms, default: 60000). */
  failureWindow: number;
  /** Logical name for this circuit breaker. */
  name: string;
}

/**
 * Circuit breaker statistics snapshot.
 */
export interface CircuitBreakerStats {
  /** Current circuit state. */
  state: CircuitState;
  /** Total recorded failures (since last reset). */
  failureCount: number;
  /** Total recorded successes (since last reset). */
  successCount: number;
  /** Total number of calls through the breaker. */
  totalCalls: number;
  /** Timestamp of the last failure, or `null`. */
  lastFailureTime: number | null;
  /** Timestamp of the last state transition, or `null`. */
  lastStateChange: number | null;
}

/**
 * Retry policy configuration.
 */
export interface RetryConfig {
  /** Maximum number of attempts (default: 3). */
  maxAttempts: number;
  /** Initial delay before the first retry (ms, default: 100). */
  initialDelay: number;
  /** Maximum delay cap between retries (ms, default: 30000). */
  maxDelay: number;
  /** Multiplier applied to the delay on each attempt (default: 2). */
  multiplier: number;
  /** The backoff strategy to use. */
  strategy: BackoffStrategy;
  /** Jitter factor between 0 and 1 (default: 0.1). */
  jitterFactor: number;
  /** Logical name for this retry policy. */
  name: string;
}

/**
 * Result of a retry execution.
 *
 * @typeParam T - The expected result type on success.
 */
export interface RetryResult<T> {
  /** The result if the operation succeeded. */
  result: T | null;
  /** The last error if all attempts failed. */
  error: Error | null;
  /** Total number of attempts made. */
  attempts: number;
  /** Total elapsed time including delays (ms). */
  totalTime: number;
  /** Whether the operation ultimately succeeded. */
  success: boolean;
}

/**
 * Rate limiter configuration.
 */
export interface RateLimitConfig {
  /** Maximum number of requests per window. */
  maxRequests: number;
  /** Window duration in milliseconds (default: 1000). */
  windowMs: number;
  /** The rate limiting strategy to use. */
  strategy: RateLimitStrategy;
  /** Token refill rate in tokens per second (for TokenBucket strategy). */
  refillRate: number;
  /** Logical name for this rate limiter. */
  name: string;
}

/**
 * Result of a rate limit acquisition attempt.
 */
export interface RateLimitResult {
  /** Whether the request is allowed. */
  allowed: boolean;
  /** Remaining requests in the current window/bucket. */
  remaining: number;
  /** Time until the window/bucket resets (ms). */
  resetIn: number;
  /** Suggested `Retry-After` header value (ms). */
  retryAfter: number;
}

/**
 * Health check configuration.
 */
export interface HealthCheckConfig {
  /** Logical name for this check. */
  name: string;
  /** Interval between periodic checks (ms, default: 30000). */
  interval: number;
  /** Maximum execution time per check (ms, default: 5000). */
  timeout: number;
  /** Consecutive failures before marking Unhealthy (default: 3). */
  failureThreshold: number;
  /** Consecutive successes before marking Healthy (default: 1). */
  successThreshold: number;
  /** Delay before the first check runs (ms, default: 0). */
  initialDelay: number;
}

/**
 * Result of a single health check execution.
 */
export interface HealthCheckResult {
  /** Name of the check. */
  name: string;
  /** Current health status. */
  status: HealthStatus;
  /** Optional human-readable message. */
  message?: string;
  /** Unix timestamp (ms) when the check was performed. */
  timestamp: number;
  /** Duration of the check in milliseconds. */
  duration: number;
  /** Optional arbitrary details (e.g., memory stats). */
  details?: Record<string, unknown>;
}

/**
 * Aggregated health report containing all check results.
 */
export interface HealthReport {
  /** Overall status (worst status among all checks). */
  status: HealthStatus;
  /** Individual check results keyed by check name. */
  checks: Record<string, HealthCheckResult>;
  /** Unix timestamp (ms) of the report. */
  timestamp: number;
}

/**
 * Bulkhead configuration.
 */
export interface BulkheadConfig {
  /** Maximum number of concurrent calls (default: 10). */
  maxConcurrent: number;
  /** Maximum number of queued calls (default: 0 = no queue). */
  maxQueueSize: number;
  /** Maximum wait time in the queue (ms, default: 0 = immediate rejection). */
  queueTimeout: number;
  /** Logical name for this bulkhead. */
  name: string;
}

/**
 * Bulkhead statistics snapshot.
 */
export interface BulkheadStats {
  /** Currently executing calls. */
  active: number;
  /** Currently queued calls. */
  queueSize: number;
  /** Available permits (maxConcurrent - active). */
  available: number;
  /** Total calls rejected due to capacity. */
  rejected: number;
  /** Total calls accepted for execution. */
  accepted: number;
}

/**
 * OpenTelemetry tracing configuration.
 */
export interface TracingConfig {
  /** Whether tracing is enabled (default: true). */
  enabled: boolean;
  /** Service name reported to the tracer (default: `'aether-resilience'`). */
  serviceName: string;
  /** Sampling rate from 0.0 (none) to 1.0 (all) (default: 1.0). */
  sampleRate: number;
}

/**
 * Tracing context interface for span operations.
 */
export interface TracingContext {
  /** Set an attribute on the current span. */
  setAttribute(key: string, value: unknown): void;
  /** Record an event on the current span. */
  addEvent(name: string, attributes?: Record<string, unknown>): void;
  /** End the current span, optionally recording an error. */
  end(error?: Error): void;
}

/**
 * Function type for health checks.
 *
 * May return the result synchronously or as a promise.
 */
export type HealthCheckFn = () => Promise<HealthCheckResult> | HealthCheckResult;

/**
 * Generic async function type for execution wrappers.
 *
 * @typeParam T - The return type of the function.
 */
export type AsyncFunction<T> = () => Promise<T>;
