package resilience

import (
	"context"
	"testing"
	"time"
)

func TestRateLimiter_NewWithZeroConfig(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{})
	if rl.config.RequestsPerSecond != 100 {
		t.Errorf("expected 100, got %d", rl.config.RequestsPerSecond)
	}
	if rl.config.BurstSize != 100 {
		t.Errorf("expected 100 burst, got %d", rl.config.BurstSize)
	}
}

func TestRateLimiter_DefaultConfig(t *testing.T) {
	cfg := DefaultRateLimitConfig()
	if cfg.RequestsPerSecond != 100 {
		t.Errorf("expected 100, got %d", cfg.RequestsPerSecond)
	}
	if cfg.Strategy != StrategyTokenBucket {
		t.Errorf("expected token bucket strategy, got %v", cfg.Strategy)
	}
}

func TestRateLimiter_TokenBucket_AllowsUpToBurst(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 5,
		BurstSize:         5,
		Strategy:          StrategyTokenBucket,
	})

	for i := 0; i < 5; i++ {
		result := rl.TryAcquire()
		if !result.Allowed {
			t.Errorf("request %d should be allowed", i)
		}
	}

	result := rl.TryAcquire()
	if result.Allowed {
		t.Error("6th request should be rejected")
	}
}

func TestRateLimiter_SlidingWindow(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 3,
		Strategy:          StrategySlidingWindow,
		WindowSize:        time.Second,
	})

	for i := 0; i < 3; i++ {
		if !rl.TryAcquire().Allowed {
			t.Errorf("request %d should be allowed", i)
		}
	}

	if rl.TryAcquire().Allowed {
		t.Error("4th request should be rejected")
	}
}

func TestRateLimiter_FixedWindow(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 2,
		Strategy:          StrategyFixedWindow,
		WindowSize:        time.Second,
	})

	if !rl.TryAcquire().Allowed {
		t.Error("1st should be allowed")
	}
	if !rl.TryAcquire().Allowed {
		t.Error("2nd should be allowed")
	}
	if rl.TryAcquire().Allowed {
		t.Error("3rd should be rejected")
	}
}

func TestRateLimiter_TryAcquire_MultipleTokens(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 10,
		BurstSize:         10,
		Strategy:          StrategyTokenBucket,
	})

	result := rl.TryAcquire(5)
	if !result.Allowed {
		t.Error("5 tokens should be available")
	}
}

func TestRateLimiter_Acquire(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 1000,
		BurstSize:         1,
		Strategy:          StrategyTokenBucket,
	})

	rl.TryAcquire()
	err := rl.Acquire(context.Background(), 100*time.Millisecond)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestRateLimiter_Acquire_ExceedsMaxWait(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 1,
		BurstSize:         1,
		Strategy:          StrategyTokenBucket,
	})

	rl.TryAcquire()
	rl.TryAcquire()

	err := rl.Acquire(context.Background(), time.Millisecond)
	if err != RateLimitExhaustedError {
		t.Errorf("expected RateLimitExhaustedError, got %v", err)
	}
}

func TestRateLimiter_Execute(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 100,
		BurstSize:         100,
		Strategy:          StrategyTokenBucket,
	})

	result, err := rl.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "done", nil
	}, time.Second)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "done" {
		t.Errorf("expected 'done', got %v", result)
	}
}

func TestRateLimiter_Execute_Rejected(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 1,
		BurstSize:         1,
		Strategy:          StrategyTokenBucket,
	})

	rl.TryAcquire()
	rl.TryAcquire()

	_, err := rl.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "nope", nil
	}, time.Millisecond)
	if err == nil {
		t.Error("expected error when rate limited")
	}
}

func TestRateLimiter_GetStats(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 10,
		BurstSize:         10,
		Strategy:          StrategyTokenBucket,
	})

	rl.TryAcquire()
	rl.TryAcquire()

	stats := rl.GetStats()
	if stats.AllowedRequests != 2 {
		t.Errorf("expected 2 allowed, got %d", stats.AllowedRequests)
	}
}

func TestRateLimiter_ResetStats(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 10,
		BurstSize:         10,
		Strategy:          StrategyTokenBucket,
	})

	rl.TryAcquire()
	rl.TryAcquire()
	rl.TryAcquire()

	rl.ResetStats()
	stats := rl.GetStats()
	if stats.AllowedRequests != 0 {
		t.Errorf("expected 0 after reset, got %d", stats.AllowedRequests)
	}
	if stats.RejectedRequests != 0 {
		t.Errorf("expected 0 after reset, got %d", stats.RejectedRequests)
	}
}

func TestRateLimiter_Manager(t *testing.T) {
	mgr := NewRateLimiterManager(DefaultRateLimitConfig())
	rl1 := mgr.Get("api", nil)
	rl2 := mgr.Get("api", nil)
	if rl1 != rl2 {
		t.Error("same name should return same limiter")
	}

	stats := mgr.GetAllStats()
	if _, exists := stats["api"]; !exists {
		t.Error("api stats should exist")
	}
}

func TestRateLimiter_Manager_ResetAll(t *testing.T) {
	mgr := NewRateLimiterManager(RateLimitConfig{RequestsPerSecond: 10, BurstSize: 10, Strategy: StrategyTokenBucket})
	rl := mgr.Get("svc", nil)
	rl.TryAcquire()

	mgr.ResetAllStats()
	stats := rl.GetStats()
	if stats.AllowedRequests != 0 {
		t.Errorf("expected 0 after reset, got %d", stats.AllowedRequests)
	}
}

func TestAPIRateLimiter(t *testing.T) {
	rl := APIRateLimiter()
	if rl.config.RequestsPerSecond != 100 {
		t.Errorf("expected 100, got %d", rl.config.RequestsPerSecond)
	}
	if rl.config.BurstSize != 200 {
		t.Errorf("expected 200 burst, got %d", rl.config.BurstSize)
	}
}

func TestStrictRateLimiter(t *testing.T) {
	rl := StrictRateLimiter(50)
	if rl.config.RequestsPerSecond != 50 {
		t.Errorf("expected 50, got %d", rl.config.RequestsPerSecond)
	}
	if rl.config.Strategy != StrategySlidingWindow {
		t.Errorf("expected sliding window, got %v", rl.config.Strategy)
	}
}

func TestBurstyRateLimiter(t *testing.T) {
	rl := BurstyRateLimiter(500, 100)
	if rl.config.BurstSize != 500 {
		t.Errorf("expected 500 burst, got %d", rl.config.BurstSize)
	}
	if rl.config.RequestsPerSecond != 100 {
		t.Errorf("expected 100 rps, got %d", rl.config.RequestsPerSecond)
	}
}

func TestRateLimiter_Acquire_ContextCanceled(t *testing.T) {
	rl := NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 1,
		BurstSize:         1,
		Strategy:          StrategyTokenBucket,
	})

	rl.TryAcquire()
	rl.TryAcquire()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	err := rl.Acquire(ctx, 5*time.Second)
	if err == nil {
		t.Error("expected error when context is canceled")
	}
}

func TestRateLimitStrategy_Values(t *testing.T) {
	if StrategyTokenBucket != 0 {
		t.Error("StrategyTokenBucket should be 0")
	}
	if StrategySlidingWindow != 1 {
		t.Error("StrategySlidingWindow should be 1")
	}
	if StrategyFixedWindow != 2 {
		t.Error("StrategyFixedWindow should be 2")
	}
}
