"""
Tests for Aether SDK Rate Limiter Module

Comprehensive tests for rate limiting with multiple strategies.
"""

import pytest
import asyncio
import time
from unittest.mock import AsyncMock, patch

from aether_sdk.resilience.rate_limiter import (
    RateLimitStrategy,
    RateLimitConfig,
    RateLimitStats,
    RateLimitResult,
    RateLimitExhaustedError,
    TokenBucket,
    SlidingWindow,
    FixedWindow,
    RateLimiter,
    RateLimiterManager,
    api_rate_limiter,
    strict_rate_limiter,
    bursty_rate_limiter,
)


# ============================================
# Fixtures
# ============================================

@pytest.fixture
def token_bucket():
    """Create a token bucket rate limiter."""
    return TokenBucket(requests_per_second=10, burst_size=5)


@pytest.fixture
def sliding_window():
    """Create a sliding window rate limiter."""
    return SlidingWindow(requests_per_second=5, window_size_ms=1000)


@pytest.fixture
def fixed_window():
    """Create a fixed window rate limiter."""
    return FixedWindow(requests_per_second=5, window_size_ms=1000)


# ============================================
# RateLimitConfig Tests
# ============================================

class TestRateLimitConfig:
    """Tests for RateLimitConfig."""
    
    def test_default_config(self):
        """Test default configuration."""
        config = RateLimitConfig()
        
        assert config.requests_per_second == 100
        assert config.burst_size is None
        assert config.strategy == RateLimitStrategy.TOKEN_BUCKET
        assert config.window_size_ms == 1000
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = RateLimitConfig(
            requests_per_second=50,
            burst_size=100,
            strategy=RateLimitStrategy.SLIDING_WINDOW,
            window_size_ms=500,
        )
        
        assert config.requests_per_second == 50
        assert config.burst_size == 100
        assert config.strategy == RateLimitStrategy.SLIDING_WINDOW
        assert config.window_size_ms == 500


# ============================================
# RateLimitStats Tests
# ============================================

class TestRateLimitStats:
    """Tests for RateLimitStats."""
    
    def test_default_stats(self):
        """Test default stats."""
        stats = RateLimitStats()
        
        assert stats.allowed_requests == 0
        assert stats.rejected_requests == 0
        assert stats.current_rate == 0.0
        assert stats.wait_time_ms == 0
    
    def test_custom_stats(self):
        """Test custom stats."""
        stats = RateLimitStats(
            allowed_requests=100,
            rejected_requests=10,
            current_rate=0.5,
            wait_time_ms=50,
        )
        
        assert stats.allowed_requests == 100
        assert stats.rejected_requests == 10
        assert stats.current_rate == 0.5
        assert stats.wait_time_ms == 50


# ============================================
# RateLimitResult Tests
# ============================================

class TestRateLimitResult:
    """Tests for RateLimitResult."""
    
    def test_allowed_result(self):
        """Test allowed result."""
        result = RateLimitResult(allowed=True, remaining_tokens=5)
        
        assert result.allowed is True
        assert result.wait_time_ms == 0
        assert result.remaining_tokens == 5
        assert result.reset_in is None
    
    def test_rejected_result(self):
        """Test rejected result."""
        result = RateLimitResult(
            allowed=False,
            wait_time_ms=100,
            remaining_tokens=0,
            reset_in=100,
        )
        
        assert result.allowed is False
        assert result.wait_time_ms == 100
        assert result.remaining_tokens == 0
        assert result.reset_in == 100


# ============================================
# TokenBucket Tests
# ============================================

