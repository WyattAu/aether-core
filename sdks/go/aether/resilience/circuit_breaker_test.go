package resilience

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"
)

func TestCircuitBreaker_NewWithZeroConfig(t *testing.T) {
	cb := NewCircuitBreaker(CircuitBreakerConfig{})
	if cb.config.FailureThreshold != 5 {
		t.Errorf("expected default failure threshold 5, got %d", cb.config.FailureThreshold)
	}
	if cb.config.SuccessThreshold != 3 {
		t.Errorf("expected default success threshold 3, got %d", cb.config.SuccessThreshold)
	}
	if cb.State() != StateClosed {
		t.Error("initial state should be closed")
	}
}

func TestCircuitBreaker_DefaultConfig(t *testing.T) {
	cfg := DefaultCircuitBreakerConfig()
	if cfg.FailureThreshold != 5 {
		t.Errorf("expected 5, got %d", cfg.FailureThreshold)
	}
	if cfg.SuccessThreshold != 3 {
		t.Errorf("expected 3, got %d", cfg.SuccessThreshold)
	}
	if cfg.Timeout != 30*time.Second {
		t.Errorf("expected 30s, got %v", cfg.Timeout)
	}
}

func TestCircuitBreaker_CircuitState_String(t *testing.T) {
	tests := []struct {
		state CircuitState
		want  string
	}{
		{StateClosed, "closed"},
		{StateOpen, "open"},
		{StateHalfOpen, "half-open"},
		{CircuitState(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.state.String(); got != tt.want {
			t.Errorf("CircuitState(%d).String() = %q, want %q", tt.state, got, tt.want)
		}
	}
}

func TestCircuitBreaker_StateAccessors(t *testing.T) {
	cb := NewCircuitBreaker(DefaultCircuitBreakerConfig())

	if !cb.IsClosed() {
		t.Error("initially should be closed")
	}
	if cb.IsOpen() {
		t.Error("initially should not be open")
	}
	if cb.IsHalfOpen() {
		t.Error("initially should not be half-open")
	}

	cb.ForceOpen()
	if !cb.IsOpen() {
		t.Error("should be open after ForceOpen")
	}
}

func TestCircuitBreaker_ForceOpen(t *testing.T) {
	cb := NewCircuitBreaker(DefaultCircuitBreakerConfig())
	cb.ForceOpen()

	if cb.State() != StateOpen {
		t.Error("expected open state")
	}
}

func TestCircuitBreaker_ForceClose(t *testing.T) {
	cb := NewCircuitBreaker(DefaultCircuitBreakerConfig())
	cb.ForceOpen()
	cb.ForceClose()

	if cb.State() != StateClosed {
		t.Error("expected closed state")
	}
}

func TestCircuitBreaker_Reset(t *testing.T) {
	config := CircuitBreakerConfig{FailureThreshold: 2, Timeout: 100 * time.Millisecond}
	cb := NewCircuitBreaker(config)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})
	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})

	if !cb.IsOpen() {
		t.Fatal("expected open state")
	}

	cb.Reset()

	if !cb.IsClosed() {
		t.Error("expected closed state after reset")
	}

	stats := cb.GetStats()
	if stats.Failures != 0 {
		t.Errorf("expected 0 failures after reset, got %d", stats.Failures)
	}
	if stats.TotalCalls != 0 {
		t.Errorf("expected 0 total calls after reset, got %d", stats.TotalCalls)
	}
}

func TestCircuitBreaker_GetStats(t *testing.T) {
	cb := NewCircuitBreaker(DefaultCircuitBreakerConfig())
	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})

	stats := cb.GetStats()
	if stats.State != StateClosed {
		t.Errorf("expected closed state in stats, got %v", stats.State)
	}
	if stats.TotalCalls != 1 {
		t.Errorf("expected 1 total call, got %d", stats.TotalCalls)
	}
	if stats.LastFailure.IsZero() {
		t.Error("last failure should be zero time for no failures")
	}
}

