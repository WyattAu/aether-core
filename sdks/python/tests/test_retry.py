"""
Tests for Aether SDK Retry Module

Tests for retry policy with exponential backoff strategies.
"""

import asyncio

import pytest
from aether_sdk.resilience.retry import (
    BackoffStrategy,
    RetryConfig,
    RetryExhaustedError,
    RetryPolicy,
    RetryResult,
    RetryStats,
    aggressive_retry_policy,
    conservative_retry_policy,
    database_retry_policy,
    network_retry_policy,
)

# ============================================
# BackoffStrategy Tests
# ============================================


class TestBackoffStrategy:
    """Tests for BackoffStrategy enum."""

    def test_fixed_strategy(self):
        """Test FIXED strategy value."""
        assert BackoffStrategy.FIXED.value == "fixed"

    def test_linear_strategy(self):
        """Test LINEAR strategy value."""
        assert BackoffStrategy.LINEAR.value == "linear"

    def test_exponential_strategy(self):
        """Test EXPONENTIAL strategy value."""
        assert BackoffStrategy.EXPONENTIAL.value == "exponential"

    def test_exponential_jitter_strategy(self):
        """Test EXPONENTIAL_JITTER strategy value."""
        assert BackoffStrategy.EXPONENTIAL_JITTER.value == "exponential-jitter"

    def test_all_strategies_defined(self):
        """Test that all expected strategies are defined."""
        strategies = list(BackoffStrategy)
        assert len(strategies) == 4


# ============================================
# RetryConfig Tests
# ============================================


class TestRetryConfig:
    """Tests for RetryConfig dataclass."""

    def test_default_config(self):
        """Test default configuration values."""
        config = RetryConfig()

        assert config.max_attempts == 3
        assert config.backoff == BackoffStrategy.EXPONENTIAL_JITTER
        assert config.base_delay_ms == 100
        assert config.max_delay_ms == 30000
        assert config.multiplier == 2.0
        assert config.jitter_factor == 0.1
        assert config.is_retryable is None
        assert config.on_retry is None
        assert config.on_exhausted is None

    def test_custom_config(self):
        """Test custom configuration values."""
        config = RetryConfig(
            max_attempts=5,
            backoff=BackoffStrategy.LINEAR,
            base_delay_ms=200,
            max_delay_ms=60000,
            multiplier=1.5,
            jitter_factor=0.2,
        )

        assert config.max_attempts == 5
        assert config.backoff == BackoffStrategy.LINEAR
        assert config.base_delay_ms == 200
        assert config.max_delay_ms == 60000
        assert config.multiplier == 1.5
        assert config.jitter_factor == 0.2

    def test_config_with_callbacks(self):
        """Test configuration with callbacks."""
        retry_called = []
        exhausted_called = []

        config = RetryConfig(
            is_retryable=lambda e, a: True,
            on_retry=lambda e, a, d: retry_called.append((str(e), a, d)),
            on_exhausted=lambda e, a: exhausted_called.append((str(e), a)),
        )

        assert config.is_retryable(Exception("test"), 1) is True
        config.on_retry(Exception("test"), 1, 100)
        config.on_exhausted(Exception("test"), 3)

        assert len(retry_called) == 1
        assert len(exhausted_called) == 1


# ============================================
# RetryStats Tests
# ============================================


class TestRetryStats:
    """Tests for RetryStats dataclass."""

    def test_default_stats(self):
        """Test default statistics values."""
        stats = RetryStats()

        assert stats.total_attempts == 0
        assert stats.successful_attempts == 0
        assert stats.failed_attempts == 0
        assert stats.retried_calls == 0
        assert stats.exhausted_calls == 0
        assert stats.total_retry_delay_ms == 0

    def test_custom_stats(self):
        """Test custom statistics values."""
        stats = RetryStats(
            total_attempts=10,
            successful_attempts=8,
            failed_attempts=2,
            retried_calls=3,
            exhausted_calls=1,
            total_retry_delay_ms=500,
        )

        assert stats.total_attempts == 10
        assert stats.successful_attempts == 8
        assert stats.failed_attempts == 2
        assert stats.retried_calls == 3
        assert stats.exhausted_calls == 1
        assert stats.total_retry_delay_ms == 500


