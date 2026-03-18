# Aether JavaScript SDK Changelog

All notable changes to the Aether JavaScript SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2024-03-18

### Added

#### Resilience Module (`aether-sdk/resilience`)

- **Circuit Breaker** (`circuit_breaker.ts`)
  - `CircuitBreaker` class with closed/open/half-open states
  - `CircuitState` enum
  - `CircuitBreakerConfig`, `CircuitBreakerStats` interfaces
  - `CircuitBreakerError` class
  - `execute()` and `executeWithFallback()` methods
  - `CircuitBreakerManager` for managing multiple breakers
  - Factory functions: `apiCircuitBreaker()`, `databaseCircuitBreaker()`

- **Retry Policy** (`retry.ts`)
  - `RetryPolicy` class with backoff strategies
  - `BackoffStrategy` enum: Fixed, Linear, Exponential, ExponentialJitter
  - `RetryConfig`, `RetryResult` interfaces
  - `RetryExhaustedError` class
  - Factory functions: `networkRetryPolicy()`, `databaseRetryPolicy()`, `aggressiveRetryPolicy()`, `conservativeRetryPolicy()`

- **Rate Limiter** (`rate_limiter.ts`)
  - `RateLimiter` class with multiple strategies
  - `RateLimitStrategy` enum: TokenBucket, SlidingWindow, FixedWindow
  - `RateLimitConfig`, `RateLimitResult` interfaces
  - `RateLimitExhaustedError` class
  - `tryAcquire()` and `acquire()` methods
  - Factory functions: `apiRateLimiter()`, `strictRateLimiter()`, `burstyRateLimiter()`

- **Health Check** (`health_check.ts`)
  - `HealthChecker` class with Kubernetes probes
  - `HealthStatus` enum: Healthy, Unhealthy, Degraded, Starting
  - `HealthCheckConfig`, `HealthCheckResult`, `HealthReport` interfaces
  - `liveness()`, `readiness()`, `startup()` methods
  - Pre-built health checks: `pingHealthCheck()`, `memoryHealthCheck()`, `stateHealthCheck()`, `dependencyHealthCheck()`

- **Bulkhead** (`bulkhead.ts`)
  - `Bulkhead` class for resource isolation
  - `BulkheadConfig`, `BulkheadStats` interfaces
  - `BulkheadRejectedError`, `BulkheadTimeoutError` classes
  - `hasCapacity()`, `availablePermits()` methods
  - Factory functions: `apiBulkhead()`, `databaseBulkhead()`, `strictBulkhead()`

- **Resilient Executor** (`executor.ts`)
  - `ResilientExecutor` class combining all patterns
  - `ResilientExecutorBuilder` for fluent configuration
  - Order: Rate Limiter → Bulkhead → Circuit Breaker → Retry

- **Tracing** (`tracing.ts`)
  - OpenTelemetry integration with graceful fallback
  - `withTracing()` async helper
  - Traced wrappers: `tracedCircuitBreaker()`, `tracedRetry()`, `tracedRateLimiter()`, `tracedBulkhead()`, `tracedHealthCheck()`
  - `ResilienceInstrumentation` class

#### Validation Module (`aether-sdk/validation`)

- **Validator Class** (`validators.ts`)
  - Fluent API for building validation rules
  - Type validations: `string()`, `integer()`, `float()`, `boolean()`, `array()`, `object()`
  - String validations: `minLength()`, `maxLength()`, `pattern()`
  - Numeric validations: `minValue()`, `maxValue()`, `range()`
  - Format validations: `email()`, `url()`, `uuid()`, `phone()`, `slug()`
  - List validations: `minItems()`, `maxItems()`
  - Conditional validation: `when()`
  - Custom validation: `custom()`

- **Standalone Validators**
  - `validateEmail()`, `validateURL()`, `validateUUID()`
  - `validatePhone()`, `validateSlug()`, `validateAlphanumeric()`, `validateUsername()`
  - `validateInteger()`, `validateFloat()`, `validateString()`
  - `validateDateTime()`, `validateEnum()`, `validateList()`, `validateObject()`, `validateRequired()`
  - Regex patterns exported: `EMAIL_PATTERN`, `UUID_PATTERN`, `ALPHANUMERIC_PATTERN`, etc.

- **Sanitization Functions** (`sanitize.ts`)
  - `sanitizeString()`: Remove null bytes, trim, truncate
  - `sanitizeHTML()`: Escape HTML entities
  - `sanitizeSQL()`: Basic SQL injection prevention
  - `sanitizeURL()`: Validate and normalize URLs
  - `sanitizeJSON()`: Recursively sanitize JSON data
  - `sanitizeFilename()`, `sanitizePath()`, `sanitizeSlug()`
  - `removeControlChars()`, `trimAndNormalizeWhitespace()`
  - `redactSensitive()`: Redact sensitive data for logging
  - `escapeRegex()`, `escapeShell()`
  - `normalizeLineEndings()`, `stripHTML()`, `truncate()`

### Changed
- Version bumped from 0.1.0 to 0.2.0
- Main `index.ts` now exports both `resilience` and `validation` modules

---

## [0.1.0] - 2024-03-11

### Added
- Initial SDK release
- Core actor functionality
- State management
- Messaging support
- HTTP client
- Capability-based security
- Error handling
