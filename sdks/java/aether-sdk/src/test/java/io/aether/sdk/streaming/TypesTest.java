package io.aether.sdk.streaming;

import io.aether.sdk.streaming.Types.*;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Instant;
import java.time.LocalDateTime;
import java.util.Map;
import java.util.Optional;

class TypesTest {

    @Test
    @DisplayName("Timestamp now returns current time")
    void testTimestampNow() {
        Timestamp ts = Timestamp.now();
        assertTrue(ts.getMilliseconds() > 0);
    }

    @Test
    @DisplayName("Timestamp fromMillis")
    void testTimestampFromMillis() {
        Timestamp ts = Timestamp.fromMillis(1234567890L);
        assertEquals(1234567890L, ts.getMilliseconds());
    }

    @Test
    @DisplayName("Timestamp fromSeconds")
    void testTimestampFromSeconds() {
        Timestamp ts = Timestamp.fromSeconds(1000.5);
        assertEquals(1000500L, ts.getMilliseconds());
    }

    @Test
    @DisplayName("Timestamp fromInstant")
    void testTimestampFromInstant() {
        Instant now = Instant.now();
        Timestamp ts = Timestamp.fromInstant(now);
        assertEquals(now.toEpochMilli(), ts.getMilliseconds());
    }

    @Test
    @DisplayName("Timestamp getSeconds")
    void testTimestampGetSeconds() {
        Timestamp ts = Timestamp.fromMillis(2500);
        assertEquals(2.5, ts.getSeconds());
    }

    @Test
    @DisplayName("Timestamp toInstant")
    void testTimestampToInstant() {
        Timestamp ts = Timestamp.fromMillis(1234567890L);
        assertEquals(1234567890L, ts.toInstant().toEpochMilli());
    }

    @Test
    @DisplayName("Timestamp arithmetic")
    void testTimestampArithmetic() {
        Timestamp ts = Timestamp.fromMillis(10000);
        Timestamp plus = ts.plus(Duration.fromMillis(5000));
        Timestamp minus = ts.minus(Duration.fromMillis(3000));
        assertEquals(15000, plus.getMilliseconds());
        assertEquals(7000, minus.getMilliseconds());
        Duration diff = ts.minus(Timestamp.fromMillis(3000));
        assertEquals(7000, diff.getMilliseconds());
    }

    @Test
    @DisplayName("Timestamp compareTo")
    void testTimestampCompareTo() {
        Timestamp a = Timestamp.fromMillis(100);
        Timestamp b = Timestamp.fromMillis(200);
        assertTrue(a.compareTo(b) < 0);
        assertTrue(b.compareTo(a) > 0);
        assertEquals(0, a.compareTo(a));
    }

    @Test
    @DisplayName("Timestamp equals and hashCode")
    void testTimestampEquals() {
        Timestamp a = Timestamp.fromMillis(100);
        Timestamp b = Timestamp.fromMillis(100);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
    }

    @Test
    @DisplayName("Duration fromMillis")
    void testDurationFromMillis() {
        Duration d = Duration.fromMillis(5000);
        assertEquals(5000, d.getMilliseconds());
    }

    @Test
    @DisplayName("Duration fromSeconds")
    void testDurationFromSeconds() {
        Duration d = Duration.fromSeconds(2.5);
        assertEquals(2500, d.getMilliseconds());
    }

    @Test
    @DisplayName("Duration fromMinutes")
    void testDurationFromMinutes() {
        Duration d = Duration.fromMinutes(5);
        assertEquals(300000, d.getMilliseconds());
    }

    @Test
    @DisplayName("Duration fromHours")
    void testDurationFromHours() {
        Duration d = Duration.fromHours(1);
        assertEquals(3600000, d.getMilliseconds());
    }

    @Test
    @DisplayName("Duration arithmetic")
    void testDurationArithmetic() {
        Duration d = Duration.fromMillis(1000);
        assertEquals(1500, d.plus(Duration.fromMillis(500)).getMilliseconds());
        assertEquals(500, d.minus(Duration.fromMillis(500)).getMilliseconds());
        assertEquals(2000, d.multipliedBy(2).getMilliseconds());
        assertEquals(500, d.dividedBy(2).getMilliseconds());
    }

    @Test
    @DisplayName("Duration equals and hashCode")
    void testDurationEquals() {
        Duration a = Duration.fromMillis(100);
        Duration b = Duration.fromMillis(100);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
    }

