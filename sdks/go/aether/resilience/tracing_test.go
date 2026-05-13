package resilience

import (
	"context"
	"errors"
	"testing"
)

func TestStartSpan_NonNil(t *testing.T) {
	tc, ctx := StartSpan(context.Background(), "test.span")
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
}

func TestStartSpan_WithAttributes(t *testing.T) {
	attrs := map[string]interface{}{
		"key": "value",
	}
	tc, _ := StartSpan(context.Background(), "test.span", attrs)
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if tc.attributes["key"] != "value" {
		t.Errorf("expected attribute key=value, got %v", tc.attributes["key"])
	}
}

func TestEnd_NoPanic(t *testing.T) {
	tc, _ := StartSpan(context.Background(), "test.span")
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("End() panicked: %v", r)
		}
	}()
	tc.End(nil)
}

func TestEnd_WithError(t *testing.T) {
	tc, _ := StartSpan(context.Background(), "test.span")
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("End(err) panicked: %v", r)
		}
	}()
	tc.End(errors.New("test error"))
}

func TestSetAttribute(t *testing.T) {
	tc, _ := StartSpan(context.Background(), "test.span")
	tc.SetAttribute("foo", "bar")
	if tc.attributes["foo"] != "bar" {
		t.Errorf("expected foo=bar, got %v", tc.attributes["foo"])
	}
	tc.SetAttribute("count", 42)
	if tc.attributes["count"] != 42 {
		t.Errorf("expected count=42, got %v", tc.attributes["count"])
	}
}

func TestSetAttribute_NilAttributes(t *testing.T) {
	tc := &TracingContext{}
	tc.SetAttribute("key", "value")
	if tc.attributes["key"] != "value" {
		t.Errorf("expected key=value, got %v", tc.attributes["key"])
	}
}

func TestAddEvent_NoPanic(t *testing.T) {
	tc, _ := StartSpan(context.Background(), "test.span")
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("AddEvent() panicked: %v", r)
		}
	}()
	tc.AddEvent("test.event")
}

func TestAddEvent_WithAttributes(t *testing.T) {
	tc, _ := StartSpan(context.Background(), "test.span")
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("AddEvent() panicked: %v", r)
		}
	}()
	tc.AddEvent("test.event", map[string]interface{}{"key": "value"})
}

func TestInstrumentCircuitBreaker_NoPanic(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentCircuitBreaker() panicked: %v", r)
		}
	}()
	err := InstrumentCircuitBreaker("test-cb", "closed", func() error {
		return nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestInstrumentCircuitBreaker_Error(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentCircuitBreaker() panicked: %v", r)
		}
	}()
	expectedErr := errors.New("cb error")
	err := InstrumentCircuitBreaker("test-cb", "open", func() error {
		return expectedErr
	})
	if err != expectedErr {
		t.Errorf("expected %v, got %v", expectedErr, err)
	}
}

func TestInstrumentRetry_NoPanic(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentRetry() panicked: %v", r)
		}
	}()
	err := InstrumentRetry("test-retry", 1, 3, func() error {
		return nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestInstrumentRetry_Error(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentRetry() panicked: %v", r)
		}
	}()
	expectedErr := errors.New("retry error")
	err := InstrumentRetry("test-retry", 2, 3, func() error {
		return expectedErr
	})
	if err != expectedErr {
		t.Errorf("expected %v, got %v", expectedErr, err)
	}
}

func TestInstrumentRateLimiter_NoPanic(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentRateLimiter() panicked: %v", r)
		}
	}()
	err := InstrumentRateLimiter("test-rl", true, func() error {
		return nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestInstrumentRateLimiter_Error(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentRateLimiter() panicked: %v", r)
		}
	}()
	expectedErr := errors.New("rl error")
	err := InstrumentRateLimiter("test-rl", false, func() error {
		return expectedErr
	})
	if err != expectedErr {
		t.Errorf("expected %v, got %v", expectedErr, err)
	}
}

func TestInstrumentBulkhead_NoPanic(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentBulkhead() panicked: %v", r)
		}
	}()
	err := InstrumentBulkhead("test-bh", 1, 5, func() error {
		return nil
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestInstrumentBulkhead_Error(t *testing.T) {
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("InstrumentBulkhead() panicked: %v", r)
		}
	}()
	expectedErr := errors.New("bh error")
	err := InstrumentBulkhead("test-bh", 5, 5, func() error {
		return expectedErr
	})
	if err != expectedErr {
		t.Errorf("expected %v, got %v", expectedErr, err)
	}
}

func TestDefaultTracingConfig(t *testing.T) {
	cfg := DefaultTracingConfig()
	if cfg.Enabled {
		t.Error("expected default config to have Enabled=false")
	}
	if cfg.ServiceName != "aether-resilience" {
		t.Errorf("expected ServiceName=aether-resilience, got %s", cfg.ServiceName)
	}
	if cfg.SampleRate != 1.0 {
		t.Errorf("expected SampleRate=1.0, got %f", cfg.SampleRate)
	}
}

func TestNewResilienceInstrumentation(t *testing.T) {
	cfg := DefaultTracingConfig()
	cfg.Enabled = true
	ri := NewResilienceInstrumentation(cfg)
	if ri == nil {
		t.Fatal("expected non-nil ResilienceInstrumentation")
	}
}

func TestResilienceInstrumentation_TraceCircuitBreaker(t *testing.T) {
	ri := NewResilienceInstrumentation(DefaultTracingConfig())
	tc, ctx := ri.TraceCircuitBreaker(context.Background(), "cb", "closed", "execute")
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
}

func TestResilienceInstrumentation_TraceRetry(t *testing.T) {
	ri := NewResilienceInstrumentation(DefaultTracingConfig())
	tc, ctx := ri.TraceRetry(context.Background(), "retry", 1, 3, "attempt")
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
}

func TestResilienceInstrumentation_TraceRateLimiter(t *testing.T) {
	ri := NewResilienceInstrumentation(DefaultTracingConfig())
	tc, ctx := ri.TraceRateLimiter(context.Background(), "rl", "acquire", 100)
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
}

func TestResilienceInstrumentation_TraceBulkhead(t *testing.T) {
	ri := NewResilienceInstrumentation(DefaultTracingConfig())
	tc, ctx := ri.TraceBulkhead(context.Background(), "bh", "execute", 1, 5)
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
}

func TestResilienceInstrumentation_TraceHealthCheck(t *testing.T) {
	ri := NewResilienceInstrumentation(DefaultTracingConfig())
	tc, ctx := ri.TraceHealthCheck(context.Background(), "service", "db-check")
	if tc == nil {
		t.Fatal("expected non-nil TracingContext")
	}
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
}
