package io.aether.sdk.streaming;

import java.time.Instant;
import java.time.LocalDateTime;
import java.time.ZoneId;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/**
 * Core types for stream processing.
 *
 * <p>This class contains the fundamental types used throughout the streaming module:
 * <ul>
 *   <li>{@link Timestamp} - Event timestamp with millisecond precision</li>
 *   <li>{@link Duration} - Time duration with millisecond precision</li>
 *   <li>{@link StreamEvent} - Individual event in a stream</li>
 *   <li>{@link Watermark} - Time marker for event progress</li>
 *   <li>{@link WindowSpec} - Window configuration</li>
 *   <li>{@link StreamConfig} - Stream actor configuration</li>
 * </ul>
 *
 * <p>Example usage:
 * <pre>{@code
 * Timestamp ts = Timestamp.now();
 * Duration windowSize = Duration.fromMinutes(5);
 * StreamEvent<String> event = StreamEvent.create("key1", "value", ts);
 * }</pre>
 */
public final class Types {

    private Types() {
    }

    /**
     * Types of windowing strategies.
     */
    public enum WindowType {
        /** Fixed-size, non-overlapping windows */
        TUMBLING,
        /** Fixed-size, overlapping windows */
        SLIDING,
        /** Dynamic size based on activity gaps */
        SESSION
    }

    /**
     * How to handle late-arriving data.
     */
    public enum LateDataPolicy {
        /** Discard late events */
        DROP,
        /** Route to side output stream */
        SIDE_OUTPUT,
        /** Reprocess affected windows */
        REPROCESS
    }

    /**
     * Watermark generation strategy.
     */
    public enum WatermarkStrategy {
        /** Based on event timestamps */
        EVENT_TIME,
        /** Based on processing time */
        PROCESSING_TIME,
        /** Allow bounded lateness */
        BOUNDED_OUT_OF_ORDER
    }

    /**
     * Backpressure handling strategies.
     */
    public enum BackpressureStrategy {
        /** Buffer events up to limit */
        BUFFER,
        /** Drop events when overloaded */
        DROP,
        /** Raise error when overloaded */
        FAIL,
        /** Keep only latest events */
        LATEST
    }

    /**
     * Message delivery guarantees.
     */
    public enum DeliverySemantics {
        /** Fire and forget */
        AT_MOST_ONCE,
        /** May duplicate */
        AT_LEAST_ONCE,
        /** No duplicates, no loss */
        EXACTLY_ONCE
    }

    /**
     * Window pane type.
     */
    public enum PaneInfo {
        /** Early firing before watermark */
        EARLY,
        /** On-time firing at watermark */
        ON_TIME,
        /** Late firing after watermark */
        LATE
    }

    /**
     * Event timestamp with millisecond precision.
     *
     * <p>Example:
     * <pre>{@code
     * Timestamp now = Timestamp.now();
     * Timestamp fromSeconds = Timestamp.fromSeconds(1234567890);
     * Timestamp fromDateTime = Timestamp.fromDateTime(LocalDateTime.now());
     *
     * Duration diff = now.minus(fromSeconds);
     * Timestamp future = now.plus(Duration.fromMinutes(5));
     * }</pre>
     */
    public static final class Timestamp implements Comparable<Timestamp> {
        private final long milliseconds;

        private Timestamp(long milliseconds) {
            this.milliseconds = milliseconds;
        }

        /**
         * Create timestamp from current time.
         */
        public static Timestamp now() {
            return new Timestamp(System.currentTimeMillis());
        }

        /**
         * Create timestamp from milliseconds since epoch.
         */
        public static Timestamp fromMillis(long milliseconds) {
            return new Timestamp(milliseconds);
        }

        /**
         * Create timestamp from seconds since epoch.
         */
        public static Timestamp fromSeconds(double seconds) {
            return new Timestamp((long) (seconds * 1000));
        }

        /**
         * Create timestamp from LocalDateTime.
         */
        public static Timestamp fromDateTime(LocalDateTime dateTime) {
            return new Timestamp(
                dateTime.atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
            );
        }

        /**
         * Create timestamp from Instant.
         */
        public static Timestamp fromInstant(Instant instant) {
            return new Timestamp(instant.toEpochMilli());
        }

