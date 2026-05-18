"""
Tests for Aether SDK Circuit Breaker Module

Tests for circuit breaker pattern implementation including state transitions,
failure handling, and manager functionality.
"""

import asyncio
import time

import pytest
from aether_sdk.resilience.circuit_breaker import (
    CircuitBreaker,
    CircuitBreakerConfig,
    CircuitBreakerError,
    CircuitBreakerManager,
    CircuitBreakerStats,
    CircuitState,
    FailureRecord,
)

# ============================================
# CircuitState Tests
# ============================================


class TestCircuitState:
    """Tests for CircuitState enum."""

    def test_closed_state(self):
        """Test CLOSED state value."""
        assert CircuitState.CLOSED.value == "closed"

    def test_open_state(self):
        """Test OPEN state value."""
        assert CircuitState.OPEN.value == "open"

    def test_half_open_state(self):
        """Test HALF_OPEN state value."""
        assert CircuitState.HALF_OPEN.value == "half-open"

    def test_all_states_defined(self):
        """Test that all expected states are defined."""
        states = list(CircuitState)
        assert len(states) == 3


# ============================================
# CircuitBreakerConfig Tests
# ============================================


class TestCircuitBreakerConfig:
    """Tests for CircuitBreakerConfig dataclass."""

    def test_default_config(self):
        """Test default configuration values."""
        config = CircuitBreakerConfig()

        assert config.failure_threshold == 5
        assert config.success_threshold == 3
        assert config.timeout_ms == 30000
        assert config.half_open_max_calls == 3
        assert config.failure_window_ms == 60000
        assert config.on_open is None
        assert config.on_close is None
        assert config.on_half_open is None

    def test_custom_config(self):
        """Test custom configuration values."""
        config = CircuitBreakerConfig(
            failure_threshold=10,
            success_threshold=5,
            timeout_ms=60000,
            half_open_max_calls=5,
            failure_window_ms=120000,
        )

        assert config.failure_threshold == 10
        assert config.success_threshold == 5
        assert config.timeout_ms == 60000
        assert config.half_open_max_calls == 5
        assert config.failure_window_ms == 120000

    def test_config_with_callbacks(self):
        """Test configuration with callbacks."""
        open_called = []
        close_called = []
        half_open_called = []

        config = CircuitBreakerConfig(
            on_open=lambda: open_called.append(True),
            on_close=lambda: close_called.append(True),
            on_half_open=lambda: half_open_called.append(True),
        )

        config.on_open()
        config.on_close()
        config.on_half_open()

        assert len(open_called) == 1
        assert len(close_called) == 1
        assert len(half_open_called) == 1


# ============================================
# CircuitBreakerStats Tests
# ============================================


class TestCircuitBreakerStats:
    """Tests for CircuitBreakerStats dataclass."""

    def test_default_stats(self):
        """Test default statistics values."""
        stats = CircuitBreakerStats()

        assert stats.state == CircuitState.CLOSED
        assert stats.failures == 0
        assert stats.successes == 0
        assert stats.rejected_calls == 0
        assert stats.total_calls == 0
        assert stats.last_failure is None
        assert stats.last_success is None
        assert stats.last_state_change is None


# ============================================
# FailureRecord Tests
# ============================================


class TestFailureRecord:
    """Tests for FailureRecord dataclass."""

    def test_failure_record_creation(self):
        """Test creating a failure record."""
        error = ValueError("Test error")
        record = FailureRecord(timestamp=time.time(), error=error)

        assert record.timestamp > 0
        assert record.error == error


# ============================================
# CircuitBreaker Basic Tests
# ============================================


class TestCircuitBreakerBasic:
    """Basic tests for CircuitBreaker class."""

    def setup_method(self):
        """Set up test fixtures."""
        self.breaker = CircuitBreaker()

    def test_initialization_default_config(self):
        """Test initialization with default config."""
        assert self.breaker.state == CircuitState.CLOSED
        assert self.breaker.is_closed is True
        assert self.breaker.is_open is False
        assert self.breaker.is_half_open is False

    def test_initialization_custom_config(self):
        """Test initialization with custom config."""
        config = CircuitBreakerConfig(failure_threshold=10)
        breaker = CircuitBreaker(config)

        assert breaker.state == CircuitState.CLOSED

    def test_get_stats(self):
        """Test getting statistics."""
        stats = self.breaker.get_stats()

        assert stats.state == CircuitState.CLOSED
        assert stats.failures == 0
        assert stats.total_calls == 0


