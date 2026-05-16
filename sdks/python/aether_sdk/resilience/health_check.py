"""
Health Check Implementation

Provides standardized health check functionality for actors
compatible with Kubernetes liveness/readiness probes.

Example:
    >>> from aether_sdk.resilience.health_check import HealthChecker
    >>> checker = HealthChecker(service_id="my-service")
    >>> await checker.register_check("ping", ping_health_check())
    >>> report = await checker.run_all()
    >>> liveness = await checker.get_liveness()
"""

from __future__ import annotations

import asyncio
import platform
import resource
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Awaitable, Callable, Dict, Optional, Union


class HealthStatus(Enum):
    """Possible health statuses.

    Attributes:
        HEALTHY: All checks passing.
        DEGRADED: Non-critical checks failing.
        UNHEALTHY: Critical checks failing or service is down.
    """

    HEALTHY = "healthy"
    DEGRADED = "degraded"
    UNHEALTHY = "unhealthy"


@dataclass
class HealthCheckResult:
    """Result of a single health check.

    Attributes:
        status: The health status reported by this check.
        component_id: Identifier for the component being checked.
        component_type: Type of component (e.g. ``"self"``,
            ``"system"``, ``"storage"``, ``"dependency"``).
        observed_value: Optional numeric observation (e.g. latency).
        observed_unit: Unit for *observed_value* (e.g. ``"ms"``).
        output: Human-readable description of the check result.
        time: ISO 8601 timestamp of when the check ran.
        details: Arbitrary key-value details about the check.
    """

    status: HealthStatus
    component_id: str
    component_type: str
    observed_value: Optional[Any] = None
    observed_unit: Optional[str] = None
    output: Optional[str] = None
    time: str = ""
    details: Dict[str, Any] = field(default_factory=dict)


@dataclass
class HealthReport:
    """Overall health report aggregating all checks.

    Attributes:
        status: Aggregated health status.
        version: Service version string.
        service_id: Service identifier.
        time: ISO 8601 timestamp of the report.
        checks: Map of check names to their results.
        uptime: Service uptime in seconds.
    """

    status: HealthStatus
    version: str
    service_id: str
    time: str
    checks: Dict[str, HealthCheckResult]
    uptime: int


@dataclass
class HealthCheckOptions:
    """Options for an individual health check.

    Attributes:
        timeout_ms: Maximum time (ms) the check may take.
        critical: If ``True``, a failing check sets the overall
            status to UNHEALTHY. Non-critical failures result in
            DEGRADED.
        interval_ms: Run the check periodically at this interval.
            Set to ``0`` for on-demand only.
        cache_duration_ms: Cache the result for this many ms.
            Set to ``0`` to disable caching.
    """

    timeout_ms: int = 5000
    critical: bool = False
    interval_ms: int = 0
    cache_duration_ms: int = 0


HealthCheckFn = Callable[[], Union[HealthCheckResult, Awaitable[HealthCheckResult]]]


# ============================================
# Health Checker Implementation
# ============================================


