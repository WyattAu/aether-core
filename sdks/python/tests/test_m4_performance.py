"""
Tests for M4 Performance Modules

Comprehensive tests for:
- Zero-Copy Messaging (MemoryPool, PooledBuffer, RingBuffer)
- Batch Processing (BatchCollector)
- Partitioning (Partitioner, KeyExtractor)
"""

import os
import sys

import pytest

# Add the SDK to the path
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from aether_sdk.streaming import StreamEvent
from aether_sdk.streaming.batch import BatchCollector, BatchConfig
from aether_sdk.streaming.partition import (
    KeyExtractor,
    PartitionConfig,
    Partitioner,
    PartitionStrategy,
)
from aether_sdk.streaming.zero_copy import MemoryPool, RingBuffer

# ============================================
# Test Fixtures
# ============================================


@pytest.fixture
def memory_pool():
    """Create a test memory pool."""
    return MemoryPool(buffer_size=1024, initial_count=10)


@pytest.fixture
def ring_buffer():
    """Create a test ring buffer."""
    return RingBuffer(capacity=10)


@pytest.fixture
def batch_config():
    """Create a test batch config."""
    return BatchConfig(
        max_batch_size=100,
        max_wait_time_ms=50,
        max_bytes=1024 * 1024,
    )


@pytest.fixture
def partition_config():
    """Create a test partition config."""
    return PartitionConfig(
        strategy=PartitionStrategy.KEY,
        num_partitions=10,
        partition_key="user_id",
    )


# ============================================
# Zero-Copy Tests
# ============================================


class TestMemoryPool:
    """Tests for MemoryPool."""

    def test_memory_pool_initialization(self):
        """Test memory pool initialization."""
        pool = MemoryPool(buffer_size=1024, initial_count=10)
        stats = pool.get_stats()
        assert stats.total_buffers == 10
        assert stats.available_buffers == 10
        used = stats.total_buffers - stats.available_buffers
        assert used == 0

    def test_memory_pool_acquire_release(self, memory_pool):
        """Test acquiring and releasing buffers."""
        buffer = memory_pool.acquire()
        assert buffer is not None
        stats = memory_pool.get_stats()
        assert stats.available_buffers == 9
        used = stats.total_buffers - stats.available_buffers
        assert used == 1

        memory_pool.release(buffer)
        stats = memory_pool.get_stats()
        assert stats.available_buffers == 10
        used = stats.total_buffers - stats.available_buffers
        assert used == 0

    def test_memory_pool_exhaustion(self):
        """Test pool exhaustion raises MemoryError."""
        pool = MemoryPool(buffer_size=1024, initial_count=2, max_count=2)

        buffer1 = pool.acquire()
        pool.acquire()

        # Pool should be exhausted - acquire raises MemoryError
        with pytest.raises(MemoryError):
            pool.acquire()

        stats = pool.get_stats()
        assert stats.available_buffers == 0
        used = stats.total_buffers - stats.available_buffers
        assert used == 2

        pool.release(buffer1)
        stats = pool.get_stats()
        assert stats.available_buffers == 1
        used = stats.total_buffers - stats.available_buffers
        assert used == 1

    def test_memory_pool_write_read(self, memory_pool):
        """Test writing and reading data."""
        buffer = memory_pool.acquire()

        data = b"Hello, World!"
        written = buffer.write(data)
        assert written == len(data)
        assert buffer.position == len(data)

        read_data = buffer.read()
        assert read_data == data

        memory_pool.release(buffer)

    def test_memory_pool_clear(self, memory_pool):
        """Test clearing buffer."""
        buffer = memory_pool.acquire()

        data = b"Test data"
        buffer.write(data)
        assert buffer.position > 0

        buffer.clear()
        assert buffer.position == 0

        memory_pool.release(buffer)


class TestPooledBuffer:
    """Tests for PooledBuffer."""

    def test_pooled_buffer_write(self, memory_pool):
        """Test writing to pooled buffer."""
        buffer = memory_pool.acquire()

        data = b"Test data"
        written = buffer.write(data)
        assert written == len(data)
        assert buffer.position == len(data)

        memory_pool.release(buffer)

    def test_pooled_buffer_read(self, memory_pool):
        """Test reading from pooled buffer."""
        buffer = memory_pool.acquire()

        data = b"Test data"
        buffer.write(data)

        read_data = buffer.read()
        assert read_data == data

        memory_pool.release(buffer)


