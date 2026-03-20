package io.aether.sdk.streaming;

import java.nio.ByteBuffer;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Statistics for buffer usage.
 */
public class BufferStats {
    public final AtomicLong totalBuffers = new AtomicLong(0);
    public final AtomicLong activeBuffers = new AtomicLong(0);
    public final AtomicLong freeBuffers = new AtomicLong(0);
    public final AtomicLong allocatedBytes = new AtomicLong(0);
    public final AtomicLong peakUsage = new AtomicLong(0);
    public final AtomicLong allocationCount = new AtomicLong(0);
    public final AtomicLong releaseCount = new AtomicLong(0);

    public BufferStats copy() {
        BufferStats copy = new BufferStats();
        copy.totalBuffers.set(totalBuffers.get());
        copy.activeBuffers.set(activeBuffers.get());
        copy.freeBuffers.set(freeBuffers.get());
        copy.allocatedBytes.set(allocatedBytes.get());
        copy.peakUsage.set(peakUsage.get());
        copy.allocationCount.set(allocationCount.get());
        copy.releaseCount.set(releaseCount.get());
        return copy;
    }
}

/**
 * Memory pool for reusable byte buffers.
 */
public class MemoryPool {
    private final int bufferSize;
    private final int initialSize;
    private final ConcurrentLinkedQueue<ByteBuffer> freeList = new ConcurrentLinkedQueue<>();
    private final BufferStats stats = new BufferStats();

    public MemoryPool(int bufferSize, int initialCount) {
        this.bufferSize = bufferSize;
        this.initialSize = initialCount;

        // Pre-allocate initial buffers
        for (int i = 0; i < initialCount; i++) {
            ByteBuffer buf = ByteBuffer.allocateDirect(bufferSize);
            freeList.offer(buf);
        }

        stats.totalBuffers.set(initialCount);
        stats.freeBuffers.set(initialCount);
    }

    /**
     * Acquire a buffer from the pool.
     */
    public ByteBuffer acquire() {
        ByteBuffer buf = freeList.poll();
        if (buf != null) {
            buf.clear();
            stats.activeBuffers.incrementAndGet();
            stats.freeBuffers.decrementAndGet();
            stats.allocationCount.incrementAndGet();
            return buf;
        }

        // Create new buffer if none available
        buf = ByteBuffer.allocateDirect(bufferSize);
        stats.totalBuffers.incrementAndGet();
        stats.activeBuffers.incrementAndGet();
        stats.allocatedBytes.addAndGet(bufferSize);
        stats.allocationCount.incrementAndGet();

        long currentActive = stats.activeBuffers.get();
        long peak = stats.peakUsage.get();
        while (currentActive > peak) {
            if (stats.peakUsage.compareAndSet(peak, currentActive)) {
                break;
            }
            peak = stats.peakUsage.get();
        }

        return buf;
    }

    /**
     * Release a buffer back to the pool.
     */
    public void release(ByteBuffer buffer) {
        if (buffer.capacity() < bufferSize) {
            stats.activeBuffers.decrementAndGet();
            stats.releaseCount.incrementAndGet();
            return;
        }

        if (freeList.size() < initialSize * 2) {
            buffer.clear();
            freeList.offer(buffer);
        }

        stats.activeBuffers.decrementAndGet();
        stats.freeBuffers.incrementAndGet();
        stats.releaseCount.incrementAndGet();
    }

    /**
     * Get current pool statistics.
     */
    public BufferStats getStats() {
        return stats.copy();
    }

    public int getBufferSize() {
        return bufferSize;
    }
}

/**
 * Pooled buffer with reference counting.
 */
public class PooledBuffer {
    private ByteBuffer data;
    private final MemoryPool pool;
    private final AtomicInteger refCount;
    private boolean released = false;

    public PooledBuffer(MemoryPool pool) {
        this.pool = pool;
        this.data = pool.acquire();
        this.refCount = new AtomicInteger(1);
    }

    /**
     * Get the underlying buffer.
     */
    public ByteBuffer getData() {
        return data;
    }

    /**
     * Increment reference count.
     */
    public void retain() {
        if (released) {
            throw new IllegalStateException("Cannot retain a released buffer");
        }
        refCount.incrementAndGet();
    }

    /**
     * Decrement reference count and release to pool when zero.
     */
    public void release() {
        if (released) {
            return;
        }

        if (refCount.decrementAndGet() == 0) {
            released = true;
            pool.release(data);
            data = null;
        }
    }

    /**
     * Get current reference count.
     */
    public int getRefCount() {
        return refCount.get();
    }
}

/**
 * Ring buffer for circular data storage.
 */
public class RingBuffer {
    private final byte[] data;
    private final int size;
    private final int mask;
    private final AtomicInteger head = new AtomicInteger(0);
    private final AtomicInteger tail = new AtomicInteger(0);
    private final Object writeLock = new Object();
    private final Object readLock = new Object();

    public RingBuffer(int size) {
        // Round up to power of 2
        int actualSize = 1;
        while (actualSize < size) {
            actualSize <<= 1;
        }
        this.data = new byte[actualSize];
        this.size = actualSize;
        this.mask = actualSize - 1;
    }

    /**
     * Write data to the ring buffer.
     */
    public int write(byte[] src, int offset, int length) {
        synchronized (writeLock) {
            int available = available();
            int toWrite = Math.min(length, available);

            for (int i = 0; i < toWrite; i++) {
                int tailIdx = tail.get();
                data[(tailIdx + i) & mask] = src[offset + i];
            }
            tail.addAndGet(toWrite);

            return toWrite;
        }
    }

    /**
     * Read data from the ring buffer.
     */
    public int read(byte[] dst, int offset, int length) {
        synchronized (readLock) {
            int available = length();
            int toRead = Math.min(length, available);

            for (int i = 0; i < toRead; i++) {
                int headIdx = head.get();
                dst[offset + i] = data[(headIdx + i) & mask];
            }
            head.addAndGet(toRead);

            return toRead;
        }
    }

    /**
     * Get current length.
     */
    public int length() {
        return tail.get() - head.get();
    }

    /**
     * Get available space.
     */
    public int available() {
        return size - length() - 1;
    }

    /**
     * Get total capacity.
     */
    public int getCapacity() {
        return size;
    }

    /**
     * Clear the buffer.
     */
    public void reset() {
        synchronized (writeLock) {
            synchronized (readLock) {
                head.set(0);
                tail.set(0);
            }
        }
    }
}

/**
 * Zero-copy emitter for high-throughput event emission.
 */
public class ZeroCopyEmitter<T> {
    private final MemoryPool pool;
    private final java.util.List<Handler<T>> handlers = new java.util.concurrent.CopyOnWriteArrayList<>();

    @FunctionalInterface
    public interface Handler<T> {
        void handle(PooledBuffer buffer, StreamEvent<T> event);
    }

    public ZeroCopyEmitter(MemoryPool pool) {
        this.pool = pool;
    }

    /**
     * Add a handler for emitted events.
     */
    public void addHandler(Handler<T> handler) {
        handlers.add(handler);
    }

    /**
     * Emit an event to all handlers.
     */
    public void emit(PooledBuffer buffer, StreamEvent<T> event) {
        buffer.retain();
        try {
            for (Handler<T> handler : handlers) {
                handler.handle(buffer, event);
            }
        } finally {
            buffer.release();
        }
    }

    /**
     * Emit data without additional copying.
     */
    public void emitZeroCopy(byte[] data, StreamEvent<T> event) {
        PooledBuffer buffer = new PooledBuffer(pool);
        buffer.getData().put(data);
        buffer.getData().flip();
        emit(buffer, event);
    }
}
