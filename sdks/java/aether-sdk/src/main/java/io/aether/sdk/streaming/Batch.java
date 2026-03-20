package io.aether.sdk.streaming;

import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * Configuration for batch processing.
 */
public class BatchConfig {
    public int maxBatchSize = 1000;
    public Duration maxWaitTime = Duration.ofMillis(100);
    public long maxBytes = 1024 * 1024; // 1MB
    public boolean timeoutOnFull = true;
    public boolean partialOnTimeout = true;
    public boolean partialOnShutdown = true;
    public boolean parallel = false;
    public int maxParallelBatches = 10;
    public Duration batchTimeout = Duration.ofSeconds(1);
    public boolean retryOnFailure = true;
    public Duration retryDelay = Duration.ofMillis(100);
    public double retryBackoff = 2.0;
    public boolean enableAsync = true;
    public boolean adaptiveBatching = false;
    public double batchTimeoutFactor = 1.5;
    public int maxConcurrency = 4;

    public static BatchConfig defaults() {
        return new BatchConfig();
    }
}

/**
 * Result of batch processing.
 */
public class BatchResult<T> {
    public final List<T> items;
    public final long sizeBytes;
    public final Duration processingTime;
    public final String batchId;
    public final Instant timestamp;
    public Object aggregated;
    public String aggregationKey;
    public String checksum;

    public BatchResult(List<T> items, long sizeBytes, Duration processingTime, String batchId) {
        this.items = items;
        this.sizeBytes = sizeBytes;
        this.processingTime = processingTime;
        this.batchId = batchId;
        this.timestamp = Instant.now();
    }
}

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

/**
 * Emits batch results to downstream consumers.
 */
public class BatchEmitter<T> {
    private final List<Consumer<BatchResult<T>>> handlers = new java.util.concurrent.CopyOnWriteArrayList<>();

    /**
     * Add a handler for batch results.
     */
    public void addHandler(Consumer<BatchResult<T>> handler) {
        handlers.add(handler);
    }

    /**
     * Emit batch to all handlers.
     */
    public void emit(BatchResult<T> batch) {
        for (Consumer<BatchResult<T>> handler : handlers) {
            handler.accept(batch);
        }
    }
}

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
        if (event.value instanceof byte[]) {
            sizeBytes = ((byte[]) event.value).length;
        } else if (event.value instanceof String) {
            sizeBytes = ((String) event.value).length();
        }

        BatchResult<T> batchResult = collector.add(event.value, sizeBytes);
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
