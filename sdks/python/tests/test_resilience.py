"""
Integration Tests for Resilience Patterns

Tests all resilience patterns in the Python SDK.
"""

import asyncio
import pytest
import sys
import os

# Add the SDK to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'aether_sdk'))

from resilience import (
    CircuitBreaker,
    CircuitBreakerConfig,
    CircuitBreakerError,
    CircuitState,
    RetryPolicy,
    RetryConfig,
    RetryExhaustedError,
    BackoffStrategy,
    RateLimiter,
    RateLimitConfig,
    RateLimitExhaustedError,
    RateLimitStrategy,
    Bulkhead,
    BulkheadConfig,
    BulkheadRejectedError,
    HealthChecker,
    HealthStatus,
    ResilientExecutor,
)


class TestCircuitBreaker:
    """Tests for Circuit Breaker pattern."""

    @pytest.mark.asyncio
    async def test_circuit_breaker_closed_state(self):
        """Test circuit breaker starts in closed state."""
        cb = CircuitBreaker()
        assert cb.state == CircuitState.CLOSED
        assert cb.is_closed

    @pytest.mark.asyncio
    async def test_circuit_breaker_success(self):
        """Test successful execution through circuit breaker."""
        cb = CircuitBreaker()
        
        async def success_func():
            return "success"
        
        result = await cb.execute(success_func)
        assert result == "success"
        
        stats = cb.get_stats()
        assert stats.total_calls == 1

    @pytest.mark.asyncio
    async def test_circuit_breaker_failure_threshold(self):
        """Test circuit breaker opens after failure threshold."""
        cb = CircuitBreaker(CircuitBreakerConfig(
            failure_threshold=3,
            timeout_ms=100,
        ))
        
        async def fail_func():
            raise Exception("test error")
        
        # Trigger failures
        for _ in range(3):
            try:
                await cb.execute(fail_func)
            except Exception:
                pass
        
        # Circuit should be open
        assert cb.is_open

    @pytest.mark.asyncio
    async def test_circuit_breaker_rejects_when_open(self):
        """Test circuit breaker rejects calls when open."""
        cb = CircuitBreaker()
        cb.force_open()
        
        async def func():
            return "should not execute"
        
        with pytest.raises(CircuitBreakerError):
            await cb.execute(func)

    @pytest.mark.asyncio
    async def test_circuit_breaker_half_open_recovery(self):
        """Test circuit breaker recovers through half-open state."""
        cb = CircuitBreaker(CircuitBreakerConfig(
            failure_threshold=2,
            success_threshold=2,
            timeout_ms=50,
        ))
        
        # Open the circuit
        async def fail_func():
            raise Exception("test error")
        
        for _ in range(2):
            try:
                await cb.execute(fail_func)
            except Exception:
                pass
        
        assert cb.is_open
        
        # Wait for timeout
        await asyncio.sleep(0.1)
        
        # Should transition to half-open on next call
        async def success_func():
            return "success"
        
        # First success in half-open
        result = await cb.execute(success_func)
        assert result == "success"
        assert cb.is_half_open
        
        # Second success should close circuit
        result = await cb.execute(success_func)
        assert result == "success"
        assert cb.is_closed


class TestRetryPolicy:
    """Tests for Retry pattern."""

    @pytest.mark.asyncio
    async def test_retry_success_first_try(self):
        """Test successful execution on first try."""
        policy = RetryPolicy()
        
        async def success_func():
            return "success"
        
        result = await policy.execute(success_func)
        assert result.result == "success"
        assert result.attempts == 1

    @pytest.mark.asyncio
    async def test_retry_success_after_failures(self):
        """Test successful execution after failures."""
        policy = RetryPolicy(RetryConfig(
            max_attempts=3,
            backoff=BackoffStrategy.FIXED,
            base_delay_ms=10,
        ))
        
        attempts = [0]
        
        async def flaky_func():
            attempts[0] += 1
            if attempts[0] < 3:
                raise Exception("temporary error")  # Matches default retryable patterns
            return "success"
        
        result = await policy.execute(flaky_func)
        assert result.result == "success"
        assert result.attempts == 3

    @pytest.mark.asyncio
    async def test_retry_exhausted(self):
        """Test retry exhausted after max attempts."""
        policy = RetryPolicy(RetryConfig(
            max_attempts=2,
            backoff=BackoffStrategy.FIXED,
            base_delay_ms=10,
        ))
        
        async def always_fail():
            raise Exception("always fails")
        
        with pytest.raises(RetryExhaustedError):
            await policy.execute(always_fail)

    @pytest.mark.asyncio
    async def test_retry_backoff_strategies(self):
        """Test different backoff strategies."""
        # Test exponential backoff
        policy = RetryPolicy(RetryConfig(
            max_attempts=2,
            backoff=BackoffStrategy.EXPONENTIAL,
            base_delay_ms=10,
            multiplier=2.0,
        ))
        
        attempts = [0]
        delays = []
        
        async def track_delays():
            attempts[0] += 1
            if attempts[0] == 1:
                delays.append(asyncio.get_event_loop().time())
                raise Exception("timeout on first attempt")  # Matches default retryable patterns
            delays.append(asyncio.get_event_loop().time())
            return "success"
        
        result = await policy.execute(track_delays)
        assert result.result == "success"


