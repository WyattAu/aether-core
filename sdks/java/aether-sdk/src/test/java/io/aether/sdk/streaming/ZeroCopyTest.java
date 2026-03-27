package io.aether.sdk.streaming;

import io.aether.sdk.streaming.Types.StreamEvent;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicReference;

class ZeroCopyTest {

    @Test
    @DisplayName("MemoryPool acquire returns buffer")
    void testAcquire() {
        MemoryPool pool = new MemoryPool(1024, 2);
        ByteBuffer buf = pool.acquire();
        assertNotNull(buf);
        assertEquals(1024, buf.capacity());
    }

    @Test
    @DisplayName("MemoryPool release returns buffer")
    void testRelease() {
        MemoryPool pool = new MemoryPool(1024, 2);
        ByteBuffer buf = pool.acquire();
        pool.release(buf);
        BufferStats stats = pool.getStats();
        assertEquals(2, stats.freeBuffers.get());
    }

    @Test
    @DisplayName("MemoryPool stats track allocations")
    void testPoolStats() {
        MemoryPool pool = new MemoryPool(1024, 2);
        pool.acquire();
        pool.acquire();
        BufferStats stats = pool.getStats();
        assertEquals(2, stats.totalBuffers.get());
        assertEquals(2, stats.activeBuffers.get());
        assertEquals(2, stats.allocationCount.get());
    }

    @Test
    @DisplayName("MemoryPool creates new buffer when exhausted")
    void testPoolExhausted() {
        MemoryPool pool = new MemoryPool(1024, 1);
        ByteBuffer b1 = pool.acquire();
        ByteBuffer b2 = pool.acquire();
        assertNotNull(b2);
        assertEquals(1024, b2.capacity());
        pool.release(b1);
        pool.release(b2);
    }

    @Test
    @DisplayName("MemoryPool discards undersized buffers")
    void testPoolDiscardSmall() {
        MemoryPool pool = new MemoryPool(1024, 2);
        ByteBuffer small = ByteBuffer.allocateDirect(512);
        pool.release(small);
        BufferStats stats = pool.getStats();
        assertEquals(0, stats.freeBuffers.get());
    }

    @Test
    @DisplayName("MemoryPool peak usage tracking")
    void testPoolPeakUsage() {
        MemoryPool pool = new MemoryPool(1024, 1);
        ByteBuffer b1 = pool.acquire();
        ByteBuffer b2 = pool.acquire();
        ByteBuffer b3 = pool.acquire();
        pool.release(b1);
        pool.release(b2);
        pool.release(b3);
        BufferStats stats = pool.getStats();
        assertTrue(stats.peakUsage.get() >= 2);
    }

    @Test
    @DisplayName("PooledBuffer initial ref count is 1")
    void testPooledBufferRefCount() {
        MemoryPool pool = new MemoryPool(1024, 2);
        PooledBuffer pb = new PooledBuffer(pool);
        assertEquals(1, pb.getRefCount());
        assertNotNull(pb.getData());
    }

    @Test
    @DisplayName("PooledBuffer retain increments ref count")
    void testPooledBufferRetain() {
        MemoryPool pool = new MemoryPool(1024, 2);
        PooledBuffer pb = new PooledBuffer(pool);
        pb.retain();
        assertEquals(2, pb.getRefCount());
    }

    @Test
    @DisplayName("PooledBuffer release decrements ref count")
    void testPooledBufferRelease() {
        MemoryPool pool = new MemoryPool(1024, 2);
        PooledBuffer pb = new PooledBuffer(pool);
        pb.retain();
        pb.release();
        assertEquals(1, pb.getRefCount());
        assertNotNull(pb.getData());
    }

    @Test
    @DisplayName("PooledBuffer release at zero returns to pool")
    void testPooledBufferReleaseAtZero() {
        MemoryPool pool = new MemoryPool(1024, 2);
        PooledBuffer pb = new PooledBuffer(pool);
        pb.release();
        assertEquals(0, pb.getRefCount());
        assertTrue(pool.getStats().freeBuffers.get() > 0);
    }

    @Test
    @DisplayName("PooledBuffer retain after release throws")
    void testPooledBufferRetainAfterRelease() {
        MemoryPool pool = new MemoryPool(1024, 2);
        PooledBuffer pb = new PooledBuffer(pool);
        pb.release();
        assertThrows(IllegalStateException.class, pb::retain);
    }

    @Test
    @DisplayName("PooledBuffer double release is safe")
    void testPooledBufferDoubleRelease() {
        MemoryPool pool = new MemoryPool(1024, 2);
        PooledBuffer pb = new PooledBuffer(pool);
        pb.release();
        assertDoesNotThrow(pb::release);
    }

    @Test
    @DisplayName("RingBuffer write and read")
    void testRingBufferWriteRead() {
        RingBuffer rb = new RingBuffer(64);
        byte[] data = {1, 2, 3, 4, 5};
        int written = rb.write(data, 0, data.length);
        assertEquals(5, written);
        assertEquals(5, rb.length());

        byte[] out = new byte[5];
        int read = rb.read(out, 0, out.length);
        assertEquals(5, read);
        assertArrayEquals(data, out);
        assertEquals(0, rb.length());
    }

    @Test
    @DisplayName("RingBuffer capacity is power of 2")
    void testRingBufferPowerOfTwo() {
        RingBuffer rb = new RingBuffer(10);
        assertEquals(16, rb.getCapacity());
    }

    @Test
    @DisplayName("RingBuffer available space")
    void testRingBufferAvailable() {
        RingBuffer rb = new RingBuffer(16);
        int cap = rb.getCapacity();
        assertEquals(cap - 1, rb.available());
    }

    @Test
    @DisplayName("RingBuffer reset clears data")
    void testRingBufferReset() {
        RingBuffer rb = new RingBuffer(64);
        rb.write(new byte[]{1, 2, 3}, 0, 3);
        rb.reset();
        assertEquals(0, rb.length());
        assertEquals(rb.getCapacity() - 1, rb.available());
    }

    @Test
    @DisplayName("RingBuffer wrap-around")
    void testRingBufferWrapAround() {
        RingBuffer rb = new RingBuffer(16);
        byte[] data = new byte[8];
        for (int i = 0; i < data.length; i++) data[i] = (byte) i;
        rb.write(data, 0, 8);
        byte[] out = new byte[4];
        rb.read(out, 0, 4);
        rb.write(new byte[]{10, 11, 12, 13}, 0, 4);
        assertEquals(8, rb.length());
    }

    @Test
    @DisplayName("BufferStats copy")
    void testBufferStatsCopy() {
        BufferStats stats = new BufferStats();
        stats.totalBuffers.set(10);
        stats.activeBuffers.set(3);
        BufferStats copy = stats.copy();
        assertEquals(10, copy.totalBuffers.get());
        assertEquals(3, copy.activeBuffers.get());
        copy.totalBuffers.set(99);
        assertEquals(10, stats.totalBuffers.get());
    }

    @Test
    @DisplayName("ZeroCopyEmitter delivers to handlers")
    void testZeroCopyEmitter() {
        MemoryPool pool = new MemoryPool(1024, 2);
        ZeroCopyEmitter<String> emitter = new ZeroCopyEmitter<>(pool);
        AtomicReference<PooledBuffer> receivedBuf = new AtomicReference<>();
        emitter.addHandler((buf, event) -> receivedBuf.set(buf));
        PooledBuffer pb = new PooledBuffer(pool);
        StreamEvent<String> event = StreamEvent.create("k", "v");
        emitter.emit(pb, event);
        assertNotNull(receivedBuf.get());
    }
}
