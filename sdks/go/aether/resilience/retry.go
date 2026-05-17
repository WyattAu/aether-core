package resilience

import (
	"context"
	"errors"
	"math/rand"
	"strings"
	"time"
)

// BackoffStrategy defines how delays are calculated between retries.
type BackoffStrategy int

const (
	// BackoffFixed uses a constant delay.
	BackoffFixed BackoffStrategy = iota
	// BackoffLinear increases delay linearly with each attempt.
	BackoffLinear
	// BackoffExponential doubles the delay with each attempt.
	BackoffExponential
	// BackoffExponentialJitter doubles delay with random jitter.
	BackoffExponentialJitter
)

// RetryConfig holds configuration for the retry policy.
type RetryConfig struct {
	// MaxAttempts is the maximum number of retry attempts.
	MaxAttempts int
	// Backoff strategy to use.
	Backoff BackoffStrategy
	// BaseDelay is the initial delay.
	BaseDelay time.Duration
	// MaxDelay is the maximum delay.
	MaxDelay time.Duration
	// Multiplier for exponential backoff.
	Multiplier float64
	// JitterFactor for adding randomness (0-1).
	JitterFactor float64
	// IsRetryable determines if an error is retryable.
	IsRetryable func(err error, attempt int) bool
	// OnRetry is called before each retry.
	OnRetry func(err error, attempt int, delay time.Duration)
	// OnExhausted is called when all retries are exhausted.
	OnExhausted func(err error, attempt int)
}

// DefaultRetryConfig returns the default retry configuration.
func DefaultRetryConfig() RetryConfig {
	return RetryConfig{
		MaxAttempts:   3,
		Backoff:       BackoffExponentialJitter,
		BaseDelay:     100 * time.Millisecond,
		MaxDelay:      30 * time.Second,
		Multiplier:    2.0,
		JitterFactor:  0.1,
		IsRetryable:   nil,
		OnRetry:       nil,
		OnExhausted:   nil,
	}
}

// RetryStats holds statistics for the retry policy.
type RetryStats struct {
	TotalAttempts     int64
	SuccessfulCalls   int64
	FailedCalls       int64
	RetriedCalls      int64
	ExhaustedCalls    int64
	TotalRetryDelay   time.Duration
}

// RetryResult holds the result of a retry operation.
type RetryResult[T any] struct {
	Result       T
	Attempts     int
	TotalDelay   time.Duration
}

// RetryExhaustedError is returned when all retries are exhausted.
type RetryExhaustedError struct {
	LastErr     error
	Attempts    int
	TotalDelay  time.Duration
}

func (e *RetryExhaustedError) Error() string {
	return "all retry attempts exhausted"
}

func (e *RetryExhaustedError) Unwrap() error {
	return e.LastErr
}

// RetryPolicy implements retry with configurable backoff.
type RetryPolicy struct {
	config RetryConfig
	stats  RetryStats
}

// NewRetryPolicy creates a new retry policy.
func NewRetryPolicy(config RetryConfig) *RetryPolicy {
	if config.MaxAttempts == 0 {
		config.MaxAttempts = 3
	}
	if config.BaseDelay == 0 {
		config.BaseDelay = 100 * time.Millisecond
	}
	if config.MaxDelay == 0 {
		config.MaxDelay = 30 * time.Second
	}
	if config.Multiplier == 0 {
		config.Multiplier = 2.0
	}

	return &RetryPolicy{
		config: config,
	}
}

// Execute runs the given function with retry logic.
func (r *RetryPolicy) Execute(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	attempt := 0
	var totalDelay time.Duration
	var lastErr error

	for attempt < r.config.MaxAttempts {
		attempt++
		r.stats.TotalAttempts++

		result, err := fn(ctx)
		if err == nil {
			r.stats.SuccessfulCalls++
			if attempt > 1 {
				r.stats.RetriedCalls++
			}
			return result, nil
		}

		lastErr = err
		r.stats.FailedCalls++

		// Check if we should retry
		isRetryable := r.isRetryable(err, attempt)
		if attempt >= r.config.MaxAttempts || !isRetryable {
			break
		}

		// Calculate delay
		delay := r.calculateDelay(attempt)
		totalDelay += delay
		r.stats.TotalRetryDelay += delay

		// Notify callback
		if r.config.OnRetry != nil {
			r.config.OnRetry(err, attempt, delay)
		}

		// Wait before retry
		select {
		case <-time.After(delay):
		case <-ctx.Done():
			return nil, ctx.Err()
		}
	}

	// All retries exhausted
	r.stats.ExhaustedCalls++
	if r.config.OnExhausted != nil {
		r.config.OnExhausted(lastErr, attempt)
	}

	return nil, &RetryExhaustedError{
		LastErr:    lastErr,
		Attempts:   attempt,
		TotalDelay: totalDelay,
	}
}

