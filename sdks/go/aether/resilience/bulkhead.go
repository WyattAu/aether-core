package resilience

import (
	"context"
	"errors"
	"sync"
	"time"
)

// BulkheadConfig holds configuration for the bulkhead.
type BulkheadConfig struct {
	// MaxConcurrent is the maximum concurrent calls.
	MaxConcurrent int
	// MaxQueued is the maximum queued calls.
	MaxQueued int
	// Timeout for queued calls (0 = no timeout).
	Timeout time.Duration
}

// DefaultBulkheadConfig returns the default configuration.
func DefaultBulkheadConfig() BulkheadConfig {
	return BulkheadConfig{
		MaxConcurrent: 10,
		MaxQueued:     100,
		Timeout:       0,
	}
}

// BulkheadStats holds statistics for the bulkhead.
type BulkheadStats struct {
	Active        int
	Queued        int
	MaxConcurrent int
	MaxQueued     int
	TotalAccepted int64
	TotalRejected int64
	TotalTimeout  int64
}

// BulkheadRejectedError is returned when the bulkhead rejects a call.
var BulkheadRejectedError = errors.New("bulkhead at capacity")

// BulkheadTimeoutError is returned when a queued call times out.
var BulkheadTimeoutError = errors.New("bulkhead queued call timed out")

// Bulkhead implements the bulkhead pattern for resource isolation.
type Bulkhead struct {
	config BulkheadConfig

	semaphore     chan struct{}
	queueSemaphore chan struct{}

	mu             sync.Mutex
	active         int
	queued         int
	totalAccepted  int64
	totalRejected  int64
	totalTimeout   int64
}

// NewBulkhead creates a new bulkhead.
func NewBulkhead(config BulkheadConfig) *Bulkhead {
	if config.MaxConcurrent == 0 {
		config.MaxConcurrent = 10
	}
	if config.MaxQueued == 0 {
		config.MaxQueued = 100
	}

	return &Bulkhead{
		config:         config,
		semaphore:      make(chan struct{}, config.MaxConcurrent),
		queueSemaphore: make(chan struct{}, config.MaxQueued),
	}
}

// Execute runs the given function with bulkhead protection.
func (b *Bulkhead) Execute(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	// Check if queue has space
	select {
	case b.queueSemaphore <- struct{}{}:
		// Got queue slot
	default:
		b.mu.Lock()
		b.totalRejected++
		b.mu.Unlock()
		return nil, BulkheadRejectedError
	}
	defer func() { <-b.queueSemaphore }()

	b.mu.Lock()
	b.queued++
	b.mu.Unlock()

	defer func() {
		b.mu.Lock()
		b.queued--
		b.mu.Unlock()
	}()

	// Try to acquire execution slot
	var acquired bool
	if b.config.Timeout > 0 {
		ctx, cancel := context.WithTimeout(ctx, b.config.Timeout)
		defer cancel()

		select {
		case b.semaphore <- struct{}{}:
			acquired = true
		case <-ctx.Done():
			b.mu.Lock()
			b.totalTimeout++
			b.mu.Unlock()
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				return nil, BulkheadTimeoutError
			}
			return nil, ctx.Err()
		}
	} else {
		select {
		case b.semaphore <- struct{}{}:
			acquired = true
		case <-ctx.Done():
			return nil, ctx.Err()
		}
	}

	if !acquired {
		return nil, BulkheadRejectedError
	}

	b.mu.Lock()
	b.queued--
	b.active++
	b.totalAccepted++
	b.mu.Unlock()

	defer func() {
		<-b.semaphore
		b.mu.Lock()
		b.active--
		b.mu.Unlock()
	}()

	return fn(ctx)
}

// TryExecute tries to execute without queuing.
func (b *Bulkhead) TryExecute(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	select {
	case b.semaphore <- struct{}{}:
		// Got slot
	default:
		b.mu.Lock()
		b.totalRejected++
		b.mu.Unlock()
		return nil, BulkheadRejectedError
	}

	b.mu.Lock()
	b.active++
	b.totalAccepted++
	b.mu.Unlock()

	defer func() {
		<-b.semaphore
		b.mu.Lock()
		b.active--
		b.mu.Unlock()
	}()

	return fn(ctx)
}

// GetStats returns current statistics.
func (b *Bulkhead) GetStats() BulkheadStats {
	b.mu.Lock()
	defer b.mu.Unlock()

	return BulkheadStats{
		Active:        b.active,
		Queued:        b.queued,
		MaxConcurrent: b.config.MaxConcurrent,
		MaxQueued:     b.config.MaxQueued,
		TotalAccepted: b.totalAccepted,
		TotalRejected: b.totalRejected,
		TotalTimeout:  b.totalTimeout,
	}
}

// ResetStats resets statistics.
func (b *Bulkhead) ResetStats() {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.totalAccepted = 0
	b.totalRejected = 0
	b.totalTimeout = 0
}

// Available returns the number of available execution slots.
func (b *Bulkhead) Available() int {
	return b.config.MaxConcurrent - b.active
}

// IsAvailable returns true if there's at least one available slot.
func (b *Bulkhead) IsAvailable() bool {
	return b.active < b.config.MaxConcurrent
}

// ============================================
// Bulkhead Manager
// ============================================

// BulkheadManager manages multiple bulkheads by name.
type BulkheadManager struct {
	mu            sync.RWMutex
	bulkheads     map[string]*Bulkhead
	defaultConfig BulkheadConfig
}

// NewBulkheadManager creates a new manager.
func NewBulkheadManager(defaultConfig BulkheadConfig) *BulkheadManager {
	return &BulkheadManager{
		bulkheads:     make(map[string]*Bulkhead),
		defaultConfig: defaultConfig,
	}
}

// Get gets or creates a bulkhead by name.
func (m *BulkheadManager) Get(name string, config *BulkheadConfig) *Bulkhead {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, exists := m.bulkheads[name]; !exists {
		cfg := m.defaultConfig
		if config != nil {
			cfg = *config
		}
		m.bulkheads[name] = NewBulkhead(cfg)
	}

	return m.bulkheads[name]
}

// GetAllStats returns statistics for all bulkheads.
func (m *BulkheadManager) GetAllStats() map[string]BulkheadStats {
	m.mu.RLock()
	defer m.mu.RUnlock()

	stats := make(map[string]BulkheadStats)
	for name, bulkhead := range m.bulkheads {
		stats[name] = bulkhead.GetStats()
	}
	return stats
}

// ResetAllStats resets all statistics.
func (m *BulkheadManager) ResetAllStats() {
	m.mu.RLock()
	defer m.mu.RUnlock()

	for _, bulkhead := range m.bulkheads {
		bulkhead.ResetStats()
	}
}

// ============================================
// Predefined Bulkheads
// ============================================

// APIBulkhead creates a bulkhead for API calls.
func APIBulkhead(maxConcurrent int) *Bulkhead {
	return NewBulkhead(BulkheadConfig{
		MaxConcurrent: maxConcurrent,
		MaxQueued:     100,
	})
}

// DatabaseBulkhead creates a bulkhead for database operations.
func DatabaseBulkhead(maxConcurrent int) *Bulkhead {
	return NewBulkhead(BulkheadConfig{
		MaxConcurrent: maxConcurrent,
		MaxQueued:     50,
		Timeout:       30 * time.Second,
	})
}

// StrictBulkhead creates a bulkhead with no queuing.
func StrictBulkhead(maxConcurrent int) *Bulkhead {
	return NewBulkhead(BulkheadConfig{
		MaxConcurrent: maxConcurrent,
		MaxQueued:     0,
	})
}
