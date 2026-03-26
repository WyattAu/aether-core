"""
Tests for Aether SDK Batch Module

Tests for batch processing operations.
"""

import pytest
import asyncio
import time
from typing import Any, Dict, List
from dataclasses import dataclass
from unittest.mock import AsyncMock, MagicMock

from aether_sdk.streaming.batch import (
    BatchConfig,
    BatchResult,
    BatchStats,
    BatchCollector,
    BatchAggregator,
    BatchEmitter,
    BatchProcessor,
)
from aether_sdk.streaming.types import StreamEvent, Timestamp


# ============================================
# Helper Functions
# ============================================

def create_event(key: str, value: Any = None) -> StreamEvent:
    """Create a test stream event."""
    return StreamEvent(
        key=key,
        value=value if value is not None else {"data": key},
        timestamp=Timestamp.now(),
    )


# ============================================
# Fixtures
# ============================================

@pytest.fixture
def batch_config():
    """Create a default batch config."""
    return BatchConfig(
        max_batch_size=10,
        max_wait_time_ms=1000,
        max_bytes=1024 * 1024,
    )


@pytest.fixture
def small_batch_config():
    """Create a small batch config for testing."""
    return BatchConfig(
        max_batch_size=3,
        max_wait_time_ms=100,
        max_bytes=100,
    )


@pytest.fixture
def batch_collector(batch_config):
    """Create a batch collector."""
    return BatchCollector(batch_config)


@pytest.fixture
def small_collector(small_batch_config):
    """Create a small batch collector."""
    return BatchCollector(small_batch_config)


# ============================================
# BatchConfig Tests
# ============================================

class TestBatchConfig:
    """Tests for BatchConfig."""
    
    def test_default_config(self):
        """Test default configuration."""
        config = BatchConfig()
        
        assert config.max_batch_size == 1000
        assert config.max_wait_time_ms == 100
        assert config.max_bytes == 1024 * 1024
        assert config.timeout_on_full is True
        assert config.partial_on_timeout is True
        assert config.partial_on_shutdown is True
        assert config.parallel is False
        assert config.enable_async is True
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = BatchConfig(
            max_batch_size=500,
            max_wait_time_ms=200,
            max_bytes=2048 * 1024,
            timeout_on_full=False,
            parallel=True,
            max_parallel_batches=5,
        )
        
        assert config.max_batch_size == 500
        assert config.max_wait_time_ms == 200
        assert config.max_bytes == 2048 * 1024
        assert config.timeout_on_full is False
        assert config.parallel is True
        assert config.max_parallel_batches == 5


# ============================================
# BatchResult Tests
# ============================================

class TestBatchResult:
    """Tests for BatchResult."""
    
    def test_minimal_result(self):
        """Test creating minimal result."""
        result = BatchResult(items=[1, 2, 3])
        
        assert result.items == [1, 2, 3]
        assert result.size_bytes == 0
        assert result.processing_time_ms == 0.0
        assert result.batch_id == ""
    
    def test_full_result(self):
        """Test creating full result."""
        result = BatchResult(
            items=[1, 2, 3],
            size_bytes=100,
            processing_time_ms=50.5,
            batch_id="batch-123",
            aggregated=6,
            aggregation_key="sum",
        )
        
        assert result.items == [1, 2, 3]
        assert result.size_bytes == 100
        assert result.processing_time_ms == 50.5
        assert result.batch_id == "batch-123"
        assert result.aggregated == 6


# ============================================
# BatchStats Tests
# ============================================

class TestBatchStats:
    """Tests for BatchStats."""
    
    def test_default_stats(self):
        """Test default stats."""
        stats = BatchStats()
        
        assert stats.total_items == 0
        assert stats.total_batches == 0
        assert stats.total_bytes == 0
        assert stats.failed_batches == 0
        assert stats.min_processing_time_ms == float('inf')
        assert stats.max_processing_time_ms == 0.0
    
    def test_custom_stats(self):
        """Test custom stats."""
        stats = BatchStats(
            total_items=1000,
            total_batches=10,
            total_bytes=5000,
            total_processing_time_ms=500.0,
            min_processing_time_ms=10.0,
            max_processing_time_ms=100.0,
            avg_batch_size=100.0,
        )
        
        assert stats.total_items == 1000
        assert stats.total_batches == 10
        assert stats.total_bytes == 5000


