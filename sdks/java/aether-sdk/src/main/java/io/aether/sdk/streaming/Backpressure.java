package io.aether.sdk.streaming;

import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.concurrent.locks.*;
import java.util.function.*;

import io.aether.sdk.streaming.Types.*;

/**
 * Backpressure handling for stream processing.
 *
 * <p>This class provides backpressure mechanisms to handle overload scenarios:
 * <ul>
 *   <li>{@link BackpressureStats} - Statistics about backpressure state</li>
 *   <li>{@link BackpressureController} - Controls flow with BUFFER, DROP, FAIL, LATEST strategies</li>
 *   <li>{@link MultiLevelBackpressure} - Priority-based multi-level backpressure</li>
 *   <li>{@link RateBasedBackpressure} - Rate limiting based backpressure</li>
 * </ul>
 *
 * <p>Example:
 * <pre>{@code
 * BackpressureController<Event> controller = new BackpressureController<>(
 *     BackpressureConfig.builder()
 *         .strategy(BackpressureStrategy.BUFFER)
 *         .bufferSize(10000)
 *         .highWatermark(0.9)
 *         .lowWatermark(0.5)
 *         .build()
 * );
 *
 * BackpressureResult<Event> result = controller.process(event);
 * if (result.isAccepted()) {
 *     // process event
 * } else {
 *     // handle rejection
 * }
 * }</pre>
 */
public final class Backpressure {

    private Backpressure() {
    }

    /**
     * Statistics about backpressure state.
     */
    public static final class BackpressureStats {
        private final long totalReceived;
        private final long totalAccepted;
        private final long totalDropped;
        private final long totalFailed;
        private final int currentBufferSize;
        private final int maxBufferSize;
        private final double bufferUtilization;
        private final boolean inBackpressure;
        private final long backpressureStartTime;
        private final long totalBackpressureDurationMs;

        public BackpressureStats(
            long totalReceived,
            long totalAccepted,
            long totalDropped,
            long totalFailed,
            int currentBufferSize,
            int maxBufferSize,
            boolean inBackpressure,
            long backpressureStartTime,
            long totalBackpressureDurationMs
        ) {
            this.totalReceived = totalReceived;
            this.totalAccepted = totalAccepted;
            this.totalDropped = totalDropped;
            this.totalFailed = totalFailed;
            this.currentBufferSize = currentBufferSize;
            this.maxBufferSize = maxBufferSize;
            this.bufferUtilization = maxBufferSize > 0 
                ? (double) currentBufferSize / maxBufferSize 
                : 0.0;
            this.inBackpressure = inBackpressure;
            this.backpressureStartTime = backpressureStartTime;
            this.totalBackpressureDurationMs = totalBackpressureDurationMs;
        }

        public long getTotalReceived() { return totalReceived; }
        public long getTotalAccepted() { return totalAccepted; }
        public long getTotalDropped() { return totalDropped; }
        public long getTotalFailed() { return totalFailed; }
        public int getCurrentBufferSize() { return currentBufferSize; }
        public int getMaxBufferSize() { return maxBufferSize; }
        public double getBufferUtilization() { return bufferUtilization; }
        public boolean isInBackpressure() { return inBackpressure; }
        public long getBackpressureStartTime() { return backpressureStartTime; }
        public long getTotalBackpressureDurationMs() { return totalBackpressureDurationMs; }

        public double getDropRate() {
            return totalReceived > 0 ? (double) totalDropped / totalReceived : 0.0;
        }

        public double getAcceptRate() {
            return totalReceived > 0 ? (double) totalAccepted / totalReceived : 0.0;
        }

        @Override
        public String toString() {
            return String.format(
                "BackpressureStats{received=%d, accepted=%d, dropped=%d, utilization=%.2f%%, inBackpressure=%s}",
                totalReceived, totalAccepted, totalDropped, bufferUtilization * 100, inBackpressure
            );
        }
    }

    /**
     * Result of processing an element through backpressure.
     *
     * @param <T> the element type
     */
    public static final class BackpressureResult<T> {
        private final boolean accepted;
        private final T element;
        private final T droppedElement;
        private final String reason;
        private final long waitTimeMs;

