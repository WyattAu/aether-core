# Thread Safety Analysis for Project Aether

**Document ID:** CONC-TSA-001  
**Version:** 1.0.0  
**Status:** Active  
**Created:** 2026-03-05  
**Reference Architecture:** BP-HOST-RUNTIME-001, BP-WASM-ENGINE-001, BP-MESH-NETWORK-001, BP-STATE-MANAGER-001

---

## 1. Executive Summary

This document analyzes thread safety requirements for Aether's dual-runtime architecture (Monoio + Tokio). The analysis identifies shared state, thread access patterns, and synchronization strategies for each major component.

### Key Findings

| Component | Shared State | Thread Model | Synchronization Strategy |
|-----------|-------------|--------------|-------------------------|
| Host Runtime | Global config, subsystem registry | Multi-threaded (Tokio) | RwLock + channels |
| WASM Engine | Instance pool, module cache | Single-threaded per actor | Actor isolation |
| Mesh Network | Connection pool, routing table | Thread-per-core (Monoio) | Lock-free + atomic |
| State Manager | Cache, FDB client | Hybrid (both runtimes) | RwLock + lock-free |
| Actor Lifecycle | Actor registry | Event-driven | Message passing |

---

## 2. Host Runtime Thread Safety

### 2.1 Shared State Identification

```
┌─────────────────────────────────────────────────────────────────┐
│                    Host Runtime Shared State                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │  HostConfig     │     │ SubsystemRegistry│                   │
│  │  (Read-Heavy)   │     │ (Read-Write)     │                   │
│  └────────┬────────┘     └────────┬─────────┘                   │
│           │                       │                              │
│           ▼                       ▼                              │
│  ┌─────────────────────────────────────────────────┐            │
│  │              CapabilityManager                   │            │
│  │         (Read-Heavy, Write-Rare)                │            │
│  └─────────────────────────────────────────────────┘            │
│                                                                  │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │ HealthMonitor   │     │ResourceAccountant│                   │
│  │ (Write-Heavy)   │     │ (Atomic Ops)     │                   │
│  └─────────────────┘     └─────────────────┘                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Thread Access Patterns

| State | Reader Threads | Writer Threads | Access Frequency |
|-------|---------------|----------------|------------------|
| `HostConfig` | All subsystems | Config loader | Read: 10^6/s, Write: 10^-3/s |
| `SubsystemRegistry` | Control plane | Main daemon | Read: 10^3/s, Write: 10^-2/s |
| `CapabilityManager` | All subsystems | Capability admin | Read: 10^6/s, Write: 10^-4/s |
| `HealthMonitor` | Health checker | All subsystems | Write: 10^2/s |
| `ResourceAccountant` | Metrics exporter | All subsystems | Write: 10^5/s |

### 2.3 Synchronization Requirements

#### HostConfig
```rust
pub struct HostConfig {
    inner: Arc<RwLock<HostConfigInner>>,
}

impl HostConfig {
    pub fn get(&self) -> RwLockReadGuard<'_, HostConfigInner> {
        self.inner.read().unwrap()
    }
    
    pub fn update(&self, new_config: HostConfigInner) {
        let mut guard = self.inner.write().unwrap();
        *guard = new_config;
    }
}
```

**Analysis:**
- Read-heavy access pattern (RwLock preferred)
- Write operations are rare configuration updates
- Readers never block each other
- Writer starvation unlikely due to low write frequency

#### CapabilityManager
```rust
pub struct CapabilityManager {
    grants: DashMap<CapabilityTarget, CapabilitySet>,
    roles: RwLock<HashMap<Role, CapabilitySet>>,
}

impl CapabilityManager {
    pub fn check_capability(&self, target: &CapabilityTarget, cap: Capability) -> bool {
        self.grants
            .get(target)
            .map(|entry| entry.value().contains(cap))
            .unwrap_or(false)
    }
    
    pub fn grant_capability(&self, target: CapabilityTarget, cap: Capability) {
        self.grants
            .entry(target)
            .or_default()
            .insert(cap);
    }
}
```

**Analysis:**
- DashMap provides lock-free reads with fine-grained locking
- O(1) check complexity maintained
- No global lock contention
- Sharding prevents hotspots

#### ResourceAccountant
```rust
pub struct ResourceAccountant {
    cpu_time_ns: AtomicU64,
    memory_bytes: AtomicU64,
    io_operations: AtomicU64,
    network_bytes: AtomicU64,
}

