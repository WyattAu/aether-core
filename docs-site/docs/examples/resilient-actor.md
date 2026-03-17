---
title: Resilient Actor with Observability
description: Complete example showing resilience patterns with OpenTelemetry tracing and Prometheus metrics
---

# Resilient Actor with Observability

This example demonstrates how to build a fault-tolerant actor with comprehensive observability using:
- **Circuit Breaker** - Prevents cascading failures
- **Retry** - Handles transient failures with exponential backoff
- **Rate Limiter** - Controls request rates
- **Bulkhead** - Isolates resources
- **Health Checks** - Kubernetes-compatible probes
- **OpenTelemetry** - Distributed tracing
- **Prometheus** - Metrics collection

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Resilient Actor                           │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │  Rate    │→ │ Bulkhead │→ │ Circuit  │→ │  Retry   │→ Call  │
│  │ Limiter  │  │          │  │ Breaker  │  │          │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│       ↓              ↓             ↓             ↓               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   OpenTelemetry Tracing                   │   │
│  └──────────────────────────────────────────────────────────┘   │
│       ↓              ↓             ↓             ↓               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   Prometheus Metrics                      │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Python SDK Example

```python
import asyncio
from aether_sdk import Actor, actor
from aether_sdk.resilience import (
    CircuitBreaker,
    CircuitBreakerConfig,
    RetryPolicy,
    RetryConfig,
    BackoffStrategy,
    RateLimiter,
    RateLimitConfig,
    RateLimitStrategy,
    Bulkhead,
    BulkheadConfig,
    ResilientExecutor,
    ResilienceInstrumentation,
    HealthChecker,
    HealthStatus,
    TRACING_AVAILABLE,
)

# Configure OpenTelemetry (optional but recommended)
if TRACING_AVAILABLE:
    from opentelemetry import trace
    from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor

    provider = TracerProvider()
    processor = BatchSpanProcessor(OTLPSpanExporter())
    provider.add_span_processor(processor)
    trace.set_tracer_provider(provider)


@actor("resilient-api-actor")
class ResilientApiActor(Actor):
    """An actor with comprehensive resilience and observability."""

    def __init__(self):
        super().__init__()

        # Initialize resilience patterns
        self.breaker = CircuitBreaker(
            CircuitBreakerConfig(
                name="external-api",
                failure_threshold=5,
                success_threshold=3,
                reset_timeout=30000,
            )
        )

        self.retry = RetryPolicy(
            RetryConfig(
                name="external-api",
                max_attempts=3,
                initial_delay=100,
                max_delay=10000,
                strategy=BackoffStrategy.ExponentialJitter,
            )
        )

        self.limiter = RateLimiter(
            RateLimitConfig(
                name="external-api",
                max_requests=100,
                window_ms=1000,
                strategy=RateLimitStrategy.SlidingWindow,
            )
        )

        self.bulkhead = Bulkhead(
            BulkheadConfig(
                name="external-api",
                max_concurrent=10,
                max_queue_size=5,
                queue_timeout=5000,
            )
        )

        # Combined executor
        self.executor = ResilientExecutor(
            breaker=self.breaker,
            retry=self.retry,
            rate_limiter=self.limiter,
            bulkhead=self.bulkhead,
        )

        # Health checker
        self.health = HealthChecker()
        self.health.add_check("api-connection", self._check_api_connection)

        # Tracing instrumentation
        self.instrumentation = ResilienceInstrumentation("resilient-actor")

    async def _check_api_connection(self):
        """Health check for external API."""
        try:
            # Simple connectivity check
            return {
                "name": "api-connection",
                "status": HealthStatus.Healthy if self.breaker.state.value == "closed"
                          else HealthStatus.Degraded,
                "timestamp": asyncio.get_event_loop().time(),
                "duration": 0,
            }
        except Exception as e:
            return {
                "name": "api-connection",
                "status": HealthStatus.Unhealthy,
                "message": str(e),
                "timestamp": asyncio.get_event_loop().time(),
                "duration": 0,
            }

    @actor.endpoint("call-external-api")
    async def call_external_api(self, request_id: str) -> dict:
        """Call external API with full resilience protection."""

        async def fetch_data():
            # Simulate API call
            await asyncio.sleep(0.1)
            return {"request_id": request_id, "data": "response"}

        # Execute with all resilience patterns
        result = await self.executor.execute(fetch_data)
        return result

    @actor.endpoint("health")
    async def health_check(self) -> dict:
        """Get health status."""
        report = await self.health.check()
        return {
            "status": report.status.value,
            "checks": {
                name: {
                    "status": check.status.value,
                    "message": check.message,
                }
                for name, check in report.checks.items()
            },
        }

    @actor.endpoint("metrics")
    async def get_metrics(self) -> dict:
        """Get resilience metrics."""
        return {
            "circuit_breaker": self.breaker.get_stats().__dict__,
            "bulkhead": self.bulkhead.get_stats().__dict__,
            "rate_limiter_remaining": self.limiter.try_acquire().remaining,
        }
```

