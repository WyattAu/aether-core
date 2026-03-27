"""Rate limiting middleware for the Aether server.

Provides token-bucket rate limiting with per-IP and global limits.
Configurable per-endpoint limits with sliding window counters.

Usage::

    from server.rate_limit import RateLimitMiddleware, RateLimitConfig

    config = RateLimitConfig(
        enabled=True,
        requests_per_second=100,
        burst=200,
    )
    app.add_middleware(RateLimitMiddleware, config=config)
"""

import logging
import threading
import time
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Dict, Optional, Tuple

from fastapi import Request, Response
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import JSONResponse

logger = logging.getLogger("aether-server.rate_limit")


@dataclass
class RateLimitConfig:
    """Configuration for rate limiting.

    Attributes:
        enabled: Whether rate limiting is active.
        requests_per_second: Sustained request rate (tokens added per second).
        burst: Maximum burst size (bucket capacity).
        key_func: Function to extract the rate limit key from a request.
            Default is per-IP rate limiting.
        per_endpoint: Whether to track limits separately per endpoint path.
        default_limit: Default requests/second if no per-endpoint override.
        default_burst: Default burst if no per-endpoint override.
        endpoint_limits: Override limits per endpoint path pattern.
            Keys are path prefixes (e.g. ``"/api/v1/actors"``), values are
            ``(requests_per_second, burst)`` tuples.
        public_paths: Paths that bypass rate limiting entirely.
    """
    enabled: bool = False
    requests_per_second: float = 100.0
    burst: int = 200
    per_endpoint: bool = True
    default_limit: float = 100.0
    default_burst: int = 200
    endpoint_limits: Dict[str, Tuple[float, int]] = field(default_factory=dict)
    public_paths: set = field(default_factory=lambda: {
        "/health",
        "/health/ready",
        "/api/v1/info",
        "/docs",
        "/openapi.json",
        "/redoc",
        "/metrics",
    })


class TokenBucket:
    """Thread-safe token bucket rate limiter.

    Tokens are added at a fixed rate and consumed on each request.
    When the bucket is empty, requests are rejected.

    Uses integer token tracking to avoid floating-point drift.
    Accumulated time is saved between calls, and whole tokens are
    only added when enough time has elapsed (``1/rate`` seconds
    per token).

    Uses a threading lock for thread safety, making it compatible
    with both sync and async contexts.

    Args:
        rate: Tokens added per second.
        capacity: Maximum bucket size (burst).
    """

    def __init__(self, rate: float, capacity: int):
        self.rate = rate
        self.capacity = capacity
        self._tokens: int = capacity
        self._accumulated: float = 0.0  # accumulated fractional time
        self._last_refill: float = time.monotonic()
        self._lock = threading.Lock()

    def consume(self, tokens: int = 1) -> bool:
        """Try to consume tokens. Returns ``True`` if allowed, ``False`` if rate limited.

        Args:
            tokens: Number of tokens to consume (default 1).

        Returns:
            ``True`` if the request is allowed, ``False`` if rate limited.
        """
        with self._lock:
            self._refill()
            if self._tokens >= tokens:
                self._tokens -= tokens
                return True
            return False

    def _refill(self):
        """Add whole tokens based on accumulated elapsed time.

        Saves up fractional time so that sub-token intervals are not
        lost.  Tokens are only added as whole integers, preventing
        floating-point drift from making the bucket appear non-empty
        when it should be at zero.
        """
        now = time.monotonic()
        elapsed = now - self._last_refill
        self._last_refill = now

        if self.rate <= 0:
            return

        self._accumulated += elapsed
        token_interval = 1.0 / self.rate

        # Add whole tokens that have been earned
        new_tokens = int(self._accumulated / token_interval)
        if new_tokens > 0:
            self._tokens = min(self.capacity, self._tokens + new_tokens)
            # Keep the leftover fractional time
            self._accumulated -= new_tokens * token_interval

    @property
    def available(self) -> int:
        """Current number of available tokens."""
        with self._lock:
            self._refill()
            return self._tokens

    def reset(self):
        """Reset the bucket to full capacity."""
        with self._lock:
            self._tokens = self.capacity
            self._accumulated = 0.0
            self._last_refill = time.monotonic()


class SlidingWindowCounter:
    """Sliding window counter for tracking request counts.

    Maintains request counts in fixed time slots and sums recent slots
    to approximate a sliding window.

    Args:
        window_seconds: Total window duration in seconds.
        num_slots: Number of time slots for approximation.
    """

    def __init__(self, window_seconds: float = 60.0, num_slots: int = 6):
        self.window_seconds = window_seconds
        self.num_slots = num_slots
        self.slot_duration = window_seconds / num_slots
        self._counts: Dict[int, int] = defaultdict(int)
        self._current_slot = self._get_slot()

    def _get_slot(self) -> int:
        """Get the current time slot index."""
        return int(time.monotonic() / self.slot_duration)

    def increment(self):
        """Record a request in the current slot."""
        slot = self._get_slot()
        # Clear expired slots
        expired_slot = slot - self.num_slots
        for s in list(self._counts.keys()):
            if s <= expired_slot:
                del self._counts[s]
        self._counts[slot] += 1

    def count(self) -> int:
        """Get the approximate count within the sliding window."""
        current = self._get_slot()
        total = 0
        for i in range(self.num_slots):
            total += self._counts.get(current - i, 0)
        return total

    def reset(self):
        """Clear all counters."""
        self._counts.clear()


