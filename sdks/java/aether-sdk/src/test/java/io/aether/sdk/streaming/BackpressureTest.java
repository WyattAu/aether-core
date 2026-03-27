package io.aether.sdk.streaming;

import io.aether.sdk.streaming.Types.BackpressureConfig;
import io.aether.sdk.streaming.Types.BackpressureStrategy;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.Objects;

class BackpressureTest {

    @Test
    @DisplayName("accepted result factory")
    void testAcceptedResult() {
        Backpressure.BackpressureResult<String> result =
            Backpressure.BackpressureResult.accepted("item");
        assertTrue(result.isAccepted());
        assertTrue(result.getElement().isPresent());
        assertEquals("item", result.getElement().get());
        assertFalse(result.getDroppedElement().isPresent());
        assertFalse(result.getReason().isPresent());
        assertEquals(0, result.getWaitTimeMs());
    }

    @Test
    @DisplayName("dropped result factory")
    void testDroppedResult() {
        Backpressure.BackpressureResult<String> result =
            Backpressure.BackpressureResult.dropped("item", "buffer_full");
        assertFalse(result.isAccepted());
        assertFalse(result.getElement().isPresent());
        assertTrue(result.getDroppedElement().isPresent());
        assertEquals("buffer_full", result.getReason().orElse(""));
    }

    @Test
    @DisplayName("failed result factory")
    void testFailedResult() {
        Backpressure.BackpressureResult<String> result =
            Backpressure.BackpressureResult.failed("item", "error");
        assertFalse(result.isAccepted());
        assertEquals("error", result.getReason().orElse(""));
    }

    @Test
    @DisplayName("throttled result factory")
    void testThrottledResult() {
        Backpressure.BackpressureResult<String> result =
            Backpressure.BackpressureResult.throttled("item", 500);
        assertFalse(result.isAccepted());
        assertEquals(500, result.getWaitTimeMs());
        assertEquals("throttled", result.getReason().orElse(""));
    }