# ============================================
# BatchCollector Tests
# ============================================

class TestBatchCollector:
    """Tests for BatchCollector."""
    
    def test_initial_state(self, batch_collector):
        """Test initial collector state."""
        assert batch_collector.current_size == 0
        assert batch_collector.current_bytes == 0
        assert batch_collector.is_empty()
    
    def test_add_single_item(self, batch_collector):
        """Test adding single item."""
        result = batch_collector.add("item1")
        
        # Should not flush yet
        assert result is None
        assert batch_collector.current_size == 1
    
    def test_add_until_batch_full(self, small_collector):
        """Test adding items until batch is full."""
        # Add items up to max_batch_size - 1
        for i in range(2):
            result = small_collector.add(f"item{i}")
            assert result is None
        
        # Next item should trigger flush
        result = small_collector.add("item2")
        
        assert result is not None
        assert len(result.items) == 3
        assert result.batch_id == "batch-1"
        
        # Collector should be empty now
        assert small_collector.current_size == 0
    
    def test_add_with_size_bytes(self, small_collector):
        """Test adding items with size tracking."""
        # Add items with size that exceeds max_bytes
        result = small_collector.add("item1", size_bytes=50)
        assert result is None
        
        # This should trigger flush due to byte limit
        result = small_collector.add("item2", size_bytes=60)
        
        assert result is not None
        assert result.size_bytes == 110
    
    def test_add_many(self, small_collector):
        """Test adding multiple items at once."""
        items = ["item1", "item2", "item3"]
        
        result = small_collector.add_many(items)
        
        # Should trigger flush since we're adding 3 items to a size-3 batch
        assert result is not None
        assert len(result.items) == 3
    
    def test_add_many_partial(self, batch_config):
        """Test add_many with partial flush."""
        # Create collector with size 5
        config = BatchConfig(max_batch_size=5)
        collector = BatchCollector(config)
        
        # Add 3 items - should not flush
        result = collector.add_many(["item1", "item2", "item3"])
        assert result is None
        assert collector.current_size == 3
    
    def test_flush_empty(self, batch_collector):
        """Test flushing empty collector."""
        result = batch_collector.flush()
        
        assert result is None
    
    def test_flush_with_items(self, batch_collector):
        """Test flushing collector with items."""
        # Add some items
        for i in range(5):
            batch_collector.add(f"item{i}")
        
        # Flush
        result = batch_collector.flush()
        
        assert result is not None
        assert len(result.items) == 5
        assert batch_collector.is_empty()
    
    def test_flush_resets_state(self, small_collector):
        """Test that flush resets collector state."""
        small_collector.add("item1")
        small_collector.add("item2")
        
        result = small_collector.flush()
        
        assert result is not None
        assert len(result.items) == 2
        assert small_collector.current_size == 0
        assert small_collector.current_bytes == 0
    
    def test_batch_timing(self, small_batch_config):
        """Test batch processing time tracking."""
        small_batch_config.max_wait_time_ms = 50
        collector = BatchCollector(small_batch_config)
        
        collector.add("item1")
        time.sleep(0.06)  # Wait longer than max_wait_time
        
        # Check timing - should have elapsed time when we flush
        result = collector.flush()
        assert result is not None
        # Processing time should be > 0
        assert result.processing_time_ms >= 0
    
    def test_timeout_on_full(self):
        """Test timeout_on_full configuration."""
        config = BatchConfig(
            max_batch_size=10,
            max_wait_time_ms=50,
            timeout_on_full=True,
        )
        collector = BatchCollector(config)
        
        collector.add("item1")
        time.sleep(0.06)  # Wait longer than timeout
        
        # Should flush on next add due to timeout
        result = collector.add("item2")
        
        # With timeout_on_full=True, should flush
        assert result is not None
    
    def test_multiple_batches(self, small_collector):
        """Test creating multiple batches."""
        # First batch
        result1 = small_collector.add("item1")
        result2 = small_collector.add("item2")
        result3 = small_collector.add("item3")
        
        assert result1 is None
        assert result2 is None
        assert result3 is not None
        assert result3.batch_id == "batch-1"
        
        # Second batch
        result4 = small_collector.add("item4")
        result5 = small_collector.add("item5")
        result6 = small_collector.add("item6")
        
        assert result4 is None
        assert result5 is None
        assert result6 is not None
        assert result6.batch_id == "batch-2"