        /**
         * Get milliseconds since epoch.
         */
        public long getMilliseconds() {
            return milliseconds;
        }

        /**
         * Get seconds since epoch.
         */
        public double getSeconds() {
            return milliseconds / 1000.0;
        }

        /**
         * Convert to LocalDateTime.
         */
        public LocalDateTime toDateTime() {
            return LocalDateTime.ofInstant(
                Instant.ofEpochMilli(milliseconds),
                ZoneId.systemDefault()
            );
        }

        /**
         * Convert to Instant.
         */
        public Instant toInstant() {
            return Instant.ofEpochMilli(milliseconds);
        }

        /**
         * Add duration to this timestamp.
         */
        public Timestamp plus(Duration duration) {
            return new Timestamp(this.milliseconds + duration.getMilliseconds());
        }

        /**
         * Subtract duration from this timestamp.
         */
        public Timestamp minus(Duration duration) {
            return new Timestamp(this.milliseconds - duration.getMilliseconds());
        }

        /**
         * Calculate duration between this and another timestamp.
         */
        public Duration minus(Timestamp other) {
            return Duration.fromMillis(this.milliseconds - other.milliseconds);
        }

        @Override
        public int compareTo(Timestamp other) {
            return Long.compare(this.milliseconds, other.milliseconds);
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            Timestamp timestamp = (Timestamp) o;
            return milliseconds == timestamp.milliseconds;
        }

        @Override
        public int hashCode() {
            return Objects.hash(milliseconds);
        }

        @Override
        public String toString() {
            return "Timestamp{" + milliseconds + "ms}";
        }
    }

    /**
     * Duration with millisecond precision.
     *
     * <p>Example:
     * <pre>{@code
     * Duration fiveMinutes = Duration.fromMinutes(5);
     * Duration oneHour = Duration.fromHours(1);
     * Duration custom = Duration.fromMillis(12345);
     *
     * Duration combined = fiveMinutes.plus(oneHour);
     * Duration doubled = fiveMinutes.multipliedBy(2);
     * }</pre>
     */
    public static final class Duration {
        private final long milliseconds;

        private Duration(long milliseconds) {
            this.milliseconds = milliseconds;
        }

        /**
         * Create duration from milliseconds.
         */
        public static Duration fromMillis(long milliseconds) {
            return new Duration(milliseconds);
        }

        /**
         * Create duration from seconds.
         */
        public static Duration fromSeconds(double seconds) {
            return new Duration((long) (seconds * 1000));
        }

        /**
         * Create duration from minutes.
         */
        public static Duration fromMinutes(double minutes) {
            return new Duration((long) (minutes * 60 * 1000));
        }

        /**
         * Create duration from hours.
         */
        public static Duration fromHours(double hours) {
            return new Duration((long) (hours * 3600 * 1000));
        }

        /**
         * Create from Java time Duration.
         */
        public static Duration fromJavaDuration(java.time.Duration duration) {
            return new Duration(duration.toMillis());
        }

        /**
         * Get milliseconds.
         */
        public long getMilliseconds() {
            return milliseconds;
        }

        /**
         * Get seconds.
         */
        public double getSeconds() {
            return milliseconds / 1000.0;
        }

        /**
         * Convert to Java time Duration.
         */
        public java.time.Duration toJavaDuration() {
            return java.time.Duration.ofMillis(milliseconds);
        }

        /**
         * Add another duration.
         */
        public Duration plus(Duration other) {
            return new Duration(this.milliseconds + other.milliseconds);
        }

        /**
         * Subtract another duration.
         */
        public Duration minus(Duration other) {
            return new Duration(this.milliseconds - other.milliseconds);
        }

        /**
         * Multiply by a factor.
         */
        public Duration multipliedBy(long factor) {
            return new Duration(this.milliseconds * factor);
        }

        /**
         * Divide by a factor.
         */
        public Duration dividedBy(long divisor) {
            return new Duration(this.milliseconds / divisor);
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            Duration duration = (Duration) o;
            return milliseconds == duration.milliseconds;
        }

        @Override
        public int hashCode() {
            return Objects.hash(milliseconds);
        }

        @Override
        public String toString() {
            return "Duration{" + milliseconds + "ms}";
        }
    }

