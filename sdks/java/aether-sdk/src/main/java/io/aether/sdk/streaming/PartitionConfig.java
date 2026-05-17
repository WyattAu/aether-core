package io.aether.sdk.streaming;

import java.util.List;
import java.util.function.Function;

/**
 * Configuration for partitioning.
 */
public class PartitionConfig {
    public PartitionStrategy strategy = PartitionStrategy.KEY;
    public int partitions = 10;
    public Function<Object, String> keyExtractor;
    public List<String> rangeBounds;

    public static PartitionConfig defaults() {
        return new PartitionConfig();
    }
}
