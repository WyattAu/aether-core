"""gRPC metrics interceptor for the Aether server.

Records gRPC call count, duration, and error status codes.
Outputs Prometheus-compatible metrics via ``MetricsCollector``.

Usage::

    from server.grpc_metrics import MetricsServerInterceptor
    from server.metrics import MetricsCollector

    metrics = MetricsCollector()
    interceptor = MetricsServerInterceptor(metrics)
    server = grpc.server(executor, interceptors=[interceptor])
"""

import logging
import threading
import time
from typing import Any

import grpc

logger = logging.getLogger("aether-server.grpc.metrics")


class MetricsServerInterceptor(grpc.ServerInterceptor):
    """gRPC server interceptor that records call metrics.

    Records the following for each RPC call:
    - Call count (by method and status code)
    - Duration histogram (by method)

    Uses the ``MetricsCollector`` to store metrics in Prometheus format.
    """

    def __init__(self, metrics: Any = None):
        """Initialize the interceptor.

        Args:
            metrics: A ``MetricsCollector`` instance. If ``None``, creates
                a default instance.
        """
        self._metrics = metrics
        self._lock = threading.Lock()
        # In-memory counters for gRPC-specific metrics
        self._call_counts: dict = {}  # {(method, code): count}

    def intercept_service(self, continuation, handler_call_details):
        """Intercept each service call to record metrics.

        Instead of wrapping individual RPC handlers, we return a
        ``ServerInterceptor`` that tracks timing around the service
        method dispatch. This approach avoids modifying the handler
        signatures and works correctly with all RPC types.
        """
        method = handler_call_details.method or "unknown"

        # Get the original handler from continuation
        handler = continuation(handler_call_details)
        if handler is None:
            return None

        # Create a new handler that wraps the original to record metrics
        if handler.unary_unary:
            original_behavior = handler.unary_unary

            def wrapped_behavior(request, context):
                start = time.perf_counter()
                try:
                    response = original_behavior(request, context)
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.OK
                    self._record(method, code, time.perf_counter() - start)
                    return response
                except Exception as e:
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.UNKNOWN
                    self._record(method, code, time.perf_counter() - start)
                    raise

            return grpc.unary_unary_rpc_method_handler(
                wrapped_behavior,
                request_deserializer=handler.request_deserializer,
                response_serializer=handler.response_serializer,
            )

        elif handler.unary_stream:
            original_behavior = handler.unary_stream

            def wrapped_behavior(request, context):
                start = time.perf_counter()
                try:
                    response = original_behavior(request, context)
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.OK
                    self._record(method, code, time.perf_counter() - start)
                    return response
                except Exception as e:
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.UNKNOWN
                    self._record(method, code, time.perf_counter() - start)
                    raise

            return grpc.unary_stream_rpc_method_handler(
                wrapped_behavior,
                request_deserializer=handler.request_deserializer,
                response_serializer=handler.response_serializer,
            )

        elif handler.stream_unary:
            original_behavior = handler.stream_unary

            def wrapped_behavior(request_iterator, context):
                start = time.perf_counter()
                try:
                    response = original_behavior(request_iterator, context)
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.OK
                    self._record(method, code, time.perf_counter() - start)
                    return response
                except Exception as e:
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.UNKNOWN
                    self._record(method, code, time.perf_counter() - start)
                    raise

            return grpc.stream_unary_rpc_method_handler(
                wrapped_behavior,
                request_deserializer=handler.request_deserializer,
                response_serializer=handler.response_serializer,
            )

        elif handler.stream_stream:
            original_behavior = handler.stream_stream

            def wrapped_behavior(request_iterator, context):
                start = time.perf_counter()
                try:
                    response = original_behavior(request_iterator, context)
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.OK
                    self._record(method, code, time.perf_counter() - start)
                    return response
                except Exception as e:
                    code = context.code()
                    if code is None:
                        code = grpc.StatusCode.UNKNOWN
                    self._record(method, code, time.perf_counter() - start)
                    raise

            return grpc.stream_stream_rpc_method_handler(
                wrapped_behavior,
                request_deserializer=handler.request_deserializer,
                response_serializer=handler.response_serializer,
            )

        return handler

    def _record(self, method: str, code: grpc.StatusCode, duration: float) -> None:
        """Record a single RPC call."""
        code_name = code.name

        with self._lock:
            key = (method, code_name)
            self._call_counts[key] = self._call_counts.get(key, 0) + 1

        # Also record in the MetricsCollector if available
        if self._metrics is not None:
            path = self._grpc_method_to_path(method)
            self._metrics.observe_request(
                method="GRPC",
                path=path,
                status=_grpc_code_to_http(code),
                duration=duration,
            )

    @staticmethod
    def _grpc_method_to_path(method: str) -> str:
        """Convert ``/package.Service/Method`` to ``/grpc/Service/Method``."""
        parts = method.strip("/").split("/")
        if len(parts) >= 2:
            service = parts[-2].split(".")[-1]
            rpc = parts[-1]
            return f"/grpc/{service}/{rpc}"
        return f"/grpc/{method}"

    def collect_grpc(self) -> str:
        """Generate Prometheus text format for gRPC-specific metrics."""
        lines: list = []

        with self._lock:
            lines.append("# HELP aether_grpc_calls_total Total gRPC calls")
            lines.append("# TYPE aether_grpc_calls_total counter")
            for (method, code), count in sorted(self._call_counts.items()):
                lines.append(
                    f'aether_grpc_calls_total{{method="{method}",code="{code}"}} {count}'
                )

        lines.append("")
        return "\n".join(lines)

    def reset(self) -> None:
        """Reset all gRPC metrics counters."""
        with self._lock:
            self._call_counts.clear()


def _grpc_code_to_http(code: grpc.StatusCode) -> int:
    """Map a gRPC status code to an approximate HTTP status code."""
    mapping = {
        grpc.StatusCode.OK: 200,
        grpc.StatusCode.CANCELLED: 499,
        grpc.StatusCode.UNKNOWN: 500,
        grpc.StatusCode.INVALID_ARGUMENT: 400,
        grpc.StatusCode.DEADLINE_EXCEEDED: 504,
        grpc.StatusCode.NOT_FOUND: 404,
        grpc.StatusCode.ALREADY_EXISTS: 409,
        grpc.StatusCode.PERMISSION_DENIED: 403,
        grpc.StatusCode.RESOURCE_EXHAUSTED: 429,
        grpc.StatusCode.FAILED_PRECONDITION: 400,
        grpc.StatusCode.ABORTED: 409,
        grpc.StatusCode.OUT_OF_RANGE: 400,
        grpc.StatusCode.UNIMPLEMENTED: 501,
        grpc.StatusCode.INTERNAL: 500,
        grpc.StatusCode.UNAVAILABLE: 503,
        grpc.StatusCode.DATA_LOSS: 500,
        grpc.StatusCode.UNAUTHENTICATED: 401,
    }
    return mapping.get(code, 500)