impl ResourceAccountant {
    pub fn record_cpu_time(&self, delta_ns: u64) {
        self.cpu_time_ns.fetch_add(delta_ns, Ordering::Relaxed);
    }
    
    pub fn record_memory(&self, bytes: u64) {
        self.memory_bytes.store(bytes, Ordering::Relaxed);
    }
    
    pub fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            cpu_time_ns: self.cpu_time_ns.load(Ordering::Relaxed),
            memory_bytes: self.memory_bytes.load(Ordering::Relaxed),
            io_operations: self.io_operations.load(Ordering::Relaxed),
            network_bytes: self.network_bytes.load(Ordering::Relaxed),
        }
    }
}
```

**Analysis:**
- Pure atomic operations, no locks required
- Relaxed ordering sufficient (eventual consistency acceptable)
- Lock-free updates from any thread
- Consistent snapshots require all atomics read together

---

## 3. WASM Instance Management Thread Safety

### 3.1 Shared State Identification

```
┌─────────────────────────────────────────────────────────────────┐
│                    WASM Engine Shared State                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Module Cache (Global)                       │   │
│  │  HashMap<ModuleHash, Arc<CompiledModule>>                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Instance Registry (Per-Actor)               │   │
│  │  Actor → Instance (Single-Owner)                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Fuel Counter (Per-Instance)                 │   │
│  │  AtomicU64 (Lock-Free)                                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Thread Access Patterns

| State | Access Pattern | Owner | Concurrent Access |
|-------|---------------|-------|-------------------|
| Module Cache | Read-heavy, Write-rare | WASM Engine | Multiple readers, single writer |
| Instance State | Single-threaded per actor | Actor scheduler | No concurrent access (isolated) |
| Fuel Counter | Read-write | Actor thread | Atomic operations only |
| Memory Handle | Single-threaded | Instance owner | No concurrent access |

### 3.3 Actor Isolation Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    Actor Isolation Architecture                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Core i:0          Core i:1          Core i:2          ...      │
│  ┌─────────┐      ┌─────────┐      ┌─────────┐                 │
│  │ Actor A │      │ Actor B │      │ Actor C │                 │
│  │ Instance│      │ Instance│      │ Instance│                 │
│  │ Memory  │      │ Memory  │      │ Memory  │                 │
│  │ Fuel    │      │ Fuel    │      │ Fuel    │                 │
│  └─────────┘      └─────────┘      └─────────┘                 │
│       │                │                │                       │
│       │                │                │                       │
│       ▼                ▼                ▼                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Module Cache (Shared, Read-Only)            │   │
│  │              Arc<CompiledModule>                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Key Property:** Each actor instance is owned by exactly one thread. No shared mutable state between actors.

### 3.4 Module Cache Synchronization

```rust
pub struct ModuleCache {
    modules: RwLock<HashMap<ModuleHash, Arc<CompiledModule>>>,
    stats: CacheStats,
}

struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl ModuleCache {
    pub async fn get_or_compile(&self, wasm_bytes: &[u8]) -> Result<Arc<CompiledModule>, Error> {
        let hash = compute_hash(wasm_bytes);
        
        // Fast path: check cache with read lock
        {
            let guard = self.modules.read().unwrap();
            if let Some(module) = guard.get(&hash) {
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Arc::clone(module));
            }
        }
        
        // Slow path: compile with write lock
        let module = compile_module(wasm_bytes).await?;
        
        {
            let mut guard = self.modules.write().unwrap();
            // Double-check pattern
            if let Some(existing) = guard.get(&hash) {
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Arc::clone(existing));
            }
            let module = Arc::new(module);
            guard.insert(hash, Arc::clone(&module));
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            Ok(module)
        }
    }
}
```

**Analysis:**
- Double-checked locking pattern prevents duplicate compilation
- Read lock for fast path (cache hit)
- Write lock only for cache misses
- Arc enables shared ownership of compiled modules
- No lock held during expensive compilation

---

## 4. Connection Pool (Mesh) Thread Safety

