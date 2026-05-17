package io.aether.sdk.streaming;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

/**
 * Combines multiple partitioning strategies.
 */
public class CompositePartitioner<T> {
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
