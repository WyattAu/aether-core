package streaming

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"time"
)

var (
	ErrBufferFull     = errors.New("buffer is full")
	ErrDropped        = errors.New("event dropped")
	ErrRateLimitExceeded = errors.New("rate limit exceeded")
)

type BackpressureStats struct {
	TotalEvents      int64
	ProcessedEvents  int64
	DroppedEvents    int64
	BufferSize       int
	BufferUsed       int
	HighWatermarkHit int64
	LowWatermarkHit  int64
	CurrentLevel     float64
	Strategy         BackpressureStrategy
}

type BackpressureController[T any] struct {
	config    BackpressureConfig
	buffer    []T
	mu        sync.RWMutex
	head      int
	tail      int
	count     int

	stats      BackpressureStats
	statsMu    sync.RWMutex
}

func NewBackpressureController[T any](config BackpressureConfig) *BackpressureController[T] {
	if config.BufferSize <= 0 {
		config.BufferSize = 10000
	}
	if config.HighWatermark <= 0 || config.HighWatermark > 1 {
		config.HighWatermark = 0.9
	}
	if config.LowWatermark <= 0 || config.LowWatermark >= config.HighWatermark {
		config.LowWatermark = 0.5
	}

	return &BackpressureController[T]{
		config: config,
		buffer: make([]T, config.BufferSize),
		stats: BackpressureStats{
			Strategy: config.Strategy,
		},
	}
}

func (bc *BackpressureController[T]) Offer(event T) error {
	bc.mu.Lock()
	defer bc.mu.Unlock()

	bc.statsMu.Lock()
	bc.stats.TotalEvents++
	bc.statsMu.Unlock()

	if bc.count >= bc.config.BufferSize {
		return bc.handleFullBuffer(event)
	}

	bc.buffer[bc.tail] = event
	bc.tail = (bc.tail + 1) % bc.config.BufferSize
	bc.count++

	bc.updateWatermarks()

	return nil
}

func (bc *BackpressureController[T]) handleFullBuffer(event T) error {
	switch bc.config.Strategy {
	case BackpressureStrategyBuffer:
		bc.statsMu.Lock()
		bc.stats.DroppedEvents++
		bc.statsMu.Unlock()
		return ErrBufferFull

	case BackpressureStrategyDrop:
		bc.statsMu.Lock()
		bc.stats.DroppedEvents++
		bc.statsMu.Unlock()
		return ErrDropped

	case BackpressureStrategyFail:
		bc.statsMu.Lock()
		bc.stats.DroppedEvents++
		bc.statsMu.Unlock()
		return ErrBufferFull

	case BackpressureStrategyLatest:
		if bc.count > 0 {
			bc.head = (bc.head + 1) % bc.config.BufferSize
			bc.count--
			bc.buffer[bc.tail] = event
			bc.tail = (bc.tail + 1) % bc.config.BufferSize
			bc.count++
		}
		bc.statsMu.Lock()
		bc.stats.DroppedEvents++
		bc.statsMu.Unlock()
		return nil

	default:
		return ErrBufferFull
	}
}

func (bc *BackpressureController[T]) Poll() (T, bool) {
	bc.mu.Lock()
	defer bc.mu.Unlock()

	if bc.count == 0 {
		var zero T
		return zero, false
	}

	event := bc.buffer[bc.head]
	bc.buffer[bc.head] = *new(T)
	bc.head = (bc.head + 1) % bc.config.BufferSize
	bc.count--

	bc.statsMu.Lock()
	bc.stats.ProcessedEvents++
	bc.statsMu.Unlock()

	bc.updateWatermarks()

	return event, true
}

func (bc *BackpressureController[T]) PollBatch(maxSize int) []T {
	bc.mu.Lock()
	defer bc.mu.Unlock()

	if bc.count == 0 {
		return nil
	}

	size := maxSize
	if size > bc.count {
		size = bc.count
	}

	result := make([]T, size)
	for i := 0; i < size; i++ {
		result[i] = bc.buffer[bc.head]
		bc.buffer[bc.head] = *new(T)
		bc.head = (bc.head + 1) % bc.config.BufferSize
	}
	bc.count -= size

	bc.statsMu.Lock()
	bc.stats.ProcessedEvents += int64(size)
	bc.statsMu.Unlock()

	bc.updateWatermarks()

	return result
}

