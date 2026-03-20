package io.aether.sdk.streaming;

import io.aether.sdk.actor.Actor;
import io.aether.sdk.messaging.Message;
import io.aether.sdk.state.StateHandle;

import java.time.Instant;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;

/**
 * Base class for stream processing actors.
 *
 * <p>Extends Actor with:
 * <ul>
 *   <li>Event-time processing and watermarks</li>
 *   <li>Windowed aggregation</li>
 *   <li>Backpressure handling</li>
 *   <li>Stream state management</li>
 * </ul>
 *
 * <p>Example usage:
 * <pre>{@code
 * public class MyStreamProcessor extends StreamActor<String, Event> {
 *     @Override
 *     protected void processEvent(StreamEvent<Event> event) {
 *         Event data = event.getValue();
 *         emit("output", transform(data));
 *     }
 * }
 * }</pre>
 *
 * @param <K> Key type for partitioning
 * @param <V> Value type for stream events
 */
public abstract class StreamActor<K, V> extends Actor {

    protected final Types.StreamConfig streamConfig;
    protected final StreamState streamState;
    protected final BackpressureController<V> backpressure;

    private final Map<String, Consumer<StreamEvent<?>>> outputHandlers = new ConcurrentHashMap<>();
    private Consumer<StreamEvent<V>> lateDataHandler;

    /**
     * Creates a new StreamActor with default configuration.
     */
    protected StreamActor() {
        this(Types.StreamConfig.builder().build());
    }

    /**
     * Creates a new StreamActor with the specified configuration.
     *
     * @param streamConfig Stream processing configuration
     */
    protected StreamActor(Types.StreamConfig streamConfig) {
        this.streamConfig = streamConfig;
        this.streamState = new StreamState();
        this.backpressure = new BackpressureController<>(
            BackpressureConfig.builder()
                .strategy(BackpressureStrategy.BUFFER)
                .bufferSize(10000)
                .highWatermark(0.9)
                .lowWatermark(0.5)
                .build()
        );
    }

    /**
     * Process a single stream event.
     *
     * <p>Override this method to implement event processing logic.
     *
     * @param event The stream event to process
     */
    protected abstract void processEvent(StreamEvent<V> event);

    /**
     * Handle incoming message.
     */
    @Override
    protected void handleMessage(Message message) {
        String type = message.getType();
        
        if ("stream_event".equals(type)) {
            Object payload = message.getPayload();
            if (payload instanceof StreamEvent) {
                @SuppressWarnings("unchecked")
                StreamEvent<V> event = (StreamEvent<V>) payload;
                processWithBackpressure(event);
            } else if (payload instanceof Map) {
                @SuppressWarnings("unchecked")
                StreamEvent<V> event = dictToEvent((Map<String, Object>) payload);
                if (event != null) {
                    processWithBackpressure(event);
                }
            }
        } else if ("watermark".equals(type)) {
            Object payload = message.getPayload();
            if (payload instanceof Types.Watermark) {
                advanceWatermark((Types.Watermark) payload);
            } else if (payload instanceof Map) {
                @SuppressWarnings("unchecked")
                Map<String, Object> map = (Map<String, Object>) payload;
                Types.Watermark watermark = new Types.Watermark(
                    new Types.Timestamp(((Number) map.get("timestamp")).longValue()),
                    (String) map.get("streamId"),
                    map.containsKey("partition") ? ((Number) map.get("partition")).intValue() : null
                );
                advanceWatermark(watermark);
            }
        }
    }

    @SuppressWarnings("unchecked")
    private StreamEvent<V> dictToEvent(Map<String, Object> data) {
        try {
            return new StreamEvent<V>(
                (String) data.get("key"),
                (V) data.get("value"),
                new Types.Timestamp(((Number) data.get("timestamp")).longValue()),
                (Map<String, String>) data.getOrDefault("headers", Collections.emptyMap()),
                data.containsKey("partition") ? ((Number) data.get("partition")).intValue() : null,
                data.containsKey("offset") ? ((Number) data.get("offset")).longValue() : null,
                (String) data.get("eventType")
            );
        } catch (Exception e) {
            return null;
        }
    }

    private void processWithBackpressure(StreamEvent<V> event) {
        if (!backpressure.tryPush(event)) {
            return;
        }

        while (true) {
            StreamEvent<V> bufferedEvent = backpressure.pop();
            if (bufferedEvent == null) {
                break;
            }

            try {
                processEventInternal(bufferedEvent);
            } catch (Exception e) {
                System.err.println("Error processing event: " + e.getMessage());
            }
        }
    }

    private void processEventInternal(StreamEvent<V> event) {
        streamState.processedCount++;

        // Check if event is late
        Types.Timestamp currentWatermark = streamState.watermarks.getOrDefault(
            event.getEventType() != null ? event.getEventType() : "default",
            new Types.Timestamp(0)
        );

        if (event.getTimestamp().getMilliseconds() < currentWatermark.getMilliseconds()) {
            streamState.lateEventsCount++;
            handleLateEvent(event);
            return;
        }

        // Call user's processEvent
        processEvent(event);

        // Update last processed timestamp
        streamState.lastProcessedTimestamp = event.getTimestamp();
    }

