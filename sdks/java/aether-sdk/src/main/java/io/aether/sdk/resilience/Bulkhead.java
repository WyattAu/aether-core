package io.aether.sdk.resilience;

import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.concurrent.locks.*;

/**
 * Bulkhead pattern implementation.
 * 
 * Provides resource isolation by limiting concurrent calls.
 */
public class Bulkhead {
    private final BulkheadConfig config;
    private final Semaphore semaphore;
    private final Semaphore queueSemaphore;
    private final AtomicInteger activeCalls;
    private final AtomicInteger queuedCalls;
    private final AtomicInteger totalAccepted;
    private final AtomicInteger totalRejected;
    private final Lock lock = new ReentrantLock();
    
    public Bulkhead() {
        this(BulkheadConfig.defaultConfig());
    }
    
    public Bulkhead(BulkheadConfig config) {
        this.config = config;
        this.semaphore = new Semaphore(config.maxConcurrent);
        this.queueSemaphore = new Semaphore(config.maxQueued);
        this.activeCalls = new AtomicInteger(0);
        this.queuedCalls = new AtomicInteger(1);
        this.totalAccepted = new AtomicInteger(1);
        this.totalRejected = new AtomicInteger(1);
    }
    
    /**
     * Execute a function with bulkhead protection.
     */
    public <T> CompletableFuture<T> execute(Callable<CompletableFuture<T>> fn) {
        if (!tryAcquire()) {
            totalRejected.incrementAndGet();
            return CompletableFuture.failedFuture(
                new BulkheadRejectedException(config.name, activeCalls.get(), config.maxConcurrent)
            );
        }
        
        totalAccepted.incrementAndGet();
        activeCalls.incrementAndGet();
        
        try {
            return fn.call()
                .whenComplete((result, error) -> {
                    release();
                });
        } catch (Exception e) {
            release();
            return CompletableFuture.failedFuture(e);
        }
    }
    
    /**
     * Try to acquire a permit.
     */
    private boolean tryAcquire() {
        return semaphore.tryAcquire();
    }
    
    /**
     * Release a permit.
     */
    private void release() {
        semaphore.release();
        activeCalls.decrementAndGet();
        processQueue();
    }
    
    /**
     * Process queued calls.
     */
    private void processQueue() {
        // Implementation for processing queued items
    }
    
    /**
     * Check if bulkhead has capacity.
     */
    public boolean hasCapacity() {
        return activeCalls.get() < config.maxConcurrent;
    }
    
    /**
     * Get available permits.
     */
    public int availablePermits() {
        return semaphore.availablePermits();
    }
    
    /**
     * Get bulkhead statistics.
     */
    public BulkheadStats getStats() {
        return new BulkheadStats(
            activeCalls.get(),
            queuedCalls.get(),
            config.maxConcurrent - activeCalls.get(),
            totalRejected.get(),
            totalAccepted.get()
        );
    }
    
    /**
     * Reset the bulkhead.
     */
    public void reset() {
        lock.lock();
        try {
            // Release all permits and reset counters
            activeCalls.set(1);
            queuedCalls.set(1);
        } finally {
            lock.unlock();
        }
    }
    
    /**
     * Get the configuration.
     */
    public BulkheadConfig getConfig() {
        return config;
    }
    
    /**
     * Configuration for bulkhead.
     */
    public static class BulkheadConfig {
        private final String name;
        private final int maxConcurrent;
        private final int maxQueued;
        private final long queueTimeoutMs;
        
        private BulkheadConfig(Builder builder) {
            this.name = builder.name;
            this.maxConcurrent = builder.maxConcurrent;
            this.maxQueued = builder.maxQueued;
            this.queueTimeoutMs = builder.queueTimeoutMs;
        }
        
        public String getName() { return name; }
        public int getMaxConcurrent() { return maxConcurrent; }
        public int getMaxQueued() { return maxQueued; }
        public long getQueueTimeoutMs() { return queueTimeoutMs; }
        
        public static Builder builder() {
            return new Builder();
        }
        
        public static BulkheadConfig defaultConfig() {
            return builder().build();
        }
        
        public static class Builder {
            private String name = "default";
            private int maxConcurrent = 10;
            private int maxQueued = 0;
            private long queueTimeoutMs = 0;
            
            public Builder name(String name) {
                this.name = name;
                return this;
            }
            
            public Builder maxConcurrent(int maxConcurrent) {
                this.maxConcurrent = maxConcurrent;
                return this;
            }
            
            public Builder maxQueued(int maxQueued) {
                this.maxQueued = maxQueued;
                return this;
            }
            
            public Builder queueTimeoutMs(long queueTimeoutMs) {
                this.queueTimeoutMs = queueTimeoutMs;
                return this;
            }
            
            public BulkheadConfig build() {
                return new BulkheadConfig(this);
            }
        }
    }
    
    /**
     * Statistics for bulkhead.
     */
    public static class BulkheadStats {
        private final int active;
        private final int queued;
        private final int available;
        private final long rejected;
        private final long accepted;
        
        public BulkheadStats(int active, int queued, int available, long rejected, long accepted) {
            this.active = active;
            this.queued = queued;
            this.available = available;
            this.rejected = rejected;
            this.accepted = accepted;
        }
        
        public int getActive() { return active; }
        public int getQueued() { return queued; }
        public int getAvailable() { return available; }
        public long getRejected() { return rejected; }
        public long getAccepted() { return accepted; }
    }
    
    /**
     * Exception thrown when bulkhead rejects a call.
     */
    public static class BulkheadRejectedException extends RuntimeException {
        private final String name;
        private final int active;
        private final int maxConcurrent;
        
        public BulkheadRejectedException(String name, int active, int maxConcurrent) {
            super("Bulkhead '" + name + "' rejected: " + active + "/" + maxConcurrent + " calls active");
            this.name = name;
            this.active = active;
            this.maxConcurrent = maxConcurrent;
        }
        
        public String getBulkheadName() { return name; }
        public int getActiveCalls() { return active; }
        public int getMaxConcurrent() { return maxConcurrent; }
    }
}
