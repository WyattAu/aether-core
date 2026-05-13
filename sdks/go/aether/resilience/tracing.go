//go:build !otel

// Package resilience provides tracing integration with OpenTelemetry.
package resilience

import (
	"context"
	"time"
)

// TracingEnabled indicates if OpenTelemetry tracing is available.
var TracingEnabled = false

func init() {
	// Check if OpenTelemetry is available at runtime
	// The actual check happens when trying to use the tracer
}

// TracingContext provides a context for tracing operations.
type TracingContext struct {
	spanName   string
	attributes  map[string]interface{}
	startTime  time.Time
	enabled    bool
}

// StartSpan creates a new tracing context.
func StartSpan(ctx context.Context, spanName string, attrs ...map[string]interface{}) (*TracingContext, context.Context) {
	tc := &TracingContext{
		spanName:  spanName,
		startTime: time.Now(),
		enabled:   TracingEnabled,
	}
	
	if len(attrs) > 0 {
		tc.attributes = attrs[0]
	}
	
	// Attempt to use OpenTelemetry if available.
	// Importing otel packages is handled via build tags to avoid
	// hard dependencies when OTel is not used.
	return tc, ctx
}

// End completes the tracing context.
func (tc *TracingContext) End(err error) {
	if !tc.enabled {
		return
	}
	
	duration := time.Since(tc.startTime)
	_ = duration // Use in actual implementation
}

// SetAttribute sets an attribute on the span.
func (tc *TracingContext) SetAttribute(key string, value interface{}) {
	if tc.attributes == nil {
		tc.attributes = make(map[string]interface{})
	}
	tc.attributes[key] = value
}

// AddEvent adds an event to the span.
func (tc *TracingContext) AddEvent(name string, attrs ...map[string]interface{}) {
	// Placeholder for event recording
}

// ============================================
// Instrumented Operations
// ============================================

// InstrumentCircuitBreaker adds tracing to circuit breaker operations.
func InstrumentCircuitBreaker(name string, state string, fn func() error) error {
	ctx := context.Background()
	tc, _ := StartSpan(ctx, "circuit_breaker."+name, map[string]interface{}{
		"circuit_breaker.name":  name,
		"circuit_breaker.state": state,
	})
	defer tc.End(nil)
	
	err := fn()
	if err != nil {
		tc.SetAttribute("circuit_breaker.result", "error")
		return err
	}
	
	tc.SetAttribute("circuit_breaker.result", "success")
	return nil
}

// InstrumentRetry adds tracing to retry operations.
func InstrumentRetry(name string, attempt int, maxAttempts int, fn func() error) error {
	ctx := context.Background()
	tc, _ := StartSpan(ctx, "retry."+name, map[string]interface{}{
		"retry.name":         name,
		"retry.attempt":      attempt,
		"retry.max_attempts": maxAttempts,
	})
	defer tc.End(nil)
	
	err := fn()
	if err != nil {
		tc.SetAttribute("retry.result", "error")
		return err
	}
	
	tc.SetAttribute("retry.result", "success")
	return nil
}

// InstrumentRateLimiter adds tracing to rate limiter operations.
func InstrumentRateLimiter(name string, allowed bool, fn func() error) error {
	ctx := context.Background()
	tc, _ := StartSpan(ctx, "rate_limiter."+name, map[string]interface{}{
		"rate_limiter.name":    name,
		"rate_limiter.allowed": allowed,
	})
	defer tc.End(nil)
	
	err := fn()
	if err != nil {
		tc.SetAttribute("rate_limiter.result", "error")
		return err
	}
	
	tc.SetAttribute("rate_limiter.result", "success")
	return nil
}

// InstrumentBulkhead adds tracing to bulkhead operations.
func InstrumentBulkhead(name string, active int, maxConcurrent int, fn func() error) error {
	ctx := context.Background()
	tc, _ := StartSpan(ctx, "bulkhead."+name, map[string]interface{}{
		"bulkhead.name":           name,
		"bulkhead.active":         active,
		"bulkhead.max_concurrent": maxConcurrent,
	})
	defer tc.End(nil)
	
	err := fn()
	if err != nil {
		tc.SetAttribute("bulkhead.result", "error")
		return err
	}
	
	tc.SetAttribute("bulkhead.result", "success")
	return nil
}

// ============================================
// OpenTelemetry Integration Helpers
// ============================================

// TracingConfig holds configuration for tracing.
type TracingConfig struct {
	// Enabled enables OpenTelemetry tracing
	Enabled bool
	// ServiceName is the name reported to the tracer
	ServiceName string
	// SampleRate is the sampling rate (0.0 to 1.0)
	SampleRate float64
}

// DefaultTracingConfig returns default tracing configuration.
func DefaultTracingConfig() TracingConfig {
	return TracingConfig{
		Enabled:     false,
		ServiceName: "aether-resilience",
		SampleRate:  1.0,
	}
}

// ResilienceInstrumentation provides tracing utilities.
type ResilienceInstrumentation struct {
	config TracingConfig
}

// NewResilienceInstrumentation creates a new instrumentation instance.
func NewResilienceInstrumentation(config TracingConfig) *ResilienceInstrumentation {
	return &ResilienceInstrumentation{
		config: config,
	}
}

// TraceCircuitBreaker creates a tracing context for circuit breaker operations.
func (ri *ResilienceInstrumentation) TraceCircuitBreaker(ctx context.Context, name, state, operation string) (*TracingContext, context.Context) {
	return StartSpan(ctx, "circuit_breaker."+name+"."+operation, map[string]interface{}{
		"circuit_breaker.name":  name,
		"circuit_breaker.state": state,
	})
}

// TraceRetry creates a tracing context for retry operations.
func (ri *ResilienceInstrumentation) TraceRetry(ctx context.Context, name string, attempt, maxAttempts int, operation string) (*TracingContext, context.Context) {
	return StartSpan(ctx, "retry."+name+"."+operation, map[string]interface{}{
		"retry.name":         name,
		"retry.attempt":      attempt,
		"retry.max_attempts": maxAttempts,
	})
}

// TraceRateLimiter creates a tracing context for rate limiter operations.
func (ri *ResilienceInstrumentation) TraceRateLimiter(ctx context.Context, name, operation string, rps int) (*TracingContext, context.Context) {
	return StartSpan(ctx, "rate_limiter."+name+"."+operation, map[string]interface{}{
		"rate_limiter.name":              name,
		"rate_limiter.requests_per_second": rps,
	})
}

// TraceBulkhead creates a tracing context for bulkhead operations.
func (ri *ResilienceInstrumentation) TraceBulkhead(ctx context.Context, name, operation string, active, maxConcurrent int) (*TracingContext, context.Context) {
	return StartSpan(ctx, "bulkhead."+name+"."+operation, map[string]interface{}{
		"bulkhead.name":           name,
		"bulkhead.active":         active,
		"bulkhead.max_concurrent": maxConcurrent,
	})
}

// TraceHealthCheck creates a tracing context for health check operations.
func (ri *ResilienceInstrumentation) TraceHealthCheck(ctx context.Context, name, checkName string) (*TracingContext, context.Context) {
	return StartSpan(ctx, "health_check."+name+"."+checkName, map[string]interface{}{
		"health_check.name":  name,
		"health_check.check": checkName,
	})
}
