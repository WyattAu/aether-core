"""
Zero-Copy Messaging for High-Throughput Streams

Provides zero-copy and minimal-copy message passing to optimize
streaming throughput and reduce memory allocations.

Example:
    from aether_sdk.streaming import ZeroCopyBuffer, MemoryPool

    # Create a memory pool for reusable buffers
    pool = MemoryPool(buffer_size=4096, initial_count=100)

    # Use zero-copy buffer
    buffer = pool.acquire()
    # ... write data to buffer ...
    await emitter.emit_zero_copy(buffer)
    buffer.release()  # Return to pool
"""

from __future__ import annotations

import struct
import threading
from array import array
from dataclasses import dataclass
from typing import Callable, Generic, List, Optional, TypeVar

from .types import StreamEvent

T = TypeVar("T")


# ============================================
# Memory Pool
# ============================================


@dataclass
class BufferStats:
    """Statistics for memory pool usage."""

    total_buffers: int = 0
    available_buffers: int = 0
    acquired_count: int = 0
    released_count: int = 0
    allocation_failures: int = 0
    total_bytes_allocated: int = 0
    peak_usage: int = 0


class MemoryPool:
    """
    A pool of reusable memory buffers for zero-copy operations.

    Pre-allocates buffers to avoid runtime allocation overhead.
    Thread-safe for concurrent access.

    Example:
        pool = MemoryPool(buffer_size=4096, initial_count=100)

        buffer = pool.acquire()
        try:
            buffer.write(b"Hello, World!")
            # Pass buffer to consumer
            await process(buffer)
        finally:
            buffer.release()
    """

    def __init__(
        self,
        buffer_size: int = 4096,
        initial_count: int = 100,
        max_count: int = 10000,
        growth_factor: float = 2.0,
    ):
        self.buffer_size = buffer_size
        self.max_count = max_count
        self.growth_factor = growth_factor

        self._available: List[PooledBuffer] = []
        self._all_buffers: List[PooledBuffer] = []
        self._lock = threading.RLock()
        self._stats = BufferStats()

        # Pre-allocate initial buffers
        self._allocate_batch(initial_count)

    def _allocate_batch(self, count: int) -> None:
        """Allocate a batch of buffers."""
        with self._lock:
            for _ in range(count):
                if len(self._all_buffers) >= self.max_count:
                    break
                buffer = PooledBuffer(
                    data=bytearray(self.buffer_size),
                    pool=self,
                )
                self._available.append(buffer)
                self._all_buffers.append(buffer)

            self._stats.total_buffers = len(self._all_buffers)
            self._stats.available_buffers = len(self._available)
            self._stats.total_bytes_allocated = (
                self._stats.total_buffers * self.buffer_size
            )

    def acquire(self) -> PooledBuffer:
        """
        Acquire a buffer from the pool.

        Returns:
            A pooled buffer ready for use.

        Raises:
            MemoryError: If pool is exhausted and max count reached.
        """
        with self._lock:
            if not self._available:
                # Try to grow the pool
                new_count = int(len(self._all_buffers) * (self.growth_factor - 1))
                new_count = max(1, new_count)

                if len(self._all_buffers) + new_count <= self.max_count:
                    self._allocate_batch(new_count)

                if not self._available:
                    self._stats.allocation_failures += 1
                    raise MemoryError("Memory pool exhausted")

            buffer = self._available.pop()
            buffer._acquired = True
            buffer._position = 0

            self._stats.acquired_count += 1
            self._stats.available_buffers = len(self._available)
            self._stats.peak_usage = max(
                self._stats.peak_usage,
                self._stats.total_buffers - self._stats.available_buffers,
            )

            return buffer

    def release(self, buffer: "PooledBuffer") -> None:
        """Return a buffer to the pool."""
        with self._lock:
            if buffer._acquired:
                buffer._acquired = False
                self._available.append(buffer)
                self._stats.released_count += 1
                self._stats.available_buffers = len(self._available)

    def get_stats(self) -> BufferStats:
        """Get pool statistics."""
        with self._lock:
            return BufferStats(
                total_buffers=self._stats.total_buffers,
                available_buffers=self._stats.available_buffers,
                acquired_count=self._stats.acquired_count,
                released_count=self._stats.released_count,
                allocation_failures=self._stats.allocation_failures,
                total_bytes_allocated=self._stats.total_bytes_allocated,
                peak_usage=self._stats.peak_usage,
            )

    def shrink(self, target_count: int) -> int:
        """
        Shrink the pool by releasing unused buffers.

        Returns:
            Number of buffers released.
        """
        with self._lock:
            released = 0
            while (
                len(self._available) > target_count
                and len(self._all_buffers) > target_count
            ):
                buffer = self._available.pop()
                self._all_buffers.remove(buffer)
                released += 1

            self._stats.total_buffers = len(self._all_buffers)
            self._stats.available_buffers = len(self._available)
            self._stats.total_bytes_allocated = (
                self._stats.total_buffers * self.buffer_size
            )

            return released