class TestRingBuffer:
    """Tests for RingBuffer."""

    def test_ring_buffer_write_read(self, ring_buffer):
        """Test ring buffer write and read."""
        for i in range(5):
            data = f"item-{i}"
            result = ring_buffer.write(data.encode())
            assert result is True

        for i in range(5):
            item = ring_buffer.read()
            assert bytes(item) == f"item-{i}".encode()

    def test_ring_buffer_overwrite(self, ring_buffer):
        """Test ring buffer returns False when full."""
        for i in range(10):
            data = f"item-{i}"
            result = ring_buffer.write(data.encode())
            assert result is True

        # Buffer is now full, write should return False
        result = ring_buffer.write(b"new-item")
        assert result is False

    def test_ring_buffer_empty(self, ring_buffer):
        """Test empty ring buffer."""
        item = ring_buffer.read()
        assert item is None

    def test_ring_buffer_properties(self, ring_buffer):
        """Test ring buffer property tracking."""
        assert ring_buffer.is_empty is True
        assert ring_buffer.available == 10

        ring_buffer.write(b"data")
        assert ring_buffer.is_empty is False
        assert ring_buffer.available == 9

        for i in range(5):
            ring_buffer.write(f"data-{i}".encode())

        assert ring_buffer.available == 4


# ============================================
# Batch Processing Tests
# ============================================


class TestBatchCollector:
    """Tests for BatchCollector."""

    def test_batch_collector_add(self, batch_config):
        """Test adding items to collector."""
        collector = BatchCollector[int](batch_config)

        for i in range(5):
            result = collector.add(i, size_bytes=8)
            assert result is None

        assert collector.current_size == 5

    def test_batch_collector_flush_on_full(self, batch_config):
        """Test flush when batch is full."""
        config = BatchConfig(max_batch_size=3, max_wait_time_ms=100)
        collector = BatchCollector[int](config)

        result = None
        for i in range(3):
            result = collector.add(i, size_bytes=8)

        assert result is not None
        assert len(result.items) == 3
        assert collector.is_empty()

    def test_batch_collector_manual_flush(self, batch_config):
        """Test manual flush."""
        collector = BatchCollector[int](batch_config)

        for i in range(5):
            collector.add(i, size_bytes=8)

        result = collector.flush()
        assert result is not None
        assert len(result.items) == 5
        assert collector.is_empty()

    def test_batch_collector_current_properties(self, batch_config):
        """Test current properties."""
        collector = BatchCollector[int](batch_config)

        assert collector.current_size == 0
        assert collector.current_bytes == 0
        assert collector.is_empty()

        collector.add(1, size_bytes=10)
        collector.add(2, size_bytes=20)

        assert collector.current_size == 2
        assert collector.current_bytes == 30


# ============================================
# Partitioning Tests
# ============================================


class TestPartitioner:
    """Tests for Partitioner."""

    def test_partitioner_key_strategy(self, partition_config):
        """Test key-based partitioning."""
        partitioner = Partitioner[dict, str](partition_config)

        event1 = StreamEvent.create(
            key="key1",
            value={"user_id": "user-a"},
        )
        event2 = StreamEvent.create(
            key="key2",
            value={"user_id": "user-b"},
        )

        p1 = partitioner.partition(event1)
        p2 = partitioner.partition(event2)

        assert p1 == partitioner.partition(event1)
        assert p2 == partitioner.partition(event2)

        assert isinstance(p1, int)
        assert isinstance(p2, int)
        assert 0 <= p1 < 10
        assert 0 <= p2 < 10

    def test_partitioner_distribution_stats(self, partition_config):
        """Test partition distribution statistics."""
        partitioner = Partitioner[dict, str](partition_config)

        for i in range(100):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"user_id": f"user-{i % 10}"},
            )
            partitioner.partition(event)

        stats = partitioner.get_distribution_stats()
        assert stats["total_events"] == 100
        assert stats["num_partitions"] == 10

        assert "distribution" in stats
        assert "skew" in stats


class TestKeyExtractor:
    """Tests for KeyExtractor."""

    def test_key_extractor_from_field(self):
        """Test key extractor from field name."""
        extractor = KeyExtractor[dict, str].from_field("user_id")

        value = {"user_id": "user-123", "data": "test"}
        key = extractor(value)
        assert key == "user-123"

    def test_key_extractor_composite(self):
        """Test composite key extractor."""
        extractor1 = KeyExtractor.from_field("user_id")
        extractor2 = KeyExtractor.from_field("region")

        # composite always returns int (hash of tuple)
        composite = KeyExtractor.composite(extractor1, extractor2)

        value = {"user_id": "user-123", "region": "us-west"}
        key = composite(value)
        assert isinstance(key, int)


# Run all tests
if __name__ == "__main__":
    pytest.main([__file__, "-v"])