### 4.1 Shared State Identification

```
┌─────────────────────────────────────────────────────────────────┐
│                    Mesh Network Shared State                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Connection Pool                             │   │
│  │  DashMap<NodeID, PoolEntry> (Lock-Free Reads)           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Routing Table                               │   │
│  │  ArcSwap<RoutingTable> (Lock-Free Updates)              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Flow Control State                          │   │
│  │  Per-Stream Atomic Counters                              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Thread-Per-Core Model (Monoio)

```
┌─────────────────────────────────────────────────────────────────┐
│                    Monoio Thread-Per-Core Model                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Core 0              Core 1              Core 2                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │ Local Queue │    │ Local Queue │    │ Local Queue │         │
│  │  Actors     │    │  Actors     │    │  Actors     │         │
│  │  Conn 1,2   │    │  Conn 3,4   │    │  Conn 5,6   │         │
│  └─────────────┘    └─────────────┘    └─────────────┘         │
│        │                  │                  │                  │
│        │    No Work       │    Stealing     │                  │
│        │    Stealing ◄────┼────►Disabled    │                  │
│        ▼                  ▼                  ▼                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Shared Connection Pool                      │   │
│  │              (DashMap for concurrent access)             │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Key Property:** Monoio uses thread-per-core without work stealing. Each core owns its connections and actors.

### 4.3 Connection Pool Synchronization

```rust
pub struct ConnectionPool {
    connections: DashMap<NodeID, PoolEntry>,
    lru_list: Mutex<LinkedList<NodeID>>,
    max_size: usize,
    config: PoolConfig,
}

pub struct PoolEntry {
    connection: QuinnConnection,
    last_used: AtomicInstant,
    health_status: AtomicHealthStatus,
    stats: ConnectionStats,
}

impl ConnectionPool {
    pub fn acquire(&self, node_id: &NodeID) -> Result<ConnectionGuard, PoolError> {
        // Fast path: get existing connection
        if let Some(entry) = self.connections.get(node_id) {
            entry.last_used.store(Instant::now(), Ordering::Release);
            return Ok(ConnectionGuard::new(entry));
        }
        
        // Slow path: create new connection
        let entry = self.create_connection(node_id)?;
        
        // Check capacity and evict if needed
        if self.connections.len() >= self.max_size {
            self.evict_lru();
        }
        
        // Insert (may race with another inserter - that's OK)
        let entry = self.connections.entry(*node_id).or_insert(entry);
        Ok(ConnectionGuard::new(entry))
    }
    
    fn evict_lru(&self) {
        let mut guard = self.lru_list.lock().unwrap();
        if let Some(victim) = guard.pop_front() {
            self.connections.remove(&victim);
        }
    }
}
```

**Analysis:**
- DashMap provides lock-free reads for hot path
- LRU list requires mutex but is rarely accessed
- Connection creation is idempotent (races are safe)
- Last-used timestamp uses atomic for lock-free updates

### 4.4 Routing Table Lock-Free Updates

```rust
pub struct RoutingTable {
    inner: ArcSwap<RoutingTableInner>,
}

struct RoutingTableInner {
    local_cache: HashMap<ActorID, NodeID>,
    dht_entries: HashMap<ActorID, Vec<NodeID>>,
    version: u64,
}

impl RoutingTable {
    pub fn resolve(&self, actor_id: &ActorID) -> Option<NodeID> {
        let guard = self.inner.load();
        guard.local_cache.get(actor_id).copied()
            .or_else(|| guard.dht_entries.get(actor_id).and_then(|v| v.first().copied()))
    }
    
    pub fn update(&self, actor_id: ActorID, node_id: NodeID) {
        let current = self.inner.load();
        let mut new_inner = RoutingTableInner::clone(&current);
        new_inner.local_cache.insert(actor_id, node_id);
        new_inner.version += 1;
        self.inner.store(Arc::new(new_inner));
    }
}
```

**Analysis:**
- ArcSwap enables lock-free reads with atomic pointer swap
- Updates copy the entire table (acceptable for small tables)
- Readers always see consistent snapshot
- No reader-writer contention

---

## 5. State Cache (FDB/Redb) Thread Safety

### 5.1 Shared State Identification

