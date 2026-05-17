package io.aether.sdk.streaming;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;

/**
 * Partition strategies.
 */
public enum PartitionStrategy {
    ROUND_ROBIN,
    KEY,
    HASH,
    RANDOM,
    RANGE
}

/**
 * Configuration for partitioning.
 */
class PartitionConfig {
    public PartitionStrategy strategy = PartitionStrategy.KEY;
    public int partitions = 10;
    public Function<Object, String> keyExtractor;
    public List<String> rangeBounds;

    public static PartitionConfig defaults() {
        return new PartitionConfig();
    }
}

/**
 * Statistics for partition distribution.
 */
class PartitionStats {
    public final AtomicLong totalEvents = new AtomicLong(0);
    public final AtomicLong[] partitionCount;
    public final AtomicLong rebalances = new AtomicLong(0);
    public volatile double rebalancesAvg = 0.0;

    public PartitionStats(int numPartitions) {
        partitionCount = new AtomicLong[numPartitions];
        for (int i = 0; i < numPartitions; i++) {
            partitionCount[i] = new AtomicLong(0);
        }
    }

    public PartitionStats copy() {
        PartitionStats copy = new PartitionStats(partitionCount.length);
        copy.totalEvents.set(totalEvents.get());
        for (int i = 0; i < partitionCount.length; i++) {
            copy.partitionCount[i].set(partitionCount[i].get());
        }
        copy.rebalances.set(rebalances.get());
        copy.rebalancesAvg = rebalancesAvg;
        return copy;
    }
}

/**
 * Partitions events across multiple consumers.
 */
class Partitioner {
    private PartitionConfig config;
    private final AtomicInteger currentIndex = new AtomicInteger(0);
    private PartitionStats stats;
    private final Object lock = new Object();

    public Partitioner(PartitionConfig config) {
        if (config.partitions <= 0) {
            config.partitions = 1;
        }
        this.config = config;
        this.stats = new PartitionStats(config.partitions);
    }

    /**
     * Get partition for an event.
     */
    public int partition(StreamEvent<?> event) {
        return partitionByKey(event.key);
    }

    /**
     * Get partition for a key.
     */
    public int partitionByKey(String key) {
        if (key == null || key.isEmpty()) {
            return partitionRoundRobin();
        }

        switch (config.strategy) {
            case KEY:
            case HASH:
                return hashPartition(key);
            case ROUND_ROBIN:
                return partitionRoundRobin();
            case RANGE:
                return rangePartition(key);
            case RANDOM:
                return randomPartition();
            default:
                return hashPartition(key);
        }
    }

    /**
     * Get partition for a value using key extractor.
     */
    public int partitionByValue(Object value) {
        if (config.keyExtractor != null) {
            String key = config.keyExtractor.apply(value);
            return partitionByKey(key);
        }
        return partitionRoundRobin();
    }

    private int hashPartition(String key) {
        int hash = Math.abs(key.hashCode());
        int partition = hash % config.partitions;
        updateStats(partition);
        return partition;
    }

    private int partitionRoundRobin() {
        int partition = Math.abs(currentIndex.getAndIncrement() % config.partitions);
        updateStats(partition);
        return partition;
    }

    private int rangePartition(String key) {
        List<String> bounds = config.rangeBounds;
        if (bounds == null || bounds.isEmpty()) {
            return partitionRoundRobin();
        }

        int partition = 0;
        for (int i = 0; i < bounds.size(); i++) {
            if (key.compareTo(bounds.get(i)) < 0) {
                partition = i;
                break;
            }
            partition = i + 1;
        }

        if (partition >= config.partitions) {
            partition = config.partitions - 1;
        }

        updateStats(partition);
        return partition;
    }

    private int randomPartition() {
        int partition = (int) (Math.random() * config.partitions);
        updateStats(partition);
        return partition;
    }

    private void updateStats(int partition) {
        stats.totalEvents.incrementAndGet();
        stats.partitionCount[partition].incrementAndGet();
    }

    /**
     * Get current statistics.
     */
    public PartitionStats getStats() {
        return stats.copy();
    }

    /**
     * Get number of partitions.
     */
    public int getNumPartitions() {
        return config.partitions;
    }

