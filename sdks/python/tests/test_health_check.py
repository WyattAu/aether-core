"""
Tests for Aether SDK Health Check Module

Tests for health checking functionality compatible with Kubernetes probes.
"""

import asyncio

import pytest

from aether_sdk.resilience.health_check import (
    HealthChecker,
    HealthCheckOptions,
    HealthCheckResult,
    HealthStatus,
    dependency_health_check,
    memory_health_check,
    ping_health_check,
    state_health_check,
)

# ============================================
# Fixtures
# ============================================


@pytest.fixture
def health_checker():
    """Create a health checker instance."""
    return HealthChecker(service_id="test-service", version="1.0.0")


@pytest.fixture
def healthy_check():
    """Create a healthy check function."""

    def check() -> HealthCheckResult:
        return HealthCheckResult(
            status=HealthStatus.HEALTHY,
            component_id="test",
            component_type="test",
            observed_value=1,
            observed_unit="count",
        )

    return check


@pytest.fixture
def degraded_check():
    """Create a degraded check function."""

    def check() -> HealthCheckResult:
        return HealthCheckResult(
            status=HealthStatus.DEGRADED,
            component_id="test-degraded",
            component_type="test",
        )

    return check


@pytest.fixture
def unhealthy_check():
    """Create an unhealthy check function."""

    def check() -> HealthCheckResult:
        return HealthCheckResult(
            status=HealthStatus.UNHEALTHY,
            component_id="test-unhealthy",
            component_type="test",
        )

    return check


@pytest.fixture
def async_check():
    """Create an async check function."""

    async def check() -> HealthCheckResult:
        await asyncio.sleep(0.01)
        return HealthCheckResult(
            status=HealthStatus.HEALTHY,
            component_id="async-test",
            component_type="test",
        )

    return check


# ============================================
# HealthCheckResult Tests
# ============================================


class TestHealthCheckResult:
    """Tests for HealthCheckResult dataclass."""

    def test_minimal_result(self):
        """Test creating minimal result."""
        result = HealthCheckResult(
            status=HealthStatus.HEALTHY,
            component_id="test",
            component_type="test",
        )
        assert result.status == HealthStatus.HEALTHY
        assert result.component_id == "test"
        assert result.observed_value is None
        assert result.details == {}

    def test_full_result(self):
        """Test creating full result."""
        result = HealthCheckResult(
            status=HealthStatus.DEGRADED,
            component_id="memory",
            component_type="system",
            observed_value=80,
            observed_unit="%",
            output="High memory usage",
            time="2024-01-01T00:00:00Z",
            details={"used_mb": 800, "total_mb": 1000},
        )
        assert result.status == HealthStatus.DEGRADED
        assert result.observed_value == 80
        assert result.details["used_mb"] == 800


# ============================================
# HealthCheckOptions Tests
# ============================================


class TestHealthCheckOptions:
    """Tests for HealthCheckOptions dataclass."""

    def test_default_options(self):
        """Test default options."""
        options = HealthCheckOptions()
        assert options.timeout_ms == 5000
        assert options.critical is False
        assert options.interval_ms == 0
        assert options.cache_duration_ms == 0

    def test_custom_options(self):
        """Test custom options."""
        options = HealthCheckOptions(
            timeout_ms=10000,
            critical=True,
            interval_ms=1000,
            cache_duration_ms=500,
        )
        assert options.timeout_ms == 10000
        assert options.critical is True
        assert options.interval_ms == 1000
        assert options.cache_duration_ms == 500


# ============================================
# HealthChecker Registration Tests
# ============================================


class TestHealthCheckerRegistration:
    """Tests for health check registration."""

    @pytest.mark.asyncio
    async def test_register_check(self, health_checker, healthy_check):
        """Test registering a health check."""
        await health_checker.register_check("test", healthy_check)

        assert "test" in health_checker._checks
        assert health_checker._checks["test"]["fn"] == healthy_check

    @pytest.mark.asyncio
    async def test_register_check_with_options(self, health_checker, healthy_check):
        """Test registering with custom options."""
        options = HealthCheckOptions(timeout_ms=10000, critical=True)
        await health_checker.register_check("test", healthy_check, options)

        assert health_checker._checks["test"]["options"].timeout_ms == 10000
        assert health_checker._checks["test"]["options"].critical is True

    @pytest.mark.asyncio
    async def test_unregister_check(self, health_checker, healthy_check):
        """Test unregistering a health check."""
        await health_checker.register_check("test", healthy_check)
        assert "test" in health_checker._checks

        await health_checker.unregister_check("test")
        assert "test" not in health_checker._checks

    @pytest.mark.asyncio
    async def test_unregister_nonexistent_check(self, health_checker):
        """Test unregistering a check that doesn't exist."""
        # Should not raise
        await health_checker.unregister_check("nonexistent")

    @pytest.mark.asyncio
    async def test_register_check_with_interval(self, health_checker):
        """Test registering a check with interval."""
        call_count = []

        def check():
            call_count.append(1)
            return HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="interval-test",
                component_type="test",
            )

        options = HealthCheckOptions(interval_ms=50)
        await health_checker.register_check("interval-test", check, options)

        # Wait for a few intervals
        await asyncio.sleep(0.15)

        # Should have been called at least twice
        assert len(call_count) >= 2

        # Clean up
        await health_checker.unregister_check("interval-test")


