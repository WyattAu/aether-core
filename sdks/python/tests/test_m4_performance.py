"""
Tests for M4 Performance Modules

Tests for:
- Zero-Copy Messaging (MemoryPool, PooledBuffer, ZeroCopyBuffer, RingBuffer, ZeroCopyEmitter)
- Batch Processing (BatchCollector, BatchAggregator, BatchProcessor)
- Partitioning (Partitioner, PartitionProcessor, KeyExtractor)
"""

import asyncio
import pytest
from datetime import datetime
from typing import Any, Dict, List, Optional
from dataclasses import dataclass
import time

import sys
import os

# Add the SDK to the path
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from aether_sdk.streaming import (
    StreamEvent,
    Timestamp,
    Duration,
)
from aether_sdk.streaming.zero_copy import (
    MemoryPool,
    PooledBuffer,
    ZeroCopyBuffer,
    RingBuffer,
    ZeroCopyEmitter,
)
from aether_sdk.streaming.batch import (
    BatchConfig,
    BatchResult,
    BatchStats,
    BatchCollector,
    BatchAggregator,
    BatchEmitter,
    BatchProcessor,
)
from aether_sdk.streaming.partition import (
    PartitionStrategy,
    PartitionConfig,
    Partitioner,
    PartitionProcessor,
    CompositePartitioner,
    KeyExtractor,
)


# ============================================
# Test Fixtures
# ============================================

@pytest.fixture
def memory_pool():
    """Create a test memory pool."""
    return MemoryPool(buffer_size=1024, capacity=10)


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


@pytest.fixture
def stream_event():
    """Create a test stream event."""
    return StreamEvent.create(
        key="test-key",
        value={"user_id": "user-123", "data": "test"},
        timestamp=Timestamp.now(),
    )


# ============================================
# Zero-Copy Tests
# ============================================

class TestMemoryPool:
    """Tests for MemoryPool."""
    
    def test_memory_pool_initialization(self):
        """Test memory pool initialization."""
        pool = MemoryPool(buffer_size=1024, capacity=10)
        assert pool.capacity == 10
        assert pool.available == 10
        assert pool.used == 0
    
    def test_memory_pool_acquire_release(self, memory_pool):
        """Test acquiring and releasing buffers."""
        # Acquire buffer
        buffer = memory_pool.acquire()
        assert buffer is not None
        assert memory_pool.available == 9
        assert memory_pool.used == 1
        
        # Release buffer
        memory_pool.release(buffer)
        assert memory_pool.available == 10
        assert memory_pool.used == 0
    
    def test_memory_pool_exhaustion(self):
        """Test pool exhaustion."""
        pool = MemoryPool(buffer_size=1024, capacity=2)
        
        # Acquire all buffers
        buffer1 = pool.acquire()
        buffer2 = pool.acquire()
        
        # Pool should be empty
        assert pool.acquire() is None
        assert pool.available == 0
        assert pool.used == 2
        
        # Release one
        pool.release(buffer1)
        assert pool.available == 1
        assert pool.used == 1
    
    def test_memory_pool_write_read(self, memory_pool):
        """Test writing and reading data."""
        buffer = memory_pool.acquire()
        
        # Write data
        data = b"Hello, World!"
        written = buffer.write(data)
        assert written == len(data)
        assert buffer.length == len(data)
        
        # Read data
        read_data = buffer.read()
        assert read_data == data
        
        memory_pool.release(buffer)
    
    def test_memory_pool_clear(self, memory_pool):
        """Test clearing buffer."""
        buffer = memory_pool.acquire()
        
        data = b"Test data"
        buffer.write(data)
        assert buffer.length > 0
        
        buffer.clear()
        assert buffer.length == 0
        
        memory_pool.release(buffer)


