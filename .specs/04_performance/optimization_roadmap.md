# Optimization Roadmap
**Project Aether - Phase 4: Performance Engineering**

## Document Control
- **Version**: 1.0
- **Status**: Approved
- **Created**: 2026-03-05
- **Last Updated**: 2026-03-05
- **Author**: Performance Engineering Team
- **Review Status**: Complete

## 1. Executive Summary

This document outlines the phased optimization roadmap for Project Aether, prioritizing efforts based on impact and dependencies. The roadmap covers four main phases: cold start optimization, network stack optimization, state hydration optimization, and memory layout optimization.

## 2. Optimization Prioritization Framework

### 2.1 Prioritization Criteria

| Criterion | Weight | Description |
|-----------|--------|-------------|
| User Impact | 35% | Direct effect on user-visible latency/throughput |
| Implementation Complexity | 25% | Effort and risk of implementation |
| Dependencies | 20% | Whether other optimizations depend on this |
| Measurability | 10% | Ability to quantify improvement |
| Risk | 10% | Risk of regression or instability |

### 2.2 Current Baseline

| Metric | Current | Target | Gap | Priority |
|--------|---------|--------|-----|----------|
| WASM Cold Start | 120µs | 50µs | 70µs | **Critical** |
| Mesh Latency P50 | 2.5ms | 1ms | 1.5ms | **High** |
| State Hydration | 100ms | 50ms | 50ms | **High** |
| Actor Density | 50,000 | 100,000 | 50,000 | **Medium** |
| Memory per Actor | 5MB | 2MB | 3MB | **Medium** |

## 3. Phase 1: Cold Start Optimization (Weeks 1-4)

### 3.1 Goals

- Reduce WASM cold start from 120µs to <50µs (P99)
- Reduce VM cold start from 180ms to <125ms (P99)
- Improve actor activation latency

### 3.2 Sub-Phase 1.1: WASM Module Caching (Week 1)

#### 3.2.1 Module Compilation Cache

**Current Issue:**
- Each cold start recompiles WAT to WASM
- Cranelift compilation adds 50-80µs
- No caching of compiled modules

**Optimization:**
```rust
pub struct ModuleCache {
    cache: LruCache<ModuleHash, Arc<CompiledModule>>,
    stats: CacheStats,
}

impl ModuleCache {
    pub fn get_or_compile(&mut self, wasm: &[u8]) -> Result<Arc<CompiledModule>> {
        let hash = blake3::hash(wasm);
        
        if let Some(cached) = self.cache.get(&hash) {
            self.stats.hits += 1;
            return Ok(cached.clone());
        }
        
        self.stats.misses += 1;
        let compiled = self.compile_module(wasm)?;
        self.cache.put(hash, Arc::new(compiled.clone()));
        Ok(Arc::new(compiled))
    }
}
```

**Expected Improvement:**
- Cold start: 120µs → 70µs (42% reduction)
- Warm start: 120µs → 2µs (98% reduction)

**Implementation Tasks:**
- [ ] Implement module hash computation (blake3)
- [ ] Create thread-safe LRU cache
- [ ] Add cache statistics and monitoring
- [ ] Implement cache persistence to disk
- [ ] Add cache warming on startup

#### 3.2.2 Instance Template Caching

**Current Issue:**
- Instance creation involves memory allocation
- Table initialization repeated for each instance
- Import resolution overhead

**Optimization:**
```rust
pub struct InstanceTemplate {
    module: Arc<CompiledModule>,
    memory_template: MemoryTemplate,
    table_templates: Vec<TableTemplate>,
    import_resolutions: Vec<ImportResolution>,
}

impl InstanceTemplate {
    pub fn instantiate(&self) -> Result<Instance> {
        // Fast path: clone pre-configured structures
        let memory = self.memory_template.clone();
        let tables = self.table_templates.iter()
            .map(|t| t.clone())
            .collect();
        
        Instance::from_template(self.module.clone(), memory, tables)
    }
}
```

**Expected Improvement:**
- Instance creation: 50µs → 20µs (60% reduction)

**Implementation Tasks:**
- [ ] Profile instance creation overhead
- [ ] Identify clonable components
- [ ] Implement template creation
- [ ] Benchmark template vs direct instantiation

### 3.3 Sub-Phase 1.2: Lazy Initialization (Week 2)

#### 3.3.1 Deferred Memory Allocation

**Current Issue:**
- All memory pages allocated upfront
- Many actors use <10% of allocated memory
- Allocation overhead significant in cold start path

**Optimization:**
```rust
pub struct LazyMemory {
    max_pages: u32,
    committed_pages: AtomicU32,
    page_table: Box<[AtomicPtr<Page>]>,
}

impl LazyMemory {
    pub fn new(max_pages: u32) -> Self {
        Self {
            max_pages,
            committed_pages: AtomicU32::new(0),
            page_table: Box::new_zeroed_slice(max_pages as usize).assume_init(),
        }
    }
    
    pub fn access(&self, page_idx: u32) -> *mut Page {
        let ptr = self.page_table[page_idx as usize].load(Ordering::Acquire);
        
        if ptr.is_null() {
            self.allocate_page(page_idx)
        } else {
            ptr
        }
    }
    
    #[cold]
    fn allocate_page(&self, page_idx: u32) -> *mut Page {
        // Slow path: allocate on first access
        let page = Box::into_raw(Box::new(Page::zeroed()));
        self.page_table[page_idx as usize].store(page, Ordering::Release);
        page
    }
}
```

**Expected Improvement:**
- Memory allocation: 30µs → 5µs (83% reduction)

**Implementation Tasks:**
- [ ] Implement lazy page allocation
- [ ] Add guard pages for bounds checking
- [ ] Implement page deallocation
- [ ] Profile memory access patterns
- [ ] Add lazy memory metrics

