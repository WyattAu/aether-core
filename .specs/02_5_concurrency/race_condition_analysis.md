# Race Condition Analysis for Project Aether

**Document ID:** CONC-RCA-001  
**Version:** 1.0.0  
**Status:** Active  
**Created:** 2026-03-05

---

## 1. Executive Summary

This document analyzes potential race conditions in Aether's dual-runtime architecture. It identifies data races, memory ordering requirements, and establishes happens-before relationships to ensure correctness.

### Race Condition Risk Assessment

| Component | Data Race Risk | Memory Safety | Mitigation Status |
|-----------|---------------|---------------|-------------------|
| Host Runtime | Low | High | Arc + RwLock |
| WASM Engine | Very Low | Very High | Actor isolation |
| Mesh Network | Medium | High | Lock-free atomics |
| State Manager | Medium | High | FDB transactions |
| Actor Lifecycle | Medium | High | Atomic status |

---

## 2. Data Race Identification

### 2.1 Definition of Data Race

A data race occurs when:
1. Two or more threads access the same memory location concurrently
2. At least one access is a write
3. There is no synchronization establishing happens-before relationship

### 2.2 Potential Data Race Locations

```
┌─────────────────────────────────────────────────────────────────┐
│                    Data Race Risk Matrix                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  HIGH RISK (requires careful handling)                          │
│  ├── Actor status transitions                                   │
│  ├── Connection pool eviction                                   │
│  ├── State cache invalidation                                   │
│  └── Flow control credit management                             │
│                                                                  │
│  MEDIUM RISK (requires synchronization)                         │
│  ├── Module cache updates                                       │
│  ├── Routing table updates                                      │
│  ├── Capability grants/revocations                              │
│  └── Resource accounting                                        │
│                                                                  │
│  LOW RISK (protected by design)                                 │
│  ├── Configuration reads                                        │
│  ├── Health status updates                                      │
│  ├── Metrics collection                                         │
│  └── Logging                                                    │
│                                                                  │
│  NO RISK (single-threaded or immutable)                         │
│  ├── Actor instance state                                       │
│  ├── Compiled modules                                           │
│  ├── Static configuration                                       │
│  └── Channel messages                                           │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 Detailed Race Analysis

#### Actor Status Race

**Problem:**
```rust
// RACE CONDITION: Non-atomic status check + update
struct ActorEntry {
    status: ActorStatus, // NOT atomic
}

fn suspend_actor(entry: &ActorEntry) -> bool {
    if entry.status == ActorStatus::Running {  // Check
        entry.status = ActorStatus::Suspending; // Update
        true
    } else {
        false
    }
}

// Thread A: Check (Running), about to update
// Thread B: Check (Running), about to update
// Both threads proceed with suspension!
```

**Solution:**
```rust
struct ActorEntry {
    status: AtomicActorStatus,
}

fn suspend_actor(entry: &ActorEntry) -> Result<(), SuspendError> {
    entry.status.compare_exchange(
        ActorStatus::Running,
        ActorStatus::Suspending,
        Ordering::AcqRel,
        Ordering::Acquire,
    ).map_err(|_| SuspendError::InvalidState)
}
```

#### Connection Pool Eviction Race

**Problem:**
```rust
// RACE CONDITION: Check-then-evict pattern
fn evict_if_needed(pool: &mut ConnectionPool) {
    if pool.connections.len() >= pool.max_size {  // Check
        let victim = pool.lru_list.pop_front();   // Evict
        pool.connections.remove(&victim);
    }
}

// Thread A: Check (99/100), about to evict
// Thread B: Check (99/100), about to evict
// Both evict, pool drops to 98/100
```

**Solution:**
```rust
fn evict_if_needed(pool: &DashMap<NodeID, PoolEntry>) {
    // DashMap handles fine-grained locking internally
    if pool.len() >= MAX_SIZE {
        // Use entry API for atomic check-and-evict
        let mut victim = None;
        for entry in pool.iter() {
            if entry.health_status == HealthStatus::Unhealthy {
                victim = Some(entry.key().clone());
                break;
            }
        }
        
        if let Some(key) = victim {
            pool.remove(&key);
        }
    }
}
```

#### State Cache Invalidation Race

**Problem:**
```rust
// RACE CONDITION: Read-then-invalidate
struct CacheEntry {
    data: Vec<u8>,
    valid: bool,
}

