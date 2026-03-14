# Synchronization Design for Project Aether

**Document ID:** CONC-SD-001  
**Version:** 1.0.0  
**Status:** Active  
**Created:** 2026-03-05

---

## 1. Executive Summary

This document defines synchronization primitive selection criteria and usage patterns for Aether's dual-runtime architecture. It establishes guidelines for choosing between Mutex, RwLock, channels, atomic types, and lock-free data structures.

### Synchronization Strategy Summary

| Use Case | Primitive | Rationale |
|----------|-----------|-----------|
| Read-heavy shared state | RwLock | Concurrent reads |
| Write-heavy shared state | DashMap | Fine-grained locking |
| Immutable snapshots | ArcSwap | Lock-free reads |
| Cross-thread coordination | Channels | Message passing |
| Simple counters | Atomic | Lock-free |
| Actor instance | Mutex | Simple, low contention |
| Event broadcast | Watch channel | Single writer, multiple readers |

---

## 2. Synchronization Primitive Catalog

### 2.1 Mutex

**When to use:**
- Write-heavy access patterns
- Complex critical sections
- When readers need to see consistent state

**Implementation:**
```rust
use parking_lot::Mutex;

pub struct ActorInstance {
    inner: Mutex<ActorInstanceInner>,
}

impl ActorInstance {
    pub fn invoke(&self, message: Message) -> Result<Response, Error> {
        let mut guard = self.inner.lock();
        guard.process_message(message)
    }
}
```

**Pros:**
- Simple to use correctly
- Lower overhead than RwLock for write-heavy
- Guaranteed exclusive access

**Cons:**
- No concurrent reads
- Potential for contention
- Can cause convoy effects

### 2.2 RwLock

**When to use:**
- Read-heavy access patterns (>10:1 read:write ratio)
- Large data structures
- When reads need consistent snapshot

**Implementation:**
```rust
use parking_lot::RwLock;

pub struct HostConfig {
    inner: RwLock<HostConfigInner>,
}

impl HostConfig {
    pub fn get(&self) -> RwLockReadGuard<'_, HostConfigInner> {
        self.inner.read()
    }
    
    pub fn update(&self, new_config: HostConfigInner) {
        *self.inner.write() = new_config;
    }
}
```

**Pros:**
- Concurrent reads
- Good for read-heavy workloads
- Readers don't block readers

**Cons:**
- Higher overhead than Mutex
- Writer starvation possible
- Upgrade (read → write) can deadlock

### 2.3 Spin Locks

**When to use:**
- Very short critical sections (<100 CPU cycles)
- When context switch cost exceeds wait time
- In interrupt context

**Implementation:**
```rust
use spin::Mutex as SpinMutex;

pub struct FastPath {
    counter: SpinMutex<u64>,
}

impl FastPath {
    pub fn increment(&self) {
        let mut guard = self.counter.lock();
        *guard += 1;
    }
}
```

**Pros:**
- No context switch overhead
- Very fast for short waits

**Cons:**
- Wastes CPU cycles
- Can cause priority inversion
- Not suitable for long critical sections

**Recommendation:** Avoid in Aether. Use Mutex instead.

### 2.4 DashMap

**When to use:**
- Concurrent hash map access
- Mixed read/write patterns
- When fine-grained locking needed

**Implementation:**
```rust
use dashmap::DashMap;

pub struct ActorRegistry {
    actors: DashMap<ActorId, ActorEntry>,
}

impl ActorRegistry {
    pub fn register(&self, id: ActorId, entry: ActorEntry) {
        self.actors.insert(id, entry);
    }
    
    pub fn get(&self, id: &ActorId) -> Option<ActorEntry> {
        self.actors.get(id).map(|v| v.clone())
    }
    
    pub fn remove(&self, id: &ActorId) -> Option<ActorEntry> {
        self.actors.remove(id).map(|(_, v)| v)
    }
}
```

**Pros:**
- Lock-free reads (mostly)
- Fine-grained locking per bucket
- Good concurrency

