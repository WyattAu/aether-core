package resilience

import (
	"context"
	"errors"
	"sync"
	"time"
)

// RateLimitStrategy defines the rate limiting algorithm.
type RateLimitStrategy int

const (
	// StrategyTokenBucket allows bursts up to bucket size.
	StrategyTokenBucket RateLimitStrategy = iota
	// StrategySlidingWindow provides smooth rate limiting.
	StrategySlidingWindow
	// StrategyFixedWindow uses simple window-based limiting.
	StrategyFixedWindow
)

// RateLimitConfig holds configuration for the rate limiter.
type RateLimitConfig struct {
	// RequestsPerSecond is the maximum requests per second.
	RequestsPerSecond int
	// BurstSize is the maximum burst size (for token bucket).
	BurstSize int
	// Strategy is the rate limiting algorithm.
	Strategy RateLimitStrategy
	// WindowSize is the window duration (for window strategies).
	WindowSize time.Duration
}

// DefaultRateLimitConfig returns the default configuration.
func DefaultRateLimitConfig() RateLimitConfig {
	return RateLimitConfig{
		RequestsPerSecond: 100,
		BurstSize:         100,
		Strategy:          StrategyTokenBucket,
		WindowSize:        time.Second,
	}
}

// RateLimitStats holds statistics for the rate limiter.
type RateLimitStats struct {
	AllowedRequests  int64
	RejectedRequests int64
	CurrentRate      float64
	WaitTime         time.Duration
}

// RateLimitResult is the result of a rate limit check.
type RateLimitResult struct {
	Allowed        bool
	WaitTime       time.Duration
	Remaining      int
	ResetIn        time.Duration
}

// RateLimitExhaustedError is returned when rate limit is exceeded.
var RateLimitExhaustedError = errors.New("rate limit exceeded")

// ============================================
// Token Bucket Implementation
// ============================================

type tokenBucket struct {
	maxTokens  float64
	tokens     float64
	refillRate float64 // tokens per nanosecond
	lastRefill time.Time
	mu         sync.Mutex
}

func newTokenBucket(requestsPerSecond int, burstSize int) *tokenBucket {
	return &tokenBucket{
		maxTokens:  float64(burstSize),
		tokens:     float64(burstSize),
		refillRate: float64(requestsPerSecond) / float64(time.Second),
		lastRefill: time.Now(),
	}
}

func (tb *tokenBucket) tryAcquire(tokens int) RateLimitResult {
	tb.mu.Lock()
	defer tb.mu.Unlock()

	tb.refill()

	if tb.tokens >= float64(tokens) {
		tb.tokens -= float64(tokens)
		return RateLimitResult{
			Allowed:   true,
			WaitTime:  0,
			Remaining: int(tb.tokens),
		}
	}

	// Calculate wait time
	tokensNeeded := float64(tokens) - tb.tokens
	waitTime := time.Duration(tokensNeeded / tb.refillRate)

	return RateLimitResult{
		Allowed:   false,
		WaitTime:  waitTime,
		Remaining: int(tb.tokens),
	}
}

func (tb *tokenBucket) refill() {
	now := time.Now()
	elapsed := now.Sub(tb.lastRefill)
	tokensToAdd := elapsed.Seconds() * tb.refillRate * float64(time.Second.Nanoseconds()) / float64(time.Second.Nanoseconds())
	tb.tokens = min(tb.maxTokens, tb.tokens+tokensToAdd*float64(time.Second))
	tb.lastRefill = now
}

func (tb *tokenBucket) getTokens() int {
	tb.mu.Lock()
	defer tb.mu.Unlock()
	tb.refill()
	return int(tb.tokens)
}

// ============================================
// Sliding Window Implementation
// ============================================

type slidingWindow struct {
	maxRequests int
	windowSize  time.Duration
	requests    []time.Time
	mu          sync.Mutex
}

func newSlidingWindow(requestsPerSecond int, windowSize time.Duration) *slidingWindow {
	return &slidingWindow{
		maxRequests: requestsPerSecond,
		windowSize:  windowSize,
		requests:    make([]time.Time, 0),
	}
}