# ============================================
# RetryResult Tests
# ============================================


class TestRetryResult:
    """Tests for RetryResult dataclass."""

    def test_retry_result_creation(self):
        """Test creating a retry result."""
        result = RetryResult(result="success", attempts=2, total_delay_ms=150)

        assert result.result == "success"
        assert result.attempts == 2
        assert result.total_delay_ms == 150


# ============================================
# RetryExhaustedError Tests
# ============================================


class TestRetryExhaustedError:
    """Tests for RetryExhaustedError exception."""

    def test_error_creation(self):
        """Test creating retry exhausted error."""
        last_error = ValueError("Final error")
        error = RetryExhaustedError(
            "All retries exhausted", last_error, attempts=3, total_delay_ms=500
        )

        assert "exhausted" in str(error)
        assert error.last_error == last_error
        assert error.attempts == 3
        assert error.total_delay_ms == 500


# ============================================
# RetryPolicy Basic Tests
# ============================================


class TestRetryPolicyBasic:
    """Basic tests for RetryPolicy class."""

    def setup_method(self):
        """Set up test fixtures."""
        self.policy = RetryPolicy()

    def test_initialization_default_config(self):
        """Test initialization with default config."""
        assert self.policy._config is not None
        assert self.policy._config.max_attempts == 3

    def test_initialization_custom_config(self):
        """Test initialization with custom config."""
        config = RetryConfig(max_attempts=5)
        policy = RetryPolicy(config)

        assert policy._config.max_attempts == 5

    def test_get_stats(self):
        """Test getting statistics."""
        stats = self.policy.get_stats()

        assert stats.total_attempts == 0
        assert stats.successful_attempts == 0

    def test_reset_stats(self):
        """Test resetting statistics."""
        # Modify stats
        self.policy._stats.total_attempts = 10

        self.policy.reset_stats()

        stats = self.policy.get_stats()
        assert stats.total_attempts == 0


# ============================================
# RetryPolicy Execute Tests
# ============================================


class TestRetryPolicyExecute:
    """Tests for RetryPolicy execute method."""

    def setup_method(self):
        """Set up test fixtures."""
        self.config = RetryConfig(
            max_attempts=3,
            base_delay_ms=10,  # Short delays for testing
            max_delay_ms=100,
        )
        self.policy = RetryPolicy(self.config)

    @pytest.mark.asyncio
    async def test_execute_success_first_attempt(self):
        """Test successful execution on first attempt."""

        async def success_func():
            return "success"

        result = await self.policy.execute(success_func)

        assert result.result == "success"
        assert result.attempts == 1
        assert result.total_delay_ms == 0

        stats = self.policy.get_stats()
        assert stats.total_attempts == 1
        assert stats.successful_attempts == 1

    @pytest.mark.asyncio
    async def test_execute_success_after_retry(self):
        """Test successful execution after retry."""
        call_count = 0

        async def eventual_success():
            nonlocal call_count
            call_count += 1
            if call_count < 2:
                raise ValueError("Temporary error")
            return "success"

        result = await self.policy.execute(eventual_success)

        assert result.result == "success"
        assert result.attempts == 2

        stats = self.policy.get_stats()
        assert stats.retried_calls == 1

    @pytest.mark.asyncio
    async def test_execute_exhausted_retries(self):
        """Test execution with exhausted retries."""
        # Use a config where errors are retryable
        config = RetryConfig(
            max_attempts=3,
            base_delay_ms=10,
            is_retryable=lambda e, a: True,  # Always retry
        )
        policy = RetryPolicy(config)

        async def always_fail():
            raise ValueError("Always fails")

        with pytest.raises(RetryExhaustedError) as exc_info:
            await policy.execute(always_fail)

        # Should have attempted 3 times
        assert exc_info.value.attempts == 3

    @pytest.mark.asyncio
    async def test_execute_safe_returns_none_on_exhaustion(self):
        """Test execute_safe returns None on exhaustion."""

        async def always_fail():
            raise ValueError("Always fails")

        result = await self.policy.execute_safe(always_fail)

        assert result is None

        stats = self.policy.get_stats()
        assert stats.exhausted_calls == 1

    @pytest.mark.asyncio
    async def test_execute_safe_returns_result_on_success(self):
        """Test execute_safe returns result on success."""

        async def success_func():
            return "success"

        result = await self.policy.execute_safe(success_func)

        assert result is not None
        assert result.result == "success"