    /**
     * Event in a stream with metadata.
     *
     * <p>Example:
     * <pre>{@code
     * StreamEvent<String> event = StreamEvent.create("user-123", "click");
     * StreamEvent<MyData> event2 = StreamEvent.<MyData>builder()
     *     .key("order-456")
     *     .value(new MyData(...))
     *     .timestamp(Timestamp.now())
     *     .header("source", "web")
     *     .build();
     * }</pre>
     *
     * @param <T> the type of the event value
     */
    public static final class StreamEvent<T> {
        private final String key;
        private final T value;
        private final Timestamp timestamp;
        private final Map<String, String> headers;
        private final Integer partition;
        private final Long offset;
        private final String eventType;

        private StreamEvent(Builder<T> builder) {
            this.key = builder.key;
            this.value = builder.value;
            this.timestamp = builder.timestamp != null ? builder.timestamp : Timestamp.now();
            this.headers = Collections.unmodifiableMap(new HashMap<>(builder.headers));
            this.partition = builder.partition;
            this.offset = builder.offset;
            this.eventType = builder.eventType;
        }

        /**
         * Create a new stream event with key and value.
         */
        public static <T> StreamEvent<T> create(String key, T value) {
            return new Builder<T>().key(key).value(value).build();
        }

        /**
         * Create a new stream event with key, value, and timestamp.
         */
        public static <T> StreamEvent<T> create(String key, T value, Timestamp timestamp) {
            return new Builder<T>().key(key).value(value).timestamp(timestamp).build();
        }

        /**
         * Create a new builder.
         */
        public static <T> Builder<T> builder() {
            return new Builder<>();
        }

        /**
         * Get the partition key.
         */
        public String getKey() {
            return key;
        }

        /**
         * Get the event value.
         */
        public T getValue() {
            return value;
        }

        /**
         * Get the event timestamp.
         */
        public Timestamp getTimestamp() {
            return timestamp;
        }

        /**
         * Get the event headers.
         */
        public Map<String, String> getHeaders() {
            return headers;
        }

        /**
         * Get the partition number.
         */
        public Optional<Integer> getPartition() {
            return Optional.ofNullable(partition);
        }

        /**
         * Get the offset in partition.
         */
        public Optional<Long> getOffset() {
            return Optional.ofNullable(offset);
        }

        /**
         * Get the event type identifier.
         */
        public Optional<String> getEventType() {
            return Optional.ofNullable(eventType);
        }

        /**
         * Create a copy with a new value.
         */
        public <U> StreamEvent<U> withValue(U newValue) {
            return new Builder<U>()
                .key(key)
                .value(newValue)
                .timestamp(timestamp)
                .headers(headers)
                .partition(partition)
                .offset(offset)
                .eventType(eventType)
                .build();
        }

        /**
         * Create a copy with a new timestamp.
         */
        public StreamEvent<T> withTimestamp(Timestamp newTimestamp) {
            return new Builder<T>()
                .key(key)
                .value(value)
                .timestamp(newTimestamp)
                .headers(headers)
                .partition(partition)
                .offset(offset)
                .eventType(eventType)
                .build();
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            StreamEvent<?> that = (StreamEvent<?>) o;
            return Objects.equals(key, that.key) &&
                   Objects.equals(value, that.value) &&
                   Objects.equals(timestamp, that.timestamp) &&
                   Objects.equals(headers, that.headers) &&
                   Objects.equals(partition, that.partition) &&
                   Objects.equals(offset, that.offset) &&
                   Objects.equals(eventType, that.eventType);
        }

        @Override
        public int hashCode() {
            return Objects.hash(key, value, timestamp, headers, partition, offset, eventType);
        }

        @Override
        public String toString() {
            return "StreamEvent{key='" + key + "', timestamp=" + timestamp + '}';
        }

        /**
         * Builder for StreamEvent.
         */
        public static final class Builder<T> {
            private String key;
            private T value;
            private Timestamp timestamp;
            private Map<String, String> headers = new HashMap<>();
            private Integer partition;
            private Long offset;
            private String eventType;

            public Builder<T> key(String key) {
                this.key = key;
                return this;
            }

            public Builder<T> value(T value) {
                this.value = value;
                return this;
            }

