package resilience

import (
	"context"
	"errors"
	"sync"
	"time"
)

// CircuitState represents the state of a circuit breaker.
type CircuitState int

const (
	// StateClosed means the circuit is closed and requests pass through.
	StateClosed CircuitState = iota
	// StateOpen means the circuit is open and requests are rejected.
	StateOpen
	// StateHalfOpen means the circuit is testing if the service recovered.
	StateHalfOpen
)

func (s CircuitState) String() string {
	switch s {
	case StateClosed:
		return "closed"
	case StateOpen:
		return "open"
	case StateHalfOpen:
		return "half-open"
	default:
		return "unknown"
	}
}

// CircuitBreakerConfig holds configuration for the circuit breaker.
type CircuitBreakerConfig struct {
	// FailureThreshold is the number of failures before opening.
	FailureThreshold int
	// SuccessThreshold is the number of successes before closing from half-open.
	SuccessThreshold int
	// Timeout is how long to wait before attempting reset.
	Timeout time.Duration
	// HalfOpenMaxCalls is the max calls allowed in half-open state.
	HalfOpenMaxCalls int
	// FailureWindow is the time window for counting failures (0 = no window).
	FailureWindow time.Duration
	// OnOpen is called when circuit opens.
	OnOpen func()
	// OnClose is called when circuit closes.
	OnClose func()
	// OnHalfOpen is called when circuit enters half-open.
	OnHalfOpen func()
}

// DefaultCircuitBreakerConfig returns the default configuration.
func DefaultCircuitBreakerConfig() CircuitBreakerConfig {
	return CircuitBreakerConfig{
		FailureThreshold: 5,
		SuccessThreshold: 3,
		Timeout:          30 * time.Second,
		HalfOpenMaxCalls: 3,
		FailureWindow:    time.Minute,
	}
}

// CircuitBreakerStats holds statistics for the circuit breaker.
type CircuitBreakerStats struct {
	State           CircuitState
	Failures        int
	Successes       int
	RejectedCalls   int64
	TotalCalls      int64
	LastFailure     time.Time
	LastSuccess     time.Time
	LastStateChange time.Time
}

// CircuitBreakerError is returned when the circuit is open.
var CircuitBreakerError = errors.New("circuit breaker is open")

// failureRecord tracks a failure for window-based counting.
type failureRecord struct {
	timestamp time.Time
	err       error
}

// CircuitBreaker implements the circuit breaker pattern.
type CircuitBreaker struct {
	config CircuitBreakerConfig

	mu              sync.RWMutex
	state           CircuitState
	failures        int
	successes       int
	rejectedCalls   int64
	totalCalls      int64
	halfOpenCalls   int
	lastFailure     time.Time
	lastSuccess     time.Time
	lastStateChange time.Time
	failureHistory  []failureRecord
}

// NewCircuitBreaker creates a new circuit breaker.
func NewCircuitBreaker(config CircuitBreakerConfig) *CircuitBreaker {
	if config.FailureThreshold == 0 {
		config.FailureThreshold = 5
	}
	if config.SuccessThreshold == 0 {
		config.SuccessThreshold = 3
	}
	if config.Timeout == 0 {
		config.Timeout = 30 * time.Second
	}
	if config.HalfOpenMaxCalls == 0 {
		config.HalfOpenMaxCalls = 3
	}

	return &CircuitBreaker{
		config:         config,
		state:          StateClosed,
		failureHistory: make([]failureRecord, 0),
	}
}

// State returns the current circuit state.
func (cb *CircuitBreaker) State() CircuitState {
	cb.mu.RLock()
	defer cb.mu.RUnlock()
	return cb.state
}

// IsClosed returns true if the circuit is closed.
func (cb *CircuitBreaker) IsClosed() bool {
	return cb.State() == StateClosed
}

// IsOpen returns true if the circuit is open.
func (cb *CircuitBreaker) IsOpen() bool {
	return cb.State() == StateOpen
}

// IsHalfOpen returns true if the circuit is half-open.
func (cb *CircuitBreaker) IsHalfOpen() bool {
	return cb.State() == StateHalfOpen
}

// GetStats returns current statistics.
func (cb *CircuitBreaker) GetStats() CircuitBreakerStats {
	cb.mu.RLock()
	defer cb.mu.RUnlock()

	return CircuitBreakerStats{
		State:           cb.state,
		Failures:        cb.failures,
		Successes:       cb.successes,
		RejectedCalls:   cb.rejectedCalls,
		TotalCalls:      cb.totalCalls,
		LastFailure:     cb.lastFailure,
		LastSuccess:     cb.lastSuccess,
		LastStateChange: cb.lastStateChange,
	}
}

// Execute runs the given function through the circuit breaker.
func (cb *CircuitBreaker) Execute(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	cb.mu.Lock()
	cb.totalCalls++

	// Check if we should transition from open to half-open
	if cb.state == StateOpen {
		if cb.shouldAttemptResetLocked() {
			cb.transitionToLocked(StateHalfOpen)
		} else {
			cb.rejectedCalls++
			cb.mu.Unlock()
			return nil, CircuitBreakerError
		}
	}

	// Check half-open call limit
	if cb.state == StateHalfOpen && cb.halfOpenCalls >= cb.config.HalfOpenMaxCalls {
		cb.rejectedCalls++
		cb.mu.Unlock()
		return nil, CircuitBreakerError
	}

	// Increment half-open calls if in that state
	if cb.state == StateHalfOpen {
		cb.halfOpenCalls++
	}
	cb.mu.Unlock()

	// Execute the function
	result, err := fn(ctx)

	cb.mu.Lock()
	defer cb.mu.Unlock()

	if err != nil {
		cb.onFailureLocked(err)
		return nil, err
	}

	cb.onSuccessLocked()
	return result, nil
}