// GetStats returns current statistics.
func (r *RetryPolicy) GetStats() RetryStats {
	return RetryStats{
		TotalAttempts:   r.stats.TotalAttempts,
		SuccessfulCalls: r.stats.SuccessfulCalls,
		FailedCalls:     r.stats.FailedCalls,
		RetriedCalls:    r.stats.RetriedCalls,
		ExhaustedCalls:  r.stats.ExhaustedCalls,
		TotalRetryDelay: r.stats.TotalRetryDelay,
	}
}

// ResetStats resets statistics.
func (r *RetryPolicy) ResetStats() {
	r.stats = RetryStats{}
}

func (r *RetryPolicy) isRetryable(err error, attempt int) bool {
	if r.config.IsRetryable != nil {
		return r.config.IsRetryable(err, attempt)
	}
	return r.isRetryableDefault(err)
}

func (r *RetryPolicy) isRetryableDefault(err error) bool {
	// Check for transient error messages
	msg := strings.ToLower(err.Error())
	transient := []string{
		"connection reset",
		"timeout",
		"temporary",
		"transient",
		"unavailable",
		"network",
		"eof",
	}

	for _, t := range transient {
		if strings.Contains(msg, t) {
			return true
		}
	}

	// Check for context errors
	if errors.Is(err, context.DeadlineExceeded) {
		return true
	}

	return false
}

func (r *RetryPolicy) calculateDelay(attempt int) time.Duration {
	var delay time.Duration

	switch r.config.Backoff {
	case BackoffFixed:
		delay = r.config.BaseDelay
	case BackoffLinear:
		delay = r.config.BaseDelay * time.Duration(attempt)
	case BackoffExponential:
		delay = r.config.BaseDelay * time.Duration(1<<uint(attempt-1))
	case BackoffExponentialJitter:
		baseDelay := r.config.BaseDelay * time.Duration(1<<uint(attempt-1))
		delay = r.addJitter(baseDelay)
	default:
		delay = r.config.BaseDelay
	}

	if delay > r.config.MaxDelay {
		delay = r.config.MaxDelay
	}

	return delay
}

func (r *RetryPolicy) addJitter(delay time.Duration) time.Duration {
	if r.config.JitterFactor <= 0 {
		return delay
	}

	jitter := float64(delay) * r.config.JitterFactor
	randomJitter := (rand.Float64()*2 - 1) * jitter // -jitter to +jitter
	return time.Duration(float64(delay) + randomJitter)
}

// ============================================
// Predefined Retry Policies
// ============================================

// NetworkRetryPolicy creates a retry policy for transient network errors.
func NetworkRetryPolicy() *RetryPolicy {
	return NewRetryPolicy(RetryConfig{
		MaxAttempts:  3,
		Backoff:      BackoffExponentialJitter,
		BaseDelay:    100 * time.Millisecond,
		MaxDelay:     5 * time.Second,
		Multiplier:   2.0,
		JitterFactor: 0.1,
	})
}

// DatabaseRetryPolicy creates a retry policy for database operations.
func DatabaseRetryPolicy() *RetryPolicy {
	return NewRetryPolicy(RetryConfig{
		MaxAttempts:  5,
		Backoff:      BackoffExponential,
		BaseDelay:    50 * time.Millisecond,
		MaxDelay:     2 * time.Second,
		Multiplier:   2.0,
		JitterFactor: 0,
	})
}

// AggressiveRetryPolicy creates an aggressive retry policy (many attempts, short delays).
func AggressiveRetryPolicy() *RetryPolicy {
	return NewRetryPolicy(RetryConfig{
		MaxAttempts:   10,
		Backoff:       BackoffExponentialJitter,
		BaseDelay:     10 * time.Millisecond,
		MaxDelay:      1 * time.Second,
		Multiplier:    1.5,
		JitterFactor:  0.2,
	})
}

// ConservativeRetryPolicy creates a conservative retry policy (few attempts, longer delays).
func ConservativeRetryPolicy() *RetryPolicy {
	return NewRetryPolicy(RetryConfig{
		MaxAttempts:   2,
		Backoff:       BackoffExponential,
		BaseDelay:     1 * time.Second,
		MaxDelay:      10 * time.Second,
		Multiplier:    3.0,
		JitterFactor:  0,
	})
}