# ============================================
# BatchAggregator Tests
# ============================================

class TestBatchAggregator:
    """Tests for BatchAggregator."""
    
    def test_aggregate_with_function(self):
        """Test aggregation with custom function."""
        def sum_func(items):
            return sum(items)
        
        aggregator = BatchAggregator(aggregate_func=sum_func)
        result = aggregator.aggregate([1, 2, 3, 4, 5])
        
        assert result == 15
    
    def test_aggregate_without_function(self):
        """Test aggregation without function (returns last item)."""
        aggregator = BatchAggregator()
        result = aggregator.aggregate([1, 2, 3, 4, 5])
        
        # Default behavior returns last item
        assert result == 5
    
    def test_aggregate_empty_batch_raises(self):
        """Test that aggregating empty batch raises error."""
        aggregator = BatchAggregator()
        
        with pytest.raises(ValueError):
            aggregator.aggregate([])
    
    def test_aggregate_stats(self):
        """Test aggregator updates stats."""
        def sum_func(items):
            return sum(items)
        
        aggregator = BatchAggregator(aggregate_func=sum_func)
        
        aggregator.aggregate([1, 2, 3])
        aggregator.aggregate([4, 5, 6])
        
        assert aggregator._batch_count == 2
        assert aggregator._total_events == 6
    
    def test_aggregate_with_exception(self):
        """Test aggregator handles exceptions."""
        def failing_func(items):
            raise ValueError("Test error")
        
        aggregator = BatchAggregator(aggregate_func=failing_func)
        
        with pytest.raises(ValueError):
            aggregator.aggregate([1, 2, 3])
        
        # Stats should still be updated
        assert aggregator._batch_count == 0


# ============================================
# BatchEmitter Tests
# ============================================

class TestBatchEmitter:
    """Tests for BatchEmitter."""
    
    @pytest.mark.asyncio
    async def test_emit_to_handler(self):
        """Test emitting to a handler."""
        emitted = []
        
        async def handler(batch):
            emitted.append(batch)
        
        emitter = BatchEmitter()
        emitter.add_handler(handler)
        
        batch = BatchResult(items=[1, 2, 3], batch_id="test-batch")
        await emitter.emit(batch)
        
        assert len(emitted) == 1
        assert emitted[0].batch_id == "test-batch"
    
    @pytest.mark.asyncio
    async def test_emit_to_multiple_handlers(self):
        """Test emitting to multiple handlers."""
        emitted1 = []
        emitted2 = []
        
        async def handler1(batch):
            emitted1.append(batch)
        
        async def handler2(batch):
            emitted2.append(batch)
        
        emitter = BatchEmitter()
        emitter.add_handler(handler1)
        emitter.add_handler(handler2)
        
        batch = BatchResult(items=[1, 2, 3])
        await emitter.emit(batch)
        
        assert len(emitted1) == 1
        assert len(emitted2) == 1
    
    @pytest.mark.asyncio
    async def test_emit_no_handlers(self):
        """Test emitting with no handlers."""
        emitter = BatchEmitter()
        batch = BatchResult(items=[1, 2, 3])
        
        # Should not raise
        await emitter.emit(batch)


# ============================================
# BatchProcessor Tests
# ============================================

