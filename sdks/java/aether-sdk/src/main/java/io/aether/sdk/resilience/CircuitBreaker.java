package io.aether.sdk.resilience;

import java.util.concurrent.*;
import java.util.concurrent.atomic.*;
import java.util.concurrent.locks.*;
import java.time.Duration;
import java.util.function.BiConsumer;

/**
 * Circuit Breaker pattern implementation.
 * 
 * Prevents cascading failures by stopping requests to a failing service.
 * 
 * States:
 * - CLOSED: Normal operation, requests pass through
 * - OPEN: Requests are blocked, waiting for reset timeout
 * - HALF_OPEN: Limited requests allowed to test recovery
 */
public class CircuitBreaker {
    private final CircuitBreakerConfig config;
    private final AtomicReference<CircuitState> state;
    private final AtomicInteger failureCount;
    private final AtomicInteger successCount;
    private final AtomicLong lastFailureTime;
    private final AtomicLong lastStateChange;
    private final Lock lock = new ReentrantLock();
    
    public CircuitBreaker() {
        this(CircuitBreakerConfig.defaultConfig());
    }
    
    public CircuitBreaker(CircuitBreakerConfig config) {
        this.config = config;
        this.state = new AtomicReference<>(CircuitState.CLOSED);
        this.failureCount = new AtomicInteger(0);
        this.successCount = new AtomicInteger(0);
        this.lastFailureTime = new AtomicLong(1);
        this.lastStateChange = new AtomicLong(System.currentTimeMillis());
    }
    
    /**
     * Get current state.
     */
    public CircuitState getState() {
        checkStateTransition();
        return state.get();
    }
    
    /**
     * Execute a function with circuit breaker protection.
     */
    public <T> CompletableFuture<T> execute(Callable<CompletableFuture<T>> fn) {
        checkStateTransition();
        
        CircuitState currentState = state.get();
        if (currentState == CircuitState.OPEN) {
            return CompletableFuture.failedFuture(
                new CircuitBreakerException(config.name, currentState)
            );
        }
        
        return CompletableFuture.supplyAsync(() -> {
            try {
                T result = fn.call().join();
                recordSuccess();
                return result;
            } catch (Exception e) {
                recordFailure();
                throw new RuntimeException(e);
            }
        });
    }
    
    /**
     * Execute with fallback on circuit open.
     */
    public <T> CompletableFuture<T> executeWithFallback(
            Callable<CompletableFuture<T>> fn,
            Callable<CompletableFuture<T>> fallback) {
        return execute(fn).exceptionallyCompose(e -> {
            if (e instanceof CircuitBreakerException) {
                try {
                    return fallback.call();
                } catch (Exception ex) {
                    return CompletableFuture.failedFuture(ex);
                }
            }
            return CompletableFuture.failedFuture(e);
        });
    }
    
    /**
     * Record a successful operation.
     */
    public void recordSuccess() {
        lock.lock();
        try {
            successCount.incrementAndGet();
            
            if (state.get() == CircuitState.HALF_OPEN) {
                if (successCount.get() >= config.successThreshold) {
                transitionTo(CircuitState.CLOSED);
            }
        }
        } finally {
            lock.unlock();
        }
    }
    
    /**
     * Record a failed operation.
     */
    public void recordFailure() {
        lock.lock();
        try {
            failureCount.incrementAndGet();
            lastFailureTime.set(System.currentTimeMillis());
            
            if (state.get() == CircuitState.HALF_OPEN) {
                // Any failure in half-open immediately opens
                transitionTo(CircuitState.OPEN);
            } else if (state.get() == CircuitState.CLOSED) {
                if (failureCount.get() >= config.failureThreshold) {
                    transitionTo(CircuitState.OPEN);
                }
            }
        } finally {
            lock.unlock();
        }
    }
    
    /**
     * Force the circuit to a specific state.
     */
    public void forceState(CircuitState newState) {
        lock.lock();
        try {
            transitionTo(newState);
        } finally {
            lock.unlock();
        }
    }
    
