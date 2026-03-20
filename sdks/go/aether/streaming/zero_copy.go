package streaming

import (
	"sync"
	"sync/atomic"
	"unsafe"
)

// BufferStats tracks statistics for memory pool and buffer usage.
type BufferStats struct {
	TotalBuffers    int64
	ActiveBuffers   int64
	FreeBuffers     int64
	AllocatedBytes  int64
	PeakUsage       int64
	AllocationCount int64
	ReleaseCount    int64
}

// MemoryPool manages reusable byte buffers for zero-copy operations.
type MemoryPool struct {
	bufferSize  int
	initialSize int
	buffers     [][]byte
	freeList    chan []byte
	stats       BufferStats
	mu          sync.RWMutex
}

// NewMemoryPool creates a new memory pool with the specified buffer size and initial count.
func NewMemoryPool(bufferSize, initialCount int) *MemoryPool {
	pool := &MemoryPool{
		bufferSize:  bufferSize,
		initialSize: initialCount,
		freeList:    make(chan []byte, initialCount*2),
		buffers:     make([][]byte, 0, initialCount),
	}

	// Pre-allocate initial buffers
	for i := 0; i < initialCount; i++ {
		buf := make([]byte, bufferSize)
		pool.buffers = append(pool.buffers, buf)
		pool.freeList <- buf
	}

	pool.stats.TotalBuffers = int64(initialCount)
	pool.stats.FreeBuffers = int64(initialCount)

	return pool
}

// Acquire gets a buffer from the pool.
func (p *MemoryPool) Acquire() []byte {
	select {
	case buf := <-p.freeList:
		atomic.AddInt64(&p.stats.ActiveBuffers, 1)
		atomic.AddInt64(&p.stats.FreeBuffers, -1)
		atomic.AddInt64(&p.stats.AllocationCount, 1)
		return buf
	default:
		// Create new buffer if none available
		buf := make([]byte, p.bufferSize)
		p.mu.Lock()
		p.buffers = append(p.buffers, buf)
		p.mu.Unlock()
		atomic.AddInt64(&p.stats.TotalBuffers, 1)
		atomic.AddInt64(&p.stats.ActiveBuffers, 1)
		atomic.AddInt64(&p.stats.AllocatedBytes, int64(p.bufferSize))
		atomic.AddInt64(&p.stats.AllocationCount, 1)
		return buf
	}
}

// Release returns a buffer to the pool.
func (p *MemoryPool) Release(buf []byte) {
	if cap(buf) < p.bufferSize {
		return // Don't pool undersized buffers
	}

	select {
	case p.freeList <- buf:
		atomic.AddInt64(&p.stats.ActiveBuffers, -1)
		atomic.AddInt64(&p.stats.FreeBuffers, 1)
		atomic.AddInt64(&p.stats.ReleaseCount, 1)
	default:
		// Free list full, let GC handle it
		atomic.AddInt64(&p.stats.ActiveBuffers, -1)
		atomic.AddInt64(&p.stats.ReleaseCount, 1)
	}
}

// GetStats returns current pool statistics.
func (p *MemoryPool) GetStats() BufferStats {
	return BufferStats{
		TotalBuffers:    atomic.LoadInt64(&p.stats.TotalBuffers),
		ActiveBuffers:   atomic.LoadInt64(&p.stats.ActiveBuffers),
		FreeBuffers:     atomic.LoadInt64(&p.stats.FreeBuffers),
		AllocatedBytes:  atomic.LoadInt64(&p.stats.AllocatedBytes),
		PeakUsage:       atomic.LoadInt64(&p.stats.PeakUsage),
		AllocationCount: atomic.LoadInt64(&p.stats.AllocationCount),
		ReleaseCount:    atomic.LoadInt64(&p.stats.ReleaseCount),
	}
}

// PooledBuffer wraps a buffer from the pool with reference counting.
type PooledBuffer struct {
	data   []byte
	pool   *MemoryPool
	refCnt int32
}

// NewPooledBuffer creates a new pooled buffer.
func NewPooledBuffer(pool *MemoryPool) *PooledBuffer {
	return &PooledBuffer{
		data:   pool.Acquire(),
		pool:   pool,
		refCnt: 1,
	}
}

// Data returns the underlying byte slice.
func (pb *PooledBuffer) Data() []byte {
	return pb.data
}

// Retain increments the reference count.
func (pb *PooledBuffer) Retain() {
	atomic.AddInt32(&pb.refCnt, 1)
}

// Release decrements the reference count and returns buffer to pool when zero.
func (pb *PooledBuffer) Release() {
	if atomic.AddInt32(&pb.refCnt, -1) == 0 {
		pb.pool.Release(pb.data)
	}
}

// RefCount returns the current reference count.
func (pb *PooledBuffer) RefCount() int32 {
	return atomic.LoadInt32(&pb.refCnt)
}

// ZeroCopyBuffer provides a buffer for zero-copy operations.
type ZeroCopyBuffer struct {
	ptr      unsafe.Pointer
	size     int
	capacity int
}

