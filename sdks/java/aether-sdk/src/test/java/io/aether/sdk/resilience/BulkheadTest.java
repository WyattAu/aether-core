package io.aether.sdk.resilience;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.concurrent.*;

class BulkheadTest {

    private Bulkhead bulkhead;

    @BeforeEach
    void setUp() {
        bulkhead = new Bulkhead(
            Bulkhead.BulkheadConfig.builder()
                .maxConcurrent(2)
                .maxQueued(0)
                .name("test")
                .build()
        );
    }

    @Test
    @DisplayName("hasCapacity when not at limit")
    void testHasCapacity() {
        assertTrue(bulkhead.hasCapacity());
    }

    @Test
    @DisplayName("execute succeeds when capacity available")
    void testExecuteSuccess() {
        CompletableFuture<String> result = bulkhead.execute(() ->
            CompletableFuture.completedFuture("ok"));
        assertEquals("ok", result.join());
    }

    @Test
    @DisplayName("execute rejects when at capacity")
    void testExecuteRejectsAtCapacity() {
        CompletableFuture<Void> blocker = new CompletableFuture<>();
        bulkhead.execute(() -> blocker);
        bulkhead.execute(() -> blocker);

        CompletableFuture<String> result = bulkhead.execute(() ->
            CompletableFuture.completedFuture("third"));
        ExecutionException ex = assertThrows(ExecutionException.class, result::get);
        assertTrue(ex.getCause() instanceof Bulkhead.BulkheadRejectedException);

        blocker.complete(null);
    }

    @Test
    @DisplayName("availablePermits starts at maxConcurrent")
    void testAvailablePermits() {
        assertEquals(2, bulkhead.availablePermits());
    }

    @Test
    @DisplayName("getStats returns current statistics")
    void testGetStats() {
        bulkhead.execute(() -> CompletableFuture.completedFuture("ok")).join();
        Bulkhead.BulkheadStats stats = bulkhead.getStats();
        assertTrue(stats.getAccepted() >= 1);
    }

    @Test
    @DisplayName("default config values")
    void testDefaultConfig() {
        Bulkhead.BulkheadConfig config = Bulkhead.BulkheadConfig.defaultConfig();
        assertEquals("default", config.getName());
        assertEquals(10, config.getMaxConcurrent());
        assertEquals(0, config.getMaxQueued());
        assertEquals(0, config.getQueueTimeoutMs());
    }

    @Test
    @DisplayName("BulkheadRejectedException contains details")
    void testBulkheadRejectedException() {
        Bulkhead.BulkheadRejectedException ex =
            new Bulkhead.BulkheadRejectedException("my-bh", 10, 10);
        assertEquals("my-bh", ex.getBulkheadName());
        assertEquals(10, ex.getActiveCalls());
        assertEquals(10, ex.getMaxConcurrent());
        assertTrue(ex.getMessage().contains("my-bh"));
    }

    @Test
    @DisplayName("reset clears counters")
    void testReset() {
        bulkhead.execute(() -> CompletableFuture.completedFuture("ok")).join();
        bulkhead.reset();
        Bulkhead.BulkheadStats stats = bulkhead.getStats();
        assertEquals(1, stats.getActive());
    }

    @Test
    @DisplayName("getConfig returns config")
    void testGetConfig() {
        assertEquals("test", bulkhead.getConfig().getName());
        assertEquals(2, bulkhead.getConfig().getMaxConcurrent());
    }

    @Test
    @DisplayName("execute releases permit after completion")
    void testExecuteReleasesPermit() {
        int before = bulkhead.availablePermits();
        bulkhead.execute(() -> CompletableFuture.completedFuture("ok")).join();
        assertEquals(before, bulkhead.availablePermits());
    }

    @Test
    @DisplayName("execute releases permit on failure")
    void testExecuteReleasesPermitOnFailure() {
        int before = bulkhead.availablePermits();
        CompletableFuture<String> result = bulkhead.execute(() -> {
            CompletableFuture<String> f = new CompletableFuture<>();
            f.completeExceptionally(new RuntimeException("fail"));
            return f;
        });
        assertThrows(ExecutionException.class, result::get);
        assertEquals(before, bulkhead.availablePermits());
    }

    @Test
    @DisplayName("builder sets all fields")
    void testBuilder() {
        Bulkhead.BulkheadConfig config = Bulkhead.BulkheadConfig.builder()
            .name("custom")
            .maxConcurrent(5)
            .maxQueued(10)
            .queueTimeoutMs(5000)
            .build();
        assertEquals("custom", config.getName());
        assertEquals(5, config.getMaxConcurrent());
        assertEquals(10, config.getMaxQueued());
        assertEquals(5000, config.getQueueTimeoutMs());
    }

    @Test
    @DisplayName("stats getters return correct values")
    void testBulkheadStatsGetters() {
        Bulkhead.BulkheadStats stats = new Bulkhead.BulkheadStats(3, 1, 7, 2, 15);
        assertEquals(3, stats.getActive());
        assertEquals(1, stats.getQueued());
        assertEquals(7, stats.getAvailable());
        assertEquals(2, stats.getRejected());
        assertEquals(15, stats.getAccepted());
    }
}