        private BackpressureResult(boolean accepted, T element, T droppedElement, String reason, long waitTimeMs) {
            this.accepted = accepted;
            this.element = element;
            this.droppedElement = droppedElement;
            this.reason = reason;
            this.waitTimeMs = waitTimeMs;
        }

        public static <T> BackpressureResult<T> accepted(T element) {
            return new BackpressureResult<>(true, element, null, null, 0);
        }

        public static <T> BackpressureResult<T> dropped(T element, String reason) {
            return new BackpressureResult<>(false, null, element, reason, 0);
        }

        public static <T> BackpressureResult<T> failed(T element, String reason) {
            return new BackpressureResult<>(false, null, element, reason, 0);
        }

        public static <T> BackpressureResult<T> throttled(T element, long waitTimeMs) {
            return new BackpressureResult<>(false, null, element, "throttled", waitTimeMs);
        }

        public boolean isAccepted() { return accepted; }
        public Optional<T> getElement() { return Optional.ofNullable(element); }
        public Optional<T> getDroppedElement() { return Optional.ofNullable(droppedElement); }
        public Optional<String> getReason() { return Optional.ofNullable(reason); }
        public long getWaitTimeMs() { return waitTimeMs; }
    }

    /**
     * Controller for backpressure handling.
     *
     * <p>Supports strategies:
     * <ul>
     *   <li>BUFFER - Buffer events up to capacity</li>
     *   <li>DROP - Drop events when buffer is full</li>
     *   <li>FAIL - Throw exception when buffer is full</li>
     *   <li>LATEST - Keep only the most recent events</li>
     * </ul>
     *
     * @param <T> the element type
     */
    public static final class BackpressureController<T> {
        private final BackpressureConfig config;
        private final Queue<T> buffer;
        private final Lock lock = new ReentrantLock();
        
        private final AtomicLong totalReceived = new AtomicLong(0);
        private final AtomicLong totalAccepted = new AtomicLong(0);
        private final AtomicLong totalDropped = new AtomicLong(0);
        private final AtomicLong totalFailed = new AtomicLong(0);
        private final AtomicLong backpressureStartTime = new AtomicLong(0);
        private final AtomicLong totalBackpressureDuration = new AtomicLong(0);
        
        private volatile boolean inBackpressure = false;

        public BackpressureController(BackpressureConfig config) {
            this.config = Objects.requireNonNull(config);
            this.buffer = new ArrayDeque<>(config.getBufferSize());
        }

        public BackpressureController() {
            this(BackpressureConfig.defaultConfig());
        }

        /**
         * Process an element through backpressure control.
         */
        public BackpressureResult<T> process(T element) {
            Objects.requireNonNull(element);
            totalReceived.incrementAndGet();
            
            lock.lock();
            try {
                double utilization = (double) buffer.size() / config.getBufferSize();
                
                if (!inBackpressure && utilization >= config.getHighWatermark()) {
                    enterBackpressure();
                } else if (inBackpressure && utilization <= config.getLowWatermark()) {
                    exitBackpressure();
                }
                
                if (buffer.size() < config.getBufferSize()) {
                    switch (config.getStrategy()) {
                        case LATEST:
                            if (buffer.size() >= config.getBufferSize()) {
                                T dropped = buffer.poll();
                                buffer.offer(element);
                                totalDropped.incrementAndGet();
                                totalAccepted.incrementAndGet();
                                return Backpressure.withDropped(
                                    BackpressureResult.accepted(element), dropped, "replaced_by_newer");
                            }
                            buffer.offer(element);
                            totalAccepted.incrementAndGet();
                            return BackpressureResult.accepted(element);
                            
                        default:
                            buffer.offer(element);
                            totalAccepted.incrementAndGet();
                            return BackpressureResult.accepted(element);
                    }
                }
                
                switch (config.getStrategy()) {
                    case DROP:
                        totalDropped.incrementAndGet();
                        return BackpressureResult.dropped(element, "buffer_full");
                        
                    case FAIL:
                        totalFailed.incrementAndGet();
                        throw new BackpressureException("Buffer full, strategy is FAIL");
                        
                    case LATEST:
                        T dropped = buffer.poll();
                        buffer.offer(element);
                        totalDropped.incrementAndGet();
                        totalAccepted.incrementAndGet();
                        return new BackpressureResult<>(true, element, dropped, "replaced_oldest", 0);
                        
                    case BUFFER:
                    default:
                        totalDropped.incrementAndGet();
                        return BackpressureResult.dropped(element, "buffer_full");
                }
            } finally {
                lock.unlock();
            }
        }