class HealthChecker:
    """Health checker with Kubernetes-compatible probes.

    Manages named health checks, runs them on demand or at intervals,
    and aggregates results into :class:`HealthReport` objects.

    Example:
        >>> checker = HealthChecker("my-service", version="2.1.0")
        >>> await checker.register_check("db", db_health_check())
        >>> report = await checker.run_all()
    """

    def __init__(self, service_id: str = "aether-actor", version: str = "1.0.0"):
        """Initialize the health checker.

        Args:
            service_id: Identifier included in health reports.
            version: Service version string included in reports.
        """
        self._service_id = service_id
        self._version = version
        self._start_time = time.time()

        self._checks: Dict[str, dict] = {}
        self._lock = asyncio.Lock()

    async def register_check(
        self,
        name: str,
        fn: HealthCheckFn,
        options: Optional[HealthCheckOptions] = None,
    ) -> None:
        """Register a named health check.

        Args:
            name: Unique name for the check.
            fn: A sync or async callable returning a
                :class:`HealthCheckResult`.
            options: Optional configuration for the check.
        """
        resolved_options = options or HealthCheckOptions()

        entry = {
            "fn": fn,
            "options": resolved_options,
            "last_result": None,
            "last_run": None,
            "task": None,
        }

        async with self._lock:
            self._checks[name] = entry

        if resolved_options.interval_ms > 0:

            async def run_periodically():
                while True:
                    try:
                        result = await self._run_check_with_timeout(
                            name, fn, resolved_options.timeout_ms
                        )
                        entry["last_result"] = result
                        entry["last_run"] = time.time()
                    except Exception as e:
                        entry["last_result"] = HealthCheckResult(
                            status=HealthStatus.UNHEALTHY,
                            component_id=name,
                            component_type="check",
                            output=str(e),
                            time=self._get_timestamp(),
                        )
                        entry["last_run"] = time.time()
                    await asyncio.sleep(resolved_options.interval_ms / 1000)

            entry["task"] = asyncio.create_task(run_periodically())

    async def unregister_check(self, name: str) -> None:
        """Unregister a health check and cancel any periodic task.

        Args:
            name: The name of the check to remove.
        """
        async with self._lock:
            entry = self._checks.pop(name, None)
            if entry and entry.get("task"):
                entry["task"].cancel()

    async def run_check(self, name: str) -> HealthCheckResult:
        """Run a single health check by name.

        If caching is configured and the cached result is still valid,
        the cached result is returned without re-running the check.

        Args:
            name: The name of the check to run.

        Returns:
            A :class:`HealthCheckResult`. Returns UNHEALTHY with
            output ``"Check not found"`` if the name is not registered.
        """
        entry = self._checks.get(name)
        if not entry:
            return HealthCheckResult(
                status=HealthStatus.UNHEALTHY,
                component_id=name,
                component_type="check",
                output="Check not found",
                time=self._get_timestamp(),
            )

        options: HealthCheckOptions = entry["options"]

        if options.cache_duration_ms > 0 and entry["last_result"] and entry["last_run"]:
            if time.time() - entry["last_run"] < options.cache_duration_ms / 1000:
                return entry["last_result"]

        try:
            result = await self._run_check_with_timeout(
                name, entry["fn"], options.timeout_ms
            )
            entry["last_result"] = result
            entry["last_run"] = time.time()
            return result
        except Exception as e:
            result = HealthCheckResult(
                status=HealthStatus.UNHEALTHY,
                component_id=name,
                component_type="check",
                output=str(e),
                time=self._get_timestamp(),
            )
            entry["last_result"] = result
            entry["last_run"] = time.time()
            return result

    async def run_all(self) -> HealthReport:
        """Run all registered health checks and produce an aggregate report.

        Returns:
            A :class:`HealthReport` with overall status, per-check
            results, and service uptime.
        """
        check_results: Dict[str, HealthCheckResult] = {}

        for name in list(self._checks.keys()):
            check_results[name] = await self.run_check(name)

        status = self._calculate_overall_status(check_results)

        return HealthReport(
            status=status,
            version=self._version,
            service_id=self._service_id,
            time=self._get_timestamp(),
            checks=check_results,
            uptime=int(time.time() - self._start_time),
        )

    async def get_liveness(self) -> Dict[str, Any]:
        """Return a liveness probe response.

        Liveness indicates whether the process is alive and not deadlocked.

        Returns:
            A dict with ``"alive"`` (bool) and ``"time"`` (ISO 8601).
        """
        return {
            "alive": True,
            "time": self._get_timestamp(),
        }

    async def get_readiness(self) -> Dict[str, Any]:
        """Return a readiness probe response.

        Readiness indicates whether the service can accept traffic.
        Only **critical** checks affect readiness.

        Returns:
            A dict with ``"ready"`` (bool), ``"time"``, and optional
            ``"checks"`` dict mapping critical check names to booleans.
        """
        report = await self.run_all()

        checks: Dict[str, bool] = {}
        for name, result in report.checks.items():
            entry = self._checks.get(name)
            if entry and entry["options"].critical:
                checks[name] = result.status != HealthStatus.UNHEALTHY

        ready = report.status != HealthStatus.UNHEALTHY

        return {
            "ready": ready,
            "time": report.time,
            "checks": checks if checks else None,
        }

    async def get_startup(self) -> Dict[str, Any]:
        """Return a startup probe response.

        Startup indicates whether the service has completed its
        initialization.

        Returns:
            A dict with ``"started"`` (bool) and ``"time"`` (ISO 8601).
        """
        return {
            "started": True,
            "time": self._get_timestamp(),
        }

    async def shutdown(self) -> None:
        """Cancel all periodic health check tasks and clear registrations."""
        async with self._lock:
            for entry in self._checks.values():
                if entry.get("task"):
                    entry["task"].cancel()
            self._checks.clear()

    def _calculate_overall_status(
        self, checks: Dict[str, HealthCheckResult]
    ) -> HealthStatus:
        """Determine the overall status from individual check results.

        Critical UNHEALTHY checks immediately yield UNHEALTHY.
        Otherwise, any UNHEALTHY or DEGRADED check yields DEGRADED.

        Args:
            checks: Map of check names to results.

        Returns:
            The aggregated :class:`HealthStatus`.
        """
        has_degraded = False
        has_unhealthy = False

        for name, result in checks.items():
            entry = self._checks.get(name)

            if result.status == HealthStatus.UNHEALTHY:
                if entry and entry["options"].critical:
                    return HealthStatus.UNHEALTHY
                has_unhealthy = True
            elif result.status == HealthStatus.DEGRADED:
                has_degraded = True

        if has_unhealthy or has_degraded:
            return HealthStatus.DEGRADED

        return HealthStatus.HEALTHY

    async def _run_check_with_timeout(
        self,
        name: str,
        fn: HealthCheckFn,
        timeout_ms: int,
    ) -> HealthCheckResult:
        """Run a health check function with a timeout.

        Args:
            name: Check name (used in error messages).
            fn: Sync or async callable.
            timeout_ms: Maximum execution time.

        Returns:
            The :class:`HealthCheckResult` from the check function.

        Raises:
            Exception: If the check times out or raises.
        """
        try:
            result = fn()
            if asyncio.iscoroutine(result):
                result = await asyncio.wait_for(
                    result,
                    timeout=timeout_ms / 1000,
                )
            return result
        except asyncio.TimeoutError:
            raise Exception(f"Health check timed out after {timeout_ms}ms")

    def _get_timestamp(self) -> str:
        """Return the current UTC time as an ISO 8601 string.

        Returns:
            A string like ``"2025-01-15T12:00:00Z"``.
        """
        return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