func (bc *BackpressureController[T]) updateWatermarks() {
	level := float64(bc.count) / float64(bc.config.BufferSize)

	bc.statsMu.Lock()
	bc.stats.BufferSize = bc.config.BufferSize
	bc.stats.BufferUsed = bc.count
	bc.stats.CurrentLevel = level

	if level >= bc.config.HighWatermark {
		bc.stats.HighWatermarkHit++
	} else if level <= bc.config.LowWatermark {
		bc.stats.LowWatermarkHit++
	}
	bc.statsMu.Unlock()
}

func (bc *BackpressureController[T]) IsHighWatermark() bool {
	bc.mu.RLock()
	defer bc.mu.RUnlock()

	level := float64(bc.count) / float64(bc.config.BufferSize)
	return level >= bc.config.HighWatermark
}

func (bc *BackpressureController[T]) IsLowWatermark() bool {
	bc.mu.RLock()
	defer bc.mu.RUnlock()

	level := float64(bc.count) / float64(bc.config.BufferSize)
	return level <= bc.config.LowWatermark
}

func (bc *BackpressureController[T]) GetStats() BackpressureStats {
	bc.statsMu.RLock()
	defer bc.statsMu.RUnlock()

	return BackpressureStats{
		TotalEvents:      bc.stats.TotalEvents,
		ProcessedEvents:  bc.stats.ProcessedEvents,
		DroppedEvents:    bc.stats.DroppedEvents,
		BufferSize:       bc.stats.BufferSize,
		BufferUsed:       bc.stats.BufferUsed,
		HighWatermarkHit: bc.stats.HighWatermarkHit,
		LowWatermarkHit:  bc.stats.LowWatermarkHit,
		CurrentLevel:     bc.stats.CurrentLevel,
		Strategy:         bc.stats.Strategy,
	}
}

func (bc *BackpressureController[T]) Size() int {
	bc.mu.RLock()
	defer bc.mu.RUnlock()
	return bc.count
}

func (bc *BackpressureController[T]) Capacity() int {
	return bc.config.BufferSize
}

func (bc *BackpressureController[T]) Clear() {
	bc.mu.Lock()
	defer bc.mu.Unlock()

	bc.buffer = make([]T, bc.config.BufferSize)
	bc.head = 0
	bc.tail = 0
	bc.count = 0
}

type PriorityItem[T any] struct {
	Value    T
	Priority int
}

type MultiLevelBackpressure[T any] struct {
	levels    map[int]*BackpressureController[PriorityItem[T]]
	configs   map[int]BackpressureConfig
	mu        sync.RWMutex
}

func NewMultiLevelBackpressure[T any](levels map[int]BackpressureConfig) *MultiLevelBackpressure[T] {
	mlb := &MultiLevelBackpressure[T]{
		levels:  make(map[int]*BackpressureController[PriorityItem[T]]),
		configs: levels,
	}

	for priority, config := range levels {
		mlb.levels[priority] = NewBackpressureController[PriorityItem[T]](config)
	}

	return mlb
}

func (mlb *MultiLevelBackpressure[T]) Offer(event T, priority int) error {
	mlb.mu.RLock()
	controller, exists := mlb.levels[priority]
	mlb.mu.RUnlock()

	if !exists {
		mlb.mu.Lock()
		config := BackpressureConfig{
			Strategy:  BackpressureStrategyBuffer,
			BufferSize: 10000,
		}
		if defaultConfig, ok := mlb.configs[0]; ok {
			config = defaultConfig
		}
		controller = NewBackpressureController[PriorityItem[T]](config)
		mlb.levels[priority] = controller
		mlb.mu.Unlock()
	}

	return controller.Offer(PriorityItem[T]{
		Value:    event,
		Priority: priority,
	})
}

