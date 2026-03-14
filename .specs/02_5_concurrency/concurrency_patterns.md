# Concurrency Patterns for Project Aether

**Document ID:** CONC-CP-001  
**Version:** 1.0.0  
**Status:** Active  
**Created:** 2026-03-05

---

## 1. Executive Summary

This document defines concurrency design patterns used in Aether's dual-runtime architecture. These patterns ensure thread safety, prevent deadlocks, and enable scalable concurrent execution.

### Pattern Summary

| Pattern | Use Case | Benefit |
|---------|----------|---------|
| Actor Isolation | WASM execution | No shared state |
| Message Passing | Cross-component communication | Deadlock freedom |
| Shared-Nothing | Data plane processing | Lock-free |
| Thread-Local Storage | Per-core state | Zero contention |
| Work Stealing | Load balancing | Utilization |
| Event Loop | I/O handling | Deterministic |

---

## 2. Actor Isolation Pattern

### 2.1 Pattern Description

Each actor executes in complete isolation with no shared mutable state. Communication occurs only through message passing.

```
┌─────────────────────────────────────────────────────────────────┐
│                    Actor Isolation Pattern                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐    │
│  │   Actor A    │     │   Actor B    │     │   Actor C    │    │
│  │  ┌────────┐  │     │  ┌────────┐  │     │  ┌────────┐  │    │
│  │  │ State  │  │     │  │ State  │  │     │  │ State  │  │    │
│  │  └────────┘  │     │  └────────┘  │     │  └────────┘  │    │
│  │  ┌────────┐  │     │  ┌────────┐  │     │  ┌────────┐  │    │
│  │  │ Mailbox│  │     │  │ Mailbox│  │     │  │ Mailbox│  │    │
│  │  └────────┘  │     │  └────────┘  │     │  └────────┘  │    │
│  └──────┬───────┘     └──────┬───────┘     └──────┬───────┘    │
│         │                    │                    │             │
│         └────────────────────┼────────────────────┘             │
│                              │                                   │
│                    ┌─────────▼─────────┐                        │
│                    │   Message Router   │                        │
│                    │   (Lock-Free)      │                        │
│                    └────────────────────┘                        │
│                                                                  │
│  Invariant: No actor can access another actor's state directly  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Implementation

```rust
pub struct IsolatedActor {
    id: ActorId,
    mailbox: Mutex<VecDeque<Message>>,
    state: Mutex<ActorState>,
    status: AtomicActorStatus,
}

impl IsolatedActor {
    pub fn send_message(&self, msg: Message) {
        let mut mailbox = self.mailbox.lock();
        mailbox.push_back(msg);
    }
    
    pub fn process_messages(&self) -> Result<(), ActorError> {
        // Only one thread processes at a time
        let mut state = self.state.lock();
        let mut mailbox = self.mailbox.lock();
        
        while let Some(msg) = mailbox.pop_front() {
            state.handle_message(msg)?;
        }
        
        Ok(())
    }
}

/// Actor system guarantees
/// 
/// 1. Each actor has isolated state (no shared mutable state)
/// 2. Communication only via message passing
/// 3. Messages are delivered in order (per sender)
/// 4. Actor processes one message at a time
pub struct ActorSystem {
    actors: DashMap<ActorId, Arc<IsolatedActor>>,
    router: MessageRouter,
}
```

### 2.3 Benefits

- **No data races**: Each actor's state is isolated
- **No deadlocks**: No shared locks between actors
- **Deterministic**: Message ordering is preserved
- **Scalable**: Actors can run in parallel

---

## 3. Message Passing Pattern

### 3.1 Pattern Description

Components communicate exclusively through message passing, avoiding shared mutable state.

```
┌─────────────────────────────────────────────────────────────────┐
│                    Message Passing Pattern                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Component A                     Component B                     │
│  ───────────                     ───────────                     │
│                                                                  │
│  ┌─────────────┐                ┌─────────────┐                 │
│  │   State     │                │   State     │                 │
│  │  (private)  │                │  (private)  │                 │
│  └──────┬──────┘                └──────┬──────┘                 │
│         │                              │                         │
│         ▼                              ▼                         │
│  ┌─────────────┐    Channel    ┌─────────────┐                 │
│  │   Sender    │ ────────────► │  Receiver   │                 │
│  └─────────────┘               └─────────────┘                 │
│                                                                  │
│  Types:                                                          │
│  ├── mpsc: Multi-producer, single-consumer                      │
│  ├── broadcast: Multi-producer, multi-consumer                  │
│  ├── watch: Single-producer, multi-consumer                     │
│  └── oneshot: Single-producer, single-consumer                  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Implementation

