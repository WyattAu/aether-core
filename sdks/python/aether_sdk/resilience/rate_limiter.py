"""
Rate Limiter Implementation

Provides rate limiting with multiple strategies:
- Token Bucket: Allows bursts up to bucket size
- Sliding Window: Smooth rate limiting over time
- Fixed Window: Simple window-based limiting

Example:
    >>> from aether_sdk.resilience.rate_limiter import RateLimiter, RateLimitConfig
    >>> limiter = RateLimiter(RateLimitConfig(requests_per_second=100))
    >>> await limiter.acquire(max_wait_ms=5000)
    >>> result = await limiter.execute(my_func)
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, Callable
from enum import Enum
import asyncio
import time
import random


class RateLimitStrategy(Enum):
    """Rate limiting strategy selection.

    Attributes:
        TOKEN_BUCKET: Tokens refill at a constant rate; allows bursts
            up to the bucket capacity.
        SLIDING_WINDOW: Counts requests over a sliding time window
            for smooth, accurate limiting.
        FIXED_WINDOW: Counts requests in fixed time intervals;
            allows a burst at the start of each window.
    """
    TOKEN_BUCKET = "token-bucket"
    SLIDING_WINDOW = "sliding-window"
    FIXED_WINDOW = "fixed-window"


@dataclass
class RateLimitConfig:
    """Configuration for a :class:`RateLimiter`.

    Attributes:
        requests_per_second: Target request rate.
        burst_size: Maximum burst size (token bucket only). Defaults
            to ``requests_per_second`` if not set.
        strategy: The rate limiting algorithm to use.
        window_size_ms: Window duration for sliding/fixed strategies
            (default 1000 ms).
    """
    requests_per_second: int = 100
    burst_size: Optional[int] = None
    strategy: RateLimitStrategy = RateLimitStrategy.TOKEN_BUCKET
    window_size_ms: int = 1000


@dataclass
class RateLimitStats:
    """Statistics for a :class:`RateLimiter`.

    Attributes:
        allowed_requests: Total requests that were permitted.
        rejected_requests: Total requests that were rejected.
        current_rate: Approximate current request rate.
        wait_time_ms: Estimated wait time for the next request (ms).
    """
    allowed_requests: int = 0
    rejected_requests: int = 0
    current_rate: float = 0.0
    wait_time_ms: int = 0


@dataclass
class RateLimitResult:
    """Result of a non-blocking rate limit check.

    Attributes:
        allowed: Whether the request is permitted.
        wait_time_ms: Estimated milliseconds until a slot is available.
        remaining_tokens: Remaining tokens (token bucket only).
        reset_in: Milliseconds until the current window resets
            (window strategies only).
    """
    allowed: bool
    wait_time_ms: int = 0
    remaining_tokens: Optional[int] = None
    reset_in: Optional[int] = None


class RateLimitExhaustedError(Exception):
    """Raised when a rate limit cannot be satisfied within the allowed wait time."""
    pass


# ============================================
# Token Bucket Implementation
# ============================================

class TokenBucket:
    """Token bucket rate limiter implementation.

    Tokens refill at a constant rate. Requests consume tokens; if
    insufficient tokens are available the request is rejected but the
    caller can calculate how long to wait.

    Args:
        requests_per_second: Refill rate.
        burst_size: Maximum token capacity (bucket depth).
    """

    def __init__(self, requests_per_second: int, burst_size: int):
        self._max_tokens = burst_size
        self._tokens = float(burst_size)
        self._refill_rate = requests_per_second / 1000.0
        self._last_refill = time.time() * 1000
        self._lock = asyncio.Lock()

    async def try_acquire(self, tokens: int = 1) -> RateLimitResult:
        """Attempt to acquire tokens without blocking.

        Args:
            tokens: Number of tokens to acquire (default 1).

        Returns:
            A :class:`RateLimitResult` indicating whether the
            acquisition succeeded and, if not, how long to wait.
        """
        async with self._lock:
            self._refill()

            if self._tokens >= tokens:
                self._tokens -= tokens
                return RateLimitResult(
                    allowed=True,
                    wait_time_ms=0,
                    remaining_tokens=int(self._tokens),
                )

            tokens_needed = tokens - self._tokens
            wait_time_ms = int(tokens_needed / self._refill_rate)

            return RateLimitResult(
                allowed=False,
                wait_time_ms=wait_time_ms,
                remaining_tokens=int(self._tokens),
            )

    def _refill(self) -> None:
        """Refill tokens based on elapsed time since the last refill."""
        now = time.time() * 1000
        elapsed = now - self._last_refill
        tokens_to_add = elapsed * self._refill_rate

        self._tokens = min(self._max_tokens, self._tokens + tokens_to_add)
        self._last_refill = now

    def get_tokens(self) -> int:
        """Return the current number of available tokens.

        Returns:
            Integer token count after refilling.
        """
        self._refill()
        return int(self._tokens)


# ============================================
# Sliding Window Implementation
# ============================================

class SlidingWindow:
    """Sliding window rate limiter implementation.

    Tracks individual request timestamps within a sliding window to
    provide smooth and accurate rate limiting.

    Args:
        requests_per_second: Maximum requests per window.
        window_size_ms: Window duration in milliseconds (default 1000).
    """

    def __init__(self, requests_per_second: int, window_size_ms: int = 1000):
        self._max_requests = requests_per_second
        self._window_size_ms = window_size_ms
        self._requests: list[float] = []
        self._lock = asyncio.Lock()

    async def try_acquire(self) -> RateLimitResult:
        """Attempt to acquire permission without blocking.

        Returns:
            A :class:`RateLimitResult`.
        """
        async with self._lock:
            now = time.time() * 1000
            window_start = now - self._window_size_ms

            self._requests = [t for t in self._requests if t > window_start]

            if len(self._requests) < self._max_requests:
                self._requests.append(now)
                return RateLimitResult(
                    allowed=True,
                    wait_time_ms=0,
                    remaining_tokens=self._max_requests - len(self._requests),
                )

            oldest_request = self._requests[0]
            wait_time_ms = int(oldest_request + self._window_size_ms - now)

            return RateLimitResult(
                allowed=False,
                wait_time_ms=max(1, wait_time_ms),
                reset_in=wait_time_ms,
            )

    def get_current_count(self) -> int:
        """Return the number of requests in the current window.

        Returns:
            Integer count of recorded requests within the window.
        """
        now = time.time() * 1000
        window_start = now - self._window_size_ms
        return len([t for t in self._requests if t > window_start])


# ============================================
# Fixed Window Implementation
# ============================================

class FixedWindow:
    """Fixed window rate limiter implementation.

    Divides time into fixed-size windows and counts requests per window.
    Simpler than sliding window but allows a burst at window boundaries.

    Args:
        requests_per_second: Maximum requests per window.
        window_size_ms: Window duration in milliseconds (default 1000).
    """

    def __init__(self, requests_per_second: int, window_size_ms: int = 1000):
        self._max_requests = requests_per_second
        self._window_size_ms = window_size_ms
        self._count = 0
        self._window_start = time.time() * 1000
        self._lock = asyncio.Lock()

    async def try_acquire(self) -> RateLimitResult:
        """Attempt to acquire permission without blocking.

        Returns:
            A :class:`RateLimitResult`.
        """
        async with self._lock:
            now = time.time() * 1000

            if now - self._window_start >= self._window_size_ms:
                self._count = 0
                self._window_start = now

            if self._count < self._max_requests:
                self._count += 1
                return RateLimitResult(
                    allowed=True,
                    wait_time_ms=0,
                    remaining_tokens=self._max_requests - self._count,
                    reset_in=int(self._window_start + self._window_size_ms - now),
                )

            return RateLimitResult(
                allowed=False,
                wait_time_ms=int(self._window_start + self._window_size_ms - now),
                reset_in=int(self._window_start + self._window_size_ms - now),
            )

    def get_current_count(self) -> int:
        """Return the request count in the current fixed window.

        Returns:
            Integer count of requests recorded in this window.
        """
        return self._count


# ============================================
# Rate Limiter
# ============================================

class RateLimiter:
    """Rate limiter with pluggable strategy backends.

    Delegates to :class:`TokenBucket`, :class:`SlidingWindow`, or
    :class:`FixedWindow` based on the configured
    :class:`RateLimitStrategy`.

    Example:
        >>> rl = RateLimiter(RateLimitConfig(requests_per_second=50))
        >>> await rl.execute(my_func)
    """

    def __init__(self, config: Optional[RateLimitConfig] = None):
        """Initialize the rate limiter.

        Args:
            config: Optional configuration. Defaults to
                :class:`RateLimitConfig`.

        Raises:
            ValueError: If an unknown strategy is specified.
        """
        self._config = config or RateLimitConfig()

        burst_size = (
            self._config.burst_size
            if self._config.burst_size is not None
            else self._config.requests_per_second
        )

        if self._config.strategy == RateLimitStrategy.TOKEN_BUCKET:
            self._impl: Any = TokenBucket(
                self._config.requests_per_second,
                burst_size,
            )
        elif self._config.strategy == RateLimitStrategy.SLIDING_WINDOW:
            self._impl = SlidingWindow(
                self._config.requests_per_second,
                self._config.window_size_ms,
            )
        elif self._config.strategy == RateLimitStrategy.FIXED_WINDOW:
            self._impl = FixedWindow(
                self._config.requests_per_second,
                self._config.window_size_ms,
            )
        else:
            raise ValueError(f"Unknown rate limit strategy: {self._config.strategy}")

        self._allowed_requests = 0
        self._rejected_requests = 0

    async def try_acquire(self, tokens: int = 1) -> RateLimitResult:
        """Non-blocking rate limit check.

        Args:
            tokens: Number of tokens to acquire (token bucket only).

        Returns:
            A :class:`RateLimitResult` indicating whether the request
            is allowed.
        """
        if isinstance(self._impl, TokenBucket):
            result = await self._impl.try_acquire(tokens)
        else:
            result = await self._impl.try_acquire()

        if result.allowed:
            self._allowed_requests += 1
        else:
            self._rejected_requests += 1

        return result

    async def acquire(self, max_wait_ms: int = 5000) -> None:
        """Blocking acquire — waits if necessary for a permit.

        Args:
            max_wait_ms: Maximum time to wait in milliseconds.

        Raises:
            RateLimitExhaustedError: If the wait time would exceed
                *max_wait_ms*, or if the rate limit is still exceeded
                after waiting.
        """
        result = await self.try_acquire()

        if result.allowed:
            return

        if result.wait_time_ms > max_wait_ms:
            self._rejected_requests += 1
            raise RateLimitExhaustedError(
                f"Rate limit exceeded. Wait time {result.wait_time_ms}ms "
                f"exceeds max {max_wait_ms}ms"
            )

        await asyncio.sleep(result.wait_time_ms / 1000)

        retry_result = await self.try_acquire()
        if not retry_result.allowed:
            raise RateLimitExhaustedError("Rate limit still exceeded after waiting")

    async def execute(self, func: Callable[[], Any], max_wait_ms: int = 5000) -> Any:
        """Execute a function after acquiring a rate-limit permit.

        Args:
            func: A zero-argument async callable.
            max_wait_ms: Maximum time to wait for a permit.

        Returns:
            The result of *func*.

        Raises:
            RateLimitExhaustedError: If a permit cannot be obtained
                within *max_wait_ms*.
        """
        await self.acquire(max_wait_ms)
        return await func()

    def get_stats(self) -> RateLimitStats:
        """Return a snapshot of the rate limiter statistics.

        Returns:
            A :class:`RateLimitStats` dataclass.
        """
        current_rate = 0.0

        if isinstance(self._impl, TokenBucket):
            current_rate = self._config.requests_per_second * (
                1 - self._impl.get_tokens() / self._config.burst_size
            ) if self._config.burst_size else 0
        elif isinstance(self._impl, SlidingWindow):
            current_rate = self._impl.get_current_count()
        elif isinstance(self._impl, FixedWindow):
            current_rate = self._impl.get_current_count()

        return RateLimitStats(
            allowed_requests=self._allowed_requests,
            rejected_requests=self._rejected_requests,
            current_rate=current_rate,
            wait_time_ms=0,
        )

    def reset_stats(self) -> None:
        """Reset acceptance/rejection counters to zero."""
        self._allowed_requests = 0
        self._rejected_requests = 0


# ============================================
# Rate Limiter Manager
# ============================================

class RateLimiterManager:
    """Registry for named :class:`RateLimiter` instances.

    Example:
        >>> mgr = RateLimiterManager()
        >>> rl = mgr.get("external-api")
    """

    def __init__(self, default_config: Optional[RateLimitConfig] = None):
        """Initialize the manager.

        Args:
            default_config: Default configuration for new limiters.
        """
        self._limiters: Dict[str, RateLimiter] = {}
        self._default_config = default_config or RateLimitConfig()

    def get(
        self,
        name: str,
        config: Optional[RateLimitConfig] = None
    ) -> RateLimiter:
        """Get or create a rate limiter by name.

        Args:
            name: Unique name for the limiter.
            config: Optional per-limiter configuration.

        Returns:
            The :class:`RateLimiter` instance for *name*.
        """
        if name not in self._limiters:
            merged_config = RateLimitConfig(
                requests_per_second=(
                    config.requests_per_second
                    if config
                    else self._default_config.requests_per_second
                ),
                burst_size=(
                    config.burst_size
                    if config
                    else self._default_config.burst_size
                ),
                strategy=(
                    config.strategy
                    if config
                    else self._default_config.strategy
                ),
                window_size_ms=(
                    config.window_size_ms
                    if config
                    else self._default_config.window_size_ms
                ),
            )
            self._limiters[name] = RateLimiter(merged_config)
        return self._limiters[name]

    def get_all_stats(self) -> Dict[str, RateLimitStats]:
        """Return statistics for every registered rate limiter.

        Returns:
            A dict mapping limiter names to their :class:`RateLimitStats`.
        """
        return {name: limiter.get_stats() for name, limiter in self._limiters.items()}

    def reset_all_stats(self) -> None:
        """Reset statistics for every registered rate limiter."""
        for limiter in self._limiters.values():
            limiter.reset_stats()


# ============================================
# Predefined Rate Limiters
# ============================================

def api_rate_limiter() -> RateLimiter:
    """Create a rate limiter for API requests (100 req/s with 200 burst).

    Returns:
        A :class:`RateLimiter` using the token bucket strategy.
    """
    return RateLimiter(RateLimitConfig(
        requests_per_second=100,
        burst_size=200,
        strategy=RateLimitStrategy.TOKEN_BUCKET,
    ))


def strict_rate_limiter(requests_per_second: int) -> RateLimiter:
    """Create a strict rate limiter without burst allowance.

    Args:
        requests_per_second: Maximum requests per second.

    Returns:
        A :class:`RateLimiter` using the sliding window strategy.
    """
    return RateLimiter(RateLimitConfig(
        requests_per_second=requests_per_second,
        strategy=RateLimitStrategy.SLIDING_WINDOW,
    ))


def bursty_rate_limiter(burst_size: int, refill_rate: int) -> RateLimiter:
    """Create a rate limiter optimized for bursty traffic patterns.

    Args:
        burst_size: Maximum burst size (bucket capacity).
        refill_rate: Tokens added per second.

    Returns:
        A :class:`RateLimiter` using the token bucket strategy.
    """
    return RateLimiter(RateLimitConfig(
        requests_per_second=refill_rate,
        burst_size=burst_size,
        strategy=RateLimitStrategy.TOKEN_BUCKET,
    ))
