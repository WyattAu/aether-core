# Memory Management Strategy
**Aether Resource Management - Phase 3.5**
**Document ID**: RM-MEM-001
**Version**: 1.0
**Date**: 2026-03-05
**Status**: Final

---

## 1. Overview

Aether implements a **zero-allocation hot path** memory strategy using mimalloc pools with strict per-actor memory limits to ensure deterministic resource consumption and eliminate memory-related attack vectors.

---

## 2. Memory Allocation Strategy

### 2.1 Allocation Tiers

| Tier | Description | Allocation Policy | Performance Target |
|------|-------------|-------------------|-------------------|
| **Hot Path** | Message dispatch, syscall handling | **ZERO ALLOCATION** | <100ns |
| **Warm Path** | Actor initialization, capability grants | Pool allocation only | <1µs |
| **Cold Path** | Actor spawn, VM creation | General heap | <10ms |

### 2.2 Hot Path Definition

The hot path includes:
- Message routing and dispatch
- Capability validation
- Syscall entry/exit
- Inter-actor communication
- I/O submission/completion

**Requirement**: No heap allocation permitted on hot path. All necessary structures must be pre-allocated.

### 2.3 Memory Regions

```
┌─────────────────────────────────────────────────────────┐
│ Host Runtime Memory Space                                │
├─────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────────────┐ │
│ │ Global Pools (mimalloc managed)                      │ │
│ │ • Message pool (64-byte blocks, 10K capacity)       │ │
│ │ • Capability pool (128-byte blocks, 5K capacity)    │ │
│ │ • Handle pool (64-byte blocks, 10K capacity)        │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ Actor Memory Regions (per-actor isolation)           │ │
│ │ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐  │ │
│ │ │ Actor A      │ │ Actor B      │ │ Actor C      │  │ │
│ │ │ (WASM: 16MB) │ │ (WASM: 16MB) │ │ (VM: 128MB)  │  │ │
│ │ └──────────────┘ └──────────────┘ └──────────────┘  │ │
│ └─────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ Shared Memory (capability-controlled)                │ │
│ │ • Ring buffers for IPC                              │ │
│ │ • Shared capability tables                          │ │
│ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

## 3. mimalloc Pool Configuration

### 3.1 Why mimalloc?

- **Performance**: 2-5x faster than system allocators
- **Fragmentation**: Minimal heap fragmentation
- **Security**: Page-level isolation, guard pages
- **Observability**: Built-in statistics and tracking

### 3.2 Pool Configuration

```rust
pub struct AetherMemoryPools {
    pub message_pool: MiPool,
    pub capability_pool: MiPool,
    pub handle_pool: MiPool,
    pub buffer_pool: MiPool,
}

impl AetherMemoryPools {
    pub fn new() -> Self {
        let message_pool = mi_pool_create(
            MiPoolOptions {
                block_size: 64,          // Message headers
                capacity: 10_000,         // Max in-flight messages
                page_size: 64 * 1024,    // 64KB pages
                thread_affinity: true,   // NUMA-aware
            }
        );

        let capability_pool = mi_pool_create(
            MiPoolOptions {
                block_size: 128,         // Capability entries
                capacity: 5_000,          // Max capabilities
                page_size: 64 * 1024,
                thread_affinity: true,
            }
        );

        let handle_pool = mi_pool_create(
            MiPoolOptions {
                block_size: 64,          // Handle descriptors
                capacity: 10_000,         // Max handles
                page_size: 64 * 1024,
                thread_affinity: true,
            }
        );

        let buffer_pool = mi_pool_create(
            MiPoolOptions {
                block_size: 4096,        // Small buffers
                capacity: 1_000,          // Max buffers
                page_size: 4 * 1024 * 1024, // 4MB pages
                thread_affinity: true,
            }
        );

        Self {
            message_pool,
            capability_pool,
            handle_pool,
            buffer_pool,
        }
    }
}
```

### 3.3 Pool Statistics (Telemetry)

```rust
pub struct PoolStats {
    pub allocations_total: u64,
    pub deallocations_total: u64,
    pub current_blocks: u64,
    pub peak_blocks: u64,
    pub fragmentation_ratio: f32,
    pub pages_reserved: u64,
    pub pages_committed: u64,
}

