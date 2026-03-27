package io.aether.sdk.streaming;

import io.aether.sdk.streaming.Types.StreamEvent;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Duration;
import java.util.List;

class BatchTest {

    @Test
    @DisplayName("BatchConfig defaults")
    void testBatchConfigDefaults() {
        BatchConfig config = BatchConfig.defaults();
        assertEquals(1000, config.maxBatchSize);
        assertEquals(Duration.ofMillis(100), config.maxWaitTime);
        assertEquals(1024 * 1024, config.maxBytes);
        assertTrue(config.partialOnTimeout);
    }

    @Test
    @DisplayName("BatchCollector add returns null until full")
    void testCollectorAddNotFull() {
        BatchConfig config = BatchConfig.defaults();
        config.maxBatchSize = 5;
        BatchCollector<String> collector = new BatchCollector<>(config);
        assertNull(collector.add("a", 10));
        assertNull(collector.add("b", 10));
        assertEquals(2, collector.getCurrentSize());
    }

    @Test
    @DisplayName("BatchCollector flushes at maxBatchSize")
    void testCollectorFlushesAtSize() {
        BatchConfig config = new BatchConfig();
        config.maxBatchSize = 3;
        BatchCollector<String> collector = new BatchCollector<>(config);
        assertNull(collector.add("a", 10));
        assertNull(collector.add("b", 10));
        BatchResult<String> result = collector.add("c", 10);
        assertNotNull(result);
        assertEquals(3, result.items.size());
        assertTrue(collector.isEmpty());
    }

    @Test
    @DisplayName("BatchCollector flushes at maxBytes")
    void testCollectorFlushesAtBytes() {
        BatchConfig config = new BatchConfig();
        config.maxBatchSize = 100;
        config.maxBytes = 50;
        BatchCollector<String> collector = new BatchCollector<>(config);
        BatchResult<String> result = collector.add("a", 60);
        assertNotNull(result);
        assertEquals(1, result.items.size());
        assertEquals(60, result.sizeBytes);
    }

    @Test
    @DisplayName("BatchCollector manual flush")
    void testCollectorFlush() {
        BatchConfig config = BatchConfig.defaults();
        BatchCollector<String> collector = new BatchCollector<>(config);
        collector.add("a", 10);
        collector.add("b", 10);
        BatchResult<String> result = collector.flush();
        assertNotNull(result);
        assertEquals(2, result.items.size());
        assertTrue(result.items.contains("a"));
        assertTrue(result.items.contains("b"));
    }

    @Test
    @DisplayName("BatchCollector flush empty returns null")
    void testCollectorFlushEmpty() {
        BatchCollector<String> collector = new BatchCollector<>(BatchConfig.defaults());
        assertNull(collector.flush());
    }

    @Test
    @DisplayName("BatchCollector isEmpty and getCurrentSize")
    void testCollectorEmptyAndSize() {
        BatchCollector<String> collector = new BatchCollector<>(BatchConfig.defaults());
        assertTrue(collector.isEmpty());
        assertEquals(0, collector.getCurrentSize());
        collector.add("x", 10);
        assertFalse(collector.isEmpty());
        assertEquals(1, collector.getCurrentSize());
    }

    @Test
    @DisplayName("BatchCollector getCurrentBytes")
    void testCollectorCurrentBytes() {
        BatchCollector<String> collector = new BatchCollector<>(BatchConfig.defaults());
        assertEquals(0, collector.getCurrentBytes());
        collector.add("a", 50);
        assertEquals(50, collector.getCurrentBytes());
        collector.add("b", 30);
        assertEquals(80, collector.getCurrentBytes());
    }

    @Test
    @DisplayName("BatchCollector addMany")
    void testCollectorAddMany() {
        BatchConfig config = new BatchConfig();
        config.maxBatchSize = 3;
        BatchCollector<Integer> collector = new BatchCollector<>(config);
        List<Integer> items = List.of(1, 2, 3);
        BatchResult<Integer> result = collector.addMany(items, 30);
        assertNotNull(result);
        assertEquals(3, result.items.size());
    }

    @Test
    @DisplayName("BatchCollector addMany empty list returns null")
    void testCollectorAddManyEmpty() {
        BatchCollector<String> collector = new BatchCollector<>(BatchConfig.defaults());
        assertNull(collector.addMany(List.of(), 10));
    }

    @Test
    @DisplayName("BatchAggregator aggregates")
    void testBatchAggregator() {
        BatchAggregator<Integer, Integer> agg =
            new BatchAggregator<>(batch -> batch.stream().mapToInt(i -> i).sum());
        int result = agg.aggregate(List.of(1, 2, 3, 4));
        assertEquals(10, result);
        int[] stats = agg.getStats();
        assertEquals(1, stats[0]);
        assertEquals(4, stats[1]);
    }

    @Test
    @DisplayName("BatchAggregator rejects empty batch")
    void testBatchAggregatorEmpty() {
        BatchAggregator<String, String> agg =
            new BatchAggregator<>(batch -> String.join(",", batch));
        assertThrows(IllegalArgumentException.class, () -> agg.aggregate(List.of()));
    }

    @Test
    @DisplayName("BatchResult has correct fields")
    void testBatchResultFields() {
        BatchResult<String> result = new BatchResult<>(
            List.of("a", "b"), 20, Duration.ofMillis(50), "batch-1");
        assertEquals(2, result.items.size());
        assertEquals(20, result.sizeBytes);
        assertEquals("batch-1", result.batchId);
        assertNotNull(result.timestamp);
    }

    @Test
    @DisplayName("BatchStats calculations")
    void testBatchStats() {
        BatchStats stats = new BatchStats();
        stats.totalItems.set(100);
        stats.totalBatches.set(10);
        assertEquals(10.0, stats.getAvgBatchSize());
        BatchStats copy = stats.copy();
        assertEquals(100, copy.totalItems.get());
    }

    @Test
    @DisplayName("BatchEmitter delivers to handlers")
    void testBatchEmitter() {
        BatchEmitter<String> emitter = new BatchEmitter<>();
        final String[] received = {null};
        emitter.addHandler(batch -> received[0] = batch.batchId);
        BatchResult<String> result = new BatchResult<>(
            List.of("x"), 10, Duration.ZERO, "test-batch");
        emitter.emit(result);
        assertEquals("test-batch", received[0]);
    }

    @Test
    @DisplayName("BatchProcessor start and stop")
    void testBatchProcessorStartStop() {
        BatchProcessor<String> processor = new BatchProcessor<>(BatchConfig.defaults());
        processor.start();
        assertThrows(IllegalStateException.class, processor::start);
        processor.stop();
    }

    @Test
    @DisplayName("BatchProcessor rejects events when not running")
    void testBatchProcessorNotRunning() {
        BatchProcessor<String> processor = new BatchProcessor<>(BatchConfig.defaults());
        StreamEvent<String> event = StreamEvent.create("k", "v");
        assertThrows(IllegalStateException.class, () -> processor.add(event));
    }
}