            public Builder<T> timestamp(Timestamp timestamp) {
                this.timestamp = timestamp;
                return this;
            }

            public Builder<T> headers(Map<String, String> headers) {
                this.headers = new HashMap<>(headers);
                return this;
            }

            public Builder<T> header(String key, String value) {
                this.headers.put(key, value);
                return this;
            }

            public Builder<T> partition(Integer partition) {
                this.partition = partition;
                return this;
            }

            public Builder<T> offset(Long offset) {
                this.offset = offset;
                return this;
            }

            public Builder<T> eventType(String eventType) {
                this.eventType = eventType;
                return this;
            }

            public StreamEvent<T> build() {
                Objects.requireNonNull(key, "key is required");
                Objects.requireNonNull(value, "value is required");
                return new StreamEvent<>(this);
            }
        }
    }

    /**
     * Watermark indicating event time progress.
     *
     * <p>Watermarks are used to track the progress of event time in a stream.
     * Events with timestamps before the watermark are considered "late".
     *
     * <p>Example:
     * <pre>{@code
     * Watermark watermark = new Watermark(Timestamp.now(), "stream-1");
     * if (watermark.isLate(event.getTimestamp())) {
     *     handleLateEvent(event);
     * }
     * }</pre>
     */
    public static final class Watermark {
        private final Timestamp timestamp;
        private final String streamId;
        private final Integer partition;

        /**
         * Create a new watermark.
         */
        public Watermark(Timestamp timestamp, String streamId) {
            this(timestamp, streamId, null);
        }

        /**
         * Create a new watermark with partition.
         */
        public Watermark(Timestamp timestamp, String streamId, Integer partition) {
            this.timestamp = Objects.requireNonNull(timestamp);
            this.streamId = Objects.requireNonNull(streamId);
            this.partition = partition;
        }

        /**
         * Get the watermark timestamp.
         */
        public Timestamp getTimestamp() {
            return timestamp;
        }

        /**
         * Get the stream identifier.
         */
        public String getStreamId() {
            return streamId;
        }

        /**
         * Get the partition number.
         */
        public Optional<Integer> getPartition() {
            return Optional.ofNullable(partition);
        }

        /**
         * Check if an event timestamp is late relative to this watermark.
         */
        public boolean isLate(Timestamp eventTimestamp) {
            return eventTimestamp.compareTo(timestamp) < 0;
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            Watermark watermark = (Watermark) o;
            return Objects.equals(timestamp, watermark.timestamp) &&
                   Objects.equals(streamId, watermark.streamId) &&
                   Objects.equals(partition, watermark.partition);
        }

        @Override
        public int hashCode() {
            return Objects.hash(timestamp, streamId, partition);
        }

        @Override
        public String toString() {
            return "Watermark{streamId='" + streamId + "', timestamp=" + timestamp + '}';
        }
    }

    /**
     * Window specification for stream processing.
     *
     * <p>Example:
     * <pre>{@code
     * // Tumbling window of 5 minutes
     * WindowSpec tumbling = WindowSpec.tumbling(Duration.fromMinutes(5));
     *
     * // Sliding window of 10 minutes sliding every 1 minute
     * WindowSpec sliding = WindowSpec.sliding(
     *     Duration.fromMinutes(10),
     *     Duration.fromMinutes(1)
     * );
     *
     * // Session window with 5 minute gap
     * WindowSpec session = WindowSpec.session(Duration.fromMinutes(5));
     * }</pre>
     */
    public static final class WindowSpec {
        private final WindowType type;
        private final Duration size;
        private final Duration slide;
        private final Duration gap;
        private final Duration lateTolerance;
        private final Duration allowedLateness;

        private WindowSpec(Builder builder) {
            this.type = builder.type;
            this.size = builder.size;
            this.slide = builder.slide;
            this.gap = builder.gap;
            this.lateTolerance = builder.lateTolerance != null ? builder.lateTolerance : Duration.fromMillis(0);
            this.allowedLateness = builder.allowedLateness != null ? builder.allowedLateness : Duration.fromMillis(0);
            validate();
        }

        /**
         * Create a tumbling window specification.
         */
        public static WindowSpec tumbling(Duration size) {
            return builder().type(WindowType.TUMBLING).size(size).build();
        }

