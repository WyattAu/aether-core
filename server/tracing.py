import logging
import functools
from contextlib import contextmanager
from typing import Optional, Any, Dict

logger = logging.getLogger("aether-server")

TRACING_AVAILABLE = False
_trace_module = None

try:
    from opentelemetry import trace as _trace_module
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor, ConsoleSpanExporter
    from opentelemetry.sdk.resources import Resource
    from opentelemetry.trace import SpanKind, Status, StatusCode
    TRACING_AVAILABLE = True
except ImportError:
    pass


def setup_tracing(service_name: str = "aether-server", service_version: str = "0.1.0") -> bool:
    if not TRACING_AVAILABLE or not _trace_module:
        logger.info("OpenTelemetry not available, tracing disabled")
        return False

    try:
        resource = Resource.create({
            "service.name": service_name,
            "service.version": service_version,
        })
        provider = TracerProvider(resource=resource)
        processor = BatchSpanProcessor(ConsoleSpanExporter())
        provider.add_span_processor(processor)
        _trace_module.set_tracer_provider(provider)
        logger.info("OpenTelemetry tracing configured for %s", service_name)
        return True
    except Exception as e:
        logger.warning("Failed to setup OpenTelemetry tracing: %s", e)
        return False


def get_tracer(service_name: str = "aether-server"):
    if not TRACING_AVAILABLE or not _trace_module:
        return None
    try:
        return _trace_module.get_tracer(service_name)
    except Exception:
        return None


@contextmanager
def trace_span(name: str, attributes: Optional[Dict[str, Any]] = None, kind=None):
    if not TRACING_AVAILABLE or not _trace_module:
        yield None
        return

    tracer = get_tracer()
    if not tracer:
        yield None
        return

    span_kind = kind or SpanKind.INTERNAL
    with tracer.start_as_current_span(name, kind=span_kind, attributes=attributes) as span:
        try:
            yield span
        except Exception as exc:
            if span:
                span.set_status(Status(StatusCode.ERROR, str(exc)))
                span.set_attribute("error.type", type(exc).__name__)
                span.set_attribute("error.message", str(exc))
            raise


def traced(operation_name: str = None):
    def decorator(func):
        @functools.wraps(func)
        async def async_wrapper(*args, **kwargs):
            name = operation_name or f"{func.__module__}.{func.__name__}"
            with trace_span(name):
                return await func(*args, **kwargs)

        @functools.wraps(func)
        def sync_wrapper(*args, **kwargs):
            name = operation_name or f"{func.__module__}.{func.__name__}"
            with trace_span(name):
                return func(*args, **kwargs)

        if functools.iscoroutinefunction(func):
            return async_wrapper
        return sync_wrapper

    return decorator


def get_trace_id_hex() -> Optional[str]:
    if not TRACING_AVAILABLE or not _trace_module:
        return None
    try:
        span = _trace_module.get_current_span()
        ctx = span.get_span_context()
        if ctx and ctx.trace_id != 0:
            return format(ctx.trace_id, "032x")
    except Exception:
        pass
    return None
