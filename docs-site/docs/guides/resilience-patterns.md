# Aether Resilience Patterns

This guide covers the resilience patterns available in Aether for building robust, fault-tolerant actor systems.

> **NOTE**: The examples below use SDK syntax (Python, JavaScript, Go). In the Aether v2.0.0 Rust-native runtime, these patterns are built into `aether_actor::resilience`. See the [API Reference](../api-reference.md) for Rust usage.

## Overview

The resilience module provides production-ready patterns for building robust, fault-tolerant actor systems:

| Pattern | Purpose | Use Case |
|---------|---------|----------|
| **Circuit Breaker** | Prevent cascading failures | External service calls |
| **Retry** | Handle transient failures | Network operations |
| **Rate Limiter** | Control request rates | API throttling |
| **Health Check** | Monitor system health | Kubernetes probes |
| **Bulkhead** | Isolate resources | Critical operations |

## Installation (Rust)

```bash
cargo add aether-actor
```

The resilience patterns are included in the `aether_actor` crate under the `resilience` module.

### External SDKs

```bash
# Python
pip install aether-sdk

# JavaScript/TypeScript
pnpm add @aether/sdk

# Go
go get github.com/aether-core/sdk-go/aether/resilience
```

## Quick Start

### Rust

```python
from aether_sdk.resilience import (
    CircuitBreaker,
    RetryPolicy,
    RateLimiter,
    Bulkhead,
    ResilientExecutor,
)

# Create individual patterns
breaker = CircuitBreaker()
retry = RetryPolicy()
limiter = RateLimiter()
bulkhead = Bulkhead()

# Or use the combined executor
executor = ResilientExecutor(
    breaker=breaker,
    retry=retry,
    rate_limiter=limiter,
    bulkhead=bulkhead,
)

# Execute with all protections
async def my_operation():
    # Your async operation here
    return "result"

result = await executor.execute(my_operation)
```

### JavaScript/TypeScript

```typescript
import {
  CircuitBreaker,
  RetryPolicy,
  RateLimiter,
  Bulkhead,
  ResilientExecutor,
} from '@aether/sdk/resilience';

// Create individual patterns
const breaker = new CircuitBreaker();
const retry = new RetryPolicy();
const limiter = new RateLimiter();
const bulkhead = new Bulkhead();

// Or use the combined executor
const executor = new ResilientExecutor({
  breaker,
  retry,
  rateLimiter: limiter,
  bulkhead,
});

// Execute with all protections
const result = await executor.execute(async () => {
  // Your operation here
  return 'result';
});
```

### Go

```go
import "github.com/aether-core/sdk-go/aether/resilience"

func main() {
    // Create individual patterns
    breaker := resilience.NewCircuitBreaker(resilience.DefaultCircuitBreakerConfig())
    retry := resilience.NewRetryPolicy(resilience.DefaultRetryConfig())
    limiter := resilience.NewRateLimiter(resilience.DefaultRateLimitConfig())
    bulkhead := resilience.NewBulkhead(resilience.DefaultBulkheadConfig())

    // Or use the combined executor
    executor := resilience.DefaultExecutor()

    // Execute with all protections
    result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
        // Your operation here
        return "result", nil
    })
}
```

## Circuit Breaker

Prevents cascading failures by stopping requests to a failing service.

### States

- **Closed**: Normal operation, requests pass through
- **Open**: Failing, requests are rejected immediately
- **Half-Open**: Testing if service recovered

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `failure_threshold` | 5 | Failures before opening |
| `success_threshold` | 3 | Successes before closing from half-open |
| `timeout_ms` | 30000 | Time before attempting reset |
| `half_open_max_calls` | 3 | Max calls allowed in half-open state |
| `failure_window_ms` | 60000 | Time window for counting failures |

### Example

```python
from aether_sdk.resilience import CircuitBreaker, CircuitBreakerConfig

config = CircuitBreakerConfig(
    failure_threshold=5,
    success_threshold=3,
    timeout_ms=30000,
)

breaker = CircuitBreaker(config)

# Use with callbacks
def on_open():
    print("Circuit opened!")

def on_close():
    print("Circuit closed!")

config = CircuitBreakerConfig(
    failure_threshold=5,
    on_open=on_open,
    on_close=on_close,
)

# Execute through circuit breaker
try:
    result = await breaker.execute(lambda: external_service_call())
except CircuitBreakerError:
    # Handle rejected call
    pass
```

## Retry

Handles transient failures with configurable backoff strategies.

### Backoff Strategies