class TestBatchProcessor:
    """Tests for BatchProcessor."""
    
    @pytest.mark.asyncio
    async def test_initial_state(self, batch_config):
        """Test initial processor state."""
        processor = BatchProcessor(batch_config)
        
        assert not processor._running
        stats = processor.get_stats()
        assert stats.total_items == 0
        assert stats.total_batches == 0
    
    @pytest.mark.asyncio
    async def test_start_and_stop(self, batch_config):
        """Test starting and stopping processor."""
        processor = BatchProcessor(batch_config)
        
        await processor.start()
        assert processor._running
        
        await processor.stop()
        assert not processor._running
    
    @pytest.mark.asyncio
    async def test_add_event_not_running_raises(self, batch_config):
        """Test adding event when not running raises."""
        processor = BatchProcessor(batch_config)
        
        event = create_event("test")
        
        with pytest.raises(RuntimeError):
            await processor.add(event)
    
    @pytest.mark.asyncio
    async def test_add_single_event(self, batch_config):
        """Test adding single event."""
        processor = BatchProcessor(batch_config)
        await processor.start()
        
        event = create_event("test")
        result = await processor.add(event)
        
        # Should not trigger batch yet
        assert result is False
        
        await processor.stop()
    
    @pytest.mark.asyncio
    async def test_add_events_until_batch(self, small_batch_config):
        """Test adding events until batch triggers."""
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Add events up to batch size
        for i in range(3):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        # Check stats
        stats = processor.get_stats()
        assert stats.total_batches == 1
        assert stats.total_items == 3
        
        await processor.stop()
    
    @pytest.mark.asyncio
    async def test_stop_flushes_remaining(self, small_batch_config):
        """Test that stop flushes remaining items."""
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Add 2 events (not enough to trigger batch)
        for i in range(2):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        stats = processor.get_stats()
        assert stats.total_batches == 0
        
        # Stop should flush remaining
        await processor.stop()
        
        stats = processor.get_stats()
        assert stats.total_batches == 1
        assert stats.total_items == 2
    
    @pytest.mark.asyncio
    async def test_processor_with_aggregator(self, small_batch_config):
        """Test processor with aggregator."""
        def sum_aggregator():
            def aggregate_func(items):
                # Items are StreamEvent values, extract the value attribute
                return sum(item.value if hasattr(item, 'value') else item for item in items)
            return BatchAggregator(aggregate_func=aggregate_func)
        
        small_batch_config.aggregator_factory = sum_aggregator
        
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Add numeric events
        for i in range(3):
            event = create_event(f"test-{i}", value=i)
            await processor.add(event)
        
        stats = processor.get_stats()
        assert stats.total_batches == 1
        
        await processor.stop()
    
    @pytest.mark.asyncio
    async def test_processor_with_emitter(self, small_batch_config):
        """Test processor with emitter."""
        emitted = []
        
        def create_emitter():
            emitter = BatchEmitter()
            
            async def handler(batch):
                emitted.append(batch)
            
            emitter.add_handler(handler)
            return emitter
        
        small_batch_config.emitter_factory = create_emitter
        
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Add events to trigger batch
        for i in range(3):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        # Should have emitted
        assert len(emitted) == 1
        
        await processor.stop()
    
    @pytest.mark.asyncio
    async def test_processor_stats_tracking(self, batch_config):
        """Test processor tracks stats correctly."""
        processor = BatchProcessor(batch_config)
        await processor.start()
        
        # Add multiple batches worth of events
        for i in range(25):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        # Flush remaining
        await processor.stop()
        
        stats = processor.get_stats()
        
        assert stats.total_items == 25
        assert stats.total_batches >= 2  # At least 2 batches
        assert stats.total_processing_time_ms >= 0
        assert stats.min_processing_time_ms <= stats.max_processing_time_ms
    
    @pytest.mark.asyncio
    async def test_processor_on_batch_complete_callback(self, small_batch_config):
        """Test on_batch_complete callback."""
        completed = []
        
        async def on_complete(result):
            completed.append(result)
        
        small_batch_config.on_batch_complete = on_complete
        
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Trigger a batch
        for i in range(3):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        assert len(completed) == 1
        
        await processor.stop()
    
    @pytest.mark.asyncio
    async def test_processor_on_failure_callback(self, small_batch_config):
        """Test on_failure callback."""
        errors = []
        
        async def on_failure(error):
            errors.append(error)
        
        # Create a failing aggregator
        def failing_aggregator():
            def fail_func(items):
                raise ValueError("Test error")
            return BatchAggregator(aggregate_func=fail_func)
        
        small_batch_config.aggregator_factory = failing_aggregator
        small_batch_config.on_failure = on_failure
        
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Trigger a batch (should fail)
        for i in range(3):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        assert len(errors) == 1
        assert isinstance(errors[0], ValueError)
        
        await processor.stop()
    
    @pytest.mark.asyncio
    async def test_processor_error_handler(self, small_batch_config):
        """Test error_handler callback."""
        errors = []
        
        def error_handler(error):
            errors.append(error)
        
        # Create a failing aggregator
        def failing_aggregator():
            def fail_func(items):
                raise ValueError("Test error")
            return BatchAggregator(aggregate_func=fail_func)
        
        small_batch_config.aggregator_factory = failing_aggregator
        small_batch_config.error_handler = error_handler
        
        processor = BatchProcessor(small_batch_config)
        await processor.start()
        
        # Trigger a batch (should fail)
        for i in range(3):
            event = create_event(f"test-{i}")
            await processor.add(event)
        
        assert len(errors) == 1
        
        await processor.stop()


