"""
Performance Benchmarks for Resilience Patterns.

Run with: python -m pytest tests/performance/benchmark_resilience.py -v --benchmark-only

Or without pytest-benchmark:
    python tests/performance/benchmark_resilience.py
"""

from __future__ import annotations

import asyncio
import statistics
import time
from dataclasses import dataclass
from typing import Any, Callable, List

# ============================================
# Simple Benchmark Framework (no dependencies)
# ============================================


@dataclass
class BenchmarkResult:
    """Result of a benchmark run."""

    name: str
    iterations: int
    total_time_ms: float
    mean_time_ms: float
    median_time_ms: float
    min_time_ms: float
    max_time_ms: float
    std_dev_ms: float
    ops_per_second: float


def benchmark_sync(
    name: str,
    func: Callable[[], Any],
    iterations: int = 10000,
    warmup: int = 100,
) -> BenchmarkResult:
    """Run a synchronous benchmark."""
    # Warmup
    for _ in range(warmup):
        func()

    # Measure
    times: List[float] = []
    for _ in range(iterations):
        start = time.perf_counter_ns()
        func()
        end = time.perf_counter_ns()
        times.append((end - start) / 1_000_000)  # Convert to ms

    total_time = sum(times)
    return BenchmarkResult(
        name=name,
        iterations=iterations,
        total_time_ms=total_time,
        mean_time_ms=statistics.mean(times),
        median_time_ms=statistics.median(times),
        min_time_ms=min(times),
        max_time_ms=max(times),
        std_dev_ms=statistics.stdev(times) if len(times) > 1 else 0,
        ops_per_second=iterations / (total_time / 1000),
    )


async def benchmark_async(
    name: str,
    func: Callable[[], Any],
    iterations: int = 10000,
    warmup: int = 100,
) -> BenchmarkResult:
    """Run an asynchronous benchmark."""
    # Warmup
    for _ in range(warmup):
        await func()

    # Measure
    times: List[float] = []
    for _ in range(iterations):
        start = time.perf_counter_ns()
        await func()
        end = time.perf_counter_ns()
        times.append((end - start) / 1_000_000)  # Convert to ms

    total_time = sum(times)
    return BenchmarkResult(
        name=name,
        iterations=iterations,
        total_time_ms=total_time,
        mean_time_ms=statistics.mean(times),
        median_time_ms=statistics.median(times),
        min_time_ms=min(times),
        max_time_ms=max(times),
        std_dev_ms=statistics.stdev(times) if len(times) > 1 else 0,
        ops_per_second=iterations / (total_time / 1000),
    )


def print_result(result: BenchmarkResult) -> None:
    """Print benchmark result."""
    print(f"\n{result.name}")
    print("-" * 60)
    print(f"  Iterations:    {result.iterations:,}")
    print(f"  Total Time:    {result.total_time_ms:.2f} ms")
    print(f"  Mean:          {result.mean_time_ms:.6f} ms")
    print(f"  Median:        {result.median_time_ms:.6f} ms")
    print(f"  Min:           {result.min_time_ms:.6f} ms")
    print(f"  Max:           {result.max_time_ms:.6f} ms")
    print(f"  Std Dev:       {result.std_dev_ms:.6f} ms")
    print(f"  Ops/sec:       {result.ops_per_second:,.0f}")


# ============================================
# Circuit Breaker Benchmarks
# ============================================


def test_circuit_breaker_creation():
    """Benchmark circuit breaker creation."""
    from aether_sdk.resilience import CircuitBreaker, CircuitBreakerConfig

    result = benchmark_sync(
        "Circuit Breaker Creation",
        lambda: CircuitBreaker(CircuitBreakerConfig()),
        iterations=10000,
    )
    print_result(result)

    # Assert reasonable performance
    assert (
        result.mean_time_ms < 1.0
    ), f"Circuit breaker creation too slow: {result.mean_time_ms}ms"


def test_circuit_breaker_execute():
    """Benchmark circuit breaker execution."""
    from aether_sdk.resilience import CircuitBreaker

    breaker = CircuitBreaker()

    async def execute():
        return await breaker.execute(lambda: asyncio.sleep(0))

    result = asyncio.run(
        benchmark_async(
            "Circuit Breaker Execute (Success)",
            execute,
            iterations=10000,
        )
    )
    print_result(result)

    # Assert reasonable overhead
    assert (
        result.mean_time_ms < 0.5
    ), f"Circuit breaker execute too slow: {result.mean_time_ms}ms"


# ============================================
# Retry Benchmarks
# ============================================


