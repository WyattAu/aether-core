package io.aether.sdk.streaming;

import java.util.*;
import java.util.function.*;

import io.aether.sdk.streaming.Types.*;

/**
 * Windowing functions for stream processing.
 *
 * <p>This class provides time-based windowing capabilities:
 * <ul>
 *   <li>{@link TumblingWindow} - Fixed-size, non-overlapping windows</li>
 *   <li>{@link SlidingWindow} - Fixed-size, overlapping windows</li>
 *   <li>{@link SessionWindow} - Dynamic size based on activity gaps</li>
 * </ul>
 *
 * <p>Example:
 * <pre>{@code
 * // Create a tumbling window of 5 minutes
 * TumblingWindow<String, Event> window = new TumblingWindow<>(
 *     Duration.fromMinutes(5),
 *     this::processWindow
 * );
 *
 * // Process events
 * List<Result> results = window.process(event, event.getKey());
 *
 * // Advance watermark to trigger completed windows
 * List<Result> triggered = window.advanceWatermark(Timestamp.now());
 * }</pre>
 */
public final class Window {

    private Window() {
    }

    /**
     * State for a single window.
     *
     * @param <K> the key type
     * @param <V> the value type
     */
    public static final class WindowState<K, V> {
        private final String windowId;
        private final K key;
        private final Timestamp start;
        private final Timestamp end;
        private final List<StreamEvent<V>> events;
        private Timestamp maxTimestamp;
        private boolean isClosed;
        private boolean earlyFired;
        private boolean onTimeFired;

        public WindowState(String windowId, K key, Timestamp start, Timestamp end) {
            this.windowId = Objects.requireNonNull(windowId);
            this.key = key;
            this.start = Objects.requireNonNull(start);
            this.end = Objects.requireNonNull(end);
            this.events = new ArrayList<>();
            this.maxTimestamp = null;
            this.isClosed = false;
            this.earlyFired = false;
            this.onTimeFired = false;
        }

        /**
         * Add event to window.
         *
         * @return true if event was added
         */
        public boolean addEvent(StreamEvent<V> event) {
            if (isClosed) {
                return false;
            }

            Timestamp eventTs = event.getTimestamp();
            if (eventTs.compareTo(start) < 0 || eventTs.compareTo(end) >= 0) {
                return false;
            }

            events.add(event);

            if (maxTimestamp == null || eventTs.compareTo(maxTimestamp) > 0) {
                maxTimestamp = eventTs;
            }

            return true;
        }

        /**
         * Check if window has no events.
         */
        public boolean isEmpty() {
            return events.isEmpty();
        }

        /**
         * Clear window events and mark as closed.
         */
        public void clear() {
            events.clear();
            isClosed = true;
        }

        public String getWindowId() {
            return windowId;
        }

        public K getKey() {
            return key;
        }

        public Timestamp getStart() {
            return start;
        }

        public Timestamp getEnd() {
            return end;
        }

        public List<StreamEvent<V>> getEvents() {
            return Collections.unmodifiableList(events);
        }

        public Optional<Timestamp> getMaxTimestamp() {
            return Optional.ofNullable(maxTimestamp);
        }

        public boolean isClosed() {
            return isClosed;
        }

        public boolean isEarlyFired() {
            return earlyFired;
        }

        public void setEarlyFired(boolean earlyFired) {
            this.earlyFired = earlyFired;
        }

        public boolean isOnTimeFired() {
            return onTimeFired;
        }

        public void setOnTimeFired(boolean onTimeFired) {
            this.onTimeFired = onTimeFired;
        }
    }

    /**
     * Assigns events to windows based on the window specification.
     *
     * @param <K> the key type
     * @param <V> the value type
     */
    public static final class WindowAssigner<K, V> {
        private final WindowSpec spec;
        private final Map<String, WindowState<K, V>> windows;
        private final Map<K, List<String>> keyWindows;

        public WindowAssigner(WindowSpec spec) {
            this.spec = Objects.requireNonNull(spec);
            this.windows = new HashMap<>();
            this.keyWindows = new HashMap<>();
        }

        /**
         * Assign event to one or more windows.
         */
        public List<WindowState<K, V>> assign(StreamEvent<V> event, K key) {
            List<WindowState<K, V>> result = new ArrayList<>();

            switch (spec.getType()) {
                case TUMBLING:
                    WindowState<K, V> tumbling = assignTumbling(event, key);
                    if (tumbling != null) {
                        result.add(tumbling);
                    }
                    break;
                case SLIDING:
                    result.addAll(assignSliding(event, key));
                    break;
                case SESSION:
                    WindowState<K, V> session = assignSession(event, key);
                    if (session != null) {
                        result.add(session);
                    }
                    break;
            }

            return result;
        }