    /**
     * Handle late-arriving event based on policy.
     */
    protected void handleLateEvent(StreamEvent<V> event) {
        Types.LateDataPolicy policy = streamConfig.getLateDataPolicy();
        if (policy == null) {
            policy = Types.LateDataPolicy.DROP;
        }

        switch (policy) {
            case DROP:
                // Silently drop
                break;

            case SIDE_OUTPUT:
                if (lateDataHandler != null) {
                    lateDataHandler.accept(event);
                } else if (streamConfig.getLateDataOutput() != null) {
                    emit(streamConfig.getLateDataOutput(), event);
                }
                break;

            case REPROCESS:
                // Trigger reprocessing of affected windows
                break;
        }
    }

    /**
     * Advance watermark for a stream.
     */
    public void advanceWatermark(Types.Watermark watermark) {
        String streamId = watermark.getStreamId();
        Types.Timestamp oldWatermark = streamState.watermarks.get(streamId);

        // Only advance if new watermark is ahead
        if (oldWatermark == null || watermark.getTimestamp().getMilliseconds() > oldWatermark.getMilliseconds()) {
            streamState.watermarks.put(streamId, watermark.getTimestamp());
        }
    }

    /**
     * Get current watermark for a stream.
     */
    public Optional<Types.Timestamp> getWatermark(String streamId) {
        return Optional.ofNullable(streamState.watermarks.get(streamId));
    }

    /**
     * Emit a value to an output stream.
     */
    protected void emit(String stream, Object value) {
        StreamEvent<Object> event = new StreamEvent<>(
            String.valueOf(hashCode()),
            value,
            Types.Timestamp.now(),
            Collections.emptyMap()
        );
        doEmit(stream, event);
    }

    /**
     * Emit a value with specific timestamp.
     */
    protected void emitWithTimestamp(String stream, Object value, Types.Timestamp timestamp) {
        StreamEvent<Object> event = new StreamEvent<>(
            String.valueOf(hashCode()),
            value,
            timestamp,
            Collections.emptyMap()
        );
        doEmit(stream, event);
    }

    /**
     * Emit a pre-constructed stream event.
     */
    protected void emitEvent(String stream, StreamEvent<?> event) {
        doEmit(stream, event);
    }

    private void doEmit(String stream, StreamEvent<?> event) {
        Consumer<StreamEvent<?>> handler = outputHandlers.get(stream);
        if (handler != null) {
            handler.accept(event);
        }
    }

    /**
     * Register a handler for output stream.
     */
    public void registerOutputHandler(String stream, Consumer<StreamEvent<?>> handler) {
        outputHandlers.put(stream, handler);
    }

    /**
     * Register handler for late-arriving data.
     */
    public void registerLateDataHandler(Consumer<StreamEvent<V>> handler) {
        this.lateDataHandler = handler;
    }

    /**
     * Get value state.
     */
    protected <T> Optional<T> getState(String name) {
        // Implementation would use StateHandle
        return Optional.empty();
    }

    /**
     * Set value state.
     */
    protected <T> void setState(String name, T value) {
        // Implementation would use StateHandle
    }

    /**
     * Get list state.
     */
    protected <T> List<T> getListState(String name) {
        // Implementation would use StateHandle
        return Collections.emptyList();
    }

    /**
     * Update list state.
     */
    protected <T> void updateListState(String name, T item) {
        // Implementation would use StateHandle
    }

    /**
     * Get map state.
     */
    protected <K2, V2> Map<K2, V2> getMapState(String name) {
        // Implementation would use StateHandle
        return Collections.emptyMap();
    }

    /**
     * Update map state.
     */
    protected <K2, V2> void updateMapState(String name, K2 key, V2 value) {
        // Implementation would use StateHandle
    }

    /**
     * Get stream processing metrics.
     */
    public Map<String, Object> getMetrics() {
        Map<String, Object> metrics = new HashMap<>();
        metrics.put("processedCount", streamState.processedCount);
        metrics.put("lateEventsCount", streamState.lateEventsCount);
        metrics.put("lastProcessedTimestamp", 
            streamState.lastProcessedTimestamp != null 
                ? streamState.lastProcessedTimestamp.getMilliseconds() 
                : null);
        
        Map<String, Long> watermarkMap = new HashMap<>();
        streamState.watermarks.forEach((k, v) -> watermarkMap.put(k, v.getMilliseconds()));
        metrics.put("watermarks", watermarkMap);
        
        metrics.put("backpressure", backpressure.getStats());
        
        return metrics;
    }

    /**
     * Internal stream state.
     */
    protected static class StreamState {
        final Map<String, Types.Timestamp> watermarks = new ConcurrentHashMap<>();
        volatile long processedCount = 0;
        volatile long lateEventsCount = 0;
        volatile Types.Timestamp lastProcessedTimestamp = null;
    }
}
