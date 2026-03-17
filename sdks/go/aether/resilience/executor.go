package resilience

import (
	"context"
	"time"
)

// ResilientExecutor combines multiple resilience patterns.
//
// Order of operations:
// 1. Rate limiting (check if request is allowed)
// 2. Bulkhead (check capacity)
// 3. Circuit breaker (check if service is healthy)
// 4. Retry (handle transient failures)
type ResilientExecutor struct {
	breaker     *CircuitBreaker
	retry       *RetryPolicy
	rateLimiter *RateLimiter
	bulkhead    *Bulkhead
}

// ExecutorOption is a functional option for configuring the executor.
type ExecutorOption func(*ResilientExecutor)

// WithCircuitBreaker adds a circuit breaker to the executor.
func WithCircuitBreaker(breaker *CircuitBreaker) ExecutorOption {
	return func(e *ResilientExecutor) {
		e.breaker = breaker
	}
}

// WithRetry adds a retry policy to the executor.
func WithRetry(retry *RetryPolicy) ExecutorOption {
	return func(e *ResilientExecutor) {
		e.retry = retry
	}
}

// WithRateLimiter adds a rate limiter to the executor.
func WithRateLimiter(limiter *RateLimiter) ExecutorOption {
	return func(e *ResilientExecutor) {
		e.rateLimiter = limiter
	}
}

// WithBulkhead adds a bulkhead to the executor.
func WithBulkhead(bulkhead *Bulkhead) ExecutorOption {
	return func(e *ResilientExecutor) {
		e.bulkhead = bulkhead
	}
}

// NewResilientExecutor creates a new executor with the given options.
func NewResilientExecutor(opts ...ExecutorOption) *ResilientExecutor {
	e := &ResilientExecutor{}
	for _, opt := range opts {
		opt(e)
	}
	return e
}

// Execute runs the function with all configured resilience patterns.
func (e *ResilientExecutor) Execute(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	// Apply rate limiting first
	if e.rateLimiter != nil {
		if err := e.rateLimiter.Acquire(ctx, 5*time.Second); err != nil {
			return nil, err
		}
	}

	// Apply bulkhead
	if e.bulkhead != nil {
		return e.bulkhead.Execute(ctx, func(ctx context.Context) (any, error) {
			return e.executeWithRetry(ctx, fn)
		})
	}

	return e.executeWithRetry(ctx, fn)
}

// executeWithRetry applies retry logic.
func (e *ResilientExecutor) executeWithRetry(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	// Apply circuit breaker
	if e.breaker != nil {
		return e.breaker.Execute(ctx, func(ctx context.Context) (any, error) {
			return e.executeWithRetryInternal(ctx, fn)
		})
	}

	return e.executeWithRetryInternal(ctx, fn)
}

// executeWithRetryInternal applies retry without circuit breaker.
func (e *ResilientExecutor) executeWithRetryInternal(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	if e.retry != nil {
		result, err := e.retry.Execute(ctx, fn)
		if err != nil {
			return nil, err
		}
		return result, nil
	}

	return fn(ctx)
}

// ============================================
// Executor Builder
// ============================================

// ExecutorBuilder provides a fluent interface for building executors.
type ExecutorBuilder struct {
	breaker     *CircuitBreaker
	retry       *RetryPolicy
	rateLimiter *RateLimiter
	bulkhead    *Bulkhead
}

// NewExecutorBuilder creates a new builder.
func NewExecutorBuilder() *ExecutorBuilder {
	return &ExecutorBuilder{}
}

// CircuitBreaker sets the circuit breaker.
func (b *ExecutorBuilder) CircuitBreaker(config CircuitBreakerConfig) *ExecutorBuilder {
	b.breaker = NewCircuitBreaker(config)
	return b
}

// Retry sets the retry policy.
func (b *ExecutorBuilder) Retry(config RetryConfig) *ExecutorBuilder {
	b.retry = NewRetryPolicy(config)
	return b
}

