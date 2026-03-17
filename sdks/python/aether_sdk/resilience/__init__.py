"""
Aether SDK Resilience Module

Provides patterns for building robust, fault-tolerant actor systems:
- Circuit Breaker: Prevents cascading failures
- Retry: Handles transient failures with backoff
- Rate Limiter: Controls request rates
- Health Check: Kubernetes-compatible probes
- Bulkhead: Resource isolation

Example usage:
    from aether_sdk.resilience import (
        CircuitBreaker,
        RetryPolicy,
        RateLimiter,
        Bulkhead,
        ResilientExecutor,
    )
    
    # Create resilience patterns
    breaker = CircuitBreaker()
    retry = RetryPolicy()
    limiter = RateLimiter()
    bulkhead = Bulkhead()
    
    # Combine with ResilientExecutor
    executor = ResilientExecutor(
        breaker=breaker,
        retry=retry,
        rate_limiter=limiter,
        bulkhead=bulkhead,
    )
    
    # Execute with all protections
    result = await executor.execute(my_async_function)
"""

from __future__ import annotations
from typing import Optional, Any, Callable

# Circuit Breaker
from .circuit_breaker import (
    CircuitBreaker,
    CircuitBreakerConfig,
    CircuitBreakerError,
    CircuitBreakerManager,
    CircuitBreakerStats,
    CircuitState,
)

# Retry
from .retry import (
    RetryPolicy,
    RetryConfig,
    RetryStats,
    RetryResult,
    RetryExhaustedError,
    BackoffStrategy,
    network_retry_policy,
    database_retry_policy,
    aggressive_retry_policy,
    conservative_retry_policy,
)

# Rate Limiter
from .rate_limiter import (
    RateLimiter,
    RateLimitConfig,
    RateLimitStats,
    RateLimitResult,
    RateLimitStrategy,
    RateLimitExhaustedError,
    RateLimiterManager,
    api_rate_limiter,
    strict_rate_limiter,
    bursty_rate_limiter,
)

# Health Check
from .health_check import (
    HealthChecker,
    HealthStatus,
    HealthCheckResult,
    HealthReport,
    HealthCheckOptions,
    HealthCheckFn,
    ping_health_check,
    memory_health_check,
    state_health_check,
    dependency_health_check,
)

# Bulkhead
from .bulkhead import (
    Bulkhead,
    BulkheadConfig,
    BulkheadStats,
    BulkheadRejectedError,
    BulkheadTimeoutError,
    BulkheadManager,
    api_bulkhead,
    database_bulkhead,
    strict_bulkhead,
)

# Tracing
from .tracing import (
    TRACING_AVAILABLE,
    TracingContext,
    get_tracer,
    traced_circuit_breaker,
    traced_retry,
    traced_rate_limiter,
    traced_bulkhead,
    create_resilience_span,
    record_resilience_event,
    set_resilience_attribute,
    ResilienceInstrumentation,
)


# ============================================
# Resilient Executor
# ============================================

class ResilientExecutor:
    """Combined resilience executor that applies all patterns.
    
    Order of operations:
    1. Rate limiting (check if request is allowed)
    2. Bulkhead (check capacity)
    3. Circuit breaker (check if service is healthy)
    4. Retry (handle transient failures)
    
    Example:
        executor = ResilientExecutor(
            breaker=CircuitBreaker(),
            retry=RetryPolicy(),
            rate_limiter=RateLimiter(),
            bulkhead=Bulkhead(),
        )
        
        result = await executor.execute(my_function)
    """
    
    def __init__(
        self,
        breaker: Optional[CircuitBreaker] = None,
        retry: Optional[RetryPolicy] = None,
        rate_limiter: Optional[RateLimiter] = None,
        bulkhead: Optional[Bulkhead] = None,
    ):
        self._breaker = breaker
        self._retry = retry
        self._rate_limiter = rate_limiter
        self._bulkhead = bulkhead
    
    async def execute(self, func: Callable[[], Any]) -> Any:
        """Execute with all configured resilience patterns.
        
        Args:
            func: Async function to execute
            
        Returns:
            Result of the function
            
        Raises:
            CircuitBreakerError: If circuit is open
            RetryExhaustedError: If all retries fail
            RateLimitExhaustedError: If rate limit exceeded
            BulkheadRejectedError: If bulkhead at capacity
        """
        # Apply rate limiting first
        if self._rate_limiter:
            await self._rate_limiter.acquire()
        
        # Apply bulkhead
        if self._bulkhead:
            return await self._bulkhead.execute(
                lambda: self._execute_with_retry(func)
            )
        
        return await self._execute_with_retry(func)
    
    async def _execute_with_retry(self, func: Callable[[], Any]) -> Any:
        """Execute with retry and circuit breaker."""
        # Apply circuit breaker
        if self._breaker:
            return await self._breaker.execute(
                lambda: self._execute_with_retry_internal(func)
            )
        
        return await self._execute_with_retry_internal(func)
    
    async def _execute_with_retry_internal(self, func: Callable[[], Any]) -> Any:
        """Execute with retry logic."""
        if self._retry:
            result = await self._retry.execute(func)
            return result.result
        
        return await func()


# ============================================
# Module Exports
# ============================================

__all__ = [
    # Circuit Breaker
    'CircuitBreaker',
    'CircuitBreakerConfig',
    'CircuitBreakerError',
    'CircuitBreakerManager',
    'CircuitBreakerStats',
    'CircuitState',
    
    # Retry
    'RetryPolicy',
    'RetryConfig',
    'RetryStats',
    'RetryResult',
    'RetryExhaustedError',
    'BackoffStrategy',
    'network_retry_policy',
    'database_retry_policy',
    'aggressive_retry_policy',
    'conservative_retry_policy',
    
    # Rate Limiter
    'RateLimiter',
    'RateLimitConfig',
    'RateLimitStats',
    'RateLimitResult',
    'RateLimitStrategy',
    'RateLimitExhaustedError',
    'RateLimiterManager',
    'api_rate_limiter',
    'strict_rate_limiter',
    'bursty_rate_limiter',
    
    # Health Check
    'HealthChecker',
    'HealthStatus',
    'HealthCheckResult',
    'HealthReport',
    'HealthCheckOptions',
    'HealthCheckFn',
    'ping_health_check',
    'memory_health_check',
    'state_health_check',
    'dependency_health_check',
    
    # Bulkhead
    'Bulkhead',
    'BulkheadConfig',
    'BulkheadStats',
    'BulkheadRejectedError',
    'BulkheadTimeoutError',
    'BulkheadManager',
    'api_bulkhead',
    'database_bulkhead',
    'strict_bulkhead',
    
    # Tracing
    'TRACING_AVAILABLE',
    'TracingContext',
    'get_tracer',
    'traced_circuit_breaker',
    'traced_retry',
    'traced_rate_limiter',
    'traced_bulkhead',
    'create_resilience_span',
    'record_resilience_event',
    'set_resilience_attribute',
    'ResilienceInstrumentation',
    
    # Combined
    'ResilientExecutor',
]