impl AetherMemoryPools {
    pub fn collect_stats(&self) -> HashMap<&'static str, PoolStats> {
        let mut stats = HashMap::new();
        
        stats.insert("message", self.message_pool.stats());
        stats.insert("capability", self.capability_pool.stats());
        stats.insert("handle", self.handle_pool.stats());
        stats.insert("buffer", self.buffer_pool.stats());
        
        stats
    }
}
```

---

## 4. Hot Path Allocation Ban

### 4.1 Static Enforcement

**Compile-time check using Rust's type system**:

```rust
#[cfg_attr(feature = "hot-path", deny(unused_allocation))]
pub struct HotPathContext<'a> {
    message_buffer: &'a mut [u8; 4096],
    capability_cache: &'a CapabilityCache,
    handle_table: &'a HandleTable,
}

impl<'a> HotPathContext<'a> {
    pub fn new(
        message_buffer: &'a mut [u8; 4096],
        capability_cache: &'a CapabilityCache,
        handle_table: &'a HandleTable,
    ) -> Self {
        Self {
            message_buffer,
            capability_cache,
            handle_table,
        }
    }

    #[inline(always)]
    pub fn dispatch_message(&mut self, msg: &Message) -> Result<(), DispatchError> {
        // All necessary data in pre-allocated buffers
        // NO ALLOCATION PERMITTED
        self.process_in_place(msg, &mut self.message_buffer)
    }
}
```

### 4.2 Runtime Enforcement

```rust
#[cfg(debug_assertions)]
thread_local! {
    static IN_HOT_PATH: Cell<bool> = Cell::new(false);
}

#[cfg(debug_assertions)]
pub fn enter_hot_path() {
    IN_HOT_PATH.with(|flag| flag.set(true));
}

#[cfg(debug_assertions)]
pub fn exit_hot_path() {
    IN_HOT_PATH.with(|flag| flag.set(false));
}

#[cfg(debug_assertions)]
pub fn check_allocation() {
    IN_HOT_PATH.with(|flag| {
        if flag.get() {
            panic!("Allocation on hot path detected!");
        }
    });
}

#[cfg(debug_assertions)]
#[global_allocator]
struct CheckingAllocator;

unsafe impl GlobalAlloc for CheckingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        check_allocation();
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}
```

### 4.3 Custom Allocator Tracking

```rust
pub struct TrackingAllocator {
    inner: mimalloc::MiMalloc,
    stats: Arc<AtomicMemoryStats>,
}