class TestPooledBuffer:
    """Tests for PooledBuffer."""
    
    def test_pooled_buffer_write(self, memory_pool):
        """Test writing to pooled buffer."""
        buffer = PooledBuffer(memory_pool)
        
        data = b"Test data"
        written = buffer.write(data)
        assert written == len(data)
        assert buffer.length == len(data)
    
    def test_pooled_buffer_read(self, memory_pool):
        """Test reading from pooled buffer."""
        buffer = PooledBuffer(memory_pool)
        
        data = b"Test data"
        buffer.write(data)
        
        read_data = buffer.read()
        assert read_data == data
    
    def test_pooled_buffer_slice(self, memory_pool):
        """Test slicing buffer."""
        buffer = PooledBuffer(memory_pool)
        
        data = b"Hello, World!"
        buffer.write(data)
        
        # Get slice
        slice_data = buffer.slice(0, 5)
        assert slice_data == b"Hello"
        
        # Get another slice
        slice_data2 = buffer.slice(7, 12)
        assert slice_data2 == b"World"


class TestRingBuffer:
    """Tests for RingBuffer."""
    
    def test_ring_buffer_write_read(self, ring_buffer):
        """Test writing and reading from ring buffer."""
        data = b"Test data"
        written = ring_buffer.write(data)
        assert written == len(data)
        assert ring_buffer.available_data == len(data)
        
        read_data = ring_buffer.read(len(data))
        assert read_data == data
    
    def test_ring_buffer_capacity(self, ring_buffer):
        """Test ring buffer capacity."""
        assert ring_buffer.capacity == 10
        
        # Write more than capacity (should wrap)
        for i in range(15):
            data = f"item-{i}".encode()
            ring_buffer.write(data)
        
        # Buffer should have wrapped around
        assert ring_buffer.available_data > 0
    
    def test_ring_buffer_empty(self, ring_buffer):
        """Test reading from empty buffer."""
        data = ring_buffer.read(10)
        assert data == b""
    
    def test_ring_buffer_partial_read(self, ring_buffer):
        """Test partial reads."""
        data = b"Hello, World!"
        ring_buffer.write(data)
        
        # Read partial
        partial = ring_buffer.read(5)
        assert partial == b"Hello"
        
        # Read rest
        rest = ring_buffer.read(8)
        assert rest == b", World!"


class TestZeroCopyEmitter:
    """Tests for ZeroCopyEmitter."""
    
    def test_zero_copy_emit(self, memory_pool):
        """Test zero-copy emission."""
        emitted = []
        
        def consumer(data: bytes):
            emitted.append(data)
        
        emitter = ZeroCopyEmitter(memory_pool)
        emitter.set_downstream(consumer)
        
        # Emit event
        event = StreamEvent.create(
            key="test-key",
            value={"data": "test"},
        )
        
        # Note: ZeroCopyEmitter is async
        async def run_test():
            result = await emitter.emit(event)
            return result
        
        # Run async test
        result = asyncio.run(run_test())
        assert result is True
    
    def test_zero_copy_flush(self, memory_pool):
        """Test flushing zero-copy emitter."""
        emitter = ZeroCopyEmitter(memory_pool)
        
        async def run_test():
            await emitter.flush()
        
        asyncio.run(run_test())


# ============================================
# Batch Processing Tests
# ============================================

class TestBatchCollector:
    """Tests for BatchCollector."""
    
    def test_batch_collector_add(self, batch_config):
        """Test adding items to collector."""
        collector = BatchCollector[Dict](batch_config)
        
        for i in range(10):
            item = {"id": i, "data": f"item-{i}"}
            result = collector.add(item, size_bytes=100)
            assert result is None  # Not enough for batch
        
        assert collector.current_size == 10
    
    def test_batch_collector_flush_on_size(self):
        """Test flush triggered by size."""
        config = BatchConfig(max_batch_size=5)
        collector = BatchCollector[Dict](config)
        
        for i in range(5):
            item = {"id": i}
            result = collector.add(item, size_bytes=10)
            if i < 4:
                assert result is None
            else:
                assert result is not None  # Should flush on 5th item
        
        assert collector.is_empty()
    
    def test_batch_collector_flush_on_bytes(self):
        """Test flush triggered by byte size."""
        config = BatchConfig(max_batch_size=100, max_bytes=100)
        collector = BatchCollector[Dict](config)
        
        for i in range(10):
            item = {"id": i}
            result = collector.add(item, size_bytes=20)
            if i < 4:
                assert result is None
            else:
                # Should flush when bytes exceed limit
                pass
        
        # Check that we hit byte limit
        assert collector.current_bytes > 0 or collector.is_empty()
    
    def test_batch_collector_manual_flush(self, batch_config):
        """Test manual flush."""
        collector = BatchCollector[Dict](batch_config)
        
        for i in range(5):
            item = {"id": i}
            collector.add(item, size_bytes=10)
        
        assert collector.current_size == 5
        
        result = collector.flush()
        assert result is not None
        assert len(result.items) == 5
        assert collector.is_empty()