| Strategy | Description |
|----------|-------------|
| `FIXED` | Constant delay between retries |
| `LINEAR` | Delay increases linearly |
| `EXPONENTIAL` | Delay doubles each time |
| `EXPONENTIAL_JITTER` | Exponential with random jitter |

### Predefined Policies

| Policy | Attempts | Base Delay | Max Delay | Use Case |
|--------|----------|------------|-----------|----------|
| `network_retry_policy` | 3 | 100ms | 5s | Network errors |
| `database_retry_policy` | 5 | 50ms | 2s | Database ops |
| `aggressive_retry_policy` | 10 | 10ms | 1s | Quick recovery |
| `conservative_retry_policy` | 2 | 1s | 10s | Critical ops |

### Example

```python
from aether_sdk.resilience import RetryPolicy, RetryConfig, BackoffStrategy

config = RetryConfig(
    max_attempts=5,
    backoff=BackoffStrategy.EXPONENTIAL_JITTER,
    base_delay_ms=100,
    max_delay_ms=10000,
    multiplier=2.0,
    jitter_factor=0.1,
    is_retryable=lambda error, attempt: "timeout" in str(error).lower(),
)

policy = RetryPolicy(config)

try:
    result = await policy.execute(lambda: flaky_operation())
    print(f"Success after {result.attempts} attempts")
except RetryExhaustedError as e:
    print(f"Failed after {e.attempts} attempts")
```

## Rate Limiter

Controls request rates with multiple strategies.

### Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| `TOKEN_BUCKET` | Allows bursts up to bucket size | APIs with bursty traffic |
| `SLIDING_WINDOW` | Smooth rate limiting | Strict rate enforcement |
| `FIXED_WINDOW` | Simple window-based limiting | Basic throttling |

### Example

```python
from aether_sdk.resilience import RateLimiter, RateLimitConfig, RateLimitStrategy

# Token bucket for API (100 req/s with 200 burst)
config = RateLimitConfig(
    requests_per_second=100,
    burst_size=200,
    strategy=RateLimitStrategy.TOKEN_BUCKET,
)

limiter = RateLimiter(config)

# Try acquire (non-blocking)
result = await limiter.try_acquire()
if result.allowed:
    # Process request
    pass
else:
    # Wait time available in result.wait_time_ms
    print(f"Rate limited, wait {result.wait_time_ms}ms")

# Acquire with wait (blocking)
await limiter.acquire(max_wait_ms=5000)  # Wait up to 5 seconds
```

## Health Check

Kubernetes-compatible health probes for actors.

### Probe Types

| Probe | Purpose | Kubernetes |
|-------|---------|------------|
| **Liveness** | Is the service alive? | `/healthz` |
| **Readiness** | Is the service ready? | `/readyz` |
| **Startup** | Has the service started? | `/startupz` |

### Predefined Checks

- `ping_health_check()` - Basic liveness
- `memory_health_check()` - Memory usage
- `cpu_health_check()` - CPU usage (JS only)
- `dependency_health_check()` - External dependencies
- `state_health_check()` - State storage

### Example

```python
from aether_sdk.resilience import (
    HealthChecker,
    ping_health_check,
    memory_health_check,
    dependency_health_check,
)

checker = HealthChecker("my-actor", "1.0.0")

# Register checks
checker.register_check("ping", ping_health_check())
checker.register_check("memory", memory_health_check(max_heap_mb=1024))

# Custom dependency check
async def check_database():
    # Check database connectivity
    return HealthCheckResult(
        status=HealthStatus.HEALTHY,
        component_id="database",
        component_type="dependency",
    )

checker.register_check("database", check_database, critical=True)

# Get probe results
liveness = await checker.get_liveness()       # { "alive": true }
readiness = await checker.get_readiness()      # { "ready": true, "checks": {...} }
```

## Bulkhead

Isolates resources to prevent one failing component from taking down the system.

### Example

```python
from aether_sdk.resilience import Bulkhead, BulkheadConfig

config = BulkheadConfig(
    max_concurrent=10,
    max_queued=50,
    timeout_ms=30000,
)

bulkhead = Bulkhead(config)

# Execute with bulkhead protection
try:
    result = await bulkhead.execute(lambda: critical_operation())
except BulkheadRejectedError:
    # At capacity
    pass
except BulkheadTimeoutError:
    # Queued and timed out
    pass
```

## ResilientExecutor

Combines all patterns for comprehensive protection.

### Order of Operations

1. **Rate Limiting** - Check if request is allowed
2. **Bulkhead** - Check capacity
3. **Circuit Breaker** - Check if service is healthy
4. **Retry** - Handle transient failures