# ============================================
# Predefined Health Checks
# ============================================


def ping_health_check() -> HealthCheckFn:
    """Create a simple liveness ping health check.

    Always returns :data:`HealthStatus.HEALTHY`.

    Returns:
        A :data:`HealthCheckFn` callable.
    """

    def check() -> HealthCheckResult:
        return HealthCheckResult(
            status=HealthStatus.HEALTHY,
            component_id="ping",
            component_type="self",
            observed_value=1,
            observed_unit="ms",
            time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        )

    return check


def memory_health_check(
    max_heap_mb: int = 1024,
    warn_threshold: float = 0.8,
) -> HealthCheckFn:
    """Create a health check that monitors process memory usage.

    Uses ``resource.getrusage`` to estimate RSS and reports DEGRADED
    when usage exceeds *warn_threshold* and UNHEALTHY above 95 %.

    Args:
        max_heap_mb: Maximum allowed heap size in MB.
        warn_threshold: Fraction (0–1) at which to report DEGRADED.

    Returns:
        A :data:`HealthCheckFn` callable.
    """

    def check() -> HealthCheckResult:
        rusage = resource.getrusage(resource.RUSAGE_SELF)
        if platform.system() == "Darwin":
            rss_mb = rusage.ru_maxrss / (1024 * 1024)
        else:
            rss_mb = rusage.ru_maxrss / 1024

        heap_used_mb = rss_mb * 0.7
        heap_total_mb = max_heap_mb
        usage = min(heap_used_mb / heap_total_mb, 1.0)

        if heap_used_mb > max_heap_mb or usage > 0.95:
            status = HealthStatus.UNHEALTHY
        elif usage > warn_threshold:
            status = HealthStatus.DEGRADED
        else:
            status = HealthStatus.HEALTHY

        return HealthCheckResult(
            status=status,
            component_id="memory",
            component_type="system",
            observed_value=int(heap_used_mb),
            observed_unit="MB",
            output=f"Heap usage: {int(heap_used_mb)}MB / {int(heap_total_mb)}MB ({int(usage * 100)}%)",
            time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            details={
                "heap_used_mb": heap_used_mb,
                "heap_total_mb": heap_total_mb,
                "rss_mb": rss_mb,
            },
        )

    return check


