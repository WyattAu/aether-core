package resilience

import (
	"context"
	"encoding/json"
	"errors"
	"runtime"
	"sync"
	"time"
)

// HealthStatus represents the health status of a component.
type HealthStatus string

const (
	// StatusHealthy means the component is healthy.
	StatusHealthy HealthStatus = "healthy"
	// StatusDegraded means the component is degraded but functional.
	StatusDegraded HealthStatus = "degraded"
	// StatusUnhealthy means the component is unhealthy.
	StatusUnhealthy HealthStatus = "unhealthy"
)

// HealthCheckResult represents the result of a single health check.
type HealthCheckResult struct {
	Status        HealthStatus      `json:"status"`
	ComponentID   string            `json:"componentId"`
	ComponentType string            `json:"componentType"`
	ObservedValue any               `json:"observedValue,omitempty"`
	ObservedUnit  string            `json:"observedUnit,omitempty"`
	Output        string            `json:"output,omitempty"`
	Time          string            `json:"time"`
	Details       map[string]any    `json:"details,omitempty"`
}

// HealthReport represents the overall health report.
type HealthReport struct {
	Status    HealthStatus               `json:"status"`
	Version   string                     `json:"version"`
	ServiceID string                     `json:"serviceId"`
	Time      string                     `json:"time"`
	Checks    map[string]HealthCheckResult `json:"checks"`
	Uptime    int64                      `json:"uptime"`
}

// HealthCheckOptions holds options for a health check.
type HealthCheckOptions struct {
	// Timeout for the check.
	Timeout time.Duration
	// Critical check (failure = unhealthy).
	Critical bool
	// Interval to run check (0 = on demand only).
	Interval time.Duration
	// CacheDuration for result caching.
	CacheDuration time.Duration
}

// HealthCheckFn is a function that performs a health check.
type HealthCheckFn func(ctx context.Context) HealthCheckResult

type healthCheckEntry struct {
	fn            HealthCheckFn
	options       HealthCheckOptions
	lastResult    *HealthCheckResult
	lastRun       time.Time
	cancel        context.CancelFunc
}

// HealthChecker provides Kubernetes-compatible health checks.
type HealthChecker struct {
	serviceID string
	version   string
	startTime time.Time

	mu     sync.RWMutex
	checks map[string]*healthCheckEntry
}

// NewHealthChecker creates a new health checker.
func NewHealthChecker(serviceID, version string) *HealthChecker {
	return &HealthChecker{
		serviceID: serviceID,
		version:   version,
		startTime: time.Now(),
		checks:    make(map[string]*healthCheckEntry),
	}
}

// RegisterCheck registers a health check.
func (h *HealthChecker) RegisterCheck(name string, fn HealthCheckFn, options *HealthCheckOptions) {
	h.mu.Lock()
	defer h.mu.Unlock()

	opts := HealthCheckOptions{
		Timeout:       5 * time.Second,
		Critical:      false,
		Interval:      0,
		CacheDuration: 0,
	}
	if options != nil {
		opts = *options
	}

	entry := &healthCheckEntry{
		fn:      fn,
		options: opts,
	}

	h.checks[name] = entry

	// Start periodic check if interval is set
	if opts.Interval > 0 {
		ctx, cancel := context.WithCancel(context.Background())
		entry.cancel = cancel
		go h.runPeriodicCheck(ctx, name, entry)
	}
}

// UnregisterCheck unregisters a health check.
func (h *HealthChecker) UnregisterCheck(name string) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if entry, exists := h.checks[name]; exists {
		if entry.cancel != nil {
			entry.cancel()
		}
		delete(h.checks, name)
	}
}

// RunCheck runs a single health check.
func (h *HealthChecker) RunCheck(ctx context.Context, name string) HealthCheckResult {
	h.mu.RLock()
	entry, exists := h.checks[name]
	if !exists {
		h.mu.RUnlock()
		return HealthCheckResult{
			Status:        StatusUnhealthy,
			ComponentID:   name,
			ComponentType: "check",
			Output:        "Check not found",
			Time:          time.Now().UTC().Format(time.RFC3339),
		}
	}
	h.mu.RUnlock()

	// Return cached result if still valid
	if entry.options.CacheDuration > 0 && entry.lastResult != nil {
		if time.Since(entry.lastRun) < entry.options.CacheDuration {
			return *entry.lastResult
		}
	}

	// Run check with timeout
	var result HealthCheckResult
	done := make(chan struct{}, 1)

	go func() {
		checkCtx, cancel := context.WithTimeout(ctx, entry.options.Timeout)
		defer cancel()
		result = entry.fn(checkCtx)
		done <- struct{}{}
	}()

	select {
	case <-done:
		// Check completed
	case <-ctx.Done():
		result = HealthCheckResult{
			Status:        StatusUnhealthy,
			ComponentID:   name,
			ComponentType: "check",
			Output:        "check canceled",
			Time:          time.Now().UTC().Format(time.RFC3339),
		}
	}
	result.Time = time.Now().UTC().Format(time.RFC3339)

	// Cache result
	h.mu.Lock()
	entry.lastResult = &result
	entry.lastRun = time.Now()
	h.mu.Unlock()

	return result
}