    /**
     * Rebalance to a new partition count.
     */
    public void rebalance(int newPartitionCount) {
        synchronized (lock) {
            config.partitions = newPartitionCount;
            stats = new PartitionStats(newPartitionCount);
            stats.rebalances.incrementAndGet();
            stats.rebalancesAvg = (double) stats.totalEvents.get() / newPartitionCount;
        }
    }
}

/**
 * Extracts partition keys from events.
 */
class KeyExtractor<T> {
    private final Function<T, String> extractor;
    private String fallback = "default";
    private final AtomicLong count = new AtomicLong(0);
    private final AtomicLong nullCount = new AtomicLong(0);

    public KeyExtractor(Function<T, String> extractor) {
        this.extractor = extractor;
    }

    /**
     * Extract key from value.
     */
    public String extract(T value) {
        count.incrementAndGet();

        if (extractor == null) {
            nullCount.incrementAndGet();
            return fallback;
        }

        String key = extractor.apply(value);
        if (key == null || key.isEmpty()) {
            nullCount.incrementAndGet();
            return fallback;
        }
        return key;
    }

    /**
     * Set fallback key for null/empty results.
     */
    public void setFallback(String fallback) {
        this.fallback = fallback;
    }

    /**
     * Get extraction statistics.
     */
    public long[] getStats() {
        return new long[]{ count.get(), nullCount.get() };
    }
}

/**
 * Processes events for a specific partition.
 */
class PartitionProcessor<T> {
    private final int partitionId;
    private final Function<StreamEvent<T>, Void> handler;
    private final AtomicLong eventCount = new AtomicLong(0);
    private final AtomicLong errorCount = new AtomicLong(0);

    public PartitionProcessor(int partitionId, Function<StreamEvent<T>, Void> handler) {
        this.partitionId = partitionId;
        this.handler = handler;
    }

    /**
     * Process an event.
     */
    public void process(StreamEvent<T> event) throws Exception {
        eventCount.incrementAndGet();

        try {
            handler.apply(event);
        } catch (Exception e) {
            errorCount.incrementAndGet();
            throw e;
        }
    }

    /**
     * Get partition ID.
     */
    public int getPartitionId() {
        return partitionId;
    }

    /**
     * Get processor statistics.
     */
    public long[] getStats() {
        return new long[]{ eventCount.get(), errorCount.get() };
    }
}

/**
 * Strategy weight tuple for composite partitioning.
 */
class StrategyWeight<T> {
    public final PartitionStrategy strategy;
    public final double weight;
    public final Function<T, String> extractor;

    public StrategyWeight(PartitionStrategy strategy, double weight, Function<T, String> extractor) {
        this.strategy = strategy;
        this.weight = weight;
        this.extractor = extractor;
    }
}

/**
 * Combines multiple partitioning strategies.
 */
class CompositePartitioner<T> {
    private final List<StrategyWeight<T>> strategies;
    private final List<Partitioner> partitioners;
    private final int numPartitions;
    private final Object lock = new Object();

    public CompositePartitioner(List<StrategyWeight<T>> strategies, int numPartitions) {
        this.strategies = new ArrayList<>(strategies);
        this.numPartitions = numPartitions;
        this.partitioners = new ArrayList<>();

        for (StrategyWeight<T> sw : strategies) {
            PartitionConfig config = new PartitionConfig();
            config.strategy = sw.strategy;
            config.partitions = numPartitions;
            config.keyExtractor = sw.extractor != null ? v -> sw.extractor.apply((T) v) : null;
            partitioners.add(new Partitioner(config));
        }
    }

    /**
     * Get partition for a value using weighted strategies.
     */
    public int partition(T value) {
        if (partitioners.isEmpty()) {
            return 0;
        }

        // Simple weighted selection - use first strategy's result
        String key = strategies.get(0).extractor != null
            ? strategies.get(0).extractor.apply(value)
            : "";
        return partitioners.get(0).partitionByKey(key);
    }

    /**
     * Add a new strategy.
     */
    public void addStrategy(PartitionStrategy strategy, double weight, Function<T, String> extractor) {
        synchronized (lock) {
            strategies.add(new StrategyWeight<>(strategy, weight, extractor));

            PartitionConfig config = new PartitionConfig();
            config.strategy = strategy;
            config.partitions = numPartitions;
            config.keyExtractor = extractor != null ? v -> extractor.apply((T) v) : null;
            partitioners.add(new Partitioner(config));
        }
    }

    /**
     * Get number of partitions.
     */
    public int getNumPartitions() {
        return numPartitions;
    }
}