fn get_or_load(cache: &mut HashMap<K, CacheEntry>, key: K) -> &Vec<u8> {
    if let Some(entry) = cache.get(&key) {
        if entry.valid {
            return &entry.data;  // Return reference
        }
    }
    
    cache.invalidate(&key);  // May invalidate while reference held
    // ...
}
```

**Solution:**
```rust
struct CacheEntry {
    data: Arc<Vec<u8>>,
    version: u64,
}

fn get_or_load(cache: &DashMap<K, CacheEntry>, key: K) -> Arc<Vec<u8>> {
    // Return Arc, not reference
    if let Some(entry) = cache.get(&key) {
        return Arc::clone(&entry.data);
    }
    
    // Load new data
    let data = Arc::new(load_from_fdb(&key));
    cache.insert(key, CacheEntry { data: Arc::clone(&data), version: 1 });
    data
}
```

---

## 3. Rust Memory Model Considerations

### 3.1 Memory Ordering Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                    Memory Ordering Hierarchy                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Strongest ←────────────────────────────────────→ Weakest       │
│                                                                  │
│  SeqCst > AcqRel > Acquire/Release > Relaxed                    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ SeqCst (Sequentially Consistent)                        │   │
│  │ - Total order of all operations                         │   │
│  │ - Use for: Flags, complex coordination                  │   │
│  │ - Cost: Highest (full memory barrier)                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Acquire/Release                                         │   │
│  │ - Paired: Store-Release, Load-Acquire                   │   │
│  │ - Use for: Producer-consumer, message passing           │   │
│  │ - Cost: Medium (half barriers)                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Relaxed                                                 │   │
│  │ - No ordering guarantees                                │   │
│  │ - Use for: Counters, statistics                         │   │
│  │ - Cost: Lowest (no barrier)                             │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Memory Ordering Guidelines

| Use Case | Recommended Ordering | Rationale |
|----------|---------------------|-----------|
| Simple counters | Relaxed | No synchronization needed |
| Statistics | Relaxed | Eventual consistency acceptable |
| Status flags | Acquire/Release | Need visibility of state change |
| Reference counting | Acquire/Release | Prevent premature release |
| Spin locks | Acquire/Release | Critical section protection |
| Global flags | SeqCst | Need total order |
| Lock-free algorithms | Varies | Depends on algorithm |

### 3.3 Atomic Types Usage

```rust
// Counters: Relaxed is sufficient
pub struct Metrics {
    messages_processed: AtomicU64,
    bytes_sent: AtomicU64,
}

impl Metrics {
    pub fn record_message(&self, bytes: u64) {
        self.messages_processed.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }
}

// Flags: Acquire/Release needed
pub struct ActorStatus {
    inner: AtomicU8,
}

impl ActorStatus {
    pub fn transition_to(&self, new: Status) -> Result<(), StatusError> {
        loop {
            let current = self.inner.load(Ordering::Acquire);
            if !valid_transition(current, new) {
                return Err(StatusError::InvalidTransition);
            }
            match self.inner.compare_exchange(
                current,
                new as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue, // Retry
            }
        }
    }
}

// Pointers: SeqCst for safety
pub struct AtomicHandle<T> {
    ptr: AtomicPtr<T>,
    epoch: AtomicU64,
}