def state_health_check(
    state_key: str,
    read_fn: Callable[[str], Awaitable[bool]],
) -> HealthCheckFn:
    """Create a health check that verifies state storage accessibility.

    Args:
        state_key: Key to read in order to verify connectivity.
        read_fn: Async callable ``(key) -> bool`` that reads from
            the state store.

    Returns:
        A :data:`HealthCheckFn` callable. Reports DEGRADED if the
        read takes longer than 1 second.
    """

    async def check() -> HealthCheckResult:
        start = time.time()
        try:
            exists = await read_fn(state_key)
            latency_ms = int((time.time() - start) * 1000)

            if latency_ms > 1000:
                status = HealthStatus.DEGRADED
            else:
                status = HealthStatus.HEALTHY

            return HealthCheckResult(
                status=status,
                component_id="state-storage",
                component_type="storage",
                observed_value=latency_ms,
                observed_unit="ms",
                output=f"State storage {'accessible' if exists else 'empty'}",
                time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            )
        except Exception as e:
            return HealthCheckResult(
                status=HealthStatus.UNHEALTHY,
                component_id="state-storage",
                component_type="storage",
                output=str(e),
                time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            )

    return check


def dependency_health_check(
    name: str,
    check_fn: Callable[[], Awaitable[bool]],
    timeout_ms: int = 5000,
) -> HealthCheckFn:
    """Create an async health check for an external dependency.

    Args:
        name: Human-readable dependency name (used in results).
        check_fn: Async callable ``() -> bool`` that returns ``True``
            if the dependency is healthy.
        timeout_ms: Maximum time to wait for *check_fn*.

    Returns:
        A :data:`HealthCheckFn` callable.
    """

    async def check() -> HealthCheckResult:
        start = time.time()
        try:
            result = await asyncio.wait_for(
                check_fn(),
                timeout=timeout_ms / 1000,
            )

            latency_ms = int((time.time() - start) * 1000)
            status = HealthStatus.HEALTHY if result else HealthStatus.UNHEALTHY

            return HealthCheckResult(
                status=status,
                component_id=name,
                component_type="dependency",
                observed_value=latency_ms,
                observed_unit="ms",
                time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            )
        except asyncio.TimeoutError:
            return HealthCheckResult(
                status=HealthStatus.UNHEALTHY,
                component_id=name,
                component_type="dependency",
                output=f"Dependency check timed out after {timeout_ms}ms",
                time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            )
        except Exception as e:
            return HealthCheckResult(
                status=HealthStatus.UNHEALTHY,
                component_id=name,
                component_type="dependency",
                output=str(e),
                time=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            )

    return check
