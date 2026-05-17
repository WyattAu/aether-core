package io.aether.sdk.streaming;

import java.time.Duration;

/**
 * Configuration for batch processing.
 */
public class BatchConfig {
    public int maxBatchSize = 1000;
    public Duration maxWaitTime = Duration.ofMillis(100);
    public long maxBytes = 1024 * 1024; // 1MB
    public boolean timeoutOnFull = true;
    public boolean partialOnTimeout = true;
    public boolean partialOnShutdown = true;
    public boolean parallel = false;
    public int maxParallelBatches = 10;
    public Duration batchTimeout = Duration.ofSeconds(1);
    public boolean retryOnFailure = true;
    public Duration retryDelay = Duration.ofMillis(100);
    public double retryBackoff = 2.0;
    public boolean enableAsync = true;
    public boolean adaptiveBatching = false;
    public double batchTimeoutFactor = 1.5;
    public int maxConcurrency = 4;

    public static BatchConfig defaults() {
        return new BatchConfig();
    }
}