### Predefined Executors

| Executor | Use Case |
|----------|----------|
| `DefaultExecutor()` | General purpose |
| `APIExecutor()` | API calls |
| `DatabaseExecutor()` | Database operations |
| `CriticalExecutor()` | Critical operations |

### Example

```python
from aether_sdk.resilience import (
    CircuitBreaker,
    CircuitBreakerConfig,
    RetryPolicy,
    RateLimiter,
    Bulkhead,
    ResilientExecutor,
)

# Build custom executor
executor = ResilientExecutor(
    breaker=CircuitBreaker(CircuitBreakerConfig(
        failure_threshold=5,
        timeout_ms=30000,
    )),
    retry=RetryPolicy(),
    rate_limiter=RateLimiter(),
    bulkhead=Bulkhead(),
)

# Execute with all protections
async def my_api_call():
    response = await http_client.get("https://api.example.com/data")
    return response.json()

result = await executor.execute(my_api_call)
```

## Metrics and Monitoring

### Prometheus Metrics

All resilience patterns expose Prometheus-compatible metrics:

```
# Circuit Breaker
aether_circuit_breaker_state{name="api"} 0
aether_circuit_breaker_calls_total{name="api",result="success"} 100
aether_circuit_breaker_calls_total{name="api",result="rejected"} 5

# Retry
aether_retry_attempts_total{name="db"} 150
aether_retry_exhausted_total{name="db"} 2

# Rate Limiter
aether_rate_limiter_requests_total{name="api",result="allowed"} 1000
aether_rate_limiter_requests_total{name="api",result="rejected"} 50

# Bulkhead
aether_bulkhead_active_calls{name="api"} 5
aether_bulkhead_calls_total{name="api",result="accepted"} 500
```

### OpenTelemetry Tracing

All patterns are instrumented with OpenTelemetry spans:

- `aether.circuit_breaker.execute`
- `aether.retry.execute`
- `aether.rate_limiter.acquire`
- `aether.bulkhead.execute`

## Best Practices

### 1. Layer Your Patterns

```python
# Good: Rate limit at edge, circuit breaker for services
edge_executor = ResilientExecutor(rate_limiter=edge_limiter)
service_executor = ResilientExecutor(
    breaker=service_breaker,
    retry=service_retry,
)
```

### 2. Set Appropriate Timeouts

```python
# Bad: No timeout
result = await executor.execute(lambda: slow_operation())

# Good: Context with timeout
async with asyncio.timeout(30):
    result = await executor.execute(lambda: operation())
```

### 3. Handle Errors Gracefully

```python
try:
    result = await executor.execute(operation)
except CircuitBreakerError:
    # Return cached data or default
    return get_cached_data()
except RetryExhaustedError:
    # Log and alert
    logger.error("Operation failed after retries")
    raise
except RateLimitExhaustedError:
    # Queue for later or return 429
    return Response(status_code=429)
```

### 4. Monitor Your Patterns

```python
# Export metrics for monitoring
metrics = resilience_metrics.export_prometheus()
expose_metrics_endpoint(metrics)

# Check circuit breaker health
open_breakers = manager.get_open_breakers()
if open_breakers:
    alert(f"Circuits open: {open_breakers}")
```

## Migration from v1.3.0

No breaking changes. Simply import the new resilience module:

```python
# New in v1.4.0
from aether_sdk.resilience import (
    CircuitBreaker,
    RetryPolicy,
    RateLimiter,
    HealthChecker,
    Bulkhead,
    ResilientExecutor,
)
```

## Reference

### Error Types

| Error | Pattern | When Raised |
|-------|---------|-------------|
| `CircuitBreakerError` | Circuit Breaker | Circuit is open |
| `RetryExhaustedError` | Retry | All attempts failed |
| `RateLimitExhaustedError` | Rate Limiter | Rate limit exceeded |
| `BulkheadRejectedError` | Bulkhead | At capacity |
| `BulkheadTimeoutError` | Bulkhead | Queued and timed out |

### Configuration Defaults

| Pattern | Key | Default |
|---------|-----|---------|
| Circuit Breaker | `failure_threshold` | 5 |
| Circuit Breaker | `timeout_ms` | 30000 |
| Retry | `max_attempts` | 3 |
| Retry | `base_delay_ms` | 100 |
| Rate Limiter | `requests_per_second` | 100 |
| Rate Limiter | `burst_size` | 100 |
| Bulkhead | `max_concurrent` | 10 |
| Bulkhead | `max_queued` | 100 |
