package io.aether.sdk.streaming;

import java.time.Duration;
import java.time.Instant;
import java.util.List;

/**
 * Result of batch processing.
 */
public class BatchResult<T> {
    public final List<T> items;
    public final long sizeBytes;
    public final Duration processingTime;
    public final String batchId;
    public final Instant timestamp;
    public Object aggregated;
    public String aggregationKey;
    public String checksum;

    public BatchResult(List<T> items, long sizeBytes, Duration processingTime, String batchId) {
        this.items = items;
        this.sizeBytes = sizeBytes;
        this.processingTime = processingTime;
        this.batchId = batchId;
        this.timestamp = Instant.now();
    }
}