## Go SDK Example

```go
package main

import (
    "context"
    "fmt"
    "time"

    "github.com/aether-sdk/aether-go/aether"
    "github.com/aether-sdk/aether-go/aether/resilience"
)

// ResilientApiActor demonstrates resilience patterns with observability
type ResilientApiActor struct {
    *aether.Actor

    breaker     *resilience.CircuitBreaker
    retry       *resilience.RetryPolicy
    limiter     *resilience.RateLimiter
    bulkhead    *resilience.Bulkhead
    executor    *resilience.ResilientExecutor
    health      *resilience.HealthChecker
}

func NewResilientApiActor() *ResilientApiActor {
    actor := &ResilientApiActor{
        Actor: aether.NewActor("resilient-api-actor"),
    }

    // Initialize resilience patterns
    actor.breaker = resilience.NewCircuitBreaker(resilience.CircuitBreakerConfig{
        Name:            "external-api",
        FailureThreshold: 5,
        SuccessThreshold: 3,
        ResetTimeout:     30 * time.Second,
    })

    actor.retry = resilience.NewRetryPolicy(resilience.RetryConfig{
        Name:          "external-api",
        MaxAttempts:   3,
        InitialDelay:  100 * time.Millisecond,
        MaxDelay:      10 * time.Second,
        Strategy:      resilience.ExponentialJitter,
    })

    actor.limiter = resilience.NewRateLimiter(resilience.RateLimitConfig{
        Name:       "external-api",
        MaxRequests: 100,
        WindowMs:   time.Second,
        Strategy:   resilience.SlidingWindow,
    })

    actor.bulkhead = resilience.NewBulkhead(resilience.BulkheadConfig{
        Name:           "external-api",
        MaxConcurrent:  10,
        MaxQueueSize:   5,
        QueueTimeout:   5 * time.Second,
    })

    // Combined executor
    actor.executor = resilience.NewResilientExecutor(
        actor.breaker,
        actor.retry,
        actor.limiter,
        actor.bulkhead,
    )

    // Health checker
    actor.health = resilience.NewHealthChecker()
    actor.health.AddCheck("api-connection", actor.checkAPIConnection)

    return actor
}

func (a *ResilientApiActor) CallExternalAPI(ctx context.Context, requestID string) (map[string]interface{}, error) {
    fetch := func() (interface{}, error) {
        // Simulate API call
        time.Sleep(100 * time.Millisecond)
        return map[string]interface{}{
            "request_id": requestID,
            "data":       "response",
        }, nil
    }

    return a.executor.Execute(ctx, fetch)
}

func (a *ResilientApiActor) checkAPIConnection(ctx context.Context) resilience.HealthCheckResult {
    stats := a.breaker.Stats()

    status := resilience.Healthy
    if stats.State == resilience.Open {
        status = resilience.Unhealthy
    } else if stats.State == resilience.HalfOpen {
        status = resilience.Degraded
    }

    return resilience.HealthCheckResult{
        Name:      "api-connection",
        Status:    status,
        Timestamp: time.Now(),
    }
}

func main() {
    actor := NewResilientApiActor()

    // Start health check endpoint
    http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
        report := actor.health.Check(r.Context())
        status := http.StatusOK
        if report.Status == resilience.Unhealthy {
            status = http.StatusServiceUnavailable
        }
        json.NewEncoder(w).Encode(report)
    })

    http.ListenAndServe(":8080", nil)
}
```

## JavaScript SDK Example