func (sw *slidingWindow) tryAcquire() RateLimitResult {
	sw.mu.Lock()
	defer sw.mu.Unlock()

	now := time.Now()
	windowStart := now.Add(-sw.windowSize)

	// Remove old requests
	newRequests := make([]time.Time, 0)
	for _, t := range sw.requests {
		if t.After(windowStart) {
			newRequests = append(newRequests, t)
		}
	}
	sw.requests = newRequests

	if len(sw.requests) < sw.maxRequests {
		sw.requests = append(sw.requests, now)
		return RateLimitResult{
			Allowed:   true,
			WaitTime:  0,
			Remaining: sw.maxRequests - len(sw.requests),
		}
	}

	// Calculate wait time
	oldestRequest := sw.requests[0]
	waitTime := oldestRequest.Add(sw.windowSize).Sub(now)

	return RateLimitResult{
		Allowed:   false,
		WaitTime:  waitTime,
		Remaining: 0,
		ResetIn:   waitTime,
	}
}

func (sw *slidingWindow) getCurrentCount() int {
	sw.mu.Lock()
	defer sw.mu.Unlock()

	now := time.Now()
	windowStart := now.Add(-sw.windowSize)
	count := 0
	for _, t := range sw.requests {
		if t.After(windowStart) {
			count++
		}
	}
	return count
}

// ============================================
// Fixed Window Implementation
// ============================================

type fixedWindow struct {
	maxRequests int
	windowSize  time.Duration
	count       int
	windowStart time.Time
	mu          sync.Mutex
}

func newFixedWindow(requestsPerSecond int, windowSize time.Duration) *fixedWindow {
	return &fixedWindow{
		maxRequests: requestsPerSecond,
		windowSize:  windowSize,
		windowStart: time.Now(),
	}
}

func (fw *fixedWindow) tryAcquire() RateLimitResult {
	fw.mu.Lock()
	defer fw.mu.Unlock()

	now := time.Now()

	// Reset window if needed
	if now.Sub(fw.windowStart) >= fw.windowSize {
		fw.count = 0
		fw.windowStart = now
	}

	if fw.count < fw.maxRequests {
		fw.count++
		return RateLimitResult{
			Allowed:   true,
			WaitTime:  0,
			Remaining: fw.maxRequests - fw.count,
			ResetIn:   fw.windowStart.Add(fw.windowSize).Sub(now),
		}
	}

	return RateLimitResult{
		Allowed:   false,
		WaitTime:  fw.windowStart.Add(fw.windowSize).Sub(now),
		Remaining: 0,
		ResetIn:   fw.windowStart.Add(fw.windowSize).Sub(now),
	}
}

func (fw *fixedWindow) getCurrentCount() int {
	return fw.count
}

// ============================================
// Rate Limiter
// ============================================

// RateLimiter implements rate limiting with multiple strategies.
type RateLimiter struct {
	config    RateLimitConfig
	impl      interface{}
	allowed   int64
	rejected  int64
	mu        sync.Mutex
}

// NewRateLimiter creates a new rate limiter.
func NewRateLimiter(config RateLimitConfig) *RateLimiter {
	if config.RequestsPerSecond == 0 {
		config.RequestsPerSecond = 100
	}
	if config.BurstSize == 0 {
		config.BurstSize = config.RequestsPerSecond
	}
	if config.WindowSize == 0 {
		config.WindowSize = time.Second
	}

	var impl interface{}
	switch config.Strategy {
	case StrategyTokenBucket:
		impl = newTokenBucket(config.RequestsPerSecond, config.BurstSize)
	case StrategySlidingWindow:
		impl = newSlidingWindow(config.RequestsPerSecond, config.WindowSize)
	case StrategyFixedWindow:
		impl = newFixedWindow(config.RequestsPerSecond, config.WindowSize)
	default:
		impl = newTokenBucket(config.RequestsPerSecond, config.BurstSize)
	}

	return &RateLimiter{
		config: config,
		impl:   impl,
	}
}

// TryAcquire attempts to acquire permission without waiting.
func (rl *RateLimiter) TryAcquire(tokens ...int) RateLimitResult {
	tokenCount := 1
	if len(tokens) > 0 {
		tokenCount = tokens[0]
	}

	var result RateLimitResult
	switch impl := rl.impl.(type) {
	case *tokenBucket:
		result = impl.tryAcquire(tokenCount)
	case *slidingWindow:
		result = impl.tryAcquire()
	case *fixedWindow:
		result = impl.tryAcquire()
	}

	rl.mu.Lock()
	if result.Allowed {
		rl.allowed++
	} else {
		rl.rejected++
	}
	rl.mu.Unlock()

	return result
}