impl<T> AtomicHandle<T> {
    pub fn swap(&self, new: *mut T) -> *mut T {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.ptr.swap(new, Ordering::SeqCst)
    }
}
```

---

## 4. Happens-Before Relationships

### 4.1 Synchronization Points

```
┌─────────────────────────────────────────────────────────────────┐
│                    Happens-Before Graph                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Thread A                      Thread B                         │
│  ────────                      ────────                         │
│                                                                  │
│  1. write(data)                                                  │
│           │                                                      │
│           ▼                                                      │
│  2. mutex.lock()                                                 │
│           │                                                      │
│           │  ──────────────►  3. mutex.lock() [blocks]          │
│           │                           │                          │
│  4. mutex.unlock()                   │ [unblocks]               │
│           │                           │                          │
│           │  ◄──────────────    4. mutex.lock() succeeds        │
│           │                           │                          │
│           │                           ▼                          │
│           │                      5. read(data)                   │
│           │                          [sees write from 1]        │
│                                                                  │
│  HB(1,2) ∧ HB(2,4) ∧ HB(4,3) ∧ HB(3,5)                         │
│  ∴ HB(1,5) [data race prevented]                                │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Channel Happens-Before

```
┌─────────────────────────────────────────────────────────────────┐
│                    Channel Happens-Before                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Sender                          Receiver                        │
│  ───────                         ────────                        │
│                                                                  │
│  1. prepare(message)                                              │
│           │                                                      │
│           ▼                                                      │
│  2. tx.send(message)                                             │
│           │                                                      │
│           │  ──────────────►  3. rx.recv() returns              │
│           │                           │                          │
│           │                           ▼                          │
│           │                      4. process(message)             │
│           │                          [sees all writes before 2] │
│                                                                  │
│  HB(1,2) ∧ HB(2,3) ∧ HB(3,4)                                    │
│  ∴ HB(1,4) [message data race-free]                             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 Atomic Happens-Before

```
┌─────────────────────────────────────────────────────────────────┐
│                    Atomic Happens-Before                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Thread A                      Thread B                         │
│  ────────                      ────────                         │
│                                                                  │
│  1. data = prepare()                                             │
│           │                                                      │
│           ▼                                                      │
│  2. flag.store(true, Release)                                    │
│           │                                                      │
│           │  ──────────────►  3. flag.load(Acquire) == true     │
│           │                           │                          │
│           │                           ▼                          │
│           │                      4. use(data)                    │
│           │                          [sees data from 1]         │
│                                                                  │
│  HB(1,2) [Release] ∧ HB(2,3) [synchronizes-with]               │
│  ∧ HB(3,4) [Acquire]                                            │
│  ∴ HB(1,4) [data visible]                                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Atomic Operations Requirements

### 5.1 Per-Component Requirements

| Component | Atomic Operations | Ordering | Rationale |
|-----------|------------------|----------|-----------|
| Actor status | compare_exchange | AcqRel | State transition |
| Fuel counter | fetch_sub | Relaxed | Monotonic decrease |
| Reference count | fetch_add/fetch_sub | Acquire | Lifetime management |
| Connection count | load/store | Relaxed | Statistics |
| Flow credit | fetch_sub | AcqRel | Credit management |
| Cache version | load | Acquire | Version check |

### 5.2 Atomic Implementation Patterns

```rust
/// Actor status with proper atomic operations
pub struct AtomicActorStatus {
    inner: AtomicU8,
}

impl AtomicActorStatus {
    pub fn new(status: ActorStatus) -> Self {
        Self {
            inner: AtomicU8::new(status as u8),
        }
    }
    
    pub fn load(&self) -> ActorStatus {
        ActorStatus::from(self.inner.load(Ordering::Acquire))
    }
    
    pub fn store(&self, status: ActorStatus) {
        self.inner.store(status as u8, Ordering::Release);
    }
    
    pub fn compare_exchange(
        &self,
        expected: ActorStatus,
        new: ActorStatus,
    ) -> Result<ActorStatus, ActorStatus> {
        self.inner
            .compare_exchange(
                expected as u8,
                new as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|v| ActorStatus::from(v))
            .map_err(|v| ActorStatus::from(v))
    }
    
    pub fn transition(&self, from: ActorStatus, to: ActorStatus) -> bool {
        self.compare_exchange(from, to).is_ok()
    }
}

/// Flow control credits with proper ordering
pub struct FlowCredits {
    available: AtomicU64,
    window_size: u64,
}

impl FlowCredits {
    pub fn try_acquire(&self, amount: u64) -> bool {
        loop {
            let current = self.available.load(Ordering::Acquire);
            if current < amount {
                return false;
            }
            
            match self.available.compare_exchange(
                current,
                current - amount,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }
    
    pub fn release(&self, amount: u64) {
        self.available.fetch_add(amount, Ordering::Release);
    }
}
```