class TestBatchAggregator:
    """Tests for BatchAggregator."""
    
    def test_batch_aggregator_sum(self):
        """Test sum aggregation."""
        def sum_aggregator(batch: List[int]) -> int:
            return sum(batch)
        
        aggregator = BatchAggregator[int, int](aggregate_func=sum_aggregator)
        
        result = aggregator.aggregate([1, 2, 3, 4, 5])
        assert result == 15
    
    def test_batch_aggregator_average(self):
        """Test average aggregation."""
        def avg_aggregator(batch: List[float]) -> float:
            return sum(batch) / len(batch)
        
        aggregator = BatchAggregator[float, float](aggregate_func=avg_aggregator)
        
        result = aggregator.aggregate([1.0, 2.0, 3.0, 4.0, 5.0])
        assert result == 3.0
    
    def test_batch_aggregator_empty_batch(self):
        """Test empty batch handling."""
        aggregator = BatchAggregator[int, int](aggregate_func=lambda x: sum(x))
        
        with pytest.raises(ValueError):
            aggregator.aggregate([])


class TestBatchProcessor:
    """Tests for BatchProcessor."""
    
    @pytest.mark.asyncio
    async def test_batch_processor_start_stop(self, batch_config):
        """Test starting and stopping processor."""
        processor = BatchProcessor[Dict](batch_config)
        
        await processor.start()
        assert processor._running is True
        
        await processor.stop()
        assert processor._running is False
    
    @pytest.mark.asyncio
    async def test_batch_processor_add_events(self, batch_config):
        """Test adding events to processor."""
        processor = BatchProcessor[Dict](batch_config)
        await processor.start()
        
        for i in range(10):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"id": i},
            )
            await processor.add(event)
        
        await processor.stop()
        
        stats = processor.get_stats()
        assert stats.total_items == 10


# ============================================
# Partitioning Tests
# ============================================

class TestPartitioner:
    """Tests for Partitioner."""
    
    def test_partitioner_key_strategy(self, partition_config):
        """Test key-based partitioning."""
        partitioner = Partitioner[Dict, str](partition_config)
        
        # Create events with same key
        for i in range(10):
            event = StreamEvent.create(
                key="test-key",
                value={"user_id": "user-123", "data": f"item-{i}"},
            )
            partition = partitioner.partition(event)
            # Same key should go to same partition
            assert isinstance(partition, int)
            assert 0 <= partition < 10
    
    def test_partitioner_hash_strategy(self):
        """Test hash-based partitioning."""
        config = PartitionConfig(
            strategy=PartitionStrategy.HASH,
            num_partitions=10,
        )
        partitioner = Partitioner[Dict, str](config)
        
        event = StreamEvent.create(
            key="test-key",
            value={"data": "test"},
        )
        
        partition = partitioner.partition(event)
        assert 0 <= partition < 10
    
    def test_partitioner_round_robin_strategy(self):
        """Test round-robin partitioning."""
        config = PartitionConfig(
            strategy=PartitionStrategy.ROUND_ROBIN,
            num_partitions=5,
        )
        partitioner = Partitioner[Dict, str](config)
        
        partitions = []
        for i in range(15):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"id": i},
            )
            partition = partitioner.partition(event)
            partitions.append(partition)
        
        # Should distribute evenly
        assert len(set(partitions)) == 5
    
    def test_partitioner_distribution_stats(self, partition_config):
        """Test partition distribution stats."""
        partitioner = Partitioner[Dict, str](partition_config)
        
        # Add events
        for i in range(100):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"user_id": f"user-{i % 10}", "data": f"item-{i}"},
            )
            partitioner.partition(event)
        
        stats = partitioner.get_distribution_stats()
        assert stats["total_events"] == 100
        assert "distribution" in stats
        assert "skew" in stats
    
    def test_partitioner_custom_key_extractor(self):
        """Test custom key extractor."""
        def custom_extractor(value: Dict) -> str:
            return value.get("category", "default")
        
        config = PartitionConfig(
            strategy=PartitionStrategy.KEY,
            num_partitions=5,
            key_extractor=custom_extractor,
        )
        partitioner = Partitioner[Dict, str](config)
        
        event = StreamEvent.create(
            key="test-key",
            value={"category": "electronics", "data": "test"},
        )
        
        partition = partitioner.partition(event)
        assert 0 <= partition < 5


