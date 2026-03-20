/**
 * Zero-Copy Messaging for High-Throughput Streams
 * 
 * @module aether/streaming/zero_copy
 */

import { StreamEvent } from './types';

/**
 * Buffer statistics
 */
export interface BufferStats {
  totalBuffers: number;
  activeBuffers: number;
  freeBuffers: number;
  allocatedBytes: number;
  peakUsage: number;
  allocationCount: number;
  releaseCount: number;
}

/**
 * Memory pool for reusable buffers
 */
export class MemoryPool {
  private readonly bufferSize: number;
  private readonly initialSize: number;
  private buffers: ArrayBuffer[];
  private freeList: ArrayBuffer[];
  private stats: BufferStats;
  private lock: Promise<void> = Promise.resolve();

  constructor(bufferSize: number, initialCount: number) {
    this.bufferSize = bufferSize;
    this.initialSize = initialCount;
    this.buffers = [];
    this.freeList = [];

    // Pre-allocate initial buffers
    for (let i = 0; i < initialCount; i++) {
      const buf = new ArrayBuffer(bufferSize);
      this.buffers.push(buf);
      this.freeList.push(buf);
    }

    this.stats = {
      totalBuffers: initialCount,
      activeBuffers: 0,
      freeBuffers: initialCount,
      allocatedBytes: 0,
      peakUsage: 0,
      allocationCount: 0,
      releaseCount: 0,
    };
  }

  /**
   * Acquire a buffer from the pool
   */
  async acquire(): Promise<ArrayBuffer> {
    return this.withLock(() => {
      if (this.freeList.length > 0) {
        const buf = this.freeList.pop()!;
        this.stats.activeBuffers++;
        this.stats.freeBuffers--;
        this.stats.allocationCount++;
        return buf;
      }

      // Create new buffer if none available
      const buf = new ArrayBuffer(this.bufferSize);
      this.buffers.push(buf);
      this.stats.totalBuffers++;
      this.stats.activeBuffers++;
      this.stats.allocatedBytes += this.bufferSize;
      this.stats.allocationCount++;

      if (this.stats.activeBuffers > this.stats.peakUsage) {
        this.stats.peakUsage = this.stats.activeBuffers;
      }

      return buf;
    });
  }

  /**
   * Release a buffer back to the pool
   */
  async release(buffer: ArrayBuffer): Promise<void> {
    return this.withLock(() => {
      if (buffer.byteLength < this.bufferSize) {
        this.stats.activeBuffers--;
        this.stats.releaseCount++;
        return;
      }

      if (this.freeList.length < this.initialSize * 2) {
        this.freeList.push(buffer);
      }

      this.stats.activeBuffers--;
      this.stats.freeBuffers++;
      this.stats.releaseCount++;
    });
  }

  /**
   * Get current pool statistics
   */
  getStats(): BufferStats {
    return { ...this.stats };
  }

  private async withLock<T>(fn: () => T): Promise<T> {
    const oldLock = this.lock;
    let releaseLock: () => void;
    this.lock = new Promise((resolve) => {
      releaseLock = resolve;
    });
    await oldLock;
    try {
      return fn();
    } finally {
      releaseLock!();
    }
  }
}

/**
 * Pooled buffer with reference counting
 */
export class PooledBuffer {
  private data: ArrayBuffer;
  private pool: MemoryPool;
  private refCount: number;
  private released: boolean = false;

  constructor(pool: MemoryPool) {
    this.pool = pool;
    this.data = new ArrayBuffer(0); // Will be set by acquire
    this.refCount = 1;
  }

  /**
   * Initialize with a buffer from the pool
   */
  static async create(pool: MemoryPool): Promise<PooledBuffer> {
    const pb = new PooledBuffer(pool);
    pb.data = await pool.acquire();
    return pb;
  }

  /**
   * Get the underlying buffer
   */
  getData(): ArrayBuffer {
    return this.data;
  }

  /**
   * Get a view of the buffer as Uint8Array
   */
  asUint8Array(): Uint8Array {
    return new Uint8Array(this.data);
  }

  /**
   * Increment reference count
   */
  retain(): void {
    if (this.released) {
      throw new Error('Cannot retain a released buffer');
    }
    this.refCount++;
  }

  /**
   * Decrement reference count and release to pool when zero
   */
  async release(): Promise<void> {
    if (this.released) {
      return;
    }

    this.refCount--;
    if (this.refCount === 0) {
      this.released = true;
      await this.pool.release(this.data);
    }
  }

  /**
   * Get current reference count
   */
  getRefCount(): number {
    return this.refCount;
  }
}

