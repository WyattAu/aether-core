package io.aether.sdk.resilience;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Duration;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BiConsumer;

class CircuitBreakerTest {

    private CircuitBreaker breaker;

    @BeforeEach
    void setUp() {
        breaker = new CircuitBreaker();
    }

    @Test
    @DisplayName("starts in CLOSED state")
    void testInitialState() {
        assertEquals(CircuitState.CLOSED, breaker.getState());
    }

    @Test
    @DisplayName("recordSuccess keeps CLOSED")
    void testSuccessKeepsClosed() {
        breaker.recordSuccess();
        assertEquals(CircuitState.CLOSED, breaker.getState());
    }

    @Test
    @DisplayName("transitions OPEN after failure threshold")
    void testTransitionsToOpen() {
        CircuitBreaker cb = new CircuitBreaker(
            CircuitBreaker.CircuitBreakerConfig.builder()
                .failureThreshold(3)
                .successThreshold(2)
                .resetTimeout(Duration.ofMillis(100))
                .build()
        );
        cb.recordFailure();
        cb.recordFailure();
        assertEquals(CircuitState.CLOSED, cb.getState());
        cb.recordFailure();
        assertEquals(CircuitState.OPEN, cb.getState());
    }

    @Test
    @DisplayName("execute succeeds when CLOSED")
    void testExecuteWhenClosed() {
        CompletableFuture<String> result = breaker.execute(() ->
            CompletableFuture.completedFuture("ok"));
        assertEquals("ok", result.join());
    }

    @Test
    @DisplayName("execute fails immediately when OPEN")
    void testExecuteWhenOpen() {
        CircuitBreaker cb = new CircuitBreaker(
            CircuitBreaker.CircuitBreakerConfig.builder()
                .failureThreshold(1)
                .resetTimeout(Duration.ofSeconds(60))
                .build()
        );
        cb.recordFailure();
        assertEquals(CircuitState.OPEN, cb.getState());

        CompletableFuture<String> result = cb.execute(() ->
            CompletableFuture.completedFuture("ok"));
        ExecutionException ex = assertThrows(ExecutionException.class, result::get);
        assertTrue(ex.getCause() instanceof CircuitBreaker.CircuitBreakerException);
    }

    @Test
    @DisplayName("execute records failure on exception")
    void testExecuteRecordsFailure() {
        CircuitBreaker cb = new CircuitBreaker(
            CircuitBreaker.CircuitBreakerConfig.builder()
                .failureThreshold(1)
                .resetTimeout(Duration.ofSeconds(60))
                .build()
        );
        CompletableFuture<String> result = cb.execute(() -> {
            throw new RuntimeException("boom");
        });
        assertThrows(ExecutionException.class, result::get);
        assertEquals(CircuitState.OPEN, cb.getState());
    }

    @Test
    @DisplayName("reset returns to CLOSED")
    void testReset() {
        CircuitBreaker cb = new CircuitBreaker(
            CircuitBreaker.CircuitBreakerConfig.builder()
                .failureThreshold(1)
                .resetTimeout(Duration.ofSeconds(60))
                .build()
        );
        cb.recordFailure();
        assertEquals(CircuitState.OPEN, cb.getState());
        cb.reset();
        assertEquals(CircuitState.CLOSED, cb.getState());
    }

    @Test
    @DisplayName("forceState changes state")
    void testForceState() {
        breaker.forceState(CircuitState.OPEN);
        assertEquals(CircuitState.OPEN, breaker.getState());
        breaker.forceState(CircuitState.HALF_OPEN);
        assertEquals(CircuitState.HALF_OPEN, breaker.getState());
        breaker.forceState(CircuitState.CLOSED);
        assertEquals(CircuitState.CLOSED, breaker.getState());
    }

    @Test
    @DisplayName("getStats returns correct values")
    void testGetStats() {
        breaker.recordSuccess();
        breaker.recordSuccess();
        CircuitBreaker.CircuitBreakerStats stats = breaker.getStats();
        assertEquals(CircuitState.CLOSED, stats.getState());
        assertTrue(stats.getSuccessCount() >= 2);
    }

    @Test
    @DisplayName("onStateChange callback fires")
    void testOnStateChangeCallback() {
        AtomicReference<CircuitState> fromRef = new AtomicReference<>();
        AtomicReference<CircuitState> toRef = new AtomicReference<>();

        CircuitBreaker cb = new CircuitBreaker(
            CircuitBreaker.CircuitBreakerConfig.builder()
                .failureThreshold(1)
                .resetTimeout(Duration.ofSeconds(60))
                .onStateChange((from, to) -> {
                    fromRef.set(from);
                    toRef.set(to);
                })
                .build()
        );
        cb.recordFailure();
        assertEquals(CircuitState.CLOSED, fromRef.get());
        assertEquals(CircuitState.OPEN, toRef.get());
    }

    @Test
    @DisplayName("CircuitBreakerException contains name and state")
    void testCircuitBreakerException() {
        CircuitBreaker.CircuitBreakerException ex =
            new CircuitBreaker.CircuitBreakerException("my-breaker", CircuitState.OPEN);
        assertEquals("my-breaker", ex.getBreakerName());
        assertEquals(CircuitState.OPEN, ex.getBreakerState());
        assertTrue(ex.getMessage().contains("my-breaker"));
    }

    @Test
    @DisplayName("half-open failure returns to OPEN")
    void testHalfOpenFailure() {
        CircuitBreaker cb = new CircuitBreaker(
            CircuitBreaker.CircuitBreakerConfig.builder()
                .failureThreshold(1)
                .successThreshold(2)
                .resetTimeout(Duration.ofSeconds(60))
                .build()
        );
        cb.recordFailure();
        assertEquals(CircuitState.OPEN, cb.getState());
        cb.forceState(CircuitState.HALF_OPEN);
        cb.recordFailure();
        assertEquals(CircuitState.OPEN, cb.getState());
    }

    @Test
    @DisplayName("default config values")
    void testDefaultConfig() {
        CircuitBreaker.CircuitBreakerConfig config = CircuitBreaker.CircuitBreakerConfig.defaultConfig();
        assertEquals("default", config.name);
        assertEquals(5, config.failureThreshold);
        assertEquals(3, config.successThreshold);
        assertEquals(Duration.ofSeconds(30), config.resetTimeout);
    }
}