```
┌─────────────────────────────────────────────────────────────────┐
│                    State Manager Shared State                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              L1 Memory Cache                             │   │
│  │  LruCache<ActorID, Arc<CachedState>> (Mutex-Protected)  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              L2 Redb Cache                               │   │
│  │  Redb Database (ACID, Thread-Safe)                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              FDB Client Pool                             │   │
│  │  Pool<FdbConnection> (Arc<Mutex<VecDeque>>>)            │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Watch Registry                              │   │
│  │  DashMap<ActorID, Vec<Watcher>>                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Multi-Tier Cache Synchronization

```rust
pub struct StateCache {
    l1: Mutex<LruCache<ActorID, Arc<CachedState>>>,
    l2: RedbDatabase,
    fdb_pool: FdbConnectionPool,
    watchers: DashMap<ActorID, Vec<Watcher>>,
}

pub struct CachedState {
    data: Vec<u8>,
    versionstamp: Versionstamp,
    checksum: u64,
}

impl StateCache {
    pub async fn read(&self, actor_id: &ActorID) -> Result<Arc<CachedState>, StateError> {
        // L1 lookup (fast path)
        {
            let mut l1 = self.l1.lock().unwrap();
            if let Some(state) = l1.get(actor_id) {
                return Ok(Arc::clone(state));
            }
        }
        
        // L2 lookup
        let state = self.read_from_l2(actor_id).await?;
        
        // Populate L1
        {
            let mut l1 = self.l1.lock().unwrap();
            l1.put(*actor_id, Arc::clone(&state));
        }
        
        Ok(state)
    }
    
    pub async fn write(&self, actor_id: ActorID, state: CachedState) -> Result<(), StateError> {
        // Write through to FDB
        self.fdb_pool.write(&actor_id, &state).await?;
        
        // Update L2
        self.write_to_l2(&actor_id, &state).await?;
        
        // Update L1
        {
            let mut l1 = self.l1.lock().unwrap();
            l1.put(actor_id, Arc::new(state));
        }
        
        // Invalidate remote caches via watch
        self.invalidate_watchers(&actor_id);
        
        Ok(())
    }
}
```

### 5.3 Cross-Runtime State Access

```
┌─────────────────────────────────────────────────────────────────┐
│                    Cross-Runtime State Access                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Monoio (Data Plane)              Tokio (Control Plane)         │
│  ┌─────────────────┐             ┌─────────────────┐           │
│  │ Actor I/O       │             │ FDB Client      │           │
│  │ State Hydration │             │ Migration       │           │
│  └────────┬────────┘             └────────┬────────┘           │
│           │                               │                     │
│           │                               │                     │
│           ▼                               ▼                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Shared State Cache                          │   │
│  │              (Thread-safe by design)                     │   │
│  │                                                          │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐                 │   │
│  │  │   L1    │  │   L2    │  │   FDB   │                 │   │
│  │  │ Mutex   │  │  Redb   │  │  Pool   │                 │   │
│  │  └─────────┘  └─────────┘  └─────────┘                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Boundary Rules:**
1. Both runtimes can read from cache
2. Only Tokio runtime performs FDB writes
3. Monoio requests writes via channel to Tokio
4. Cache invalidation broadcasts to both runtimes

---

## 6. Actor Lifecycle Management Thread Safety

### 6.1 Actor Registry Synchronization

```rust
pub struct ActorRegistry {
    actors: DashMap<ActorID, ActorEntry>,
    pending_creates: Mutex<HashSet<ActorID>>,
    stats: RegistryStats,
}

pub struct ActorEntry {
    instance: Arc<Mutex<ActorInstance>>,
    status: AtomicActorStatus,
    owner_core: AtomicU32,
    created_at: Instant,
    metrics: ActorMetrics,
}

impl ActorRegistry {
    pub fn register(&self, actor_id: ActorID, instance: ActorInstance) -> Result<(), RegistryError> {
        // Check for duplicate creation
        {
            let mut pending = self.pending_creates.lock().unwrap();
            if pending.contains(&actor_id) {
                return Err(RegistryError::AlreadyCreating);
            }
            pending.insert(actor_id);
        }
        
        let entry = ActorEntry {
            instance: Arc::new(Mutex::new(instance)),
            status: AtomicActorStatus::new(ActorStatus::Creating),
            owner_core: AtomicU32::new(current_core()),
            created_at: Instant::now(),
            metrics: ActorMetrics::default(),
        };
        
        self.actors.insert(actor_id, entry);
        
        // Remove from pending
        {
            let mut pending = self.pending_creates.lock().unwrap();
            pending.remove(&actor_id);
        }
        
        Ok(())
    }
    
    pub fn get(&self, actor_id: &ActorID) -> Option<ActorGuard> {
        self.actors.get(actor_id).map(|entry| {
            ActorGuard::new(entry)
        })
    }
}
```

