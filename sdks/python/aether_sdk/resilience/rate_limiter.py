"""
Rate Limiter Implementation

Provides rate limiting with multiple strategies:
- Token Bucket: Allows bursts up to bucket size
- Sliding Window: Smooth rate limiting over time
- Fixed Window: Simple window-based limiting
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional, Dict, Any, Callable
from enum import Enum
import asyncio
import time
import random


class RateLimitStrategy(Enum):
    TOKEN_BUCKET = "token-bucket"
    SLIDING_WINDOW = "sliding-window"
    FIXED_WINDOW = "fixed-window"


@dataclass
class RateLimitConfig:
    """Configuration for rate limiter."""
    requests_per_second: int = 100
    burst_size: Optional[int] = None  # For token bucket
    strategy: RateLimitStrategy = RateLimitStrategy.TOKEN_BUCKET
    window_size_ms: int = 1000  # For window strategies


@dataclass
class RateLimitStats:
    """Statistics for rate limiter."""
    allowed_requests: int = 0
    rejected_requests: int = 0
    current_rate: float = 0.0
    wait_time_ms: int = 0


@dataclass
class RateLimitResult:
    """Result of a rate limit check."""
    allowed: bool
    wait_time_ms: int = 0
    remaining_tokens: Optional[int] = None
    reset_in: Optional[int] = None


class RateLimitExhaustedError(Exception):
    """Raised when rate limit is exceeded."""
    pass


# ============================================
# Token Bucket Implementation
# ============================================

class TokenBucket:
    """Token bucket rate limiter implementation."""
    
    def __init__(self, requests_per_second: int, burst_size: int):
        self._max_tokens = burst_size
        self._tokens = float(burst_size)
        self._refill_rate = requests_per_second / 1000.0  # tokens per ms
        self._last_refill = time.time() * 1000
        self._lock = asyncio.Lock()
    
    async def try_acquire(self, tokens: int = 1) -> RateLimitResult:
        """Try to acquire tokens."""
        async with self._lock:
            self._refill()
            
            if self._tokens >= tokens:
                self._tokens -= tokens
                return RateLimitResult(
                    allowed=True,
                    wait_time_ms=0,
                    remaining_tokens=int(self._tokens),
                )
            
            # Calculate wait time for required tokens
            tokens_needed = tokens - self._tokens
            wait_time_ms = int(tokens_needed / self._refill_rate)
            
            return RateLimitResult(
                allowed=False,
                wait_time_ms=wait_time_ms,
                remaining_tokens=int(self._tokens),
            )
    
    def _refill(self) -> None:
        """Refill tokens based on elapsed time."""
        now = time.time() * 1000
        elapsed = now - self._last_refill
        tokens_to_add = elapsed * self._refill_rate
        
        self._tokens = min(self._max_tokens, self._tokens + tokens_to_add)
        self._last_refill = now
    
    def get_tokens(self) -> int:
        """Get current token count."""
        self._refill()
        return int(self._tokens)


# ============================================
# Sliding Window Implementation
# ============================================

class SlidingWindow:
    """Sliding window rate limiter implementation."""
    
    def __init__(self, requests_per_second: int, window_size_ms: int = 1000):
        self._max_requests = requests_per_second
        self._window_size_ms = window_size_ms
        self._requests: list[float] = []
        self._lock = asyncio.Lock()
    
    async def try_acquire(self) -> RateLimitResult:
        """Try to acquire permission."""
        async with self._lock:
            now = time.time() * 1000
            window_start = now - self._window_size_ms
            
            # Remove old requests
            self._requests = [t for t in self._requests if t > window_start]
            
            if len(self._requests) < self._max_requests:
                self._requests.append(now)
                return RateLimitResult(
                    allowed=True,
                    wait_time_ms=0,
                    remaining_tokens=self._max_requests - len(self._requests),
                )
            
            # Calculate wait time until oldest request exits window
            oldest_request = self._requests[0]
            wait_time_ms = int(oldest_request + self._window_size_ms - now)
            
            return RateLimitResult(
                allowed=False,
                wait_time_ms=max(1, wait_time_ms),
                reset_in=wait_time_ms,
            )
    
    def get_current_count(self) -> int:
        """Get current request count in window."""
        now = time.time() * 1000
        window_start = now - self._window_size_ms
        return len([t for t in self._requests if t > window_start])


# ============================================
# Fixed Window Implementation
# ============================================

class FixedWindow:
    """Fixed window rate limiter implementation."""
    
    def __init__(self, requests_per_second: int, window_size_ms: int = 1000):
        self._max_requests = requests_per_second
        self._window_size_ms = window_size_ms
        self._count = 0
        self._window_start = time.time() * 1000
        self._lock = asyncio.Lock()
    
    async def try_acquire(self) -> RateLimitResult:
        """Try to acquire permission."""
        async with self._lock:
            now = time.time() * 1000
            
            # Check if we need to reset the window
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
        """Get current count in window."""
        return self._count


# ============================================
# Rate Limiter
# ============================================

class RateLimiter:
    """Rate limiter with multiple strategies."""
    
    def __init__(self, config: Optional[RateLimitConfig] = None):
        self._config = config or RateLimitConfig()
        
        # Resolve burst size
        burst_size = (
            self._config.burst_size 
            if self._config.burst_size is not None 
            else self._config.requests_per_second
        )
        
        # Create strategy implementation
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
        """Try to acquire permission (non-blocking)."""
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
        """Acquire permission, waiting if necessary.
        
        Raises:
            RateLimitExhaustedError: If wait time exceeds max
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
        
        # Try again after waiting
        retry_result = await self.try_acquire()
        if not retry_result.allowed:
            raise RateLimitExhaustedError("Rate limit still exceeded after waiting")
    
    async def execute(self, func: Callable[[], Any], max_wait_ms: int = 5000) -> Any:
        """Execute a function with rate limiting."""
        await self.acquire(max_wait_ms)
        return await func()
    
    def get_stats(self) -> RateLimitStats:
        """Get current statistics."""
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
            wait_time_ms=0,  # Would need to call try_acquire to get this
        )
    
    def reset_stats(self) -> None:
        """Reset statistics."""
        self._allowed_requests = 0
        self._rejected_requests = 0


