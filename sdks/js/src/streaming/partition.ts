/**
 * Partitioning for Parallel Stream Processing
 * 
 * @module aether/streaming/partition
 */

import { StreamEvent } from './types';

/**
 * Partition strategies
 */
export enum PartitionStrategy {
  RoundRobin = 'RoundRobin',
  Key = 'Key',
  Hash = 'Hash',
  Random = 'Random',
  Range = 'Range',
}

/**
 * Partition configuration
 */
export interface PartitionConfig {
  strategy: PartitionStrategy;
  partitions: number;
  keyExtractor?: (value: unknown) => string;
  rangeBounds?: string[];
}

/**
 * Default partition configuration
 */
export function defaultPartitionConfig(): PartitionConfig {
  return {
    strategy: PartitionStrategy.Key,
    partitions: 10,
  };
}

/**
 * Partition statistics
 */
export interface PartitionStats {
  totalEvents: number;
  partitionCount: number[];
  rebalances: number;
  rebalancesAvg: number;
}

/**
 * Partitioner for routing events to partitions
 */
export class Partitioner {
  private config: PartitionConfig;
  private currentIndex: number = 0;
  private stats: PartitionStats;

  constructor(config: PartitionConfig) {
    this.config = { ...config };
    if (this.config.partitions <= 0) {
      this.config.partitions = 1;
    }
    this.stats = {
      totalEvents: 0,
      partitionCount: new Array(this.config.partitions).fill(0),
      rebalances: 0,
      rebalancesAvg: 0,
    };
  }

  /**
   * Get partition for an event
   */
  partition(event: StreamEvent<unknown>): number {
    return this.partitionByKey(event.key);
  }

  /**
   * Get partition for a key
   */
  partitionByKey(key: string): number {
    if (!key) {
      return this.partitionRoundRobin();
    }

    switch (this.config.strategy) {
      case PartitionStrategy.Key:
      case PartitionStrategy.Hash:
        return this.hashPartition(key);
      case PartitionStrategy.RoundRobin:
        return this.partitionRoundRobin();
      case PartitionStrategy.Range:
        return this.rangePartition(key);
      case PartitionStrategy.Random:
        return this.randomPartition();
      default:
        return this.hashPartition(key);
    }
  }

  /**
   * Get partition for a value using key extractor
   */
  partitionByValue(value: unknown): number {
    if (this.config.keyExtractor) {
      const key = this.config.keyExtractor(value);
      return this.partitionByKey(key);
    }
    return this.partitionRoundRobin();
  }

  private hashPartition(key: string): number {
    // Simple hash function (djb2)
    let hash = 5381;
    for (let i = 0; i < key.length; i++) {
      hash = ((hash << 5) + hash) + key.charCodeAt(i);
      hash = hash & hash; // Convert to 32-bit integer
    }

    const partition = Math.abs(hash) % this.config.partitions;
    this.updateStats(partition);
    return partition;
  }

  private partitionRoundRobin(): number {
    const partition = this.currentIndex % this.config.partitions;
    this.currentIndex++;
    this.updateStats(partition);
    return partition;
  }

  private rangePartition(key: string): number {
    const bounds = this.config.rangeBounds || [];
    if (bounds.length === 0) {
      return this.partitionRoundRobin();
    }

    let partition = 0;
    for (let i = 0; i < bounds.length; i++) {
      if (key < bounds[i]) {
        partition = i;
        break;
      }
      partition = i + 1;
    }

    if (partition >= this.config.partitions) {
      partition = this.config.partitions - 1;
    }

    this.updateStats(partition);
    return partition;
  }

  private randomPartition(): number {
    const partition = Math.floor(Math.random() * this.config.partitions);
    this.updateStats(partition);
    return partition;
  }

  private updateStats(partition: number): void {
    this.stats.totalEvents++;
    this.stats.partitionCount[partition]++;
  }

  /**
   * Get current statistics
   */
  getStats(): PartitionStats {
    return {
      ...this.stats,
      partitionCount: [...this.stats.partitionCount],
    };
  }

  /**
   * Get number of partitions
   */
  get numPartitions(): number {
    return this.config.partitions;
  }