#### 3.3.2 Deferred Import Resolution

**Current Issue:**
- All imports resolved at instantiation
- Import resolution involves hash lookups
- Many imports not used immediately

**Optimization:**
```rust
pub struct LazyImports {
    import_resolvers: Vec<Box<dyn Fn() -> Result<Value>>>,
    resolved: Vec<OnceCell<Value>>,
}

impl LazyImports {
    pub fn get(&self, idx: usize) -> Result<Value> {
        self.resolved[idx].get_or_try_init(|| {
            (self.import_resolvers[idx])()
        }).cloned()
    }
}
```

**Expected Improvement:**
- Import resolution: 10µs → 0µs (deferred to first use)

**Implementation Tasks:**
- [ ] Identify deferrable imports
- [ ] Implement lazy resolution
- [ ] Add resolution metrics
- [ ] Handle resolution failures

### 3.4 Sub-Phase 1.3: VM Snapshot Optimization (Week 3)

#### 3.4.1 VM Memory Snapshot Optimization

**Current Issue:**
- Full memory snapshot takes 80-100ms
- No dirty page tracking
- Serial snapshot creation

**Optimization:**
```rust
pub struct SnapshotOptimizer {
    dirty_bitmap: DirtyBitmap,
    previous_snapshot: Option<Snapshot>,
}

impl SnapshotOptimizer {
    pub fn create_snapshot(&mut self, vm: &VmHandle) -> Result<Snapshot> {
        // Track dirty pages since last snapshot
        let dirty_pages = self.dirty_bitmap.get_dirty_pages();
        
        // Only snapshot dirty pages
        let delta = self.snapshot_dirty_pages(vm, &dirty_pages)?;
        
        // Combine with previous snapshot
        let snapshot = if let Some(prev) = &self.previous_snapshot {
            prev.apply_delta(delta)?
        } else {
            delta.into_snapshot()
        };
        
        self.previous_snapshot = Some(snapshot.clone());
        self.dirty_bitmap.clear();
        
        Ok(snapshot)
    }
}
```

**Expected Improvement:**
- Snapshot creation: 100ms → 30ms (70% reduction for incremental)

**Implementation Tasks:**
- [ ] Implement dirty page tracking
- [ ] Create incremental snapshot format
- [ ] Implement delta application
- [ ] Benchmark incremental vs full snapshots

#### 3.4.2 VM Pool Pre-warming

**Current Issue:**
- VM boot on-demand adds latency
- No VM reuse

**Optimization:**
```rust
pub struct VmPool {
    pool: Vec<VmHandle>,
    min_size: usize,
    max_size: usize,
    config: VmConfig,
}

impl VmPool {
    pub async fn acquire(&mut self) -> Result<VmHandle> {
        if let Some(vm) = self.pool.pop() {
            // Fast path: return pre-warmed VM
            return Ok(vm);
        }
        
        // Slow path: create new VM
        let vm = self.create_vm().await?;
        Ok(vm)
    }
    
    pub async fn release(&mut self, vm: VmHandle) {
        // Reset VM state
        vm.reset().await.unwrap();
        
        if self.pool.len() < self.max_size {
            self.pool.push(vm);
        } else {
            vm.destroy().await.unwrap();
        }
    }
    
    async fn maintain_pool(&mut self) {
        while self.pool.len() < self.min_size {
            if let Ok(vm) = self.create_vm().await {
                self.pool.push(vm);
            }
        }
    }
}
```

**Expected Improvement:**
- VM acquisition: 125ms → 5ms (96% reduction for pooled VMs)

**Implementation Tasks:**
- [ ] Implement VM pool data structure
- [ ] Create VM reset mechanism
- [ ] Add pool sizing logic
- [ ] Implement background pool maintenance
- [ ] Add pool metrics

### 3.5 Sub-Phase 1.4: Code Generation Optimization (Week 4)

#### 3.5.1 Cranelift Optimization Flags

**Current Issue:**
- Default Cranelift settings not tuned for speed
- Unnecessary optimizations enabled
- Code generation takes too long

**Optimization:**
```rust
pub fn create_optimized_engine() -> Engine {
    let mut flag_builder = settings::builder();
    
    // Optimize for speed over code size
    flag_builder.set("opt_level", "speed").unwrap();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "false").unwrap();
    flag_builder.set("unwind_info", "false").unwrap();
    
    // Enable SIMD
    flag_builder.set("enable_simd", "true").unwrap();
    flag_builder.set("enable_avx", "true").unwrap();
    
    let isa = isa::lookup(target_lexicon::HOST).unwrap()
        .finish(settings::Flags::new(flag_builder));
    
    Engine::new(Config::new()
        .target(isa)
        .cranelift_opt_level(settings::OptLevel::Speed))
}
```

**Expected Improvement:**
- Compilation: 50µs → 35µs (30% reduction)

**Implementation Tasks:**
- [ ] Profile Cranelift phases
- [ ] Benchmark different flag combinations
- [ ] Identify unnecessary passes
- [ ] Implement optimized engine builder

#### 3.5.2 Pre-compiled Runtime Library

**Current Issue:**
- Runtime library functions interpreted
- No AOT compilation of common operations

**Optimization:**
```rust
pub struct RuntimeLibrary {
    precompiled: HashMap<&'static str, *const u8>,
}

impl RuntimeLibrary {
    pub fn new() -> Self {
        let mut lib = Self {
            precompiled: HashMap::new(),
        };
        
        // Pre-compile common operations
        lib.compile("memory_copy", memory_copy_impl as *const u8);
        lib.compile("memory_fill", memory_fill_impl as *const u8);
        lib.compile("table_copy", table_copy_impl as *const u8);
        
        lib
    }
}
```

