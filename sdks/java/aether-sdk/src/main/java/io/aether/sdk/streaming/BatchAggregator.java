package io.aether.sdk.streaming;

import java.util.List;
import java.util.function.Function;

/**
 * Aggregates batch items into a single result.
 */
public class BatchAggregator<T, R> {
    private final Function<List<T>, R> aggregateFunc;
    private final Function<T, String> keyExtractor;
    private int batchCount = 0;
    private int totalEvents = 0;
    private long processingTimeMs = 0;
    private final Object lock = new Object();

    public BatchAggregator(Function<List<T>, R> aggregateFunc) {
        this.aggregateFunc = aggregateFunc;
        this.keyExtractor = null;
    }

    public BatchAggregator(Function<List<T>, R> aggregateFunc, Function<T, String> keyExtractor) {
        this.aggregateFunc = aggregateFunc;
        this.keyExtractor = keyExtractor;
    }

    /**
     * Aggregate a batch of items.
     */
    public R aggregate(List<T> batch) {
        if (batch.isEmpty()) {
            throw new IllegalArgumentException("Batch cannot be empty");
        }

        synchronized (lock) {
            long start = System.currentTimeMillis();
            try {
                R result = aggregateFunc.apply(batch);
                batchCount++;
                totalEvents += batch.size();
                return result;
            } finally {
                processingTimeMs += System.currentTimeMillis() - start;
            }
        }
    }

    public int[] getStats() {
        synchronized (lock) {
            return new int[]{ batchCount, totalEvents, (int) processingTimeMs };
        }
    }
}