// RunAll runs all health checks and generates a report.
func (h *HealthChecker) RunAll(ctx context.Context) *HealthReport {
	h.mu.RLock()
	checkNames := make([]string, 0, len(h.checks))
	for name := range h.checks {
		checkNames = append(checkNames, name)
	}
	h.mu.RUnlock()

	checkResults := make(map[string]HealthCheckResult)
	for _, name := range checkNames {
		checkResults[name] = h.RunCheck(ctx, name)
	}

	status := h.calculateOverallStatus(checkResults)

	return &HealthReport{
		Status:    status,
		Version:   h.version,
		ServiceID: h.serviceID,
		Time:      time.Now().UTC().Format(time.RFC3339),
		Checks:    checkResults,
		Uptime:    int64(time.Since(h.startTime).Seconds()),
	}
}

// GetLiveness returns liveness status (is the service alive?).
func (h *HealthChecker) GetLiveness() map[string]any {
	return map[string]any{
		"alive": true,
		"time":  time.Now().UTC().Format(time.RFC3339),
	}
}

// GetReadiness returns readiness status (is the service ready to accept traffic?).
func (h *HealthChecker) GetReadiness(ctx context.Context) map[string]any {
	report := h.RunAll(ctx)

	checks := make(map[string]bool)
	h.mu.RLock()
	for name, result := range report.Checks {
		if entry, exists := h.checks[name]; exists && entry.options.Critical {
			checks[name] = result.Status != StatusUnhealthy
		}
	}
	h.mu.RUnlock()

	ready := report.Status != StatusUnhealthy

	result := map[string]any{
		"ready": ready,
		"time":  report.Time,
	}
	if len(checks) > 0 {
		result["checks"] = checks
	}

	return result
}

// GetStartup returns startup status (has the service started?).
func (h *HealthChecker) GetStartup() map[string]any {
	return map[string]any{
		"started": true,
		"time":    time.Now().UTC().Format(time.RFC3339),
	}
}

// Shutdown stops all periodic checks.
func (h *HealthChecker) Shutdown() {
	h.mu.Lock()
	defer h.mu.Unlock()

	for _, entry := range h.checks {
		if entry.cancel != nil {
			entry.cancel()
		}
	}
	h.checks = make(map[string]*healthCheckEntry)
}

// ToJSON returns the health report as JSON.
func (r *HealthReport) ToJSON() ([]byte, error) {
	return json.Marshal(r)
}

func (h *HealthChecker) runPeriodicCheck(ctx context.Context, name string, entry *healthCheckEntry) {
	ticker := time.NewTicker(entry.options.Interval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			checkCtx, cancel := context.WithTimeout(context.Background(), entry.options.Timeout)
			result := entry.fn(checkCtx)
			cancel()

			result.Time = time.Now().UTC().Format(time.RFC3339)
			entry.lastResult = &result
			entry.lastRun = time.Now()
		}
	}
}

func (h *HealthChecker) calculateOverallStatus(checks map[string]HealthCheckResult) HealthStatus {
	hasDegraded := false
	hasUnhealthy := false

	h.mu.RLock()
	defer h.mu.RUnlock()

	for name, result := range checks {
		entry, exists := h.checks[name]
		if !exists {
			continue
		}

		if result.Status == StatusUnhealthy {
			if entry.options.Critical {
				return StatusUnhealthy
			}
			hasUnhealthy = true
		} else if result.Status == StatusDegraded {
			hasDegraded = true
		}
	}

	if hasUnhealthy || hasDegraded {
		return StatusDegraded
	}

	return StatusHealthy
}

// ============================================
// Predefined Health Checks
// ============================================

// PingHealthCheck creates a simple ping health check.
func PingHealthCheck() HealthCheckFn {
	return func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{
			Status:        StatusHealthy,
			ComponentID:   "ping",
			ComponentType: "self",
			ObservedValue: 1,
			ObservedUnit:  "ms",
			Time:          time.Now().UTC().Format(time.RFC3339),
		}
	}
}