        private WindowState<K, V> assignTumbling(StreamEvent<V> event, K key) {
            long sizeMs = spec.getSize().getMilliseconds();
            long startMs = (event.getTimestamp().getMilliseconds() / sizeMs) * sizeMs;
            long endMs = startMs + sizeMs;

            String windowId = key + "_" + startMs;

            WindowState<K, V> window = windows.get(windowId);
            if (window == null) {
                window = new WindowState<>(
                    windowId,
                    key,
                    Timestamp.fromMillis(startMs),
                    Timestamp.fromMillis(endMs)
                );
                windows.put(windowId, window);
                keyWindows.computeIfAbsent(key, k -> new ArrayList<>()).add(windowId);
            }

            window.addEvent(event);
            return window;
        }

        private List<WindowState<K, V>> assignSliding(StreamEvent<V> event, K key) {
            List<WindowState<K, V>> result = new ArrayList<>();
            long sizeMs = spec.getSize().getMilliseconds();
            long slideMs = spec.getSlide().orElseThrow().getMilliseconds();
            long eventTs = event.getTimestamp().getMilliseconds();

            long windowStart = (eventTs / slideMs) * slideMs;
            while (windowStart + sizeMs > eventTs && windowStart >= 0) {
                windowStart -= slideMs;
            }
            windowStart += slideMs;

            long currentStart = windowStart;
            while (currentStart <= eventTs) {
                String windowId = key + "_" + currentStart;

                WindowState<K, V> window = windows.get(windowId);
                if (window == null) {
                    window = new WindowState<>(
                        windowId,
                        key,
                        Timestamp.fromMillis(currentStart),
                        Timestamp.fromMillis(currentStart + sizeMs)
                    );
                    windows.put(windowId, window);
                    keyWindows.computeIfAbsent(key, k -> new ArrayList<>()).add(windowId);
                }

                if (window.addEvent(event)) {
                    result.add(window);
                }

                currentStart += slideMs;
            }

            return result;
        }

        private WindowState<K, V> assignSession(StreamEvent<V> event, K key) {
            long gapMs = spec.getGap().orElse(Duration.fromMillis(0)).getMilliseconds();
            long eventTs = event.getTimestamp().getMilliseconds();

            List<String> keyWindowIds = keyWindows.getOrDefault(key, Collections.emptyList());
            WindowState<K, V> mergedWindow = null;

            for (String windowId : new ArrayList<>(keyWindowIds)) {
                WindowState<K, V> window = windows.get(windowId);
                if (window == null || window.isClosed()) {
                    continue;
                }

                if (window.getMaxTimestamp().isPresent()) {
                    long timeDiff = Math.abs(eventTs - window.getMaxTimestamp().get().getMilliseconds());
                    if (timeDiff <= gapMs) {
                        if (mergedWindow == null) {
                            window.addEvent(event);
                            mergedWindow = window;
                        } else {
                            for (StreamEvent<V> evt : window.getEvents()) {
                                mergedWindow.addEvent(evt);
                            }
                            window.clear();
                        }
                    }
                }
            }

            if (mergedWindow != null) {
                return mergedWindow;
            }

            String windowId = key + "_session_" + eventTs;
            WindowState<K, V> window = new WindowState<>(
                windowId,
                key,
                Timestamp.fromMillis(eventTs),
                Timestamp.fromMillis(eventTs + gapMs + 1)
            );
            window.addEvent(event);
            windows.put(windowId, window);
            keyWindows.computeIfAbsent(key, k -> new ArrayList<>()).add(windowId);

            return window;
        }

        /**
         * Get windows ready to fire based on watermark.
         */
        public List<WindowState<K, V>> getTriggeredWindows(Timestamp watermark) {
            List<WindowState<K, V>> triggered = new ArrayList<>();

            for (WindowState<K, V> window : windows.values()) {
                if (window.isClosed()) {
                    continue;
                }

                if (window.getEnd().compareTo(watermark) <= 0) {
                    window.setOnTimeFired(true);
                    triggered.add(window);
                }
            }

            return triggered;
        }