// ForceOpen forces the circuit to open state.
func (cb *CircuitBreaker) ForceOpen() {
	cb.mu.Lock()
	defer cb.mu.Unlock()
	cb.lastFailure = time.Now()
	cb.transitionToLocked(StateOpen)
}

// ForceClose forces the circuit to closed state.
func (cb *CircuitBreaker) ForceClose() {
	cb.mu.Lock()
	defer cb.mu.Unlock()
	cb.transitionToLocked(StateClosed)
}

// Reset resets all statistics and state.
func (cb *CircuitBreaker) Reset() {
	cb.mu.Lock()
	defer cb.mu.Unlock()

	cb.failures = 0
	cb.successes = 0
	cb.rejectedCalls = 0
	cb.totalCalls = 0
	cb.halfOpenCalls = 0
	cb.failureHistory = nil
	cb.transitionToLocked(StateClosed)
}

func (cb *CircuitBreaker) shouldAttemptResetLocked() bool {
	if cb.lastFailure.IsZero() {
		return true
	}
	return time.Since(cb.lastFailure) >= cb.config.Timeout
}

func (cb *CircuitBreaker) onSuccessLocked() {
	cb.lastSuccess = time.Now()
	cb.failureHistory = nil

	if cb.state == StateHalfOpen {
		cb.successes++
		if cb.successes >= cb.config.SuccessThreshold {
			cb.transitionToLocked(StateClosed)
		}
	} else if cb.state == StateClosed {
		cb.failures = 0
	}
}

func (cb *CircuitBreaker) onFailureLocked(err error) {
	cb.lastFailure = time.Now()
	cb.failures++

	// Record failure for window
	cb.failureHistory = append(cb.failureHistory, failureRecord{
		timestamp: time.Now(),
		err:       err,
	})

	// Clean old failures outside window
	if cb.config.FailureWindow > 0 {
		cutoff := time.Now().Add(-cb.config.FailureWindow)
		newHistory := make([]failureRecord, 0)
		for _, f := range cb.failureHistory {
			if f.timestamp.After(cutoff) {
				newHistory = append(newHistory, f)
			}
		}
		cb.failureHistory = newHistory
	}

	if cb.state == StateHalfOpen {
		// Any failure in half-open immediately opens
		cb.transitionToLocked(StateOpen)
	} else if cb.state == StateClosed {
		// Check if we should open based on failure count
		failureCount := cb.failures
		if cb.config.FailureWindow > 0 {
			failureCount = len(cb.failureHistory)
		}

		if failureCount >= cb.config.FailureThreshold {
			cb.transitionToLocked(StateOpen)
		}
	}
}

func (cb *CircuitBreaker) transitionToLocked(newState CircuitState) {
	if cb.state == newState {
		return
	}

	oldState := cb.state
	cb.state = newState
	cb.lastStateChange = time.Now()

	// Reset counters on state change
	if newState == StateClosed {
		cb.failures = 0
		cb.successes = 0
		cb.halfOpenCalls = 0
		cb.failureHistory = nil
		if cb.config.OnClose != nil {
			go cb.config.OnClose()
		}
	} else if newState == StateOpen {
		cb.successes = 0
		cb.halfOpenCalls = 0
		if cb.config.OnOpen != nil {
			go cb.config.OnOpen()
		}
	} else if newState == StateHalfOpen {
		cb.successes = 0
		cb.halfOpenCalls = 0
		if cb.config.OnHalfOpen != nil {
			go cb.config.OnHalfOpen()
		}
	}

	_ = oldState // Avoid unused variable warning
}

// CircuitBreakerManager manages multiple circuit breakers by name.
type CircuitBreakerManager struct {
	mu             sync.RWMutex
	breakers       map[string]*CircuitBreaker
	defaultConfig  CircuitBreakerConfig
}

// NewCircuitBreakerManager creates a new manager.
func NewCircuitBreakerManager(defaultConfig CircuitBreakerConfig) *CircuitBreakerManager {
	return &CircuitBreakerManager{
		breakers:      make(map[string]*CircuitBreaker),
		defaultConfig: defaultConfig,
	}
}

// Get gets or creates a circuit breaker by name.
func (m *CircuitBreakerManager) Get(name string, config *CircuitBreakerConfig) *CircuitBreaker {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, exists := m.breakers[name]; !exists {
		cfg := m.defaultConfig
		if config != nil {
			cfg = *config
		}
		m.breakers[name] = NewCircuitBreaker(cfg)
	}

	return m.breakers[name]
}

// GetAllStats returns statistics for all circuit breakers.
func (m *CircuitBreakerManager) GetAllStats() map[string]CircuitBreakerStats {
	m.mu.RLock()
	defer m.mu.RUnlock()

	stats := make(map[string]CircuitBreakerStats)
	for name, breaker := range m.breakers {
		stats[name] = breaker.GetStats()
	}
	return stats
}

// ResetAll resets all circuit breakers.
func (m *CircuitBreakerManager) ResetAll() {
	m.mu.RLock()
	defer m.mu.RUnlock()

	for _, breaker := range m.breakers {
		breaker.Reset()
	}
}

// GetOpenBreakers returns names of all open circuit breakers.
func (m *CircuitBreakerManager) GetOpenBreakers() []string {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var open []string
	for name, breaker := range m.breakers {
		if breaker.IsOpen() {
			open = append(open, name)
		}
	}
	return open
}