// NewZeroCopyBuffer creates a new zero-copy buffer.
func NewZeroCopyBuffer(size int) *ZeroCopyBuffer {
	data := make([]byte, size)
	return &ZeroCopyBuffer{
		ptr:      unsafe.Pointer(&data[0]),
		size:     size,
		capacity: size,
	}
}

// FromSlice creates a ZeroCopyBuffer from an existing slice without copying.
func FromSlice(data []byte) *ZeroCopyBuffer {
	if len(data) == 0 {
		return &ZeroCopyBuffer{}
	}
	return &ZeroCopyBuffer{
		ptr:      unsafe.Pointer(&data[0]),
		size:     len(data),
		capacity: cap(data),
	}
}

// Size returns the current size of the buffer.
func (zcb *ZeroCopyBuffer) Size() int {
	return zcb.size
}

// Capacity returns the capacity of the buffer.
func (zcb *ZeroCopyBuffer) Capacity() int {
	return zcb.capacity
}

// AsSlice returns the buffer as a byte slice.
func (zcb *ZeroCopyBuffer) AsSlice() []byte {
	if zcb.ptr == nil {
		return nil
	}
	return (*[1 << 30]byte)(zcb.ptr)[:zcb.size:zcb.capacity]
}

// RingBuffer implements a thread-safe circular buffer.
type RingBuffer struct {
	data     []byte
	size     int64
	mask     int64
	head     int64
	tail     int64
	writeMu  sync.Mutex
	readMu   sync.Mutex
}

// NewRingBuffer creates a new ring buffer with the specified size (rounded to power of 2).
func NewRingBuffer(size int64) *RingBuffer {
	// Round up to power of 2 for efficient modulo
	actualSize := int64(1)
	for actualSize < size {
		actualSize <<= 1
	}
	return &RingBuffer{
		data: make([]byte, actualSize),
		size: actualSize,
		mask: actualSize - 1,
	}
}

// Write writes data to the ring buffer.
func (rb *RingBuffer) Write(data []byte) (int, error) {
	rb.writeMu.Lock()
	defer rb.writeMu.Unlock()

	available := rb.available()
	if len(data) > int(available) {
		data = data[:available]
	}

	for i, b := range data {
		rb.data[(rb.tail+int64(i))&rb.mask] = b
	}
	atomic.AddInt64(&rb.tail, int64(len(data)))

	return len(data), nil
}

// Read reads data from the ring buffer.
func (rb *RingBuffer) Read(dst []byte) (int, error) {
	rb.readMu.Lock()
	defer rb.readMu.Unlock()

	available := rb.Len()
	if available == 0 {
		return 0, nil
	}

	toRead := len(dst)
	if int(available) < toRead {
		toRead = int(available)
	}

	for i := 0; i < toRead; i++ {
		dst[i] = rb.data[(rb.head+int64(i))&rb.mask]
	}
	atomic.AddInt64(&rb.head, int64(toRead))

	return toRead, nil
}

// Len returns the current number of bytes in the buffer.
func (rb *RingBuffer) Len() int64 {
	return atomic.LoadInt64(&rb.tail) - atomic.LoadInt64(&rb.head)
}

// Available returns the number of bytes that can be written.
func (rb *RingBuffer) available() int64 {
	return rb.size - rb.Len()
}

// Capacity returns the total capacity of the buffer.
func (rb *RingBuffer) Capacity() int64 {
	return rb.size
}

// Reset clears the buffer.
func (rb *RingBuffer) Reset() {
	rb.writeMu.Lock()
	rb.readMu.Lock()
	rb.head = 0
	rb.tail = 0
	rb.readMu.Unlock()
	rb.writeMu.Unlock()
}

// ZeroCopyEmitter emits events with zero-copy optimization.
type ZeroCopyEmitter[T any] struct {
	pool     *MemoryPool
	handlers []func(*PooledBuffer, StreamEvent[T]) error
	mu       sync.RWMutex
}

// NewZeroCopyEmitter creates a new zero-copy emitter.
func NewZeroCopyEmitter[T any](pool *MemoryPool) *ZeroCopyEmitter[T] {
	return &ZeroCopyEmitter[T]{
		pool:     pool,
		handlers: make([]func(*PooledBuffer, StreamEvent[T]) error, 0),
	}
}

// AddHandler registers a handler for emitted events.
func (e *ZeroCopyEmitter[T]) AddHandler(handler func(*PooledBuffer, StreamEvent[T]) error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.handlers = append(e.handlers, handler)
}

// Emit sends an event to all registered handlers.
func (e *ZeroCopyEmitter[T]) Emit(buf *PooledBuffer, event StreamEvent[T]) error {
	buf.Retain()
	defer buf.Release()

	e.mu.RLock()
	defer e.mu.RUnlock()

	for _, handler := range e.handlers {
		if err := handler(buf, event); err != nil {
			return err
		}
	}
	return nil
}

// EmitZeroCopy emits data without additional copying.
func (e *ZeroCopyEmitter[T]) EmitZeroCopy(data []byte, event StreamEvent[T]) error {
	buf := NewPooledBuffer(e.pool)
	copy(buf.Data(), data)
	return e.Emit(buf, event)
}
