package resilience

import (
	"context"
	"errors"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestRetryPolicy_NewWithZeroConfig(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{})
	if rp.config.MaxAttempts != 3 {
		t.Errorf("expected default max attempts 3, got %d", rp.config.MaxAttempts)
	}
	if rp.config.BaseDelay != 100*time.Millisecond {
		t.Errorf("expected default base delay 100ms, got %v", rp.config.BaseDelay)
	}
}

func TestRetryPolicy_DefaultConfig(t *testing.T) {
	cfg := DefaultRetryConfig()
	if cfg.MaxAttempts != 3 {
		t.Errorf("expected 3, got %d", cfg.MaxAttempts)
	}
	if cfg.Backoff != BackoffExponentialJitter {
		t.Errorf("expected exponential jitter, got %v", cfg.Backoff)
	}
}

func TestRetryPolicy_SuccessFirstTry(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{MaxAttempts: 3, Backoff: BackoffFixed, BaseDelay: time.Millisecond})

	result, err := rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}

	stats := rp.GetStats()
	if stats.SuccessfulCalls != 1 {
		t.Errorf("expected 1 successful call, got %d", stats.SuccessfulCalls)
	}
}

func TestRetryPolicy_SuccessAfterRetry(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 3,
		Backoff:     BackoffFixed,
		BaseDelay:   5 * time.Millisecond,
	})

	var attempts int32
	result, err := rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		a := atomic.AddInt32(&attempts, 1)
		if a < 3 {
			return nil, errors.New("transient error")
		}
		return "recovered", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "recovered" {
		t.Errorf("expected 'recovered', got %v", result)
	}
	if atomic.LoadInt32(&attempts) != 3 {
		t.Errorf("expected 3 attempts, got %d", atomic.LoadInt32(&attempts))
	}
}

func TestRetryPolicy_Exhausted(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 2,
		Backoff:     BackoffFixed,
		BaseDelay:   5 * time.Millisecond,
	})

	_, err := rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("always fails")
	})
	if err == nil {
		t.Fatal("expected error")
	}

	var retryErr *RetryExhaustedError
	if !errors.As(err, &retryErr) {
		t.Errorf("expected RetryExhaustedError, got %T: %v", err, err)
	}
	if retryErr.Attempts != 2 {
		t.Errorf("expected 2 attempts, got %d", retryErr.Attempts)
	}
}

func TestRetryExhaustedError_Unwrap(t *testing.T) {
	inner := errors.New("root cause")
	retryErr := &RetryExhaustedError{LastErr: inner, Attempts: 3}
	if !errors.Is(retryErr, inner) {
		t.Error("errors.Is should work through Unwrap")
	}
}

func TestRetryExhaustedError_Error(t *testing.T) {
	err := &RetryExhaustedError{Attempts: 5}
	if !strings.Contains(err.Error(), "exhausted") {
		t.Errorf("error message should contain 'exhausted', got %q", err.Error())
	}
}

func TestRetryPolicy_NonRetryableError(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 5,
		Backoff:     BackoffFixed,
		BaseDelay:   time.Millisecond,
	})

	var attempts int32
	_, err := rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		atomic.AddInt32(&attempts, 1)
		return nil, errors.New("permanent error")
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if atomic.LoadInt32(&attempts) != 1 {
		t.Errorf("non-retryable error should only attempt once, got %d", atomic.LoadInt32(&attempts))
	}
}

func TestRetryPolicy_CustomIsRetryable(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 5,
		Backoff:     BackoffFixed,
		BaseDelay:   time.Millisecond,
		IsRetryable: func(err error, attempt int) bool {
			return strings.Contains(err.Error(), "retry_me")
		},
	})

	var attempts int32
	_, _ = rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		atomic.AddInt32(&attempts, 1)
		return nil, errors.New("do_not_retry")
	})
	if atomic.LoadInt32(&attempts) != 1 {
		t.Errorf("custom IsRetryable should reject non-matching, got %d attempts", atomic.LoadInt32(&attempts))
	}
}

func TestRetryPolicy_OnRetryCallback(t *testing.T) {
	var retryCount int32
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 3,
		Backoff:     BackoffFixed,
		BaseDelay:   time.Millisecond,
		OnRetry: func(err error, attempt int, delay time.Duration) {
			atomic.AddInt32(&retryCount, 1)
		},
	})

	_, _ = rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("timeout: retry this")
	})

	if atomic.LoadInt32(&retryCount) == 0 {
		t.Error("OnRetry callback should have been called")
	}
}

func TestRetryPolicy_OnExhaustedCallback(t *testing.T) {
	var exhausted bool
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 1,
		Backoff:     BackoffFixed,
		BaseDelay:   time.Millisecond,
		OnExhausted: func(err error, attempt int) {
			exhausted = true
		},
	})

	_, _ = rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("timeout: retry")
	})

	if !exhausted {
		t.Error("OnExhausted callback should have been called")
	}
}