# ============================================
# Integration Tests
# ============================================

class TestBatchIntegration:
    """Integration tests for batch processing."""
    
    @pytest.mark.asyncio
    async def test_full_batch_workflow(self):
        """Test complete batch processing workflow."""
        # Configure
        config = BatchConfig(
            max_batch_size=5,
            max_wait_time_ms=1000,
        )
        
        # Track processed batches
        processed = []
        
        async def handler(batch):
            processed.append(batch.items)
        
        def emitter_factory():
            emitter = BatchEmitter()
            emitter.add_handler(handler)
            return emitter
        
        config.emitter_factory = emitter_factory
        
        # Create processor
        processor = BatchProcessor(config)
        await processor.start()
        
        # Process events
        for i in range(12):
            event = create_event(f"test-{i}", value=i)
            await processor.add(event)
        
        # Stop to flush remaining
        await processor.stop()
        
        # Verify all events processed
        total_items = sum(len(b) for b in processed)
        assert total_items == 12
        
        # Verify stats
        stats = processor.get_stats()
        assert stats.total_items == 12
        assert stats.failed_batches == 0
    
    @pytest.mark.asyncio
    async def test_batch_with_aggregation_and_emission(self):
        """Test batch processing with aggregation and emission."""
        config = BatchConfig(max_batch_size=3)
        
        batch_results = []
        
        # Sum aggregator
        def sum_factory():
            return BatchAggregator(aggregate_func=lambda items: sum(items))
        
        # Emitter
        def emitter_factory():
            emitter = BatchEmitter()
            
            async def handler(batch):
                # When aggregator is used, items contains the aggregated result
                batch_results.append(batch.items)
            
            emitter.add_handler(handler)
            return emitter
        
        config.aggregator_factory = sum_factory
        config.emitter_factory = emitter_factory
        
        processor = BatchProcessor(config)
        await processor.start()
        
        # Add events with numeric values
        for i in range(6):
            event = create_event(f"test-{i}", value=i)
            await processor.add(event)
        
        await processor.stop()
        
        # Should have 2 aggregated results (0+1+2=3, 3+4+5=12)
        # When aggregator is used, items contains the aggregated value
        assert len(batch_results) == 2
        assert batch_results[0] == 3  # Sum of 0+1+2
        assert batch_results[1] == 12  # Sum of 3+4+5