**Expected Improvement:**
- Runtime calls: 2µs → 0.5µs (75% reduction)

**Implementation Tasks:**
- [ ] Identify hot runtime functions
- [ ] Implement native versions
- [ ] Create runtime library interface
- [ ] Benchmark native vs WASM implementations

### 3.6 Phase 1 Success Metrics

| Metric | Before | After | Target | Status |
|--------|--------|-------|--------|--------|
| WASM Cold Start (P99) | 120µs | - | 50µs | Pending |
| WASM Warm Start (P99) | 120µs | - | 10µs | Pending |
| VM Cold Boot (P99) | 180ms | - | 125ms | Pending |
| VM Warm Boot (P99) | 180ms | - | 50ms | Pending |

## 4. Phase 2: Network Stack Optimization (Weeks 5-8)

### 4.1 Goals

- Reduce mesh latency P50 from 2.5ms to <1ms
- Improve message throughput to >10M msg/s per node
- Reduce network overhead

### 4.2 Sub-Phase 2.1: io_uring Optimization (Week 5)

#### 4.2.1 Batch Submission

**Current Issue:**
- Single operation submissions
- System call per operation
- High syscall overhead

**Optimization:**
```rust
pub struct BatchedIoUring {
    ring: IoUring,
    batch: Vec<Sqe>,
    batch_size: usize,
}

impl BatchedIoUring {
    pub fn submit_batched(&mut self) -> Result<usize> {
        let sq = self.ring.submission();
        
        // Add all batched entries
        for entry in &self.batch {
            sq.push(entry)?;
        }
        
        // Single syscall for all operations
        let submitted = self.ring.submit()?;
        self.batch.clear();
        
        Ok(submitted)
    }
    
    pub fn add_to_batch(&mut self, entry: Sqe) {
        self.batch.push(entry);
        
        if self.batch.len() >= self.batch_size {
            self.submit_batched().unwrap();
        }
    }
}
```

**Expected Improvement:**
- Syscall overhead: 1µs per op → 0.05µs per op (95% reduction with batch of 32)

**Implementation Tasks:**
- [ ] Implement batched submission queue
- [ ] Add adaptive batch sizing
- [ ] Implement batch timeout
- [ ] Benchmark batch sizes

#### 4.2.2 SQPoll Optimization

**Current Issue:**
- Kernel thread wakes on each submission
- Context switch overhead

**Optimization:**
```rust
pub fn create_sqpoll_ring(entries: u32) -> Result<IoUring> {
    IoUring::builder()
        .setup_sqpoll(1)  // Kernel thread for submission polling
        .setup_sqpoll_cpu(0)  // Pin to CPU 0
        .setup_coop_taskrun()  // Cooperative task running
        .build(entries)
}
```

**Expected Improvement:**
- Submission latency: 1µs → 0.1µs (90% reduction)

**Implementation Tasks:**
- [ ] Enable SQPoll in io_ring setup
- [ ] Pin kernel thread to dedicated CPU
- [ ] Measure latency improvement
- [ ] Monitor CPU usage of kernel thread

#### 4.2.3 Zero-Copy Networking

**Current Issue:**
- Data copied between kernel and userspace
- Copy overhead significant for large messages

**Optimization:**
```rust
pub struct ZeroCopySocket {
    fd: RawFd,
    ring: IoUring,
}

impl ZeroCopySocket {
    pub async fn send_zero_copy(&self, buf: &[u8]) -> Result<usize> {
        let entry = opcode::SendZc::new(Fd(self.fd), buf.as_ptr(), buf.len() as _)
            .build()
            .user_data(0);
        
        self.ring.submission().push(&entry)?;
        self.ring.submit_and_wait(1)?;
        
        let cqe = self.ring.completion().next().unwrap();
        Ok(cqe.result() as usize)
    }
}
```

**Expected Improvement:**
- Large message send: 5µs/KB → 0.5µs/KB (90% reduction)

**Implementation Tasks:**
- [ ] Implement MSG_ZEROCOPY for sends
- [ ] Handle completion notifications
- [ ] Benchmark zero-copy vs copy
- [ ] Identify optimal message size threshold

### 4.3 Sub-Phase 2.2: Message Serialization Optimization (Week 6)

#### 4.3.1 rkyv Zero-Copy Optimization

**Current Issue:**
- Unnecessary validation on hot path
- Redundant checks

**Optimization:**
```rust
pub struct FastDeserializer;

impl FastDeserializer {
    /// Unsafe fast path: trust the sender
    pub unsafe fn deserialize_unchecked<T: Archive>(&self, bytes: &[u8]) -> &T::Archived {
        // Skip validation for trusted sources
        rkyv::archived_root::<T>(bytes)
    }
    
    /// Safe path: validate for untrusted sources
    pub fn deserialize_checked<T: Archive + CheckBytes<DefaultValidator>>(
        &self,
        bytes: &[u8]
    ) -> Result<&T::Archived> {
        rkyv::check_archived_root::<T>(bytes)
    }
}
```

**Expected Improvement:**
- Deserialize (trusted): 2µs → 0.1µs (95% reduction)

**Implementation Tasks:**
- [ ] Implement fast/slow path selection
- [ ] Add trust level tracking
- [ ] Benchmark validation overhead
- [ ] Document safety requirements

#### 4.3.2 Message Pooling

**Current Issue:**
- Message allocation on each send
- Memory fragmentation

