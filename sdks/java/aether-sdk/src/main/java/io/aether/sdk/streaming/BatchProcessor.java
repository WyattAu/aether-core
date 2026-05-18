package io.aether.sdk.streaming;

import java.time.Instant;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

import io.aether.sdk.streaming.Types.StreamEvent;

/**
 * Processes events in batches with configurable size and timing.
 */
public class BatchProcessor<T> {
    private final BatchConfig config;
    private final BatchCollector<T> collector;
    private final ConcurrentLinkedQueue<BatchResult<T>> queue = new ConcurrentLinkedQueue<>();
    private final AtomicInteger running = new AtomicInteger(0);
    private final BatchStats stats = new BatchStats();

    public BatchProcessor(BatchConfig config) {
        this.config = config;
        this.collector = new BatchCollector<>(config);
    }

    /**
     * Start the batch processor.
     */
    public void start() {
        if (!running.compareAndSet(0, 1)) {
            throw new IllegalStateException("Processor already running");
        }
        stats.startTime = Instant.now();
    }

    /**
     * Stop the batch processor.
     */
    public void stop() {
        if (!running.compareAndSet(1, 0)) {
            return;
        }

        // Process remaining batches
        BatchResult<T> batch;
        while ((batch = queue.poll()) != null) {
            processBatch(batch);
        }

        // Flush collector
        BatchResult<T> remaining = collector.flush();
        if (remaining != null) {
            processBatch(remaining);
        }

        stats.endTime = Instant.now();
    }

    /**
     * Add an event to the batch processor.
     */
    public boolean add(StreamEvent<T> event) {
        if (running.get() == 0) {
            throw new IllegalStateException("Batch processor not running");
        }

        // Get event size
        int sizeBytes = 0;
        if (event.getValue() instanceof byte[]) {
            sizeBytes = ((byte[]) event.getValue()).length;
        } else if (event.getValue() instanceof String) {
            sizeBytes = ((String) event.getValue()).length();
        }

        BatchResult<T> batchResult = collector.add(event.getValue(), sizeBytes);
        if (batchResult != null) {
            processBatch(batchResult);
            return true;
        }

        return false;
    }

    private void processBatch(BatchResult<T> batch) {
        long start = System.currentTimeMillis();

        stats.totalBatches.incrementAndGet();
        stats.totalItems.addAndGet(batch.items.size());
        stats.totalBytes.addAndGet(batch.sizeBytes);

        long processingTimeMs = System.currentTimeMillis() - start;
        stats.totalProcessingTimeMs.addAndGet(processingTimeMs);

        long minMs = stats.minProcessingTimeMs.get();
        while (processingTimeMs < minMs) {
            if (stats.minProcessingTimeMs.compareAndSet(minMs, processingTimeMs)) {
                break;
            }
            minMs = stats.minProcessingTimeMs.get();
        }

        long maxMs = stats.maxProcessingTimeMs.get();
        while (processingTimeMs > maxMs) {
            if (stats.maxProcessingTimeMs.compareAndSet(maxMs, processingTimeMs)) {
                break;
            }
            maxMs = stats.maxProcessingTimeMs.get();
        }
    }

    /**
     * Get current statistics.
     */
    public BatchStats getStats() {
        return stats.copy();
    }
}
