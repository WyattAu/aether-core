# Aether SDK Changelog

All notable changes to the Aether SDKs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2024-03-18

### Added

#### Resilience Module (M1 - Reliability Foundation)
- **Circuit Breaker**: Prevents cascading failures with closed/open/half-open states
  - Configurable failure threshold, success threshold, and reset timeout
  - State change callbacks for monitoring
  - Statistics tracking (failures, successes, rejected calls)
  - `CircuitBreakerManager` for managing multiple circuit breakers
  - Pre-configured factory functions: `api_circuit_breaker()`, `database_circuit_breaker()`

- **Retry Policy**: Handles transient failures with multiple backoff strategies
  - Backoff strategies: Fixed, Linear, Exponential, Exponential-Jitter
  - Configurable max attempts, delays, and jitter
  - Custom retry predicates for fine-grained control
  - Pre-configured policies: `network_retry_policy()`, `database_retry_policy()`, `aggressive_retry_policy()`, `conservative_retry_policy()`

- **Rate Limiter**: Controls request rates with multiple strategies
  - Token Bucket strategy with configurable burst size
  - Sliding Window strategy for smooth rate limiting
  - Fixed Window strategy for simple window-based limiting
  - Async `try_acquire()` and `acquire()` methods
  - `RateLimiterManager` for managing multiple rate limiters
  - Pre-configured limiters: `api_rate_limiter()`, `strict_rate_limiter()`, `bursty_rate_limiter()`

- **Health Check**: Kubernetes-compatible health probes
  - Liveness, Readiness, and Startup probes
  - Configurable check intervals, timeouts, and thresholds
  - Periodic background health checking
  - Pre-built health checks: `ping_health_check()`, `memory_health_check()`, `state_health_check()`, `dependency_health_check()`

- **Bulkhead**: Resource isolation with concurrent call limits
  - Configurable max concurrent calls and queue size
  - Queue timeout support
  - `BulkheadManager` for managing multiple bulkheads
  - Pre-configured bulkheads: `api_bulkhead()`, `database_bulkhead()`, `strict_bulkhead()`

- **ResilientExecutor**: Combined executor that applies all resilience patterns
  - Order: Rate Limiter → Bulkhead → Circuit Breaker → Retry
  - Builder pattern for configuration
  - Single `execute()` method with full protection

#### Observability Module (M2 - Observability)
- **OpenTelemetry Tracing**: Distributed tracing integration
  - Graceful fallback when OpenTelemetry is not installed
  - Decorators for traced operations: `@traced_circuit_breaker`, `@traced_retry`, `@traced_rate_limiter`, `@traced_bulkhead`
  - `ResilienceInstrumentation` class for context managers
  - Helper functions: `create_resilience_span()`, `record_resilience_event()`, `set_resilience_attribute()`

#### Validation Module (M3 - Security Hardening)
- **Validator**: Fluent validation API
  - Type validations: `string()`, `integer()`, `float()`, `boolean()`, `list()`, `dict()`
  - String validations: `min_length()`, `max_length()`, `pattern()`
  - Numeric validations: `min_value()`, `max_value()`, `range()`
  - Format validations: `email()`, `url()`, `uuid()`, `phone()`, `slug()`
  - Conditional validation with `when()`
  - Custom validation with `custom()`

- **SchemaValidator**: JSON Schema-like validation
  - Object validation with properties, required fields
  - Array validation with items schema
  - String constraints: minLength, maxLength, pattern, format, enum
  - Number constraints: minimum, maximum, exclusiveMinimum, exclusiveMaximum

- **Standalone Validators**: `validate_email()`, `validate_url()`, `validate_uuid()`, `validate_phone()`, `validate_slug()`, `validate_alphanumeric()`, `validate_username()`, `validate_integer()`, `validate_float()`, `validate_string()`, `validate_datetime()`, `validate_enum()`, `validate_list()`, `validate_dict()`

- **Sanitization Functions**:
  - `sanitize_string()`: Remove null bytes, trim whitespace
  - `sanitize_html()`: Escape HTML entities
  - `sanitize_sql()`: Basic SQL injection prevention
  - `sanitize_url()`: Validate and normalize URLs
  - `sanitize_json()`: Recursively sanitize JSON data

### Changed
- Version bumped from 0.1.0 to 0.2.0

### Documentation
- Added `docs-site/docs/guides/resilience-patterns.md`
- Added `docs-site/docs/examples/resilient-actor.md`
- Added Grafana dashboard at `docs-site/static/dashboards/aether-resilience.json`

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