# ============================================
# CircuitBreaker Execute Tests
# ============================================


class TestCircuitBreakerExecute:
    """Tests for CircuitBreaker execute method."""

    def setup_method(self):
        """Set up test fixtures."""
        self.config = CircuitBreakerConfig(
            failure_threshold=3,
            success_threshold=2,
            timeout_ms=100,
            half_open_max_calls=2,
        )
        self.breaker = CircuitBreaker(self.config)

    @pytest.mark.asyncio
    async def test_execute_success(self):
        """Test successful execution."""

        async def success_func():
            return "success"

        result = await self.breaker.execute(success_func)

        assert result == "success"
        stats = self.breaker.get_stats()
        assert stats.total_calls == 1

    @pytest.mark.asyncio
    async def test_execute_failure(self):
        """Test failed execution."""

        async def fail_func():
            raise ValueError("Test error")

        with pytest.raises(ValueError, match="Test error"):
            await self.breaker.execute(fail_func)

        stats = self.breaker.get_stats()
        assert stats.failures == 1
        assert stats.total_calls == 1

    @pytest.mark.asyncio
    async def test_execute_opens_after_threshold(self):
        """Test circuit opens after failure threshold."""

        async def fail_func():
            raise ValueError("Test error")

        # Fail 3 times (threshold)
        for _ in range(3):
            with pytest.raises(ValueError):
                await self.breaker.execute(fail_func)

        assert self.breaker.is_open

    @pytest.mark.asyncio
    async def test_execute_rejected_when_open(self):
        """Test execution is rejected when circuit is open."""
        # Force open
        self.breaker.force_open()

        async def success_func():
            return "success"

        with pytest.raises(CircuitBreakerError, match="Circuit breaker is open"):
            await self.breaker.execute(success_func)

        stats = self.breaker.get_stats()
        assert stats.rejected_calls == 1


# ============================================
# CircuitBreaker State Transition Tests
# ============================================


class TestCircuitBreakerStateTransitions:
    """Tests for circuit breaker state transitions."""

    def setup_method(self):
        """Set up test fixtures."""
        self.config = CircuitBreakerConfig(
            failure_threshold=2,
            success_threshold=2,
            timeout_ms=50,  # Short timeout for testing
            half_open_max_calls=3,
        )
        self.breaker = CircuitBreaker(self.config)

    def test_force_open(self):
        """Test forcing circuit open."""
        self.breaker.force_open()

        assert self.breaker.is_open
        assert self.breaker.state == CircuitState.OPEN

    def test_force_close(self):
        """Test forcing circuit closed."""
        self.breaker.force_open()
        self.breaker.force_close()

        assert self.breaker.is_closed
        assert self.breaker.state == CircuitState.CLOSED

    def test_reset(self):
        """Test resetting circuit breaker."""
        self.breaker.force_open()
        self.breaker.reset()

        assert self.breaker.is_closed
        stats = self.breaker.get_stats()
        assert stats.failures == 0
        assert stats.successes == 0
        assert stats.rejected_calls == 0
        assert stats.total_calls == 0

    @pytest.mark.asyncio
    async def test_closed_to_open_transition(self):
        """Test transition from closed to open."""

        async def fail_func():
            raise ValueError("error")

        # Trigger failures
        for _ in range(2):
            with pytest.raises(ValueError):
                await self.breaker.execute(fail_func)

        assert self.breaker.is_open

    @pytest.mark.asyncio
    async def test_open_to_half_open_transition(self):
        """Test transition from open to half-open after timeout."""
        self.breaker.force_open()

        # Wait for timeout
        await asyncio.sleep(0.1)

        async def success_func():
            return "success"

        # Should transition to half-open and execute
        result = await self.breaker.execute(success_func)
        assert result == "success"
        assert self.breaker.is_half_open

    @pytest.mark.asyncio
    async def test_half_open_to_closed_transition(self):
        """Test transition from half-open to closed after successes."""
        self.breaker.force_open()
        await asyncio.sleep(0.1)  # Wait for timeout

        async def success_func():
            return "success"

        # Need success_threshold successes
        for _ in range(2):
            await self.breaker.execute(success_func)

        assert self.breaker.is_closed

    @pytest.mark.asyncio
    async def test_half_open_to_open_transition(self):
        """Test transition from half-open to open on failure."""
        self.breaker.force_open()
        await asyncio.sleep(0.1)  # Wait for timeout

        async def fail_func():
            raise ValueError("error")

        # First call transitions to half-open
        # Second call (failure) transitions back to open
        async def success_func():
            return "success"

        await self.breaker.execute(success_func)  # half-open now

        with pytest.raises(ValueError):
            await self.breaker.execute(fail_func)  # failure -> open

        assert self.breaker.is_open


