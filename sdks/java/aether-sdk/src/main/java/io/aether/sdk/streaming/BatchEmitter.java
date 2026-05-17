package io.aether.sdk.streaming;

import java.util.List;
import java.util.function.Consumer;

/**
 * Emits batch results to downstream consumers.
 */
public class BatchEmitter<T> {
    private final List<Consumer<BatchResult<T>>> handlers = new java.util.concurrent.CopyOnWriteArrayList<>();

    /**
     * Add a handler for batch results.
     */
    public void addHandler(Consumer<BatchResult<T>> handler) {
        handlers.add(handler);
    }

    /**
     * Emit batch to all handlers.
     */
    public void emit(BatchResult<T> batch) {
        for (Consumer<BatchResult<T>> handler : handlers) {
            handler.accept(batch);
        }
    }
}