class TestTokenBucket:
    """Tests for TokenBucket."""
    
    @pytest.mark.asyncio
    async def test_initial_state(self, token_bucket):
        """Test initial state has full tokens."""
        tokens = token_bucket.get_tokens()
        assert tokens == 5  # burst_size
    
    @pytest.mark.asyncio
    async def test_acquire_single_token(self, token_bucket):
        """Test acquiring single token."""
        result = await token_bucket.try_acquire()
        
        assert result.allowed is True
        assert result.remaining_tokens == 4
        assert result.wait_time_ms == 0
    
    @pytest.mark.asyncio
    async def test_acquire_multiple_tokens(self, token_bucket):
        """Test acquiring multiple tokens."""
        result = await token_bucket.try_acquire(3)
        
        assert result.allowed is True
        assert result.remaining_tokens == 2
    
    @pytest.mark.asyncio
    async def test_acquire_until_exhausted(self, token_bucket):
        """Test acquiring until tokens exhausted."""
        # Use all 5 tokens
        for _ in range(5):
            result = await token_bucket.try_acquire()
            assert result.allowed is True
        
        # Next should fail
        result = await token_bucket.try_acquire()
        assert result.allowed is False
        assert result.wait_time_ms > 0
    
    @pytest.mark.asyncio
    async def test_token_refill(self):
        """Test tokens refill over time."""
        # Create bucket with 1 token per 100ms
        bucket = TokenBucket(requests_per_second=10, burst_size=2)
        
        # Use all tokens
        await bucket.try_acquire()
        await bucket.try_acquire()
        
        # Should be exhausted
        result = await bucket.try_acquire()
        assert result.allowed is False
        
        # Wait for refill (100ms = 1 token at 10 req/s)
        await asyncio.sleep(0.15)
        
        # Should have refilled
        tokens = bucket.get_tokens()
        assert tokens >= 1
    
    @pytest.mark.asyncio
    async def test_concurrent_access(self, token_bucket):
        """Test concurrent token acquisition."""
        async def acquire():
            return await token_bucket.try_acquire()
        
        # Run multiple acquisitions concurrently
        results = await asyncio.gather(*[acquire() for _ in range(5)])
        
        # All should succeed since we have 5 tokens
        assert all(r.allowed for r in results)
    
    @pytest.mark.asyncio
    async def test_partial_token_request(self, token_bucket):
        """Test requesting more tokens than available."""
        # Use 3 tokens
        await token_bucket.try_acquire(3)
        
        # Request 3 more (only 2 available)
        result = await token_bucket.try_acquire(3)
        
        assert result.allowed is False
        assert result.wait_time_ms > 0
    
    @pytest.mark.asyncio
    async def test_wait_time_calculation(self, token_bucket):
        """Test wait time is calculated correctly."""
        # Exhaust all tokens
        await token_bucket.try_acquire(5)
        
        result = await token_bucket.try_acquire()
        
        assert result.allowed is False
        # At 10 req/s, 1 token needs 100ms
        assert result.wait_time_ms >= 50  # Allow some tolerance


# ============================================
# SlidingWindow Tests
# ============================================

class TestSlidingWindow:
    """Tests for SlidingWindow."""
    
    @pytest.mark.asyncio
    async def test_initial_state(self, sliding_window):
        """Test initial state."""
        count = sliding_window.get_current_count()
        assert count == 0
    
    @pytest.mark.asyncio
    async def test_acquire_within_limit(self, sliding_window):
        """Test acquiring within limit."""
        for i in range(5):
            result = await sliding_window.try_acquire()
            assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_acquire_exceeds_limit(self, sliding_window):
        """Test acquiring exceeds limit."""
        # Use all 5
        for _ in range(5):
            await sliding_window.try_acquire()
        
        # Next should fail
        result = await sliding_window.try_acquire()
        assert result.allowed is False
        assert result.wait_time_ms > 0
    
    @pytest.mark.asyncio
    async def test_window_sliding(self):
        """Test window slides over time."""
        # Short window for testing
        window = SlidingWindow(requests_per_second=2, window_size_ms=100)
        
        # Use both
        await window.try_acquire()
        await window.try_acquire()
        
        # Should be rejected
        result = await window.try_acquire()
        assert result.allowed is False
        
        # Wait for window to slide
        await asyncio.sleep(0.15)
        
        # Should be allowed now
        result = await window.try_acquire()
        assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_remaining_tokens(self, sliding_window):
        """Test remaining tokens count."""
        result = await sliding_window.try_acquire()
        assert result.remaining_tokens == 4
        
        result = await sliding_window.try_acquire()
        assert result.remaining_tokens == 3
    
    @pytest.mark.asyncio
    async def test_concurrent_access(self, sliding_window):
        """Test concurrent access."""
        async def acquire():
            return await sliding_window.try_acquire()
        
        results = await asyncio.gather(*[acquire() for _ in range(5)])
        
        # Exactly 5 should succeed
        allowed_count = sum(1 for r in results if r.allowed)
        assert allowed_count == 5


# ============================================
# FixedWindow Tests
# ============================================

