package resilience

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestResilientExecutor_WithNoPatterns(t *testing.T) {
	executor := NewResilientExecutor()

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "direct", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "direct" {
		t.Errorf("expected 'direct', got %v", result)
	}
}

func TestResilientExecutor_WithRetryOnly(t *testing.T) {
	executor := NewResilientExecutor(
		WithRetry(NewRetryPolicy(RetryConfig{
			MaxAttempts: 3,
			Backoff:     BackoffFixed,
			BaseDelay:   5 * time.Millisecond,
		})),
	)

	var attempts int
	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		attempts++
		if attempts < 2 {
			return nil, errors.New("timeout: transient")
		}
		return "retried", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "retried" {
		t.Errorf("expected 'retried', got %v", result)
	}
}

func TestResilientExecutor_WithCircuitBreakerOnly(t *testing.T) {
	cb := NewCircuitBreaker(CircuitBreakerConfig{
		FailureThreshold: 2,
		Timeout:          100 * time.Millisecond,
	})
	executor := NewResilientExecutor(WithCircuitBreaker(cb))

	_, _ = executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})
	_, _ = executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})

	if !cb.IsOpen() {
		t.Fatal("expected open circuit")
	}

	_, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "nope", nil
	})
	if err == nil {
		t.Error("expected error when circuit is open")
	}
}

func TestResilientExecutor_WithRateLimiterOnly(t *testing.T) {
	executor := NewResilientExecutor(
		WithRateLimiter(NewRateLimiter(RateLimitConfig{
			RequestsPerSecond: 100,
			BurstSize:         100,
			Strategy:          StrategyTokenBucket,
		})),
	)

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}
}

func TestResilientExecutor_WithBulkheadOnly(t *testing.T) {
	executor := NewResilientExecutor(
		WithBulkhead(NewBulkhead(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 10})),
	)

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}
}

func TestResilientExecutor_WithAllPatterns(t *testing.T) {
	executor := NewResilientExecutor(
		WithCircuitBreaker(NewCircuitBreaker(DefaultCircuitBreakerConfig())),
		WithRetry(NewRetryPolicy(RetryConfig{
			MaxAttempts: 3,
			Backoff:     BackoffFixed,
			BaseDelay:   5 * time.Millisecond,
		})),
		WithRateLimiter(NewRateLimiter(RateLimitConfig{
			RequestsPerSecond: 100,
			BurstSize:         100,
			Strategy:          StrategyTokenBucket,
		})),
		WithBulkhead(NewBulkhead(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 10})),
	)

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "success", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "success" {
		t.Errorf("expected 'success', got %v", result)
	}
}

func TestExecutorBuilder_Build(t *testing.T) {
	executor := NewExecutorBuilder().
		CircuitBreaker(DefaultCircuitBreakerConfig()).
		Retry(DefaultRetryConfig()).
		RateLimiter(DefaultRateLimitConfig()).
		Bulkhead(DefaultBulkheadConfig()).
		Build()

	if executor == nil {
		t.Fatal("expected non-nil executor")
	}

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}
}

func TestExecutorBuilder_EmptyBuild(t *testing.T) {
	executor := NewExecutorBuilder().Build()
	if executor == nil {
		t.Fatal("expected non-nil executor")
	}

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "direct", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "direct" {
		t.Errorf("expected 'direct', got %v", result)
	}
}

func TestDefaultExecutor(t *testing.T) {
	executor := DefaultExecutor()
	if executor == nil {
		t.Fatal("expected non-nil executor")
	}

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}
}

func TestAPIExecutor(t *testing.T) {
	executor := APIExecutor()
	if executor == nil {
		t.Fatal("expected non-nil executor")
	}

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "api-ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "api-ok" {
		t.Errorf("expected 'api-ok', got %v", result)
	}
}

func TestDatabaseExecutor(t *testing.T) {
	executor := DatabaseExecutor()
	if executor == nil {
		t.Fatal("expected non-nil executor")
	}

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "db-ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "db-ok" {
		t.Errorf("expected 'db-ok', got %v", result)
	}
}

func TestCriticalExecutor(t *testing.T) {
	executor := CriticalExecutor()
	if executor == nil {
		t.Fatal("expected non-nil executor")
	}

	result, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "critical-ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "critical-ok" {
		t.Errorf("expected 'critical-ok', got %v", result)
	}
}

func TestResilientExecutor_ErrorPropagation(t *testing.T) {
	executor := NewResilientExecutor(
		WithRetry(NewRetryPolicy(RetryConfig{
			MaxAttempts: 1,
			Backoff:     BackoffFixed,
			BaseDelay:   time.Millisecond,
		})),
	)

	_, err := executor.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("permanent failure")
	})
	if err == nil {
		t.Error("expected error propagation")
	}
}

func TestExecutorOption_WithCircuitBreaker(t *testing.T) {
	cb := NewCircuitBreaker(DefaultCircuitBreakerConfig())
	e := &ResilientExecutor{}
	WithCircuitBreaker(cb)(e)
	if e.breaker != cb {
		t.Error("circuit breaker not set")
	}
}

func TestExecutorOption_WithRetry(t *testing.T) {
	rp := NewRetryPolicy(DefaultRetryConfig())
	e := &ResilientExecutor{}
	WithRetry(rp)(e)
	if e.retry != rp {
		t.Error("retry policy not set")
	}
}

func TestExecutorOption_WithRateLimiter(t *testing.T) {
	rl := NewRateLimiter(DefaultRateLimitConfig())
	e := &ResilientExecutor{}
	WithRateLimiter(rl)(e)
	if e.rateLimiter != rl {
		t.Error("rate limiter not set")
	}
}

func TestExecutorOption_WithBulkhead(t *testing.T) {
	bh := NewBulkhead(DefaultBulkheadConfig())
	e := &ResilientExecutor{}
	WithBulkhead(bh)(e)
	if e.bulkhead != bh {
		t.Error("bulkhead not set")
	}
}
