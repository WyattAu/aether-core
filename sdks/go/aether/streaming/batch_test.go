package streaming

import (
	"errors"
	"testing"
	"time"
)

func TestDefaultBatchConfig(t *testing.T) {
	cfg := DefaultBatchConfig()
	if cfg.MaxBatchSize != 1000 {
		t.Errorf("expected 1000, got %d", cfg.MaxBatchSize)
	}
	if cfg.MaxWaitTime != 100*time.Millisecond {
		t.Errorf("expected 100ms, got %v", cfg.MaxWaitTime)
	}
}

func TestBatchCollector_New(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{MaxBatchSize: 10, MaxWaitTime: time.Second})
	if bc == nil {
		t.Fatal("expected non-nil collector")
	}
	if bc.CurrentSize() != 0 {
		t.Errorf("expected 0, got %d", bc.CurrentSize())
	}
	if !bc.IsEmpty() {
		t.Error("new collector should be empty")
	}
}

func TestBatchCollector_Add(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{
		MaxBatchSize: 3,
		MaxWaitTime:  time.Hour,
	})

	result := bc.Add(1, 10)
	if result != nil {
		t.Error("should not flush at size 1")
	}
	if bc.CurrentSize() != 1 {
		t.Errorf("expected size 1, got %d", bc.CurrentSize())
	}

	result = bc.Add(2, 10)
	if result != nil {
		t.Error("should not flush at size 2")
	}

	result = bc.Add(3, 10)
	if result == nil {
		t.Error("should flush at size 3")
	}
	if len(result.Items) != 3 {
		t.Errorf("expected 3 items, got %d", len(result.Items))
	}
	if bc.CurrentSize() != 0 {
		t.Errorf("expected size 0 after flush, got %d", bc.CurrentSize())
	}
}

func TestBatchCollector_AddByBytes(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{
		MaxBatchSize: 100,
		MaxBytes:     50,
		MaxWaitTime:  time.Hour,
		TimeoutOnFull: true,
	})

	for i := 0; i < 10; i++ {
		result := bc.Add(i, 10)
		if result != nil {
			if result.SizeBytes != 100 {
				t.Errorf("expected 100 bytes, got %d", result.SizeBytes)
			}
			break
		}
	}
}

func TestBatchCollector_AddMany(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{MaxBatchSize: 10, MaxWaitTime: time.Hour})
	result := bc.AddMany([]int{1, 2, 3}, 30)
	if result != nil {
		t.Error("should not flush")
	}
	if bc.CurrentSize() != 3 {
		t.Errorf("expected size 3, got %d", bc.CurrentSize())
	}
}

func TestBatchCollector_AddMany_Empty(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{MaxBatchSize: 10, MaxWaitTime: time.Hour})
	result := bc.AddMany([]int{}, 0)
	if result != nil {
		t.Error("empty AddMany should return nil")
	}
}

func TestBatchCollector_Flush(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{MaxBatchSize: 100, MaxWaitTime: time.Hour})
	bc.Add(1, 10)
	bc.Add(2, 10)

	result := bc.Flush()
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if len(result.Items) != 2 {
		t.Errorf("expected 2 items, got %d", len(result.Items))
	}
	if result.BatchID == "" {
		t.Error("batch ID should be set")
	}
	if bc.CurrentSize() != 0 {
		t.Errorf("expected 0 after flush, got %d", bc.CurrentSize())
	}
}

func TestBatchCollector_Flush_Empty(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{MaxBatchSize: 100, MaxWaitTime: time.Hour})
	result := bc.Flush()
	if result != nil {
		t.Error("flushing empty should return nil")
	}
}

func TestBatchCollector_CurrentBytes(t *testing.T) {
	bc := NewBatchCollector[int](BatchConfig{MaxBatchSize: 100, MaxWaitTime: time.Hour})
	bc.Add(1, 50)
	bc.Add(2, 30)

	if bc.CurrentBytes() != 80 {
		t.Errorf("expected 80 bytes, got %d", bc.CurrentBytes())
	}
}

func TestBatchAggregator_Aggregate(t *testing.T) {
	sumFn := func(items []int) (int, error) {
		sum := 0
		for _, i := range items {
			sum += i
		}
		return sum, nil
	}

	ba := NewBatchAggregator[int, int](sumFn)
	result, err := ba.Aggregate([]int{1, 2, 3, 4, 5})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if result != 15 {
		t.Errorf("expected 15, got %d", result)
	}
}

func TestBatchAggregator_Aggregate_Empty(t *testing.T) {
	ba := NewBatchAggregator[int, int](func(items []int) (int, error) { return 0, nil })
	_, err := ba.Aggregate([]int{})
	if err != ErrEmptyBatch {
		t.Errorf("expected ErrEmptyBatch, got %v", err)
	}
}