        /**
         * Poll the next element from the buffer.
         */
        public Optional<T> poll() {
            lock.lock();
            try {
                T element = buffer.poll();
                if (element != null) {
                    double utilization = (double) buffer.size() / config.getBufferSize();
                    if (inBackpressure && utilization <= config.getLowWatermark()) {
                        exitBackpressure();
                    }
                }
                return Optional.ofNullable(element);
            } finally {
                lock.unlock();
            }
        }

        /**
         * Peek at the next element without removing it.
         */
        public Optional<T> peek() {
            lock.lock();
            try {
                return Optional.ofNullable(buffer.peek());
            } finally {
                lock.unlock();
            }
        }

        /**
         * Get current buffer size.
         */
        public int size() {
            lock.lock();
            try {
                return buffer.size();
            } finally {
                lock.unlock();
            }
        }

        /**
         * Check if buffer is empty.
         */
        public boolean isEmpty() {
            lock.lock();
            try {
                return buffer.isEmpty();
            } finally {
                lock.unlock();
            }
        }

        /**
         * Check if buffer is full.
         */
        public boolean isFull() {
            lock.lock();
            try {
                return buffer.size() >= config.getBufferSize();
            } finally {
                lock.unlock();
            }
        }

        /**
         * Clear the buffer.
         */
        public void clear() {
            lock.lock();
            try {
                int dropped = buffer.size();
                buffer.clear();
                totalDropped.addAndGet(dropped);
                if (inBackpressure) {
                    exitBackpressure();
                }
            } finally {
                lock.unlock();
            }
        }

        /**
         * Get current statistics.
         */
        public BackpressureStats getStats() {
            lock.lock();
            try {
                return new BackpressureStats(
                    totalReceived.get(),
                    totalAccepted.get(),
                    totalDropped.get(),
                    totalFailed.get(),
                    buffer.size(),
                    config.getBufferSize(),
                    inBackpressure,
                    backpressureStartTime.get(),
                    totalBackpressureDuration.get()
                );
            } finally {
                lock.unlock();
            }
        }

        /**
         * Check if currently in backpressure state.
         */
        public boolean isInBackpressure() {
            return inBackpressure;
        }

        /**
         * Get the configuration.
         */
        public BackpressureConfig getConfig() {
            return config;
        }

        private void enterBackpressure() {
            if (!inBackpressure) {
                inBackpressure = true;
                backpressureStartTime.set(System.currentTimeMillis());
            }
        }

        private void exitBackpressure() {
            if (inBackpressure) {
                long startTime = backpressureStartTime.get();
                if (startTime > 0) {
                    totalBackpressureDuration.addAndGet(System.currentTimeMillis() - startTime);
                }
                inBackpressure = false;
                backpressureStartTime.set(0);
            }
        }
    }

    /**
     * Multi-level backpressure with priority queues.
     *
     * <p>Events are processed according to their priority level.
     * Higher priority events are processed first, and backpressure
     * is applied to lower priority levels first.
     *
     * @param <T> the element type
     */
    public static final class MultiLevelBackpressure<T> {
        private final int levels;
        private final List<PriorityLevel<T>> priorityLevels;
        private final Lock lock = new ReentrantLock();

        private final AtomicLong totalReceived = new AtomicLong(0);
        private final AtomicLong totalProcessed = new AtomicLong(0);

        public MultiLevelBackpressure(int levels, int bufferSizePerLevel) {
            if (levels < 1 || levels > 10) {
                throw new IllegalArgumentException("Levels must be between 1 and 10");
            }
            this.levels = levels;
            this.priorityLevels = new ArrayList<>(levels);
            for (int i = 0; i < levels; i++) {
                priorityLevels.add(new PriorityLevel<>(i, bufferSizePerLevel));
            }
        }

        public static <T> MultiLevelBackpressure<T> createDefault() {
            return new MultiLevelBackpressure<>(3, 1000);
        }