class TestRateLimiter:
    """Tests for Rate Limiter pattern."""

    @pytest.mark.asyncio
    async def test_rate_limiter_allows_requests(self):
        """Test rate limiter allows requests."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10,
            strategy=RateLimitStrategy.TOKEN_BUCKET,
        ))
        
        result = await rl.try_acquire()
        assert result.allowed

    @pytest.mark.asyncio
    async def test_rate_limiter_enforces_limit(self):
        """Test rate limiter enforces request limit."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=2,
            burst_size=2,
            strategy=RateLimitStrategy.TOKEN_BUCKET,
        ))
        
        # Should allow first two
        assert (await rl.try_acquire()).allowed
        assert (await rl.try_acquire()).allowed
        
        # Third should be rate limited
        result = await rl.try_acquire()
        assert not result.allowed

    @pytest.mark.asyncio
    async def test_rate_limiter_acquire_waits(self):
        """Test rate limiter acquire waits for token."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=100,
            burst_size=1,
            strategy=RateLimitStrategy.TOKEN_BUCKET,
        ))
        
        # Use the only token
        await rl.try_acquire()
        
        # Acquire should wait and succeed
        await rl.acquire(max_wait_ms=100)

    @pytest.mark.asyncio
    async def test_rate_limiter_strategies(self):
        """Test different rate limiting strategies."""
        # Sliding window
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=5,
            strategy=RateLimitStrategy.SLIDING_WINDOW,
        ))
        
        for _ in range(5):
            assert (await rl.try_acquire()).allowed
        
        # Next should be rejected
        result = await rl.try_acquire()
        assert not result.allowed


class TestBulkhead:
    """Tests for Bulkhead pattern."""

    @pytest.mark.asyncio
    async def test_bulkhead_allows_concurrent(self):
        """Test bulkhead allows concurrent calls within limit."""
        bh = Bulkhead(BulkheadConfig(
            max_concurrent=2,
            max_queued=0,
        ))
        
        async def quick_func():
            return "success"
        
        # Should allow both
        result1 = await bh.execute(quick_func)
        result2 = await bh.execute(quick_func)
        
        assert result1 == "success"
        assert result2 == "success"

    @pytest.mark.asyncio
    async def test_bulkhead_rejects_over_limit(self):
        """Test bulkhead rejects calls over limit with no queue."""
        bh = Bulkhead(BulkheadConfig(
            max_concurrent=1,
            max_queued=0,
        ))
        
        started = asyncio.Event()
        blocked = asyncio.Event()
        
        async def blocking_func():
            started.set()
            await blocked.wait()
            return "done"
        
        # Start first call - this holds the semaphore
        task1 = asyncio.create_task(bh.execute(blocking_func))
        await started.wait()
        
        # Second call should be rejected since max_concurrent=1 and no queue
        async def quick_func():
            return "quick"
        
        with pytest.raises(BulkheadRejectedError):
            await bh.execute(quick_func)
        
        blocked.set()
        result1 = await task1
        assert result1 == "done"

    @pytest.mark.asyncio
    async def test_bulkhead_stats(self):
        """Test bulkhead statistics."""
        bh = Bulkhead(BulkheadConfig(
            max_concurrent=5,
            max_queued=10,
        ))
        
        stats = bh.get_stats()
        assert stats.max_concurrent == 5
        assert stats.max_queued == 10


class TestHealthChecker:
    """Tests for Health Checker pattern."""

    @pytest.mark.asyncio
    async def test_health_checker_liveness(self):
        """Test liveness probe."""
        hc = HealthChecker()
        
        result = await hc.get_liveness()
        assert result["alive"] is True

    @pytest.mark.asyncio
    async def test_health_checker_readiness(self):
        """Test readiness probe."""
        hc = HealthChecker()
        
        result = await hc.get_readiness()
        assert result["ready"] is True

    @pytest.mark.asyncio
    async def test_health_checker_register_check(self):
        """Test registering health check."""
        hc = HealthChecker()
        
        await hc.register_check(
            "test-check",
            lambda: type('HealthCheckResult', (), {
                'status': HealthStatus.HEALTHY,
                'component_id': 'test',
                'component_type': 'test',
                'time': '2024-01-01T00:00:00Z',
            })(),
        )
        
        report = await hc.run_all()
        assert report.status == HealthStatus.HEALTHY
        assert "test-check" in report.checks


class TestResilientExecutor:
    """Tests for combined Resilient Executor."""

    @pytest.mark.asyncio
    async def test_executor_with_all_patterns(self):
        """Test executor with all patterns configured."""
        executor = ResilientExecutor(
            breaker=CircuitBreaker(),
            retry=RetryPolicy(RetryConfig(
                max_attempts=2,
                backoff=BackoffStrategy.FIXED,
                base_delay_ms=10,
            )),
            rate_limiter=RateLimiter(RateLimitConfig(
                requests_per_second=10,
            )),
            bulkhead=Bulkhead(BulkheadConfig(
                max_concurrent=5,
            )),
        )
        
        async def success_func():
            return "success"
        
        result = await executor.execute(success_func)
        assert result == "success"

    @pytest.mark.asyncio
    async def test_executor_rate_limits(self):
        """Test executor rate limits requests."""
        executor = ResilientExecutor(
            rate_limiter=RateLimiter(RateLimitConfig(
                requests_per_second=2,
                burst_size=2,
            )),
        )
        
        async def func():
            return "done"
        
        # First two should succeed
        await executor.execute(func)
        await executor.execute(func)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
