package streaming

import (
	"context"
	"sync"
	"sync/atomic"
	"time"
)

// BatchConfig holds configuration for batch processing.
type BatchConfig struct {
	MaxBatchSize      int
	MaxWaitTime       time.Duration
	MaxBytes          int
	TimeoutOnFull     bool
	PartialOnTimeout  bool
	PartialOnShutdown bool
	Parallel          bool
	MaxParallelBatches int
	BatchTimeout      time.Duration
	RetryOnFailure    bool
	RetryDelay        time.Duration
	RetryBackoff      float64
	EnableAsync       bool
	AdaptiveBatching  bool
	BatchTimeoutFactor float64
	MaxConcurrency    int
}

// DefaultBatchConfig returns a BatchConfig with sensible defaults.
func DefaultBatchConfig() BatchConfig {
	return BatchConfig{
		MaxBatchSize:       1000,
		MaxWaitTime:        100 * time.Millisecond,
		MaxBytes:           1024 * 1024, // 1MB
		TimeoutOnFull:      true,
		PartialOnTimeout:   true,
		PartialOnShutdown:  true,
		Parallel:           false,
		MaxParallelBatches: 10,
		BatchTimeout:       time.Second,
		RetryOnFailure:     true,
		RetryDelay:         100 * time.Millisecond,
		RetryBackoff:       2.0,
		EnableAsync:        true,
		AdaptiveBatching:   false,
		BatchTimeoutFactor: 1.5,
		MaxConcurrency:     4,
	}
}

// BatchResult represents the result of batch processing.
type BatchResult[T any] struct {
	Items           []T
	SizeBytes       int64
	ProcessingTime  time.Duration
	BatchID         string
	Timestamp       time.Time
	Aggregated      any
	AggregationKey  string
	Checksum        string
}

// BatchStats tracks statistics for batch processing.
type BatchStats struct {
	TotalItems          int64
	TotalBatches        int64
	TotalBytes          int64
	TotalProcessingTime time.Duration
	MinProcessingTime   time.Duration
	MaxProcessingTime   time.Duration
	AvgBatchSize        float64
	FailedBatches       int64
	StartTime           time.Time
	EndTime             time.Time
}

// BatchCollector collects items into batches based on size, time, or byte limits.
type BatchCollector[T any] struct {
	config          BatchConfig
	items           []T
	currentBytes    int64
	batchStartTime  time.Time
	batchCount      int64
	mu              sync.Mutex
}

// NewBatchCollector creates a new batch collector.
func NewBatchCollector[T any](config BatchConfig) *BatchCollector[T] {
	return &BatchCollector[T]{
		config: config,
		items:  make([]T, 0, config.MaxBatchSize),
	}
}

// Add adds an item to the current batch.
func (bc *BatchCollector[T]) Add(item T, sizeBytes int) *BatchResult[T] {
	bc.mu.Lock()
	defer bc.mu.Unlock()

	// Initialize batch timing
	if bc.batchStartTime.IsZero() {
		bc.batchStartTime = time.Now()
	}

	// Add item
	bc.items = append(bc.items, item)
	bc.currentBytes += int64(sizeBytes)

	// Check if batch should be flushed
	if bc.shouldFlush() {
		return bc.flush()
	}

	return nil
}

// AddMany adds multiple items at once.
func (bc *BatchCollector[T]) AddMany(items []T, sizeBytes int) *BatchResult[T] {
	if len(items) == 0 {
		return nil
	}

	itemSize := sizeBytes / len(items)
	for _, item := range items {
		result := bc.Add(item, itemSize)
		if result != nil {
			return result
		}
	}
	return nil
}

func (bc *BatchCollector[T]) shouldFlush() bool {
	if len(bc.items) >= bc.config.MaxBatchSize {
		return true
	}
	if bc.currentBytes >= int64(bc.config.MaxBytes) {
		return true
	}
	if !bc.batchStartTime.IsZero() {
		elapsed := time.Since(bc.batchStartTime)
		if elapsed >= bc.config.MaxWaitTime {
			return bc.config.TimeoutOnFull
		}
	}
	return false
}

// Flush returns the current batch and resets the collector.
func (bc *BatchCollector[T]) Flush() *BatchResult[T] {
	bc.mu.Lock()
	defer bc.mu.Unlock()
	return bc.flush()
}