// RateLimiter sets the rate limiter.
func (b *ExecutorBuilder) RateLimiter(config RateLimitConfig) *ExecutorBuilder {
	b.rateLimiter = NewRateLimiter(config)
	return b
}

// Bulkhead sets the bulkhead.
func (b *ExecutorBuilder) Bulkhead(config BulkheadConfig) *ExecutorBuilder {
	b.bulkhead = NewBulkhead(config)
	return b
}

// Build creates the executor.
func (b *ExecutorBuilder) Build() *ResilientExecutor {
	return NewResilientExecutor(
		WithCircuitBreaker(b.breaker),
		WithRetry(b.retry),
		WithRateLimiter(b.rateLimiter),
		WithBulkhead(b.bulkhead),
	)
}

// ============================================
// Predefined Executors
// ============================================

// DefaultExecutor creates an executor with sensible defaults.
func DefaultExecutor() *ResilientExecutor {
	return NewExecutorBuilder().
		CircuitBreaker(DefaultCircuitBreakerConfig()).
		Retry(DefaultRetryConfig()).
		RateLimiter(DefaultRateLimitConfig()).
		Bulkhead(DefaultBulkheadConfig()).
		Build()
}

// APIExecutor creates an executor for API calls.
func APIExecutor() *ResilientExecutor {
	return NewExecutorBuilder().
		CircuitBreaker(CircuitBreakerConfig{
			FailureThreshold: 5,
			SuccessThreshold: 3,
			Timeout:          30 * time.Second,
		}).
		Retry(RetryConfig{
			MaxAttempts:  3,
			Backoff:      BackoffExponentialJitter,
			BaseDelay:    100 * time.Millisecond,
			MaxDelay:     5 * time.Second,
			Multiplier:   2.0,
			JitterFactor: 0.1,
		}).
		RateLimiter(RateLimitConfig{
			RequestsPerSecond: 100,
			BurstSize:         200,
			Strategy:          StrategyTokenBucket,
		}).
		Bulkhead(BulkheadConfig{
			MaxConcurrent: 50,
			MaxQueued:     100,
		}).
		Build()
}

// DatabaseExecutor creates an executor for database operations.
func DatabaseExecutor() *ResilientExecutor {
	return NewExecutorBuilder().
		CircuitBreaker(CircuitBreakerConfig{
			FailureThreshold: 3,
			SuccessThreshold: 2,
			Timeout:          60 * time.Second,
		}).
		Retry(RetryConfig{
			MaxAttempts: 5,
			Backoff:     BackoffExponential,
			BaseDelay:   50 * time.Millisecond,
			MaxDelay:    2 * time.Second,
			Multiplier:  2.0,
		}).
		RateLimiter(RateLimitConfig{
			RequestsPerSecond: 50,
			BurstSize:         100,
			Strategy:          StrategyTokenBucket,
		}).
		Bulkhead(BulkheadConfig{
			MaxConcurrent: 10,
			MaxQueued:     50,
			Timeout:       30 * time.Second,
		}).
		Build()
}

// CriticalExecutor creates an executor for critical operations.
func CriticalExecutor() *ResilientExecutor {
	return NewExecutorBuilder().
		CircuitBreaker(CircuitBreakerConfig{
			FailureThreshold: 2,
			SuccessThreshold: 5,
			Timeout:          10 * time.Second,
		}).
		Retry(RetryConfig{
			MaxAttempts:   3,
			Backoff:       BackoffExponential,
			BaseDelay:     200 * time.Millisecond,
			MaxDelay:      10 * time.Second,
			Multiplier:    2.0,
			JitterFactor:  0.05,
		}).
		RateLimiter(RateLimitConfig{
			RequestsPerSecond: 20,
			BurstSize:         30,
			Strategy:          StrategySlidingWindow,
		}).
		Bulkhead(BulkheadConfig{
			MaxConcurrent: 5,
			MaxQueued:     20,
			Timeout:       10 * time.Second,
		}).
		Build()
}