        /**
         * Create a sliding window specification.
         */
        public static WindowSpec sliding(Duration size, Duration slide) {
            return builder().type(WindowType.SLIDING).size(size).slide(slide).build();
        }

        /**
         * Create a session window specification.
         */
        public static WindowSpec session(Duration gap) {
            return builder().type(WindowType.SESSION).gap(gap).build();
        }

        /**
         * Create a new builder.
         */
        public static Builder builder() {
            return new Builder();
        }

        private void validate() {
            Objects.requireNonNull(type, "type is required");
            Objects.requireNonNull(size, "size is required");

            if (type == WindowType.SLIDING && slide == null) {
                throw new IllegalArgumentException("Sliding window requires 'slide' parameter");
            }
            if (type == WindowType.SESSION && gap == null) {
                throw new IllegalArgumentException("Session window requires 'gap' parameter");
            }
        }

        public WindowType getType() {
            return type;
        }

        public Duration getSize() {
            return size;
        }

        public Optional<Duration> getSlide() {
            return Optional.ofNullable(slide);
        }

        public Optional<Duration> getGap() {
            return Optional.ofNullable(gap);
        }

        public Duration getLateTolerance() {
            return lateTolerance;
        }

        public Duration getAllowedLateness() {
            return allowedLateness;
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            WindowSpec that = (WindowSpec) o;
            return type == that.type &&
                   Objects.equals(size, that.size) &&
                   Objects.equals(slide, that.slide) &&
                   Objects.equals(gap, that.gap) &&
                   Objects.equals(lateTolerance, that.lateTolerance) &&
                   Objects.equals(allowedLateness, that.allowedLateness);
        }

        @Override
        public int hashCode() {
            return Objects.hash(type, size, slide, gap, lateTolerance, allowedLateness);
        }

        /**
         * Builder for WindowSpec.
         */
        public static final class Builder {
            private WindowType type;
            private Duration size;
            private Duration slide;
            private Duration gap;
            private Duration lateTolerance;
            private Duration allowedLateness;

            public Builder type(WindowType type) {
                this.type = type;
                return this;
            }

            public Builder size(Duration size) {
                this.size = size;
                return this;
            }

            public Builder slide(Duration slide) {
                this.slide = slide;
                return this;
            }

            public Builder gap(Duration gap) {
                this.gap = gap;
                return this;
            }

            public Builder lateTolerance(Duration lateTolerance) {
                this.lateTolerance = lateTolerance;
                return this;
            }

            public Builder allowedLateness(Duration allowedLateness) {
                this.allowedLateness = allowedLateness;
                return this;
            }

            public WindowSpec build() {
                return new WindowSpec(this);
            }
        }
    }

    /**
     * Information about an active window.
     */
    public static final class WindowInfo {
        private final Timestamp start;
        private final Timestamp end;
        private final Timestamp maxTimestamp;
        private final PaneInfo pane;
        private final String windowId;

        public WindowInfo(Timestamp start, Timestamp end, Timestamp maxTimestamp, PaneInfo pane) {
            this(start, end, maxTimestamp, pane, null);
        }

        public WindowInfo(Timestamp start, Timestamp end, Timestamp maxTimestamp, PaneInfo pane, String windowId) {
            this.start = Objects.requireNonNull(start);
            this.end = Objects.requireNonNull(end);
            this.maxTimestamp = Objects.requireNonNull(maxTimestamp);
            this.pane = Objects.requireNonNull(pane);
            this.windowId = windowId;
        }

        /**
         * Check if timestamp falls within this window.
         */
        public boolean contains(Timestamp timestamp) {
            return timestamp.compareTo(start) >= 0 && timestamp.compareTo(end) < 0;
        }

        /**
         * Check if timestamp is late for this window.
         */
        public boolean isLate(Timestamp timestamp) {
            return timestamp.compareTo(start) < 0;
        }

        public Timestamp getStart() {
            return start;
        }

        public Timestamp getEnd() {
            return end;
        }

        public Timestamp getMaxTimestamp() {
            return maxTimestamp;
        }

        public PaneInfo getPane() {
            return pane;
        }