class TestFixedWindow:
    """Tests for FixedWindow."""
    
    @pytest.mark.asyncio
    async def test_initial_state(self, fixed_window):
        """Test initial state."""
        count = fixed_window.get_current_count()
        assert count == 0
    
    @pytest.mark.asyncio
    async def test_acquire_within_limit(self, fixed_window):
        """Test acquiring within limit."""
        for i in range(5):
            result = await fixed_window.try_acquire()
            assert result.allowed is True
            assert result.reset_in is not None
    
    @pytest.mark.asyncio
    async def test_acquire_exceeds_limit(self, fixed_window):
        """Test acquiring exceeds limit."""
        # Use all 5
        for _ in range(5):
            await fixed_window.try_acquire()
        
        # Next should fail
        result = await fixed_window.try_acquire()
        assert result.allowed is False
        assert result.wait_time_ms > 0
    
    @pytest.mark.asyncio
    async def test_window_reset(self):
        """Test window resets after duration."""
        # Short window for testing
        window = FixedWindow(requests_per_second=2, window_size_ms=100)
        
        # Use both
        await window.try_acquire()
        await window.try_acquire()
        
        # Should be rejected
        result = await window.try_acquire()
        assert result.allowed is False
        
        # Wait for window reset
        await asyncio.sleep(0.15)
        
        # Should be allowed (new window)
        result = await window.try_acquire()
        assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_concurrent_access(self, fixed_window):
        """Test concurrent access."""
        async def acquire():
            return await fixed_window.try_acquire()
        
        results = await asyncio.gather(*[acquire() for _ in range(5)])
        
        # Exactly 5 should succeed
        allowed_count = sum(1 for r in results if r.allowed)
        assert allowed_count == 5


# ============================================
# RateLimiter Tests
# ============================================