    @Test
    @DisplayName("Duration toJavaDuration")
    void testDurationToJava() {
        Duration d = Duration.fromMillis(5000);
        assertEquals(java.time.Duration.ofMillis(5000), d.toJavaDuration());
    }

    @Test
    @DisplayName("StreamEvent create with key and value")
    void testStreamEventCreate() {
        StreamEvent<String> event = StreamEvent.create("k1", "v1");
        assertEquals("k1", event.getKey());
        assertEquals("v1", event.getValue());
        assertNotNull(event.getTimestamp());
    }

    @Test
    @DisplayName("StreamEvent builder with all fields")
    void testStreamEventBuilder() {
        StreamEvent<String> event = StreamEvent.<String>builder()
            .key("k")
            .value("v")
            .timestamp(Timestamp.fromMillis(1000))
            .header("h1", "v1")
            .partition(3)
            .offset(42L)
            .eventType("click")
            .build();
        assertEquals("k", event.getKey());
        assertEquals("v", event.getValue());
        assertEquals(1000, event.getTimestamp().getMilliseconds());
        assertEquals("v1", event.getHeaders().get("h1"));
        assertEquals(3, event.getPartition().orElse(-1));
        assertEquals(42L, event.getOffset().orElse(-1L));
        assertEquals("click", event.getEventType().orElse(""));
    }

    @Test
    @DisplayName("StreamEvent withValue creates copy")
    void testStreamEventWithValue() {
        StreamEvent<String> event = StreamEvent.create("k", "v1");
        StreamEvent<Integer> event2 = event.withValue(42);
        assertEquals("k", event2.getKey());
        assertEquals(42, event2.getValue());
    }

    @Test
    @DisplayName("StreamEvent withTimestamp creates copy")
    void testStreamEventWithTimestamp() {
        StreamEvent<String> event = StreamEvent.create("k", "v");
        StreamEvent<String> event2 = event.withTimestamp(Timestamp.fromMillis(9999));
        assertEquals(9999, event2.getTimestamp().getMilliseconds());
    }

    @Test
    @DisplayName("StreamEvent builder requires key and value")
    void testStreamEventBuilderRequired() {
        assertThrows(NullPointerException.class, () -> StreamEvent.builder().build());
    }

    @Test
    @DisplayName("Watermark isLate")
    void testWatermarkIsLate() {
        Watermark wm = new Watermark(Timestamp.fromMillis(1000), "stream-1");
        assertTrue(wm.isLate(Timestamp.fromMillis(500)));
        assertFalse(wm.isLate(Timestamp.fromMillis(1500)));
        assertFalse(wm.isLate(Timestamp.fromMillis(1000)));
    }

    @Test
    @DisplayName("Watermark getters")
    void testWatermarkGetters() {
        Watermark wm = new Watermark(Timestamp.fromMillis(1000), "s1", 3);
        assertEquals("s1", wm.getStreamId());
        assertEquals(3, wm.getPartition().orElse(-1));
    }

    @Test
    @DisplayName("StreamConfig builder with defaults")
    void testStreamConfigDefaults() {
        StreamConfig config = StreamConfig.builder().build();
        assertTrue(config.getInputStreams().isEmpty());
        assertTrue(config.getOutputStreams().isEmpty());
        assertEquals(1, config.getParallelism());
        assertEquals(WatermarkStrategy.PROCESSING_TIME, config.getWatermarkStrategy());
        assertFalse(config.isCheckpointingEnabled());
        assertEquals(10000, config.getBufferCapacity());
    }

    @Test
    @DisplayName("StreamConfig builder with custom values")
    void testStreamConfigCustom() {
        StreamConfig config = StreamConfig.builder()
            .inputStreams(List.of("in"))
            .outputStreams(List.of("out"))
            .parallelism(4)
            .checkpointingEnabled(true)
            .lateDataPolicy(LateDataPolicy.SIDE_OUTPUT)
            .build();
        assertEquals(1, config.getInputStreams().size());
        assertEquals(4, config.getParallelism());
        assertTrue(config.isCheckpointingEnabled());
        assertEquals(LateDataPolicy.SIDE_OUTPUT, config.getLateDataPolicy());
    }

    @Test
    @DisplayName("BackpressureConfig defaults")
    void testBackpressureConfigDefaults() {
        BackpressureConfig config = BackpressureConfig.defaultConfig();
        assertEquals(BackpressureStrategy.BUFFER, config.getStrategy());
        assertEquals(10000, config.getBufferSize());
        assertEquals(0.9, config.getHighWatermark());
        assertEquals(0.5, config.getLowWatermark());
    }
}