# ============================================
# CircuitBreaker Half-Open Tests
# ============================================


class TestCircuitBreakerHalfOpen:
    """Tests for circuit breaker half-open state."""

    def setup_method(self):
        """Set up test fixtures."""
        self.config = CircuitBreakerConfig(
            failure_threshold=2,
            success_threshold=2,
            timeout_ms=50,
            half_open_max_calls=2,
        )
        self.breaker = CircuitBreaker(self.config)

    @pytest.mark.asyncio
    async def test_half_open_max_calls_limit(self):
        """Test half-open max calls limit."""
        self.breaker.force_open()
        await asyncio.sleep(0.1)  # Wait for timeout

        async def slow_func():
            await asyncio.sleep(0.5)
            return "success"

        # Start two slow calls
        tasks = [asyncio.create_task(self.breaker.execute(slow_func)) for _ in range(2)]

        await asyncio.sleep(0.05)  # Let them start

        # Third call should be rejected
        async def quick_func():
            return "quick"

        with pytest.raises(CircuitBreakerError, match="at max calls"):
            await self.breaker.execute(quick_func)

        # Cancel pending tasks
        for task in tasks:
            task.cancel()


# ============================================
# CircuitBreaker Callback Tests
# ============================================


class TestCircuitBreakerCallbacks:
    """Tests for circuit breaker callbacks."""

    def test_on_open_callback(self):
        """Test on_open callback is called."""
        called = []
        config = CircuitBreakerConfig(
            failure_threshold=1, on_open=lambda: called.append("open")
        )
        breaker = CircuitBreaker(config)

        breaker.force_open()

        assert "open" in called

    def test_on_close_callback(self):
        """Test on_close callback is called."""
        called = []
        config = CircuitBreakerConfig(on_close=lambda: called.append("close"))
        breaker = CircuitBreaker(config)

        breaker.force_open()
        breaker.force_close()

        assert "close" in called

    def test_on_half_open_callback(self):
        """Test on_half_open callback is called."""
        called = []
        config = CircuitBreakerConfig(
            timeout_ms=50, on_half_open=lambda: called.append("half_open")
        )
        breaker = CircuitBreaker(config)

        breaker.force_open()
        # Directly transition to half-open
        breaker._transition_to(CircuitState.HALF_OPEN)

        assert "half_open" in called


# ============================================
# CircuitBreakerManager Tests
# ============================================


