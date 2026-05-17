package io.aether.sdk.streaming;

import java.time.Instant;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Statistics for batch processing.
 */
public class BatchStats {
    public final AtomicLong totalItems = new AtomicLong(0);
    public final AtomicLong totalBatches = new AtomicLong(0);
    public final AtomicLong totalBytes = new AtomicLong(0);
    public final AtomicLong totalProcessingTimeMs = new AtomicLong(0);
    public final AtomicLong minProcessingTimeMs = new AtomicLong(Long.MAX_VALUE);
    public final AtomicLong maxProcessingTimeMs = new AtomicLong(0);
    public final AtomicLong failedBatches = new AtomicLong(0);
    public volatile Instant startTime;
    public volatile Instant endTime;

    public double getAvgBatchSize() {
        long batches = totalBatches.get();
        return batches > 0 ? (double) totalItems.get() / batches : 0.0;
    }

    public BatchStats copy() {
        BatchStats copy = new BatchStats();
        copy.totalItems.set(totalItems.get());
        copy.totalBatches.set(totalBatches.get());
        copy.totalBytes.set(totalBytes.get());
        copy.totalProcessingTimeMs.set(totalProcessingTimeMs.get());
        copy.minProcessingTimeMs.set(minProcessingTimeMs.get());
        copy.maxProcessingTimeMs.set(maxProcessingTimeMs.get());
        copy.failedBatches.set(failedBatches.get());
        copy.startTime = startTime;
        copy.endTime = endTime;
        return copy;
    }
}
