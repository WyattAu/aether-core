/**
 * Common types for the resilience module.
 * @module aether/resilience/types
 */

/**
 * Circuit breaker states.
 */
export enum CircuitState {
  Closed = 'closed',
  Open = 'open',
  HalfOpen = 'half-open',
}

/**
 * Backoff strategies for retry.
 */
export enum BackoffStrategy {
  Fixed = 'fixed',
  Linear = 'linear',
  Exponential = 'exponential',
  ExponentialJitter = 'exponential-jitter',
}

/**
 * Rate limiting strategies.
 */
export enum RateLimitStrategy {
  TokenBucket = 'token-bucket',
  SlidingWindow = 'sliding-window',
  FixedWindow = 'fixed-window',
}

/**
 * Health status for health checks.
 */
export enum HealthStatus {
  Healthy = 'healthy',
  Unhealthy = 'unhealthy',
  Degraded = 'degraded',
  Starting = 'starting',
}

/**
 * Circuit breaker configuration.
 */
export interface CircuitBreakerConfig {
  /** Failure threshold before opening (default: 5) */
  failureThreshold: number;
  /** Success threshold in half-open to close (default: 3) */
  successThreshold: number;
  /** Time in open state before half-open (ms, default: 30000) */
  resetTimeout: number;
  /** Time window for counting failures (ms, default: 60000) */
  failureWindow: number;
  /** Name for this circuit breaker */
  name: string;
}

/**
 * Circuit breaker statistics.
 */
export interface CircuitBreakerStats {
  state: CircuitState;
  failureCount: number;
  successCount: number;
  totalCalls: number;
  lastFailureTime: number | null;
  lastStateChange: number | null;
}

/**
 * Retry configuration.
 */
export interface RetryConfig {
  /** Maximum retry attempts (default: 3) */
  maxAttempts: number;
  /** Initial delay in ms (default: 100) */
  initialDelay: number;
  /** Maximum delay in ms (default: 30000) */
  maxDelay: number;
  /** Backoff multiplier (default: 2) */
  multiplier: number;
  /** Backoff strategy */
  strategy: BackoffStrategy;
  /** Jitter factor 0-1 (default: 0.1) */
  jitterFactor: number;
  /** Name for this retry policy */
  name: string;
}

/**
 * Retry result.
 */
export interface RetryResult<T> {
  /** The result if successful */
  result: T | null;
  /** Error if all retries failed */
  error: Error | null;
  /** Number of attempts made */
  attempts: number;
  /** Total time spent in ms */
  totalTime: number;
  /** Whether the operation succeeded */
  success: boolean;
}

/**
 * Rate limiter configuration.
 */
export interface RateLimitConfig {
  /** Maximum requests per window */
  maxRequests: number;
  /** Window duration in ms (default: 1000) */
  windowMs: number;
  /** Rate limiting strategy */
  strategy: RateLimitStrategy;
  /** Token bucket refill rate (tokens per second, for token bucket) */
  refillRate: number;
  /** Name for this rate limiter */
  name: string;
}

/**
 * Rate limit result.
 */
export interface RateLimitResult {
  /** Whether the request is allowed */
  allowed: boolean;
  /** Remaining requests in current window */
  remaining: number;
  /** Time until reset in ms */
  resetIn: number;
  /** Current retry-after header value */
  retryAfter: number;
}

/**
 * Health check configuration.
 */
export interface HealthCheckConfig {
  /** Health check name */
  name: string;
  /** Check interval in ms (default: 30000) */
  interval: number;
  /** Check timeout in ms (default: 5000) */
  timeout: number;
  /** Number of failures before unhealthy (default: 3) */
  failureThreshold: number;
  /** Number of successes before healthy (default: 1) */
  successThreshold: number;
  /** Initial delay before first check in ms (default: 0) */
  initialDelay: number;
}

/**
 * Health check result.
 */
export interface HealthCheckResult {
  /** Name of the check */
  name: string;
  /** Current status */
  status: HealthStatus;
  /** Optional message */
  message?: string;
  /** Timestamp of the check */
  timestamp: number;
  /** Duration of the check in ms */
  duration: number;
  /** Additional details */
  details?: Record<string, unknown>;
}

/**
 * Health report containing all check results.
 */
export interface HealthReport {
  /** Overall status */
  status: HealthStatus;
  /** Individual check results */
  checks: Record<string, HealthCheckResult>;
  /** Timestamp of the report */
  timestamp: number;
}

/**
 * Bulkhead configuration.
 */
export interface BulkheadConfig {
  /** Maximum concurrent calls (default: 10) */
  maxConcurrent: number;
  /** Maximum queue size (default: 0, no queue) */
  maxQueueSize: number;
  /** Queue timeout in ms (default: 0, immediate rejection) */
  queueTimeout: number;
  /** Name for this bulkhead */
  name: string;
}

/**
 * Bulkhead statistics.
 */
export interface BulkheadStats {
  /** Current active calls */
  active: number;
  /** Current queue size */
  queueSize: number;
  /** Available permits */
  available: number;
  /** Total calls rejected */
  rejected: number;
  /** Total calls accepted */
  accepted: number;
}

/**
 * Tracing configuration.
 */
export interface TracingConfig {
  /** Enable OpenTelemetry tracing */
  enabled: boolean;
  /** Service name reported to tracer */
  serviceName: string;
  /** Sampling rate (0.0 to 1.0) */
  sampleRate: number;
}

/**
 * Tracing context interface.
 */
export interface TracingContext {
  /** Set an attribute on the span */
  setAttribute(key: string, value: unknown): void;
  /** Add an event to the span */
  addEvent(name: string, attributes?: Record<string, unknown>): void;
  /** End the span */
  end(error?: Error): void;
}

/**
 * Health check function type.
 */
export type HealthCheckFn = () => Promise<HealthCheckResult> | HealthCheckResult;

/**
 * Async function type for execution.
 */
export type AsyncFunction<T> = () => Promise<T>;