/**
 * Zero-copy buffer wrapper
 */
export class ZeroCopyBuffer {
  private ptr: number;
  private size: number;
  private capacity: number;

  constructor(size: number) {
    const buffer = new ArrayBuffer(size);
    this.ptr = 0;
    this.size = size;
    this.capacity = size;
  }

  /**
   * Create from existing data without copying
   */
  static fromArrayBuffer(data: ArrayBuffer): ZeroCopyBuffer {
    const zcb = new ZeroCopyBuffer(0);
    zcb.ptr = 0;
    zcb.size = data.byteLength;
    zcb.capacity = data.byteLength;
    return zcb;
  }

  /**
   * Get current size
   */
  getSize(): number {
    return this.size;
  }

  /**
   * Get capacity
   */
  getCapacity(): number {
    return this.capacity;
  }
}

/**
 * Ring buffer for circular data storage
 */
export class RingBuffer {
  private data: Uint8Array;
  private size: number;
  private mask: number;
  private head: number = 0;
  private tail: number = 0;
  private writeLock: Promise<void> = Promise.resolve();
  private readLock: Promise<void> = Promise.resolve();

  constructor(size: number) {
    // Round up to power of 2
    let actualSize = 1;
    while (actualSize < size) {
      actualSize <<= 1;
    }
    this.data = new Uint8Array(actualSize);
    this.size = actualSize;
    this.mask = actualSize - 1;
  }

  /**
   * Write data to the ring buffer
   */
  async write(data: Uint8Array): Promise<number> {
    return this.withWriteLock(() => {
      const available = this.available();
      const toWrite = Math.min(data.length, available);

      for (let i = 0; i < toWrite; i++) {
        this.data[(this.tail + i) & this.mask] = data[i];
      }
      this.tail = (this.tail + toWrite) & this.mask;

      return toWrite;
    });
  }

  /**
   * Read data from the ring buffer
   */
  async read(dst: Uint8Array): Promise<number> {
    return this.withReadLock(() => {
      const available = this.length();
      const toRead = Math.min(dst.length, available);

      for (let i = 0; i < toRead; i++) {
        dst[i] = this.data[(this.head + i) & this.mask];
      }
      this.head = (this.head + toRead) & this.mask;

      return toRead;
    });
  }

  /**
   * Get current length
   */
  length(): number {
    return (this.tail - this.head + this.size) & this.mask;
  }

  /**
   * Get available space
   */
  available(): number {
    return this.size - this.length() - 1;
  }

  /**
   * Get total capacity
   */
  getCapacity(): number {
    return this.size;
  }

  /**
   * Clear the buffer
   */
  reset(): void {
    this.head = 0;
    this.tail = 0;
  }

  private async withWriteLock<T>(fn: () => T): Promise<T> {
    const oldLock = this.writeLock;
    let releaseLock: () => void;
    this.writeLock = new Promise((resolve) => {
      releaseLock = resolve;
    });
    await oldLock;
    try {
      return fn();
    } finally {
      releaseLock!();
    }
  }

  private async withReadLock<T>(fn: () => T): Promise<T> {
    const oldLock = this.readLock;
    let releaseLock: () => void;
    this.readLock = new Promise((resolve) => {
      releaseLock = resolve;
    });
    await oldLock;
    try {
      return fn();
    } finally {
      releaseLock!();
    }
  }
}

/**
 * Zero-copy emitter for high-throughput event emission
 */
export class ZeroCopyEmitter<T> {
  private pool: MemoryPool;
  private handlers: Array<(buf: PooledBuffer, event: StreamEvent<T>) => Promise<void>> = [];
  private lock: Promise<void> = Promise.resolve();

  constructor(pool: MemoryPool) {
    this.pool = pool;
  }

  /**
   * Add a handler for emitted events
   */
  addHandler(handler: (buf: PooledBuffer, event: StreamEvent<T>) => Promise<void>): void {
    this.handlers.push(handler);
  }

  /**
   * Emit an event to all handlers
   */
  async emit(buf: PooledBuffer, event: StreamEvent<T>): Promise<void> {
    buf.retain();
    try {
      for (const handler of this.handlers) {
        await handler(buf, event);
      }
    } finally {
      await buf.release();
    }
  }

  /**
   * Emit data without additional copying
   */
  async emitZeroCopy(data: Uint8Array, event: StreamEvent<T>): Promise<void> {
    const buf = await PooledBuffer.create(this.pool);
    const view = buf.asUint8Array();
    view.set(data);
    await this.emit(buf, event);
  }
}
