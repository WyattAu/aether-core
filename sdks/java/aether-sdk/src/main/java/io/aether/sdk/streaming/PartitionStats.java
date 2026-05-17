package io.aether.sdk.streaming;

import java.util.concurrent.atomic.AtomicLong;

/**
 * Statistics for partition distribution.
 */
public class PartitionStats {
    public final AtomicLong totalEvents = new AtomicLong(0);
    public final AtomicLong[] partitionCount;
    public final AtomicLong rebalances = new AtomicLong(0);
    public volatile double rebalancesAvg = 0.0;

    public PartitionStats(int numPartitions) {
        partitionCount = new AtomicLong[numPartitions];
        for (int i = 0; i < numPartitions; i++) {
            partitionCount[i] = new AtomicLong(0);
        }
    }

    public PartitionStats copy() {
        PartitionStats copy = new PartitionStats(partitionCount.length);
        copy.totalEvents.set(totalEvents.get());
        for (int i = 0; i < partitionCount.length; i++) {
            copy.partitionCount[i].set(partitionCount[i].get());
        }
        copy.rebalances.set(rebalances.get());
        copy.rebalancesAvg = rebalancesAvg;
        return copy;
    }
}
