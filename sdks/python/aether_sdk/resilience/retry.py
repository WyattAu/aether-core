"""
Retry Policy with Exponential Backoff Implementation

Provides configurable retry logic for transient failures.

Example:
    >>> from aether_sdk.resilience.retry import RetryPolicy, RetryConfig, BackoffStrategy
    >>> policy = RetryPolicy(RetryConfig(max_attempts=5, backoff=BackoffStrategy.EXPONENTIAL))
    >>> result = await policy.execute(my_async_func)
"""

from __future__ import annotations

import asyncio
import random
from dataclasses import dataclass
from enum import Enum
from typing import Awaitable, Callable, Generic, Optional, TypeVar

T = TypeVar("T")


class BackoffStrategy(Enum):
    """Supported backoff strategies for retry delay calculation.

    Attributes:
        FIXED: Constant delay between retries.
        LINEAR: Delay increases linearly with the attempt number.
        EXPONENTIAL: Delay doubles (or multiplies) with each attempt.
        EXPONENTIAL_JITTER: Exponential delay with random jitter to
            avoid thundering-herd effects.
    """

    FIXED = "fixed"
    LINEAR = "linear"
    EXPONENTIAL = "exponential"
    EXPONENTIAL_JITTER = "exponential-jitter"


@dataclass
class RetryConfig:
    """Configuration for a :class:`RetryPolicy`.

    Attributes:
        max_attempts: Maximum number of attempts (including the first).
        backoff: Strategy used to calculate delay between attempts.
        base_delay_ms: Base delay in milliseconds.
        max_delay_ms: Upper bound for the calculated delay.
        multiplier: Multiplier for exponential/linear strategies.
        jitter_factor: Fraction of the delay used for jitter (e.g.
            ``0.1`` means ±10 %).
        is_retryable: Optional callback ``(error, attempt) -> bool``
            that determines whether a specific error should be retried.
        on_retry: Optional callback invoked before each retry.
        on_exhausted: Optional callback invoked when all retries are
            exhausted.
    """

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
    """Aggregated statistics for a :class:`RetryPolicy`.

    Attributes:
        total_attempts: Total function invocations.
        successful_attempts: Invocations that returned without error.
        failed_attempts: Invocations that raised an exception.
        retried_calls: Calls that required at least one retry.
        exhausted_calls: Calls that exhausted all retry attempts.
        total_retry_delay_ms: Cumulative delay (ms) spent waiting
            between retries.
    """

    total_attempts: int = 0
    successful_attempts: int = 0
    failed_attempts: int = 0
    retried_calls: int = 0
    exhausted_calls: int = 0
    total_retry_delay_ms: int = 0


@dataclass
class RetryResult(Generic[T]):
    """Result of a successful retry operation.

    Attributes:
        result: The value returned by the async function.
        attempts: Number of attempts required (1 = first try succeeded).
        total_delay_ms: Total time spent waiting between retries.
    """

    result: T
    attempts: int
    total_delay_ms: int


class RetryExhaustedError(Exception):
    """Raised when all retry attempts have been exhausted.

    Attributes:
        last_error: The exception from the final attempt.
        attempts: Total number of attempts made.
        total_delay_ms: Cumulative delay between retries.
    """

    def __init__(
        self, message: str, last_error: Exception, attempts: int, total_delay_ms: int
    ):
        super().__init__(message)
        self.last_error = last_error
        self.attempts = attempts
        self.total_delay_ms = total_delay_ms