# ============================================
# RetryPolicy Backoff Strategy Tests
# ============================================


class TestRetryPolicyBackoff:
    """Tests for retry policy backoff strategies."""

    def test_calculate_delay_fixed(self):
        """Test fixed backoff strategy."""
        config = RetryConfig(backoff=BackoffStrategy.FIXED, base_delay_ms=100)
        policy = RetryPolicy(config)

        # Fixed delay should always be the same
        assert policy._calculate_delay(1) == 100
        assert policy._calculate_delay(2) == 100
        assert policy._calculate_delay(3) == 100

    def test_calculate_delay_linear(self):
        """Test linear backoff strategy."""
        config = RetryConfig(backoff=BackoffStrategy.LINEAR, base_delay_ms=100)
        policy = RetryPolicy(config)

        # Linear: base * attempt
        assert policy._calculate_delay(1) == 100
        assert policy._calculate_delay(2) == 200
        assert policy._calculate_delay(3) == 300

    def test_calculate_delay_exponential(self):
        """Test exponential backoff strategy."""
        config = RetryConfig(
            backoff=BackoffStrategy.EXPONENTIAL, base_delay_ms=100, multiplier=2.0
        )
        policy = RetryPolicy(config)

        # Exponential: base * multiplier^(attempt-1)
        assert policy._calculate_delay(1) == 100  # 100 * 2^0
        assert policy._calculate_delay(2) == 200  # 100 * 2^1
        assert policy._calculate_delay(3) == 400  # 100 * 2^2

    def test_calculate_delay_exponential_jitter(self):
        """Test exponential with jitter backoff strategy."""
        config = RetryConfig(
            backoff=BackoffStrategy.EXPONENTIAL_JITTER,
            base_delay_ms=100,
            multiplier=2.0,
            jitter_factor=0.1,
        )
        policy = RetryPolicy(config)

        # Should be around 100 with some jitter
        delay1 = policy._calculate_delay(1)
        assert 90 <= delay1 <= 110  # 100 +/- 10%

        delay2 = policy._calculate_delay(2)
        assert 180 <= delay2 <= 220  # 200 +/- 10%

    def test_calculate_delay_respects_max(self):
        """Test that delay respects max_delay_ms."""
        config = RetryConfig(
            backoff=BackoffStrategy.EXPONENTIAL,
            base_delay_ms=1000,
            multiplier=10.0,
            max_delay_ms=5000,
        )
        policy = RetryPolicy(config)

        # 1000 * 10^2 = 100000, should cap at 5000
        assert policy._calculate_delay(3) == 5000

    def test_add_jitter(self):
        """Test jitter is added correctly."""
        config = RetryConfig(jitter_factor=0.5)
        policy = RetryPolicy(config)

        # Run multiple times to verify jitter is applied
        delays = [policy._add_jitter(100) for _ in range(10)]

        # All delays should be different (with high probability)
        assert len(set(delays)) > 1

        # All delays should be within 50% of base
        for delay in delays:
            assert 50 <= delay <= 150


# ============================================
# RetryPolicy Custom Retryable Tests
# ============================================