# ============================================
# Rate Limiter Manager
# ============================================

class RateLimiterManager:
    """Manages multiple rate limiters by name."""
    
    def __init__(self, default_config: Optional[RateLimitConfig] = None):
        self._limiters: Dict[str, RateLimiter] = {}
        self._default_config = default_config or RateLimitConfig()
    
    def get(
        self, 
        name: str, 
        config: Optional[RateLimitConfig] = None
    ) -> RateLimiter:
        """Get or create a rate limiter by name."""
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
        """Get statistics for all rate limiters."""
        return {name: limiter.get_stats() for name, limiter in self._limiters.items()}
    
    def reset_all_stats(self) -> None:
        """Reset all statistics."""
        for limiter in self._limiters.values():
            limiter.reset_stats()


# ============================================
# Predefined Rate Limiters
# ============================================

def api_rate_limiter() -> RateLimiter:
    """Create a rate limiter for API requests (100 req/s with bursts)."""
    return RateLimiter(RateLimitConfig(
        requests_per_second=100,
        burst_size=200,
        strategy=RateLimitStrategy.TOKEN_BUCKET,
    ))


def strict_rate_limiter(requests_per_second: int) -> RateLimiter:
    """Create a rate limiter for strict limiting (no bursts)."""
    return RateLimiter(RateLimitConfig(
        requests_per_second=requests_per_second,
        strategy=RateLimitStrategy.SLIDING_WINDOW,
    ))


def bursty_rate_limiter(burst_size: int, refill_rate: int) -> RateLimiter:
    """Create a rate limiter for bursty traffic."""
    return RateLimiter(RateLimitConfig(
        requests_per_second=refill_rate,
        burst_size=burst_size,
        strategy=RateLimitStrategy.TOKEN_BUCKET,
    ))