**Optimization:**
```rust
pub struct MessagePool {
    pools: Vec<Pool<MessageBuffer>>,
    size_classes: Vec<usize>,
}

impl MessagePool {
    pub fn acquire(&self, size: usize) -> PooledBuffer {
        let class_idx = self.size_classes.iter()
            .position(|&s| s >= size)
            .unwrap_or(self.size_classes.len() - 1);
        
        PooledBuffer {
            buffer: self.pools[class_idx].acquire(),
            len: size,
        }
    }
}

pub struct PooledBuffer {
    buffer: Arc<MessageBuffer>,
    len: usize,
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        // Return to pool instead of deallocating
    }
}
```

**Expected Improvement:**
- Allocation overhead: 1µs → 0.05µs (95% reduction)

**Implementation Tasks:**
- [ ] Implement size classes (64B, 256B, 1KB, 4KB, 16KB, 64KB)
- [ ] Create pool data structure
- [ ] Add pool metrics
- [ ] Implement pool resizing

### 4.4 Sub-Phase 2.3: Routing Optimization (Week 7)

#### 4.4.1 Fast Path Routing

**Current Issue:**
- Full routing table lookup on each message
- Hash table overhead

**Optimization:**
```rust
pub struct FastRouter {
    // Fast path: direct lookup for common routes
    direct_routes: Vec<Option<NodeId>>,
    
    // Slow path: hash table for all routes
    route_table: HashMap<ActorId, NodeId>,
    
    // Cache of recent lookups
    lru_cache: LruCache<ActorId, NodeId>,
}

impl FastRouter {
    pub fn route(&mut self, actor_id: ActorId) -> Option<NodeId> {
        // Fast path: direct array lookup
        let idx = actor_id.as_u64() as usize;
        if idx < self.direct_routes.len() {
            if let Some(node) = &self.direct_routes[idx] {
                return Some(*node);
            }
        }
        
        // Medium path: LRU cache
        if let Some(node) = self.lru_cache.get(&actor_id) {
            return Some(*node);
        }
        
        // Slow path: hash table
        let node = self.route_table.get(&actor_id).cloned();
        if let Some(n) = node {
            self.lru_cache.put(actor_id, n);
        }
        node
    }
}
```

**Expected Improvement:**
- Route lookup: 200ns → 20ns (90% reduction for fast path)

**Implementation Tasks:**
- [ ] Implement direct route array
- [ ] Add LRU cache
- [ ] Profile cache hit rates
- [ ] Tune cache size

#### 4.4.2 Connection Pooling

**Current Issue:**
- New connection per message
- Connection establishment overhead

**Optimization:**
```rust
pub struct ConnectionPool {
    connections: HashMap<NodeId, Vec<Connection>>,
    max_per_node: usize,
}

impl ConnectionPool {
    pub async fn get_connection(&mut self, node: NodeId) -> Result<Connection> {
        if let Some(pool) = self.connections.get_mut(&node) {
            if let Some(conn) = pool.pop() {
                // Reuse existing connection
                return Ok(conn);
            }
        }
        
        // Create new connection
        let conn = self.create_connection(node).await?;
        Ok(conn)
    }
    
    pub fn return_connection(&mut self, node: NodeId, conn: Connection) {
        let pool = self.connections.entry(node).or_insert_with(Vec::new);
        
        if pool.len() < self.max_per_node {
            pool.push(conn);
        }
        // Otherwise, drop connection
    }
}
```

**Expected Improvement:**
- Connection overhead: 5ms → 0ms (100% reduction for pooled connections)

**Implementation Tasks:**
- [ ] Implement connection pool
- [ ] Add health checking
- [ ] Implement connection timeouts
- [ ] Add pool metrics

### 4.5 Sub-Phase 2.4: Protocol Optimization (Week 8)

#### 4.5.1 Binary Protocol

**Current Issue:**
- Text-based protocol overhead
- Parsing overhead

**Optimization:**
```rust
#[repr(C, packed)]
pub struct BinaryMessageHeader {
    magic: u16,      // Protocol magic number
    version: u8,     // Protocol version
    flags: u8,       // Message flags
    msg_type: u16,   // Message type
    payload_len: u32, // Payload length
    src_actor: u64,  // Source actor ID
    dst_actor: u64,  // Destination actor ID
    trace_id: u64,   // Tracing ID
    checksum: u32,   // CRC32 checksum
}

impl BinaryMessageHeader {
    pub const SIZE: usize = 42;
    pub const MAGIC: u16 = 0xA378;
}
```

**Expected Improvement:**
- Header parsing: 500ns → 50ns (90% reduction)

**Implementation Tasks:**
- [ ] Define binary protocol
- [ ] Implement encoder/decoder
- [ ] Add checksum validation
- [ ] Benchmark vs text protocol

#### 4.5.2 Header Compression

**Current Issue:**
- Repeated header fields
- Bandwidth overhead

**Optimization:**
```rust
pub struct HeaderCompressor {
    // HPACK-like compression for common fields
    static_table: Vec<HeaderField>,
    dynamic_table: LruCache<HeaderField, u8>,
}

impl HeaderCompressor {
    pub fn compress(&mut self, header: &MessageHeader) -> Vec<u8> {
        let mut compressed = Vec::new();
        
        // Use indexed representation for common fields
        for field in header.fields() {
            if let Some(idx) = self.static_table.iter().position(|f| f == field) {
                compressed.push(idx as u8);
            } else if let Some(idx) = self.dynamic_table.get(field) {
                compressed.push(0x80 | idx);
            } else {
                // Literal representation
                compressed.push(0);
                compressed.extend_from_slice(&field.to_bytes());
            }
        }
        
        compressed
    }
}
```

**Expected Improvement:**
- Header size: 42 bytes → 15 bytes (64% reduction)

**Implementation Tasks:**
- [ ] Implement header compressor
- [ ] Create static table of common fields
- [ ] Add dynamic table management
- [ ] Benchmark compression ratio

