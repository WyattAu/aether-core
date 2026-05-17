package resilience

import (
	"context"
	"errors"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestHealthChecker_New(t *testing.T) {
	hc := NewHealthChecker("test-svc", "1.0.0")
	if hc.serviceID != "test-svc" {
		t.Errorf("expected serviceID 'test-svc', got %q", hc.serviceID)
	}
	if hc.version != "1.0.0" {
		t.Errorf("expected version '1.0.0', got %q", hc.version)
	}
	if hc.startTime.IsZero() {
		t.Error("startTime should be set")
	}
}

func TestHealthChecker_GetLiveness(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	result := hc.GetLiveness()
	if result["alive"] != true {
		t.Error("liveness should report alive=true")
	}
	if _, ok := result["time"]; !ok {
		t.Error("liveness should include time")
	}
}

func TestHealthChecker_GetStartup(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	result := hc.GetStartup()
	if result["started"] != true {
		t.Error("startup should report started=true")
	}
}

func TestHealthChecker_GetReadiness_NoChecks(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	result := hc.GetReadiness(context.Background())
	if result["ready"] != true {
		t.Error("readiness should be true with no checks")
	}
}

func TestHealthChecker_RegisterAndRunCheck(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")

	hc.RegisterCheck("db", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{
			Status:        StatusHealthy,
			ComponentID:   "db",
			ComponentType: "dependency",
			Time:          time.Now().UTC().Format(time.RFC3339),
		}
	}, &HealthCheckOptions{Timeout: 5 * time.Second})

	result := hc.RunCheck(context.Background(), "db")
	if result.Status != StatusHealthy {
		t.Errorf("expected healthy, got %v", result.Status)
	}
	if result.ComponentID != "db" {
		t.Errorf("expected componentID 'db', got %q", result.ComponentID)
	}
}

func TestHealthChecker_RunCheck_NotFound(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	result := hc.RunCheck(context.Background(), "missing")
	if result.Status != StatusUnhealthy {
		t.Errorf("expected unhealthy for missing check, got %v", result.Status)
	}
}