class RateLimitMiddleware(BaseHTTPMiddleware):
    """FastAPI middleware for token-bucket rate limiting.

    Tracks per-key (default: per-IP) rate limits with optional
    per-endpoint overrides. Returns ``429 Too Many Requests``
    when the limit is exceeded.

    Attributes set on the response:
        ``X-RateLimit-Limit``: The burst capacity
        ``X-RateLimit-Remaining``: Remaining tokens in the bucket
        ``X-RateLimit-Reset``: Seconds until the bucket is fully refilled
        ``Retry-After``: Seconds to wait before retrying (on 429)
    """

    def __init__(self, app, config: RateLimitConfig):
        super().__init__(app)
        self._config = config
        self._buckets: Dict[str, TokenBucket] = {}
        self._counters: Dict[str, SlidingWindowCounter] = {}

    def _get_key(self, request: Request) -> str:
        """Extract the rate limit key from the request.

        Defaults to the client IP address. Override by providing
        a custom ``key_func`` in the config.
        """
        # FastAPI/Starlette behind proxy: check X-Forwarded-For, then X-Real-IP
        forwarded = request.headers.get("X-Forwarded-For")
        if forwarded:
            return forwarded.split(",")[0].strip()

        real_ip = request.headers.get("X-Real-IP")
        if real_ip:
            return real_ip

        if request.client:
            return request.client.host

        return "unknown"

    def _get_bucket(self, key: str, path: str) -> TokenBucket:
        """Get or create a token bucket for the given key and path."""
        if self._config.per_endpoint:
            bucket_key = f"{key}:{path}"
        else:
            bucket_key = key

        if bucket_key not in self._buckets:
            rate, burst = self._get_limits(path)
            self._buckets[bucket_key] = TokenBucket(rate=rate, capacity=burst)

        return self._buckets[bucket_key]

    def _get_limits(self, path: str) -> Tuple[float, int]:
        """Get the rate and burst for a given path.

        Checks per-endpoint overrides, then falls back to defaults.
        """
        for prefix, (rate, burst) in self._config.endpoint_limits.items():
            if path.startswith(prefix):
                return rate, burst
        return self._config.default_limit, self._config.default_burst

    def _is_public_path(self, path: str) -> bool:
        """Check if a path should bypass rate limiting."""
        if path in self._config.public_paths:
            return True
        for public in self._config.public_paths:
            if public.endswith("/") and path.startswith(public):
                return True
        return False

    async def dispatch(self, request: Request, call_next):
        """Process request through rate limiting."""
        path = request.url.path

        # Skip if disabled
        if not self._config.enabled:
            return await call_next(request)

        # Skip public paths
        if self._is_public_path(path):
            return await call_next(request)

        # Skip OPTIONS (CORS preflight)
        if request.method == "OPTIONS":
            return await call_next(request)

        key = self._get_key(request)
        bucket = self._get_bucket(key, path)

        if not bucket.consume():
            # Rate limited
            available = bucket.available
            retry_after = max(1, int((1 - available) / bucket.rate))

            # Track the rate limit event
            counter_key = f"{key}:{path}:limited"
            if counter_key not in self._counters:
                self._counters[counter_key] = SlidingWindowCounter()
            self._counters[counter_key].increment()

            logger.warning(
                "Rate limit exceeded for key=%s path=%s (retry_after=%ds)",
                key, path, retry_after,
            )

            return JSONResponse(
                status_code=429,
                content={
                    "detail": "Rate limit exceeded",
                    "retry_after": retry_after,
                },
                headers={
                    "Retry-After": str(retry_after),
                    "X-RateLimit-Limit": str(bucket.capacity),
                    "X-RateLimit-Remaining": "0",
                    "X-RateLimit-Reset": str(retry_after),
                },
            )

        # Process the request
        response = await call_next(request)

        # Add rate limit headers
        remaining = max(0, int(bucket.available))
        reset_seconds = max(1, int((bucket.capacity - bucket.available) / bucket.rate))

        response.headers["X-RateLimit-Limit"] = str(bucket.capacity)
        response.headers["X-RateLimit-Remaining"] = str(remaining)
        response.headers["X-RateLimit-Reset"] = str(reset_seconds)

        return response

    def get_stats(self) -> Dict:
        """Get rate limiting statistics.

        Returns a dict with bucket counts and rate limit event counters.
        """
        return {
            "active_buckets": len(self._buckets),
            "active_counters": len(self._counters),
            "total_limited_events": sum(
                c.count() for c in self._counters.values()
            ),
        }

    def reset(self):
        """Reset all rate limit buckets and counters."""
        self._buckets.clear()
        self._counters.clear()