```rust
/// Command pattern for cross-component communication
pub enum ControlCommand {
    CreateActor {
        descriptor: ActorDescriptor,
        reply: oneshot::Sender<Result<ActorId, Error>>,
    },
    DestroyActor {
        actor_id: ActorId,
        reply: oneshot::Sender<Result<(), Error>>,
    },
    Checkpoint {
        actor_id: ActorId,
        reply: oneshot::Sender<Result<CheckpointId, Error>>,
    },
}

pub struct ControlPlane {
    command_tx: mpsc::Sender<ControlCommand>,
}

impl ControlPlane {
    pub async fn create_actor(&self, descriptor: ActorDescriptor) -> Result<ActorId, Error> {
        let (reply_tx, reply_rx) = oneshot::channel();
        
        self.command_tx.send(ControlCommand::CreateActor {
            descriptor,
            reply: reply_tx,
        }).await?;
        
        reply_rx.await?
    }
}

pub struct ControlPlaneHandler {
    command_rx: mpsc::Receiver<ControlCommand>,
}

impl ControlPlaneHandler {
    pub async fn run(&mut self) {
        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                ControlCommand::CreateActor { descriptor, reply } => {
                    let result = self.do_create_actor(descriptor).await;
                    let _ = reply.send(result);
                }
                // ...
            }
        }
    }
}
```

### 3.3 Channel Selection Guide

| Scenario | Channel Type | Bounded? | Rationale |
|----------|-------------|----------|-----------|
| Actor mailbox | mpsc | Yes | Backpressure |
| Event broadcast | broadcast | Yes | History needed |
| Config updates | watch | N/A | Latest only |
| Request/response | oneshot | N/A | Single use |
| Worker pool | mpsc | Yes | Bounded queue |

---

## 4. Shared-Nothing Architecture

### 4.1 Pattern Description

Each thread/core owns its data exclusively, eliminating shared state and synchronization overhead.

```
┌─────────────────────────────────────────────────────────────────┐
│                  Shared-Nothing Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Core 0           Core 1           Core 2           Core 3     │
│  ┌─────┐          ┌─────┐          ┌─────┐          ┌─────┐    │
│  │RunQ │          │RunQ │          │RunQ │          │RunQ │    │
│  ├─────┤          ├─────┤          ├─────┤          ├─────┤    │
│  │Cache│          │Cache│          │Cache│          │Cache│    │
│  ├─────┤          ├─────┤          ├─────┤          ├─────┤    │
│  │Conn │          │Conn │          │Conn │          │Conn │    │
│  │Pool │          │Pool │          │Pool │          │Pool │    │
│  └─────┘          └─────┘          └─────┘          └─────┘    │
│                                                                  │
│  No shared state between cores                                  │
│  Communication via lock-free message passing                    │
│                                                                  │
│  Benefits:                                                       │
│  ├── Zero lock contention                                       │
│  ├── Cache-friendly (data stays local)                          │
│  ├── Predictable latency                                        │
│  └── Linear scalability                                         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Implementation

```rust
pub struct PerCoreData {
    run_queue: VecDeque<Task>,
    connection_pool: LocalConnectionPool,
    cache: LocalCache,
}

thread_local! {
    static PER_CORE: RefCell<PerCoreData> = RefCell::new(PerCoreData::new());
}

impl PerCoreData {
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&mut PerCoreData) -> R,
    {
        PER_CORE.with(|cell| f(&mut cell.borrow_mut()))
    }
    
    pub fn process_task(&mut self) -> Option<TaskResult> {
        self.run_queue.pop_front().map(|task| {
            // Process without any synchronization
            task.execute(&mut self.cache)
        })
    }
}

/// Work stealing for load balancing
pub struct WorkStealingQueue {
    local: VecDeque<Task>,
    global: Arc<Mutex<VecDeque<Task>>>,
}

impl WorkStealingQueue {
    pub fn push(&mut self, task: Task) {
        self.local.push_back(task);
    }
    
    pub fn pop(&mut self) -> Option<Task> {
        // Try local first
        if let Some(task) = self.local.pop_front() {
            return Some(task);
        }
        
        // Steal from global
        let mut global = self.global.lock();
        global.pop_front()
    }
}
```

---

## 5. Thread-Local Storage Pattern

### 5.1 Pattern Description

Use thread-local storage for per-thread state to avoid synchronization overhead.

```rust
use std::cell::RefCell;

pub struct ThreadLocalState {
    buffer: Vec<u8>,
    statistics: Statistics,
}

