package streaming

import (
	"testing"
)

func TestPartitionStrategy_String(t *testing.T) {
	tests := []struct {
		ps   PartitionStrategy
		want string
	}{
		{PartitionStrategyRoundRobin, "RoundRobin"},
		{PartitionStrategyKey, "Key"},
		{PartitionStrategyHash, "Hash"},
		{PartitionStrategyRandom, "Random"},
		{PartitionStrategyRange, "Range"},
		{PartitionStrategy(99), "Unknown"},
	}
	for _, tt := range tests {
		if got := tt.ps.String(); got != tt.want {
			t.Errorf("PartitionStrategy(%d).String() = %q, want %q", tt.ps, got, tt.want)
		}
	}
}

func TestPartitioner_New(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 10,
	})
	if p.NumPartitions() != 10 {
		t.Errorf("expected 10, got %d", p.NumPartitions())
	}
}

func TestPartitioner_New_DefaultPartitions(t *testing.T) {
	p := NewPartitioner(PartitionConfig{})
	if p.NumPartitions() != 1 {
		t.Errorf("expected default 1 partition, got %d", p.NumPartitions())
	}
}

func TestPartitioner_PartitionByKey(t *testing.T) {
	tests := []struct {
		name     string
		strategy PartitionStrategy
		key      string
		want     int
	}{
		{"key strategy", PartitionStrategyKey, "user-1", 0},
		{"hash strategy", PartitionStrategyHash, "user-1", 0},
		{"round robin", PartitionStrategyRoundRobin, "any", 0},
		{"empty key round robin", PartitionStrategyKey, "", 0},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := NewPartitioner(PartitionConfig{
				Strategy:      tt.strategy,
				NumPartitions: 10,
			})
			part := p.PartitionByKey(tt.key)
			if part < 0 || part >= 10 {
				t.Errorf("partition %d out of range [0, 10)", part)
			}
		})
	}
}

func TestPartitioner_PartitionByKey_Consistent(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 10,
	})

	p1 := p.PartitionByKey("consistent-key")
	p2 := p.PartitionByKey("consistent-key")
	if p1 != p2 {
		t.Error("same key should always route to same partition")
	}
}

func TestPartitioner_PartitionByKey_Different(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 10,
	})

	p1 := p.PartitionByKey("key-a")
	p2 := p.PartitionByKey("key-b")
	if p1 == p2 {
		t.Error("different keys should (likely) route to different partitions")
	}
}

func TestPartitioner_Partition(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 5,
	})

	event := NewStreamEvent[any]("user-1", "data")
	part := p.Partition(event)
	if part < 0 || part >= 5 {
		t.Errorf("partition %d out of range [0, 5)", part)
	}
}

func TestPartitioner_PartitionByValue(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy: PartitionStrategyKey,
		KeyExtractor: func(v any) string {
			return "extracted"
		},
		NumPartitions: 5,
	})

	part := p.PartitionByValue("anything")
	if part < 0 || part >= 5 {
		t.Errorf("partition %d out of range", part)
	}
}

func TestPartitioner_PartitionByValue_NoExtractor(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyRoundRobin,
		NumPartitions: 3,
	})

	p1 := p.PartitionByValue("a")
	p2 := p.PartitionByValue("b")
	if p1 == p2 {
		t.Error("round robin should give different partitions")
	}
}

func TestPartitioner_RangePartition(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyRange,
		NumPartitions: 3,
		RangeBounds:   []string{"m", "s"},
	})

	p1 := p.PartitionByKey("a")
	p2 := p.PartitionByKey("n")
	p3 := p.PartitionByKey("z")

	if p1 >= p2 {
		t.Error("'a' should be in earlier partition than 'n'")
	}
	if p2 >= p3 {
		t.Error("'n' should be in earlier partition than 'z'")
	}
}

func TestPartitioner_RangePartition_NoBounds(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyRange,
		NumPartitions: 5,
		RangeBounds:   nil,
	})

	part := p.PartitionByKey("any")
	if part != 0 {
		t.Errorf("expected 0 for no bounds, got %d", part)
	}
}

func TestPartitioner_GetStats(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 5,
	})

	p.PartitionByKey("k1")
	p.PartitionByKey("k2")
	p.PartitionByKey("k3")

	stats := p.GetStats()
	if stats.TotalEvents != 3 {
		t.Errorf("expected 3 total events, got %d", stats.TotalEvents)
	}
	if len(stats.PartitionCount) != 5 {
		t.Errorf("expected 5 partition counts, got %d", len(stats.PartitionCount))
	}
}

func TestPartitioner_Rebalance(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 3,
	})

	p.PartitionByKey("k1")
	p.Rebalance(10)

	if p.NumPartitions() != 10 {
		t.Errorf("expected 10 after rebalance, got %d", p.NumPartitions())
	}
	stats := p.GetStats()
	if stats.Rebalances != 1 {
		t.Errorf("expected 1 rebalance, got %d", stats.Rebalances)
	}
}

