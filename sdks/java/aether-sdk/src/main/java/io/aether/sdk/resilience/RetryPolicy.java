package io.aether.sdk.resilience;

import java.util.concurrent.*;
import java.time.Duration;
import java.util.*;
import java.util.function.Predicate;

/**
 * Retry policy with configurable backoff strategies.
 */
public class RetryPolicy {
    private final RetryConfig config;
    private final Random random;
    
    public RetryPolicy() {
        this(RetryConfig.defaultConfig());
    }
    
    public RetryPolicy(RetryConfig config) {
        this.config = config;
        this.random = new Random();
    }
    
    /**
     * Execute a function with retry logic.
     */
    public <T> CompletableFuture<T> execute(Callable<CompletableFuture<T>> fn) {
        return executeWithRetry(fn, 1);
    }
    
    private <T> CompletableFuture<T> executeWithRetry(
            Callable<CompletableFuture<T>> fn, int attempt) {
        
        try {
            return fn.call().exceptionallyComposeAsync(
                error -> {
                    if (attempt >= config.maxAttempts) {
                        return CompletableFuture.failedFuture(
                            new RetryExhaustedException(attempt, error)
                        );
                    }
                    
                    if (!shouldRetry(error)) {
                        return CompletableFuture.failedFuture(error);
                    }
                    
                    long delay = calculateDelay(attempt);
                    return CompletableFuture.runAsync(() -> {
                        try {
                            Thread.sleep(delay);
                        } catch (InterruptedException e) {
                            Thread.currentThread().interrupt();
                        }
                    }).thenComposeAsync(v -> executeWithRetry(fn, attempt + 1));
                }
            );
        } catch (Exception e) {
            return CompletableFuture.failedFuture(e);
        }
    }
    
    /**
     * Determine if error should trigger retry.
     */
    private boolean shouldRetry(Throwable error) {
        if (config.retryOn == null) {
            // Default: retry on common transient errors
            return isTransientError(error);
        }
        return config.retryOn.test(error);
    }
    
    private boolean isTransientError(Throwable error) {
        String message = error.getMessage();
        if (message == null) return false;
        
        return message.contains("ECONNRESET") ||
               message.contains("ETIMEDOUT") ||
               message.contains("ECONNREFUSED") ||
               message.contains("timeout") ||
               message.contains("Timeout") ||
               message.contains("503") ||
               message.contains("504");
    }
    
    /**
     * Calculate delay for a given attempt.
     */
    private long calculateDelay(int attempt) {
        long baseDelay;
        
        switch (config.strategy) {
            case FIXED:
                baseDelay = config.initialDelay.toMillis();
                break;
            case LINEAR:
                baseDelay = config.initialDelay.toMillis() * attempt;
                break;
            case EXPONENTIAL:
            case EXPONENTIAL_JITTER:
                baseDelay = config.initialDelay.toMillis() * 
                    (long) Math.pow(config.multiplier, attempt - 1);
                break;
            default:
                baseDelay = config.initialDelay.toMillis();
        }
        
        // Add jitter for exponential-jitter strategy
        if (config.strategy == BackoffStrategy.EXPONENTIAL_JITTER) {
            double jitter = baseDelay * config.jitterFactor;
            baseDelay = baseDelay + (long) ((random.nextDouble() * 2 - 1) * jitter);
        }
        
        // Cap at max delay
        return Math.min(baseDelay, config.maxDelay.toMillis());
    }
    
    /**
     * Get the retry configuration.
     */
    public RetryConfig getConfig() {
        return config;
    }
    
    /**
     * Backoff strategies.
     */
    public enum BackoffStrategy {
        FIXED,
        LINEAR,
        EXPONENTIAL,
        EXPONENTIAL_JITTER
    }
    
    /**
     * Configuration for retry policy.
     */
    public static class RetryConfig {
        private final String name;
        private final int maxAttempts;
        private final Duration initialDelay;
        private final Duration maxDelay;
        private final double multiplier;
        private final BackoffStrategy strategy;
        private final double jitterFactor;
        private final Predicate<Throwable> retryOn;
        
        private RetryConfig(Builder builder) {
            this.name = builder.name;
            this.maxAttempts = builder.maxAttempts;
            this.initialDelay = builder.initialDelay;
            this.maxDelay = builder.maxDelay;
            this.multiplier = builder.multiplier;
            this.strategy = builder.strategy;
            this.jitterFactor = builder.jitterFactor;
            this.retryOn = builder.retryOn;
        }
        
        public String getName() { return name; }
        public int getMaxAttempts() { return maxAttempts; }
        public Duration getInitialDelay() { return initialDelay; }
        public Duration getMaxDelay() { return maxDelay; }
        public double getMultiplier() { return multiplier; }
        public BackoffStrategy getStrategy() { return strategy; }
        public double getJitterFactor() { return jitterFactor; }
        public Predicate<Throwable> getRetryOn() { return retryOn; }
        
        public static Builder builder() {
            return new Builder();
        }
        
        public static RetryConfig defaultConfig() {
            return builder().build();
        }
        
        public static class Builder {
            private String name = "default";
            private int maxAttempts = 3;
            private Duration initialDelay = Duration.ofMillis(100);
            private Duration maxDelay = Duration.ofSeconds(30);
            private double multiplier = 2.0;
            private BackoffStrategy strategy = BackoffStrategy.EXPONENTIAL_JITTER;
            private double jitterFactor = 0.1;
            private Predicate<Throwable> retryOn = null;
            
            public Builder name(String name) {
                this.name = name;
                return this;
            }
            
            public Builder maxAttempts(int maxAttempts) {
                this.maxAttempts = maxAttempts;
                return this;
            }
            
            public Builder initialDelay(Duration initialDelay) {
                this.initialDelay = initialDelay;
                return this;
            }
            
            public Builder maxDelay(Duration maxDelay) {
                this.maxDelay = maxDelay;
                return this;
            }
            
            public Builder multiplier(double multiplier) {
                this.multiplier = multiplier;
                return this;
            }
            
            public Builder strategy(BackoffStrategy strategy) {
                this.strategy = strategy;
                return this;
            }
            
            public Builder jitterFactor(double jitterFactor) {
                this.jitterFactor = jitterFactor;
                return this;
            }
            
            public Builder retryOn(Predicate<Throwable> retryOn) {
                this.retryOn = retryOn;
                return this;
            }
            
            public RetryConfig build() {
                return new RetryConfig(this);
            }
        }
    }
    
    /**
     * Exception thrown when all retry attempts are exhausted.
     */
    public static class RetryExhaustedException extends RuntimeException {
        private final int attempts;
        private final Throwable lastError;
        
        public RetryExhaustedException(int attempts, Throwable lastError) {
            super("All " + attempts + " retry attempts exhausted: " + lastError.getMessage(), lastError);
            this.attempts = attempts;
            this.lastError = lastError;
        }
        
        public int getAttempts() { return attempts; }
        public Throwable getLastError() { return lastError; }
    }
}