        /**
         * Add element at the specified priority level.
         *
         * @param element the element to add
         * @param level priority level (0 = highest)
         * @return true if accepted
         */
        public boolean add(T element, int level) {
            Objects.requireNonNull(element);
            if (level < 0 || level >= levels) {
                throw new IllegalArgumentException("Invalid priority level: " + level);
            }
            
            totalReceived.incrementAndGet();
            
            lock.lock();
            try {
                boolean accepted = priorityLevels.get(level).add(element);
                if (accepted) {
                    totalProcessed.incrementAndGet();
                }
                return accepted;
            } finally {
                lock.unlock();
            }
        }

        /**
         * Poll the highest priority element available.
         */
        public Optional<T> poll() {
            lock.lock();
            try {
                for (PriorityLevel<T> level : priorityLevels) {
                    T element = level.poll();
                    if (element != null) {
                        return Optional.of(element);
                    }
                }
                return Optional.empty();
            } finally {
                lock.unlock();
            }
        }

        /**
         * Poll element from a specific priority level.
         */
        public Optional<T> poll(int level) {
            if (level < 0 || level >= levels) {
                throw new IllegalArgumentException("Invalid priority level: " + level);
            }
            
            lock.lock();
            try {
                return Optional.ofNullable(priorityLevels.get(level).poll());
            } finally {
                lock.unlock();
            }
        }

        /**
         * Get total size across all levels.
         */
        public int size() {
            lock.lock();
            try {
                return priorityLevels.stream()
                    .mapToInt(PriorityLevel::size)
                    .sum();
            } finally {
                lock.unlock();
            }
        }

        /**
         * Get size of a specific level.
         */
        public int size(int level) {
            if (level < 0 || level >= levels) {
                throw new IllegalArgumentException("Invalid priority level: " + level);
            }
            lock.lock();
            try {
                return priorityLevels.get(level).size();
            } finally {
                lock.unlock();
            }
        }

        /**
         * Check if all levels are empty.
         */
        public boolean isEmpty() {
            return size() == 0;
        }

        /**
         * Clear all levels.
         */
        public void clear() {
            lock.lock();
            try {
                priorityLevels.forEach(PriorityLevel::clear);
            } finally {
                lock.unlock();
            }
        }

        /**
         * Apply backpressure by dropping elements from lowest priority first.
         *
         * @param targetSize target total size to achieve
         * @return number of elements dropped
         */
        public int applyBackpressure(int targetSize) {
            lock.lock();
            try {
                int currentSize = size();
                int toDrop = currentSize - targetSize;
                int dropped = 0;
                
                for (int i = levels - 1; i >= 0 && dropped < toDrop; i--) {
                    dropped += priorityLevels.get(i).dropToZero();
                }
                
                return dropped;
            } finally {
                lock.unlock();
            }
        }

        /**
         * Get statistics for all levels.
         */
        public Map<Integer, LevelStats> getStats() {
            lock.lock();
            try {
                Map<Integer, LevelStats> stats = new HashMap<>();
                for (int i = 0; i < levels; i++) {
                    PriorityLevel<T> level = priorityLevels.get(i);
                    stats.put(i, new LevelStats(i, level.size(), level.getDropped()));
                }
                return stats;
            } finally {
                lock.unlock();
            }
        }

        public int getLevelCount() {
            return levels;
        }

        public long getTotalReceived() {
            return totalReceived.get();
        }

        public long getTotalProcessed() {
            return totalProcessed.get();
        }

        /**
         * Statistics for a priority level.
         */
        public static final class LevelStats {
            private final int level;
            private final int currentSize;
            private final long totalDropped;

            public LevelStats(int level, int currentSize, long totalDropped) {
                this.level = level;
                this.currentSize = currentSize;
                this.totalDropped = totalDropped;
            }

            public int getLevel() { return level; }
            public int getCurrentSize() { return currentSize; }
            public long getTotalDropped() { return totalDropped; }
        }

        private static final class PriorityLevel<T> {
            private final int level;
            private final Queue<T> queue;
            private final int capacity;
            private long dropped;

            PriorityLevel(int level, int capacity) {
                this.level = level;
                this.capacity = capacity;
                this.queue = new ArrayDeque<>(capacity);
                this.dropped = 0;
            }