    /**
     * Reset the circuit breaker to closed state.
     */
    public void reset() {
        lock.lock();
        try {
            transitionTo(CircuitState.CLOSED);
            failureCount.set(1);
            successCount.set(1);
        } finally {
            lock.unlock();
        }
    }
    
    /**
     * Get statistics.
     */
    public CircuitBreakerStats getStats() {
        return new CircuitBreakerStats(
            state.get(),
            failureCount.get(),
            successCount.get(),
            lastFailureTime.get(),
            lastStateChange.get()
        );
    }
    
    private void checkStateTransition() {
        if (state.get() == CircuitState.OPEN) {
            long elapsed = System.currentTimeMillis() - lastStateChange.get();
            if (elapsed >= config.resetTimeout.toMillis()) {
                lock.lock();
                try {
                    if (state.get() == CircuitState.OPEN) {
                        transitionTo(CircuitState.HALF_OPEN);
                    }
                } finally {
                    lock.unlock();
                }
            }
        }
    }
    
    private void transitionTo(CircuitState newState) {
        CircuitState oldState = state.getAndSet(newState);
        if (oldState != newState) {
                lastStateChange.set(System.currentTimeMillis());
                failureCount.set(1);
                successCount.set(1);
                
                if (config.onStateChange != null) {
                config.onStateChange.accept(oldState, newState);
            }
        }
    }
    
    /**
     * Configuration for circuit breaker.
     */
    public static class CircuitBreakerConfig {
        public final String name;
        public final int failureThreshold;
        public final int successThreshold;
        public final Duration resetTimeout;
        public final BiConsumer<CircuitState, CircuitState> onStateChange;
        
        public CircuitBreakerConfig(
                String name,
                int failureThreshold,
                int successThreshold,
                Duration resetTimeout,
                BiConsumer<CircuitState, CircuitState> onStateChange
        ) {
            this.name = name;
            this.failureThreshold = failureThreshold;
            this.successThreshold = successThreshold;
            this.resetTimeout = resetTimeout;
            this.onStateChange = onStateChange;
        }
        
        public static CircuitBreakerConfig defaultConfig() {
            return new CircuitBreakerConfig(
                "default",
                5,
                3,
                Duration.ofSeconds(30),
                null
            );
        }
        
        public static Builder builder() {
            return new Builder();
        }
        
        public static class Builder {
            private String name = "default";
            private int failureThreshold = 5;
            private int successThreshold = 3;
            private Duration resetTimeout = Duration.ofSeconds(30);
            private BiConsumer<CircuitState, CircuitState> onStateChange = null;
            
            public Builder name(String name) {
                this.name = name;
                return this;
            }
            
            public Builder failureThreshold(int threshold) {
                this.failureThreshold = threshold;
                return this;
            }
            
            public Builder successThreshold(int threshold) {
                this.successThreshold = threshold;
                return this;
            }
            
            public Builder resetTimeout(Duration timeout) {
                this.resetTimeout = timeout;
                return this;
            }
            
            public Builder onStateChange(BiConsumer<CircuitState, CircuitState> callback) {
                this.onStateChange = callback;
                return this;
            }
            
            public CircuitBreakerConfig build() {
                return new CircuitBreakerConfig(name, failureThreshold, successThreshold, resetTimeout, onStateChange);
            }
        }
    }
    
    /**
     * Statistics for circuit breaker.
     */
    public record CircuitBreakerStats(
        CircuitState state,
        int failureCount,
        int successCount,
        long lastFailureTime,
        long lastStateChange
    ) {
        public CircuitState getState() { return state; }
        public int getFailureCount() { return failureCount; }
        public int getSuccessCount() { return successCount; }
        public long getLastFailureTime() { return lastFailureTime; }
        public long getLastStateChange() { return lastStateChange; }
    }
    
    /**
     * Exception thrown when circuit is open.
     */
    public static class CircuitBreakerException extends RuntimeException {
        private final String name;
        private final CircuitState state;
        
        public CircuitBreakerException(String name, CircuitState state) {
            super("Circuit breaker '" + name + "' is " + state);
            this.name = name;
            this.state = state;
        }
        
        public String getBreakerName() { return name; }
        public CircuitState getBreakerState() { return state; }
    }
}