**Cons:**
- Higher memory overhead
- More complex than HashMap
- Iteration can see inconsistent state

### 2.5 ArcSwap

**When to use:**
- Immutable snapshots
- RCU-style updates
- Configuration hot-swapping

**Implementation:**
```rust
use arc_swap::ArcSwap;

pub struct RoutingTable {
    inner: ArcSwap<RoutingTableInner>,
}

impl RoutingTable {
    pub fn get(&self) -> arc_swap::Guard<Arc<RoutingTableInner>> {
        self.inner.load()
    }
    
    pub fn update(&self, new_table: Arc<RoutingTableInner>) {
        self.inner.store(new_table);
    }
}
```

**Pros:**
- Lock-free reads
- Wait-free updates
- Consistent snapshots

**Cons:**
- Requires Arc (allocation)
- Reads may see stale data
- Memory reclamation delay

---

## 3. Channel Patterns

### 3.1 mpsc (Multi-Producer, Single-Consumer)

**When to use:**
- Multiple threads sending to one handler
- Actor message queues
- Event aggregation

**Implementation:**
```rust
use tokio::sync::mpsc;

pub struct ActorMailbox {
    tx: mpsc::Sender<ActorMessage>,
    rx: mpsc::Receiver<ActorMessage>,
}

impl ActorMailbox {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self { tx, rx }
    }
    
    pub async fn send(&self, msg: ActorMessage) -> Result<(), SendError> {
        self.tx.send(msg).await
    }
    
    pub async fn recv(&mut self) -> Option<ActorMessage> {
        self.rx.recv().await
    }
}
```

**Configuration:**
```rust
// Bounded channel (recommended)
let (tx, rx) = mpsc::channel::<Message>(1024);

// Unbounded channel (use carefully)
let (tx, rx) = mpsc::unbounded_channel::<Message>();
```

### 3.2 broadcast (Multi-Producer, Multi-Consumer)

**When to use:**
- Event broadcasting
- Status notifications
- Fan-out patterns

**Implementation:**
```rust
use tokio::sync::broadcast;

pub struct EventBus {
    tx: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }
    
    pub fn broadcast(&self, event: SystemEvent) -> usize {
        self.tx.send(event).unwrap_or(0)
    }
}
```

### 3.3 watch (Single-Producer, Multi-Consumer)

**When to use:**
- Configuration updates
- Status monitoring
- Single source of truth

**Implementation:**
```rust
use tokio::sync::watch;

pub struct ConfigWatcher {
    tx: watch::Sender<Config>,
    rx: watch::Receiver<Config>,
}

impl ConfigWatcher {
    pub fn new(initial: Config) -> Self {
        let (tx, rx) = watch::channel(initial);
        Self { tx, rx }
    }
    
    pub fn update(&self, config: Config) {
        self.tx.send(config).ok();
    }
    
    pub async fn changed(&mut self) -> Result<(), watch::error::RecvError> {
        self.rx.changed().await
    }
    
    pub fn borrow(&self) -> watch::Ref<'_, Config> {
        self.rx.borrow()
    }
}
```

### 3.4 oneshot (Single-Producer, Single-Consumer)

**When to use:**
- Request/response patterns
- One-time notifications
- Future completion

**Implementation:**
```rust
use tokio::sync::oneshot;

pub async fn invoke_actor(actor: &Actor, msg: Message) -> Result<Response, Error> {
    let (tx, rx) = oneshot::channel();
    
    actor.send(ActorCommand::Invoke { 
        message: msg,
        reply: tx,
    })?;
    
    rx.await.map_err(|_| Error::ActorCrashed)
}
```

---

## 4. Atomic Types

### 4.1 Atomic Integers

**When to use:**
- Counters
- Statistics
- Simple state flags