        /**
         * Remove closed windows.
         *
         * @return count of windows removed
         */
        public int cleanupClosed() {
            List<String> toRemove = new ArrayList<>();
            for (Map.Entry<String, WindowState<K, V>> entry : windows.entrySet()) {
                if (entry.getValue().isClosed()) {
                    toRemove.add(entry.getKey());
                }
            }

            for (String windowId : toRemove) {
                windows.remove(windowId);
                for (List<String> windowIds : keyWindows.values()) {
                    windowIds.remove(windowId);
                }
            }

            return toRemove.size();
        }

        public WindowSpec getSpec() {
            return spec;
        }
    }

    /**
     * Triggers window firing with custom logic.
     *
     * @param <K> the key type
     * @param <V> the value type
     * @param <R> the result type
     */
    public static final class WindowTrigger<K, V, R> {
        private final WindowAssigner<K, V> assigner;
        private final BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler;
        private final Optional<Duration> earlyFiring;
        private final List<R> results;

        public WindowTrigger(
            WindowAssigner<K, V> assigner,
            BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler
        ) {
            this(assigner, handler, null);
        }

        public WindowTrigger(
            WindowAssigner<K, V> assigner,
            BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler,
            Duration earlyFiring
        ) {
            this.assigner = Objects.requireNonNull(assigner);
            this.handler = Objects.requireNonNull(handler);
            this.earlyFiring = Optional.ofNullable(earlyFiring);
            this.results = new ArrayList<>();
        }

        /**
         * Process event and return any triggered results.
         */
        public List<R> process(StreamEvent<V> event, K key) {
            List<R> triggeredResults = new ArrayList<>();
            List<WindowState<K, V>> assignedWindows = assigner.assign(event, key);

            if (earlyFiring.isPresent()) {
                long earlyFiringMs = earlyFiring.get().getMilliseconds();

                for (WindowState<K, V> window : assignedWindows) {
                    if (!window.isEarlyFired() && !window.isEmpty()) {
                        if (window.getMaxTimestamp().isPresent()) {
                            long elapsed = event.getTimestamp().getMilliseconds()
                                - window.getStart().getMilliseconds();
                            if (elapsed >= earlyFiringMs) {
                                R result = fireWindow(window, PaneInfo.EARLY);
                                if (result != null) {
                                    triggeredResults.add(result);
                                }
                                window.setEarlyFired(true);
                            }
                        }
                    }
                }
            }

            return triggeredResults;
        }

        /**
         * Advance watermark and fire completed windows.
         */
        public List<R> advanceWatermark(Timestamp watermark) {
            List<R> triggeredResults = new ArrayList<>();
            List<WindowState<K, V>> triggered = assigner.getTriggeredWindows(watermark);

            for (WindowState<K, V> window : triggered) {
                if (!window.isEmpty()) {
                    PaneInfo pane = window.isOnTimeFired() ? PaneInfo.LATE : PaneInfo.ON_TIME;
                    R result = fireWindow(window, pane);
                    if (result != null) {
                        triggeredResults.add(result);
                    }
                }
            }

            return triggeredResults;
        }

        private R fireWindow(WindowState<K, V> window, PaneInfo pane) {
            if (window.isEmpty()) {
                return null;
            }

            WindowInfo info = new WindowInfo(
                window.getStart(),
                window.getEnd(),
                window.getMaxTimestamp().orElse(window.getStart()),
                pane,
                window.getWindowId()
            );

            List<StreamEvent<V>> eventsCopy = new ArrayList<>(window.getEvents());
            return handler.apply(eventsCopy, info);
        }

        public WindowAssigner<K, V> getAssigner() {
            return assigner;
        }
    }

    /**
     * Convenience class for tumbling windows.
     *
     * <p>Tumbling windows are fixed-size, non-overlapping windows.
     *
     * <p>Example:
     * <pre>{@code
     * TumblingWindow<String, Event> window = new TumblingWindow<>(
     *     Duration.fromMinutes(5),
     *     (events, info) -> {
     *         // Process batch of events
     *         return new AggregateResult(...);
     *     }
     * );
     * }</pre>
     *
     * @param <K> the key type
     * @param <V> the value type
     * @param <R> the result type
     */
    public static final class TumblingWindow<K, V, R> {
        private final WindowAssigner<K, V> assigner;
        private final WindowTrigger<K, V, R> trigger;

        public TumblingWindow(Duration size, BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler) {
            this(size, handler, null);
        }

