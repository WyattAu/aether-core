"""Prometheus-compatible metrics for the Aether server.

Provides request counting, latency histograms, actor gauges, and
message throughput counters using only Python stdlib.

Metrics are exposed in Prometheus text exposition format at ``/metrics``.

Usage::

    from server.metrics import MetricsCollector

    metrics = MetricsCollector()

    # Record a request
    metrics.observe_request("GET", "/api/v1/actors", 200, duration=0.042)

    # Update gauge
    metrics.set_actor_count(42)

    # Get Prometheus exposition format
    text = metrics.collect()
"""

import threading
import time
from collections import defaultdict
from typing import Dict, List, Tuple


# Default histogram buckets (seconds): 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10
DEFAULT_BUCKETS = (0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0)


class MetricsCollector:
    """Collects and exposes Prometheus-compatible metrics.

    All methods are thread-safe.

    Metrics exposed:

    - ``aether_http_requests_total``: Counter by method, path, status.
      Labels: ``method``, ``path``, ``status``.
    - ``aether_http_request_duration_seconds``: Histogram with configurable buckets.
      Labels: ``method``, ``path``.
      Exposes ``_count``, ``_sum``, and ``_bucket`` (cumulative).
    - ``aether_active_actors``: Gauge of current registered actors.
    - ``aether_messages_total``: Counter of total messages routed.
    - ``aether_rate_limit_rejections_total``: Counter of rate-limited requests.

    Args:
        buckets: Histogram bucket boundaries in seconds.
    """

    def __init__(self, buckets: Tuple[float, ...] = DEFAULT_BUCKETS):
        self._buckets = buckets
        self._lock = threading.Lock()

        # Counters: {label_tuple: count}
        self._request_counts: Dict[Tuple[str, str, str], int] = defaultdict(int)
        self._message_count: int = 0
        self._rate_limit_count: int = 0

        # Gauge
        self._actor_count: int = 0

        # Histogram: {(method, path): {"count": int, "sum": float, "buckets": {boundary: count}}}
        self._histograms: Dict[Tuple[str, str], Dict] = {}

    def observe_request(
        self,
        method: str,
        path: str,
        status: int,
        duration: float,
    ) -> None:
        """Record an HTTP request observation.

        Args:
            method: HTTP method (GET, POST, etc.).
            path: Request path (e.g. ``/api/v1/actors``).
            status: HTTP status code (e.g. 200, 404, 429).
            duration: Request duration in seconds.
        """
        # Normalize path: strip trailing slashes, collapse numeric IDs
        # to avoid cardinality explosion
        normalized = self._normalize_path(path)

        with self._lock:
            # Increment request counter
            key = (method, normalized, str(status))
            self._request_counts[key] += 1

            # Update histogram
            hist_key = (method, normalized)
            if hist_key not in self._histograms:
                self._histograms[hist_key] = {
                    "count": 0,
                    "sum": 0.0,
                    "buckets": {b: 0 for b in self._buckets},
                }
            hist = self._histograms[hist_key]
            hist["count"] += 1
            hist["sum"] += duration
            for bucket_boundary in self._buckets:
                if duration <= bucket_boundary:
                    hist["buckets"][bucket_boundary] += 1

    def set_actor_count(self, count: int) -> None:
        """Set the active actor count gauge.

        Args:
            count: Current number of registered actors.
        """
        with self._lock:
            self._actor_count = count

    def inc_messages(self, count: int = 1) -> None:
        """Increment the total messages counter.

        Args:
            count: Number of messages to add (default 1).
        """
        with self._lock:
            self._message_count += count

    def inc_rate_limit_rejections(self, count: int = 1) -> None:
        """Increment the rate limit rejections counter.

        Args:
            count: Number of rejections to add (default 1).
        """
        with self._lock:
            self._rate_limit_count += count

    @staticmethod
    def _normalize_path(path: str) -> str:
        """Normalize a path to prevent metric cardinality explosion.

        Strips trailing slashes. Replaces UUIDs and numeric IDs with
        placeholders like ``:id``.
        """
        import re

        path = path.rstrip("/") or "/"

        # Replace UUIDs
        path = re.sub(
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            ":id", path, flags=re.IGNORECASE,
        )
        # Replace long numeric segments (likely IDs)
        path = re.sub(r"/(\d{6,})", r"/:id", path)
        # Replace hex segments that look like IDs
        path = re.sub(r"/([0-9a-f]{16,})", r"/:id", path, flags=re.IGNORECASE)

        return path

    def collect(self) -> str:
        """Generate Prometheus text exposition format output.

        Returns:
            A string in Prometheus text exposition format suitable
            for serving at ``/metrics``.
        """
        lines: List[str] = []

        with self._lock:
            lines.append("# HELP aether_http_requests_total Total HTTP requests")
            lines.append("# TYPE aether_http_requests_total counter")
            for (method, path, status), count in sorted(self._request_counts.items()):
                lines.append(
                    f'aether_http_requests_total{{method="{method}",path="{path}",status="{status}"}} {count}'
                )

            lines.append("")
            lines.append("# HELP aether_http_request_duration_seconds HTTP request duration in seconds")
            lines.append("# TYPE aether_http_request_duration_seconds histogram")
            for (method, path), hist in sorted(self._histograms.items()):
                # Buckets are already cumulative (each observation increments
                # all buckets where duration <= boundary)
                for bucket_boundary in self._buckets:
                    lines.append(
                        f'aether_http_request_duration_seconds{{method="{method}",path="{path}",le="{bucket_boundary}"}} {hist["buckets"][bucket_boundary]}'
                    )
                # +Inf bucket = total count
                lines.append(
                    f'aether_http_request_duration_seconds{{method="{method}",path="{path}",le="+Inf"}} {hist["count"]}'
                )
                lines.append(
                    f'aether_http_request_duration_seconds_sum{{method="{method}",path="{path}"}} {hist["sum"]:.6f}'
                )
                lines.append(
                    f'aether_http_request_duration_seconds_count{{method="{method}",path="{path}"}} {hist["count"]}'
                )

            lines.append("")
            lines.append("# HELP aether_active_actors Current number of registered actors")
            lines.append("# TYPE aether_active_actors gauge")
            lines.append(f"aether_active_actors {self._actor_count}")

            lines.append("")
            lines.append("# HELP aether_messages_total Total messages routed")
            lines.append("# TYPE aether_messages_total counter")
            lines.append(f"aether_messages_total {self._message_count}")

            lines.append("")
            lines.append("# HELP aether_rate_limit_rejections_total Total requests rejected by rate limiter")
            lines.append("# TYPE aether_rate_limit_rejections_total counter")
            lines.append(f"aether_rate_limit_rejections_total {self._rate_limit_count}")

        lines.append("")  # Trailing newline
        return "\n".join(lines)

    def reset(self) -> None:
        """Reset all metrics. Primarily useful for testing."""
        with self._lock:
            self._request_counts.clear()
            self._message_count = 0
            self._rate_limit_count = 0
            self._actor_count = 0
            self._histograms.clear()