**Implementation:**
```rust
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};

pub struct Metrics {
    messages_processed: AtomicU64,
    bytes_sent: AtomicU64,
    errors: AtomicU64,
    healthy: AtomicBool,
}

impl Metrics {
    pub fn record_message(&self, bytes: u64) {
        self.messages_processed.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }
    
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }
    
    pub fn set_healthy(&self, healthy: bool) {
        self.healthy.store(healthy, Ordering::Release);
    }
}
```

### 4.2 AtomicPtr

**When to use:**
- Lock-free data structures
- Atomic pointer swaps
- Hazard pointers

**Implementation:**
```rust
use std::sync::atomic::{AtomicPtr, Ordering};

pub struct AtomicStack<T> {
    head: AtomicPtr<Node<T>>,
}

struct Node<T> {
    data: T,
    next: *mut Node<T>,
}

impl<T> AtomicStack<T> {
    pub fn push(&self, data: T) {
        let new_node = Box::into_raw(Box::new(Node {
            data,
            next: std::ptr::null_mut(),
        }));
        
        loop {
            let current = self.head.load(Ordering::Acquire);
            unsafe { (*new_node).next = current; }
            
            match self.head.compare_exchange(
                current,
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }
}
```

### 4.3 Custom Atomic Types

```rust
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ActorStatus {
    Creating = 0,
    Running = 1,
    Suspending = 2,
    Suspended = 3,
    Migrating = 4,
    Checkpointing = 5,
    Destroying = 6,
    Destroyed = 7,
}

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
        // Safe because we control all valid values
        unsafe { std::mem::transmute(self.inner.load(Ordering::Acquire)) }
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
            .map(|v| unsafe { std::mem::transmute(v) })
            .map_err(|v| unsafe { std::mem::transmute(v) })
    }
}
```

---

## 5. Lock-Free Data Structures

### 5.1 Crossbeam Integration

**When to use:**
- High-performance queues
- Work stealing
- Concurrent deques

**Implementation:**
```rust
use crossbeam::queue::ArrayQueue;
use crossbeam::deque::{Worker, Stealer, Injector};

pub struct WorkPool {
    global: Injector<Task>,
    locals: Vec<Worker<Task>>,
}

impl WorkPool {
    pub fn push(&self, task: Task) {
        self.global.push(task);
    }
    
    pub fn steal(&self) -> Option<Task> {
        // Try local first, then global, then remote
        // ...
    }
}
```

### 5.2 Lock-Free Queue

```rust
use crossbeam::queue::SegQueue;

pub struct TaskQueue {
    queue: SegQueue<Task>,
}

impl TaskQueue {
    pub fn push(&self, task: Task) {
        self.queue.push(task);
    }
    
    pub fn pop(&self) -> Option<Task> {
        self.queue.pop()
    }
}
```

### 5.3 Lock-Free Stack

```rust
pub struct LockFreeStack<T> {
    head: AtomicPtr<Node<T>>,
}

impl<T> LockFreeStack<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
    
    pub fn push(&self, value: T) {
        let node = Box::into_raw(Box::new(Node {
            value,
            next: std::ptr::null_mut(),
        }));
        
        loop {
            let head = self.head.load(Ordering::Acquire);
            unsafe { (*node).next = head; }
            
            if self.head
                .compare_exchange(head, node, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
    
    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            
            if head.is_null() {
                return None;
            }
            
            let next = unsafe { (*head).next };
            
            if self.head
                .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                unsafe {
                    let node = Box::from_raw(head);
                    return Some(node.value);
                }
            }
        }
    }
}
```

---

## 6. Memory Ordering Guidelines

### 6.1 Ordering Selection Decision Tree