            boolean add(T element) {
                if (queue.size() >= capacity) {
                    dropped++;
                    return false;
                }
                return queue.offer(element);
            }

            T poll() {
                return queue.poll();
            }

            int size() {
                return queue.size();
            }

            long getDropped() {
                return dropped;
            }

            void clear() {
                dropped += queue.size();
                queue.clear();
            }

            int dropToZero() {
                int count = queue.size();
                dropped += count;
                queue.clear();
                return count;
            }
        }
    }

    /**
     * Rate-based backpressure controller.
     *
     * <p>Applies backpressure based on processing rate limits.
     */
    public static final class RateBasedBackpressure {
        private final RateConfig config;
        private final Lock lock = new ReentrantLock();
        
        private final AtomicLong tokens;
        private final AtomicLong lastRefillTime;
        private final AtomicLong totalRequested = new AtomicLong(0);
        private final AtomicLong totalAllowed = new AtomicLong(0);
        private final AtomicLong totalThrottled = new AtomicLong(0);

        public RateBasedBackpressure(RateConfig config) {
            this.config = Objects.requireNonNull(config);
            this.tokens = new AtomicLong(config.getMaxBurst());
            this.lastRefillTime = new AtomicLong(System.nanoTime());
        }

        public RateBasedBackpressure(int permitsPerSecond) {
            this(new RateConfig(permitsPerSecond, permitsPerSecond));
        }

        /**
         * Try to acquire a permit.
         *
         * @return result indicating if allowed or wait time
         */
        public RateResult tryAcquire() {
            return tryAcquire(1);
        }

        /**
         * Try to acquire multiple permits.
         */
        public RateResult tryAcquire(int permits) {
            totalRequested.incrementAndGet();
            
            lock.lock();
            try {
                refill();
                
                if (tokens.get() >= permits) {
                    tokens.addAndGet(-permits);
                    totalAllowed.incrementAndGet();
                    return new RateResult(true, permits, 0);
                }
                
                long needed = permits - tokens.get();
                long waitTimeNanos = (long) (needed * 1_000_000_000.0 / config.getPermitsPerSecond());
                totalThrottled.incrementAndGet();
                
                return new RateResult(false, 0, waitTimeNanos);
            } finally {
                lock.unlock();
            }
        }

        /**
         * Acquire a permit, blocking if necessary.
         */
        public CompletableFuture<Void> acquire() {
            return acquire(1);
        }

        /**
         * Acquire permits, blocking if necessary.
         */
        public CompletableFuture<Void> acquire(int permits) {
            RateResult result = tryAcquire(permits);
            
            if (result.isAllowed()) {
                return CompletableFuture.completedFuture(null);
            }
            
            if (result.getWaitTimeNanos() > 0) {
                long waitMs = result.getWaitTimeNanos() / 1_000_000;
                int remainingNanos = (int) (result.getWaitTimeNanos() % 1_000_000);
                
                return CompletableFuture.runAsync(() -> {
                    try {
                        Thread.sleep(waitMs, remainingNanos);
                    } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                    }
                }).thenCompose(v -> acquire(permits));
            }
            
            return CompletableFuture.failedFuture(
                new BackpressureException("Rate limit exceeded")
            );
        }

        /**
         * Get current rate statistics.
         */
        public RateStats getStats() {
            lock.lock();
            try {
                refill();
                return new RateStats(
                    config.getPermitsPerSecond(),
                    tokens.get(),
                    config.getMaxBurst(),
                    totalRequested.get(),
                    totalAllowed.get(),
                    totalThrottled.get()
                );
            } finally {
                lock.unlock();
            }
        }

        /**
         * Get the configuration.
         */
        public RateConfig getConfig() {
            return config;
        }

        private void refill() {
            long now = System.nanoTime();
            long elapsed = now - lastRefillTime.get();
            
            if (elapsed > 0) {
                long tokensToAdd = (long) (elapsed * config.getPermitsPerSecond() / 1_000_000_000.0);
                long newTokens = Math.min(config.getMaxBurst(), tokens.get() + tokensToAdd);
                tokens.set(newTokens);
                lastRefillTime.set(now);
            }
        }

