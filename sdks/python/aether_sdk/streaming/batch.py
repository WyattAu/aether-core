"""
Batch Processing Optimization

Provides efficient batch operations for stream processing including
batch collection, aggregation, and and emission.

Example:
    from aether_sdk.streaming import BatchProcessor, BatchConfig

    # Configure batch processing
    config = BatchConfig(
        max_batch_size=1000,
        max_wait_time_ms=100,
        max_bytes=1024 * 1024,  # 1MB
    )

    # Create batch processor
    processor = BatchProcessor(config)

    async for batch in processor.batches():
        # Process batch of events
        results = process_batch(batch)
        await emit_results(results)
"""

from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Awaitable, Callable, Generic, List, Optional, TypeVar

from .types import StreamEvent

T = TypeVar("T")
R = TypeVar("R")


@dataclass
class BatchConfig:
    """Configuration for batch processing."""

    max_batch_size: int = 1000
    max_wait_time_ms: int = 100
    max_bytes: int = 1024 * 1024  # 1MB
    timeout_on_full: bool = True
    partial_on_timeout: bool = True
    partial_on_shutdown: bool = True
    parallel: bool = False
    max_parallel_batches: int = 10
    batch_timeout_ms: int = 1000
    retry_on_failure: bool = True
    retry_delay_ms: int = 100
    retry_backoff: float = 2.0
    enable_async: bool = True
    adaptive_batching: bool = False
    batch_timeout_factor: float = 1.5
    max_concurrency: int = 4
    processor_factory: Optional[Callable[[], Any]] = None
    aggregator_factory: Optional[Callable[[], Any]] = None
    emitter_factory: Optional[Callable[[], Awaitable[None]]] = None
    on_failure: Optional[Callable[[Exception], Awaitable[None]]] = None
    on_batch_complete: Optional[Callable[[Any], Awaitable[None]]] = None
    error_handler: Optional[Callable[[Exception], None]] = None
    stats_callback: Optional[Callable[[Any], None]] = None


@dataclass
class BatchResult(Generic[T]):
    """Result of batch processing."""

    items: List[T]
    size_bytes: int = 0
    processing_time_ms: float = 0.0
    batch_id: str = ""
    timestamp: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    aggregated: Optional[Any] = None
    aggregation_key: Optional[str] = None
    checksum: Optional[str] = None


@dataclass
class BatchStats:
    """Statistics for batch processing."""

    total_items: int = 0
    total_batches: int = 0
    total_bytes: int = 0
    total_processing_time_ms: float = 0.0
    min_processing_time_ms: float = float("inf")
    max_processing_time_ms: float = 0.0
    avg_batch_size: float = 0.0
    failed_batches: int = 0
    start_time: Optional[float] = None
    end_time: Optional[float] = None


class BatchCollector(Generic[T]):
    """
    Collects items into batches based on size, time, or byte limits.

    Example:
        collector = BatchCollector[str](BatchConfig(max_batch_size=100))

        for item in items:
            batch = collector.add(item)
            if batch:
                await process_batch(batch)

        # Get any remaining items
        remaining = collector.flush()
    """

    def __init__(self, config: BatchConfig):
        self.config = config
        self._items: List[T] = []
        self._current_bytes = 0
        self._batch_start_time: Optional[float] = None
        self._batch_count = 0

    def add(self, item: T, size_bytes: int = 0) -> Optional[BatchResult[T]]:
        """Add an item to current batch."""
        # Initialize batch timing
        if not self._batch_start_time:
            self._batch_start_time = time.monotonic()

        # Add item
        self._items.append(item)
        self._current_bytes += size_bytes

        # Check if batch is complete
        if self._should_flush():
            return self.flush()

        return None

    def add_many(self, items: List[T], size_bytes: int = 0) -> Optional[BatchResult[T]]:
        """Add multiple items at once."""
        for i in range(len(items)):
            item_size = size_bytes // len(items) if items else 0
            result = self.add(items[i], item_size)
            if result:
                return result
        return None

    def _should_flush(self) -> bool:
        """Check if batch should be flushed."""
        if len(self._items) >= self.config.max_batch_size:
            return True
        if self._current_bytes >= self.config.max_bytes:
            return True
        if self._batch_start_time:
            elapsed_ms = (time.monotonic() - self._batch_start_time) * 1000
            if elapsed_ms >= self.config.max_wait_time_ms:
                return self.config.timeout_on_full
        return False

    def flush(self) -> Optional[BatchResult[T]]:
        """Flush the current batch."""
        if not self._items:
            return None

        processing_time = 0
        if self._batch_start_time:
            processing_time = (time.monotonic() - self._batch_start_time) * 1000

        self._batch_count += 1
        batch_id = f"batch-{self._batch_count}"

        result = BatchResult(
            items=self._items,
            size_bytes=self._current_bytes,
            processing_time_ms=processing_time,
            batch_id=batch_id,
            timestamp=datetime.now(timezone.utc),
        )

        # Reset
        self._items = []
        self._current_bytes = 0
        self._batch_start_time = None

        return result

    @property
    def current_size(self) -> int:
        """Get current batch size."""
        return len(self._items)

    @property
    def current_bytes(self) -> int:
        """Get current batch byte size."""
        return self._current_bytes

    def is_empty(self) -> bool:
        """Check if batch is empty."""
        return len(self._items) == 0