### 6.2 Actor Lifecycle State Machine

```
┌─────────────────────────────────────────────────────────────────┐
│                    Actor Lifecycle States                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  [Creating] ──────► [Running] ──────► [Suspending]             │
│       │                 │                  │                     │
│       │                 │                  ▼                     │
│       │                 │            [Suspended]                │
│       │                 │                  │                     │
│       │                 ▼                  ▼                     │
│       │            [Migrating] ◄──────► [Checkpointing]         │
│       │                 │                                        │
│       ▼                 ▼                                        │
│  [Failed] ◄───────► [Destroying] ──────► [Destroyed]           │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

Thread Safety Invariants:
- Status transitions are atomic
- Only one thread can transition at a time
- Mutex<ActorInstance> protects instance state
- AtomicActorStatus allows lock-free status checks
```

---

## 7. Lock-Free Alternatives Summary

### 7.1 Recommended Lock-Free Patterns

| Use Case | Pattern | Implementation |
|----------|---------|----------------|
| Read-heavy config | Arc<RwLock<T>> | `parking_lot::RwLock` |
| Read-mostly maps | DashMap | `dashmap::DashMap` |
| Atomic snapshots | ArcSwap | `arc_swap::ArcSwap` |
| Counters/gauges | AtomicU64 | `std::sync::atomic` |
| Per-thread state | ThreadLocal | `thread_local::ThreadLocal` |
| Message passing | Channels | `tokio::sync::mpsc` |
| Event broadcast | Watch channel | `tokio::sync::watch` |

### 7.2 When to Use Locks vs Lock-Free

| Scenario | Recommendation | Rationale |
|----------|---------------|-----------|
| High read, low write | RwLock | Readers don't block each other |
| Balanced read/write | DashMap | Fine-grained locking per bucket |
| Immutable snapshots | ArcSwap | Atomic pointer swap |
| Simple counters | Atomics | No contention possible |
| Complex state update | Mutex | Simpler than lock-free algorithms |
| Cross-thread coordination | Channels | Message passing, no shared state |

---

## 8. Thread Safety Checklist

### 8.1 Per-Component Checklist

#### Host Runtime
- [ ] HostConfig uses RwLock for read-heavy access
- [ ] CapabilityManager uses DashMap for concurrent grants
- [ ] ResourceAccountant uses atomic counters
- [ ] HealthMonitor uses proper synchronization

#### WASM Engine
- [ ] Module cache uses RwLock with double-checked locking
- [ ] Instance state is single-threaded per actor
- [ ] Fuel counter uses atomic operations
- [ ] Memory handles are not shared between actors

#### Mesh Network
- [ ] Connection pool uses DashMap
- [ ] Routing table uses ArcSwap
- [ ] Flow control uses per-stream atomics
- [ ] No cross-core data sharing

#### State Manager
- [ ] L1 cache uses Mutex
- [ ] L2 cache (Redb) is thread-safe by design
- [ ] FDB client pool uses proper connection management
- [ ] Watch registry uses DashMap

#### Actor Lifecycle
- [ ] Registry uses DashMap
- [ ] Status transitions are atomic
- [ ] Instance access is mutex-protected
- [ ] Creation deduplication works correctly

---

## 9. References

- BP-HOST-RUNTIME-001: Host Runtime Architecture
- BP-WASM-ENGINE-001: WASM Execution Engine
- BP-MESH-NETWORK-001: QUIC Mesh Network
- BP-STATE-MANAGER-001: Distributed State Manager
- ADR-001: Dual Runtime Architecture
- Rust Memory Model: https://doc.rust-lang.org/nomicon/