        public TumblingWindow(
            Duration size,
            BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler,
            Duration lateTolerance
        ) {
            WindowSpec spec = WindowSpec.builder()
                .type(WindowType.TUMBLING)
                .size(size)
                .lateTolerance(lateTolerance != null ? lateTolerance : Duration.fromMillis(0))
                .build();

            this.assigner = new WindowAssigner<>(spec);
            this.trigger = new WindowTrigger<>(assigner, handler);
        }

        /**
         * Process event.
         */
        public List<R> process(StreamEvent<V> event, K key) {
            return trigger.process(event, key);
        }

        /**
         * Advance watermark and trigger completed windows.
         */
        public List<R> advanceWatermark(Timestamp watermark) {
            return trigger.advanceWatermark(watermark);
        }

        public WindowAssigner<K, V> getAssigner() {
            return assigner;
        }
    }

    /**
     * Convenience class for sliding windows.
     *
     * <p>Sliding windows are fixed-size, overlapping windows.
     *
     * <p>Example:
     * <pre>{@code
     * SlidingWindow<String, Event> window = new SlidingWindow<>(
     *     Duration.fromMinutes(10),
     *     Duration.fromMinutes(1),
     *     (events, info) -> {
     *         // Process batch of events
     *         return new AggregateResult(...);
     *     }
     * );
     * }</pre>
     *
     * @param <K> the key type
     * @param <V> the value type
     * @param <R> the result type
     */
    public static final class SlidingWindow<K, V, R> {
        private final WindowAssigner<K, V> assigner;
        private final WindowTrigger<K, V, R> trigger;

        public SlidingWindow(
            Duration size,
            Duration slide,
            BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler
        ) {
            this(size, slide, handler, null);
        }

        public SlidingWindow(
            Duration size,
            Duration slide,
            BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler,
            Duration lateTolerance
        ) {
            WindowSpec spec = WindowSpec.builder()
                .type(WindowType.SLIDING)
                .size(size)
                .slide(slide)
                .lateTolerance(lateTolerance != null ? lateTolerance : Duration.fromMillis(0))
                .build();

            this.assigner = new WindowAssigner<>(spec);
            this.trigger = new WindowTrigger<>(assigner, handler);
        }

        /**
         * Process event.
         */
        public List<R> process(StreamEvent<V> event, K key) {
            return trigger.process(event, key);
        }

        /**
         * Advance watermark and trigger completed windows.
         */
        public List<R> advanceWatermark(Timestamp watermark) {
            return trigger.advanceWatermark(watermark);
        }

        public WindowAssigner<K, V> getAssigner() {
            return assigner;
        }
    }

    /**
     * Convenience class for session windows.
     *
     * <p>Session windows have dynamic size based on activity gaps.
     * A new window is created when the gap between events exceeds the configured gap duration.
     *
     * <p>Example:
     * <pre>{@code
     * SessionWindow<String, Event> window = new SessionWindow<>(
     *     Duration.fromMinutes(5),
     *     (events, info) -> {
     *         // Process batch of events in the same session
     *         return new SessionResult(...);
     *     }
     * );
     * }</pre>
     *
     * @param <K> the key type
     * @param <V> the value type
     * @param <R> the result type
     */
    public static final class SessionWindow<K, V, R> {
        private final WindowAssigner<K, V> assigner;
        private final WindowTrigger<K, V, R> trigger;

        public SessionWindow(Duration gap, BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler) {
            this(gap, handler, null);
        }

        public SessionWindow(
            Duration gap,
            BiFunction<List<StreamEvent<V>>, WindowInfo, R> handler,
            Duration lateTolerance
        ) {
            WindowSpec spec = WindowSpec.builder()
                .type(WindowType.SESSION)
                .size(Duration.fromMillis(0))
                .gap(gap)
                .lateTolerance(lateTolerance != null ? lateTolerance : Duration.fromMillis(0))
                .build();

            this.assigner = new WindowAssigner<>(spec);
            this.trigger = new WindowTrigger<>(assigner, handler);
        }

        /**
         * Process event.
         */
        public List<R> process(StreamEvent<V> event, K key) {
            return trigger.process(event, key);
        }

        /**
         * Advance watermark and trigger completed windows.
         */
        public List<R> advanceWatermark(Timestamp watermark) {
            return trigger.advanceWatermark(watermark);
        }

        public WindowAssigner<K, V> getAssigner() {
            return assigner;
        }
    }
}