func TestKeyExtractor_New(t *testing.T) {
	ke := NewKeyExtractor[string](func(s string) string {
		return s
	})

	if ke.Extract("test-key") != "test-key" {
		t.Errorf("expected 'test-key', got %q", ke.Extract("test-key"))
	}
}

func TestKeyExtractor_Empty(t *testing.T) {
	ke := NewKeyExtractor[string](func(s string) string {
		return ""
	})

	if ke.Extract("test") != "default" {
		t.Errorf("expected 'default' fallback, got %q", ke.Extract("test"))
	}
}

func TestKeyExtractor_Nil(t *testing.T) {
	ke := NewKeyExtractor[string](nil)
	if ke.Extract("test") != "default" {
		t.Errorf("expected 'default' for nil extractor, got %q", ke.Extract("test"))
	}
}

func TestKeyExtractor_SetFallback(t *testing.T) {
	ke := NewKeyExtractor[string](nil)
	ke.SetFallback("fallback")
	if ke.Extract("test") != "fallback" {
		t.Errorf("expected 'fallback', got %q", ke.Extract("test"))
	}
}

func TestKeyExtractor_Stats(t *testing.T) {
	ke := NewKeyExtractor[string](func(s string) string { return s })
	ke.Extract("a")
	ke.Extract("b")
	ke.Extract("")

	count, nullCount := ke.Stats()
	if count != 3 {
		t.Errorf("expected 3 extractions, got %d", count)
	}
	if nullCount != 1 {
		t.Errorf("expected 1 null, got %d", nullCount)
	}
}

func TestPartitionProcessor(t *testing.T) {
	var eventCount, errorCount int64
	pp := NewPartitionProcessor[int](1, func(event StreamEvent[int]) error {
		eventCount++
		return nil
	})

	pp.Process(NewStreamEvent[int]("k", 42))
	pp.Process(NewStreamEvent[int]("k", 43))

	if pp.PartitionID() != 1 {
		t.Errorf("expected partition ID 1, got %d", pp.PartitionID())
	}

	ec, errCnt := pp.Stats()
	if ec != 2 {
		t.Errorf("expected 2 events, got %d", ec)
	}
	if errCnt != 0 {
		t.Errorf("expected 0 errors, got %d", errCnt)
	}
	_ = eventCount
	_ = errorCount
}

func TestPartitionProcessor_Error(t *testing.T) {
	pp := NewPartitionProcessor[int](0, func(event StreamEvent[int]) error {
		return &BatchError{Message: "fail"}
	})

	pp.Process(NewStreamEvent[int]("k", 1))
	_, errCnt := pp.Stats()
	if errCnt != 1 {
		t.Errorf("expected 1 error, got %d", errCnt)
	}
}

func TestCompositePartitioner_New(t *testing.T) {
	cp := NewCompositePartitioner[string](
		[]PartitionStrategy{PartitionStrategyKey, PartitionStrategyHash},
		[]float64{0.7, 0.3},
		[]func(string) string{func(s string) string { return s }, nil},
		5,
	)

	if cp.NumPartitions() != 5 {
		t.Errorf("expected 5, got %d", cp.NumPartitions())
	}

	part := cp.Partition("test-key")
	if part < 0 || part >= 5 {
		t.Errorf("partition %d out of range", part)
	}
}

func TestCompositePartitioner_AddStrategy(t *testing.T) {
	cp := NewCompositePartitioner[string](
		[]PartitionStrategy{PartitionStrategyKey},
		[]float64{1.0},
		[]func(string) string{func(s string) string { return s }},
		5,
	)

	cp.AddStrategy(PartitionStrategyHash, 0.5, func(s string) string { return s })
	part := cp.Partition("key")
	if part < 0 || part >= 5 {
		t.Errorf("partition %d out of range", part)
	}
}

func TestDefaultPartitionConfig(t *testing.T) {
	cfg := DefaultPartitionConfig()
	if cfg.Strategy != PartitionStrategyKey {
		t.Errorf("expected key strategy, got %v", cfg.Strategy)
	}
	if cfg.NumPartitions != 10 {
		t.Errorf("expected 10, got %d", cfg.NumPartitions)
	}
}

func TestPartitioner_RoundRobinDistribution(t *testing.T) {
	p := NewPartitioner(PartitionConfig{
		Strategy:      PartitionStrategyRoundRobin,
		NumPartitions: 3,
	})

	seen := make(map[int]bool)
	for i := 0; i < 30; i++ {
		part := p.PartitionByKey("any")
		seen[part] = true
	}

	if len(seen) < 2 {
		t.Error("round robin should distribute across partitions")
	}
}
