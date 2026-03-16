"""
Health Check Implementation

Provides standardized health check functionality for actors
compatible with Kubernetes liveness/readiness probes.
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, Callable, Awaitable, Union
from enum import Enum
import asyncio
import time
import resource
import platform


class HealthStatus(Enum):
    HEALTHY = "healthy"
    DEGRADED = "degraded"
    UNHEALTHY = "unhealthy"


@dataclass
class HealthCheckResult:
    """Result of a single health check."""
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
    """Overall health report."""
    status: HealthStatus
    version: str
    service_id: str
    time: str
    checks: Dict[str, HealthCheckResult]
    uptime: int


@dataclass
class HealthCheckOptions:
    """Options for a health check."""
    timeout_ms: int = 5000
    critical: bool = False  # Failure = unhealthy
    interval_ms: int = 0  # 0 = on demand only
    cache_duration_ms: int = 0  # 0 = no caching


# Type alias for health check function
HealthCheckFn = Callable[[], Union[HealthCheckResult, Awaitable[HealthCheckResult]]]


# ============================================
# Health Checker Implementation
# ============================================

class HealthChecker:
    """Health checker for actors with Kubernetes-compatible probes."""
    
    def __init__(
        self, 
        service_id: str = "aether-actor", 
        version: str = "1.0.0"
    ):
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
        """Register a health check."""
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
        
        # Set up interval if specified
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
        """Unregister a health check."""
        async with self._lock:
            entry = self._checks.pop(name, None)
            if entry and entry.get("task"):
                entry["task"].cancel()
    
    async def run_check(self, name: str) -> HealthCheckResult:
        """Run a single health check."""
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
        
        # Return cached result if still valid
        if options.cache_duration_ms > 0 and entry["last_result"] and entry["last_run"]:
            if time.time() - entry["last_run"] < options.cache_duration_ms / 1000:
                return entry["last_result"]
        
        # Run check with timeout
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
        """Run all health checks and generate report."""
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
        """Get liveness status (is the service alive?)."""
        return {
            "alive": True,
            "time": self._get_timestamp(),
        }
    
    async def get_readiness(self) -> Dict[str, Any]:
        """Get readiness status (is the service ready to accept traffic?)."""
        report = await self.run_all()
        
        checks: Dict[str, bool] = {}
        for name, result in report.checks.items():
            entry = self._checks.get(name)
            # Non-critical checks don't affect readiness
            if entry and entry["options"].critical:
                checks[name] = result.status != HealthStatus.UNHEALTHY
        
        ready = report.status != HealthStatus.UNHEALTHY
        
        return {
            "ready": ready,
            "time": report.time,
            "checks": checks if checks else None,
        }
    
    async def get_startup(self) -> Dict[str, Any]:
        """Get startup status (has the service started?)."""
        return {
            "started": True,
            "time": self._get_timestamp(),
        }
    
    async def shutdown(self) -> None:
        """Clean up all interval-based checks."""
        async with self._lock:
            for entry in self._checks.values():
                if entry.get("task"):
                    entry["task"].cancel()
            self._checks.clear()
    
    # ============================================
    # Private Methods
    # ============================================
    
    def _calculate_overall_status(
        self, 
        checks: Dict[str, HealthCheckResult]
    ) -> HealthStatus:
        """Calculate overall status from check results."""
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
        """Run a check with timeout."""
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
        """Get current ISO timestamp."""
        return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


# ============================================
# Predefined Health Checks
# ============================================

def ping_health_check() -> HealthCheckFn:
    """Create a simple ping health check."""
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
    """Create a memory health check."""
    def check() -> HealthCheckResult:
        # Get memory info using resource module
        rusage = resource.getrusage(resource.RUSAGE_SELF)
        # On Linux, ru_maxrss is in KB; on macOS, it's in bytes
        if platform.system() == "Darwin":
            rss_mb = rusage.ru_maxrss / (1024 * 1024)
        else:
            rss_mb = rusage.ru_maxrss / 1024
        
        # Estimate heap as fraction of RSS (rough approximation)
        heap_used_mb = rss_mb * 0.7  # Assume 70% of RSS is heap
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
    """Create a state storage health check."""
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
    """Create an async dependency health check."""
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