# ============================================
# HealthChecker Run Check Tests
# ============================================


class TestHealthCheckerRunCheck:
    """Tests for running individual health checks."""

    @pytest.mark.asyncio
    async def test_run_check_sync(self, health_checker, healthy_check):
        """Test running a sync health check."""
        await health_checker.register_check("test", healthy_check)

        result = await health_checker.run_check("test")

        assert result.status == HealthStatus.HEALTHY
        assert result.component_id == "test"

    @pytest.mark.asyncio
    async def test_run_check_async(self, health_checker, async_check):
        """Test running an async health check."""
        await health_checker.register_check("async-test", async_check)

        result = await health_checker.run_check("async-test")

        assert result.status == HealthStatus.HEALTHY
        assert result.component_id == "async-test"

    @pytest.mark.asyncio
    async def test_run_check_not_found(self, health_checker):
        """Test running a check that doesn't exist."""
        result = await health_checker.run_check("nonexistent")

        assert result.status == HealthStatus.UNHEALTHY
        assert "not found" in result.output.lower()

    @pytest.mark.asyncio
    async def test_run_check_with_caching(self, health_checker):
        """Test check result caching."""
        call_count = []

        def check():
            call_count.append(1)
            return HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="cached-test",
                component_type="test",
            )

        options = HealthCheckOptions(cache_duration_ms=1000)
        await health_checker.register_check("cached-test", check, options)

        # Run twice
        await health_checker.run_check("cached-test")
        await health_checker.run_check("cached-test")

        # Should only have been called once due to caching
        assert len(call_count) == 1

    @pytest.mark.asyncio
    async def test_run_check_cache_expired(self, health_checker):
        """Test that cache expires after duration."""
        call_count = []

        def check():
            call_count.append(1)
            return HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="cache-expire-test",
                component_type="test",
            )

        options = HealthCheckOptions(cache_duration_ms=50)
        await health_checker.register_check("cache-expire-test", check, options)

        # Run once
        await health_checker.run_check("cache-expire-test")

        # Wait for cache to expire
        await asyncio.sleep(0.1)

        # Run again
        await health_checker.run_check("cache-expire-test")

        # Should have been called twice
        assert len(call_count) == 2

    @pytest.mark.asyncio
    async def test_run_check_timeout(self, health_checker):
        """Test check timeout."""

        async def slow_check():
            await asyncio.sleep(1)
            return HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="slow-test",
                component_type="test",
            )

        options = HealthCheckOptions(timeout_ms=50)
        await health_checker.register_check("slow-test", slow_check, options)

        result = await health_checker.run_check("slow-test")

        assert result.status == HealthStatus.UNHEALTHY
        assert "timed out" in result.output.lower()

    @pytest.mark.asyncio
    async def test_run_check_exception(self, health_checker):
        """Test check that raises exception."""

        def failing_check():
            raise ValueError("Check failed!")

        await health_checker.register_check("failing-test", failing_check)

        result = await health_checker.run_check("failing-test")

        assert result.status == HealthStatus.UNHEALTHY
        assert "Check failed!" in result.output


# ============================================
# HealthChecker Run All Tests
# ============================================


