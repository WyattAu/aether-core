package io.aether.sdk.streaming;

import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;

import io.aether.sdk.streaming.Types.StreamEvent;

/**
 * Processes events for a specific partition.
 */
public class PartitionProcessor<T> {
    private final int partitionId;
    private final Function<StreamEvent<T>, Void> handler;
    private final AtomicLong eventCount = new AtomicLong(0);
    private final AtomicLong errorCount = new AtomicLong(0);

    public PartitionProcessor(int partitionId, Function<StreamEvent<T>, Void> handler) {
        this.partitionId = partitionId;
        this.handler = handler;
    }

    /**
     * Process an event.
     */
    public void process(StreamEvent<T> event) throws Exception {
        eventCount.incrementAndGet();

        try {
            handler.apply(event);
        } catch (Exception e) {
            errorCount.incrementAndGet();
            throw e;
        }
    }

    /**
     * Get partition ID.
     */
    public int getPartitionId() {
        return partitionId;
    }

    /**
     * Get processor statistics.
     */
    public long[] getStats() {
        return new long[]{ eventCount.get(), errorCount.get() };
    }
}