pub struct AtomicMemoryStats {
    allocations: AtomicU64,
    deallocations: AtomicU64,
    bytes_allocated: AtomicU64,
    peak_bytes: AtomicU64,
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        check_allocation();
        let ptr = self.inner.alloc(layout);
        if !ptr.is_null() {
            self.stats.allocations.fetch_add(1, Ordering::Relaxed);
            self.stats.bytes_allocated.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.dealloc(ptr, layout);
        self.stats.deallocations.fetch_add(1, Ordering::Relaxed);
        self.stats.bytes_allocated.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}
```

---

## 5. Stack vs Heap Allocation Policy

### 5.1 Decision Matrix

| Scenario | Stack | Heap | Rationale |
|----------|-------|------|-----------|
| Small, fixed-size (<1KB) | ✅ | ❌ | Performance |
| Large or variable size | ❌ | ✅ | Stack overflow prevention |
| Long-lived across function calls | ❌ | ✅ | Lifetime requirements |
| Temporary, function-scoped | ✅ | ❌ | Automatic cleanup |
| Shared across threads | ❌ | ✅ | Sync requirements |
| Performance-critical | ✅ | ❌ | Avoid allocator overhead |

### 5.2 Stack Allocation Guidelines

```rust
const MAX_STACK_BUFFER: usize = 1024;

pub fn process_data(data: &[u8]) -> Result<ProcessedData, Error> {
    if data.len() <= MAX_STACK_BUFFER {
        let mut buffer = [0u8; MAX_STACK_BUFFER];
        process_in_place(data, &mut buffer[..data.len()])
    } else {
        let mut buffer = Vec::with_capacity(data.len());
        buffer.extend_from_slice(data);
        process_in_place(data, &mut buffer)
    }
}
```

### 5.3 Heap Allocation Guidelines

```rust
pub struct HeapAllocated {
    data: Box<[u8]>,
}

impl HeapAllocated {
    pub fn new(size: usize) -> Self {
        let data = vec![0u8; size].into_boxed_slice();
        Self { data }
    }
}
```

---

## 6. Memory Limits per Actor Tier

### 6.1 Actor Tiers

| Tier | Actor Type | Linear Memory | Heap Limit | Total Limit | Rationale |
|------|------------|---------------|------------|-------------|-----------|
| **Tier 0** | System actors | 64 MB | 32 MB | 96 MB | Core services |
| **Tier 1** | Trusted actors | 32 MB | 16 MB | 48 MB | Privileged services |
| **Tier 2** | User actors | 16 MB | 8 MB | 24 MB | Standard user code |
| **Tier 3** | Untrusted actors | 8 MB | 4 MB | 12 MB | Sandboxed code |
| **Tier 4** | VM actors | 128 MB | 64 MB | 192 MB | Full VM isolation |

### 6.2 Memory Limit Enforcement

```rust
pub struct MemoryLimiter {
    tier: ActorTier,
    linear_memory_used: AtomicUsize,
    heap_used: AtomicUsize,
    linear_memory_limit: usize,
    heap_limit: usize,
}

impl MemoryLimiter {
    pub fn new(tier: ActorTier) -> Self {
        let (linear_limit, heap_limit) = match tier {
            ActorTier::System => (64 * 1024 * 1024, 32 * 1024 * 1024),
            ActorTier::Trusted => (32 * 1024 * 1024, 16 * 1024 * 1024),
            ActorTier::User => (16 * 1024 * 1024, 8 * 1024 * 1024),
            ActorTier::Untrusted => (8 * 1024 * 1024, 4 * 1024 * 1024),
            ActorTier::VM => (128 * 1024 * 1024, 64 * 1024 * 1024),
        };

        Self {
            tier,
            linear_memory_used: AtomicUsize::new(0),
            heap_used: AtomicUsize::new(0),
            linear_memory_limit: linear_limit,
            heap_limit,
        }
    }

    pub fn allocate_linear(&self, size: usize) -> Result<(), MemoryError> {
        let current = self.linear_memory_used.fetch_add(size, Ordering::SeqCst);
        let new_total = current + size;
        
        if new_total > self.linear_memory_limit {
            self.linear_memory_used.fetch_sub(size, Ordering::SeqCst);
            Err(MemoryError::LinearMemoryExceeded {
                requested: size,
                used: current,
                limit: self.linear_memory_limit,
            })
        } else {
            Ok(())
        }
    }

    pub fn allocate_heap(&self, size: usize) -> Result<(), MemoryError> {
        let current = self.heap_used.fetch_add(size, Ordering::SeqCst);
        let new_total = current + size;
        
        if new_total > self.heap_limit {
            self.heap_used.fetch_sub(size, Ordering::SeqCst);
            Err(MemoryError::HeapMemoryExceeded {
                requested: size,
                used: current,
                limit: self.heap_limit,
            })
        } else {
            Ok(())
        }
    }

    pub fn deallocate_linear(&self, size: usize) {
        self.linear_memory_used.fetch_sub(size, Ordering::SeqCst);
    }