class TestHealthCheckerRunAll:
    """Tests for running all health checks."""

    @pytest.mark.asyncio
    async def test_run_all_healthy(self, health_checker, healthy_check):
        """Test run_all with all healthy checks."""
        await health_checker.register_check("check1", healthy_check)
        await health_checker.register_check("check2", healthy_check)

        report = await health_checker.run_all()

        assert report.status == HealthStatus.HEALTHY
        assert len(report.checks) == 2
        assert report.service_id == "test-service"
        assert report.version == "1.0.0"
        assert report.uptime >= 0

    @pytest.mark.asyncio
    async def test_run_all_with_degraded(
        self, health_checker, healthy_check, degraded_check
    ):
        """Test run_all with degraded check."""
        await health_checker.register_check("healthy", healthy_check)
        await health_checker.register_check("degraded", degraded_check)

        report = await health_checker.run_all()

        assert report.status == HealthStatus.DEGRADED

    @pytest.mark.asyncio
    async def test_run_all_with_critical_unhealthy(
        self, health_checker, healthy_check, unhealthy_check
    ):
        """Test run_all with critical unhealthy check."""
        await health_checker.register_check("healthy", healthy_check)
        await health_checker.register_check(
            "critical-unhealthy",
            unhealthy_check,
            HealthCheckOptions(critical=True),
        )

        report = await health_checker.run_all()

        assert report.status == HealthStatus.UNHEALTHY

    @pytest.mark.asyncio
    async def test_run_all_with_non_critical_unhealthy(
        self, health_checker, healthy_check, unhealthy_check
    ):
        """Test run_all with non-critical unhealthy check."""
        await health_checker.register_check("healthy", healthy_check)
        await health_checker.register_check(
            "non-critical-unhealthy",
            unhealthy_check,
            HealthCheckOptions(critical=False),
        )

        report = await health_checker.run_all()

        # Non-critical unhealthy should result in degraded
        assert report.status == HealthStatus.DEGRADED


# ============================================
# HealthChecker Probe Tests
# ============================================


class TestHealthCheckerProbes:
    """Tests for Kubernetes-style probes."""

    @pytest.mark.asyncio
    async def test_get_liveness(self, health_checker):
        """Test liveness probe."""
        result = await health_checker.get_liveness()

        assert result["alive"] is True
        assert "time" in result

    @pytest.mark.asyncio
    async def test_get_readiness_healthy(self, health_checker, healthy_check):
        """Test readiness probe when healthy."""
        await health_checker.register_check(
            "critical-check",
            healthy_check,
            HealthCheckOptions(critical=True),
        )

        result = await health_checker.get_readiness()

        assert result["ready"] is True
        assert "time" in result

    @pytest.mark.asyncio
    async def test_get_readiness_unhealthy(self, health_checker, unhealthy_check):
        """Test readiness probe when unhealthy."""
        await health_checker.register_check(
            "critical-check",
            unhealthy_check,
            HealthCheckOptions(critical=True),
        )

        result = await health_checker.get_readiness()

        assert result["ready"] is False

    @pytest.mark.asyncio
    async def test_get_readiness_non_critical_unhealthy(
        self, health_checker, healthy_check, unhealthy_check
    ):
        """Test readiness with non-critical unhealthy check."""
        await health_checker.register_check(
            "critical",
            healthy_check,
            HealthCheckOptions(critical=True),
        )
        await health_checker.register_check(
            "non-critical",
            unhealthy_check,
            HealthCheckOptions(critical=False),
        )

        result = await health_checker.get_readiness()

        # Non-critical shouldn't affect readiness
        assert result["ready"] is True

    @pytest.mark.asyncio
    async def test_get_startup(self, health_checker):
        """Test startup probe."""
        result = await health_checker.get_startup()

        assert result["started"] is True
        assert "time" in result


# ============================================
# HealthChecker Shutdown Tests
# ============================================


class TestHealthCheckerShutdown:
    """Tests for health checker shutdown."""

    @pytest.mark.asyncio
    async def test_shutdown_cancels_interval_tasks(self, health_checker):
        """Test that shutdown cancels interval tasks."""

        def check():
            return HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="test",
                component_type="test",
            )

        options = HealthCheckOptions(interval_ms=100)
        await health_checker.register_check("interval-check", check, options)

        # Verify task is running
        assert health_checker._checks["interval-check"]["task"] is not None

        # Shutdown
        await health_checker.shutdown()

        # Checks should be cleared
        assert len(health_checker._checks) == 0

    @pytest.mark.asyncio
    async def test_shutdown_clears_all_checks(self, health_checker, healthy_check):
        """Test that shutdown clears all checks."""
        await health_checker.register_check("check1", healthy_check)
        await health_checker.register_check("check2", healthy_check)

        await health_checker.shutdown()

        assert len(health_checker._checks) == 0


# ============================================
# Predefined Health Checks Tests
# ============================================


class TestPingHealthCheck:
    """Tests for ping_health_check."""

    def test_ping_check_returns_healthy(self):
        """Test that ping check returns healthy."""
        check = ping_health_check()
        result = check()

        assert result.status == HealthStatus.HEALTHY
        assert result.component_id == "ping"
        assert result.observed_value == 1
        assert result.observed_unit == "ms"


