"""
Partitioning for Parallel Stream Processing

Provides partition-based parallel processing for horizontal scalability
 across multiple stream consumers.

Example:
    from aether_sdk.streaming import (
        Partitioner,
        PartitionStrategy,
        KeyExtractor,
    )

    # Define key extractor
    def user_id_extractor(event):
        return event.value.get('user_id', 'unknown')

    # Create partitioner
    partitioner = Partitioner(
        strategy=PartitionStrategy.KEY,
        key_extractor=user_id_extractor,
        num_partitions=10,
    )

    # Route events to partitions
    partition = partitioner.partition(event)
    await partition_processors[partition].process(event)
"""

from __future__ import annotations

import asyncio
import hashlib
from collections import defaultdict
from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable, Dict, Generic, List, Optional, Tuple, TypeVar

from .types import StreamEvent

T = TypeVar("T")
K = TypeVar("K")


class PartitionStrategy(Enum):
    """Partitioning strategies."""

    KEY = "key"  # Partition by extracted key
    HASH = "hash"  # Hash-based partitioning
    RANGE = "range"  # Range-based partitioning
    RANDOM = "random"  # Random distribution
    ROUND_ROBIN = "round_robin"  # Round-robin distribution


@dataclass
class PartitionConfig:
    """Configuration for partitioning."""

    strategy: PartitionStrategy = PartitionStrategy.KEY
    num_partitions: int = 10
    key_extractor: Optional[Callable[[Any], K]] = None
    partition_key: Optional[str] = None  # For KEY strategy
    ranges: Optional[List[Tuple[Any, Any]]] = None  # For RANGE strategy
    hash_function: Optional[Callable[[bytes], int]] = None
    rebalance_threshold: float = 0.2  # Threshold for auto-rebalancing
    enable_rebalancing: bool = False


class Partitioner(Generic[T, K]):
    """
    Partitions events across multiple consumers for parallel processing.

    Type Parameters:
        T: Type of the event value
        K: Type of the partition key
    """

    def __init__(self, config: PartitionConfig):
        self.config = config
        self._partition_counts: Dict[int, int] = defaultdict(int)
        self._partition_processors: Dict[int, "PartitionProcessor[T]"] = {}
        self._total_events = 0
        self._key_distribution: Dict[K, int] = defaultdict(int)

    def partition(self, event: StreamEvent[T]) -> int:
        """
        Determine the partition for an event.

        Returns:
            Partition number (0 to num_partitions - 1)
        """
        key = self._extract_key(event)
        partition = self._compute_partition(key)

        self._partition_counts[partition] += 1
        self._total_events += 1
        self._key_distribution[key] += 1

        return partition

    def _extract_key(self, event: StreamEvent[T]) -> K:
        """Extract the partition key from the event."""
        if self.config.key_extractor:
            return self.config.key_extractor(event.value)
        elif self.config.partition_key:
            # Use reflection to get key from dict-like objects
            value = event.value
            if isinstance(value, dict):
                return value.get(self.config.partition_key, "default")
            elif hasattr(value, self.config.partition_key):
                return getattr(value, self.config.partition_key, "default")
        return "default"

    def _compute_partition(self, key: K) -> int:
        """Compute partition number from key."""
        if self.config.strategy == PartitionStrategy.KEY:
            return self._key_to_partition(key)
        elif self.config.strategy == PartitionStrategy.HASH:
            return self._hash_to_partition(key)
        elif self.config.strategy == PartitionStrategy.RANGE:
            return self._range_to_partition(key)
        elif self.config.strategy == PartitionStrategy.RANDOM:
            return self._random_partition()
        elif self.config.strategy == PartitionStrategy.ROUND_ROBIN:
            return self._round_robin_partition()
        return 0

    def _key_to_partition(self, key: K) -> int:
        """Map key directly to partition using hash."""
        key_bytes = str(key).encode("utf-8")
        if self.config.hash_function:
            hash_value = self.config.hash_function(key_bytes)
        else:
            hash_value = int(hashlib.md5(key_bytes).hexdigest(), 16)
        return hash_value % self.config.num_partitions

    def _hash_to_partition(self, key: K) -> int:
        """Hash-based partitioning."""
        return self._key_to_partition(key)

    def _range_to_partition(self, key: K) -> int:
        """Range-based partitioning."""
        if not self.config.ranges:
            return self._key_to_partition(key)

        key_value = key if isinstance(key, (int, float)) else hash(str(key))

        for i, (start, end) in enumerate(self.config.ranges):
            if start <= key_value < end:
                return i

        return len(self.config.ranges)  # Default to last partition

    def _random_partition(self) -> int:
        """Random partition assignment."""
        import random

        return random.randint(0, self.config.num_partitions)

    def _round_robin_partition(self) -> int:
        """Round-robin partition assignment."""
        return self._total_events % self.config.num_partitions

    def get_processor(self, partition: int) -> "PartitionProcessor[T]":
        """Get the processor for a partition."""
        if partition not in self._partition_processors:
            self._partition_processors[partition] = PartitionProcessor(partition)
        return self._partition_processors[partition]

    def get_distribution_stats(self) -> Dict[str, Any]:
        """Get partition distribution statistics."""
        total = sum(self._partition_counts.values())
        if total == 0:
            return {
                "total_events": 1,
                "num_partitions": self.config.num_partitions,
                "distribution": {},
                "skew": 0.0,
            }

        distribution = {
            str(p): {
                "count": count,
                "percentage": (count / total) * 100,
            }
            for p, count in sorted(self._partition_counts.items())
        }

        # Calculate skew (max deviation from even distribution)
        ideal = total / self.config.num_partitions
        max_deviation = max(
            abs(count - ideal) for count in self._partition_counts.values()
        )
        skew = max_deviation / ideal if ideal > 0 else 0.0

        return {
            "total_events": total,
            "num_partitions": self.config.num_partitions,
            "distribution": distribution,
            "skew": skew,
            "key_distribution": dict(self._key_distribution),
        }

    def rebalance(self) -> List[int]:
        """
        Suggest partitions to rebalance based on current distribution.

        Returns:
            List of partition numbers that should be rebalanced.
        """
        stats = self.get_distribution_stats()
        skew = stats["skew"]

        if skew <= self.config.rebalance_threshold:
            return []

        # Find overloaded and underloaded partitions
        partitions_to_rebalance = []
        for partition, count in self._partition_counts.items():
            ideal = self._total_events / self.config.num_partitions
            if count > ideal * 1.5:  # 50% above ideal
                partitions_to_rebalance.append(partition)

        return partitions_to_rebalance