```
┌─────────────────────────────────────────────────────────────────┐
│                Memory Ordering Decision Tree                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Is ordering important?                                         │
│       │                                                          │
│       ├── NO ──► Relaxed                                        │
│       │         (counters, statistics)                          │
│       │                                                          │
│       └── YES                                                   │
│            │                                                     │
│            ├── Is visibility bidirectional?                     │
│            │      │                                              │
│            │      ├── NO ──► Release (writer) / Acquire (reader)│
│            │      │         (flags, pointers)                   │
│            │      │                                              │
│            │      └── YES ──► AcqRel                             │
│            │               (compare_exchange, fetch_add)        │
│            │                                                     │
│            └── Is total order required?                         │
│                   │                                              │
│                   └── YES ──► SeqCst                            │
│                             (fences, global ordering)           │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Per-Primitive Ordering Recommendations

| Primitive | Default Ordering | Rationale |
|-----------|------------------|-----------|
| Counter increment | Relaxed | No ordering needed |
| Counter snapshot | Relaxed | Eventual consistency OK |
| Flag set | Release | Need visibility |
| Flag check | Acquire | Need visibility |
| Status transition | AcqRel | Bidirectional |
| Pointer update | Release | Need visibility |
| Pointer read | Acquire | Need visibility |
| Reference count | AcqRel | Bidirectional |
| Spin lock | Acquire/Release | Critical section |
| Barrier | SeqCst | Global synchronization |

---

## 7. Cross-Runtime Synchronization

### 7.1 Monoio ↔ Tokio Communication

```rust
pub struct RuntimeBridge {
    // Commands from Monoio to Tokio
    monoio_to_tokio: crossbeam::queue::SegQueue<ControlCommand>,
    
    // Commands from Tokio to Monoio
    tokio_to_monoio: crossbeam::queue::SegQueue<DataCommand>,
    
    // Shared atomic state
    shutdown_flag: AtomicBool,
}

impl RuntimeBridge {
    /// Called from Monoio runtime
    pub fn request_control_operation(&self, cmd: ControlCommand) {
        self.monoio_to_tokio.push(cmd);
    }
    
    /// Called from Tokio runtime
    pub fn request_data_operation(&self, cmd: DataCommand) {
        self.tokio_to_monoio.push(cmd);
    }
    
    /// Check if shutdown requested (both runtimes)
    pub fn is_shutdown(&self) -> bool {
        self.shutdown_flag.load(Ordering::Acquire)
    }
}
```

### 7.2 Safe Cross-Runtime Patterns

```
┌─────────────────────────────────────────────────────────────────┐
│                Cross-Runtime Communication Patterns              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ✅ SAFE Patterns:                                               │
│  ├── Message passing via lock-free queues                       │
│  ├── Shared immutable data (Arc)                                │
│  ├── Atomic primitives                                          │
│  └── One-way communication (no response needed)                 │
│                                                                  │
│  ❌ UNSAFE Patterns:                                             │
│  ├── Holding lock across runtime boundary                       │
│  ├── Blocking wait on channel from other runtime               │
│  ├── Direct function call across boundary                       │
│  └── Shared mutable state without synchronization              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 8. Synchronization Decision Matrix

### 8.1 By Access Pattern

| Pattern | Recommended | Alternative |
|---------|-------------|-------------|
| Read-only | Arc | ArcSwap |
| Read-heavy | RwLock | DashMap |
| Write-heavy | Mutex | DashMap |
| Mixed read/write | DashMap | RwLock |
| Immutable snapshots | ArcSwap | Arc + RwLock |
| Message passing | Channel | Queue |
| Event broadcast | broadcast | watch |
| Request/response | oneshot | mpsc |
| Counters | Atomic | Mutex |
| Complex state | Mutex | Channel |

### 8.2 By Contention Level

| Contention | Recommended | Rationale |
|------------|-------------|-----------|
| Low | Mutex | Simple, fast uncontended |
| Medium | DashMap | Fine-grained locking |
| High | Lock-free | No contention |
| Very High | Sharding | Partition by key |

---

## 9. References

- parking_lot: https://docs.rs/parking_lot/
- dashmap: https://docs.rs/dashmap/
- arc_swap: https://docs.rs/arc-swap/
- crossbeam: https://docs.rs/crossbeam/
- tokio::sync: https://docs.rs/tokio/#synchronization