### 4.6 Phase 2 Success Metrics

| Metric | Before | After | Target | Status |
|--------|--------|-------|--------|--------|
| Mesh Latency P50 | 2.5ms | - | 1ms | Pending |
| Message Throughput | 2M/s | - | 10M/s | Pending |
| Network Overhead | 20% | - | 5% | Pending |

## 5. Phase 3: State Hydration Optimization (Weeks 9-12)

### 5.1 Goals

- Reduce state hydration from 100ms to <50ms (P99)
- Improve state access throughput
- Reduce state memory footprint

### 5.2 Sub-Phase 3.1: Lazy State Loading (Week 9)

#### 5.2.1 Partial State Hydration

**Current Issue:**
- Full state loaded even if not needed
- Large state sizes slow activation

**Optimization:**
```rust
pub struct LazyStateLoader {
    manifest: StateManifest,
    loaded_chunks: HashSet<ChunkId>,
    backend: StateBackend,
}

impl LazyStateLoader {
    pub fn hydrate_lazy(&mut self) -> Result<()> {
        // Only load manifest and index
        self.manifest = self.backend.load_manifest()?;
        Ok(())
    }
    
    pub fn access(&mut self, key: &[u8]) -> Result<&[u8]> {
        let chunk_id = self.manifest.chunk_for_key(key);
        
        if !self.loaded_chunks.contains(&chunk_id) {
            self.load_chunk(chunk_id)?;
        }
        
        self.manifest.get(key)
    }
    
    fn load_chunk(&mut self, chunk_id: ChunkId) -> Result<()> {
        let chunk = self.backend.load_chunk(chunk_id)?;
        self.manifest.apply_chunk(chunk_id, chunk);
        self.loaded_chunks.insert(chunk_id);
        Ok(())
    }
}
```

**Expected Improvement:**
- Hydration time: 100ms → 10ms (90% reduction for lazy load)

**Implementation Tasks:**
- [ ] Implement state chunking
- [ ] Create manifest format
- [ ] Implement lazy loading
- [ ] Add access tracking

#### 5.2.2 Predictive Prefetching

**Current Issue:**
- Waiting for state on first access
- Predictable access patterns not exploited

**Optimization:**
```rust
pub struct StatePrefetcher {
    access_history: VecDeque<Key>,
    predictor: AccessPredictor,
    prefetch_queue: VecDeque<ChunkId>,
}

impl StatePrefetcher {
    pub fn record_access(&mut self, key: Key) {
        self.access_history.push_back(key);
        
        // Update predictor
        self.predictor.train(&self.access_history);
        
        // Trigger prefetch
        if let Some(next_chunks) = self.predictor.predict_next() {
            for chunk_id in next_chunks {
                self.prefetch_queue.push_back(chunk_id);
            }
        }
    }
    
    pub async fn run_prefetcher(&mut self, loader: &mut LazyStateLoader) {
        while let Some(chunk_id) = self.prefetch_queue.pop_front() {
            if !loader.is_loaded(chunk_id) {
                loader.load_chunk_async(chunk_id).await.ok();
            }
        }
    }
}
```

**Expected Improvement:**
- Cache miss rate: 40% → 10% (75% reduction)

**Implementation Tasks:**
- [ ] Implement access predictor (Markov chain or LSTM)
- [ ] Create prefetch queue
- [ ] Add background prefetch task
- [ ] Measure hit rate improvement

### 5.3 Sub-Phase 3.2: State Compression (Week 10)

#### 5.3.1 LZ4 Compression

**Current Issue:**
- State stored uncompressed
- Disk I/O bottleneck

**Optimization:**
```rust
pub struct CompressedStateBackend {
    inner: StateBackend,
    compression_level: u32,
}

impl CompressedStateBackend {
    pub fn store(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let compressed = lz4_flex::compress_prepend_size(value);
        
        // Only use compression if beneficial
        if compressed.len() < value.len() * 9 / 10 {
            self.inner.store(key, &compressed)
        } else {
            self.inner.store(key, value)
        }
    }
    
    pub fn load(&self, key: &[u8]) -> Result<Vec<u8>> {
        let data = self.inner.load(key)?;
        
        // Check if compressed
        if Self::is_compressed(&data) {
            Ok(lz4_flex::decompress_size_prepended(&data)?)
        } else {
            Ok(data)
        }
    }
}
```

**Expected Improvement:**
- State size: 100MB → 30MB (70% reduction)
- Load time: 100ms → 35ms (65% reduction)

**Implementation Tasks:**
- [ ] Implement LZ4 compression
- [ ] Add compression ratio tracking
- [ ] Implement adaptive compression
- [ ] Benchmark different algorithms

#### 5.3.2 Columnar Storage

**Current Issue:**
- Row-oriented storage inefficient for partial access
- Compression ratio suboptimal

**Optimization:**
```rust
pub struct ColumnarState {
    columns: HashMap<ColumnId, Column>,
    row_index: Vec<RowId>,
}

pub struct Column {
    data: Vec<u8>,
    offsets: Vec<u32>,
    compression: CompressionType,
}

impl ColumnarState {
    pub fn get_column(&self, col_id: ColumnId) -> Option<&Column> {
        self.columns.get(&col_id)
    }
    
    pub fn get_row(&self, row_id: RowId) -> Vec<Option<&[u8]>> {
        let row_idx = self.row_index.binary_search(&row_id).ok()?;
        
        self.columns.values()
            .map(|col| col.get(row_idx))
            .collect()
    }
}
```

**Expected Improvement:**
- Column access: 10ms → 1ms (90% reduction)
- Compression ratio: 30% → 50% (67% improvement)