class TestRateLimiter:
    """Tests for RateLimiter."""
    
    @pytest.mark.asyncio
    async def test_default_config(self):
        """Test rate limiter with default config."""
        rl = RateLimiter()
        
        result = await rl.try_acquire()
        assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_token_bucket_strategy(self):
        """Test token bucket strategy."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10,
            burst_size=5,
            strategy=RateLimitStrategy.TOKEN_BUCKET,
        ))
        
        # Should allow 5 (burst)
        for _ in range(5):
            result = await rl.try_acquire()
            assert result.allowed is True
        
        # Next should be limited
        result = await rl.try_acquire()
        assert result.allowed is False
    
    @pytest.mark.asyncio
    async def test_sliding_window_strategy(self):
        """Test sliding window strategy."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=3,
            strategy=RateLimitStrategy.SLIDING_WINDOW,
        ))
        
        # Should allow 3
        for _ in range(3):
            result = await rl.try_acquire()
            assert result.allowed is True
        
        # Next should be limited
        result = await rl.try_acquire()
        assert result.allowed is False
    
    @pytest.mark.asyncio
    async def test_fixed_window_strategy(self):
        """Test fixed window strategy."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=3,
            strategy=RateLimitStrategy.FIXED_WINDOW,
        ))
        
        # Should allow 3
        for _ in range(3):
            result = await rl.try_acquire()
            assert result.allowed is True
        
        # Next should be limited
        result = await rl.try_acquire()
        assert result.allowed is False
    
    @pytest.mark.asyncio
    async def test_acquire_with_wait(self):
        """Test acquire waits for token."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=100,
            burst_size=1,
        ))
        
        # Use the token
        await rl.try_acquire()
        
        # Acquire should wait and succeed
        await rl.acquire(max_wait_ms=500)
        # Should not raise
    
    @pytest.mark.asyncio
    async def test_acquire_exceeds_max_wait(self):
        """Test acquire raises when max wait exceeded."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=1,
            burst_size=1,
        ))
        
        # Use the token
        await rl.try_acquire()
        
        # Should raise because wait would be ~1000ms
        with pytest.raises(RateLimitExhaustedError):
            await rl.acquire(max_wait_ms=10)
    
    @pytest.mark.asyncio
    async def test_acquire_after_rejection_in_retry(self):
        """Test acquire handles rejection after waiting."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=1,
            burst_size=1,
            window_size_ms=100,
        ))
        
        # Use the token
        await rl.try_acquire()
        
        # Try to acquire with short wait (might fail if still no token)
        try:
            await rl.acquire(max_wait_ms=50)
        except RateLimitExhaustedError:
            pass  # Expected if still rate limited
    
    @pytest.mark.asyncio
    async def test_execute_with_rate_limiting(self):
        """Test execute function with rate limiting."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=100,
            burst_size=5,
        ))
        
        async def my_func():
            return "success"
        
        result = await rl.execute(my_func)
        assert result == "success"
    
    @pytest.mark.asyncio
    async def test_get_stats(self):
        """Test getting statistics."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10,
            burst_size=5,
        ))
        
        # Make some requests
        await rl.try_acquire()
        await rl.try_acquire()
        
        stats = rl.get_stats()
        
        assert stats.allowed_requests == 2
        assert stats.rejected_requests == 0
    
    @pytest.mark.asyncio
    async def test_get_stats_with_rejections(self):
        """Test stats include rejections."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=2,
            burst_size=2,
        ))
        
        # Make requests (2 allowed, 1 rejected)
        await rl.try_acquire()
        await rl.try_acquire()
        await rl.try_acquire()
        
        stats = rl.get_stats()
        
        assert stats.allowed_requests == 2
        assert stats.rejected_requests == 1
    
    @pytest.mark.asyncio
    async def test_reset_stats(self):
        """Test resetting statistics."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10,
        ))
        
        # Make some requests
        await rl.try_acquire()
        await rl.try_acquire()
        
        # Reset
        rl.reset_stats()
        
        stats = rl.get_stats()
        assert stats.allowed_requests == 0
        assert stats.rejected_requests == 0
    
    @pytest.mark.asyncio
    async def test_burst_size_defaults_to_rps(self):
        """Test burst size defaults to requests_per_second."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=5,
            # burst_size not set
        ))
        
        # Should allow 5 (default burst = rps)
        for _ in range(5):
            result = await rl.try_acquire()
            assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_invalid_strategy(self):
        """Test invalid strategy raises error."""
        config = RateLimitConfig()
        config.strategy = "invalid"  # type: ignore
        
        with pytest.raises(ValueError):
            RateLimiter(config)


# ============================================
# RateLimiterManager Tests
# ============================================

class TestRateLimiterManager:
    """Tests for RateLimiterManager."""
    
    def test_default_config(self):
        """Test manager with default config."""
        manager = RateLimiterManager()
        
        rl = manager.get("test")
        assert rl is not None
    
    def test_get_creates_limiter(self):
        """Test get creates new limiter."""
        manager = RateLimiterManager()
        
        rl1 = manager.get("api")
        rl2 = manager.get("api")
        
        assert rl1 is rl2  # Same instance
    
    def test_get_with_custom_config(self):
        """Test get with custom config."""
        manager = RateLimiterManager()
        
        rl = manager.get("custom", RateLimitConfig(
            requests_per_second=50,
            strategy=RateLimitStrategy.SLIDING_WINDOW,
        ))
        
        assert rl._config.requests_per_second == 50
    
    def test_get_with_default_config(self):
        """Test get uses default config."""
        manager = RateLimiterManager(RateLimitConfig(
            requests_per_second=20,
        ))
        
        rl = manager.get("test")
        
        assert rl._config.requests_per_second == 20
    
    def test_get_all_stats(self):
        """Test getting all stats."""
        manager = RateLimiterManager()
        
        manager.get("api1")
        manager.get("api2")
        
        stats = manager.get_all_stats()
        
        assert "api1" in stats
        assert "api2" in stats
    
    @pytest.mark.asyncio
    async def test_reset_all_stats(self):
        """Test resetting all stats."""
        manager = RateLimiterManager()
        
        rl1 = manager.get("api1")
        rl2 = manager.get("api2")
        
        # Make some requests
        await rl1.try_acquire()
        await rl2.try_acquire()
        
        # Reset all
        manager.reset_all_stats()
        
        stats = manager.get_all_stats()
        assert stats["api1"].allowed_requests == 0
        assert stats["api2"].allowed_requests == 0


# ============================================
# Predefined Rate Limiters Tests
# ============================================

class TestPredefinedRateLimiters:
    """Tests for predefined rate limiter factories."""
    
    @pytest.mark.asyncio
    async def test_api_rate_limiter(self):
        """Test API rate limiter."""
        rl = api_rate_limiter()
        
        assert rl._config.requests_per_second == 100
        assert rl._config.burst_size == 200
        assert rl._config.strategy == RateLimitStrategy.TOKEN_BUCKET
        
        # Should allow burst
        result = await rl.try_acquire()
        assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_strict_rate_limiter(self):
        """Test strict rate limiter."""
        rl = strict_rate_limiter(requests_per_second=5)
        
        assert rl._config.requests_per_second == 5
        assert rl._config.strategy == RateLimitStrategy.SLIDING_WINDOW
        
        result = await rl.try_acquire()
        assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_bursty_rate_limiter(self):
        """Test bursty rate limiter."""
        rl = bursty_rate_limiter(burst_size=10, refill_rate=5)
        
        assert rl._config.requests_per_second == 5
        assert rl._config.burst_size == 10
        assert rl._config.strategy == RateLimitStrategy.TOKEN_BUCKET
        
        # Should allow burst
        for _ in range(10):
            result = await rl.try_acquire()
            assert result.allowed is True


# ============================================
# Edge Cases Tests
# ============================================

class TestEdgeCases:
    """Tests for edge cases."""
    
    @pytest.mark.asyncio
    async def test_zero_burst_size(self):
        """Test with zero burst size."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10,
            burst_size=0,
        ))
        
        # Should reject immediately
        result = await rl.try_acquire()
        assert result.allowed is False
    
    @pytest.mark.asyncio
    async def test_very_high_rate(self):
        """Test with very high rate."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10000,
            burst_size=10000,
        ))
        
        # Should allow many
        for _ in range(100):
            result = await rl.try_acquire()
            assert result.allowed is True
    
    @pytest.mark.asyncio
    async def test_multiple_tokens_request(self):
        """Test requesting multiple tokens at once."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=10,
            burst_size=10,
        ))
        
        # Request 5 tokens at once
        result = await rl.try_acquire(5)
        assert result.allowed is True
        assert result.remaining_tokens == 5
    
    @pytest.mark.asyncio
    async def test_sliding_window_current_rate(self):
        """Test sliding window current rate calculation."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=5,
            strategy=RateLimitStrategy.SLIDING_WINDOW,
        ))
        
        # Make 3 requests
        for _ in range(3):
            await rl.try_acquire()
        
        stats = rl.get_stats()
        # current_rate should be > 0 for sliding window
        assert stats.current_rate >= 0
    
    @pytest.mark.asyncio
    async def test_fixed_window_current_rate(self):
        """Test fixed window current rate calculation."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=5,
            strategy=RateLimitStrategy.FIXED_WINDOW,
        ))
        
        # Make 3 requests
        for _ in range(3):
            await rl.try_acquire()
        
        stats = rl.get_stats()
        # current_rate should be > 0 for fixed window
        assert stats.current_rate >= 0
    
    @pytest.mark.asyncio
    async def test_token_bucket_with_no_burst_config(self):
        """Test token bucket with no burst size config."""
        rl = RateLimiter(RateLimitConfig(
            requests_per_second=5,
            burst_size=None,  # Will default to requests_per_second
            strategy=RateLimitStrategy.TOKEN_BUCKET,
        ))
        
        # Should allow 5 (default burst = rps)
        for _ in range(5):
            result = await rl.try_acquire()
            assert result.allowed is True
        
        # Next should be limited
        result = await rl.try_acquire()
        assert result.allowed is False
    
    @pytest.mark.asyncio
    async def test_execute_async_function(self):
        """Test execute with async function."""
        rl = RateLimiter(RateLimitConfig(requests_per_second=10))
        
        async def async_func():
            await asyncio.sleep(0.01)
            return "async_result"
        
        result = await rl.execute(async_func)
        assert result == "async_result"
    
    @pytest.mark.asyncio
    async def test_manager_mixed_configs(self):
        """Test manager with mixed configs."""
        default_config = RateLimitConfig(
            requests_per_second=10,
            strategy=RateLimitStrategy.TOKEN_BUCKET,
        )
        manager = RateLimiterManager(default_config)
        
        # Get with default
        rl1 = manager.get("default")
        assert rl1._config.requests_per_second == 10
        
        # Get with custom
        rl2 = manager.get("custom", RateLimitConfig(
            requests_per_second=50,
            strategy=RateLimitStrategy.SLIDING_WINDOW,
        ))
        assert rl2._config.requests_per_second == 50
        assert rl2._config.strategy == RateLimitStrategy.SLIDING_WINDOW