class TestRetryPolicyRetryable:
    """Tests for custom retryable logic."""

    @pytest.mark.asyncio
    async def test_custom_is_retryable(self):
        """Test custom is_retryable function."""
        config = RetryConfig(
            max_attempts=3,
            base_delay_ms=10,
            is_retryable=lambda e, a: "timeout" in str(e).lower(),
        )
        policy = RetryPolicy(config)

        class Counter:
            def __init__(self):
                self.value = 0

        counter = Counter()

        async def conditional_fail():
            counter.value += 1
            if counter.value == 1:
                raise ValueError("connection timeout")  # Should retry
            elif counter.value == 2:
                raise ValueError("permanent error")  # Should not retry
            return "success"

        with pytest.raises(RetryExhaustedError):
            await policy.execute(conditional_fail)

        # Should have stopped at attempt 2 due to non-retryable error
        assert counter.value == 2

    def test_default_is_retryable(self):
        """Test default is_retryable detection."""
        policy = RetryPolicy(RetryConfig(max_attempts=3, base_delay_ms=10))

        # Timeout should be retryable (lowercase "timeout" in message)
        timeout_error = ValueError("connection timeout")
        assert policy._is_retryable_default(timeout_error) is True

        # Network should be retryable (lowercase "network" in message)
        network_error = ValueError("network error")
        assert policy._is_retryable_default(network_error) is True

        # Non-transient should not be retryable
        permanent_error = ValueError("Invalid credentials")
        assert policy._is_retryable_default(permanent_error) is False


# ============================================
# RetryPolicy Callback Tests
# ============================================


class TestRetryPolicyCallbacks:
    """Tests for retry policy callbacks."""

    @pytest.mark.asyncio
    async def test_on_retry_callback(self):
        """Test on_retry callback is called."""
        retry_calls = []

        config = RetryConfig(
            max_attempts=3,
            base_delay_ms=10,
            on_retry=lambda e, a, d: retry_calls.append((str(e), a, d)),
        )
        policy = RetryPolicy(config)

        async def always_fail():
            raise ValueError("timeout error")  # Retryable error

        with pytest.raises(RetryExhaustedError):
            await policy.execute(always_fail)

        # Should have been called for each retry (2 retries after first failure)
        assert len(retry_calls) == 2

    @pytest.mark.asyncio
    async def test_on_exhausted_callback(self):
        """Test on_exhausted callback is called."""
        exhausted_calls = []

        config = RetryConfig(
            max_attempts=2,
            base_delay_ms=10,
            on_exhausted=lambda e, a: exhausted_calls.append((str(e), a)),
        )
        policy = RetryPolicy(config)

        async def always_fail():
            raise ValueError("timeout error")  # Retryable error

        with pytest.raises(RetryExhaustedError):
            await policy.execute(always_fail)

        assert len(exhausted_calls) == 1
        assert exhausted_calls[0][1] == 2  # attempts


# ============================================
# Predefined Retry Policy Tests
# ============================================


class TestPredefinedPolicies:
    """Tests for predefined retry policies."""

    def test_network_retry_policy(self):
        """Test network retry policy."""
        policy = network_retry_policy()

        assert policy._config.max_attempts == 3
        assert policy._config.backoff == BackoffStrategy.EXPONENTIAL_JITTER
        assert policy._config.base_delay_ms == 100

    def test_network_retry_policy_with_overrides(self):
        """Test network retry policy with overrides."""
        policy = network_retry_policy(max_attempts=5, base_delay_ms=200)

        assert policy._config.max_attempts == 5
        assert policy._config.base_delay_ms == 200

    def test_database_retry_policy(self):
        """Test database retry policy."""
        policy = database_retry_policy()

        assert policy._config.max_attempts == 5
        assert policy._config.backoff == BackoffStrategy.EXPONENTIAL
        assert policy._config.base_delay_ms == 50

    def test_aggressive_retry_policy(self):
        """Test aggressive retry policy."""
        policy = aggressive_retry_policy()

        assert policy._config.max_attempts == 10
        assert policy._config.base_delay_ms == 10
        assert policy._config.multiplier == 1.5

    def test_conservative_retry_policy(self):
        """Test conservative retry policy."""
        policy = conservative_retry_policy()

        assert policy._config.max_attempts == 2
        assert policy._config.base_delay_ms == 1000
        assert policy._config.multiplier == 3.0


# ============================================
# Integration Tests
# ============================================