func TestRetryPolicy_ContextCanceled(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 10,
		Backoff:     BackoffFixed,
		BaseDelay:   time.Second,
	})

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := rp.Execute(ctx, func(ctx context.Context) (any, error) {
		return nil, errors.New("timeout: transient")
	})
	if err == nil {
		t.Error("expected error when context canceled")
	}
}

func TestRetryPolicy_ResetStats(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{MaxAttempts: 3, Backoff: BackoffFixed, BaseDelay: time.Millisecond})

	_, _ = rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("timeout: fail")
	})

	rp.ResetStats()
	stats := rp.GetStats()
	if stats.TotalAttempts != 0 {
		t.Errorf("expected 0 total attempts after reset, got %d", stats.TotalAttempts)
	}
	if stats.FailedCalls != 0 {
		t.Errorf("expected 0 failed calls after reset, got %d", stats.FailedCalls)
	}
}

func TestRetryPolicy_GetStats(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{MaxAttempts: 3, Backoff: BackoffFixed, BaseDelay: time.Millisecond})

	rp.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})

	stats := rp.GetStats()
	if stats.TotalAttempts != 1 {
		t.Errorf("expected 1 total attempt, got %d", stats.TotalAttempts)
	}
	if stats.SuccessfulCalls != 1 {
		t.Errorf("expected 1 successful call, got %d", stats.SuccessfulCalls)
	}
}

func TestBackoff_Fixed(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		MaxAttempts: 3,
		Backoff:     BackoffFixed,
		BaseDelay:   50 * time.Millisecond,
		MaxDelay:    10 * time.Second,
	})
	delay1 := rp.calculateDelay(1)
	delay2 := rp.calculateDelay(2)
	if delay1 != 50*time.Millisecond {
		t.Errorf("expected 50ms, got %v", delay1)
	}
	if delay2 != 50*time.Millisecond {
		t.Errorf("expected 50ms, got %v", delay2)
	}
}

func TestBackoff_Linear(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		Backoff:   BackoffLinear,
		BaseDelay: 10 * time.Millisecond,
	})
	d1 := rp.calculateDelay(1)
	d2 := rp.calculateDelay(2)
	d3 := rp.calculateDelay(3)
	if d1 != 10*time.Millisecond {
		t.Errorf("expected 10ms, got %v", d1)
	}
	if d2 != 20*time.Millisecond {
		t.Errorf("expected 20ms, got %v", d2)
	}
	if d3 != 30*time.Millisecond {
		t.Errorf("expected 30ms, got %v", d3)
	}
}

func TestBackoff_Exponential(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		Backoff:   BackoffExponential,
		BaseDelay: 10 * time.Millisecond,
		MaxDelay:  10 * time.Second,
	})
	d1 := rp.calculateDelay(1)
	d2 := rp.calculateDelay(2)
	d3 := rp.calculateDelay(3)
	if d1 != 10*time.Millisecond {
		t.Errorf("expected 10ms, got %v", d1)
	}
	if d2 != 20*time.Millisecond {
		t.Errorf("expected 20ms, got %v", d2)
	}
	if d3 != 40*time.Millisecond {
		t.Errorf("expected 40ms, got %v", d3)
	}
}

func TestBackoff_MaxDelayCap(t *testing.T) {
	rp := NewRetryPolicy(RetryConfig{
		Backoff:   BackoffExponential,
		BaseDelay: 100 * time.Millisecond,
		MaxDelay:  200 * time.Millisecond,
	})
	d := rp.calculateDelay(10)
	if d > 200*time.Millisecond {
		t.Errorf("delay should be capped at max, got %v", d)
	}
}

func TestNetworkRetryPolicy(t *testing.T) {
	rp := NetworkRetryPolicy()
	if rp.config.MaxAttempts != 3 {
		t.Errorf("expected 3 attempts, got %d", rp.config.MaxAttempts)
	}
}

func TestDatabaseRetryPolicy(t *testing.T) {
	rp := DatabaseRetryPolicy()
	if rp.config.MaxAttempts != 5 {
		t.Errorf("expected 5 attempts, got %d", rp.config.MaxAttempts)
	}
}

func TestAggressiveRetryPolicy(t *testing.T) {
	rp := AggressiveRetryPolicy()
	if rp.config.MaxAttempts != 10 {
		t.Errorf("expected 10 attempts, got %d", rp.config.MaxAttempts)
	}
}

func TestConservativeRetryPolicy(t *testing.T) {
	rp := ConservativeRetryPolicy()
	if rp.config.MaxAttempts != 2 {
		t.Errorf("expected 2 attempts, got %d", rp.config.MaxAttempts)
	}
}