class RetryPolicy:
    """Retry policy with configurable backoff strategies.

    Wraps async functions and transparently retries on transient
    failures according to the configured :class:`RetryConfig`.

    Example:
        >>> policy = RetryPolicy()
        >>> result = await policy.execute(fetch_data)
    """

    def __init__(self, config: Optional[RetryConfig] = None):
        """Initialize the retry policy.

        Args:
            config: Optional configuration. Defaults to
                :class:`RetryConfig` with exponential jitter backoff.
        """
        self._config = config or RetryConfig()
        self._stats = RetryStats()

    async def execute(self, func: Callable[[], Awaitable[T]]) -> RetryResult[T]:
        """Execute an async function with retry logic.

        Args:
            func: A zero-argument async callable.

        Returns:
            A :class:`RetryResult` containing the function's return
            value and retry metadata.

        Raises:
            RetryExhaustedError: If all retry attempts fail or the
                error is not retryable.
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

                is_retryable = (
                    self._config.is_retryable(error, attempt)
                    if self._config.is_retryable
                    else self._is_retryable_default(error)
                )

                if attempt >= self._config.max_attempts or not is_retryable:
                    break

                delay = self._calculate_delay(attempt)
                total_delay_ms += delay
                self._stats.total_retry_delay_ms += delay

                if self._config.on_retry:
                    self._config.on_retry(error, attempt, delay)

                await asyncio.sleep(delay / 1000)

        assert last_error is not None, "last_error should not be None after loop"

        self._stats.exhausted_calls += 1
        if self._config.on_exhausted:
            self._config.on_exhausted(last_error, attempt)

        raise RetryExhaustedError(
            f"All {self._config.max_attempts} retry attempts exhausted",
            last_error,
            attempt,
            total_delay_ms,
        )

    async def execute_safe(
        self, func: Callable[[], Awaitable[T]]
    ) -> Optional[RetryResult[T]]:
        """Execute with retry logic but return ``None`` instead of raising.

        Args:
            func: A zero-argument async callable.

        Returns:
            A :class:`RetryResult` on success, or ``None`` if all
            retries are exhausted.
        """
        try:
            return await self.execute(func)
        except RetryExhaustedError:
            return None

    def get_stats(self) -> RetryStats:
        """Return a snapshot of the retry statistics.

        Returns:
            A copy of the current :class:`RetryStats`.
        """
        return RetryStats(
            total_attempts=self._stats.total_attempts,
            successful_attempts=self._stats.successful_attempts,
            failed_attempts=self._stats.failed_attempts,
            retried_calls=self._stats.retried_calls,
            exhausted_calls=self._stats.exhausted_calls,
            total_retry_delay_ms=self._stats.total_retry_delay_ms,
        )

    def reset_stats(self) -> None:
        """Reset all statistics counters to zero."""
        self._stats = RetryStats()

    def _calculate_delay(self, attempt: int) -> int:
        """Calculate the delay in milliseconds for a given attempt.

        Args:
            attempt: The 1-based attempt number.

        Returns:
            Delay in milliseconds, capped at ``max_delay_ms``.
        """
        delay = 0

        if self._config.backoff == BackoffStrategy.FIXED:
            delay = self._config.base_delay_ms
        elif self._config.backoff == BackoffStrategy.LINEAR:
            delay = self._config.base_delay_ms * attempt
        elif self._config.backoff == BackoffStrategy.EXPONENTIAL:
            delay = self._config.base_delay_ms * (
                self._config.multiplier ** (attempt - 1)
            )
        elif self._config.backoff == BackoffStrategy.EXPONENTIAL_JITTER:
            base = self._config.base_delay_ms * (
                self._config.multiplier ** (attempt - 1)
            )
            delay = self._add_jitter(base)

        return min(int(delay), self._config.max_delay_ms)

    def _add_jitter(self, delay: float) -> int:
        """Add random jitter to a delay value.

        Args:
            delay: The base delay in milliseconds.

        Returns:
            The jittered delay as an integer.
        """
        jitter = delay * self._config.jitter_factor
        return int(delay + random.uniform(-jitter, jitter))

    def _is_retryable_default(self, error: Exception) -> bool:
        """Determine whether an error is transient and retryable.

        Matches common network-related substrings in the error message.

        Args:
            error: The exception to evaluate.

        Returns:
            ``True`` if the error appears to be transient.
        """
        transient_messages = [
            "ECONNRESET",
            "ETIMEDOUT",
            "ENOTFOUND",
            "ECONNREFUSED",
            "network",
            "timeout",
            "unavailable",
            "temporary",
        ]
        message = str(error).lower()
        return any(m.lower() in message for m in transient_messages)


# ============================================
# Predefined Retry Policies
# ============================================


def network_retry_policy(**overrides) -> RetryPolicy:
    """Create a retry policy tuned for transient network errors.

    Uses exponential jitter backoff with sensible defaults for HTTP
    and RPC calls.

    Args:
        **overrides: Keyword arguments forwarded to
            :class:`RetryConfig` (e.g. ``max_attempts=5``).

    Returns:
        A configured :class:`RetryPolicy`.

    Example:
        >>> policy = network_retry_policy(max_attempts=5)
    """
    return RetryPolicy(
        RetryConfig(
            max_attempts=overrides.get("max_attempts", 3),
            backoff=BackoffStrategy.EXPONENTIAL_JITTER,
            base_delay_ms=overrides.get("base_delay_ms", 100),
            max_delay_ms=overrides.get("max_delay_ms", 5000),
        )
    )


def database_retry_policy(**overrides) -> RetryPolicy:
    """Create a retry policy tuned for database operations.

    Uses exponential backoff (no jitter) with short base delays.

    Args:
        **overrides: Keyword arguments forwarded to
            :class:`RetryConfig`.

    Returns:
        A configured :class:`RetryPolicy`.
    """
    return RetryPolicy(
        RetryConfig(
            max_attempts=overrides.get("max_attempts", 5),
            backoff=BackoffStrategy.EXPONENTIAL,
            base_delay_ms=overrides.get("base_delay_ms", 50),
            max_delay_ms=overrides.get("max_delay_ms", 2000),
            multiplier=overrides.get("multiplier", 2.0),
        )
    )


def aggressive_retry_policy(**overrides) -> RetryPolicy:
    """Create an aggressive retry policy with many attempts and short delays.

    Suitable for idempotent operations where rapid recovery is critical.

    Args:
        **overrides: Keyword arguments forwarded to
            :class:`RetryConfig`.

    Returns:
        A configured :class:`RetryPolicy`.
    """
    return RetryPolicy(
        RetryConfig(
            max_attempts=overrides.get("max_attempts", 10),
            backoff=BackoffStrategy.EXPONENTIAL_JITTER,
            base_delay_ms=overrides.get("base_delay_ms", 10),
            max_delay_ms=overrides.get("max_delay_ms", 1000),
            multiplier=overrides.get("multiplier", 1.5),
            jitter_factor=overrides.get("jitter_factor", 0.2),
        )
    )


def conservative_retry_policy(**overrides) -> RetryPolicy:
    """Create a conservative retry policy with few attempts and long delays.

    Suitable for non-idempotent operations or rate-limited APIs.

    Args:
        **overrides: Keyword arguments forwarded to
            :class:`RetryConfig`.

    Returns:
        A configured :class:`RetryPolicy`.
    """
    return RetryPolicy(
        RetryConfig(
            max_attempts=overrides.get("max_attempts", 2),
            backoff=BackoffStrategy.EXPONENTIAL,
            base_delay_ms=overrides.get("base_delay_ms", 1000),
            max_delay_ms=overrides.get("max_delay_ms", 10000),
            multiplier=overrides.get("multiplier", 3.0),
        )
    )