class TestRetryIntegration:
    """Integration tests for retry policy."""

    @pytest.mark.asyncio
    async def test_retry_with_eventual_success(self):
        """Test complete retry flow with eventual success."""
        config = RetryConfig(
            max_attempts=5,
            backoff=BackoffStrategy.EXPONENTIAL,
            base_delay_ms=10,
            multiplier=2.0,
        )
        policy = RetryPolicy(config)

        call_count = 0

        async def flaky_service():
            nonlocal call_count
            call_count += 1
            if call_count < 4:
                raise ValueError("Service unavailable")
            return {"status": "ok", "data": "result"}

        result = await policy.execute(flaky_service)

        assert result.result == {"status": "ok", "data": "result"}
        assert result.attempts == 4

        stats = policy.get_stats()
        assert stats.total_attempts == 4
        assert stats.successful_attempts == 1
        assert stats.failed_attempts == 3
        assert stats.retried_calls == 1

    @pytest.mark.asyncio
    async def test_concurrent_retries(self):
        """Test concurrent retry operations."""
        # Use a config where errors are always retryable
        config = RetryConfig(
            max_attempts=2,
            base_delay_ms=10,
            is_retryable=lambda e, a: True,  # Always retry
        )
        policy = RetryPolicy(config)

        async def always_fail():
            raise ValueError("timeout error")  # Retryable error

        async def run_retry():
            try:
                return await policy.execute(always_fail)
            except RetryExhaustedError:
                return "exhausted"

        results = await asyncio.gather(*[run_retry() for _ in range(5)])

        assert all(r == "exhausted" for r in results)
        # Each of 5 calls should exhaust after 2 attempts
        assert policy.get_stats().exhausted_calls == 5


# ============================================
# Edge Cases
# ============================================


class TestRetryEdgeCases:
    """Edge case tests for retry policy."""

    @pytest.mark.asyncio
    async def test_single_attempt(self):
        """Test with single attempt (no retries)."""
        config = RetryConfig(max_attempts=1, base_delay_ms=10)
        policy = RetryPolicy(config)

        async def always_fail():
            raise ValueError("Fail")

        with pytest.raises(RetryExhaustedError) as exc_info:
            await policy.execute(always_fail)

        assert exc_info.value.attempts == 1
        assert exc_info.value.total_delay_ms == 0

    @pytest.mark.asyncio
    async def test_zero_base_delay(self):
        """Test with zero base delay."""
        config = RetryConfig(
            max_attempts=3, base_delay_ms=0, backoff=BackoffStrategy.FIXED
        )
        policy = RetryPolicy(config)

        async def always_fail():
            raise ValueError("Fail")

        start = asyncio.get_event_loop().time()

        with pytest.raises(RetryExhaustedError):
            await policy.execute(always_fail)

        # Should complete very quickly with zero delay
        elapsed = asyncio.get_event_loop().time() - start
        assert elapsed < 0.1

    def test_large_multiplier(self):
        """Test with large multiplier."""
        config = RetryConfig(
            backoff=BackoffStrategy.EXPONENTIAL,
            base_delay_ms=1,
            multiplier=100.0,
            max_delay_ms=1000000,
        )
        policy = RetryPolicy(config)

        # 1 * 100^2 = 10000
        assert policy._calculate_delay(3) == 10000

    @pytest.mark.asyncio
    async def test_return_value_preserved(self):
        """Test that complex return values are preserved."""
        policy = RetryPolicy(RetryConfig(base_delay_ms=10))

        complex_result = {
            "items": [1, 2, 3],
            "nested": {"a": "b"},
            "none": None,
            "bool": True,
        }

        async def return_complex():
            return complex_result

        result = await policy.execute(return_complex)

        assert result.result == complex_result

    @pytest.mark.asyncio
    async def test_exception_type_preserved(self):
        """Test that original exception type is accessible."""
        policy = RetryPolicy(RetryConfig(max_attempts=2, base_delay_ms=10))

        class CustomError(Exception):
            pass

        async def raise_custom():
            raise CustomError("Custom error")

        with pytest.raises(RetryExhaustedError) as exc_info:
            await policy.execute(raise_custom)

        assert isinstance(exc_info.value.last_error, CustomError)