        public Optional<String> getWindowId() {
            return Optional.ofNullable(windowId);
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            WindowInfo that = (WindowInfo) o;
            return Objects.equals(start, that.start) &&
                   Objects.equals(end, that.end) &&
                   Objects.equals(maxTimestamp, that.maxTimestamp) &&
                   pane == that.pane &&
                   Objects.equals(windowId, that.windowId);
        }

        @Override
        public int hashCode() {
            return Objects.hash(start, end, maxTimestamp, pane, windowId);
        }

        @Override
        public String toString() {
            return "WindowInfo{start=" + start + ", end=" + end + ", pane=" + pane + '}';
        }
    }

    /**
     * Configuration for stream actors.
     *
     * <p>Example:
     * <pre>{@code
     * StreamConfig config = StreamConfig.builder()
     *     .inputStreams(List.of("input-stream"))
     *     .outputStreams(List.of("output-stream"))
     *     .parallelism(4)
     *     .watermarkStrategy(WatermarkStrategy.EVENT_TIME)
     *     .checkpointingEnabled(true)
     *     .build();
     * }</pre>
     */
    public static final class StreamConfig {
        private final List<String> inputStreams;
        private final List<String> outputStreams;
        private final int parallelism;
        private final String partitionStrategy;
        private final WatermarkStrategy watermarkStrategy;
        private final Duration watermarkInterval;
        private final Duration outOfOrderness;
        private final boolean checkpointingEnabled;
        private final Duration checkpointInterval;
        private final LateDataPolicy lateDataPolicy;
        private final String lateDataOutput;
        private final int bufferCapacity;
        private final Duration bufferTimeout;

        private StreamConfig(Builder builder) {
            this.inputStreams = builder.inputStreams != null
                ? Collections.unmodifiableList(new java.util.ArrayList<>(builder.inputStreams))
                : Collections.emptyList();
            this.outputStreams = builder.outputStreams != null
                ? Collections.unmodifiableList(new java.util.ArrayList<>(builder.outputStreams))
                : Collections.emptyList();
            this.parallelism = builder.parallelism != null ? builder.parallelism : 1;
            this.partitionStrategy = builder.partitionStrategy != null ? builder.partitionStrategy : "key";
            this.watermarkStrategy = builder.watermarkStrategy != null
                ? builder.watermarkStrategy
                : WatermarkStrategy.PROCESSING_TIME;
            this.watermarkInterval = builder.watermarkInterval != null
                ? builder.watermarkInterval
                : Duration.fromSeconds(1);
            this.outOfOrderness = builder.outOfOrderness != null
                ? builder.outOfOrderness
                : Duration.fromMillis(0);
            this.checkpointingEnabled = builder.checkpointingEnabled != null
                ? builder.checkpointingEnabled
                : false;
            this.checkpointInterval = builder.checkpointInterval != null
                ? builder.checkpointInterval
                : Duration.fromMinutes(1);
            this.lateDataPolicy = builder.lateDataPolicy != null
                ? builder.lateDataPolicy
                : LateDataPolicy.DROP;
            this.lateDataOutput = builder.lateDataOutput;
            this.bufferCapacity = builder.bufferCapacity != null ? builder.bufferCapacity : 10000;
            this.bufferTimeout = builder.bufferTimeout != null
                ? builder.bufferTimeout
                : Duration.fromSeconds(30);
        }

        public static Builder builder() {
            return new Builder();
        }

        public List<String> getInputStreams() {
            return inputStreams;
        }

        public List<String> getOutputStreams() {
            return outputStreams;
        }

        public int getParallelism() {
            return parallelism;
        }

        public String getPartitionStrategy() {
            return partitionStrategy;
        }

        public WatermarkStrategy getWatermarkStrategy() {
            return watermarkStrategy;
        }

        public Duration getWatermarkInterval() {
            return watermarkInterval;
        }

        public Duration getOutOfOrderness() {
            return outOfOrderness;
        }

        public boolean isCheckpointingEnabled() {
            return checkpointingEnabled;
        }

        public Duration getCheckpointInterval() {
            return checkpointInterval;
        }

        public LateDataPolicy getLateDataPolicy() {
            return lateDataPolicy;
        }

        public Optional<String> getLateDataOutput() {
            return Optional.ofNullable(lateDataOutput);
        }

        public int getBufferCapacity() {
            return bufferCapacity;
        }