**Implementation Tasks:**
- [ ] Design columnar format
- [ ] Implement column encoder/decoder
- [ ] Add column-level compression
- [ ] Benchmark column vs row access

### 5.4 Sub-Phase 3.3: State Caching (Week 11)

#### 5.4.1 Multi-Tier Cache

**Current Issue:**
- All state in same tier
- No differentiation by access frequency

**Optimization:**
```rust
pub struct MultiTierCache {
    l1: LruCache<Key, Arc<[u8]>>,    // Hot state (in-memory)
    l2: LruCache<Key, Arc<[u8]>>,    // Warm state (in-memory)
    l3: LruCache<Key, FileOffset>,   // Cold state (memory-mapped)
    backend: StateBackend,
}

impl MultiTierCache {
    pub fn get(&mut self, key: &Key) -> Result<Arc<[u8]>> {
        // L1 cache (hot)
        if let Some(value) = self.l1.get(key) {
            return Ok(value.clone());
        }
        
        // L2 cache (warm)
        if let Some(value) = self.l2.get(key) {
            // Promote to L1
            self.l1.put(key.clone(), value.clone());
            return Ok(value);
        }
        
        // L3 cache (cold)
        if let Some(offset) = self.l3.get(key) {
            let value = self.read_from_mmap(offset)?;
            // Promote to L2
            self.l2.put(key.clone(), value.clone());
            return Ok(value);
        }
        
        // Backend (storage)
        let value = self.backend.load(key)?;
        self.l1.put(key.clone(), value.clone());
        Ok(value)
    }
}
```

**Expected Improvement:**
- Cache hit rate: 60% → 95% (58% improvement)

**Implementation Tasks:**
- [ ] Implement three-tier cache
- [ ] Add cache promotion/demotion logic
- [ ] Implement size-based eviction
- [ ] Add cache metrics

#### 5.4.2 Cache Warming

**Current Issue:**
- Cold cache on actor activation
- High miss rate initially

**Optimization:**
```rust
pub struct CacheWarmer {
    access_patterns: HashMap<ActorType, Vec<Key>>,
}

impl CacheWarmer {
    pub fn warm_cache(&self, actor_type: ActorType, cache: &mut MultiTierCache) {
        if let Some(keys) = self.access_patterns.get(&actor_type) {
            // Prefetch based on historical patterns
            for key in keys.iter().take(100) {
                if let Ok(value) = cache.backend.load(key) {
                    cache.l1.put(key.clone(), value);
                }
            }
        }
    }
    
    pub fn learn_pattern(&mut self, actor_type: ActorType, accesses: Vec<Key>) {
        // Learn access pattern for future warming
        self.access_patterns.insert(actor_type, accesses);
    }
}
```

**Expected Improvement:**
- Initial miss rate: 100% → 30% (70% reduction)

**Implementation Tasks:**
- [ ] Implement access pattern tracking
- [ ] Create pattern learning algorithm
- [ ] Add cache warming on activation
- [ ] Measure miss rate reduction

### 5.5 Sub-Phase 3.4: State Diff Optimization (Week 12)

#### 5.5.1 Incremental Updates

**Current Issue:**
- Full state written on each update
- Write amplification

**Optimization:**
```rust
pub struct IncrementalStateUpdater {
    base: StateVersion,
    deltas: Vec<StateDelta>,
    delta_threshold: usize,
}

impl IncrementalStateUpdater {
    pub fn update(&mut self, key: Key, value: Value) {
        // Record delta
        self.deltas.push(StateDelta::Update {
            key,
            old_value: self.get(&key).cloned(),
            new_value: value,
        });
        
        // Compact if too many deltas
        if self.deltas.len() > self.delta_threshold {
            self.compact();
        }
    }
    
    pub fn flush(&mut self) -> Result<()> {
        // Only write deltas, not full state
        let compressed_delta = self.compress_deltas()?;
        self.backend.write_delta(self.base.version(), &compressed_delta)
    }
}
```

**Expected Improvement:**
- Write size: 100MB → 1MB (99% reduction for small updates)
- Flush time: 100ms → 5ms (95% reduction)

**Implementation Tasks:**
- [ ] Implement delta format
- [ ] Create delta compressor
- [ ] Add delta application logic
- [ ] Implement compaction

### 5.6 Phase 3 Success Metrics

| Metric | Before | After | Target | Status |
|--------|--------|-------|--------|--------|
| State Hydration P99 | 100ms | - | 50ms | Pending |
| State Access P50 | 1ms | - | 0.1ms | Pending |
| State Size | 100MB | - | 30MB | Pending |

## 6. Phase 4: Memory Layout Optimization (Weeks 13-16)

### 6.1 Goals

- Reduce memory per actor from 5MB to <2MB
- Improve cache hit rate
- Reduce TLB misses

### 6.2 Sub-Phase 4.1: Cache Line Alignment (Week 13)

#### 6.2.1 Hot/Cold Splitting

**Current Issue:**
- Hot and cold data mixed in structures
- Cache pollution

**Optimization:**
```rust
// Before: Mixed hot/cold data
struct ActorOld {
    id: u64,                    // Hot
    state: Vec<u8>,            // Hot
    config: ActorConfig,        // Cold
    metrics: ActorMetrics,      // Cold
    history: Vec<HistoryEntry>, // Cold
}

// After: Hot/cold split
#[repr(align(64))]
struct ActorHot {
    id: u64,
    state_ptr: *mut u8,
    state_len: usize,
    // Padding to 64 bytes
}

struct ActorCold {
    config: ActorConfig,
    metrics: ActorMetrics,
    history: Vec<HistoryEntry>,
}

struct Actor {
    hot: ActorHot,  // Always in cache
    cold: Box<ActorCold>,  // Allocated separately
}
```

