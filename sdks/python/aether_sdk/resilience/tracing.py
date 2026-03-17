"""
OpenTelemetry Tracing Integration for Resilience Patterns

Provides tracing spans for all resilience patterns to integrate with OpenTelemetry.
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, Callable
from contextlib import contextmanager
import functools
import time

# Check if OpenTelemetry is available
TRACING_AVAILABLE = False
_tracer_module = None

try:
    from opentelemetry import trace as _tracer_module
    from opentelemetry.trace import SpanKind, Status, StatusCode
    TRACING_AVAILABLE = True
except ImportError:
    pass


class TracingContext:
    """Context manager for tracing spans."""
    
    def __init__(
        self,
        tracer: Optional[Any],
        span_name: str,
        attributes: Optional[Dict[str, Any]] = None,
    ):
        self.tracer = tracer
        self.span_name = span_name
        self.attributes = attributes or {}
        self.span = None
        self.start_time: Optional[float] = None
    
    def __enter__(self):
        if not TRACING_AVAILABLE or not self.tracer:
            return self
        
        # Start span
        self.span = self.tracer.start_span(
            self.span_name,
            kind=SpanKind.INTERNAL if TRACING_AVAILABLE else None,
            attributes=self.attributes,
        )
        self.start_time = time.time()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        if not self.span:
            return
        
        if TRACING_AVAILABLE:
            if exc_type:
                self.span.set_status(Status(StatusCode.ERROR))
                self.span.set_attribute("error.type", exc_type.__name__)
                self.span.set_attribute("error.message", str(exc_val))
            else:
                self.span.set_status(Status(StatusCode.OK))
            
            if self.start_time:
                self.span.set_attribute(
                    "duration_ms",
                    int((time.time() - self.start_time) * 1000)
                )
            self.span.end()
    
    def set_attribute(self, key: str, value: Any) -> None:
        if self.span:
            self.span.set_attribute(key, value)
    
    def add_event(self, name: str, attributes: Optional[Dict[str, Any]] = None) -> None:
        if self.span:
            self.span.add_event(name, attributes=attributes or {})


def get_tracer(service_name: str = "aether-resilience") -> Optional[Any]:
    """Get or create a tracer for resilience patterns."""
    if not TRACING_AVAILABLE or not _tracer_module:
        return None
    
    try:
        tracer_provider = _tracer_module.get_tracer_provider()
        return tracer_provider.get_tracer(service_name)
    except Exception:
        return None


# ============================================
# Traced Decorators
# ============================================

def traced_circuit_breaker(name: str = "default"):
    """Decorator to add tracing to circuit breaker operations."""
    def decorator(func):
        @functools.wraps(func)
        async def wrapper(self, *args, **kwargs):
            if not TRACING_AVAILABLE:
                return await func(self, *args, **kwargs)
            
            tracer = get_tracer()
            if not tracer:
                return await func(self, *args, **kwargs)
            
            state_val = self.state.value if hasattr(self, 'state') else "unknown"
            with TracingContext(
                tracer,
                f"circuit_breaker.{name}.{func.__name__}",
                {
                    "circuit_breaker.name": name,
                    "circuit_breaker.state": state_val,
                },
            ) as ctx:
                try:
                    result = await func(self, *args, **kwargs)
                    ctx.set_attribute("circuit_breaker.result", "success")
                    return result
                except Exception as e:
                    ctx.set_attribute("circuit_breaker.result", "rejected")
                    raise
        
        return wrapper
    return decorator


def traced_retry(name: str = "default"):
    """Decorator to add tracing to retry operations."""
    def decorator(func):
        @functools.wraps(func)
        async def wrapper(self, *args, **kwargs):
            if not TRACING_AVAILABLE:
                return await func(self, *args, **kwargs)
            
            tracer = get_tracer()
            if not tracer:
                return await func(self, *args, **kwargs)
            
            max_attempts = self._config.max_attempts if hasattr(self, '_config') else 0
            with TracingContext(
                tracer,
                f"retry.{name}.{func.__name__}",
                {
                    "retry.name": name,
                    "retry.max_attempts": max_attempts,
                },
            ) as ctx:
                try:
                    result = await func(self, *args, **kwargs)
                    ctx.set_attribute("retry.result", "success")
                    return result
                except Exception as e:
                    ctx.set_attribute("retry.result", "exhausted")
                    raise
        
        return wrapper
    return decorator


def traced_rate_limiter(name: str = "default"):
    """Decorator to add tracing to rate limiter operations."""
    def decorator(func):
        @functools.wraps(func)
        async def wrapper(self, *args, **kwargs):
            if not TRACING_AVAILABLE:
                return await func(self, *args, **kwargs)
            
            tracer = get_tracer()
            if not tracer:
                return await func(self, *args, **kwargs)
            
            with TracingContext(
                tracer,
                f"rate_limiter.{name}.{func.__name__}",
                {
                    "rate_limiter.name": name,
                },
            ) as ctx:
                try:
                    result = await func(self, *args, **kwargs)
                    if hasattr(result, 'allowed'):
                        ctx.set_attribute("rate_limiter.allowed", result.allowed)
                    return result
                except Exception as e:
                    ctx.set_attribute("rate_limiter.allowed", False)
                    raise
        
        return wrapper
    return decorator


def traced_bulkhead(name: str = "default"):
    """Decorator to add tracing to bulkhead operations."""
    def decorator(func):
        @functools.wraps(func)
        async def wrapper(self, *args, **kwargs):
            if not TRACING_AVAILABLE:
                return await func(self, *args, **kwargs)
            
            tracer = get_tracer()
            if not tracer:
                return await func(self, *args, **kwargs)
            
            with TracingContext(
                tracer,
                f"bulkhead.{name}.{func.__name__}",
                {
                    "bulkhead.name": name,
                },
            ) as ctx:
                try:
                    result = await func(self, *args, **kwargs)
                    ctx.set_attribute("bulkhead.result", "success")
                    return result
                except Exception as e:
                    ctx.set_attribute("bulkhead.result", "rejected")
                    raise
        
        return wrapper
    return decorator


# ============================================
# Helper Functions
# ============================================

def create_resilience_span(
    operation: str,
    pattern_type: str,
    pattern_name: str,
    attributes: Optional[Dict[str, Any]] = None,
) -> Optional[TracingContext]:
    """Create a tracing span for resilience operations."""
    if not TRACING_AVAILABLE:
        return None
    
    tracer = get_tracer()
    if not tracer:
        return None
    
    return TracingContext(
        tracer,
        f"{pattern_type}.{pattern_name}.{operation}",
        attributes or {},
    )


def record_resilience_event(
    pattern_type: str,
    event_name: str,
    attributes: Optional[Dict[str, Any]] = None,
) -> None:
    """Record a resilience event in the current span."""
    if not TRACING_AVAILABLE or not _tracer_module:
        return
    
    try:
        span = _tracer_module.get_current_span()
        if span:
            span.add_event(
                f"{pattern_type}.{event_name}",
                attributes=attributes or {},
            )
    except Exception:
        pass


def set_resilience_attribute(key: str, value: Any) -> None:
    """Set an attribute on the current span."""
    if not TRACING_AVAILABLE or not _tracer_module:
        return
    
    try:
        span = _tracer_module.get_current_span()
        if span:
            span.set_attribute(key, value)
    except Exception:
        pass


# ============================================
# Instrumentation Class
# ============================================

class ResilienceInstrumentation:
    """Provides instrumentation utilities for resilience patterns."""
    
    def __init__(self, service_name: str = "aether-resilience"):
        self.tracer = get_tracer(service_name)
    
    @contextmanager
    def trace_circuit_breaker(
        self,
        name: str,
        state: str,
        operation: str,
    ):
        """Create a circuit breaker trace context."""
        with TracingContext(
            self.tracer,
            f"circuit_breaker.{name}.{operation}",
            {
                "circuit_breaker.name": name,
                "circuit_breaker.state": state,
            },
        ) as ctx:
            yield ctx
    
    @contextmanager
    def trace_retry(
        self,
        name: str,
        attempt: int,
        max_attempts: int,
        operation: str,
    ):
        """Create a retry trace context."""
        with TracingContext(
            self.tracer,
            f"retry.{name}.{operation}",
            {
                "retry.name": name,
                "retry.attempt": attempt,
                "retry.max_attempts": max_attempts,
            },
        ) as ctx:
            yield ctx
    
    @contextmanager
    def trace_rate_limiter(
        self,
        name: str,
        operation: str,
        requests_per_second: int = 0,
    ):
        """Create a rate limiter trace context."""
        with TracingContext(
            self.tracer,
            f"rate_limiter.{name}.{operation}",
            {
                "rate_limiter.name": name,
                "rate_limiter.requests_per_second": requests_per_second,
            },
        ) as ctx:
            yield ctx
    
    @contextmanager
    def trace_bulkhead(
        self,
        name: str,
        operation: str,
        active: int = 0,
        max_concurrent: int = 0,
    ):
        """Create a bulkhead trace context."""
        with TracingContext(
            self.tracer,
            f"bulkhead.{name}.{operation}",
            {
                "bulkhead.name": name,
                "bulkhead.active": active,
                "bulkhead.max_concurrent": max_concurrent,
            },
        ) as ctx:
            yield ctx
    
    @contextmanager
    def trace_health_check(
        self,
        name: str,
        check_name: str,
    ):
        """Create a health check trace context."""
        with TracingContext(
            self.tracer,
            f"health_check.{name}.{check_name}",
            {
                "health_check.name": name,
                "health_check.check": check_name,
            },
        ) as ctx:
            yield ctx


# ============================================
# Exports
# ============================================

__all__ = [
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
]
