package streaming

import (
	"testing"
)

func TestBackpressureController_New(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    100,
		HighWatermark: 0.8,
		LowWatermark:  0.4,
	})
	if bc.Capacity() != 100 {
		t.Errorf("expected capacity 100, got %d", bc.Capacity())
	}
	if bc.Size() != 0 {
		t.Errorf("expected size 0, got %d", bc.Size())
	}
}

func TestBackpressureController_New_Defaults(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{})
	if bc.Capacity() != 10000 {
		t.Errorf("expected default capacity 10000, got %d", bc.Capacity())
	}
}

func TestBackpressureController_OfferAndPoll(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 10,
	})

	for i := 0; i < 5; i++ {
		err := bc.Offer(i)
		if err != nil {
			t.Errorf("offer %d: unexpected error: %v", i, err)
		}
	}

	if bc.Size() != 5 {
		t.Errorf("expected size 5, got %d", bc.Size())
	}

	for i := 0; i < 5; i++ {
		val, ok := bc.Poll()
		if !ok {
			t.Errorf("poll %d: expected ok", i)
		}
		if val != i {
			t.Errorf("poll %d: expected %d, got %d", i, i, val)
		}
	}

	if bc.Size() != 0 {
		t.Errorf("expected size 0, got %d", bc.Size())
	}
}

func TestBackpressureController_Poll_Empty(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 5,
	})

	_, ok := bc.Poll()
	if ok {
		t.Error("poll from empty buffer should return false")
	}
}

func TestBackpressureController_BufferFull_DropStrategy(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyDrop,
		BufferSize: 3,
	})

	for i := 0; i < 3; i++ {
		bc.Offer(i)
	}

	err := bc.Offer(99)
	if err != ErrDropped {
		t.Errorf("expected ErrDropped, got %v", err)
	}
}

func TestBackpressureController_BufferFull_FailStrategy(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyFail,
		BufferSize: 3,
	})

	for i := 0; i < 3; i++ {
		bc.Offer(i)
	}

	err := bc.Offer(99)
	if err != ErrBufferFull {
		t.Errorf("expected ErrBufferFull, got %v", err)
	}
}

func TestBackpressureController_LatestStrategy(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyLatest,
		BufferSize: 3,
	})

	bc.Offer(1)
	bc.Offer(2)
	bc.Offer(3)

	err := bc.Offer(4)
	if err != nil {
		t.Errorf("Latest strategy should not error, got %v", err)
	}

	if bc.Size() != 3 {
		t.Errorf("expected size 3, got %d", bc.Size())
	}
}

func TestBackpressureController_PollBatch(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 10,
	})

	for i := 0; i < 7; i++ {
		bc.Offer(i)
	}

	batch := bc.PollBatch(3)
	if len(batch) != 3 {
		t.Errorf("expected batch of 3, got %d", len(batch))
	}
	if bc.Size() != 4 {
		t.Errorf("expected size 4, got %d", bc.Size())
	}

	batch = bc.PollBatch(100)
	if len(batch) != 4 {
		t.Errorf("expected batch of 4, got %d", len(batch))
	}
}

func TestBackpressureController_PollBatch_Empty(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 5,
	})

	batch := bc.PollBatch(10)
	if batch != nil {
		t.Errorf("expected nil for empty poll, got %v", batch)
	}
}

func TestBackpressureController_GetStats(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    10,
		HighWatermark: 0.5,
	})

	for i := 0; i < 8; i++ {
		bc.Offer(i)
	}

	stats := bc.GetStats()
	if stats.TotalEvents != 8 {
		t.Errorf("expected 8 total events, got %d", stats.TotalEvents)
	}
	if stats.BufferUsed != 8 {
		t.Errorf("expected 8 buffer used, got %d", stats.BufferUsed)
	}
}

func TestBackpressureController_Clear(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 10,
	})

	for i := 0; i < 5; i++ {
		bc.Offer(i)
	}
	bc.Clear()

	if bc.Size() != 0 {
		t.Errorf("expected size 0 after clear, got %d", bc.Size())
	}
}