        public Duration getBufferTimeout() {
            return bufferTimeout;
        }

        /**
         * Builder for StreamConfig.
         */
        public static final class Builder {
            private List<String> inputStreams;
            private List<String> outputStreams;
            private Integer parallelism;
            private String partitionStrategy;
            private WatermarkStrategy watermarkStrategy;
            private Duration watermarkInterval;
            private Duration outOfOrderness;
            private Boolean checkpointingEnabled;
            private Duration checkpointInterval;
            private LateDataPolicy lateDataPolicy;
            private String lateDataOutput;
            private Integer bufferCapacity;
            private Duration bufferTimeout;

            public Builder inputStreams(List<String> inputStreams) {
                this.inputStreams = inputStreams;
                return this;
            }

            public Builder outputStreams(List<String> outputStreams) {
                this.outputStreams = outputStreams;
                return this;
            }

            public Builder parallelism(int parallelism) {
                this.parallelism = parallelism;
                return this;
            }

            public Builder partitionStrategy(String partitionStrategy) {
                this.partitionStrategy = partitionStrategy;
                return this;
            }

            public Builder watermarkStrategy(WatermarkStrategy watermarkStrategy) {
                this.watermarkStrategy = watermarkStrategy;
                return this;
            }

            public Builder watermarkInterval(Duration watermarkInterval) {
                this.watermarkInterval = watermarkInterval;
                return this;
            }

            public Builder outOfOrderness(Duration outOfOrderness) {
                this.outOfOrderness = outOfOrderness;
                return this;
            }

            public Builder checkpointingEnabled(boolean checkpointingEnabled) {
                this.checkpointingEnabled = checkpointingEnabled;
                return this;
            }

            public Builder checkpointInterval(Duration checkpointInterval) {
                this.checkpointInterval = checkpointInterval;
                return this;
            }

            public Builder lateDataPolicy(LateDataPolicy lateDataPolicy) {
                this.lateDataPolicy = lateDataPolicy;
                return this;
            }

            public Builder lateDataOutput(String lateDataOutput) {
                this.lateDataOutput = lateDataOutput;
                return this;
            }

            public Builder bufferCapacity(int bufferCapacity) {
                this.bufferCapacity = bufferCapacity;
                return this;
            }

            public Builder bufferTimeout(Duration bufferTimeout) {
                this.bufferTimeout = bufferTimeout;
                return this;
            }

            public StreamConfig build() {
                return new StreamConfig(this);
            }
        }
    }

    /**
     * Configuration for backpressure handling.
     */
    public static final class BackpressureConfig {
        private final BackpressureStrategy strategy;
        private final int bufferSize;
        private final double highWatermark;
        private final double lowWatermark;

        private BackpressureConfig(Builder builder) {
            this.strategy = builder.strategy != null ? builder.strategy : BackpressureStrategy.BUFFER;
            this.bufferSize = builder.bufferSize != null ? builder.bufferSize : 10000;
            this.highWatermark = builder.highWatermark != null ? builder.highWatermark : 0.9;
            this.lowWatermark = builder.lowWatermark != null ? builder.lowWatermark : 0.5;
        }

        public static Builder builder() {
            return new Builder();
        }

        public static BackpressureConfig defaultConfig() {
            return builder().build();
        }

        public BackpressureStrategy getStrategy() {
            return strategy;
        }

        public int getBufferSize() {
            return bufferSize;
        }

        public double getHighWatermark() {
            return highWatermark;
        }

        public double getLowWatermark() {
            return lowWatermark;
        }

        public static final class Builder {
            private BackpressureStrategy strategy;
            private Integer bufferSize;
            private Double highWatermark;
            private Double lowWatermark;

            public Builder strategy(BackpressureStrategy strategy) {
                this.strategy = strategy;
                return this;
            }

            public Builder bufferSize(int bufferSize) {
                this.bufferSize = bufferSize;
                return this;
            }

            public Builder highWatermark(double highWatermark) {
                this.highWatermark = highWatermark;
                return this;
            }

            public Builder lowWatermark(double lowWatermark) {
                this.lowWatermark = lowWatermark;
                return this;
            }

            public BackpressureConfig build() {
                return new BackpressureConfig(this);
            }
        }
    }
}