thread_local! {
    static STATE: RefCell<ThreadLocalState> = RefCell::new(ThreadLocalState {
        buffer: Vec::with_capacity(64 * 1024),
        statistics: Statistics::default(),
    });
}

impl ThreadLocalState {
    pub fn with_buffer<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Vec<u8>) -> R,
    {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            f(&mut state.buffer)
        })
    }
    
    pub fn record_stat<F>(f: F)
    where
        F: FnOnce(&mut Statistics),
    {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            f(&mut state.statistics)
        })
    }
}
```

### 5.2 Use Cases

| Use Case | Thread-Local Data | Rationale |
|----------|------------------|-----------|
| Buffer reuse | Vec<u8> | Avoid allocation |
| Random state | StdRng | Avoid contention |
| Statistics | Counter | Lock-free updates |
| Connection pool | LocalPool | No synchronization |
| ID generation | Sequence | Per-thread IDs |

### 5.3 Thread-Local Allocator

```rust
use thread_local::ThreadLocal;

pub struct ThreadLocalPool<T> {
    pools: ThreadLocal<RefCell<Vec<T>>>,
    factory: fn() -> T,
}

impl<T> ThreadLocalPool<T> {
    pub fn new(factory: fn() -> T) -> Self {
        Self {
            pools: ThreadLocal::new(),
            factory,
        }
    }
    
    pub fn acquire(&self) -> Pooled<T> {
        let pool = self.pools.get_or(|| {
            RefCell::new(Vec::with_capacity(16))
        });
        
        let mut pool = pool.borrow_mut();
        let value = pool.pop().unwrap_or_else(|| (self.factory)());
        
        Pooled { value, pool: pool }
    }
}

pub struct Pooled<'a, T> {
    value: T,
    pool: RefMut<'a, Vec<T>>,
}

impl<'a, T> Drop for Pooled<'a, T> {
    fn drop(&mut self) {
        self.pool.push(mem::take(&mut self.value));
    }
}
```

---

## 6. Work Stealing Pattern

### 6.1 Pattern Description

Threads steal work from each other to balance load across cores.

```
┌─────────────────────────────────────────────────────────────────┐
│                    Work Stealing Pattern                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Thread 0              Thread 1              Thread 2           │
│  ┌─────────┐          ┌─────────┐          ┌─────────┐         │
│  │ Local Q │          │ Local Q │          │ Local Q │         │
│  │ [1,2,3] │          │ [4,5]   │          │ [ ]     │         │
│  └────┬────┘          └────┬────┘          └────┬────┘         │
│       │                    │                    │               │
│       │                    │                    │               │
│       │                    │    steal           │               │
│       │                    │◄───────────────────┤               │
│       │                    │                    │               │
│       ▼                    ▼                    ▼               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Global Injector                         │  │
│  │                    [6, 7, 8, 9, 10]                       │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Implementation

```rust
use crossbeam::deque::{Injector, Stealer, Worker};

pub struct WorkStealingScheduler {
    global: Arc<Injector<Task>>,
    locals: Vec<Worker<Task>>,
    stealers: Vec<Stealer<Task>>,
}

impl WorkStealingScheduler {
    pub fn new(num_workers: usize) -> Self {
        let global = Arc::new(Injector::new());
        let mut locals = Vec::with_capacity(num_workers);
        let mut stealers = Vec::with_capacity(num_workers);
        
        for _ in 0..num_workers {
            let worker = Worker::new_fifo();
            stealers.push(worker.stealer());
            locals.push(worker);
        }
        
        Self { global, locals, stealers }
    }
    
    pub fn push(&self, task: Task) {
        self.global.push(task);
    }
    
    pub fn pop(&self, worker_id: usize) -> Option<Task> {
        let local = &self.locals[worker_id];
        
        // 1. Try local queue
        if let Some(task) = local.pop() {
            return Some(task);
        }
        
        // 2. Try global injector
        loop {
            match self.global.steal_batch_and_pop(local) {
                crossbeam::deque::Steal::Success(task) => return Some(task),
                crossbeam::deque::Steal::Empty => break,
                crossbeam::deque::Steal::Retry => continue,
            }
        }
        
        // 3. Try stealing from other workers
        for (id, stealer) in self.stealers.iter().enumerate() {
            if id == worker_id {
                continue;
            }
            
            loop {
                match stealer.steal() {
                    crossbeam::deque::Steal::Success(task) => return Some(task),
                    crossbeam::deque::Steal::Empty => break,
                    crossbeam::deque::Steal::Retry => continue,
                }
            }
        }
        
        None
    }
}
```

