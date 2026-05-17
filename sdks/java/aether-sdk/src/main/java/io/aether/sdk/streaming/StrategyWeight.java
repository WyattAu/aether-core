package io.aether.sdk.streaming;

import java.util.function.Function;

/**
 * Strategy weight tuple for composite partitioning.
 */
public class StrategyWeight<T> {
    public final PartitionStrategy strategy;
    public final double weight;
    public final Function<T, String> extractor;

    public StrategyWeight(PartitionStrategy strategy, double weight, Function<T, String> extractor) {
        this.strategy = strategy;
        this.weight = weight;
        this.extractor = extractor;
    }
}
