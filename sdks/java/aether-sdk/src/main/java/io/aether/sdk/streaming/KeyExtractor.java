package io.aether.sdk.streaming;

import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;

/**
 * Extracts partition keys from events.
 */
public class KeyExtractor<T> {
    private final Function<T, String> extractor;
    private String fallback = "default";
    private final AtomicLong count = new AtomicLong(0);
    private final AtomicLong nullCount = new AtomicLong(0);

    public KeyExtractor(Function<T, String> extractor) {
        this.extractor = extractor;
    }

    /**
     * Extract key from value.
     */
    public String extract(T value) {
        count.incrementAndGet();

        if (extractor == null) {
            nullCount.incrementAndGet();
            return fallback;
        }

        String key = extractor.apply(value);
        if (key == null || key.isEmpty()) {
            nullCount.incrementAndGet();
            return fallback;
        }
        return key;
    }

    /**
     * Set fallback key for null/empty results.
     */
    public void setFallback(String fallback) {
        this.fallback = fallback;
    }

    /**
     * Get extraction statistics.
     */
    public long[] getStats() {
        return new long[]{ count.get(), nullCount.get() };
    }
}
