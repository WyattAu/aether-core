"""
Bulkhead Pattern Implementation

Provides resource isolation by limiting concurrent calls.
Prevents one failing component from taking down the entire system.

Example:
    >>> from aether_sdk.resilience.bulkhead import Bulkhead, BulkheadConfig
    >>> bh = Bulkhead(BulkheadConfig(max_concurrent=10, max_queued=50))
    >>> result = await bh.execute(some_async_func)
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any, Callable, Dict, Optional


@dataclass
class BulkheadConfig:
    """Configuration for a :class:`Bulkhead`.

    Attributes:
        max_concurrent: Maximum number of concurrent executions.
        max_queued: Maximum number of queued callers waiting for a
            slot. Set to ``0`` to disable queuing.
        timeout_ms: Maximum time (ms) a queued call may wait for an
            execution slot. Set to ``0`` for no timeout.
    """

    max_concurrent: int = 10
    max_queued: int = 100
    timeout_ms: int = 0


@dataclass
class BulkheadStats:
    """Snapshot of a bulkhead's current statistics.

    Attributes:
        active: Number of currently executing calls.
        queued: Number of calls waiting in the queue.
        max_concurrent: Configured concurrency limit.
        max_queued: Configured queue capacity.
        total_accepted: Total calls that started executing.
        total_rejected: Total calls rejected due to capacity.
        total_timeout: Total calls that timed out while queued.
    """

    active: int = 0
    queued: int = 0
    max_concurrent: int = 0
    max_queued: int = 0
    total_accepted: int = 0
    total_rejected: int = 0
    total_timeout: int = 0


class BulkheadRejectedError(Exception):
    """Raised when a call is rejected because the bulkhead is at capacity.

    This occurs when both the concurrency slots and the queue are full.
    """

    pass


class BulkheadTimeoutError(Exception):
    """Raised when a queued call times out before an execution slot becomes available."""

    pass


class Bulkhead:
    """Bulkhead pattern for resource isolation using semaphores.

    Limits the number of concurrent executions to prevent resource
    exhaustion. Optionally queues excess callers until a slot becomes
    available.

    Example:
        >>> bh = Bulkhead(BulkheadConfig(max_concurrent=5))
        >>> try:
        ...     result = await bh.execute(my_func)
        ... except BulkheadRejectedError:
        ...     print("System at capacity")
    """

    def __init__(self, config: Optional[BulkheadConfig] = None):
        """Initialize the bulkhead.

        Args:
            config: Optional configuration. Defaults to
                :class:`BulkheadConfig`.
        """
        self._config = config or BulkheadConfig()
        self._semaphore = asyncio.Semaphore(self._config.max_concurrent)
        self._queue_semaphore = asyncio.Semaphore(self._config.max_queued)

        self._total_accepted = 0
        self._total_rejected = 0
        self._total_timeout = 0
        self._active = 0
        self._queued = 0
        self._stats_lock = asyncio.Lock()

    @property
    def max_concurrent(self) -> int:
        """Return the configured maximum concurrency."""
        return self._config.max_concurrent

    @property
    def max_queued(self) -> int:
        """Return the configured maximum queue size."""
        return self._config.max_queued

    async def execute(self, func: Callable[[], Any]) -> Any:
        """Execute a function with bulkhead protection.

        If the bulkhead is at capacity and queuing is disabled (or the
        queue is full), the call is rejected immediately.

        Args:
            func: A zero-argument async callable.

        Returns:
            The result of *func*.

        Raises:
            BulkheadRejectedError: If the bulkhead cannot accept
                another call.
            BulkheadTimeoutError: If a queued call times out while
                waiting for an execution slot.
        """
        if self._config.max_queued == 0:
            if self._semaphore.locked() and self._semaphore._value == 0:
                async with self._stats_lock:
                    self._total_rejected += 1
                raise BulkheadRejectedError(
                    f"Bulkhead at capacity: max_concurrent={self._config.max_concurrent}, "
                    f"max_queued={self._config.max_queued}"
                )

            await self._semaphore.acquire()
            try:
                async with self._stats_lock:
                    self._active += 1
                    self._total_accepted += 1

                return await func()
            finally:
                async with self._stats_lock:
                    self._active -= 1
                self._semaphore.release()

        if self._queue_semaphore._value == 0:
            async with self._stats_lock:
                self._total_rejected += 1
            raise BulkheadRejectedError(
                f"Bulkhead at capacity: max_concurrent={self._config.max_concurrent}, "
                f"max_queued={self._config.max_queued}"
            )

        await self._queue_semaphore.acquire()

        try:
            async with self._stats_lock:
                self._queued += 1

            if self._config.timeout_ms > 0:
                try:
                    await asyncio.wait_for(
                        self._semaphore.acquire(),
                        timeout=self._config.timeout_ms / 1000,
                    )
                except asyncio.TimeoutError:
                    async with self._stats_lock:
                        self._total_timeout += 1
                    raise BulkheadTimeoutError(
                        f"Bulkhead queued call timed out after {self._config.timeout_ms}ms"
                    )
            else:
                await self._semaphore.acquire()

            try:
                async with self._stats_lock:
                    self._queued -= 1
                    self._active += 1
                    self._total_accepted += 1

                return await func()
            finally:
                async with self._stats_lock:
                    self._active -= 1
                self._semaphore.release()
        finally:
            self._queue_semaphore.release()

    def get_stats(self) -> BulkheadStats:
        """Return a snapshot of the current bulkhead statistics.

        Returns:
            A :class:`BulkheadStats` dataclass.
        """
        return BulkheadStats(
            active=self._active,
            queued=self._queued,
            max_concurrent=self._config.max_concurrent,
            max_queued=self._config.max_queued,
            total_accepted=self._total_accepted,
            total_rejected=self._total_rejected,
            total_timeout=self._total_timeout,
        )

    def reset_stats(self) -> None:
        """Reset acceptance/rejection/timeout counters to zero."""
        self._total_accepted = 0
        self._total_rejected = 0
        self._total_timeout = 0


class BulkheadManager:
    """Registry for named :class:`Bulkhead` instances.

    Example:
        >>> mgr = BulkheadManager()
        >>> bh = mgr.get("api-calls")
    """

    def __init__(self, default_config: Optional[BulkheadConfig] = None):
        """Initialize the manager.

        Args:
            default_config: Default configuration applied to bulkheads
                that do not supply their own config.
        """
        self._bulkheads: Dict[str, Bulkhead] = {}
        self._default_config = default_config or BulkheadConfig()

    def get(self, name: str, config: Optional[BulkheadConfig] = None) -> Bulkhead:
        """Get or create a bulkhead by name.

        Args:
            name: Unique name for the bulkhead.
            config: Optional per-bulkhead configuration.

        Returns:
            The :class:`Bulkhead` instance for *name*.
        """
        if name not in self._bulkheads:
            merged_config = BulkheadConfig(
                max_concurrent=(
                    config.max_concurrent
                    if config
                    else self._default_config.max_concurrent
                ),
                max_queued=(
                    config.max_queued if config else self._default_config.max_queued
                ),
                timeout_ms=(
                    config.timeout_ms if config else self._default_config.timeout_ms
                ),
            )
            self._bulkheads[name] = Bulkhead(merged_config)
        return self._bulkheads[name]

    def get_all_stats(self) -> Dict[str, BulkheadStats]:
        """Return statistics for every registered bulkhead.

        Returns:
            A dict mapping bulkhead names to their :class:`BulkheadStats`.
        """
        return {
            name: bulkhead.get_stats() for name, bulkhead in self._bulkheads.items()
        }

    def reset_all_stats(self) -> None:
        """Reset statistics for every registered bulkhead."""
        for bulkhead in self._bulkheads.values():
            bulkhead.reset_stats()


def api_bulkhead(max_concurrent: int = 50) -> Bulkhead:
    """Create a bulkhead pre-configured for API calls.

    Args:
        max_concurrent: Maximum concurrent API calls (default 50).

    Returns:
        A :class:`Bulkhead` with a 100-slot queue.
    """
    return Bulkhead(
        BulkheadConfig(
            max_concurrent=max_concurrent,
            max_queued=100,
        )
    )


def database_bulkhead(max_concurrent: int = 10) -> Bulkhead:
    """Create a bulkhead pre-configured for database operations.

    Args:
        max_concurrent: Maximum concurrent DB operations (default 10).

    Returns:
        A :class:`Bulkhead` with a 50-slot queue and 30-second timeout.
    """
    return Bulkhead(
        BulkheadConfig(
            max_concurrent=max_concurrent,
            max_queued=50,
            timeout_ms=30000,
        )
    )


def strict_bulkhead(max_concurrent: int = 5) -> Bulkhead:
    """Create a strict bulkhead with no queuing.

    Calls are rejected immediately when all slots are occupied.

    Args:
        max_concurrent: Maximum concurrent operations (default 5).

    Returns:
        A :class:`Bulkhead` with ``max_queued=0``.
    """
    return Bulkhead(
        BulkheadConfig(
            max_concurrent=max_concurrent,
            max_queued=0,
        )
    )