// MemoryHealthCheck creates a memory health check.
func MemoryHealthCheck(maxHeapMB int64, warnThreshold float64) HealthCheckFn {
	return func(ctx context.Context) HealthCheckResult {
		var m runtime.MemStats
		runtime.ReadMemStats(&m)

		heapUsedMB := int64(m.HeapAlloc / (1024 * 1024))
		heapTotalMB := int64(m.HeapSys / (1024 * 1024))
		usage := float64(heapUsedMB) / float64(heapTotalMB)

		var status HealthStatus
		if heapUsedMB > maxHeapMB || usage > 0.95 {
			status = StatusUnhealthy
		} else if usage > warnThreshold {
			status = StatusDegraded
		} else {
			status = StatusHealthy
		}

		return HealthCheckResult{
			Status:        status,
			ComponentID:   "memory",
			ComponentType: "system",
			ObservedValue: heapUsedMB,
			ObservedUnit:  "MB",
			Output:        "Heap usage checked",
			Time:          time.Now().UTC().Format(time.RFC3339),
			Details: map[string]any{
				"heapUsedMB":  heapUsedMB,
				"heapTotalMB": heapTotalMB,
				"usage":       usage,
			},
		}
	}
}

// GoroutineHealthCheck creates a goroutine health check.
func GoroutineHealthCheck(maxGoroutines int, warnThreshold float64) HealthCheckFn {
	return func(ctx context.Context) HealthCheckResult {
		count := runtime.NumGoroutine()
		usage := float64(count) / float64(maxGoroutines)

		var status HealthStatus
		if count > maxGoroutines || usage > 0.95 {
			status = StatusUnhealthy
		} else if usage > warnThreshold {
			status = StatusDegraded
		} else {
			status = StatusHealthy
		}

		return HealthCheckResult{
			Status:        status,
			ComponentID:   "goroutines",
			ComponentType: "system",
			ObservedValue: count,
			ObservedUnit:  "goroutines",
			Output:        "Goroutine count checked",
			Time:          time.Now().UTC().Format(time.RFC3339),
			Details: map[string]any{
				"count":       count,
				"maxGoroutines": maxGoroutines,
				"usage":       usage,
			},
		}
	}
}

// DependencyHealthCheck creates a dependency health check.
func DependencyHealthCheck(name string, checkFn func(ctx context.Context) error, timeout time.Duration) HealthCheckFn {
	return func(ctx context.Context) HealthCheckResult {
		checkCtx, cancel := context.WithTimeout(ctx, timeout)
		defer cancel()

		start := time.Now()
		err := checkFn(checkCtx)
		latency := time.Since(start)

		if err != nil {
			return HealthCheckResult{
				Status:        StatusUnhealthy,
				ComponentID:   name,
				ComponentType: "dependency",
				Output:        err.Error(),
				Time:          time.Now().UTC().Format(time.RFC3339),
			}
		}

		var status HealthStatus
		if latency > 1*time.Second {
			status = StatusDegraded
		} else {
			status = StatusHealthy
		}

		return HealthCheckResult{
			Status:        status,
			ComponentID:   name,
			ComponentType: "dependency",
			ObservedValue: latency.Milliseconds(),
			ObservedUnit:  "ms",
			Time:          time.Now().UTC().Format(time.RFC3339),
		}
	}
}

// StateStorageHealthCheck creates a state storage health check.
func StateStorageHealthCheck(readFn func(ctx context.Context, key string) ([]byte, error), testKey string) HealthCheckFn {
	return func(ctx context.Context) HealthCheckResult {
		start := time.Now()
		_, err := readFn(ctx, testKey)
		latency := time.Since(start)

		if err != nil && !errors.Is(err, context.Canceled) {
			return HealthCheckResult{
				Status:        StatusUnhealthy,
				ComponentID:   "state-storage",
				ComponentType: "storage",
				Output:        err.Error(),
				Time:          time.Now().UTC().Format(time.RFC3339),
			}
		}

		var status HealthStatus
		if latency > 1*time.Second {
			status = StatusDegraded
		} else {
			status = StatusHealthy
		}

		return HealthCheckResult{
			Status:        status,
			ComponentID:   "state-storage",
			ComponentType: "storage",
			ObservedValue: latency.Milliseconds(),
			ObservedUnit:  "ms",
			Output:        "State storage accessible",
			Time:          time.Now().UTC().Format(time.RFC3339),
		}
	}
}