// Acquire acquires permission, waiting if necessary.
func (rl *RateLimiter) Acquire(ctx context.Context, maxWait time.Duration) error {
	result := rl.TryAcquire()

	if result.Allowed {
		return nil
	}

	if result.WaitTime > maxWait {
		rl.mu.Lock()
		rl.rejected++
		rl.mu.Unlock()
		return RateLimitExhaustedError
	}

	select {
	case <-time.After(result.WaitTime):
	case <-ctx.Done():
		return ctx.Err()
	}

	// Try again
	retryResult := rl.TryAcquire()
	if !retryResult.Allowed {
		return RateLimitExhaustedError
	}

	return nil
}

// Execute runs a function with rate limiting.
func (rl *RateLimiter) Execute(ctx context.Context, fn func(ctx context.Context) (any, error), maxWait time.Duration) (any, error) {
	if err := rl.Acquire(ctx, maxWait); err != nil {
		return nil, err
	}
	return fn(ctx)
}

// GetStats returns current statistics.
func (rl *RateLimiter) GetStats() RateLimitStats {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	var currentRate float64
	switch impl := rl.impl.(type) {
	case *tokenBucket:
		if rl.config.BurstSize > 0 {
			currentRate = float64(rl.config.RequestsPerSecond) * (1 - float64(impl.getTokens())/float64(rl.config.BurstSize))
		}
	case *slidingWindow:
		currentRate = float64(impl.getCurrentCount())
	case *fixedWindow:
		currentRate = float64(impl.getCurrentCount())
	}

	return RateLimitStats{
		AllowedRequests:  rl.allowed,
		RejectedRequests: rl.rejected,
		CurrentRate:      currentRate,
	}
}

// ResetStats resets statistics.
func (rl *RateLimiter) ResetStats() {
	rl.mu.Lock()
	defer rl.mu.Unlock()
	rl.allowed = 0
	rl.rejected = 0
}

// ============================================
// Rate Limiter Manager
// ============================================

// RateLimiterManager manages multiple rate limiters by name.
type RateLimiterManager struct {
	mu            sync.RWMutex
	limiters      map[string]*RateLimiter
	defaultConfig RateLimitConfig
}

// NewRateLimiterManager creates a new manager.
func NewRateLimiterManager(defaultConfig RateLimitConfig) *RateLimiterManager {
	return &RateLimiterManager{
		limiters:      make(map[string]*RateLimiter),
		defaultConfig: defaultConfig,
	}
}

// Get gets or creates a rate limiter by name.
func (m *RateLimiterManager) Get(name string, config *RateLimitConfig) *RateLimiter {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, exists := m.limiters[name]; !exists {
		cfg := m.defaultConfig
		if config != nil {
			cfg = *config
		}
		m.limiters[name] = NewRateLimiter(cfg)
	}

	return m.limiters[name]
}

// GetAllStats returns statistics for all rate limiters.
func (m *RateLimiterManager) GetAllStats() map[string]RateLimitStats {
	m.mu.RLock()
	defer m.mu.RUnlock()

	stats := make(map[string]RateLimitStats)
	for name, limiter := range m.limiters {
		stats[name] = limiter.GetStats()
	}
	return stats
}

// ResetAllStats resets all statistics.
func (m *RateLimiterManager) ResetAllStats() {
	m.mu.RLock()
	defer m.mu.RUnlock()

	for _, limiter := range m.limiters {
		limiter.ResetStats()
	}
}

// ============================================
// Predefined Rate Limiters
// ============================================

// APIRateLimiter creates a rate limiter for API requests.
func APIRateLimiter() *RateLimiter {
	return NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: 100,
		BurstSize:         200,
		Strategy:          StrategyTokenBucket,
	})
}

// StrictRateLimiter creates a rate limiter with no burst allowance.
func StrictRateLimiter(requestsPerSecond int) *RateLimiter {
	return NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: requestsPerSecond,
		Strategy:          StrategySlidingWindow,
	})
}

// BurstyRateLimiter creates a rate limiter for bursty traffic.
func BurstyRateLimiter(burstSize int, refillRate int) *RateLimiter {
	return NewRateLimiter(RateLimitConfig{
		RequestsPerSecond: refillRate,
		BurstSize:         burstSize,
		Strategy:          StrategyTokenBucket,
	})
}

func min(a, b float64) float64 {
	if a < b {
		return a
	}
	return b
}