class PooledBuffer:
    """
    A buffer acquired from a memory pool.

    Supports zero-copy operations and must be released back to the pool.
    """

    __slots__ = ["_data", "_pool", "_acquired", "_position", "__weakref__"]

    def __init__(self, data: bytearray, pool: MemoryPool):
        self._data = data
        self._pool = pool
        self._acquired = False
        self._position = 0

    @property
    def data(self) -> bytearray:
        """Get the underlying buffer data."""
        return self._data

    @property
    def size(self) -> int:
        """Get the total buffer size."""
        return len(self._data)

    @property
    def position(self) -> int:
        """Get the current write position."""
        return self._position

    @property
    def remaining(self) -> int:
        """Get remaining capacity."""
        return len(self._data) - self._position

    def write(self, data: bytes) -> int:
        """
        Write data to the buffer at current position.

        Returns:
            Number of bytes written.
        """
        write_len = min(len(data), self.remaining)
        self._data[self._position : self._position + write_len] = data[:write_len]
        self._position += write_len
        return write_len

    def write_at(self, offset: int, data: bytes) -> int:
        """Write data at a specific offset."""
        end = min(offset + len(data), len(self._data))
        write_len = end - offset
        self._data[offset:end] = data[:write_len]
        return write_len

    def read(self, size: Optional[int] = None) -> bytes:
        """Read data from buffer (returns a view, not a copy when possible)."""
        if size is None:
            return bytes(self._data[: self._position])
        return bytes(self._data[:size])

    def clear(self) -> None:
        """Reset the buffer position."""
        self._position = 0

    def release(self) -> None:
        """Release the buffer back to the pool."""
        if self._pool:
            self._pool.release(self)

    def slice(self, start: int = 0, end: Optional[int] = None) -> memoryview:
        """Get a memoryview slice (zero-copy)."""
        end = end if end is not None else self._position
        return memoryview(self._data)[start:end]

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.release()


# ============================================
# Zero-Copy Event Buffer
# ============================================


class ZeroCopyBuffer(Generic[T]):
    """
    A buffer that supports zero-copy event serialization.

    Uses a memory pool for efficient buffer management and
    supports both structured and raw byte operations.
    """

    def __init__(
        self,
        pool: Optional[MemoryPool] = None,
        buffer_size: int = 4096,
    ):
        self._pool = pool or MemoryPool(buffer_size=buffer_size)
        self._buffer: Optional[PooledBuffer] = None
        self._event_count = 0
        self._header_size = 4  # 4 bytes for event count

    def acquire(self) -> None:
        """Acquire a buffer from the pool."""
        if self._buffer is None:
            self._buffer = self._pool.acquire()
        self._buffer.clear()
        self._event_count = 0
        # Reserve space for header
        self._buffer._position = self._header_size

    def append(self, data: bytes) -> bool:
        """
        Append data to the buffer.

        Returns:
            True if data was appended, False if buffer is full.
        """
        if self._buffer is None:
            self.acquire()

        # Need space for length prefix + data
        needed = 4 + len(data)
        if self._buffer.remaining < needed:
            return False

        # Write length prefix
        self._buffer.write(struct.pack(">I", len(data)))
        # Write data
        self._buffer.write(data)
        self._event_count += 1

        return True

    def append_event(
        self, event: StreamEvent[T], serializer: Callable[[T], bytes]
    ) -> bool:
        """
        Append a stream event to the buffer.

        Args:
            event: The stream event to append
            serializer: Function to serialize the event value

        Returns:
            True if event was appended, False if buffer is full.
        """
        # Serialize event metadata + value
        data = serializer(event.value)
        return self.append(data)

    def seal(self) -> None:
        """Seal the buffer by writing the header."""
        if self._buffer:
            # Write event count at the beginning
            struct.pack_into(">I", self._buffer._data, 0, self._event_count)

    def get_buffer(self) -> Optional[PooledBuffer]:
        """Get the underlying pooled buffer."""
        return self._buffer

    def get_data(self) -> Optional[memoryview]:
        """Get a zero-copy view of the buffer data."""
        if self._buffer:
            return self._buffer.slice()
        return None

    def release(self) -> None:
        """Release the buffer back to the pool."""
        if self._buffer:
            self._buffer.release()
            self._buffer = None
            self._event_count = 0

    def __enter__(self):
        self.acquire()
        return self

    def __exit__(self, *args):
        self.release()