  /**
   * Rebalance partitions
   */
  rebalance(newPartitionCount: number): void {
    const oldCount = this.config.partitions;
    this.config.partitions = newPartitionCount;

    // Reset partition counts
    this.stats.partitionCount = new Array(newPartitionCount).fill(0);
    this.stats.rebalances++;

    // Calculate average events per partition
    if (oldCount > 0) {
      this.stats.rebalancesAvg = this.stats.totalEvents / newPartitionCount;
    }
  }
}

/**
 * Key extractor for extracting partition keys from events
 */
export class KeyExtractor<T> {
  private extractor: (value: T) => string;
  private fallback: string;
  private count: number = 0;
  private nullCount: number = 0;

  constructor(extractor: (value: T) => string, fallback: string = 'default') {
    this.extractor = extractor;
    this.fallback = fallback;
  }

  /**
   * Extract key from value
   */
  extract(value: T): string {
    this.count++;

    try {
      const key = this.extractor(value);
      if (!key) {
        this.nullCount++;
        return this.fallback;
      }
      return key;
    } catch {
      this.nullCount++;
      return this.fallback;
    }
  }

  /**
   * Set fallback key for null/empty results
   */
  setFallback(fallback: string): void {
    this.fallback = fallback;
  }

  /**
   * Get extraction statistics
   */
  getStats(): { count: number; nullCount: number } {
    return {
      count: this.count,
      nullCount: this.nullCount,
    };
  }
}

/**
 * Partition processor for handling events in a specific partition
 */
export class PartitionProcessor<T> {
  private partitionId: number;
  private handler: (event: StreamEvent<T>) => Promise<void>;
  private eventCount: number = 0;
  private errorCount: number = 0;

  constructor(partitionId: number, handler: (event: StreamEvent<T>) => Promise<void>) {
    this.partitionId = partitionId;
    this.handler = handler;
  }

  /**
   * Process an event
   */
  async process(event: StreamEvent<T>): Promise<void> {
    this.eventCount++;

    try {
      await this.handler(event);
    } catch (e) {
      this.errorCount++;
      throw e;
    }
  }

  /**
   * Get partition ID
   */
  get partitionID(): number {
    return this.partitionId;
  }

  /**
   * Get processor statistics
   */
  getStats(): { eventCount: number; errorCount: number } {
    return {
      eventCount: this.eventCount,
      errorCount: this.errorCount,
    };
  }
}

/**
 * Strategy weight tuple for composite partitioning
 */
export interface StrategyWeight<T> {
  strategy: PartitionStrategy;
  weight: number;
  extractor?: (value: T) => string;
}

/**
 * Composite partitioner combining multiple strategies
 */
export class CompositePartitioner<T> {
  private strategies: StrategyWeight<T>[];
  private partitioners: Partitioner[];
  private numPartitions: number;

  constructor(
    strategies: StrategyWeight<T>[],
    numPartitions: number
  ) {
    this.strategies = strategies;
    this.numPartitions = numPartitions;
    this.partitioners = [];

    // Create partitioners for each strategy
    for (const sw of strategies) {
      this.partitioners.push(new Partitioner({
        strategy: sw.strategy,
        partitions: numPartitions,
        keyExtractor: sw.extractor as (value: unknown) => string,
      }));
    }
  }

  /**
   * Get partition for a value using weighted strategies
   */
  partition(value: T): number {
    if (this.partitioners.length === 0) {
      return 0;
    }

    // Calculate total weight
    const totalWeight = this.strategies.reduce((sum, sw) => sum + sw.weight, 0);

    // Simple weighted selection - use first strategy's result for now
    // In a real implementation, you'd use proper weighted random selection
    const key = this.strategies[0]?.extractor?.(value) || '';
    return this.partitioners[0].partitionByKey(key);
  }

  /**
   * Add a new strategy
   */
  addStrategy(
    strategy: PartitionStrategy,
    weight: number,
    extractor?: (value: T) => string
  ): void {
    this.strategies.push({ strategy, weight, extractor });
    this.partitioners.push(new Partitioner({
      strategy,
      partitions: this.numPartitions,
      keyExtractor: extractor as (value: unknown) => string,
    }));
  }

  /**
   * Get number of partitions
   */
  get numPartitions(): number {
    return this.numPartitions;
  }
}
