# Aether Go SDK Changelog

All notable changes to the Aether Go SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2024-03-18

### Added

#### Resilience Module (`aether/resilience`)
- **Circuit Breaker**: Prevents cascading failures
  - `CircuitBreaker` with closed/open/half-open states
  - Configurable failure threshold, success threshold, timeout
  - State change callbacks
  - `CircuitBreakerStats` for metrics

- **Retry Policy**: Handles transient failures
  - Multiple backoff strategies: Fixed, Linear, Exponential, ExponentialJitter
  - Configurable max attempts and delays
  - Custom retry predicates
  - Pre-configured policies

- **Rate Limiter**: Controls request rates
  - Token Bucket, Sliding Window, Fixed Window strategies
  - `TryAcquire()` and `Acquire()` methods
  - Configurable requests per second and burst size

- **Health Check**: Kubernetes-compatible probes
  - `HealthChecker` with liveness/readiness/startup probes
  - Periodic background checking
  - Configurable intervals and thresholds

- **Bulkhead**: Resource isolation
  - Configurable max concurrent calls
  - Queue support with timeout
  - `BulkheadStats` for monitoring

- **ResilientExecutor**: Combined executor
  - Applies all patterns in order: Rate Limiter → Bulkhead → Circuit Breaker → Retry
  - Builder pattern for configuration

#### Tracing Module (`aether/resilience/tracing`)
- OpenTelemetry integration helpers
- `TracingContext` for span management
- Instrumentation functions for each pattern

#### Validation Module (`aether/validation`)
- **Validator**: Fluent validation API
  - Type, string, numeric, format, and list validations
  - Conditional and custom validations

- **Standalone Validators**: 
  - `ValidateEmail()`, `ValidateURL()`, `ValidateUUID()`
  - `ValidatePhone()`, `ValidateSlug()`, `ValidateAlphanumeric()`
  - `ValidateInteger()`, `ValidateFloat()`, `ValidateString()`
  - `ValidateDateTime()`, `ValidateEnum()`, `ValidateList()`

- **Sanitization Functions**:
  - `SanitizeString()`, `SanitizeHTML()`, `SanitizeSQL()`
  - `SanitizeURL()`, `SanitizeJSON()`, `SanitizeFilename()`
  - `SanitizePath()`, `SanitizeSlug()`, `SanitizePhone()`

- **Error Types**:
  - `ValidationError`, `ValidationErrors`
  - `SchemaValidationError`, `SchemaValidationErrors`

### Changed
- Version bumped from 0.1.0 to 0.2.0

---

## [0.1.0] - 2024-03-11

### Added
- Initial SDK release
- Core actor functionality
- State management
- Messaging support
- Capability-based security
- Error handling