**Expected Improvement:**
- Cache miss rate: 15% → 5% (67% reduction)

**Implementation Tasks:**
- [ ] Identify hot/cold fields
- [ ] Refactor structures
- [ ] Add alignment attributes
- [ ] Benchmark cache performance

#### 6.2.2 Structure Packing

**Current Issue:**
- Padding waste in structures
- Poor memory utilization

**Optimization:**
```rust
// Before: 24 bytes with padding
#[derive(Default)]
struct MessageHeaderOld {
    msg_type: u16,    // 2 bytes + 6 padding
    flags: u8,        // 1 byte
    // 5 bytes padding
    timestamp: u64,   // 8 bytes
    length: u32,      // 4 bytes
}

// After: 16 bytes packed
#[repr(C, packed)]
struct MessageHeaderNew {
    timestamp: u64,   // 8 bytes
    length: u32,      // 4 bytes
    msg_type: u16,    // 2 bytes
    flags: u8,        // 1 byte
    version: u8,      // 1 byte
}
```

**Expected Improvement:**
- Structure size: 24 bytes → 16 bytes (33% reduction)
- Memory bandwidth: proportional improvement

**Implementation Tasks:**
- [ ] Profile structure sizes
- [ ] Reorder fields
- [ ] Add repr(C, packed)
- [ ] Verify correctness

### 6.3 Sub-Phase 4.2: Memory Pooling (Week 14)

#### 6.3.1 Arena Allocation

**Current Issue:**
- Individual allocations per actor
- Allocation overhead and fragmentation

**Optimization:**
```rust
pub struct ActorArena {
    chunks: Vec<Chunk>,
    current_chunk: usize,
    offset: usize,
}

struct Chunk {
    memory: Box<[u8; CHUNK_SIZE]>,
    in_use: BitSet,
}

impl ActorArena {
    pub fn allocate(&mut self, layout: Layout) -> Result<*mut u8> {
        let size = layout.size();
        let align = layout.align();
        
        // Align offset
        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        
        if aligned_offset + size <= CHUNK_SIZE {
            let ptr = self.chunks[self.current_chunk].memory[aligned_offset..].as_mut_ptr();
            self.offset = aligned_offset + size;
            Ok(ptr)
        } else {
            self.allocate_new_chunk(layout)
        }
    }
    
    pub fn reset(&mut self) {
        // Fast bulk deallocation
        for chunk in &mut self.chunks {
            chunk.in_use.clear();
        }
        self.current_chunk = 0;
        self.offset = 0;
    }
}
```

**Expected Improvement:**
- Allocation time: 500ns → 50ns (90% reduction)
- Memory fragmentation: Eliminated

**Implementation Tasks:**
- [ ] Implement arena allocator
- [ ] Add chunk management
- [ ] Implement reset mechanism
- [ ] Benchmark allocation performance

#### 6.3.2 Slab Allocation

**Current Issue:**
- Variable-size allocations inefficient
- Memory waste

**Optimization:**
```rust
pub struct SlabAllocator {
    slabs: [Slab; NUM_SIZE_CLASSES],
    size_classes: [usize; NUM_SIZE_CLASSES],
}

struct Slab {
    slots: Box<[u8]>,
    free_list: Vec<usize>,
    slot_size: usize,
}

impl SlabAllocator {
    pub fn allocate(&mut self, size: usize) -> Result<*mut u8> {
        // Find appropriate size class
        let class_idx = self.size_classes.iter()
            .position(|&s| s >= size)
            .ok_or(Error::TooLarge)?;
        
        self.slabs[class_idx].allocate()
    }
}

impl Slab {
    fn allocate(&mut self) -> Result<*mut u8> {
        if let Some(slot_idx) = self.free_list.pop() {
            let offset = slot_idx * self.slot_size;
            Ok(&mut self.slots[offset] as *mut u8)
        } else {
            Err(Error::OutOfMemory)
        }
    }
}
```

**Expected Improvement:**
- Allocation time: 200ns → 20ns (90% reduction)
- Memory utilization: 70% → 95% (36% improvement)

**Implementation Tasks:**
- [ ] Define size classes (8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096)
- [ ] Implement slab allocator
- [ ] Add free list management
- [ ] Benchmark vs system allocator

### 6.4 Sub-Phase 4.3: TLB Optimization (Week 15)

#### 6.4.1 Huge Pages

**Current Issue:**
- 4KB pages cause TLB pressure
- TLB misses expensive

**Optimization:**
```rust
pub struct HugePageAllocator {
    huge_pages: Vec<*mut u8>,
    page_size: usize,  // 2MB or 1GB
}

impl HugePageAllocator {
    pub fn new() -> Result<Self> {
        // Mount hugetlbfs if not already mounted
        Command::new("mount")
            .args(&["-t", "hugetlbfs", "nodev", "/mnt/huge"])
            .status()?;
        
        Ok(Self {
            huge_pages: Vec::new(),
            page_size: 2 * 1024 * 1024,  // 2MB
        })
    }
    
    pub fn allocate_huge_page(&mut self) -> Result<*mut u8> {
        let fd = unsafe {
            libc::open(
                b"/mnt/huge/aether_hugepage\0".as_ptr() as *const i8,
                libc::O_CREAT | libc::O_RDWR,
                0o600,
            )
        };
        
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                self.page_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        
        self.huge_pages.push(ptr as *mut u8);
        Ok(ptr as *mut u8)
    }
}
```

**Expected Improvement:**
- TLB miss rate: 5% → 0.5% (90% reduction)
- TLB miss penalty: 150 cycles → negligible

**Implementation Tasks:**
- [ ] Enable huge pages in kernel
- [ ] Implement huge page allocator
- [ ] Allocate actor memory from huge pages
- [ ] Measure TLB performance