func TestBackpressureController_Watermarks(t *testing.T) {
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    10,
		HighWatermark: 0.8,
		LowWatermark:  0.3,
	})

	if bc.IsHighWatermark() {
		t.Error("empty buffer should not be at high watermark")
	}
	if !bc.IsLowWatermark() {
		t.Error("empty buffer should be at low watermark")
	}

	for i := 0; i < 9; i++ {
		bc.Offer(i)
	}

	if !bc.IsHighWatermark() {
		t.Error("90%% full should be at high watermark")
	}
}

func TestRateBasedBackpressure_Allow(t *testing.T) {
	rbb := NewRateBasedBackpressure(10)
	for i := 0; i < 10; i++ {
		if !rbb.Allow() {
			t.Errorf("allow %d should return true", i)
		}
	}

	if rbb.Allow() {
		t.Error("should be rate limited after exhausting tokens")
	}
}

func TestRateBasedBackpressure_SetRate(t *testing.T) {
	rbb := NewRateBasedBackpressure(5)
	rbb.SetRate(20)
	if rbb.GetRate() != 20 {
		t.Errorf("expected rate 20, got %d", rbb.GetRate())
	}
}

func TestRateBasedBackpressure_AvailableTokens(t *testing.T) {
	rbb := NewRateBasedBackpressure(5)
	if rbb.AvailableTokens() != 5 {
		t.Errorf("expected 5 tokens, got %d", rbb.AvailableTokens())
	}
	rbb.Allow()
	rbb.Allow()
	if rbb.AvailableTokens() != 3 {
		t.Errorf("expected 3 tokens, got %d", rbb.AvailableTokens())
	}
}

func TestCompositeBackpressure_OfferAndPoll(t *testing.T) {
	cb := NewCompositeBackpressure[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 10,
	}, 1000)

	err := cb.Offer(nil, 42)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	val, ok := cb.Poll()
	if !ok {
		t.Error("expected value")
	}
	if val != 42 {
		t.Errorf("expected 42, got %d", val)
	}
}

func TestBackpressureMonitor(t *testing.T) {
	mon := NewBackpressureMonitor()
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyBuffer,
		BufferSize: 10,
	})

	mon.Register("test", bc)
	stats := mon.GetAllStats()
	if _, exists := stats["test"]; !exists {
		t.Error("test should be in stats")
	}

	mon.Unregister("test")
	stats = mon.GetAllStats()
	if _, exists := stats["test"]; exists {
		t.Error("test should be removed from stats")
	}
}

func TestBackpressureMonitor_TotalDropped(t *testing.T) {
	mon := NewBackpressureMonitor()
	bc := NewBackpressureController[int](BackpressureConfig{
		Strategy:   BackpressureStrategyDrop,
		BufferSize: 1,
	})

	mon.Register("test", bc)
	bc.Offer(1)
	bc.Offer(2)

	if mon.TotalDropped() != 1 {
		t.Errorf("expected 1 dropped, got %d", mon.TotalDropped())
	}
}

func TestMultiLevelBackpressure(t *testing.T) {
	levels := map[int]BackpressureConfig{
		0: {Strategy: BackpressureStrategyBuffer, BufferSize: 10},
		1: {Strategy: BackpressureStrategyDrop, BufferSize: 5},
	}
	mlb := NewMultiLevelBackpressure[int](levels)

	mlb.Offer(42, 0)
	val, ok := mlb.Poll()
	if !ok {
		t.Error("expected value")
	}
	if val != 42 {
		t.Errorf("expected 42, got %d", val)
	}
}

func TestMultiLevelBackpressure_GetStats(t *testing.T) {
	levels := map[int]BackpressureConfig{
		0: {Strategy: BackpressureStrategyBuffer, BufferSize: 10},
	}
	mlb := NewMultiLevelBackpressure[int](levels)
	stats := mlb.GetStats()
	if _, exists := stats[0]; !exists {
		t.Error("level 0 should be in stats")
	}
}

func TestAdaptiveBackpressure(t *testing.T) {
	ab := NewAdaptiveBackpressure[int](100, 50, 1000)
	ab.Offer(1)
	ab.Offer(2)

	stats := ab.GetStats()
	if stats.BufferSize != 100 {
		t.Errorf("expected initial size 100, got %d", stats.BufferSize)
	}
}