func (mlb *MultiLevelBackpressure[T]) Poll() (T, bool) {
	mlb.mu.RLock()
	defer mlb.mu.RUnlock()

	priorities := make([]int, 0, len(mlb.levels))
	for p := range mlb.levels {
		priorities = append(priorities, p)
	}

	for _, p := range priorities {
		if item, ok := mlb.levels[p].Poll(); ok {
			return item.Value, true
		}
	}

	var zero T
	return zero, false
}

func (mlb *MultiLevelBackpressure[T]) GetStats() map[int]BackpressureStats {
	mlb.mu.RLock()
	defer mlb.mu.RUnlock()

	stats := make(map[int]BackpressureStats)
	for priority, controller := range mlb.levels {
		stats[priority] = controller.GetStats()
	}
	return stats
}

func (mlb *MultiLevelBackpressure[T]) Clear() {
	mlb.mu.Lock()
	defer mlb.mu.Unlock()

	for _, controller := range mlb.levels {
		controller.Clear()
	}
}

type RateBasedBackpressure struct {
	maxRate       int64
	currentTokens int64
	refillRate    int64
	lastRefill    time.Time
	mu            sync.Mutex
}

func NewRateBasedBackpressure(maxRate int) *RateBasedBackpressure {
	return &RateBasedBackpressure{
		maxRate:       int64(maxRate),
		currentTokens: int64(maxRate),
		refillRate:    int64(maxRate),
		lastRefill:    time.Now(),
	}
}

func (rb *RateBasedBackpressure) Allow() bool {
	rb.mu.Lock()
	defer rb.mu.Unlock()

	rb.refillTokens()

	if rb.currentTokens > 0 {
		rb.currentTokens--
		return true
	}

	return false
}

func (rb *RateBasedBackpressure) refillTokens() {
	now := time.Now()
	elapsed := now.Sub(rb.lastRefill).Seconds()
	tokensToAdd := int64(elapsed * float64(rb.refillRate))

	if tokensToAdd > 0 {
		rb.currentTokens += tokensToAdd
		if rb.currentTokens > rb.maxRate {
			rb.currentTokens = rb.maxRate
		}
		rb.lastRefill = now
	}
}

func (rb *RateBasedBackpressure) WaitForToken(ctx context.Context) error {
	for {
		if rb.Allow() {
			return nil
		}

		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(time.Millisecond * 10):
			continue
		}
	}
}

func (rb *RateBasedBackpressure) SetRate(rate int) {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	rb.maxRate = int64(rate)
	rb.refillRate = int64(rate)
}

func (rb *RateBasedBackpressure) GetRate() int {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	return int(rb.maxRate)
}

func (rb *RateBasedBackpressure) AvailableTokens() int64 {
	rb.mu.Lock()
	defer rb.mu.Unlock()
	rb.refillTokens()
	return rb.currentTokens
}

type CompositeBackpressure[T any] struct {
	controller *BackpressureController[T]
	rateLimiter *RateBasedBackpressure
}

func NewCompositeBackpressure[T any](
	bufferConfig BackpressureConfig,
	maxRate int,
) *CompositeBackpressure[T] {
	return &CompositeBackpressure[T]{
		controller:  NewBackpressureController[T](bufferConfig),
		rateLimiter: NewRateBasedBackpressure(maxRate),
	}
}

func (cb *CompositeBackpressure[T]) Offer(ctx context.Context, event T) error {
	if !cb.rateLimiter.Allow() {
		if err := cb.rateLimiter.WaitForToken(ctx); err != nil {
			return err
		}
	}
	return cb.controller.Offer(event)
}

func (cb *CompositeBackpressure[T]) Poll() (T, bool) {
	return cb.controller.Poll()
}

func (cb *CompositeBackpressure[T]) GetStats() BackpressureStats {
	return cb.controller.GetStats()
}

