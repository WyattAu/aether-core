package io.aether.sdk.streaming;

import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;

import io.aether.sdk.streaming.Types.StreamEvent;

/**
 * Partitions events across multiple consumers.
 */
public class Partitioner {
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
        return partitionByKey(event.getKey());
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