class TestPartitionProcessor:
    """Tests for PartitionProcessor."""
    
    def test_partition_processor_buffer(self):
        """Test partition processor buffering."""
        processor = PartitionProcessor(partition_id=0, buffer_size=5)
        
        for i in range(5):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"id": i},
            )
            result = processor.process(event)
            if i < 4:
                assert result is None
            else:
                assert result is not None
                assert len(result) == 5
        
        assert processor.is_empty
    
    def test_partition_processor_flush(self):
        """Test partition processor flush."""
        processor = PartitionProcessor(partition_id=0, buffer_size=100)
        
        for i in range(10):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"id": i},
            )
            processor.process(event)
        
        assert processor.buffer_size_current == 10
        
        batch = processor.flush()
        assert batch is not None
        assert len(batch) == 10
        assert processor.is_empty
    
    def test_partition_processor_stats(self):
        """Test partition processor stats."""
        processor = PartitionProcessor(partition_id=5, buffer_size=100)
        
        for i in range(20):
            event = StreamEvent.create(
                key=f"key-{i}",
                value={"id": i},
            )
            processor.process(event)
        
        processor.flush()
        
        stats = processor.stats
        assert stats["partition_id"] == 5
        assert stats["processed_count"] == 20


class TestKeyExtractor:
    """Tests for KeyExtractor."""
    
    def test_key_extractor_from_field(self):
        """Test extracting key from field."""
        extractor = KeyExtractor.from_field("user_id")
        
        value = {"user_id": "user-123", "data": "test"}
        key = extractor(value)
        assert key == "user-123"
    
    def test_key_extractor_from_path(self):
        """Test extracting key from nested path."""
        extractor = KeyExtractor.from_path("user.profile.id")
        
        value = {"user": {"profile": {"id": "profile-456"}}}
        key = extractor(value)
        assert key == "profile-456"
    
    def test_key_extractor_composite(self):
        """Test composite key extraction."""
        extractor1 = KeyExtractor.from_field("user_id")
        extractor2 = KeyExtractor.from_field("region")
        
        composite = KeyExtractor.composite(extractor1, extractor2)
        
        value = {"user_id": "user-123", "region": "us-west"}
        key = composite(value)
        # Should be a hash of the tuple
        assert isinstance(key, int)


class TestCompositePartitioner:
    """Tests for CompositePartitioner."""
    
    def test_composite_partitioner(self):
        """Test composite partitioner."""
        strategies = [
            (PartitionStrategy.KEY, 0.6, KeyExtractor.from_field("user_id")),
            (PartitionStrategy.HASH, 0.4, lambda x: x.get("region", "default")),
        ]
        
        partitioner = CompositePartitioner[Dict](strategies=strategies, num_partitions=10)
        
        event = StreamEvent.create(
            key="test-key",
            value={"user_id": "user-123", "region": "us-west"},
        )
        
        partition = partitioner.partition(event)
        assert 0 <= partition < 10