---

## 7. Event Loop Pattern

### 7.1 Pattern Description

Single-threaded event processing for deterministic behavior and simplified reasoning.

```rust
pub struct EventLoop {
    events: crossbeam::queue::SegQueue<Event>,
    handlers: HashMap<EventType, Box<dyn EventHandler>>,
    running: AtomicBool,
}

impl EventLoop {
    pub fn run(&self) {
        while self.running.load(Ordering::Relaxed) {
            while let Some(event) = self.events.pop() {
                if let Some(handler) = self.handlers.get(&event.event_type) {
                    handler.handle(event);
                }
            }
            
            // Yield to prevent busy-waiting
            std::hint::spin_loop();
        }
    }
    
    pub fn submit(&self, event: Event) {
        self.events.push(event);
    }
}

pub trait EventHandler: Send + Sync {
    fn handle(&self, event: Event);
}
```

### 7.2 Monoio Event Loop

```rust
pub async fn data_plane_loop(
    config: DataPlaneConfig,
) -> Result<(), Error> {
    let mut connections = ConnectionPool::new(config.max_connections);
    let mut router = MessageRouter::new();
    
    loop {
        // Process incoming messages
        while let Some(msg) = router.recv().await? {
            let conn = connections.get_or_create(msg.target).await?;
            conn.send(msg).await?;
        }
        
        // Process outgoing messages
        for conn in connections.iter() {
            while let Some(msg) = conn.recv().await? {
                router.route(msg).await?;
            }
        }
    }
}
```

---

## 8. Barrier Synchronization Pattern

### 8.1 Pattern Description

Coordinate multiple threads at synchronization points.

```rust
use std::sync::Barrier;

pub struct ParallelProcessor {
    barrier: Arc<Barrier>,
    workers: usize,
}

impl ParallelProcessor {
    pub fn new(workers: usize) -> Self {
        Self {
            barrier: Arc::new(Barrier::new(workers)),
            workers,
        }
    }
    
    pub fn process<F>(&self, data: &mut [Data], f: F)
    where
        F: Fn(&mut Data) + Send + Sync,
    {
        let chunk_size = data.len() / self.workers;
        
        let handles: Vec<_> = (0..self.workers)
            .map(|i| {
                let barrier = self.barrier.clone();
                let start = i * chunk_size;
                let end = if i == self.workers - 1 {
                    data.len()
                } else {
                    start + chunk_size
                };
                let chunk = &mut data[start..end];
                
                thread::spawn(move || {
                    // Phase 1: Process
                    for item in chunk.iter_mut() {
                        f(item);
                    }
                    
                    // Synchronize all threads
                    barrier.wait();
                    
                    // Phase 2: Post-process (all phase 1 complete)
                    // ...
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

## 9. Double-Checked Locking Pattern

### 9.1 Pattern Description

Lazy initialization with thread-safe singleton pattern.

```rust
use std::sync::OnceLock;

pub struct Singleton {
    config: OnceLock<Config>,
    module_cache: OnceLock<Arc<ModuleCache>>,
}

impl Singleton {
    pub fn config(&self) -> &Config {
        self.config.get_or_init(|| {
            Config::load().expect("Failed to load config")
        })
    }
    
    pub fn module_cache(&self) -> &Arc<ModuleCache> {
        self.module_cache.get_or_init(|| {
            Arc::new(ModuleCache::new(self.config()))
        })
    }
}

// Modern Rust pattern using OnceLock
pub static HOST_RUNTIME: OnceLock<HostRuntime> = OnceLock::new();

pub fn get_runtime() -> &'static HostRuntime {
    HOST_RUNTIME.get_or_init(|| {
        HostRuntime::new(Config::load().unwrap())
    })
}
```

---

## 10. Pattern Selection Guide

| Scenario | Pattern | Rationale |
|----------|---------|-----------|
| WASM execution | Actor Isolation | No shared state |
| Cross-component | Message Passing | Deadlock freedom |
| Data plane | Shared-Nothing | Zero contention |
| Per-core state | Thread-Local | No synchronization |
| Load balancing | Work Stealing | Utilization |
| I/O handling | Event Loop | Deterministic |
| Initialization | Double-Checked | Lazy + thread-safe |
| Parallel processing | Barrier | Coordination |

---

## 11. References

- "Patterns for Parallel Programming" - Mattson et al.
- "Seven Concurrency Models in Seven Weeks" - Paul Butcher
- Rust Concurrency: https://doc.rust-lang.org/book/ch16-00-concurrency.html
- Tokio Patterns: https://tokio.rs/tokio/tutorial