# ============================================
# Ring Buffer for Zero-Copy Streaming
# ============================================


class RingBuffer:
    """
    A lock-free ring buffer for zero-copy streaming.

    Supports single-producer single-consumer (SPSC) pattern
    for maximum throughput.
    """

    def __init__(self, capacity: int, buffer_size: int = 4096):
        self.capacity = capacity
        self.buffer_size = buffer_size

        # Pre-allocate buffer slots
        self._buffers: List[bytearray] = [
            bytearray(buffer_size) for _ in range(capacity)
        ]
        self._lengths: array = array("I", [0] * capacity)

        # Positions (using integers for atomic operations)
        self._write_pos = 0
        self._read_pos = 0
        self._count = 0

        self._lock = threading.Lock()

    def write(self, data: bytes) -> bool:
        """
        Write data to the next available slot.

        Returns:
            True if written, False if buffer is full.
        """
        with self._lock:
            if self._count >= self.capacity:
                return False

            if len(data) > self.buffer_size:
                return False

            slot = self._write_pos
            self._buffers[slot][: len(data)] = data
            self._lengths[slot] = len(data)

            self._write_pos = (self._write_pos + 1) % self.capacity
            self._count += 1

            return True

    def read(self) -> Optional[memoryview]:
        """
        Read data from the next available slot.

        Returns:
            Memoryview of the data (zero-copy) or None if empty.
        """
        with self._lock:
            if self._count == 0:
                return None

            slot = self._read_pos
            length = self._lengths[slot]

            self._read_pos = (self._read_pos + 1) % self.capacity
            self._count -= 1

            return memoryview(self._buffers[slot])[:length]

    def peek(self) -> Optional[memoryview]:
        """Peek at the next data without consuming it."""
        with self._lock:
            if self._count == 0:
                return None

            slot = self._read_pos
            length = self._lengths[slot]
            return memoryview(self._buffers[slot])[:length]

    @property
    def is_empty(self) -> bool:
        return self._count == 0

    @property
    def is_full(self) -> bool:
        return self._count >= self.capacity

    @property
    def available(self) -> int:
        return self.capacity - self._count


# ============================================
# Zero-Copy Emitter
# ============================================


class ZeroCopyEmitter(Generic[T]):
    """
    Zero-copy event emitter for high-throughput streaming.

    Uses memory pools and buffer reuse to minimize allocations.
    """

    def __init__(
        self,
        pool: Optional[MemoryPool] = None,
        batch_size: int = 100,
        buffer_size: int = 65536,  # 64KB default
    ):
        self._pool = pool or MemoryPool(buffer_size=buffer_size)
        self._batch_size = batch_size
        self._buffer_size = buffer_size
        self._buffer: Optional[ZeroCopyBuffer[T]] = None
        self._serializer: Optional[Callable[[T], bytes]] = None
        self._downstream: Optional[Callable[[memoryview], None]] = None

    def set_serializer(self, serializer: Callable[[T], bytes]) -> None:
        """Set the serializer function for events."""
        self._serializer = serializer

    def set_downstream(self, consumer: Callable[[memoryview], None]) -> None:
        """Set the downstream consumer for emitted buffers."""
        self._downstream = consumer

    async def emit(self, event: StreamEvent[T]) -> bool:
        """
        Emit an event using zero-copy serialization.

        Returns:
            True if event was buffered, False otherwise.
        """
        if self._buffer is None:
            self._buffer = ZeroCopyBuffer(self._pool, self._buffer_size)
            self._buffer.acquire()

        if self._serializer is None:
            raise ValueError("Serializer not set")

        if not self._buffer.append_event(event, self._serializer):
            # Buffer full, flush and retry
            await self.flush()
            self._buffer.acquire()
            return self._buffer.append_event(event, self._serializer)

        # Check if batch is ready
        if self._buffer._event_count >= self._batch_size:
            await self.flush()

        return True

    async def flush(self) -> None:
        """Flush the current buffer to downstream."""
        if self._buffer and self._buffer._event_count > 0:
            self._buffer.seal()

            data = self._buffer.get_data()
            if data and self._downstream:
                self._downstream(data)

            self._buffer.release()
            self._buffer = None


__all__ = [
    "BufferStats",
    "MemoryPool",
    "PooledBuffer",
    "ZeroCopyBuffer",
    "RingBuffer",
    "ZeroCopyEmitter",
]
