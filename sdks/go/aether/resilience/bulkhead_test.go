package resilience

import (
	"context"
	"sync"
	"testing"
	"time"
)

func TestBulkhead_NewWithZeroConfig(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{})
	if bh.config.MaxConcurrent != 10 {
		t.Errorf("expected default max concurrent 10, got %d", bh.config.MaxConcurrent)
	}
	if bh.config.MaxQueued != 100 {
		t.Errorf("expected default max queued 100, got %d", bh.config.MaxQueued)
	}
}

func TestBulkhead_DefaultConfig(t *testing.T) {
	cfg := DefaultBulkheadConfig()
	if cfg.MaxConcurrent != 10 {
		t.Errorf("expected 10, got %d", cfg.MaxConcurrent)
	}
	if cfg.Timeout != 0 {
		t.Errorf("expected 0 timeout, got %v", cfg.Timeout)
	}
}

func TestBulkhead_Execute(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 10})

	result, err := bh.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "done", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "done" {
		t.Errorf("expected 'done', got %v", result)
	}

	stats := bh.GetStats()
	if stats.TotalAccepted != 1 {
		t.Errorf("expected 1 accepted, got %d", stats.TotalAccepted)
	}
}

func TestBulkhead_TryExecute(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 0})

	result, err := bh.TryExecute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}
}

func TestBulkhead_TryExecute_Rejected(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 1, MaxQueued: 0})

	done := make(chan struct{})
	go func() {
		bh.Execute(context.Background(), func(ctx context.Context) (any, error) {
			<-done
			return "blocked", nil
		})
	}()
	time.Sleep(20 * time.Millisecond)

	_, err := bh.TryExecute(context.Background(), func(ctx context.Context) (any, error) {
		return "should fail", nil
	})
	if err != BulkheadRejectedError {
		t.Errorf("expected BulkheadRejectedError, got %v", err)
	}
	close(done)
}

func TestBulkhead_Execute_QueueTimeout(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{
		MaxConcurrent: 1,
		MaxQueued:     2,
		Timeout:       20 * time.Millisecond,
	})

	done := make(chan struct{})
	go func() {
		bh.Execute(context.Background(), func(ctx context.Context) (any, error) {
			<-done
			return "blocked", nil
		})
	}()
	time.Sleep(20 * time.Millisecond)

	_, err := bh.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "queued", nil
	})
	if err != BulkheadTimeoutError {
		t.Errorf("expected BulkheadTimeoutError, got %v", err)
	}
	close(done)
}

func TestBulkhead_GetStats(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 10})
	stats := bh.GetStats()

	if stats.MaxConcurrent != 5 {
		t.Errorf("expected 5, got %d", stats.MaxConcurrent)
	}
	if stats.MaxQueued != 10 {
		t.Errorf("expected 10, got %d", stats.MaxQueued)
	}
	if stats.TotalAccepted != 0 {
		t.Errorf("expected 0 accepted, got %d", stats.TotalAccepted)
	}
}

func TestBulkhead_Available(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 3, MaxQueued: 0})
	if !bh.IsAvailable() {
		t.Error("should be available")
	}
	if bh.Available() != 3 {
		t.Errorf("expected 3 available, got %d", bh.Available())
	}
}

func TestBulkhead_ResetStats(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 0})
	bh.Execute(context.Background(), func(ctx context.Context) (any, error) { return "ok", nil })

	bh.ResetStats()
	stats := bh.GetStats()
	if stats.TotalAccepted != 0 {
		t.Errorf("expected 0 after reset, got %d", stats.TotalAccepted)
	}
	if stats.TotalRejected != 0 {
		t.Errorf("expected 0 after reset, got %d", stats.TotalRejected)
	}
}

func TestBulkhead_Manager(t *testing.T) {
	mgr := NewBulkheadManager(DefaultBulkheadConfig())

	b1 := mgr.Get("api", nil)
	b2 := mgr.Get("api", nil)
	if b1 != b2 {
		t.Error("same name should return same bulkhead")
	}

	stats := mgr.GetAllStats()
	if _, exists := stats["api"]; !exists {
		t.Error("api stats should exist")
	}
}

func TestBulkhead_Manager_ResetAll(t *testing.T) {
	mgr := NewBulkheadManager(BulkheadConfig{MaxConcurrent: 5, MaxQueued: 0})
	b := mgr.Get("svc", nil)
	b.Execute(context.Background(), func(ctx context.Context) (any, error) { return "ok", nil })

	mgr.ResetAllStats()
	stats := b.GetStats()
	if stats.TotalAccepted != 0 {
		t.Errorf("expected 0 after reset, got %d", stats.TotalAccepted)
	}
}

func TestAPIBulkhead(t *testing.T) {
	bh := APIBulkhead(20)
	if bh.config.MaxConcurrent != 20 {
		t.Errorf("expected 20, got %d", bh.config.MaxConcurrent)
	}
	if bh.config.MaxQueued != 100 {
		t.Errorf("expected 100, got %d", bh.config.MaxQueued)
	}
}

func TestDatabaseBulkhead(t *testing.T) {
	bh := DatabaseBulkhead(10)
	if bh.config.MaxConcurrent != 10 {
		t.Errorf("expected 10, got %d", bh.config.MaxConcurrent)
	}
	if bh.config.Timeout != 30*time.Second {
		t.Errorf("expected 30s timeout, got %v", bh.config.Timeout)
	}
}

func TestStrictBulkhead(t *testing.T) {
	bh := StrictBulkhead(5)
	if bh.config.MaxQueued != 0 {
		t.Errorf("expected 0 queue, got %d", bh.config.MaxQueued)
	}
}

func TestBulkhead_ConcurrentExecution(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 10, MaxQueued: 100})
	var wg sync.WaitGroup

	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := bh.Execute(context.Background(), func(ctx context.Context) (any, error) {
				return "ok", nil
			})
			if err != nil {
				t.Errorf("unexpected error: %v", err)
			}
		}()
	}
	wg.Wait()

	stats := bh.GetStats()
	if stats.TotalAccepted != 10 {
		t.Errorf("expected 10 accepted, got %d", stats.TotalAccepted)
	}
}

func TestBulkhead_ContextCanceled(t *testing.T) {
	bh := NewBulkhead(BulkheadConfig{MaxConcurrent: 1, MaxQueued: 0})

	done := make(chan struct{})
	go func() {
		bh.Execute(context.Background(), func(ctx context.Context) (any, error) {
			<-done
			return "blocked", nil
		})
	}()
	time.Sleep(20 * time.Millisecond)

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := bh.Execute(ctx, func(ctx context.Context) (any, error) {
		return "fail", nil
	})
	if err == nil {
		t.Error("expected error from canceled context")
	}
	close(done)
}
