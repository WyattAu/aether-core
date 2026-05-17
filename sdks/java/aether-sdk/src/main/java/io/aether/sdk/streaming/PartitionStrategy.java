package io.aether.sdk.streaming;

/**
 * Partition strategies.
 */
public enum PartitionStrategy {
    ROUND_ROBIN,
    KEY,
    HASH,
    RANDOM,
    RANGE
}