class TestMemoryHealthCheck:
    """Tests for memory_health_check."""

    def test_memory_check_healthy(self):
        """Test memory check when healthy."""
        check = memory_health_check(max_heap_mb=10000, warn_threshold=0.8)
        result = check()

        # Memory usage should be low in tests
        assert result.status in (HealthStatus.HEALTHY, HealthStatus.DEGRADED)
        assert result.component_id == "memory"
        assert result.observed_unit == "MB"

    def test_memory_check_with_low_limit(self):
        """Test memory check with very low limit."""
        # Set a very low limit to trigger unhealthy
        check = memory_health_check(max_heap_mb=1, warn_threshold=0.5)
        result = check()

        # Should be unhealthy or degraded due to low limit
        assert result.status in (HealthStatus.DEGRADED, HealthStatus.UNHEALTHY)

    def test_memory_check_includes_details(self):
        """Test that memory check includes details."""
        check = memory_health_check()
        result = check()

        assert "heap_used_mb" in result.details
        assert "heap_total_mb" in result.details
        assert "rss_mb" in result.details


class TestStateHealthCheck:
    """Tests for state_health_check."""

    @pytest.mark.asyncio
    async def test_state_check_healthy(self):
        """Test state check when accessible."""

        async def read_fn(key):
            return True

        check = state_health_check("test-key", read_fn)
        result = await check()

        assert result.status == HealthStatus.HEALTHY
        assert result.component_id == "state-storage"
        assert result.observed_unit == "ms"

    @pytest.mark.asyncio
    async def test_state_check_degraded_slow(self):
        """Test state check degraded when slow."""

        async def slow_read(key):
            await asyncio.sleep(1.1)  # 1100ms > 1000ms threshold
            return True

        check = state_health_check("test-key", slow_read)
        result = await check()

        assert result.status == HealthStatus.DEGRADED
        assert result.observed_value > 1000

    @pytest.mark.asyncio
    async def test_state_check_unhealthy_error(self):
        """Test state check unhealthy on error."""

        async def failing_read(key):
            raise Exception("Connection refused")

        check = state_health_check("test-key", failing_read)
        result = await check()

        assert result.status == HealthStatus.UNHEALTHY
        assert "Connection refused" in result.output


class TestDependencyHealthCheck:
    """Tests for dependency_health_check."""

    @pytest.mark.asyncio
    async def test_dependency_check_healthy(self):
        """Test dependency check when healthy."""

        async def check_fn():
            return True

        check = dependency_health_check("database", check_fn)
        result = await check()

        assert result.status == HealthStatus.HEALTHY
        assert result.component_id == "database"
        assert result.observed_unit == "ms"

    @pytest.mark.asyncio
    async def test_dependency_check_unhealthy_false(self):
        """Test dependency check when returns False."""

        async def check_fn():
            return False

        check = dependency_health_check("database", check_fn)
        result = await check()

        assert result.status == HealthStatus.UNHEALTHY

    @pytest.mark.asyncio
    async def test_dependency_check_timeout(self):
        """Test dependency check timeout."""

        async def slow_check():
            await asyncio.sleep(1)
            return True

        check = dependency_health_check("slow-service", slow_check, timeout_ms=50)
        result = await check()

        assert result.status == HealthStatus.UNHEALTHY
        assert "timed out" in result.output.lower()

    @pytest.mark.asyncio
    async def test_dependency_check_exception(self):
        """Test dependency check with exception."""

        async def failing_check():
            raise Exception("Connection failed")

        check = dependency_health_check("failing-service", failing_check)
        result = await check()

        assert result.status == HealthStatus.UNHEALTHY
        assert "Connection failed" in result.output


# ============================================
# Health Checker Integration Tests
# ============================================


class TestHealthCheckerIntegration:
    """Integration tests for health checker."""

    @pytest.mark.asyncio
    async def test_full_health_check_workflow(self, health_checker):
        """Test complete health check workflow."""
        # Register various checks
        await health_checker.register_check("ping", ping_health_check())
        await health_checker.register_check("memory", memory_health_check())

        async def db_check():
            return True

        await health_checker.register_check(
            "database",
            dependency_health_check("database", db_check),
            HealthCheckOptions(critical=True),
        )

        # Run all checks
        report = await health_checker.run_all()

        assert report.status in (HealthStatus.HEALTHY, HealthStatus.DEGRADED)
        assert len(report.checks) == 3

        # Check probes
        liveness = await health_checker.get_liveness()
        assert liveness["alive"] is True

        readiness = await health_checker.get_readiness()
        assert "ready" in readiness

        startup = await health_checker.get_startup()
        assert startup["started"] is True

        # Clean up
        await health_checker.shutdown()

    @pytest.mark.asyncio
    async def test_uptime_increases(self, health_checker):
        """Test that uptime increases over time."""
        report1 = await health_checker.run_all()
        uptime1 = report1.uptime

        # Wait for at least 1 second to ensure uptime increases
        await asyncio.sleep(1.1)

        report2 = await health_checker.run_all()
        uptime2 = report2.uptime

        assert uptime2 > uptime1