func TestBatchAggregator_Aggregate_Error(t *testing.T) {
	ba := NewBatchAggregator[int, int](func(items []int) (int, error) {
		return 0, errors.New("fail")
	})
	_, err := ba.Aggregate([]int{1})
	if err == nil {
		t.Error("expected error from failing function")
	}
}

func TestBatchAggregator_Stats(t *testing.T) {
	sumFn := func(items []int) (int, error) { return 0, nil }
	ba := NewBatchAggregator[int, int](sumFn)

	ba.Aggregate([]int{1, 2})
	ba.Aggregate([]int{3, 4, 5})

	count, events, _ := ba.Stats()
	if count != 2 {
		t.Errorf("expected 2 batches, got %d", count)
	}
	if events != 5 {
		t.Errorf("expected 5 events, got %d", events)
	}
}

func TestBatchEmitter(t *testing.T) {
	emitter := NewBatchEmitter[int]()
	var received *BatchResult[int]

	emitter.AddHandler(func(batch *BatchResult[int]) error {
		received = batch
		return nil
	})

	batch := &BatchResult[int]{Items: []int{1, 2, 3}}
	err := emitter.Emit(batch)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if len(received.Items) != 3 {
		t.Errorf("expected 3 items, got %d", len(received.Items))
	}
}

func TestBatchEmitter_HandlerError(t *testing.T) {
	emitter := NewBatchEmitter[int]()
	emitter.AddHandler(func(batch *BatchResult[int]) error {
		return errors.New("handler error")
	})

	err := emitter.Emit(&BatchResult[int]{Items: []int{1}})
	if err == nil {
		t.Error("expected error from failing handler")
	}
}

func TestBatchError(t *testing.T) {
	err := &BatchError{Message: "test"}
	if err.Error() != "test" {
		t.Errorf("expected 'test', got %q", err.Error())
	}
}

func TestBatchResult_Fields(t *testing.T) {
	br := &BatchResult[int]{
		Items:          []int{1, 2, 3},
		SizeBytes:      30,
		ProcessingTime: 5 * time.Millisecond,
		BatchID:        "batch-1",
		AggregationKey: "key1",
	}
	if len(br.Items) != 3 {
		t.Errorf("expected 3 items, got %d", len(br.Items))
	}
	if br.SizeBytes != 30 {
		t.Errorf("expected 30 bytes, got %d", br.SizeBytes)
	}
}

func TestBatchProcessor_New(t *testing.T) {
	bp := NewBatchProcessor[int](DefaultBatchConfig())
	if bp == nil {
		t.Fatal("expected non-nil processor")
	}
}

func TestBatchProcessor_StartStop(t *testing.T) {
	bp := NewBatchProcessor[int](BatchConfig{
		MaxBatchSize:    10,
		MaxWaitTime:     time.Hour,
		EnableAsync:     false,
		MaxParallelBatches: 1,
	})

	err := bp.Start()
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	err = bp.Stop()
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestBatchProcessor_Add(t *testing.T) {
	cfg := BatchConfig{
		MaxBatchSize:    3,
		MaxWaitTime:     time.Hour,
		EnableAsync:     false,
		MaxParallelBatches: 1,
	}
	bp := NewBatchProcessor[int](cfg)
	bp.Start()

	bp.Add(NewStreamEvent[int]("k", 1))
	bp.Add(NewStreamEvent[int]("k", 2))
	bp.Add(NewStreamEvent[int]("k", 3))

	bp.Stop()

	stats := bp.GetStats()
	if stats.TotalItems != 3 {
		t.Errorf("expected 3 items, got %d", stats.TotalItems)
	}
}

func TestBatchProcessor_GetStats(t *testing.T) {
	bp := NewBatchProcessor[int](DefaultBatchConfig())
	bp.Start()

	bp.Add(NewStreamEvent[int]("k", 1))
	bp.Add(NewStreamEvent[int]("k", 2))
	bp.Add(NewStreamEvent[int]("k", 3))
	bp.Stop()

	stats := bp.GetStats()
	if stats.TotalItems == 0 {
		t.Error("expected non-zero total items")
	}
}

func TestBatchStats_Fields(t *testing.T) {
	bs := BatchStats{
		TotalItems:    100,
		TotalBatches:  10,
		TotalBytes:    1000,
		FailedBatches: 1,
	}
	if bs.TotalItems != 100 {
		t.Errorf("expected 100, got %d", bs.TotalItems)
	}
	if bs.FailedBatches != 1 {
		t.Errorf("expected 1, got %d", bs.FailedBatches)
	}
}
