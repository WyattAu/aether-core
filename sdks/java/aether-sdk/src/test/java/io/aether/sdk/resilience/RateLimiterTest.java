package io.aether.sdk.resilience;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.concurrent.*;

class RateLimiterTest {

    @Test
    @DisplayName("default config values")
    void testDefaultConfig() {
        RateLimiter limiter = new RateLimiter();
        RateLimiter.RateLimitConfig config = limiter.getConfig();
        assertEquals("default", config.getName());
        assertEquals(100, config.getMaxRequests());
        assertEquals(100, config.getRefillRate());
    }

    @Test
    @DisplayName("custom config")
    void testCustomConfig() {
        RateLimiter.RateLimitConfig config = RateLimiter.RateLimitConfig.builder()
            .name("custom")
            .maxRequests(10)
            .refillRate(5)
            .build();
        assertEquals("custom", config.getName());
        assertEquals(10, config.getMaxRequests());
        assertEquals(5, config.getRefillRate());
    }

    @Test
    @DisplayName("tryAcquire allows when tokens available")
    void testTryAcquireAllowed() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(5)
                .refillRate(1000)
                .build()
        );
        RateLimiter.RateLimitResult result = limiter.tryAcquire();
        assertTrue(result.isAllowed());
    }

    @Test
    @DisplayName("tryAcquire rejects when tokens exhausted")
    void testTryAcquireRejected() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(2)
                .refillRate(0)
                .build()
        );
        assertTrue(limiter.tryAcquire().isAllowed());
        assertTrue(limiter.tryAcquire().isAllowed());
        assertFalse(limiter.tryAcquire().isAllowed());
    }

    @Test
    @DisplayName("tryAcquire multi-permit")
    void testTryAcquireMultiPermit() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(5)
                .refillRate(0)
                .build()
        );
        assertTrue(limiter.tryAcquire(3).isAllowed());
        assertTrue(limiter.tryAcquire(2).isAllowed());
        assertFalse(limiter.tryAcquire(1).isAllowed());
    }

    @Test
    @DisplayName("tryAcquire returns remaining tokens")
    void testTryAcquireRemaining() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(10)
                .refillRate(0)
                .build()
        );
        RateLimiter.RateLimitResult result = limiter.tryAcquire(3);
        assertTrue(result.isAllowed());
        assertEquals(7, result.getRemaining());
    }

    @Test
    @DisplayName("tryAcquire returns wait time when rejected")
    void testTryAcquireWaitTime() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(1)
                .refillRate(1000)
                .build()
        );
        limiter.tryAcquire();
        RateLimiter.RateLimitResult result = limiter.tryAcquire();
        assertFalse(result.isAllowed());
        assertTrue(result.getWaitTimeMs() > 0);
    }

    @Test
    @DisplayName("acquire completes immediately when allowed")
    void testAcquireAllowed() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(10)
                .refillRate(1000)
                .build()
        );
        CompletableFuture<Void> result = limiter.acquire();
        assertTrue(result.isDone());
        assertNull(result.join());
    }

    @Test
    @DisplayName("getRemainingTokens reflects usage")
    void testGetRemainingTokens() {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(10)
                .refillRate(0)
                .build()
        );
        limiter.tryAcquire(3);
        assertTrue(limiter.getRemainingTokens() <= 7);
    }

    @Test
    @DisplayName("RateLimitResult getters")
    void testRateLimitResultGetters() {
        RateLimiter.RateLimitResult result = new RateLimiter.RateLimitResult(true, 5, 0);
        assertTrue(result.isAllowed());
        assertEquals(5, result.getRemaining());
        assertEquals(0, result.getWaitTimeMs());
    }

    @Test
    @DisplayName("RateLimitExhaustedException contains wait time")
    void testRateLimitExhaustedException() {
        RateLimiter.RateLimitExhaustedException ex =
            new RateLimiter.RateLimitExhaustedException(5000);
        assertEquals(5000, ex.getWaitTimeMs());
        assertTrue(ex.getMessage().contains("5000"));
    }

    @Test
    @DisplayName("tokens refill over time")
    void testTokenRefill() throws InterruptedException {
        RateLimiter limiter = new RateLimiter(
            RateLimiter.RateLimitConfig.builder()
                .maxRequests(2)
                .refillRate(10000)
                .build()
        );
        limiter.tryAcquire();
        limiter.tryAcquire();
        assertFalse(limiter.tryAcquire().isAllowed());
        Thread.sleep(200);
        assertTrue(limiter.getRemainingTokens() > 0);
    }
}