def test_retry_creation():
    """Benchmark retry policy creation."""
    from aether_sdk.resilience import RetryConfig, RetryPolicy

    result = benchmark_sync(
        "Retry Policy Creation",
        lambda: RetryPolicy(RetryConfig()),
        iterations=10000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 1.0
    ), f"Retry policy creation too slow: {result.mean_time_ms}ms"


def test_retry_execute_success():
    """Benchmark retry execution (success path)."""
    from aether_sdk.resilience import RetryPolicy

    retry = RetryPolicy()

    async def execute():
        result = await retry.execute(lambda: asyncio.sleep(0))
        return result

    result = asyncio.run(
        benchmark_async(
            "Retry Execute (Success, No Retries)",
            execute,
            iterations=5000,
        )
    )
    print_result(result)

    assert result.mean_time_ms < 1.0, f"Retry execute too slow: {result.mean_time_ms}ms"


# ============================================
# Rate Limiter Benchmarks
# ============================================


def test_rate_limiter_creation():
    """Benchmark rate limiter creation."""
    from aether_sdk.resilience import RateLimitConfig, RateLimiter

    result = benchmark_sync(
        "Rate Limiter Creation",
        lambda: RateLimiter(RateLimitConfig()),
        iterations=10000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 1.0
    ), f"Rate limiter creation too slow: {result.mean_time_ms}ms"


def test_rate_limiter_try_acquire():
    """Benchmark rate limiter try_acquire."""
    from aether_sdk.resilience import (RateLimitConfig, RateLimiter,
                                       RateLimitStrategy)

    limiter = RateLimiter(
        RateLimitConfig(
            requests_per_second=1000000,  # High limit to avoid blocking
            strategy=RateLimitStrategy.SLIDING_WINDOW,
        )
    )

    async def try_acquire():
        return await limiter.try_acquire()

    result = asyncio.run(
        benchmark_async(
            "Rate Limiter Try Acquire (Sliding Window)",
            try_acquire,
            iterations=100000,
        )
    )
    print_result(result)

    # Should be very fast
    assert (
        result.mean_time_ms < 0.1
    ), f"Rate limiter try_acquire too slow: {result.mean_time_ms}ms"


def test_rate_limiter_token_bucket():
    """Benchmark token bucket rate limiter."""
    from aether_sdk.resilience import (RateLimitConfig, RateLimiter,
                                       RateLimitStrategy)

    limiter = RateLimiter(
        RateLimitConfig(
            requests_per_second=1000000,
            strategy=RateLimitStrategy.TOKEN_BUCKET,
            burst_size=1000000,
        )
    )

    async def try_acquire():
        return await limiter.try_acquire()

    result = asyncio.run(
        benchmark_async(
            "Rate Limiter Try Acquire (Token Bucket)",
            try_acquire,
            iterations=100000,
        )
    )
    print_result(result)

    assert result.mean_time_ms < 0.1, f"Token bucket too slow: {result.mean_time_ms}ms"


# ============================================
# Bulkhead Benchmarks
# ============================================


def test_bulkhead_creation():
    """Benchmark bulkhead creation."""
    from aether_sdk.resilience import Bulkhead, BulkheadConfig

    result = benchmark_sync(
        "Bulkhead Creation",
        lambda: Bulkhead(BulkheadConfig()),
        iterations=10000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 1.0
    ), f"Bulkhead creation too slow: {result.mean_time_ms}ms"


def test_bulkhead_execute():
    """Benchmark bulkhead execution."""
    from aether_sdk.resilience import Bulkhead

    bulkhead = Bulkhead()

    async def execute():
        return await bulkhead.execute(lambda: asyncio.sleep(0))

    result = asyncio.run(
        benchmark_async(
            "Bulkhead Execute",
            execute,
            iterations=10000,
        )
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.5
    ), f"Bulkhead execute too slow: {result.mean_time_ms}ms"


# ============================================
# Validator Benchmarks
# ============================================


def test_validator_creation():
    """Benchmark validator creation."""
    from aether_sdk.validation import Validator

    result = benchmark_sync(
        "Validator Creation",
        lambda: Validator(),
        iterations=100000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.01
    ), f"Validator creation too slow: {result.mean_time_ms}ms"


def test_validator_single_field():
    """Benchmark single field validation."""
    from aether_sdk.validation import Validator

    def validate():
        v = Validator()
        v.required("name", "test")
        v.min_length("name", "test", 1)
        v.max_length("name", "test", 100)
        return v.is_valid()

    result = benchmark_sync(
        "Validator Single Field (3 rules)",
        validate,
        iterations=100000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.1
    ), f"Single field validation too slow: {result.mean_time_ms}ms"


