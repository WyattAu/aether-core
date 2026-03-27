package io.aether.sdk.resilience;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Duration;
import java.util.concurrent.*;

class RetryPolicyTest {

    @Test
    @DisplayName("default config values")
    void testDefaultConfig() {
        RetryPolicy policy = new RetryPolicy();
        RetryPolicy.RetryConfig config = policy.getConfig();
        assertEquals("default", config.getName());
        assertEquals(3, config.getMaxAttempts());
        assertEquals(Duration.ofMillis(100), config.getInitialDelay());
        assertEquals(Duration.ofSeconds(30), config.getMaxDelay());
        assertEquals(2.0, config.getMultiplier());
        assertEquals(RetryPolicy.BackoffStrategy.EXPONENTIAL_JITTER, config.getStrategy());
    }

    @Test
    @DisplayName("fixed backoff config")
    void testFixedBackoffConfig() {
        RetryPolicy.RetryConfig config = RetryPolicy.RetryConfig.builder()
            .strategy(RetryPolicy.BackoffStrategy.FIXED)
            .maxAttempts(5)
            .initialDelay(Duration.ofMillis(200))
            .build();
        assertEquals(RetryPolicy.BackoffStrategy.FIXED, config.getStrategy());
        assertEquals(5, config.getMaxAttempts());
        assertEquals(Duration.ofMillis(200), config.getInitialDelay());
    }

    @Test
    @DisplayName("linear backoff config")
    void testLinearBackoffConfig() {
        RetryPolicy.RetryConfig config = RetryPolicy.RetryConfig.builder()
            .strategy(RetryPolicy.BackoffStrategy.LINEAR)
            .build();
        assertEquals(RetryPolicy.BackoffStrategy.LINEAR, config.getStrategy());
    }

    @Test
    @DisplayName("exponential backoff config")
    void testExponentialBackoffConfig() {
        RetryPolicy.RetryConfig config = RetryPolicy.RetryConfig.builder()
            .strategy(RetryPolicy.BackoffStrategy.EXPONENTIAL)
            .multiplier(3.0)
            .build();
        assertEquals(RetryPolicy.BackoffStrategy.EXPONENTIAL, config.getStrategy());
        assertEquals(3.0, config.getMultiplier());
    }

    @Test
    @DisplayName("execute succeeds on first attempt")
    void testExecuteSuccessFirstAttempt() {
        RetryPolicy policy = new RetryPolicy();
        CompletableFuture<String> result = policy.execute(() ->
            CompletableFuture.completedFuture("ok"));
        assertEquals("ok", result.join());
    }

    @Test
    @DisplayName("execute retries on transient error")
    void testExecuteRetriesOnTransientError() {
        java.util.concurrent.atomic.AtomicInteger attempts = new java.util.concurrent.atomic.AtomicInteger(0);
        RetryPolicy policy = new RetryPolicy(
            RetryPolicy.RetryConfig.builder()
                .maxAttempts(3)
                .strategy(RetryPolicy.BackoffStrategy.FIXED)
                .initialDelay(Duration.ofMillis(10))
                .maxDelay(Duration.ofMillis(50))
                .build()
        );

        CompletableFuture<String> result = policy.execute(() -> {
            int attempt = attempts.incrementAndGet();
            if (attempt < 3) {
                CompletableFuture<String> f = new CompletableFuture<>();
                f.completeExceptionally(new RuntimeException("ECONNRESET"));
                return f;
            }
            return CompletableFuture.completedFuture("recovered");
        });

        assertEquals("recovered", result.join());
        assertEquals(3, attempts.get());
    }

    @Test
    @DisplayName("execute throws RetryExhaustedException after max attempts")
    void testExecuteExhaustsRetries() {
        RetryPolicy policy = new RetryPolicy(
            RetryPolicy.RetryConfig.builder()
                .maxAttempts(2)
                .strategy(RetryPolicy.BackoffStrategy.FIXED)
                .initialDelay(Duration.ofMillis(10))
                .maxDelay(Duration.ofMillis(50))
                .build()
        );

        CompletableFuture<String> result = policy.execute(() -> {
            CompletableFuture<String> f = new CompletableFuture<>();
            f.completeExceptionally(new RuntimeException("timeout"));
            return f;
        });

        ExecutionException ex = assertThrows(ExecutionException.class, result::get);
        assertTrue(ex.getCause() instanceof RetryPolicy.RetryExhaustedException);
    }

    @Test
    @DisplayName("RetryExhaustedException contains attempt count")
    void testRetryExhaustedException() {
        RetryPolicy.RetryExhaustedException ex =
            new RetryPolicy.RetryExhaustedException(5, new RuntimeException("fail"));
        assertEquals(5, ex.getAttempts());
        assertNotNull(ex.getLastError());
        assertTrue(ex.getMessage().contains("5"));
    }

    @Test
    @DisplayName("custom retryOn predicate")
    void testCustomRetryOn() {
        RetryPolicy policy = new RetryPolicy(
            RetryPolicy.RetryConfig.builder()
                .maxAttempts(2)
                .strategy(RetryPolicy.BackoffStrategy.FIXED)
                .initialDelay(Duration.ofMillis(10))
                .retryOn(e -> e.getMessage() != null && e.getMessage().contains("retryable"))
                .build()
        );

        CompletableFuture<String> result = policy.execute(() -> {
            CompletableFuture<String> f = new CompletableFuture<>();
            f.completeExceptionally(new RuntimeException("non-retryable"));
            return f;
        });

        ExecutionException ex = assertThrows(ExecutionException.class, result::get);
        assertFalse(ex.getCause() instanceof RetryPolicy.RetryExhaustedException);
    }

    @Test
    @DisplayName("jitter factor config")
    void testJitterFactor() {
        RetryPolicy.RetryConfig config = RetryPolicy.RetryConfig.builder()
            .strategy(RetryPolicy.BackoffStrategy.EXPONENTIAL_JITTER)
            .jitterFactor(0.5)
            .build();
        assertEquals(0.5, config.getJitterFactor());
    }

    @Test
    @DisplayName("BackoffStrategy enum values")
    void testBackoffStrategyValues() {
        assertEquals(4, RetryPolicy.BackoffStrategy.values().length);
        assertNotNull(RetryPolicy.BackoffStrategy.valueOf("FIXED"));
        assertNotNull(RetryPolicy.BackoffStrategy.valueOf("LINEAR"));
        assertNotNull(RetryPolicy.BackoffStrategy.valueOf("EXPONENTIAL"));
        assertNotNull(RetryPolicy.BackoffStrategy.valueOf("EXPONENTIAL_JITTER"));
    }

    @Test
    @DisplayName("maxDelay caps delay")
    void testMaxDelayCap() {
        RetryPolicy.RetryConfig config = RetryPolicy.RetryConfig.builder()
            .maxDelay(Duration.ofMillis(50))
            .build();
        assertEquals(Duration.ofMillis(50), config.getMaxDelay());
    }
}