```typescript
import { Actor, actor } from 'aether-sdk';
import {
  CircuitBreaker,
  RetryPolicy,
  RateLimiter,
  Bulkhead,
  ResilientExecutor,
  HealthChecker,
  HealthStatus,
  BackoffStrategy,
  RateLimitStrategy,
  withTracing,
} from 'aether-sdk/resilience';

@actor('resilient-api-actor')
export class ResilientApiActor extends Actor {
  private breaker: CircuitBreaker;
  private retry: RetryPolicy;
  private limiter: RateLimiter;
  private bulkhead: Bulkhead;
  private executor: ResilientExecutor;
  private health: HealthChecker;

  constructor() {
    super();

    // Initialize resilience patterns
    this.breaker = new CircuitBreaker({
      name: 'external-api',
      failureThreshold: 5,
      successThreshold: 3,
      resetTimeout: 30000,
    });

    this.retry = new RetryPolicy({
      name: 'external-api',
      maxAttempts: 3,
      initialDelay: 100,
      maxDelay: 10000,
      strategy: BackoffStrategy.ExponentialJitter,
    });

    this.limiter = new RateLimiter({
      name: 'external-api',
      maxRequests: 100,
      windowMs: 1000,
      strategy: RateLimitStrategy.SlidingWindow,
    });

    this.bulkhead = new Bulkhead({
      name: 'external-api',
      maxConcurrent: 10,
      maxQueueSize: 5,
      queueTimeout: 5000,
    });

    // Combined executor
    this.executor = new ResilientExecutor({
      breaker: this.breaker,
      retry: this.retry,
      rateLimiter: this.limiter,
      bulkhead: this.bulkhead,
      name: 'resilient-api',
    });

    // Health checker
    this.health = new HealthChecker();
    this.health.addCheck('api-connection', this.checkAPIConnection.bind(this));
    this.health.addCheck('memory', memoryHealthCheck(90));
    this.health.start();
  }

  private async checkAPIConnection() {
    const stats = this.breaker.getStats();

    return {
      name: 'api-connection',
      status: stats.state === 'closed'
        ? HealthStatus.Healthy
        : stats.state === 'half-open'
          ? HealthStatus.Degraded
          : HealthStatus.Unhealthy,
      timestamp: Date.now(),
      duration: 0,
    };
  }

  @actor.endpoint('call-external-api')
  async callExternalAPI(requestId: string): Promise<Record<string, unknown>> {
    const fetch = async () => {
      // Simulate API call
      await new Promise(resolve => setTimeout(resolve, 100));
      return { requestId, data: 'response' };
    };

    // Execute with all resilience patterns
    return this.executor.execute(fetch);
  }

  @actor.endpoint('health')
  async healthCheck(): Promise<Record<string, unknown>> {
    const report = await this.health.check();
    return {
      status: report.status,
      checks: Object.fromEntries(
        Object.entries(report.checks).map(([name, check]) => [
          name,
          { status: check.status, message: check.message },
        ])
      ),
    };
  }

  @actor.endpoint('metrics')
  async getMetrics(): Promise<Record<string, unknown>> {
    return {
      circuitBreaker: this.breaker.getStats(),
      bulkhead: this.bulkhead.getStats(),
      rateLimiter: this.limiter.tryAcquire(),
    };
  }
}
```

## Prometheus Metrics

The resilience module exports Prometheus-compatible metrics:

```promql
# Circuit Breaker Metrics
aether_circuit_breaker_state{ name="external-api" } 1
aether_circuit_breaker_failures_total{ name="external-api" } 5
aether_circuit_breaker_successes_total{ name="external-api" } 100

# Retry Metrics
aether_retry_attempts_total{ name="external-api", result="success" } 95
aether_retry_attempts_total{ name="external-api", result="exhausted" } 5

# Rate Limiter Metrics
aether_rate_limiter_requests_total{ name="external-api", allowed="true" } 1000
aether_rate_limiter_requests_total{ name="external-api", allowed="false" } 50

# Bulkhead Metrics
aether_bulkhead_active{ name="external-api" } 5
aether_bulkhead_rejected_total{ name="external-api" } 10
```

## Grafana Dashboard

Import this dashboard configuration for visualizing resilience metrics:

```json
{
  "dashboard": {
    "title": "Aether Resilience Dashboard",
    "panels": [
      {
        "title": "Circuit Breaker State",
        "type": "stat",
        "targets": [
          {
            "expr": "aether_circuit_breaker_state",
            "legendFormat": "{{name}}"
          }
        ]
      },
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(aether_rate_limiter_requests_total[5m])",
            "legendFormat": "{{name}} - {{allowed}}"
          }
        ]
      },
      {
        "title": "Retry Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(aether_retry_attempts_total[5m])",
            "legendFormat": "{{name}} - {{result}}"
          }
        ]
      },
      {
        "title": "Bulkhead Utilization",
        "type": "gauge",
        "targets": [
          {
            "expr": "aether_bulkhead_active / aether_bulkhead_max_concurrent * 100",
            "legendFormat": "{{name}}"
          }
        ]
      }
    ]
  }
}
```

## Best Practices

### 1. Order of Operations

Apply resilience patterns in this order:
1. **Rate Limiter** - First line of defense, prevent overload
2. **Bulkhead** - Isolate resources before processing
3. **Circuit Breaker** - Check downstream health
4. **Retry** - Handle transient failures last

### 2. Configuration Tuning

| Pattern | Recommended Defaults | When to Adjust |
|---------|---------------------|----------------|
| Circuit Breaker | 5 failures, 30s reset | Lower for critical paths |
| Retry | 3 attempts, exp backoff | Increase for idempotent ops |
| Rate Limiter | 100 req/s | Based on downstream capacity |
| Bulkhead | 10 concurrent | Based on resource limits |

### 3. Monitoring

Monitor these key indicators:
- Circuit breaker state transitions (alert on prolonged open state)
- Retry exhaustion rate (should be < 1%)
- Rate limit rejections (indicates traffic spikes)
- Bulkhead queue depth (indicates bottlenecks)

### 4. Health Check Endpoints

Expose health checks for Kubernetes:
```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8080
readinessProbe:
  httpGet:
    path: /health/ready
    port: 8080
startupProbe:
  httpGet:
    path: /health/startup
    port: 8080
```