def test_validator_multiple_fields():
    """Benchmark multiple field validation."""
    from aether_sdk.validation import Validator

    def validate():
        v = Validator()
        v.required("name", "John Doe")
        v.required("email", "john@example.com")
        v.email("email", "john@example.com")
        v.required("age", 25)
        v.integer("age", 25)
        v.range("age", 25, 0, 150)
        v.required("bio", "A long bio")
        v.min_length("bio", "A long bio", 10)
        v.max_length("bio", "A long bio", 1000)
        return v.is_valid()

    result = benchmark_sync(
        "Validator Multiple Fields (9 rules)",
        validate,
        iterations=50000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.5
    ), f"Multiple field validation too slow: {result.mean_time_ms}ms"


def test_email_validation():
    """Benchmark email validation."""
    from aether_sdk.validation import validate_email

    result = benchmark_sync(
        "Email Validation",
        lambda: validate_email("test.user+tag@example.com"),
        iterations=100000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.01
    ), f"Email validation too slow: {result.mean_time_ms}ms"


def test_url_validation():
    """Benchmark URL validation."""
    from aether_sdk.validation import validate_url

    result = benchmark_sync(
        "URL Validation",
        lambda: validate_url("https://example.com/path?query=value"),
        iterations=100000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.05
    ), f"URL validation too slow: {result.mean_time_ms}ms"


# ============================================
# Sanitization Benchmarks
# ============================================


def test_sanitize_string():
    """Benchmark string sanitization."""
    from aether_sdk.validation import sanitize_string

    test_string = "  Hello, World! This is a test string with some content.  \x00"

    result = benchmark_sync(
        "Sanitize String",
        lambda: sanitize_string(test_string, max_length=100),
        iterations=100000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.01
    ), f"String sanitization too slow: {result.mean_time_ms}ms"


def test_sanitize_html():
    """Benchmark HTML sanitization."""
    from aether_sdk.validation import sanitize_html

    test_html = '<script>alert("xss")</script><p>Hello, World!</p>'

    result = benchmark_sync(
        "Sanitize HTML",
        lambda: sanitize_html(test_html),
        iterations=100000,
    )
    print_result(result)

    assert (
        result.mean_time_ms < 0.05
    ), f"HTML sanitization too slow: {result.mean_time_ms}ms"


# ============================================
# Resilient Executor Benchmarks
# ============================================


def test_resilient_executor():
    """Benchmark combined resilient executor."""
    from aether_sdk.resilience import (Bulkhead, CircuitBreaker, RateLimiter,
                                       ResilientExecutor, RetryPolicy)

    executor = ResilientExecutor(
        breaker=CircuitBreaker(),
        retry=RetryPolicy(),
        rate_limiter=RateLimiter(),
        bulkhead=Bulkhead(),
    )

    async def execute():
        return await executor.execute(lambda: asyncio.sleep(0))

    result = asyncio.run(
        benchmark_async(
            "Resilient Executor (All Patterns)",
            execute,
            iterations=5000,
        )
    )
    print_result(result)

    # Combined overhead should be reasonable
    assert (
        result.mean_time_ms < 2.0
    ), f"Resilient executor too slow: {result.mean_time_ms}ms"


# ============================================
# Main Entry Point
# ============================================

if __name__ == "__main__":
    print("=" * 60)
    print("Aether SDK Resilience & Validation Benchmarks")
    print("=" * 60)

    # Run all benchmarks
    print("\n### Circuit Breaker Benchmarks ###")
    test_circuit_breaker_creation()
    test_circuit_breaker_execute()

    print("\n### Retry Benchmarks ###")
    test_retry_creation()
    test_retry_execute_success()

    print("\n### Rate Limiter Benchmarks ###")
    test_rate_limiter_creation()
    test_rate_limiter_try_acquire()
    test_rate_limiter_token_bucket()

    print("\n### Bulkhead Benchmarks ###")
    test_bulkhead_creation()
    test_bulkhead_execute()

    print("\n### Validator Benchmarks ###")
    test_validator_creation()
    test_validator_single_field()
    test_validator_multiple_fields()
    test_email_validation()
    test_url_validation()

    print("\n### Sanitization Benchmarks ###")
    test_sanitize_string()
    test_sanitize_html()

    print("\n### Combined Benchmarks ###")
    test_resilient_executor()

    print("\n" + "=" * 60)
    print("All benchmarks completed!")
    print("=" * 60)