    pub fn deallocate_heap(&self, size: usize) {
        self.heap_used.fetch_sub(size, Ordering::SeqCst);
    }
}
```

### 6.3 WASM Memory Limiting

```rust
pub struct WasmMemoryLimiter {
    limiter: Arc<MemoryLimiter>,
}

impl wasmtime::ResourceLimiter for WasmMemoryLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<usize, anyhow::Error> {
        let delta = desired - current;
        self.limiter.allocate_linear(delta)?;
        Ok(desired)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<usize, anyhow::Error> {
        // Tables count toward heap limit
        let delta = desired - current;
        self.limiter.allocate_heap(delta * std::mem::size_of::<usize>())?;
        Ok(desired)
    }
}
```

---

## 7. Memory Monitoring and Telemetry

### 7.1 Real-time Metrics

```rust
pub struct MemoryMetrics {
    pub total_allocated: u64,
    pub total_freed: u64,
    pub current_usage: u64,
    pub peak_usage: u64,
    pub pool_stats: HashMap<&'static str, PoolStats>,
    pub actor_stats: HashMap<ActorId, ActorMemoryStats>,
}

pub struct ActorMemoryStats {
    pub actor_id: ActorId,
    pub tier: ActorTier,
    pub linear_memory_used: usize,
    pub heap_used: usize,
    pub allocations_count: u64,
    pub deallocations_count: u64,
}
```

### 7.2 Metrics Collection

```rust
impl MemoryManager {
    pub fn collect_metrics(&self) -> MemoryMetrics {
        MemoryMetrics {
            total_allocated: self.global_stats.allocations.load(Ordering::Relaxed),
            total_freed: self.global_stats.deallocations.load(Ordering::Relaxed),
            current_usage: self.global_stats.bytes_allocated.load(Ordering::Relaxed),
            peak_usage: self.global_stats.peak_bytes.load(Ordering::Relaxed),
            pool_stats: self.pools.collect_stats(),
            actor_stats: self.collect_actor_stats(),
        }
    }

    fn collect_actor_stats(&self) -> HashMap<ActorId, ActorMemoryStats> {
        self.actor_limiters
            .iter()
            .map(|(id, limiter)| {
                (
                    *id,
                    ActorMemoryStats {
                        actor_id: *id,
                        tier: limiter.tier,
                        linear_memory_used: limiter.linear_memory_used.load(Ordering::Relaxed),
                        heap_used: limiter.heap_used.load(Ordering::Relaxed),
                        allocations_count: 0,
                        deallocations_count: 0,
                    },
                )
            })
            .collect()
    }
}
```

---

## 8. Security Considerations

### 8.1 Memory Isolation

- **WASM Linear Memory**: Each actor has isolated linear memory
- **Host Memory**: Protected from actor access
- **Shared Memory**: Only via capability-controlled mechanisms

### 8.2 Memory Safety

- **Rust Ownership**: Compile-time memory safety
- **Bounds Checking**: Runtime bounds verification
- **Guard Pages**: mimalloc provides guard pages between allocations

### 8.3 Attack Mitigation

- **Buffer Overflow**: Bounds checking + guard pages
- **Use After Free**: Rust ownership system
- **Double Free**: Rust ownership system
- **Memory Leaks**: RAII + pool allocation
- **Heap Spraying**: Randomization + guard pages

---

## 9. Testing Requirements

### 9.1 Unit Tests

- Pool allocation/deallocation correctness
- Memory limit enforcement
- Hot path allocation detection
- Stack overflow prevention

### 9.2 Integration Tests

- Actor memory isolation
- Cross-actor memory sharing
- Memory pressure scenarios
- Graceful degradation under limits

### 9.3 Stress Tests

- High allocation rates
- Memory exhaustion recovery
- Fragmentation resilience
- Long-running stability

---

## 10. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Hot path allocation | 0 | Static analysis + runtime check |
| Pool allocation latency | <100ns | Microbenchmark |
| General allocation latency | <1µs | Microbenchmark |
| Memory limit check overhead | <10ns | Inline atomic operations |
| Memory fragmentation | <5% | mimalloc statistics |
| Peak memory overhead | <20% | Telemetry |

---

## 11. References

- mimalloc documentation: https://microsoft.github.io/mimalloc/
- WASM memory model: https://webassembly.github.io/spec/core/exec/memory.html
- Rust memory safety: https://doc.rust-lang.org/nomicon/
- ADR-003: panic=abort decision
- BP-HOST-RUNTIME-001: Host runtime design

---

**Approval**: Resource Engineer
**Review Date**: 2026-03-05
