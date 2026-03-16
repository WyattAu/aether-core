"""
Retry Policy with Exponential Backoff Implementation

Provides configurable retry logic for transient failures.
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Callable, Optional, Any, TypeVar, Union, Generic, Awaitable
from enum import Enum
import asyncio
import random

T = TypeVar('T')


class BackoffStrategy(Enum):
    FIXED = "fixed"
    LINEAR = "linear"
    EXPONENTIAL = "exponential"
    EXPONENTIAL_JITTER = "exponential-jitter"


@dataclass
class RetryConfig:
    """Configuration for retry policy."""
    max_attempts: int = 3
    backoff: BackoffStrategy = BackoffStrategy.EXPONENTIAL_JITTER
    base_delay_ms: int = 100
    max_delay_ms: int = 30000
    multiplier: float = 2.0
    jitter_factor: float = 0.1
    is_retryable: Optional[Callable[[Exception, int], bool]] = None
    on_retry: Optional[Callable[[Exception, int, int], None]] = None
    on_exhausted: Optional[Callable[[Exception, int], None]] = None


@dataclass
class RetryStats:
    """Statistics for retry policy."""
    total_attempts: int = 0
    successful_attempts: int = 0
    failed_attempts: int = 0
    retried_calls: int = 0
    exhausted_calls: int = 0
    total_retry_delay_ms: int = 0


@dataclass
class RetryResult(Generic[T]):
    """Result of a retry operation."""
    result: T
    attempts: int
    total_delay_ms: int


class RetryExhaustedError(Exception):
    """Raised when all retries are exhausted."""
    def __init__(self, message: str, last_error: Exception, attempts: int, total_delay_ms: int):
        super().__init__(message)
        self.last_error = last_error
        self.attempts = attempts
        self.total_delay_ms = total_delay_ms


class RetryPolicy:
    """Retry policy with configurable backoff strategies."""
    
    def __init__(self, config: Optional[RetryConfig] = None):
        self._config = config or RetryConfig()
        self._stats = RetryStats()
    
    async def execute(self, func: Callable[[], Awaitable[T]]) -> RetryResult[T]:
        """Execute a function with retry logic.
        
        Args:
            func: Async function to execute
            
        Returns:
            RetryResult with result and metadata
            
        Raises:
            RetryExhaustedError: If all retries exhausted
        """
        attempt = 0
        total_delay_ms = 0
        last_error: Optional[Exception] = None
        
        while attempt < self._config.max_attempts:
            attempt += 1
            self._stats.total_attempts += 1
            
            try:
                result = await func()
                self._stats.successful_attempts += 1
                
                if attempt > 1:
                    self._stats.retried_calls += 1
                
                return RetryResult(
                    result=result,
                    attempts=attempt,
                    total_delay_ms=total_delay_ms,
                )
            except Exception as error:
                last_error = error
                self._stats.failed_attempts += 1
                
                # Check if we should retry
                is_retryable = (
                    self._config.is_retryable(error, attempt)
                    if self._config.is_retryable
                    else self._is_retryable_default(error)
                )
                
                if attempt >= self._config.max_attempts or not is_retryable:
                    break
                
                # Calculate delay
                delay = self._calculate_delay(attempt)
                total_delay_ms += delay
                self._stats.total_retry_delay_ms += delay
                
                # Notify callback
                if self._config.on_retry:
                    self._config.on_retry(error, attempt, delay)
                
                # Wait before retry
                await asyncio.sleep(delay / 1000)
        
        # All retries exhausted - assert last_error is not None
        assert last_error is not None, "last_error should not be None after loop"
        
        # All retries exhausted
        self._stats.exhausted_calls += 1
        if self._config.on_exhausted:
            self._config.on_exhausted(last_error, attempt)
        
        raise RetryExhaustedError(
            f"All {self._config.max_attempts} retry attempts exhausted",
            last_error,
            attempt,
            total_delay_ms,
        )
    
    async def execute_safe(self, func: Callable[[], Awaitable[T]]) -> Optional[RetryResult[T]]:
        """Execute with result wrapper (doesn't throw on exhaustion)."""
        try:
            return await self.execute(func)
        except RetryExhaustedError:
            return None
    
    def get_stats(self) -> RetryStats:
        """Get current statistics."""
        return RetryStats(
            total_attempts=self._stats.total_attempts,
            successful_attempts=self._stats.successful_attempts,
            failed_attempts=self._stats.failed_attempts,
            retried_calls=self._stats.retried_calls,
            exhausted_calls=self._stats.exhausted_calls,
            total_retry_delay_ms=self._stats.total_retry_delay_ms,
        )
    
    def reset_stats(self) -> None:
        """Reset statistics."""
        self._stats = RetryStats()
    
    def _calculate_delay(self, attempt: int) -> int:
        """Calculate delay for the given attempt."""
        delay = 0
        
        if self._config.backoff == BackoffStrategy.FIXED:
            delay = self._config.base_delay_ms
        elif self._config.backoff == BackoffStrategy.LINEAR:
            delay = self._config.base_delay_ms * attempt
        elif self._config.backoff == BackoffStrategy.EXPONENTIAL:
            delay = self._config.base_delay_ms * (self._config.multiplier ** (attempt - 1))
        elif self._config.backoff == BackoffStrategy.EXPONENTIAL_JITTER:
            base = self._config.base_delay_ms * (self._config.multiplier ** (attempt - 1))
            delay = self._add_jitter(base)
        
        return min(int(delay), self._config.max_delay_ms)
    
    def _add_jitter(self, delay: float) -> int:
        """Add jitter to delay."""
        jitter = delay * self._config.jitter_factor
        return int(delay + random.uniform(-jitter, jitter))
    
    def _is_retryable_default(self, error: Exception) -> bool:
        """Default retryable error detection."""
        transient_messages = [
            'ECONNRESET',
            'ETIMEDOUT',
            'ENOTFOUND',
            'ECONNREFUSED',
            'network',
            'timeout',
            'unavailable',
            'temporary',
        ]
        message = str(error).lower()
        return any(m in message for m in transient_messages)


# ============================================
# Predefined Retry Policies
# ============================================

def network_retry_policy(**overrides) -> RetryPolicy:
    """Create a retry policy for transient network errors."""
    return RetryPolicy(RetryConfig(
        max_attempts=overrides.get('max_attempts', 3),
        backoff=BackoffStrategy.EXPONENTIAL_JITTER,
        base_delay_ms=overrides.get('base_delay_ms', 100),
        max_delay_ms=overrides.get('max_delay_ms', 5000),
    ))


def database_retry_policy(**overrides) -> RetryPolicy:
    """Create a retry policy for database operations."""
    return RetryPolicy(RetryConfig(
        max_attempts=overrides.get('max_attempts', 5),
        backoff=BackoffStrategy.EXPONENTIAL,
        base_delay_ms=overrides.get('base_delay_ms', 50),
        max_delay_ms=overrides.get('max_delay_ms', 2000),
        multiplier=overrides.get('multiplier', 2.0),
    ))


def aggressive_retry_policy(**overrides) -> RetryPolicy:
    """Create an aggressive retry policy (many attempts, short delays)."""
    return RetryPolicy(RetryConfig(
        max_attempts=overrides.get('max_attempts', 10),
        backoff=BackoffStrategy.EXPONENTIAL_JITTER,
        base_delay_ms=overrides.get('base_delay_ms', 10),
        max_delay_ms=overrides.get('max_delay_ms', 1000),
        multiplier=overrides.get('multiplier', 1.5),
        jitter_factor=overrides.get('jitter_factor', 0.2),
    ))


def conservative_retry_policy(**overrides) -> RetryPolicy:
    """Create a conservative retry policy (few attempts, longer delays)."""
    return RetryPolicy(RetryConfig(
        max_attempts=overrides.get('max_attempts', 2),
        backoff=BackoffStrategy.EXPONENTIAL,
        base_delay_ms=overrides.get('base_delay_ms', 1000),
        max_delay_ms=overrides.get('max_delay_ms', 10000),
        multiplier=overrides.get('multiplier', 3.0),
    ))