func TestHealthChecker_UnregisterCheck(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	hc.RegisterCheck("test", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{Status: StatusHealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, nil)

	hc.UnregisterCheck("test")
	result := hc.RunCheck(context.Background(), "test")
	if result.Status != StatusUnhealthy {
		t.Error("unregistered check should return unhealthy")
	}
}

func TestHealthChecker_RunAll(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")

	hc.RegisterCheck("check1", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{Status: StatusHealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, nil)
	hc.RegisterCheck("check2", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{Status: StatusDegraded, Time: time.Now().UTC().Format(time.RFC3339)}
	}, nil)

	report := hc.RunAll(context.Background())
	if report.Status != StatusDegraded {
		t.Errorf("expected degraded status, got %v", report.Status)
	}
	if len(report.Checks) != 2 {
		t.Errorf("expected 2 checks, got %d", len(report.Checks))
	}
	if report.Version != "1.0" {
		t.Errorf("expected version '1.0', got %q", report.Version)
	}
}

func TestHealthChecker_RunAll_CriticalUnhealthy(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")

	hc.RegisterCheck("critical", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{Status: StatusUnhealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, &HealthCheckOptions{Critical: true})
	hc.RegisterCheck("noncritical", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{Status: StatusHealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, &HealthCheckOptions{Critical: false})

	report := hc.RunAll(context.Background())
	if report.Status != StatusUnhealthy {
		t.Errorf("expected unhealthy due to critical failure, got %v", report.Status)
	}
}

func TestHealthChecker_CacheResult(t *testing.T) {
	var calls int32
	hc := NewHealthChecker("svc", "1.0")

	hc.RegisterCheck("cached", func(ctx context.Context) HealthCheckResult {
		atomic.AddInt32(&calls, 1)
		return HealthCheckResult{Status: StatusHealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, &HealthCheckOptions{CacheDuration: 5 * time.Second})

	hc.RunCheck(context.Background(), "cached")
	hc.RunCheck(context.Background(), "cached")

	if atomic.LoadInt32(&calls) != 1 {
		t.Errorf("expected 1 call with caching, got %d", atomic.LoadInt32(&calls))
	}
}

func TestHealthChecker_CheckTimeout(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")

	hc.RegisterCheck("slow", func(ctx context.Context) HealthCheckResult {
		time.Sleep(2 * time.Second)
		return HealthCheckResult{Status: StatusHealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, &HealthCheckOptions{Timeout: 10 * time.Millisecond})

	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()
	result := hc.RunCheck(ctx, "slow")
	if result.Status == StatusHealthy {
		t.Error("slow check should not be healthy")
	}
}

func TestHealthChecker_Shutdown(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	hc.RegisterCheck("test", func(ctx context.Context) HealthCheckResult {
		return HealthCheckResult{Status: StatusHealthy, Time: time.Now().UTC().Format(time.RFC3339)}
	}, nil)

	hc.Shutdown()

	result := hc.RunCheck(context.Background(), "test")
	if result.Status != StatusUnhealthy {
		t.Error("after shutdown, checks should return unhealthy")
	}
}

func TestHealthReport_ToJSON(t *testing.T) {
	hc := NewHealthChecker("svc", "1.0")
	report := hc.RunAll(context.Background())

	data, err := report.ToJSON()
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if len(data) == 0 {
		t.Error("expected non-empty JSON")
	}
	if !strings.Contains(string(data), `"status"`) {
		t.Error("JSON should contain status field")
	}
}

func TestHealthStatus_String(t *testing.T) {
	tests := []struct {
		status HealthStatus
		want   string
	}{
		{StatusHealthy, "healthy"},
		{StatusDegraded, "degraded"},
		{StatusUnhealthy, "unhealthy"},
		{HealthStatus("unknown"), "unknown"},
	}
	for _, tt := range tests {
		if got := string(tt.status); got != tt.want {
			t.Errorf("HealthStatus(%q) = %q, want %q", tt.status, got, tt.want)
		}
	}
}

func TestPingHealthCheck(t *testing.T) {
	fn := PingHealthCheck()
	result := fn(context.Background())
	if result.Status != StatusHealthy {
		t.Errorf("expected healthy, got %v", result.Status)
	}
	if result.ComponentID != "ping" {
		t.Errorf("expected componentID 'ping', got %q", result.ComponentID)
	}
}

func TestDependencyHealthCheck_Healthy(t *testing.T) {
	fn := DependencyHealthCheck("db", func(ctx context.Context) error {
		return nil
	}, 5*time.Second)

	result := fn(context.Background())
	if result.Status != StatusHealthy {
		t.Errorf("expected healthy, got %v", result.Status)
	}
}

func TestDependencyHealthCheck_Unhealthy(t *testing.T) {
	fn := DependencyHealthCheck("db", func(ctx context.Context) error {
		return errors.New("connection refused")
	}, 5*time.Second)

	result := fn(context.Background())
	if result.Status != StatusUnhealthy {
		t.Errorf("expected unhealthy, got %v", result.Status)
	}
}

func TestDependencyHealthCheck_Degraded(t *testing.T) {
	fn := DependencyHealthCheck("db", func(ctx context.Context) error {
		time.Sleep(1100 * time.Millisecond)
		return nil
	}, 5*time.Second)

	result := fn(context.Background())
	if result.Status != StatusDegraded {
		t.Errorf("expected degraded for slow response, got %v", result.Status)
	}
}

func TestStateStorageHealthCheck_Healthy(t *testing.T) {
	fn := StateStorageHealthCheck(func(ctx context.Context, key string) ([]byte, error) {
		return []byte("ok"), nil
	}, "test-key")

	result := fn(context.Background())
	if result.Status != StatusHealthy {
		t.Errorf("expected healthy, got %v", result.Status)
	}
}

func TestStateStorageHealthCheck_Unhealthy(t *testing.T) {
	fn := StateStorageHealthCheck(func(ctx context.Context, key string) ([]byte, error) {
		return nil, errors.New("store error")
	}, "test-key")

	result := fn(context.Background())
	if result.Status != StatusUnhealthy {
		t.Errorf("expected unhealthy, got %v", result.Status)
	}
}

func TestMemoryHealthCheck(t *testing.T) {
	fn := MemoryHealthCheck(1024*1024*1024, 0.9)
	result := fn(context.Background())
	if result.Status == StatusUnhealthy {
		t.Error("normal memory usage should not be unhealthy")
	}
}

func TestGoroutineHealthCheck(t *testing.T) {
	fn := GoroutineHealthCheck(100000, 0.9)
	result := fn(context.Background())
	if result.Status == StatusUnhealthy {
		t.Error("normal goroutine count should not be unhealthy")
	}
}

func TestHealthCheckResult_Fields(t *testing.T) {
	r := HealthCheckResult{
		Status:        StatusHealthy,
		ComponentID:   "comp",
		ComponentType: "test",
		ObservedValue: 42,
		ObservedUnit:  "ms",
		Output:        "all good",
		Time:          time.Now().UTC().Format(time.RFC3339),
		Details:       map[string]any{"key": "val"},
	}
	if r.Status != StatusHealthy {
		t.Error("status mismatch")
	}
	if r.Details["key"] != "val" {
		t.Error("details mismatch")
	}
}
