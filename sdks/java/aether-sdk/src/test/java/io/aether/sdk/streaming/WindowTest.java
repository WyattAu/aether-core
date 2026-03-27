package io.aether.sdk.streaming;

import io.aether.sdk.streaming.Types.*;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.List;
import java.util.function.BiFunction;

class WindowTest {

    @Test
    @DisplayName("WindowState accepts events in range")
    void testWindowStateAcceptEvent() {
        Timestamp start = Timestamp.fromMillis(0);
        Timestamp end = Timestamp.fromMillis(1000);
        Window.WindowState<String, String> ws =
            new Window.WindowState<>("w1", "k", start, end);
        StreamEvent<String> event = StreamEvent.create("k", "v",
            Timestamp.fromMillis(500));
        assertTrue(ws.addEvent(event));
        assertEquals(1, ws.getEvents().size());
    }

    @Test
    @DisplayName("WindowState rejects events before window")
    void testWindowStateRejectsBefore() {
        Timestamp start = Timestamp.fromMillis(1000);
        Timestamp end = Timestamp.fromMillis(2000);
        Window.WindowState<String, String> ws =
            new Window.WindowState<>("w1", "k", start, end);
        StreamEvent<String> event = StreamEvent.create("k", "v",
            Timestamp.fromMillis(500));
        assertFalse(ws.addEvent(event));
    }

    @Test
    @DisplayName("WindowState rejects events at or after end")
    void testWindowStateRejectsAtEnd() {
        Timestamp start = Timestamp.fromMillis(0);
        Timestamp end = Timestamp.fromMillis(1000);
        Window.WindowState<String, String> ws =
            new Window.WindowState<>("w1", "k", start, end);
        StreamEvent<String> event = StreamEvent.create("k", "v",
            Timestamp.fromMillis(1000));
        assertFalse(ws.addEvent(event));
    }

    @Test
    @DisplayName("WindowState rejects events when closed")
    void testWindowStateClosed() {
        Timestamp start = Timestamp.fromMillis(0);
        Timestamp end = Timestamp.fromMillis(1000);
        Window.WindowState<String, String> ws =
            new Window.WindowState<>("w1", "k", start, end);
        ws.clear();
        assertFalse(ws.addEvent(StreamEvent.create("k", "v")));
        assertTrue(ws.isClosed());
    }

    @Test
    @DisplayName("WindowState isEmpty")
    void testWindowStateIsEmpty() {
        Window.WindowState<String, String> ws =
            new Window.WindowState<>("w1", "k",
                Timestamp.fromMillis(0), Timestamp.fromMillis(1000));
        assertTrue(ws.isEmpty());
        ws.addEvent(StreamEvent.create("k", "v"));
        assertFalse(ws.isEmpty());
    }

    @Test
    @DisplayName("WindowState tracks maxTimestamp")
    void testWindowStateMaxTimestamp() {
        Timestamp start = Timestamp.fromMillis(0);
        Timestamp end = Timestamp.fromMillis(10000);
        Window.WindowState<String, String> ws =
            new Window.WindowState<>("w1", "k", start, end);
        ws.addEvent(StreamEvent.create("k", "a", Timestamp.fromMillis(100)));
        ws.addEvent(StreamEvent.create("k", "b", Timestamp.fromMillis(500)));
        assertTrue(ws.getMaxTimestamp().isPresent());
        assertEquals(500, ws.getMaxTimestamp().get().getMilliseconds());
    }

    @Test
    @DisplayName("WindowAssigner assigns tumbling window")
    void testTumblingAssigner() {
        WindowSpec spec = WindowSpec.tumbling(Duration.fromMillis(1000));
        Window.WindowAssigner<String, String> assigner =
            new Window.WindowAssigner<>(spec);
        Timestamp ts = Timestamp.fromMillis(1500);
        List<Window.WindowState<String, String>> windows =
            assigner.assign(StreamEvent.create("k", "v", ts), "k");
        assertEquals(1, windows.size());
        assertEquals(1000, windows.get(0).getStart().getMilliseconds());
        assertEquals(2000, windows.get(0).getEnd().getMilliseconds());
    }

    @Test
    @DisplayName("WindowAssigner assigns session window")
    void testSessionAssigner() {
        WindowSpec spec = WindowSpec.session(Duration.fromMillis(500));
        Window.WindowAssigner<String, String> assigner =
            new Window.WindowAssigner<>(spec);
        Timestamp ts = Timestamp.fromMillis(1000);
        List<Window.WindowState<String, String>> windows =
            assigner.assign(StreamEvent.create("k", "v", ts), "k");
        assertEquals(1, windows.size());
        assertEquals(1, windows.get(0).getEvents().size());
    }

    @Test
    @DisplayName("WindowAssigner sliding window")
    void testSlidingAssigner() {
        WindowSpec spec = WindowSpec.sliding(Duration.fromMillis(1000), Duration.fromMillis(500));
        Window.WindowAssigner<String, String> assigner =
            new Window.WindowAssigner<>(spec);
        Timestamp ts = Timestamp.fromMillis(750);
        List<Window.WindowState<String, String>> windows =
            assigner.assign(StreamEvent.create("k", "v", ts), "k");
        assertTrue(windows.size() >= 1);
    }