#### 6.4.2 Memory Locality

**Current Issue:**
- Actor memory scattered
- Poor locality

**Optimization:**
```rust
pub struct LocalAllocator {
    numa_node: usize,
    local_arenas: Vec<ActorArena>,
}

impl LocalAllocator {
    pub fn allocate_local(&mut self, cpu: usize) -> Result<*mut u8> {
        // Allocate from arena on same NUMA node
        let arena_idx = cpu / CORES_PER_ARENA;
        self.local_arenas[arena_idx].allocate(Layout::new::<Actor>())
    }
}
```

**Expected Improvement:**
- Remote memory access: 10% → 1% (90% reduction)
- Memory latency: 150ns → 80ns (47% reduction)

**Implementation Tasks:**
- [ ] Implement NUMA-aware allocator
- [ ] Add CPU-to-node mapping
- [ ] Allocate actor memory locally
- [ ] Measure remote access rate

### 6.5 Sub-Phase 4.4: Prefetching (Week 16)

#### 6.5.1 Software Prefetching

**Current Issue:**
- Cache misses on predictable accesses
- Latency spikes

**Optimization:**
```rust
pub struct Prefetcher;

impl Prefetcher {
    #[inline(always)]
    pub fn prefetch_read<T>(ptr: *const T) {
        unsafe {
            // Prefetch to L1 cache
            std::arch::x86_64::_mm_prefetch(
                ptr as *const i8,
                std::arch::x86_64::_MM_HINT_T0,
            );
        }
    }
    
    #[inline(always)]
    pub fn prefetch_write<T>(ptr: *mut T) {
        unsafe {
            // Prefetch for write
            std::arch::x86_64::_mm_prefetch(
                ptr as *const i8,
                std::arch::x86_64::_MM_HINT_T0,
            );
        }
    }
    
    pub fn prefetch_message_chain(&self, messages: &[Message]) {
        for i in 0..messages.len().saturating_sub(8) {
            // Prefetch 8 messages ahead
            Self::prefetch_read(&messages[i + 8]);
        }
    }
}
```

**Expected Improvement:**
- Cache miss latency: Hidden by prefetch
- Throughput: 10-20% improvement

**Implementation Tasks:**
- [ ] Identify prefetch opportunities
- [ ] Add prefetch intrinsics
- [ ] Tune prefetch distance
- [ ] Measure cache miss reduction

#### 6.5.2 Hardware Prefetch Tuning

```bash
# Enable hardware prefetchers
echo 1 > /sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference

# Check prefetcher status
rdmsr -p 0 0x1a4  # MSR_MISC_FEATURE_CONTROL
```

**Implementation Tasks:**
- [ ] Profile with different prefetcher settings
- [ ] Document optimal configuration
- [ ] Add runtime configuration

### 6.6 Phase 4 Success Metrics

| Metric | Before | After | Target | Status |
|--------|--------|-------|--------|--------|
| Memory per Actor | 5MB | - | 2MB | Pending |
| Cache Miss Rate | 15% | - | 5% | Pending |
| TLB Miss Rate | 5% | - | 0.5% | Pending |

## 7. Optimization Tracking Dashboard

### 7.1 Key Metrics to Track

```yaml
metrics:
  - name: wasm_cold_start_p99
    target: 50us
    current: 120us
    phase: 1
    
  - name: mesh_latency_p50
    target: 1ms
    current: 2.5ms
    phase: 2
    
  - name: state_hydration_p99
    target: 50ms
    current: 100ms
    phase: 3
    
  - name: memory_per_actor
    target: 2MB
    current: 5MB
    phase: 4
```

### 7.2 Regression Detection

```rust
pub struct RegressionDetector {
    baseline: HashMap<String, f64>,
    threshold: f64,  // 5% regression threshold
}

impl RegressionDetector {
    pub fn check(&self, metric: &str, value: f64) -> Option<Regression> {
        if let Some(&baseline) = self.baseline.get(metric) {
            let change = (value - baseline) / baseline;
            
            if change > self.threshold {
                return Some(Regression {
                    metric: metric.to_string(),
                    baseline,
                    current: value,
                    percent_change: change * 100.0,
                });
            }
        }
        None
    }
}
```

## 8. Risk Mitigation

### 8.1 Optimization Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Regression in other metrics | Medium | High | Comprehensive benchmarking |
| Complexity increase | Medium | Medium | Code review and documentation |
| Platform-specific behavior | Low | Medium | Test on multiple platforms |
| Memory safety issues | Low | High | Extensive testing and fuzzing |
| Thread safety issues | Medium | High | ThreadSanitizer and lock analysis |

### 8.2 Rollback Strategy

Each optimization is:
1. Implemented behind feature flag
2. Tested in isolation
3. Benchmarked comprehensively
4. Rolled out gradually
5. Monitored continuously

## 9. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-05 | Performance Team | Initial version |

## 10. Appendices

### Appendix A: Optimization Checklist

- [ ] Profile before optimization
- [ ] Identify hotspot
- [ ] Hypothesize solution
- [ ] Implement with feature flag
- [ ] Benchmark in isolation
- [ ] Test for regressions
- [ ] Document changes
- [ ] Enable by default
- [ ] Monitor in production

### Appendix B: Benchmark Commands

```bash
# Run all Phase 1 benchmarks
cargo bench --features optimization-phase1

# Compare before/after
cargo bench --baseline main --features optimization-phase1

# Generate flame graphs
./scripts/gen_flamegraph.sh before
# Apply optimization
./scripts/gen_flamegraph.sh after
# Compare
./scripts/compare_flamegraphs.sh before.svg after.svg
```
