package streaming

import (
	"hash/fnv"
	"sync"
	"sync/atomic"
)

// PartitionStrategy defines how events are distributed across partitions.
type PartitionStrategy int

const (
	// PartitionStrategyRoundRobin distributes events evenly across partitions.
	PartitionStrategyRoundRobin PartitionStrategy = iota
	// PartitionStrategyKey uses a key to consistently route to the same partition.
	PartitionStrategyKey
	// PartitionStrategyHash uses event hash for distribution.
	PartitionStrategyHash
	// PartitionStrategyRandom distributes events randomly.
	PartitionStrategyRandom
	// PartitionStrategyRange assigns partitions based on key ranges.
	PartitionStrategyRange
)

func (ps PartitionStrategy) String() string {
	switch ps {
	case PartitionStrategyRoundRobin:
		return "RoundRobin"
	case PartitionStrategyKey:
		return "Key"
	case PartitionStrategyHash:
		return "Hash"
	case PartitionStrategyRandom:
		return "Random"
	case PartitionStrategyRange:
		return "Range"
	default:
		return "Unknown"
	}
}

// PartitionConfig holds configuration for partitioning.
type PartitionConfig struct {
	Strategy      PartitionStrategy
	NumPartitions int
	KeyExtractor  func(any) string
	RangeBounds   []string // For range partitioning
}

// DefaultPartitionConfig returns a PartitionConfig with sensible defaults.
func DefaultPartitionConfig() PartitionConfig {
	return PartitionConfig{
		Strategy:      PartitionStrategyKey,
		NumPartitions: 10,
	}
}

// PartitionStats tracks statistics for partition distribution.
type PartitionStats struct {
	TotalEvents    int64
	PartitionCount []int64
	Rebalances     int64
	RebalancesAvg  float64
}

// Partitioner routes events to partitions based on the configured strategy.
type Partitioner struct {
	config         PartitionConfig
	currentIndex   int64
	stats          PartitionStats
	mu             sync.RWMutex
}

// NewPartitioner creates a new partitioner with the given configuration.
func NewPartitioner(config PartitionConfig) *Partitioner {
	if config.NumPartitions <= 0 {
		config.NumPartitions = 1
	}
	return &Partitioner{
		config: config,
		stats: PartitionStats{
			PartitionCount: make([]int64, config.NumPartitions),
		},
	}
}

// Partition returns the partition number for the given event.
func (p *Partitioner) Partition(event StreamEvent[any]) int {
	return p.partitionByKey(event.Key)
}

// PartitionByKey returns the partition for a given key.
func (p *Partitioner) PartitionByKey(key string) int {
	return p.partitionByKey(key)
}

// PartitionByValue returns the partition for a given value using the key extractor.
func (p *Partitioner) PartitionByValue(value any) int {
	if p.config.KeyExtractor != nil {
		key := p.config.KeyExtractor(value)
		return p.partitionByKey(key)
	}
	return p.partitionRoundRobin()
}

func (p *Partitioner) partitionByKey(key string) int {
	if key == "" {
		return p.partitionRoundRobin()
	}

	switch p.config.Strategy {
	case PartitionStrategyKey, PartitionStrategyHash:
		return p.hashPartition(key)
	case PartitionStrategyRoundRobin:
		return p.partitionRoundRobin()
	case PartitionStrategyRange:
		return p.rangePartition(key)
	default:
		return p.hashPartition(key)
	}
}

func (p *Partitioner) hashPartition(key string) int {
	h := fnv.New32a()
	h.Write([]byte(key))
	hash := h.Sum32()
	partition := int(hash) % p.config.NumPartitions
	if partition < 0 {
		partition = -partition
	}
	atomic.AddInt64(&p.stats.TotalEvents, 1)
	atomic.AddInt64(&p.stats.PartitionCount[partition], 1)
	return partition
}

func (p *Partitioner) partitionRoundRobin() int {
	idx := atomic.AddInt64(&p.currentIndex, 1) - 1
	partition := int(idx) % p.config.NumPartitions
	if partition < 0 {
		partition = -partition
	}
	atomic.AddInt64(&p.stats.TotalEvents, 1)
	atomic.AddInt64(&p.stats.PartitionCount[partition], 1)
	return partition
}

func (p *Partitioner) rangePartition(key string) int {
	bounds := p.config.RangeBounds
	if len(bounds) == 0 {
		return 0
	}

	partition := 0
	for i, bound := range bounds {
		if key < bound {
			partition = i
			break
		}
		partition = i + 1
	}

	if partition >= p.config.NumPartitions {
		partition = p.config.NumPartitions - 1
	}

	atomic.AddInt64(&p.stats.TotalEvents, 1)
	atomic.AddInt64(&p.stats.PartitionCount[partition], 1)
	return partition
}

// GetStats returns current partition statistics.
func (p *Partitioner) GetStats() PartitionStats {
	p.mu.RLock()
	defer p.mu.RUnlock()

	counts := make([]int64, len(p.stats.PartitionCount))
	for i, c := range p.stats.PartitionCount {
		counts[i] = atomic.LoadInt64(&p.stats.PartitionCount[i])
	}

	return PartitionStats{
		TotalEvents:    atomic.LoadInt64(&p.stats.TotalEvents),
		PartitionCount: counts,
		Rebalances:     atomic.LoadInt64(&p.stats.Rebalances),
		RebalancesAvg:  p.stats.RebalancesAvg,
	}
}

// NumPartitions returns the number of partitions.
func (p *Partitioner) NumPartitions() int {
	return p.config.NumPartitions
}