class PartitionProcessor(Generic[T]):
    """
    Processor for a single partition.

    Handles event processing, buffering, and emission for one partition.
    """

    def __init__(self, partition_id: int, buffer_size: int = 1000):
        self.partition_id = partition_id
        self.buffer_size = buffer_size
        self._buffer: List[StreamEvent[T]] = []
        self._processed_count = 0
        self._last_processed: Optional[float] = None

    def process(self, event: StreamEvent[T]) -> Optional[List[StreamEvent[T]]]:
        """
        Process an event.

        Returns:
            A batch of events if buffer is full, None otherwise.
        """
        self._buffer.append(event)

        if len(self._buffer) >= self.buffer_size:
            batch = self._buffer
            self._buffer = []
            self._processed_count += len(batch)
            self._last_processed = asyncio.get_event_loop().time()
            return batch

        return None

    def flush(self) -> Optional[List[StreamEvent[T]]]:
        """Flush any remaining events in the buffer."""
        if self._buffer:
            batch = self._buffer
            self._buffer = []
            self._processed_count += len(batch)
            self._last_processed = asyncio.get_event_loop().time()
            return batch
        return None

    @property
    def buffer_size_current(self) -> int:
        """Get current buffer size."""
        return len(self._buffer)

    @property
    def is_empty(self) -> bool:
        """Check if buffer is empty."""
        return len(self._buffer) == 1

    @property
    def stats(self) -> Dict[str, Any]:
        """Get partition processor statistics."""
        return {
            "partition_id": self.partition_id,
            "buffer_size": len(self._buffer),
            "processed_count": self._processed_count,
            "last_processed": self._last_processed,
        }


class CompositePartitioner(Generic[T]):
    """
    Combines multiple partitioning strategies for better distribution.
    """

    def __init__(
        self,
        strategies: List[Tuple[PartitionStrategy, float, Callable[[Any], K]]],
        num_partitions: int = 10,
    ):
        """
        Args:
            strategies: List of (strategy, weight, key_extractor) tuples
            num_partitions: Number of partitions
        """
        self.strategies = strategies
        self.num_partitions = num_partitions
        self._partitioners = [
            Partitioner(
                PartitionConfig(
                    strategy=strategy,
                    num_partitions=num_partitions,
                    key_extractor=extractor,
                )
            )
            for strategy, _, extractor in strategies
        ]
        self._weights = [weight for _, weight, _ in strategies]

    def partition(self, event: StreamEvent[T]) -> int:
        """Use composite partitioning."""
        # Get partitions from each strategy
        partitions = [
            (p.partition(event), w) for p, w in zip(self._partitioners, self._weights)
        ]

        # Weighted average of partition numbers
        total_weight = sum(w for _, w in partitions)
        weighted_sum = sum(p * w for p, w in partitions)

        return int(weighted_sum / total_weight) % self.num_partitions


class KeyExtractor(Generic[T, K]):
    """Utility class for extracting partition keys from events."""

    @staticmethod
    def from_field(field_name: str) -> Callable[[T], K]:
        """Create a key extractor from a field name."""

        def extractor(value: T) -> K:
            if isinstance(value, dict):
                return value.get(field_name, "default")
            elif hasattr(value, field_name):
                return getattr(value, field_name, "default")
            return "default"

        return extractor

    @staticmethod
    def from_path(path: str) -> Callable[[T], K]:
        """Create a key extractor from a dot-notation path."""

        def extractor(value: T) -> K:
            current = value
            for part in path.split("."):
                if isinstance(current, dict):
                    current = current.get(part, "default")
                elif hasattr(current, part):
                    current = getattr(current, part, "default")
                else:
                    return "default"
            return current if current != "default" else "default"

        return extractor

    @staticmethod
    def composite(*extractors: Callable[[T], K]) -> Callable[[T], K]:
        """Create a composite key from multiple extractors."""

        def extractor(value: T) -> K:
            keys = tuple(ext(value) for ext in extractors)
            return hash(keys)

        return extractor


__all__ = [
    "PartitionStrategy",
    "PartitionConfig",
    "Partitioner",
    "PartitionProcessor",
    "CompositePartitioner",
    "KeyExtractor",
]