func TestCircuitBreaker_Callbacks(t *testing.T) {
	openCalled, closeCalled, halfOpenCalled := false, false, false
	mu := sync.Mutex{}

	config := CircuitBreakerConfig{
		FailureThreshold: 1,
		SuccessThreshold: 1,
		Timeout:          50 * time.Millisecond,
		OnOpen: func() { mu.Lock(); openCalled = true; mu.Unlock() },
		OnClose: func() { mu.Lock(); closeCalled = true; mu.Unlock() },
		OnHalfOpen: func() { mu.Lock(); halfOpenCalled = true; mu.Unlock() },
	}
	cb := NewCircuitBreaker(config)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})

	time.Sleep(10 * time.Millisecond)
	mu.Lock()
	if !openCalled {
		t.Error("OnOpen should have been called")
	}
	_ = closeCalled
	_ = halfOpenCalled
	mu.Unlock()
}

func TestCircuitBreaker_HalfOpen_FailureReopens(t *testing.T) {
	config := CircuitBreakerConfig{
		FailureThreshold: 1,
		SuccessThreshold: 2,
		Timeout:          50 * time.Millisecond,
	}
	cb := NewCircuitBreaker(config)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})
	if !cb.IsOpen() {
		t.Fatal("expected open state")
	}

	time.Sleep(60 * time.Millisecond)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail again")
	})
	if !cb.IsOpen() {
		t.Error("should be open again after half-open failure")
	}
}

func TestCircuitBreaker_HalfOpenMaxCalls(t *testing.T) {
	config := CircuitBreakerConfig{
		FailureThreshold: 1,
		SuccessThreshold: 2,
		Timeout:          50 * time.Millisecond,
		HalfOpenMaxCalls: 1,
	}
	cb := NewCircuitBreaker(config)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})
	time.Sleep(60 * time.Millisecond)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})

	_, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "should reject", nil
	})
	if err != CircuitBreakerError {
		t.Errorf("expected CircuitBreakerError, got %v", err)
	}
}

func TestCircuitBreaker_Manager(t *testing.T) {
	mgr := NewCircuitBreakerManager(DefaultCircuitBreakerConfig())

	b1 := mgr.Get("svc1", nil)
	b2 := mgr.Get("svc1", nil)
	if b1 != b2 {
		t.Error("same name should return same breaker")
	}

	b3 := mgr.Get("svc2", nil)
	if b1 == b3 {
		t.Error("different names should return different breakers")
	}

	stats := mgr.GetAllStats()
	if len(stats) != 2 {
		t.Errorf("expected 2 stats entries, got %d", len(stats))
	}
}

func TestCircuitBreaker_Manager_CustomConfig(t *testing.T) {
	mgr := NewCircuitBreakerManager(DefaultCircuitBreakerConfig())
	custom := CircuitBreakerConfig{FailureThreshold: 10}
	b := mgr.Get("custom", &custom)
	if b.config.FailureThreshold != 10 {
		t.Errorf("expected failure threshold 10, got %d", b.config.FailureThreshold)
	}
}

func TestCircuitBreaker_Manager_GetOpenBreakers(t *testing.T) {
	mgr := NewCircuitBreakerManager(CircuitBreakerConfig{FailureThreshold: 1, Timeout: time.Minute})
	b := mgr.Get("svc", nil)
	b.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})

	open := mgr.GetOpenBreakers()
	if len(open) != 1 {
		t.Errorf("expected 1 open breaker, got %d", len(open))
	}
}

func TestCircuitBreaker_Manager_ResetAll(t *testing.T) {
	mgr := NewCircuitBreakerManager(CircuitBreakerConfig{FailureThreshold: 1, Timeout: time.Minute})
	b := mgr.Get("svc", nil)
	b.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail")
	})

	mgr.ResetAll()
	if !b.IsClosed() {
		t.Error("breaker should be closed after ResetAll")
	}
}

func TestCircuitBreaker_ContextCanceled(t *testing.T) {
	cb := NewCircuitBreaker(DefaultCircuitBreakerConfig())

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := cb.Execute(ctx, func(ctx context.Context) (any, error) {
		time.Sleep(time.Second)
		return "done", nil
	})
	if err == nil {
		t.Error("expected error when context is canceled")
	}
}

func TestCircuitBreaker_SuccessResetsFailureCount(t *testing.T) {
	config := CircuitBreakerConfig{FailureThreshold: 3, Timeout: time.Minute}
	cb := NewCircuitBreaker(config)

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail1")
	})
	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, errors.New("fail2")
	})

	cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})

	if cb.IsOpen() {
		t.Error("circuit should still be closed: 2 failures + 1 success should reset failure count")
	}
}