        /**
         * Configuration for rate-based backpressure.
         */
        public static final class RateConfig {
            private final int permitsPerSecond;
            private final int maxBurst;
            private final double warmupPeriodSeconds;

            private RateConfig(Builder builder) {
                this.permitsPerSecond = builder.permitsPerSecond;
                this.maxBurst = builder.maxBurst != null ? builder.maxBurst : builder.permitsPerSecond;
                this.warmupPeriodSeconds = builder.warmupPeriodSeconds != null ? builder.warmupPeriodSeconds : 0.0;
            }

            public static Builder builder() {
                return new Builder();
            }

            public RateConfig(int permitsPerSecond, int maxBurst) {
                this.permitsPerSecond = permitsPerSecond;
                this.maxBurst = maxBurst;
                this.warmupPeriodSeconds = 0.0;
            }

            public int getPermitsPerSecond() { return permitsPerSecond; }
            public int getMaxBurst() { return maxBurst; }
            public double getWarmupPeriodSeconds() { return warmupPeriodSeconds; }

            public static final class Builder {
                private int permitsPerSecond = 100;
                private Integer maxBurst;
                private Double warmupPeriodSeconds;

                public Builder permitsPerSecond(int permitsPerSecond) {
                    this.permitsPerSecond = permitsPerSecond;
                    return this;
                }

                public Builder maxBurst(int maxBurst) {
                    this.maxBurst = maxBurst;
                    return this;
                }

                public Builder warmupPeriodSeconds(double warmupPeriodSeconds) {
                    this.warmupPeriodSeconds = warmupPeriodSeconds;
                    return this;
                }

                public RateConfig build() {
                    return new RateConfig(this);
                }
            }
        }

        /**
         * Result of a rate limit check.
         */
        public static final class RateResult {
            private final boolean allowed;
            private final int permitsGranted;
            private final long waitTimeNanos;

            public RateResult(boolean allowed, int permitsGranted, long waitTimeNanos) {
                this.allowed = allowed;
                this.permitsGranted = permitsGranted;
                this.waitTimeNanos = waitTimeNanos;
            }

            public boolean isAllowed() { return allowed; }
            public int getPermitsGranted() { return permitsGranted; }
            public long getWaitTimeNanos() { return waitTimeNanos; }
            public long getWaitTimeMs() { return waitTimeNanos / 1_000_000; }
        }

        /**
         * Statistics for rate-based backpressure.
         */
        public static final class RateStats {
            private final int permitsPerSecond;
            private final long availableTokens;
            private final long maxBurst;
            private final long totalRequested;
            private final long totalAllowed;
            private final long totalThrottled;

            public RateStats(
                int permitsPerSecond,
                long availableTokens,
                long maxBurst,
                long totalRequested,
                long totalAllowed,
                long totalThrottled
            ) {
                this.permitsPerSecond = permitsPerSecond;
                this.availableTokens = availableTokens;
                this.maxBurst = maxBurst;
                this.totalRequested = totalRequested;
                this.totalAllowed = totalAllowed;
                this.totalThrottled = totalThrottled;
            }

            public int getPermitsPerSecond() { return permitsPerSecond; }
            public long getAvailableTokens() { return availableTokens; }
            public long getMaxBurst() { return maxBurst; }
            public long getTotalRequested() { return totalRequested; }
            public long getTotalAllowed() { return totalAllowed; }
            public long getTotalThrottled() { return totalThrottled; }

            public double getThrottleRate() {
                return totalRequested > 0 ? (double) totalThrottled / totalRequested : 0.0;
            }
        }
    }

    /**
     * Exception thrown when backpressure causes a failure.
     */
    public static final class BackpressureException extends RuntimeException {
        private final long timestamp;

        public BackpressureException(String message) {
            super(message);
            this.timestamp = System.currentTimeMillis();
        }

        public BackpressureException(String message, Throwable cause) {
            super(message, cause);
            this.timestamp = System.currentTimeMillis();
        }

        public long getTimestamp() {
            return timestamp;
        }
    }

    public static <T> BackpressureResult<T> withDropped(BackpressureResult<T> result, T dropped, String reason) {
        return new BackpressureResult<>(result.accepted, result.element, dropped, reason, result.waitTimeMs);
    }
}