class BatchAggregator(Generic[T, R]):
    """
    Aggregates batch items into a single result.

    Example:
        class SumAggregator(BatchAggregator[int, float]):
            def aggregate(self, batch: List[int]) -> float:
                return sum(batch) / len(batch)
    """

    def __init__(
        self,
        aggregate_func: Optional[Callable[[List[T]], R]] = None,
        key_extractor: Optional[Callable[[T], str]] = None,
    ):
        self.aggregate_func = aggregate_func
        self.key_extractor = key_extractor
        self._batch_count = 0
        self._total_events = 0
        self._processing_time = 0.0

    def aggregate(self, batch: List[T], key: Optional[str] = None) -> R:
        """Aggregate a batch of items."""
        if not batch:
            raise ValueError("batch cannot be empty")

        start_time = time.monotonic()

        try:
            # Apply aggregation function
            if self.aggregate_func:
                result = self.aggregate_func(batch)
            else:
                # Default: return last item
                result = batch[-1]  # type: ignore

            # Update stats
            self._batch_count += 1
            self._total_events += len(batch)
            self._processing_time += time.monotonic() - start_time

            return result  # type: ignore

        except Exception:
            self._processing_time += time.monotonic() - start_time
            raise


class BatchEmitter(Generic[T]):
    """
    Emits batch results to downstream consumers.

    Example:
        class MyEmitter(BatchEmitter[dict]):
            def __init__(self, downstream: Callable):
                self._downstream = downstream

            async def emit(self, batch: BatchResult[dict]) -> None:
                await self._downstream(batch)
    """

    def __init__(self):
        self._handlers: List[Callable[[BatchResult[T]], Awaitable[None]]] = []

    def add_handler(self, handler: Callable[[BatchResult[T]], Awaitable[None]]) -> None:
        """Add a handler for batch results."""
        self._handlers.append(handler)

    async def emit(self, batch: BatchResult[T]) -> None:
        """Emit batch to all handlers."""
        for handler in self._handlers:
            await handler(batch)


class BatchProcessor(Generic[T]):
    """
    Processes events in batches with configurable size and timing.

    Supports both sync and async processing patterns.
    """

    def __init__(self, config: BatchConfig):
        self.config = config
        self._collector = BatchCollector[T](config)
        self._aggregator: Optional[BatchAggregator[T, Any]] = None
        self._emitter: Optional[BatchEmitter[T]] = None
        self._queue: asyncio.Queue[BatchResult[T]] = asyncio.Queue()
        self._running = False
        self._lock = asyncio.Lock()
        self._stats = BatchStats()

        # Initialize aggregator if provided
        if self.config.aggregator_factory:
            self._aggregator = self.config.aggregator_factory()

        # Initialize emitter if provided
        if self.config.emitter_factory:
            self._emitter = self.config.emitter_factory()

    async def start(self) -> None:
        """Start the batch processor."""
        async with self._lock:
            self._running = True
            self._stats.start_time = time.monotonic()

    async def stop(self) -> None:
        """Stop the batch processor."""
        async with self._lock:
            self._running = False

            # Process remaining batches
            while not self._queue.empty():
                batch = await self._queue.get()
                await self._process_batch(batch)

            # Flush collector
            remaining = self._collector.flush()
            if remaining:
                await self._process_batch(remaining)

            self._stats.end_time = time.monotonic()

    async def add(self, event: StreamEvent[T]) -> bool:
        """Add an event to batch processor."""
        if not self._running:
            raise RuntimeError("Batch processor not running")

        async with self._lock:
            # Get event size
            size_bytes = len(event.value) if hasattr(event.value, "__len__") else 0

            # Try to add to collector
            batch_result = self._collector.add(event.value, size_bytes)

            if batch_result:
                await self._process_batch(batch_result)
                return True

            return False

    async def _process_batch(self, batch: BatchResult[T]) -> None:
        """Process a batch."""
        start_time = time.monotonic()

        try:
            # Aggregate if configured
            aggregated = None
            if self._aggregator:
                aggregated = self._aggregator.aggregate(batch.items)

            # Create result
            result = BatchResult(
                items=aggregated if aggregated is not None else batch.items,
                size_bytes=batch.size_bytes,
                processing_time_ms=batch.processing_time_ms,
                batch_id=batch.batch_id,
                timestamp=batch.timestamp,
            )

            # Emit if configured
            if self._emitter:
                await self._emitter.emit(result)

            # Update stats
            self._stats.total_batches += 1
            self._stats.total_items += len(batch.items)
            self._stats.total_bytes += batch.size_bytes

            processing_time = (time.monotonic() - start_time) * 1000
            self._stats.total_processing_time_ms += processing_time
            self._stats.min_processing_time_ms = min(
                self._stats.min_processing_time_ms, processing_time
            )
            self._stats.max_processing_time_ms = max(
                self._stats.max_processing_time_ms, processing_time
            )

            # Callback
            if self.config.on_batch_complete:
                await self.config.on_batch_complete(result)

        except Exception as e:
            self._stats.failed_batches += 1
            if self.config.on_failure:
                await self.config.on_failure(e)
            elif self.config.error_handler:
                self.config.error_handler(e)
            else:
                raise

    def get_stats(self) -> BatchStats:
        """Get current statistics."""
        return BatchStats(
            total_items=self._stats.total_items,
            total_batches=self._stats.total_batches,
            total_bytes=self._stats.total_bytes,
            total_processing_time_ms=self._stats.total_processing_time_ms,
            min_processing_time_ms=self._stats.min_processing_time_ms,
            max_processing_time_ms=self._stats.max_processing_time_ms,
            failed_batches=self._stats.failed_batches,
            start_time=self._stats.start_time,
            end_time=self._stats.end_time,
        )


__all__ = [
    "BatchConfig",
    "BatchResult",
    "BatchStats",
    "BatchCollector",
    "BatchAggregator",
    "BatchEmitter",
    "BatchProcessor",
]