func (cb *CompositeBackpressure[T]) SetRate(rate int) {
	cb.rateLimiter.SetRate(rate)
}

func (cb *CompositeBackpressure[T]) GetRate() int {
	return cb.rateLimiter.GetRate()
}

type BackpressureMonitor struct {
	controllers map[string]interface {
		GetStats() BackpressureStats
	}
	mu sync.RWMutex
}

func NewBackpressureMonitor() *BackpressureMonitor {
	return &BackpressureMonitor{
		controllers: make(map[string]interface {
			GetStats() BackpressureStats
		}),
	}
}

func (m *BackpressureMonitor) Register(name string, controller interface{ GetStats() BackpressureStats }) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.controllers[name] = controller
}

func (m *BackpressureMonitor) Unregister(name string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	delete(m.controllers, name)
}

func (m *BackpressureMonitor) GetAllStats() map[string]BackpressureStats {
	m.mu.RLock()
	defer m.mu.RUnlock()

	stats := make(map[string]BackpressureStats)
	for name, controller := range m.controllers {
		stats[name] = controller.GetStats()
	}
	return stats
}

func (m *BackpressureMonitor) GetHighWatermarkControllers() []string {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var result []string
	for name, controller := range m.controllers {
		stats := controller.GetStats()
		if stats.CurrentLevel >= 0.9 {
			result = append(result, name)
		}
	}
	return result
}

func (m *BackpressureMonitor) TotalDropped() int64 {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var total int64
	for _, controller := range m.controllers {
		stats := controller.GetStats()
		total += stats.DroppedEvents
	}
	return total
}

type AdaptiveBackpressure[T any] struct {
	controller    *BackpressureController[T]
	minBufferSize int
	maxBufferSize int
	scaleUpThreshold   float64
	scaleDownThreshold float64
	mu            sync.Mutex
}

func NewAdaptiveBackpressure[T any](
	initialSize int,
	minSize int,
	maxSize int,
) *AdaptiveBackpressure[T] {
	config := BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    initialSize,
		HighWatermark: 0.8,
		LowWatermark:  0.3,
	}

	return &AdaptiveBackpressure[T]{
		controller:         NewBackpressureController[T](config),
		minBufferSize:      minSize,
		maxBufferSize:      maxSize,
		scaleUpThreshold:   0.8,
		scaleDownThreshold: 0.3,
	}
}

func (ab *AdaptiveBackpressure[T]) Offer(event T) error {
	ab.mu.Lock()
	defer ab.mu.Unlock()

	stats := ab.controller.GetStats()

	if stats.CurrentLevel >= ab.scaleUpThreshold {
		ab.scaleUp()
	} else if stats.CurrentLevel <= ab.scaleDownThreshold && ab.controller.Capacity() > ab.minBufferSize {
		ab.scaleDown()
	}

	return ab.controller.Offer(event)
}

func (ab *AdaptiveBackpressure[T]) Poll() (T, bool) {
	return ab.controller.Poll()
}

func (ab *AdaptiveBackpressure[T]) scaleUp() {
	currentSize := ab.controller.Capacity()
	if currentSize >= ab.maxBufferSize {
		return
	}

	newSize := currentSize * 2
	if newSize > ab.maxBufferSize {
		newSize = ab.maxBufferSize
	}

	ab.controller = NewBackpressureController[T](BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    newSize,
		HighWatermark: 0.8,
		LowWatermark:  0.3,
	})
}

func (ab *AdaptiveBackpressure[T]) scaleDown() {
	currentSize := ab.controller.Capacity()
	if currentSize <= ab.minBufferSize {
		return
	}

	newSize := currentSize / 2
	if newSize < ab.minBufferSize {
		newSize = ab.minBufferSize
	}

	ab.controller = NewBackpressureController[T](BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    newSize,
		HighWatermark: 0.8,
		LowWatermark:  0.3,
	})
}

func (ab *AdaptiveBackpressure[T]) GetStats() BackpressureStats {
	return ab.controller.GetStats()
}