func (bc *BatchCollector[T]) flush() *BatchResult[T] {
	if len(bc.items) == 0 {
		return nil
	}

	processingTime := time.Duration(0)
	if !bc.batchStartTime.IsZero() {
		processingTime = time.Since(bc.batchStartTime)
	}

	bc.batchCount++
	batchID := "batch-" + time.Now().Format("20060102-150405-") + 
		time.Now().NanosecondFormat("999999999")

	result := &BatchResult[T]{
		Items:          bc.items,
		SizeBytes:      bc.currentBytes,
		ProcessingTime: processingTime,
		BatchID:        batchID,
		Timestamp:      time.Now(),
	}

	// Reset
	bc.items = make([]T, 0, bc.config.MaxBatchSize)
	bc.currentBytes = 0
	bc.batchStartTime = time.Time{}

	return result
}

// CurrentSize returns the current number of items in the batch.
func (bc *BatchCollector[T]) CurrentSize() int {
	bc.mu.Lock()
	defer bc.mu.Unlock()
	return len(bc.items)
}

// CurrentBytes returns the current byte size of the batch.
func (bc *BatchCollector[T]) CurrentBytes() int64 {
	bc.mu.Lock()
	defer bc.mu.Unlock()
	return bc.currentBytes
}

// IsEmpty returns true if the batch is empty.
func (bc *BatchCollector[T]) IsEmpty() bool {
	bc.mu.Lock()
	defer bc.mu.Unlock()
	return len(bc.items) == 0
}

// BatchAggregatorFunc is a function that aggregates batch items.
type BatchAggregatorFunc[T any, R any] func([]T) (R, error)

// BatchAggregator aggregates batch items into a single result.
type BatchAggregator[T any, R any] struct {
	aggregateFunc BatchAggregatorFunc[T, R]
	keyExtractor  func(T) string
	batchCount    int64
	totalEvents   int64
	processingTime time.Duration
	mu            sync.Mutex
}

// NewBatchAggregator creates a new batch aggregator.
func NewBatchAggregator[T any, R any](fn BatchAggregatorFunc[T, R]) *BatchAggregator[T, R] {
	return &BatchAggregator[T, R]{
		aggregateFunc: fn,
	}
}

// Aggregate processes a batch of items.
func (ba *BatchAggregator[T, R]) Aggregate(batch []T) (R, error) {
	if len(batch) == 0 {
		var zero R
		return zero, ErrEmptyBatch
	}

	ba.mu.Lock()
	defer ba.mu.Unlock()

	start := time.Now()
	defer func() {
		ba.processingTime += time.Since(start)
	}()

	result, err := ba.aggregateFunc(batch)
	if err != nil {
		return result, err
	}

	ba.batchCount++
	ba.totalEvents += int64(len(batch))

	return result, nil
}

// Stats returns aggregator statistics.
func (ba *BatchAggregator[T, R]) Stats() (int64, int64, time.Duration) {
	ba.mu.Lock()
	defer ba.mu.Unlock()
	return ba.batchCount, ba.totalEvents, ba.processingTime
}

// BatchEmitter emits batch results to downstream consumers.
type BatchEmitter[T any] struct {
	handlers []func(*BatchResult[T]) error
	mu       sync.RWMutex
}

// NewBatchEmitter creates a new batch emitter.
func NewBatchEmitter[T any]() *BatchEmitter[T] {
	return &BatchEmitter[T]{
		handlers: make([]func(*BatchResult[T]) error, 0),
	}
}

// AddHandler registers a handler for batch results.
func (e *BatchEmitter[T]) AddHandler(handler func(*BatchResult[T]) error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.handlers = append(e.handlers, handler)
}

// Emit sends the batch result to all registered handlers.
func (e *BatchEmitter[T]) Emit(batch *BatchResult[T]) error {
	e.mu.RLock()
	defer e.mu.RUnlock()

	for _, handler := range e.handlers {
		if err := handler(batch); err != nil {
			return err
		}
	}
	return nil
}

// BatchProcessor processes events in batches with configurable size and timing.
type BatchProcessor[T any] struct {
	config    BatchConfig
	collector *BatchCollector[T]
	queue     chan *BatchResult[T]
	running   int32
	stats     BatchStats
	mu        sync.RWMutex
	ctx       context.Context
	cancel    context.CancelFunc
	wg        sync.WaitGroup
}

// NewBatchProcessor creates a new batch processor.
func NewBatchProcessor[T any](config BatchConfig) *BatchProcessor[T] {
	ctx, cancel := context.WithCancel(context.Background())
	return &BatchProcessor[T]{
		config:    config,
		collector: NewBatchCollector[T](config),
		queue:     make(chan *BatchResult[T], config.MaxParallelBatches),
		ctx:       ctx,
		cancel:    cancel,
	}
}