class TestCircuitBreakerManager:
    """Tests for CircuitBreakerManager class."""

    def setup_method(self):
        """Set up test fixtures."""
        self.manager = CircuitBreakerManager()

    def test_initialization(self):
        """Test manager initialization."""
        assert self.manager._breakers == {}

    def test_get_creates_new_breaker(self):
        """Test getting a new breaker creates it."""
        breaker = self.manager.get("service-a")

        assert breaker is not None
        assert "service-a" in self.manager._breakers

    def test_get_returns_existing_breaker(self):
        """Test getting existing breaker returns same instance."""
        breaker1 = self.manager.get("service-a")
        breaker2 = self.manager.get("service-a")

        assert breaker1 is breaker2

    def test_get_with_custom_config(self):
        """Test getting breaker with custom config."""
        config = CircuitBreakerConfig(failure_threshold=10)
        breaker = self.manager.get("service-b", config)

        # Verify config is applied
        assert breaker._config.failure_threshold == 10

    def test_get_all_stats(self):
        """Test getting all breaker statistics."""
        self.manager.get("service-a")
        self.manager.get("service-b")

        stats = self.manager.get_all_stats()

        assert "service-a" in stats
        assert "service-b" in stats
        assert len(stats) == 2

    def test_reset_all(self):
        """Test resetting all breakers."""
        breaker1 = self.manager.get("service-a")
        breaker2 = self.manager.get("service-b")

        breaker1.force_open()
        breaker2.force_open()

        self.manager.reset_all()

        assert breaker1.is_closed
        assert breaker2.is_closed

    def test_get_open_breakers(self):
        """Test getting names of open breakers."""
        breaker1 = self.manager.get("service-a")
        breaker2 = self.manager.get("service-b")
        self.manager.get("service-c")

        breaker1.force_open()
        breaker2.force_open()
        # breaker3 stays closed

        open_names = self.manager.get_open_breakers()

        assert "service-a" in open_names
        assert "service-b" in open_names
        assert "service-c" not in open_names

    def test_manager_with_default_config(self):
        """Test manager with default config."""
        default_config = CircuitBreakerConfig(failure_threshold=10, success_threshold=5)
        manager = CircuitBreakerManager(default_config)

        breaker = manager.get("service-d")

        assert breaker._config.failure_threshold == 10
        assert breaker._config.success_threshold == 5


# ============================================
# CircuitBreaker Failure Window Tests
# ============================================


class TestCircuitBreakerFailureWindow:
    """Tests for circuit breaker failure window."""

    def setup_method(self):
        """Set up test fixtures."""
        self.config = CircuitBreakerConfig(
            failure_threshold=3, failure_window_ms=200  # Short window for testing
        )
        self.breaker = CircuitBreaker(self.config)

    @pytest.mark.asyncio
    async def test_failure_window_expires_old_failures(self):
        """Test that old failures expire from window."""

        async def fail_func():
            raise ValueError("error")

        # Fail twice
        for _ in range(2):
            with pytest.raises(ValueError):
                await self.breaker.execute(fail_func)

        assert len(self.breaker._failure_history) == 2

        # Wait for window to expire
        await asyncio.sleep(0.3)

        # Fail once more - should trigger cleanup
        with pytest.raises(ValueError):
            await self.breaker.execute(fail_func)

        # Old failures should be cleaned up
        assert len(self.breaker._failure_history) == 1


# ============================================
# CircuitBreaker Edge Cases
# ============================================


class TestCircuitBreakerEdgeCases:
    """Edge case tests for circuit breaker."""

    def setup_method(self):
        """Set up test fixtures."""
        self.breaker = CircuitBreaker()

    @pytest.mark.asyncio
    async def test_concurrent_executions(self):
        """Test concurrent executions through breaker."""

        async def slow_success():
            await asyncio.sleep(0.01)
            return "success"

        async def run_execute():
            return await self.breaker.execute(slow_success)

        results = await asyncio.gather(*[run_execute() for _ in range(10)])

        assert all(r == "success" for r in results)
        assert self.breaker.get_stats().total_calls == 10

    def test_transition_to_same_state_noop(self):
        """Test transitioning to same state is a no-op."""
        callback_called = []
        config = CircuitBreakerConfig(on_open=lambda: callback_called.append(True))
        breaker = CircuitBreaker(config)

        # Already closed, transition to closed
        breaker._transition_to(CircuitState.CLOSED)

        # Callback should not be called
        assert len(callback_called) == 0

    @pytest.mark.asyncio
    async def test_success_in_closed_resets_failures(self):
        """Test success in closed state resets failure count."""
        config = CircuitBreakerConfig(failure_threshold=3)
        breaker = CircuitBreaker(config)

        async def fail_func():
            raise ValueError("error")

        async def success_func():
            return "success"

        # Fail twice
        for _ in range(2):
            with pytest.raises(ValueError):
                await breaker.execute(fail_func)

        assert breaker.get_stats().failures == 2

        # Success resets failures
        await breaker.execute(success_func)

        assert breaker.get_stats().failures == 0
