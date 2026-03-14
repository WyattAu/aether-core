# Handle Management Strategy
**Aether Resource Management - Phase 3.5**
**Document ID**: RM-HANDLE-001
**Version**: 1.0
**Date**: 2026-03-05
**Status**: Final

---

## 1. Overview

Aether uses RAII-based handle management with strict pooling and cleanup guarantees to prevent resource leaks and ensure deterministic resource reclamation.

---

## 2. Handle Types

### 2.1 Handle Categories

| Category | Examples | Lifetime | Isolation |
|----------|----------|----------|-----------|
| **File Descriptors** | Files, pipes, eventfds | Actor-bound | Per-actor |
| **Sockets** | TCP, UDP, Unix domain | Actor-bound | Per-actor |
| **VM Handles** | Firecracker VM references | System-wide | Capability-controlled |
| **Capability Handles** | Capability tokens | Actor-bound | Per-actor |
| **Memory Handles** | Shared memory regions | Actor-bound | Per-actor |
| **I/O Handles** | io_uring submission/completion | Actor-bound | Per-actor |

### 2.2 Handle Identifiers

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle {
    pub actor_id: ActorId,
    pub handle_id: u32,
    pub handle_type: HandleType,
    pub generation: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleType {
    FileDescriptor,
    Socket,
    VMHandle,
    CapabilityHandle,
    MemoryRegion,
    IoRing,
}
```

### 2.3 Handle Space

- **Per-actor handle space**: Each actor has independent handle namespace
- **Handle reuse protection**: Generation counters prevent ABA problems
- **Maximum handles**: 10,000 per actor (configurable)

---

## 3. RAII Patterns

### 3.1 Basic RAII Handle

```rust
pub struct ManagedHandle<T: HandleBackend> {
    handle: Handle,
    backend: Arc<T>,
    closed: bool,
}

impl<T: HandleBackend> Drop for ManagedHandle<T> {
    fn drop(&mut self) {
        if !self.closed {
            self.backend.close(self.handle);
        }
    }
}

impl<T: HandleBackend> ManagedHandle<T> {
    pub fn new(handle: Handle, backend: Arc<T>) -> Self {
        Self {
            handle,
            backend,
            closed: false,
        }
    }

    pub fn get(&self) -> Handle {
        self.handle
    }

    pub fn close(&mut self) -> Result<(), HandleError> {
        if self.closed {
            return Err(HandleError::AlreadyClosed);
        }
        self.backend.close(self.handle)?;
        self.closed = true;
        Ok(())
    }
}

pub trait HandleBackend: Send + Sync {
    fn close(&self, handle: Handle) -> Result<(), HandleError>;
}
```

### 3.2 File Descriptor Handle

```rust
pub struct FileHandle {
    handle: ManagedHandle<FileBackend>,
}

impl FileHandle {
    pub fn open(path: &Path, flags: OpenFlags) -> Result<Self, IoError> {
        let fd = unsafe { libc::open(path.as_ptr(), flags.bits()) };
        if fd < 0 {
            return Err(IoError::from_errno());
        }

        let handle = Handle {
            actor_id: current_actor_id(),
            handle_id: fd as u32,
            handle_type: HandleType::FileDescriptor,
            generation: 0,
        };

        Ok(Self {
            handle: ManagedHandle::new(handle, FILE_BACKEND.clone()),
        })
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, IoError> {
        let fd = self.handle.get().handle_id as i32;
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if n < 0 {
            Err(IoError::from_errno())
        } else {
            Ok(n as usize)
        }
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, IoError> {
        let fd = self.handle.get().handle_id as i32;
        let n = unsafe { libc::write(fd, buf.as_ptr() as *const _, buf.len()) };
        if n < 0 {
            Err(IoError::from_errno())
        } else {
            Ok(n as usize)
        }
    }
}
```

### 3.3 Socket Handle

```rust
pub struct SocketHandle {
    handle: ManagedHandle<SocketBackend>,
}

impl SocketHandle {
    pub fn bind(addr: &SocketAddr) -> Result<Self, IoError> {
        let fd = unsafe {
            libc::socket(
                libc::AF_INET,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK,
                0,
            )
        };

        if fd < 0 {
            return Err(IoError::from_errno());
        }

        let mut sockaddr = addr.to_libc();
        let ret = unsafe {
            libc::bind(
                fd,
                &mut sockaddr as *mut _ as *mut _,
                std::mem::size_of_val(&sockaddr) as u32,
            )
        };

        if ret < 0 {
            unsafe { libc::close(fd) };
            return Err(IoError::from_errno());
        }

        let handle = Handle {
            actor_id: current_actor_id(),
            handle_id: fd as u32,
            handle_type: HandleType::Socket,
            generation: 0,
        };

        Ok(Self {
            handle: ManagedHandle::new(handle, SOCKET_BACKEND.clone()),
        })
    }

    pub fn listen(&self, backlog: i32) -> Result<(), IoError> {
        let fd = self.handle.get().handle_id as i32;
        let ret = unsafe { libc::listen(fd, backlog) };
        if ret < 0 {
            Err(IoError::from_errno())
        } else {
            Ok(())
        }
    }

    pub fn accept(&self) -> Result<SocketHandle, IoError> {
        let fd = self.handle.get().handle_id as i32;
        let client_fd = unsafe { libc::accept4(fd, std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_NONBLOCK) };
        
        if client_fd < 0 {
            return Err(IoError::from_errno());
        }

        let handle = Handle {
            actor_id: current_actor_id(),
            handle_id: client_fd as u32,
            handle_type: HandleType::Socket,
            generation: 0,
        };

        Ok(SocketHandle {
            handle: ManagedHandle::new(handle, SOCKET_BACKEND.clone()),
        })
    }
}
```

### 3.4 VM Handle

```rust
pub struct VmHandle {
    handle: ManagedHandle<VmBackend>,
    vm_id: VmId,
}

impl VmHandle {
    pub fn create(config: VmConfig) -> Result<Self, VmError> {
        let vm_id = VmId::new();
        let backend = VM_BACKEND.clone();
        
        backend.create_vm(vm_id, &config)?;

        let handle = Handle {
            actor_id: current_actor_id(),
            handle_id: vm_id.as_u32(),
            handle_type: HandleType::VMHandle,
            generation: 0,
        };

        Ok(Self {
            handle: ManagedHandle::new(handle, backend),
            vm_id,
        })
    }

    pub fn start(&self) -> Result<(), VmError> {
        self.handle.backend.start_vm(self.vm_id)
    }

    pub fn stop(&self) -> Result<(), VmError> {
        self.handle.backend.stop_vm(self.vm_id)
    }
}
```

---

## 4. Handle Pooling

### 4.1 Pool Structure

```rust
pub struct HandlePool {
    handles: Mutex<HashMap<Handle, HandleEntry>>,
    free_lists: Mutex<HashMap<HandleType, VecDeque<Handle>>>,
    generation_counters: Mutex<HashMap<(ActorId, HandleType), u16>>,
    stats: Arc<HandlePoolStats>,
}

pub struct HandleEntry {
    handle: Handle,
    state: HandleState,
    created_at: Instant,
    last_accessed: Instant,
}

pub enum HandleState {
    Active,
    Closing,
    Closed,
}
```

### 4.2 Pool Operations

```rust
impl HandlePool {
    pub fn allocate(
        &self,
        actor_id: ActorId,
        handle_type: HandleType,
    ) -> Result<Handle, HandleError> {
        let mut free_lists = self.free_lists.lock().unwrap();
        
        if let Some(handle) = free_lists.get_mut(&handle_type).and_then(|list| list.pop_front()) {
            let mut handles = self.handles.lock().unwrap();
            handles.entry(handle).and_modify(|entry| {
                entry.state = HandleState::Active;
                entry.last_accessed = Instant::now();
            });
            return Ok(handle);
        }

        let mut generation_counters = self.generation_counters.lock().unwrap();
        let generation = generation_counters
            .entry((actor_id, handle_type))
            .or_insert(0);

        let handle = Handle {
            actor_id,
            handle_id: self.next_handle_id()?,
            handle_type,
            generation: *generation,
        };

        *generation = generation.wrapping_add(1);

        let mut handles = self.handles.lock().unwrap();
        handles.insert(
            handle,
            HandleEntry {
                handle,
                state: HandleState::Active,
                created_at: Instant::now(),
                last_accessed: Instant::now(),
            },
        );

        self.stats.total_allocations.fetch_add(1, Ordering::Relaxed);
        Ok(handle)
    }

    pub fn release(&self, handle: Handle) -> Result<(), HandleError> {
        let mut handles = self.handles.lock().unwrap();
        
        let entry = handles.get_mut(&handle).ok_or(HandleError::InvalidHandle)?;
        
        if entry.state != HandleState::Active {
            return Err(HandleError::HandleNotActive);
        }

        entry.state = HandleState::Closed;
        entry.last_accessed = Instant::now();

        let mut free_lists = self.free_lists.lock().unwrap();
        free_lists
            .entry(handle.handle_type)
            .or_insert_with(VecDeque::new)
            .push_back(handle);

        self.stats.total_releases.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn next_handle_id(&self) -> Result<u32, HandleError> {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        
        if id >= 10_000 {
            return Err(HandleError::HandleLimitExceeded);
        }
        
        Ok(id)
    }
}
```

### 4.3 Pool Statistics

```rust
pub struct HandlePoolStats {
    pub total_allocations: AtomicU64,
    pub total_releases: AtomicU64,
    pub current_active: AtomicU64,
    pub peak_active: AtomicU64,
    pub pool_hits: AtomicU64,
    pub pool_misses: AtomicU64,
}

impl HandlePool {
    pub fn collect_stats(&self) -> HandlePoolStatsSnapshot {
        HandlePoolStatsSnapshot {
            total_allocations: self.stats.total_allocations.load(Ordering::Relaxed),
            total_releases: self.stats.total_releases.load(Ordering::Relaxed),
            current_active: self.stats.current_active.load(Ordering::Relaxed),
            peak_active: self.stats.peak_active.load(Ordering::Relaxed),
            pool_hits: self.stats.pool_hits.load(Ordering::Relaxed),
            pool_misses: self.stats.pool_misses.load(Ordering::Relaxed),
        }
    }
}
```

---

## 5. Cleanup Guarantees

### 5.1 RAII Guarantee

All handles are automatically closed when dropped:

```rust
impl<T: HandleBackend> Drop for ManagedHandle<T> {
    fn drop(&mut self) {
        if !self.closed {
            if let Err(e) = self.backend.close(self.handle) {
                log::error!("Failed to close handle {:?}: {}", self.handle, e);
            }
        }
    }
}
```

### 5.2 Scope-Based Cleanup

```rust
pub fn with_file_handle<F, R>(path: &Path, f: F) -> Result<R, IoError>
where
    F: FnOnce(&FileHandle) -> Result<R, IoError>,
{
    let handle = FileHandle::open(path, OpenFlags::RD_ONLY)?;
    f(&handle)
}
```

### 5.3 Actor Termination Cleanup

When an actor terminates, all its handles must be cleaned up:

```rust
impl HandleManager {
    pub fn cleanup_actor(&self, actor_id: ActorId) -> Result<(), HandleError> {
        let handles = self.pool.handles.lock().unwrap();
        
        let actor_handles: Vec<Handle> = handles
            .keys()
            .filter(|h| h.actor_id == actor_id)
            .copied()
            .collect();

        drop(handles);

        for handle in actor_handles {
            self.release(handle)?;
        }

        Ok(())
    }
}
```

### 5.4 Panic Cleanup

With `panic=abort`, panics terminate the process immediately. Cleanup handlers must be registered:

```rust
pub fn register_cleanup_handler() {
    ctrlc::set_handler(move || {
        log::info!("Received termination signal, cleaning up...");
        let handle_manager = HANDLE_MANAGER.lock().unwrap();
        if let Err(e) = handle_manager.cleanup_all() {
            log::error!("Cleanup failed: {}", e);
        }
        std::process::exit(0);
    }).expect("Failed to set Ctrl-C handler");
}
```

### 5.5 FD CLOEXEC

All file descriptors must have `FD_CLOEXEC` set to prevent leakage across exec:

```rust
impl FileHandle {
    pub fn open(path: &Path, flags: OpenFlags) -> Result<Self, IoError> {
        let fd = unsafe { libc::open(path.as_ptr(), flags.bits() | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(IoError::from_errno());
        }

        let handle = Handle {
            actor_id: current_actor_id(),
            handle_id: fd as u32,
            handle_type: HandleType::FileDescriptor,
            generation: 0,
        };

        Ok(Self {
            handle: ManagedHandle::new(handle, FILE_BACKEND.clone()),
        })
    }
}
```

---

## 6. Handle Transfer and Sharing

### 6.1 Handle Transfer (Send)

Handles can be transferred between actors via messages:

```rust
pub struct HandleTransfer {
    pub handle: Handle,
    pub from_actor: ActorId,
    pub to_actor: ActorId,
}

impl HandleManager {
    pub fn transfer_handle(&self, transfer: HandleTransfer) -> Result<(), HandleError> {
        let mut handles = self.pool.handles.lock().unwrap();
        
        let entry = handles.get_mut(&transfer.handle).ok_or(HandleError::InvalidHandle)?;
        
        if entry.handle.actor_id != transfer.from_actor {
            return Err(HandleError::NotOwner);
        }

        let new_handle = Handle {
            actor_id: transfer.to_actor,
            handle_id: entry.handle.handle_id,
            handle_type: entry.handle.handle_type,
            generation: entry.handle.generation,
        };

        handles.remove(&transfer.handle);
        handles.insert(new_handle, HandleEntry {
            handle: new_handle,
            state: HandleState::Active,
            created_at: entry.created_at,
            last_accessed: Instant::now(),
        });

        Ok(())
    }
}
```

### 6.2 Handle Sharing (Clone)

Handles can be cloned (shared) within the same actor:

```rust
impl<T: HandleBackend> Clone for ManagedHandle<T> {
    fn clone(&self) -> Self {
        let mut new_handle = ManagedHandle::new(self.handle, self.backend.clone());
        new_handle.closed = false;
        new_handle
    }
}
```

### 6.3 Cross-Actor Handle Sharing

Cross-actor handle sharing requires capability:

```rust
impl HandleManager {
    pub fn share_handle(
        &self,
        handle: Handle,
        to_actor: ActorId,
        capability: CapabilityToken,
    ) -> Result<Handle, HandleError> {
        if !capability.allows(CapabilityRight::SHARE_HANDLES) {
            return Err(HandleError::InsufficientCapability);
        }

        let mut handles = self.pool.handles.lock().unwrap();
        
        let entry = handles.get(&handle).ok_or(HandleError::InvalidHandle)?;
        
        let shared_handle = Handle {
            actor_id: to_actor,
            handle_id: entry.handle.handle_id,
            handle_type: entry.handle.handle_type,
            generation: entry.handle.generation,
        };

        handles.insert(shared_handle, HandleEntry {
            handle: shared_handle,
            state: HandleState::Active,
            created_at: entry.created_at,
            last_accessed: Instant::now(),
        });

        Ok(shared_handle)
    }
}
```

---

## 7. Handle Validation

### 7.1 Runtime Validation

```rust
impl HandleManager {
    pub fn validate(&self, handle: Handle) -> Result<(), HandleError> {
        let handles = self.pool.handles.lock().unwrap();
        
        let entry = handles.get(&handle).ok_or(HandleError::InvalidHandle)?;
        
        if entry.state != HandleState::Active {
            return Err(HandleError::HandleNotActive);
        }

        if entry.handle.generation != handle.generation {
            return Err(HandleError::StaleHandle);
        }

        Ok(())
    }
}
```

### 7.2 Capability Check

```rust
impl HandleManager {
    pub fn check_capability(
        &self,
        handle: Handle,
        required: CapabilityRight,
    ) -> Result<(), HandleError> {
        let capability = self.capability_manager.get(handle)?;
        
        if !capability.allows(required) {
            return Err(HandleError::InsufficientCapability);
        }

        Ok(())
    }
}
```

---

## 8. Handle Monitoring

### 8.1 Metrics Collection

```rust
pub struct HandleMetrics {
    pub total_handles: u64,
    pub active_handles: u64,
    pub handles_by_type: HashMap<HandleType, u64>,
    pub handles_by_actor: HashMap<ActorId, u64>,
    pub pool_stats: HandlePoolStatsSnapshot,
}

impl HandleManager {
    pub fn collect_metrics(&self) -> HandleMetrics {
        let handles = self.pool.handles.lock().unwrap();
        
        let total_handles = handles.len() as u64;
        let active_handles = handles.values().filter(|e| e.state == HandleState::Active).count() as u64;
        
        let handles_by_type = handles
            .values()
            .filter(|e| e.state == HandleState::Active)
            .map(|e| e.handle.handle_type)
            .fold(HashMap::new(), |mut acc, t| {
                *acc.entry(t).or_insert(0) += 1;
                acc
            });

        let handles_by_actor = handles
            .values()
            .filter(|e| e.state == HandleState::Active)
            .map(|e| e.handle.actor_id)
            .fold(HashMap::new(), |mut acc, a| {
                *acc.entry(a).or_insert(0) += 1;
                acc
            });

        HandleMetrics {
            total_handles,
            active_handles,
            handles_by_type,
            handles_by_actor,
            pool_stats: self.pool.collect_stats(),
        }
    }
}
```

---

## 9. Error Handling

### 9.1 Handle Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum HandleError {
    #[error("Invalid handle: {0:?}")]
    InvalidHandle(Handle),
    
    #[error("Handle not active: {0:?}")]
    HandleNotActive(Handle),
    
    #[error("Handle already closed: {0:?}")]
    AlreadyClosed(Handle),
    
    #[error("Handle limit exceeded")]
    HandleLimitExceeded,
    
    #[error("Not handle owner")]
    NotOwner,
    
    #[error("Stale handle (generation mismatch)")]
    StaleHandle,
    
    #[error("Insufficient capability")]
    InsufficientCapability,
    
    #[error("Backend error: {0}")]
    BackendError(String),
}
```

---

## 10. Testing Requirements

### 10.1 Unit Tests

- RAII cleanup correctness
- Pool allocation/deallocation
- Handle validation
- Generation counter behavior

### 10.2 Integration Tests

- Actor termination cleanup
- Handle transfer between actors
- Handle sharing
- Capability enforcement

### 10.3 Leak Detection Tests

- Long-running handle usage
- Stress testing handle creation/destruction
- Panic recovery cleanup

---

## 11. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Handle allocation | <500ns | Microbenchmark |
| Handle release | <200ns | Microbenchmark |
| Handle validation | <50ns | Microbenchmark |
| Pool hit rate | >80% | Telemetry |
| Cleanup latency (actor termination) | <1ms | Integration test |

---

## 12. References

- RAII pattern: https://doc.rust-lang.org/rust-by-example/scope/raii.html
- File descriptor management: https://man7.org/linux/man-pages/man2/open.2.html
- ADR-003: panic=abort decision
- RM-MEM-001: Memory management

---

**Approval**: Resource Engineer
**Review Date**: 2026-03-05
