"""
Bulkhead Pattern Implementation

Provides resource isolation by limiting concurrent calls.
Prevents one failing component from taking down the entire system.
"""

from __future__ import annotations
from dataclasses import dataclass
from typing import Optional, Dict, Any, Callable
import asyncio


@dataclass
class BulkheadConfig:
    """Configuration for bulkhead."""
    max_concurrent: int = 10
    max_queued: int = 100
    timeout_ms: int = 0  # 0 = no timeout for queued items


@dataclass
class BulkheadStats:
    """Statistics for bulkhead."""
    active: int = 0
    queued: int = 0
    max_concurrent: int = 0
    max_queued: int = 0
    total_accepted: int = 0
    total_rejected: int = 0
    total_timeout: int = 0


class BulkheadRejectedError(Exception):
    """Raised when bulkhead rejects a call."""
    pass


class BulkheadTimeoutError(Exception):
    """Raised when bulkhead call times out while queued."""
    pass


class Bulkhead:
    """Bulkhead pattern for resource isolation using semaphores."""
    
    def __init__(self, config: Optional[BulkheadConfig] = None):
        self._config = config or BulkheadConfig()
        self._semaphore = asyncio.Semaphore(self._config.max_concurrent)
        self._queue_semaphore = asyncio.Semaphore(self._config.max_queued)
        
        # Statistics
        self._total_accepted = 0
        self._total_rejected = 0
        self._total_timeout = 0
        self._active = 0
        self._queued = 0
        self._stats_lock = asyncio.Lock()
    
    @property
    def max_concurrent(self) -> int:
        return self._config.max_concurrent
    
    @property
    def max_queued(self) -> int:
        return self._config.max_queued
    
    async def execute(self, func: Callable[[], Any]) -> Any:
        """Execute a function with bulkhead protection."""
        # Try to get a queue slot first
        queue_acquired = self._queue_semaphore.locked() and self._queue_semaphore._value == 0
        
        if self._queue_semaphore._value == 0:
            # Queue is full
            async with self._stats_lock:
                self._total_rejected += 1
            raise BulkheadRejectedError(
                f"Bulkhead at capacity: max_concurrent={self._config.max_concurrent}, "
                f"max_queued={self._config.max_queued}"
            )
        
        # Acquire queue slot
        await self._queue_semaphore.acquire()
        
        try:
            async with self._stats_lock:
                self._queued += 1
            
            # Now try to acquire execution slot
            if self._config.timeout_ms > 0:
                try:
                    await asyncio.wait_for(
                        self._semaphore.acquire(),
                        timeout=self._config.timeout_ms / 1000
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
        """Get current statistics."""
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
        """Reset statistics."""
        self._total_accepted = 0
        self._total_rejected = 0
        self._total_timeout = 0


class BulkheadManager:
    """Manages multiple bulkheads by name."""
    
    def __init__(self, default_config: Optional[BulkheadConfig] = None):
        self._bulkheads: Dict[str, Bulkhead] = {}
        self._default_config = default_config or BulkheadConfig()
    
    def get(
        self, 
        name: str, 
        config: Optional[BulkheadConfig] = None
    ) -> Bulkhead:
        """Get or create a bulkhead by name."""
        if name not in self._bulkheads:
            merged_config = BulkheadConfig(
                max_concurrent=(
                    config.max_concurrent 
                    if config 
                    else self._default_config.max_concurrent
                ),
                max_queued=(
                    config.max_queued 
                    if config 
                    else self._default_config.max_queued
                ),
                timeout_ms=(
                    config.timeout_ms 
                    if config 
                    else self._default_config.timeout_ms
                ),
            )
            self._bulkheads[name] = Bulkhead(merged_config)
        return self._bulkheads[name]
    
    def get_all_stats(self) -> Dict[str, BulkheadStats]:
        """Get statistics for all bulkheads."""
        return {name: bulkhead.get_stats() for name, bulkhead in self._bulkheads.items()}
    
    def reset_all_stats(self) -> None:
        """Reset all statistics."""
        for bulkhead in self._bulkheads.values():
            bulkhead.reset_stats()


def api_bulkhead(max_concurrent: int = 50) -> Bulkhead:
    """Create a bulkhead for API calls."""
    return Bulkhead(BulkheadConfig(
        max_concurrent=max_concurrent,
        max_queued=100,
    ))


def database_bulkhead(max_concurrent: int = 10) -> Bulkhead:
    """Create a bulkhead for database operations."""
    return Bulkhead(BulkheadConfig(
        max_concurrent=max_concurrent,
        max_queued=50,
        timeout_ms=30000,
    ))


def strict_bulkhead(max_concurrent: int = 5) -> Bulkhead:
    """Create a strict bulkhead (no queuing)."""
    return Bulkhead(BulkheadConfig(
        max_concurrent=max_concurrent,
        max_queued=0,
    ))