// Rebalance redistributes partition assignments.
func (p *Partitioner) Rebalance(newPartitionCount int) {
	p.mu.Lock()
	defer p.mu.Unlock()

	oldCount := p.config.NumPartitions
	p.config.NumPartitions = newPartitionCount

	// Reset partition counts
	p.stats.PartitionCount = make([]int64, newPartitionCount)
	atomic.AddInt64(&p.stats.Rebalances, 1)

	// Calculate average events per partition for rebalance metric
	if oldCount > 0 {
		p.stats.RebalancesAvg = float64(atomic.LoadInt64(&p.stats.TotalEvents)) / float64(newPartitionCount)
	}
}

// KeyExtractor extracts partition keys from events.
type KeyExtractor[T any] struct {
	extractor  func(T) string
	fallback   string
	count      int64
	nullCount  int64
}

// NewKeyExtractor creates a new key extractor.
func NewKeyExtractor[T any](extractor func(T) string) *KeyExtractor[T] {
	return &KeyExtractor[T]{
		extractor: extractor,
		fallback:  "default",
	}
}

// Extract returns the key for the given value.
func (ke *KeyExtractor[T]) Extract(value T) string {
	atomic.AddInt64(&ke.count, 1)

	if ke.extractor == nil {
		atomic.AddInt64(&ke.nullCount, 1)
		return ke.fallback
	}

	key := ke.extractor(value)
	if key == "" {
		atomic.AddInt64(&ke.nullCount, 1)
		return ke.fallback
	}
	return key
}

// SetFallback sets the fallback key for null/empty results.
func (ke *KeyExtractor[T]) SetFallback(fallback string) {
	ke.fallback = fallback
}

// Stats returns extraction statistics.
func (ke *KeyExtractor[T]) Stats() (int64, int64) {
	return atomic.LoadInt64(&ke.count), atomic.LoadInt64(&ke.nullCount)
}

// PartitionProcessor processes events for a specific partition.
type PartitionProcessor[T any] struct {
	partitionID int
	handler     func(StreamEvent[T]) error
	eventCount  int64
	errorCount  int64
	mu          sync.RWMutex
}

// NewPartitionProcessor creates a new partition processor.
func NewPartitionProcessor[T any](partitionID int, handler func(StreamEvent[T]) error) *PartitionProcessor[T] {
	return &PartitionProcessor[T]{
		partitionID: partitionID,
		handler:     handler,
	}
}

// Process handles an event for this partition.
func (pp *PartitionProcessor[T]) Process(event StreamEvent[T]) error {
	atomic.AddInt64(&pp.eventCount, 1)

	err := pp.handler(event)
	if err != nil {
		atomic.AddInt64(&pp.errorCount, 1)
	}
	return err
}

// PartitionID returns the partition ID.
func (pp *PartitionProcessor[T]) PartitionID() int {
	return pp.partitionID
}

// Stats returns processor statistics.
func (pp *PartitionProcessor[T]) Stats() (int64, int64) {
	return atomic.LoadInt64(&pp.eventCount), atomic.LoadInt64(&pp.errorCount)
}

// CompositePartitioner combines multiple partitioning strategies.
type CompositePartitioner[T any] struct {
	strategies  []PartitionStrategy
	weights     []float64
	extractors  []func(T) string
	partitioners []*Partitioner
	numPartitions int
	mu          sync.RWMutex
}

// NewCompositePartitioner creates a new composite partitioner.
func NewCompositePartitioner[T any](
	strategies []PartitionStrategy,
	weights []float64,
	extractors []func(T) string,
	numPartitions int,
) *CompositePartitioner[T] {
	cp := &CompositePartitioner[T]{
		strategies:    strategies,
		weights:       weights,
		extractors:    extractors,
		numPartitions: numPartitions,
		partitioners:  make([]*Partitioner, len(strategies)),
	}

	// Create partitioners for each strategy
	for i, strategy := range strategies {
		var extractor func(any) string
		if i < len(extractors) && extractors[i] != nil {
			ext := extractors[i]
			extractor = func(v any) string {
				if t, ok := v.(T); ok {
					return ext(t)
				}
				return ""
			}
		}
		cp.partitioners[i] = NewPartitioner(PartitionConfig{
			Strategy:      strategy,
			NumPartitions: numPartitions,
			KeyExtractor:  extractor,
		})
	}

	return cp
}

// Partition returns the partition for the given value using weighted strategies.
func (cp *CompositePartitioner[T]) Partition(value T) int {
	cp.mu.RLock()
	defer cp.mu.RUnlock()

	// Use weighted random selection of strategy
	totalWeight := 0.0
	for _, w := range cp.weights {
		totalWeight += w
	}

	// Simple weighted selection - use first strategy's result for now
	// In a real implementation, you'd use proper weighted selection
	if len(cp.partitioners) == 0 {
		return 0
	}

	// Get key from first extractor
	key := ""
	if len(cp.extractors) > 0 && cp.extractors[0] != nil {
		key = cp.extractors[0](value)
	}

	return cp.partitioners[0].PartitionByKey(key)
}

// AddStrategy adds a new partitioning strategy.
func (cp *CompositePartitioner[T]) AddStrategy(
	strategy PartitionStrategy,
	weight float64,
	extractor func(T) string,
) {
	cp.mu.Lock()
	defer cp.mu.Unlock()

	cp.strategies = append(cp.strategies, strategy)
	cp.weights = append(cp.weights, weight)
	cp.extractors = append(cp.extractors, extractor)
	cp.partitioners = append(cp.partitioners, NewPartitioner(PartitionConfig{
		Strategy:      strategy,
		NumPartitions: cp.numPartitions,
		KeyExtractor: func(v any) string {
			if t, ok := v.(T); ok {
				return extractor(t)
			}
			return ""
		},
	}))
}

// NumPartitions returns the number of partitions.
func (cp *CompositePartitioner[T]) NumPartitions() int {
	return cp.numPartitions
}
