package io.aether.sdk.resilience;

import java.util.concurrent.*;
import java.time.Duration;
import java.util.concurrent.atomic.*;
import java.util.concurrent.locks.*;

/**
 * Rate limiter with multiple strategies.
 */
public class RateLimiter {
    private final RateLimitConfig config;
    private final AtomicLong tokens;
    private final AtomicLong lastRefillTime;
    private final Lock lock = new ReentrantLock();
    
    public RateLimiter() {
        this(RateLimitConfig.defaultConfig());
    }
    
    public RateLimiter(RateLimitConfig config) {
        this.config = config;
        this.tokens = new AtomicLong(config.maxRequests);
        this.lastRefillTime = new AtomicLong(System.currentTimeMillis());
    }
    
    /**
     * Try to acquire a permit without blocking.
     */
    public RateLimitResult tryAcquire() {
        return tryAcquire(1);
    }
    
    /**
     * Try to acquire a specified number of permits.
     */
    public RateLimitResult tryAcquire(int permits) {
        lock.lock();
        try {
            refill();
            
            if (tokens.get() >= permits) {
                tokens.addAndGet(-permits);
                return new RateLimitResult(
                    true,
                    (int) tokens.get(),
                    0
                );
            }
            
            long tokensNeeded = permits - tokens.get();
            long waitTimeMs = (long) (tokensNeeded * 1000.0 / config.refillRate);
            
            return new RateLimitResult(
                false,
                (int) tokens.get(),
                waitTimeMs
            );
        } finally {
            lock.unlock();
        }
    }
    
    /**
     * Wait for a permit to be available.
     */
    public CompletableFuture<Void> acquire() {
        return acquire(1);
    }
    
    /**
     * Wait for a specified number of permits.
     */
    public CompletableFuture<Void> acquire(int permits) {
        RateLimitResult result = tryAcquire(permits);
        
        if (result.isAllowed()) {
            return CompletableFuture.completedFuture(null);
        }
        
        if (result.getWaitTimeMs() > 0) {
            return CompletableFuture.runAsync(() -> {
                try {
                    Thread.sleep(result.getWaitTimeMs());
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            }).thenComposeAsync(v -> acquire(permits));
        }
        
        return CompletableFuture.failedFuture(
            new RateLimitExhaustedException(result.getWaitTimeMs())
        );
    }
    
    /**
     * Refill tokens based on elapsed time.
     */
    private void refill() {
        long now = System.currentTimeMillis();
        long elapsed = now - lastRefillTime.get();
        
        if (elapsed > 0) {
            long tokensToAdd = (long) (elapsed * config.refillRate / 1000.0);
            long newTokens = Math.min(
                config.maxRequests,
                tokens.get() + tokensToAdd
            );
            tokens.set(newTokens);
            lastRefillTime.set(now);
        }
    }
    
    /**
     * Get remaining tokens.
     */
    public int getRemainingTokens() {
        refill();
        return (int) tokens.get();
    }
    
    /**
     * Get the configuration.
     */
    public RateLimitConfig getConfig() {
        return config;
    }
    
    /**
     * Configuration for rate limiter.
     */
    public static class RateLimitConfig {
        private final String name;
        private final int maxRequests;
        private final int refillRate; // tokens per second
        
        private RateLimitConfig(Builder builder) {
            this.name = builder.name;
            this.maxRequests = builder.maxRequests;
            this.refillRate = builder.refillRate;
        }
        
        public String getName() { return name; }
        public int getMaxRequests() { return maxRequests; }
        public int getRefillRate() { return refillRate; }
        
        public static Builder builder() {
            return new Builder();
        }
        
        public static RateLimitConfig defaultConfig() {
            return builder().build();
        }
        
        public static class Builder {
            private String name = "default";
            private int maxRequests = 100;
            private int refillRate = 100;
            
            public Builder name(String name) {
                this.name = name;
                return this;
            }
            
            public Builder maxRequests(int maxRequests) {
                this.maxRequests = maxRequests;
                return this;
            }
            
            public Builder refillRate(int refillRate) {
                this.refillRate = refillRate;
                return this;
            }
            
            public RateLimitConfig build() {
                return new RateLimitConfig(this);
            }
        }
    }
    
    /**
     * Result of a rate limit check.
     */
    public static class RateLimitResult {
        private final boolean allowed;
        private final int remaining;
        private final long waitTimeMs;
        
        public RateLimitResult(boolean allowed, int remaining, long waitTimeMs) {
            this.allowed = allowed;
            this.remaining = remaining;
            this.waitTimeMs = waitTimeMs;
        }
        
        public boolean isAllowed() { return allowed; }
        public int getRemaining() { return remaining; }
        public long getWaitTimeMs() { return waitTimeMs; }
    }
    
    /**
     * Exception thrown when rate limit is exceeded.
     */
    public static class RateLimitExhaustedException extends RuntimeException {
        private final long waitTimeMs;
        
        public RateLimitExhaustedException(long waitTimeMs) {
            super("Rate limit exceeded. Wait " + waitTimeMs + "ms");
            this.waitTimeMs = waitTimeMs;
        }
        
        public long getWaitTimeMs() { return waitTimeMs; }
    }
}