    @Test
    @DisplayName("WindowAssigner getTriggeredWindows")
    void testGetTriggeredWindows() {
        WindowSpec spec = WindowSpec.tumbling(Duration.fromMillis(1000));
        Window.WindowAssigner<String, String> assigner =
            new Window.WindowAssigner<>(spec);
        assigner.assign(StreamEvent.create("k", "v",
            Timestamp.fromMillis(500)), "k");
        List<Window.WindowState<String, String>> triggered =
            assigner.getTriggeredWindows(Timestamp.fromMillis(1500));
        assertEquals(1, triggered.size());
        assertTrue(triggered.get(0).isOnTimeFired());
    }

    @Test
    @DisplayName("WindowAssigner cleanupClosed")
    void testCleanupClosed() {
        WindowSpec spec = WindowSpec.tumbling(Duration.fromMillis(1000));
        Window.WindowAssigner<String, String> assigner =
            new Window.WindowAssigner<>(spec);
        assigner.assign(StreamEvent.create("k", "v",
            Timestamp.fromMillis(500)), "k");
        assigner.getTriggeredWindows(Timestamp.fromMillis(2000));
        int removed = assigner.cleanupClosed();
        assertEquals(0, removed);
    }

    @Test
    @DisplayName("TumblingWindow convenience class")
    void testTumblingWindowConvenience() {
        BiFunction<List<StreamEvent<String>>, WindowInfo, Integer> handler =
            (events, info) -> events.size();
        Window.TumblingWindow<String, String, Integer> tw =
            new Window.TumblingWindow<>(Duration.fromMillis(1000), handler);
        Timestamp ts = Timestamp.fromMillis(500);
        List<Integer> results = tw.process(StreamEvent.create("k", "v", ts), "k");
        assertTrue(results.isEmpty());
        List<Integer> triggered = tw.advanceWatermark(Timestamp.fromMillis(1500));
        assertEquals(1, triggered.size());
        assertEquals(1, triggered.get(0));
    }

    @Test
    @DisplayName("SessionWindow convenience class")
    void testSessionWindowConvenience() {
        BiFunction<List<StreamEvent<String>>, WindowInfo, Integer> handler =
            (events, info) -> events.size();
        Window.SessionWindow<String, String, Integer> sw =
            new Window.SessionWindow<>(Duration.fromMillis(5000), handler);
        List<Integer> results = sw.process(
            StreamEvent.create("k", "v1", Timestamp.fromMillis(1000)), "k");
        assertTrue(results.isEmpty());
    }

    @Test
    @DisplayName("WindowSpec tumbling factory")
    void testWindowSpecTumbling() {
        WindowSpec spec = WindowSpec.tumbling(Duration.fromMillis(5000));
        assertEquals(WindowType.TUMBLING, spec.getType());
        assertEquals(5000, spec.getSize().getMilliseconds());
        assertFalse(spec.getSlide().isPresent());
    }

    @Test
    @DisplayName("WindowSpec sliding factory")
    void testWindowSpecSliding() {
        WindowSpec spec = WindowSpec.sliding(
            Duration.fromMillis(10000), Duration.fromMillis(1000));
        assertEquals(WindowType.SLIDING, spec.getType());
        assertTrue(spec.getSlide().isPresent());
        assertEquals(1000, spec.getSlide().get().getMilliseconds());
    }

    @Test
    @DisplayName("WindowSpec session factory")
    void testWindowSpecSession() {
        WindowSpec spec = WindowSpec.session(Duration.fromMinutes(5));
        assertEquals(WindowType.SESSION, spec.getType());
        assertTrue(spec.getGap().isPresent());
    }

    @Test
    @DisplayName("WindowSpec sliding without slide throws")
    void testWindowSpecSlidingNoSlide() {
        assertThrows(IllegalArgumentException.class, () ->
            WindowSpec.builder()
                .type(WindowType.SLIDING)
                .size(Duration.fromMillis(1000))
                .build());
    }

    @Test
    @DisplayName("WindowSpec session without gap throws")
    void testWindowSpecSessionNoGap() {
        assertThrows(IllegalArgumentException.class, () ->
            WindowSpec.builder()
                .type(WindowType.SESSION)
                .size(Duration.fromMillis(1000))
                .build());
    }

    @Test
    @DisplayName("WindowInfo contains timestamp")
    void testWindowInfoContains() {
        WindowInfo info = new WindowInfo(
            Timestamp.fromMillis(0),
            Timestamp.fromMillis(1000),
            Timestamp.fromMillis(500),
            PaneInfo.ON_TIME);
        assertTrue(info.contains(Timestamp.fromMillis(500)));
        assertFalse(info.contains(Timestamp.fromMillis(1000)));
        assertTrue(info.isLate(Timestamp.fromMillis(-1)));
    }
}