// Start begins batch processing.
func (bp *BatchProcessor[T]) Start() error {
	if !atomic.CompareAndSwapInt32(&bp.running, 0, 1) {
		return ErrProcessorAlreadyRunning
	}

	bp.mu.Lock()
	bp.stats.StartTime = time.Now()
	bp.mu.Unlock()

	// Start background processor
	if bp.config.EnableAsync {
		bp.wg.Add(1)
		go bp.processLoop()
	}

	return nil
}

// Stop halts batch processing.
func (bp *BatchProcessor[T]) Stop() error {
	if !atomic.CompareAndSwapInt32(&bp.running, 1, 0) {
		return ErrProcessorNotRunning
	}

	bp.cancel()
	bp.wg.Wait()

	// Process remaining batches
	close(bp.queue)
	for batch := range bp.queue {
		bp.processBatch(batch)
	}

	// Flush collector
	if remaining := bp.collector.Flush(); remaining != nil {
		bp.processBatch(remaining)
	}

	bp.mu.Lock()
	bp.stats.EndTime = time.Now()
	bp.mu.Unlock()

	return nil
}

// Add adds an event to the batch processor.
func (bp *BatchProcessor[T]) Add(event StreamEvent[T]) error {
	if atomic.LoadInt32(&bp.running) == 0 {
		return ErrProcessorNotRunning
	}

	// Estimate size
	sizeBytes := 0
	switch v := any(event.Value).(type) {
	case []byte:
		sizeBytes = len(v)
	case string:
		sizeBytes = len(v)
	}

	if batch := bp.collector.Add(event.Value, sizeBytes); batch != nil {
		select {
		case bp.queue <- batch:
		default:
			// Queue full, process synchronously
			bp.processBatch(batch)
		}
	}

	return nil
}

func (bp *BatchProcessor[T]) processLoop() {
	defer bp.wg.Done()

	for {
		select {
		case <-bp.ctx.Done():
			return
		case batch := <-bp.queue:
			bp.processBatch(batch)
		}
	}
}

func (bp *BatchProcessor[T]) processBatch(batch *BatchResult[T]) {
	start := time.Now()

	atomic.AddInt64(&bp.stats.TotalBatches, 1)
	atomic.AddInt64(&bp.stats.TotalItems, int64(len(batch.Items)))
	atomic.AddInt64(&bp.stats.TotalBytes, batch.SizeBytes)

	processingTime := time.Since(start)
	bp.mu.Lock()
	bp.stats.TotalProcessingTime += processingTime
	if bp.stats.MinProcessingTime == 0 || processingTime < bp.stats.MinProcessingTime {
		bp.stats.MinProcessingTime = processingTime
	}
	if processingTime > bp.stats.MaxProcessingTime {
		bp.stats.MaxProcessingTime = processingTime
	}
	bp.mu.Unlock()
}

// GetStats returns current batch processor statistics.
func (bp *BatchProcessor[T]) GetStats() BatchStats {
	bp.mu.RLock()
	defer bp.mu.RUnlock()

	stats := BatchStats{
		TotalItems:          atomic.LoadInt64(&bp.stats.TotalItems),
		TotalBatches:        atomic.LoadInt64(&bp.stats.TotalBatches),
		TotalBytes:          atomic.LoadInt64(&bp.stats.TotalBytes),
		TotalProcessingTime: bp.stats.TotalProcessingTime,
		MinProcessingTime:   bp.stats.MinProcessingTime,
		MaxProcessingTime:   bp.stats.MaxProcessingTime,
		FailedBatches:       atomic.LoadInt64(&bp.stats.FailedBatches),
		StartTime:           bp.stats.StartTime,
		EndTime:             bp.stats.EndTime,
	}

	if stats.TotalBatches > 0 {
		stats.AvgBatchSize = float64(stats.TotalItems) / float64(stats.TotalBatches)
	}

	return stats
}

// Error types
var (
	ErrEmptyBatch             = &BatchError{Message: "batch cannot be empty"}
	ErrProcessorAlreadyRunning = &BatchError{Message: "processor already running"}
	ErrProcessorNotRunning    = &BatchError{Message: "processor not running"}
)

// BatchError represents a batch processing error.
type BatchError struct {
	Message string
}

func (e *BatchError) Error() string {
	return e.Message
}
