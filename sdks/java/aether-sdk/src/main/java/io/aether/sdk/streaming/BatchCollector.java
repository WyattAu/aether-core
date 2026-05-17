package io.aether.sdk.streaming;

import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;

/**
 * Collects items into batches based on size, time, or byte limits.
 */
public class BatchCollector<T> {
    private final BatchConfig config;
    private final List<T> items = new ArrayList<>();
    private long currentBytes = 0;
    private Instant batchStartTime;
    private int batchCount = 0;
    private final Object lock = new Object();

    public BatchCollector(BatchConfig config) {
        this.config = config;
    }

    /**
     * Add an item to the current batch.
     */
    public BatchResult<T> add(T item, int sizeBytes) {
        synchronized (lock) {
            // Initialize batch timing
            if (batchStartTime == null) {
                batchStartTime = Instant.now();
            }

            // Add item
            items.add(item);
            currentBytes += sizeBytes;

            // Check if batch should be flushed
            if (shouldFlush()) {
                return flush();
            }

            return null;
        }
    }

    /**
     * Add multiple items at once.
     */
    public BatchResult<T> addMany(List<T> newItems, int sizeBytes) {
        if (newItems.isEmpty()) {
            return null;
        }

        int itemSize = sizeBytes / newItems.size();
        for (T item : newItems) {
            BatchResult<T> result = add(item, itemSize);
            if (result != null) {
                return result;
            }
        }
        return null;
    }

    private boolean shouldFlush() {
        if (items.size() >= config.maxBatchSize) {
            return true;
        }
        if (currentBytes >= config.maxBytes) {
            return true;
        }
        if (batchStartTime != null) {
            Duration elapsed = Duration.between(batchStartTime, Instant.now());
            if (elapsed.compareTo(config.maxWaitTime) >= 0) {
                return config.timeoutOnFull;
            }
        }
        return false;
    }

    /**
     * Flush the current batch.
     */
    public BatchResult<T> flush() {
        synchronized (lock) {
            if (items.isEmpty()) {
                return null;
            }

            Duration processingTime = batchStartTime != null
                ? Duration.between(batchStartTime, Instant.now())
                : Duration.ZERO;

            batchCount++;
            String batchId = "batch-" + Instant.now().toEpochMilli() + "-" + batchCount;

            BatchResult<T> result = new BatchResult<>(
                new ArrayList<>(items),
                currentBytes,
                processingTime,
                batchId
            );

            // Reset
            items.clear();
            currentBytes = 0;
            batchStartTime = null;

            return result;
        }
    }

    public int getCurrentSize() {
        synchronized (lock) {
            return items.size();
        }
    }

    public long getCurrentBytes() {
        synchronized (lock) {
            return currentBytes;
        }
    }

    public boolean isEmpty() {
        synchronized (lock) {
            return items.isEmpty();
        }
    }
}