    @Test
    @DisplayName("controller accepts when buffer has space")
    void testControllerAccepts() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder().bufferSize(5).build()
            );
        var result = ctrl.process("item1");
        assertTrue(result.isAccepted());
        assertEquals(1, ctrl.size());
    }

    @Test
    @DisplayName("controller drops with DROP strategy when full")
    void testControllerDropStrategy() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder()
                    .strategy(BackpressureStrategy.DROP)
                    .bufferSize(2)
                    .highWatermark(1.0)
                    .lowWatermark(0.0)
                    .build()
            );
        ctrl.process("a");
        ctrl.process("b");
        var result = ctrl.process("c");
        assertFalse(result.isAccepted());
        assertEquals("buffer_full", result.getReason().orElse(""));
    }

    @Test
    @DisplayName("controller fails with FAIL strategy when full")
    void testControllerFailStrategy() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder()
                    .strategy(BackpressureStrategy.FAIL)
                    .bufferSize(1)
                    .highWatermark(1.0)
                    .lowWatermark(0.0)
                    .build()
            );
        ctrl.process("a");
        assertThrows(Backpressure.BackpressureException.class, () -> ctrl.process("b"));
    }

    @Test
    @DisplayName("controller LATEST strategy replaces oldest")
    void testControllerLatestStrategy() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder()
                    .strategy(BackpressureStrategy.LATEST)
                    .bufferSize(2)
                    .highWatermark(1.0)
                    .lowWatermark(0.0)
                    .build()
            );
        ctrl.process("old1");
        ctrl.process("old2");
        var result = ctrl.process("new");
        assertTrue(result.isAccepted());
        assertEquals(2, ctrl.size());
    }

    @Test
    @DisplayName("controller poll removes element")
    void testControllerPoll() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder().bufferSize(5).build()
            );
        ctrl.process("a");
        var elem = ctrl.poll();
        assertTrue(elem.isPresent());
        assertEquals("a", elem.get());
        assertTrue(ctrl.isEmpty());
    }

    @Test
    @DisplayName("controller peek does not remove")
    void testControllerPeek() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder().bufferSize(5).build()
            );
        ctrl.process("a");
        var peeked = ctrl.peek();
        assertTrue(peeked.isPresent());
        assertEquals(1, ctrl.size());
    }

    @Test
    @DisplayName("controller clear empties buffer")
    void testControllerClear() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder().bufferSize(5).build()
            );
        ctrl.process("a");
        ctrl.process("b");
        ctrl.clear();
        assertTrue(ctrl.isEmpty());
    }

    @Test
    @DisplayName("controller isFull and isEmpty")
    void testControllerFullEmpty() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder().bufferSize(1).build()
            );
        assertTrue(ctrl.isEmpty());
        assertFalse(ctrl.isFull());
        ctrl.process("a");
        assertFalse(ctrl.isEmpty());
        assertTrue(ctrl.isFull());
    }

    @Test
    @DisplayName("controller rejects null element")
    void testControllerRejectsNull() {
        Backpressure.BackpressureController<String> ctrl =
            new Backpressure.BackpressureController<>(
                BackpressureConfig.builder().bufferSize(5).build()
            );
        assertThrows(NullPointerException.class, () -> ctrl.process(null));
    }

    @Test
    @DisplayName("multi-level backpressure add and poll")
    void testMultiLevel() {
        Backpressure.MultiLevelBackpressure<String> ml =
            new Backpressure.MultiLevelBackpressure<>(3, 10);
        assertTrue(ml.add("high", 0));
        assertTrue(ml.add("mid", 1));
        assertTrue(ml.add("low", 2));
        assertEquals(3, ml.size());

        var first = ml.poll();
        assertTrue(first.isPresent());
        assertEquals("high", first.get());
    }

    @Test
    @DisplayName("multi-level rejects invalid level")
    void testMultiLevelInvalidLevel() {
        Backpressure.MultiLevelBackpressure<String> ml =
            new Backpressure.MultiLevelBackpressure<>(3, 10);
        assertThrows(IllegalArgumentException.class, () -> ml.add("x", 5));
    }

    @Test
    @DisplayName("multi-level constructor rejects bad level count")
    void testMultiLevelBadLevelCount() {
        assertThrows(IllegalArgumentException.class, () ->
            new Backpressure.MultiLevelBackpressure<>(0, 10));
        assertThrows(IllegalArgumentException.class, () ->
            new Backpressure.MultiLevelBackpressure<>(11, 10));
    }

    @Test
    @DisplayName("BackpressureStats calculations")
    void testBackpressureStats() {
        Backpressure.BackpressureStats stats = new Backpressure.BackpressureStats(
            100, 80, 15, 5, 50, 100, true, 1000L, 500L);
        assertEquals(100, stats.getTotalReceived());
        assertEquals(80, stats.getTotalAccepted());
        assertEquals(15, stats.getTotalDropped());
        assertEquals(5, stats.getTotalFailed());
        assertEquals(0.5, stats.getBufferUtilization());
        assertTrue(stats.isInBackpressure());
        assertEquals(0.15, stats.getDropRate(), 0.001);
        assertEquals(0.8, stats.getAcceptRate(), 0.001);
    }

    @Test
    @DisplayName("rate-based backpressure tryAcquire")
    void testRateBasedTryAcquire() {
        Backpressure.RateBasedBackpressure rb =
            new Backpressure.RateBasedBackpressure(100);
        var result = rb.tryAcquire();
        assertTrue(result.isAllowed());
    }

    @Test
    @DisplayName("BackpressureException with timestamp")
    void testBackpressureException() {
        Backpressure.BackpressureException ex =
            new Backpressure.BackpressureException("overflow");
        assertTrue(ex.getMessage().contains("overflow"));
        assertTrue(ex.getTimestamp() > 0);
    }
}
