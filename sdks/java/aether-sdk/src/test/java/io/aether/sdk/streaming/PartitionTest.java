package io.aether.sdk.streaming;

import io.aether.sdk.streaming.Types.StreamEvent;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.List;

class PartitionTest {

    @Test
    @DisplayName("hash partition returns valid index")
    void testHashPartition() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.HASH;
        config.partitions = 4;
        Partitioner partitioner = new Partitioner(config);
        int p = partitioner.partitionByKey("user-123");
        assertTrue(p >= 0 && p < 4);
    }

    @Test
    @DisplayName("round robin distributes across partitions")
    void testRoundRobin() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.ROUND_ROBIN;
        config.partitions = 3;
        Partitioner partitioner = new Partitioner(config);
        int p1 = partitioner.partitionByKey("key");
        int p2 = partitioner.partitionByKey("key");
        int p3 = partitioner.partitionByKey("key");
        assertNotEquals(p1, p2);
        assertNotEquals(p2, p3);
    }

    @Test
    @DisplayName("null key falls back to round robin")
    void testNullKeyFallback() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.HASH;
        config.partitions = 5;
        Partitioner partitioner = new Partitioner(config);
        int p = partitioner.partitionByKey(null);
        assertTrue(p >= 0 && p < 5);
    }

    @Test
    @DisplayName("empty key falls back to round robin")
    void testEmptyKeyFallback() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.HASH;
        config.partitions = 5;
        Partitioner partitioner = new Partitioner(config);
        int p = partitioner.partitionByKey("");
        assertTrue(p >= 0 && p < 5);
    }

    @Test
    @DisplayName("random partition returns valid index")
    void testRandomPartition() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.RANDOM;
        config.partitions = 10;
        Partitioner partitioner = new Partitioner(config);
        for (int i = 0; i < 100; i++) {
            int p = partitioner.partitionByKey("key-" + i);
            assertTrue(p >= 0 && p < 10);
        }
    }

    @Test
    @DisplayName("partition by event")
    void testPartitionByEvent() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.KEY;
        config.partitions = 4;
        Partitioner partitioner = new Partitioner(config);
        StreamEvent<String> event = StreamEvent.create("my-key", "value");
        int p = partitioner.partition(event);
        assertTrue(p >= 0 && p < 4);
    }

    @Test
    @DisplayName("partitionByValue with key extractor")
    void testPartitionByValue() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.KEY;
        config.partitions = 3;
        config.keyExtractor = v -> ((String) v).substring(0, 1);
        Partitioner partitioner = new Partitioner(config);
        int p = partitioner.partitionByValue("abc");
        assertTrue(p >= 0 && p < 3);
    }

    @Test
    @DisplayName("getNumPartitions returns correct count")
    void testGetNumPartitions() {
        PartitionConfig config = new PartitionConfig();
        config.partitions = 7;
        Partitioner partitioner = new Partitioner(config);
        assertEquals(7, partitioner.getNumPartitions());
    }

    @Test
    @DisplayName("zero partitions defaults to 1")
    void testZeroPartitionsDefaults() {
        PartitionConfig config = new PartitionConfig();
        config.partitions = 0;
        Partitioner partitioner = new Partitioner(config);
        assertEquals(1, partitioner.getNumPartitions());
    }

    @Test
    @DisplayName("getStats tracks distribution")
    void testGetStats() {
        PartitionConfig config = new PartitionConfig();
        config.strategy = PartitionStrategy.KEY;
        config.partitions = 3;
        Partitioner partitioner = new Partitioner(config);
        partitioner.partitionByKey("a");
        partitioner.partitionByKey("b");
        PartitionStats stats = partitioner.getStats();
        assertEquals(2, stats.totalEvents.get());
    }

    @Test
    @DisplayName("rebalance resets stats")
    void testRebalance() {
        PartitionConfig config = new PartitionConfig();
        config.partitions = 3;
        Partitioner partitioner = new Partitioner(config);
        partitioner.partitionByKey("a");
        partitioner.rebalance(5);
        assertEquals(5, partitioner.getNumPartitions());
        PartitionStats stats = partitioner.getStats();
        assertEquals(0, stats.totalEvents.get());
        assertEquals(1, stats.rebalances.get());
    }

    @Test
    @DisplayName("KeyExtractor extracts key")
    void testKeyExtractor() {
        KeyExtractor<String> extractor = new KeyExtractor<>(s -> s.split(":")[0]);
        assertEquals("user", extractor.extract("user:123"));
    }

    @Test
    @DisplayName("KeyExtractor fallback for null result")
    void testKeyExtractorFallback() {
        KeyExtractor<String> extractor = new KeyExtractor<>(s -> null);
        assertEquals("default", extractor.extract("anything"));
        extractor.setFallback("fallback");
        assertEquals("fallback", extractor.extract("anything"));
    }

    @Test
    @DisplayName("KeyExtractor stats")
    void testKeyExtractorStats() {
        KeyExtractor<String> extractor = new KeyExtractor<>(s -> s);
        extractor.extract("a");
        extractor.extract(null);
        long[] stats = extractor.getStats();
        assertEquals(2, stats[0]);
        assertEquals(1, stats[1]);
    }

    @Test
    @DisplayName("PartitionProcessor tracks events and errors")
    void testPartitionProcessor() throws Exception {
        PartitionProcessor<String> pp = new PartitionProcessor<>(0, event -> null);
        pp.process(StreamEvent.create("k", "v"));
        long[] stats = pp.getStats();
        assertEquals(1, stats[0]);
        assertEquals(0, stats[1]);
        assertEquals(0, pp.getPartitionId());
    }

    @Test
    @DisplayName("PartitionProcessor error counting")
    void testPartitionProcessorError() {
        PartitionProcessor<String> pp = new PartitionProcessor<>(0, event -> {
            throw new RuntimeException("fail");
        });
        assertThrows(Exception.class, () -> pp.process(StreamEvent.create("k", "v")));
        assertEquals(1, pp.getStats()[1]);
    }

    @Test
    @DisplayName("PartitionStrategy enum values")
    void testPartitionStrategyValues() {
        assertEquals(5, PartitionStrategy.values().length);
        assertNotNull(PartitionStrategy.valueOf("ROUND_ROBIN"));
        assertNotNull(PartitionStrategy.valueOf("KEY"));
        assertNotNull(PartitionStrategy.valueOf("HASH"));
        assertNotNull(PartitionStrategy.valueOf("RANDOM"));
        assertNotNull(PartitionStrategy.valueOf("RANGE"));
    }
}
