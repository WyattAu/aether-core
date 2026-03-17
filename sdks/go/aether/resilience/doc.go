// Package resilience provides patterns for building robust, fault-tolerant actor systems.
//
// The package includes:
//   - Circuit Breaker: Prevents cascading failures
//   - Retry: Handles transient failures with backoff
//   - Rate Limiter: Controls request rates
//   - Health Check: Kubernetes-compatible probes
//   - Bulkhead: Resource isolation
//   - ResilientExecutor: Combined pattern execution
//
// # Example Usage
//
//	breaker := resilience.NewCircuitBreaker(resilience.CircuitBreakerConfig{
//	    FailureThreshold: 5,
//	    Timeout:          30 * time.Second,
//	})
//
//	result, err := breaker.Execute(ctx, func(ctx context.Context) (any, error) {
//	    return someOperation(ctx)
//	})
package resilience
