package benchmark

import (
	"context"
	"testing"
	"time"

	"github.com/aether-sdk/aether-go/aether/resilience"
	"github.com/aether-sdk/aether-go/aether/validation"
)

// ============================================
// Circuit Breaker Benchmarks
// ============================================

func BenchmarkCircuitBreakerCreation(b *testing.B) {
	config := resilience.DefaultCircuitBreakerConfig()
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = resilience.NewCircuitBreaker(config)
	}
}

func BenchmarkCircuitBreakerExecute(b *testing.B) {
	breaker := resilience.NewCircuitBreaker(resilience.DefaultCircuitBreakerConfig())
	ctx := context.Background()
	fn := func() error { return nil }
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = breaker.Execute(ctx, fn)
	}
}

// ============================================
// Retry Benchmarks
// ============================================

func BenchmarkRetryCreation(b *testing.B) {
	config := resilience.DefaultRetryConfig()
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = resilience.NewRetryPolicy(config)
	}
}

func BenchmarkRetryExecuteSuccess(b *testing.B) {
	retry := resilience.NewRetryPolicy(resilience.DefaultRetryConfig())
	ctx := context.Background()
	fn := func() error { return nil }
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = retry.Execute(ctx, fn)
	}
}

// ============================================
// Rate Limiter Benchmarks
// ============================================

func BenchmarkRateLimiterCreation(b *testing.B) {
	config := resilience.DefaultRateLimitConfig()
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = resilience.NewRateLimiter(config)
	}
}

func BenchmarkRateLimiterTryAcquire(b *testing.B) {
	// High limit to avoid blocking
	config := resilience.RateLimitConfig{
		MaxRequests: 1000000,
		WindowMs:    time.Second,
		Strategy:    resilience.SlidingWindow,
	}
	limiter := resilience.NewRateLimiter(config)
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = limiter.TryAcquire()
	}
}

func BenchmarkRateLimiterTokenBucket(b *testing.B) {
	config := resilience.RateLimitConfig{
		MaxRequests: 1000000,
		WindowMs:    time.Second,
		Strategy:    resilience.TokenBucket,
	}
	limiter := resilience.NewRateLimiter(config)
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = limiter.TryAcquire()
	}
}

// ============================================
// Bulkhead Benchmarks
// ============================================

func BenchmarkBulkheadCreation(b *testing.B) {
	config := resilience.DefaultBulkheadConfig()
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = resilience.NewBulkhead(config)
	}
}

func BenchmarkBulkheadExecute(b *testing.B) {
	bulkhead := resilience.NewBulkhead(resilience.DefaultBulkheadConfig())
	ctx := context.Background()
	fn := func() error { return nil }
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = bulkhead.Execute(ctx, fn)
	}
}

// ============================================
// Validator Benchmarks
// ============================================

func BenchmarkValidatorCreation(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = validation.NewValidator()
	}
}

func BenchmarkValidatorSingleField(b *testing.B) {
	v := validation.NewValidator()
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		v.Clear()
		v.Required("name", "test")
		v.MinLength("name", "test", 1)
		v.MaxLength("name", "test", 100)
	}
}

func BenchmarkValidatorMultipleFields(b *testing.B) {
	v := validation.NewValidator()
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		v.Clear()
		v.Required("name", "John Doe")
		v.Required("email", "john@example.com")
		v.Email("email", "john@example.com")
		v.Required("age", 25)
		v.Int("age", 25)
		v.Range("age", 25, 0, 150)
		v.Required("bio", "A long bio")
		v.MinLength("bio", "A long bio", 10)
		v.MaxLength("bio", "A long bio", 1000)
	}
}

func BenchmarkEmailValidation(b *testing.B) {
	email := "test.user+tag@example.com"
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.ValidateEmail(email)
	}
}

func BenchmarkURLValidation(b *testing.B) {
	url := "https://example.com/path?query=value"
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.ValidateURL(url)
	}
}

func BenchmarkUUIDValidation(b *testing.B) {
	uuid := "123e4567-e89b-12d3-a456-426614174000"
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.ValidateUUID(uuid)
	}
}

// ============================================
// Sanitization Benchmarks
// ============================================

func BenchmarkSanitizeString(b *testing.B) {
	input := "  Hello, World! This is a test string.  \x00"
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.SanitizeString(input, 100)
	}
}

func BenchmarkSanitizeHTML(b *testing.B) {
	input := `<script>alert("xss")</script><p>Hello, World!</p>`
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.SanitizeHTML(input)
	}
}

func BenchmarkSanitizeFilename(b *testing.B) {
	input := "../../../etc/passwd"
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.SanitizeFilename(input)
	}
}

func BenchmarkSanitizeSlug(b *testing.B) {
	input := "Hello World! This is a Test String."
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		validation.SanitizeSlug(input)
	}
}

// ============================================
// Combined Benchmarks
// ============================================

func BenchmarkCombinedExecutor(b *testing.B) {
	breaker := resilience.NewCircuitBreaker(resilience.DefaultCircuitBreakerConfig())
	retry := resilience.NewRetryPolicy(resilience.DefaultRetryConfig())
	limiter := resilience.NewRateLimiter(resilience.DefaultRateLimitConfig())
	bulkhead := resilience.NewBulkhead(resilience.DefaultBulkheadConfig())
	
	ctx := context.Background()
	fn := func() error { return nil }
	
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		// Simulate combined resilience
		if limiter.TryAcquire().Allowed {
			_ = bulkhead.Execute(ctx, func() error {
				return breaker.Execute(ctx, func() error {
					return retry.Execute(ctx, fn)
				})
			})
		}
	}
}