---

## 6. Race Detection Testing

### 6.1 ThreadSanitizer Integration

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-Z", "thread-sanitizer"]

[build]
rustflags = ["-Z", "thread-sanitizer"]
```

### 6.2 Loom Integration

```rust
#[cfg(test)]
mod loom_tests {
    use loom::sync::atomic::{AtomicUsize, Ordering};
    use loom::thread;
    
    #[test]
    fn test_actor_status_race() {
        loom::model(|| {
            let status = Arc::new(AtomicActorStatus::new(ActorStatus::Running));
            
            let status1 = status.clone();
            let h1 = thread::spawn(move || {
                status1.transition(ActorStatus::Running, ActorStatus::Suspending);
            });
            
            let status2 = status.clone();
            let h2 = thread::spawn(move || {
                status2.transition(ActorStatus::Running, ActorStatus::Migrating);
            });
            
            h1.join().unwrap();
            h2.join().unwrap();
            
            // Verify only one transition succeeded
            let final_status = status.load();
            assert!(matches!(
                final_status,
                ActorStatus::Suspending | ActorStatus::Migrating
            ));
        });
    }
}
```

### 6.3 Concurrent Test Harness

```rust
pub struct RaceTestHarness {
    iterations: usize,
    threads: usize,
}

impl RaceTestHarness {
    pub fn test_actor_concurrent_operations(&self) {
        let registry = Arc::new(ActorRegistry::new());
        let barrier = Arc::new(Barrier::new(self.threads));
        
        let handles: Vec<_> = (0..self.threads)
            .map(|i| {
                let registry = registry.clone();
                let barrier = barrier.clone();
                
                thread::spawn(move || {
                    barrier.wait();
                    
                    for j in 0..self.iterations {
                        let actor_id = ActorId::new(i * self.iterations + j);
                        
                        match j % 4 {
                            0 => { let _ = registry.register(actor_id, create_instance()); }
                            1 => { let _ = registry.get(&actor_id); }
                            2 => { let _ = registry.transition(&actor_id, ActorStatus::Suspending); }
                            _ => { let _ = registry.unregister(&actor_id); }
                        }
                    }
                })
            })
            .collect();
        
        for h in handles {
            h.join().unwrap();
        }
    }
}
```

---

## 7. Race-Free Invariants

### 7.1 Actor Isolation Invariant

```lean
theorem actor_isolation_no_race :
  ∀ (a1 a2 : Actor) (op1 op2 : Operation),
    a1.id ≠ a2.id →
    executes a1 op1 →
    executes a2 op2 →
    no_data_race op1 op2 :=
by
  intro a1 a2 op1 op2 h_different h_exec1 h_exec2
  -- Each actor has isolated memory
  -- Operations on different actors access disjoint memory
  sorry
```

### 7.2 Channel Communication Invariant

```lean
theorem channel_communication_race_free :
  ∀ (ch : Channel) (msg : Message) (sender receiver : Thread),
    sends sender ch msg →
    receives receiver ch msg →
    happens_before (write sender msg.data) (read receiver msg.data) :=
by
  intro ch msg sender receiver h_send h_recv
  -- Channel send/recv establishes happens-before
  -- All writes before send visible after recv
  sorry
```

---

## 8. References

- Rust Memory Model: https://doc.rust-lang.org/nomicon/
- C++20 Memory Model: https://en.cppreference.com/w/cpp/atomic/memory_order
- Loom Documentation: https://docs.rs/loom/
- ThreadSanitizer: https://github.com/google/sanitizers
