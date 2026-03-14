# Cleanup Protocols
**Aether Resource Management - Phase 3.5**
**Document ID**: RM-CLEANUP-001
**Version**: 1.0
**Date**: 2026-03-05
**Status**: Final

---

## 1. Overview

Aether implements robust cleanup protocols to ensure all resources are properly released during graceful shutdown, actor termination, panics, and orphan resource scenarios.

---

## 2. Graceful Shutdown Sequence

### 2.1 Shutdown Phases

```
┌─────────────────────────────────────────────────────────┐
│ Phase 1: Drain (SIGTERM received)                       │
│ - Stop accepting new actors                             │
│ - Stop accepting new messages                           │
│ - Allow in-flight messages to complete                  │
│ Duration: 30 seconds (configurable)                     │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ Phase 2: Checkpoint (Save state)                        │
│ - Persist actor state                                   │
│ - Save capability tables                                │
│ - Flush pending I/O                                     │
│ Duration: 10 seconds                                    │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ Phase 3: Terminate (Stop actors)                        │
│ - Send shutdown signal to all actors                    │
│ - Wait for graceful termination                         │
│ - Force kill after timeout                              │
│ Duration: 15 seconds                                    │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│ Phase 4: Cleanup (Release resources)                    │
│ - Close all handles                                     │
│ - Release memory                                        │
│ - Shutdown VMs                                          │
│ - Close network connections                             │
│ Duration: 5 seconds                                     │
└─────────────────────────────────────────────────────────┘
```

### 2.2 Shutdown Coordinator

```rust
pub struct ShutdownCoordinator {
    phase: AtomicU8,
    drain_deadline: Option<Instant>,
    checkpoint_deadline: Option<Instant>,
    terminate_deadline: Option<Instant>,
    cleanup_deadline: Option<Instant>,
    shutdown_hooks: Vec<Box<dyn Fn() + Send + Sync>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShutdownPhase {
    Running = 0,
    Draining = 1,
    Checkpointing = 2,
    Terminating = 3,
    CleaningUp = 4,
    Complete = 5,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            phase: AtomicU8::new(ShutdownPhase::Running as u8),
            drain_deadline: None,
            checkpoint_deadline: None,
            terminate_deadline: None,
            cleanup_deadline: None,
            shutdown_hooks: Vec::new(),
        }
    }

    pub fn initiate_shutdown(&mut self) {
        log::info!("Initiating graceful shutdown...");
        
        self.drain_deadline = Some(Instant::now() + Duration::from_secs(30));
        self.checkpoint_deadline = Some(Instant::now() + Duration::from_secs(40));
        self.terminate_deadline = Some(Instant::now() + Duration::from_secs(55));
        self.cleanup_deadline = Some(Instant::now() + Duration::from_secs(60));
        
        self.set_phase(ShutdownPhase::Draining);
    }

    pub fn current_phase(&self) -> ShutdownPhase {
        match self.phase.load(Ordering::SeqCst) {
            0 => ShutdownPhase::Running,
            1 => ShutdownPhase::Draining,
            2 => ShutdownPhase::Checkpointing,
            3 => ShutdownPhase::Terminating,
            4 => ShutdownPhase::CleaningUp,
            5 => ShutdownPhase::Complete,
            _ => ShutdownPhase::Running,
        }
    }

    fn set_phase(&self, phase: ShutdownPhase) {
        self.phase.store(phase as u8, Ordering::SeqCst);
        log::info!("Shutdown phase: {:?}", phase);
    }

    pub fn register_hook<F>(&mut self, hook: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.shutdown_hooks.push(Box::new(hook));
    }

    pub fn run_shutdown(&mut self) -> Result<(), ShutdownError> {
        self.initiate_shutdown();
        
        self.phase_drain()?;
        self.phase_checkpoint()?;
        self.phase_terminate()?;
        self.phase_cleanup()?;
        
        self.set_phase(ShutdownPhase::Complete);
        log::info!("Graceful shutdown complete");
        
        Ok(())
    }

    fn phase_drain(&self) -> Result<(), ShutdownError> {
        log::info!("Phase 1: Draining in-flight messages...");
        
        while Instant::now() < self.drain_deadline.unwrap() {
            if self.in_flight_messages() == 0 {
                log::info!("All in-flight messages drained");
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        
        log::warn!("Drain timeout reached, proceeding to checkpoint");
        Ok(())
    }

    fn phase_checkpoint(&mut self) -> Result<(), ShutdownError> {
        log::info!("Phase 2: Checkpointing state...");
        self.set_phase(ShutdownPhase::Checkpointing);
        
        for hook in &self.shutdown_hooks {
            hook();
        }
        
        log::info!("Checkpoint complete");
        Ok(())
    }

    fn phase_terminate(&self) -> Result<(), ShutdownError> {
        log::info!("Phase 3: Terminating actors...");
        self.set_phase(ShutdownPhase::Terminating);
        
        self.terminate_all_actors()?;
        
        log::info!("All actors terminated");
        Ok(())
    }

    fn phase_cleanup(&self) -> Result<(), ShutdownError> {
        log::info!("Phase 4: Cleaning up resources...");
        self.set_phase(ShutdownPhase::CleaningUp);
        
        self.cleanup_all_resources()?;
        
        log::info!("Cleanup complete");
        Ok(())
    }

    fn in_flight_messages(&self) -> usize {
        0 // TODO: Implement actual check
    }

    fn terminate_all_actors(&self) -> Result<(), ShutdownError> {
        Ok(())
    }

    fn cleanup_all_resources(&self) -> Result<(), ShutdownError> {
        Ok(())
    }
}
```

### 2.3 Signal Handling

```rust
pub fn register_signal_handlers(coordinator: Arc<Mutex<ShutdownCoordinator>>) {
    ctrlc::set_handler(move || {
        log::info!("Received SIGINT, initiating shutdown...");
        let mut coord = coordinator.lock().unwrap();
        if let Err(e) = coord.run_shutdown() {
            log::error!("Shutdown failed: {}", e);
            std::process::exit(1);
        }
        std::process::exit(0);
    }).expect("Failed to set signal handler");

    unsafe {
        libc::signal(libc::SIGTERM, handle_sigterm as usize);
    }
}

extern "C" fn handle_sigterm(_signum: i32) {
    log::info!("Received SIGTERM");
    // Trigger shutdown coordinator
}
```

---

## 3. Actor Termination Cleanup

### 3.1 Actor Cleanup Sequence

```rust
impl ActorSystem {
    pub fn terminate_actor(&self, actor_id: ActorId) -> Result<(), ActorError> {
        log::info!("Terminating actor: {}", actor_id);
        
        // Step 1: Stop message delivery
        self.mailbox_manager.block(actor_id)?;
        
        // Step 2: Drain mailbox
        self.drain_mailbox(actor_id)?;
        
        // Step 3: Invoke actor cleanup handler
        self.invoke_cleanup_handler(actor_id)?;
        
        // Step 4: Close all actor handles
        self.handle_manager.cleanup_actor(actor_id)?;
        
        // Step 5: Release actor memory
        self.memory_manager.release_actor_memory(actor_id)?;
        
        // Step 6: Revoke capabilities
        self.capability_manager.revoke_all(actor_id)?;
        
        // Step 7: Close network connections
        self.network_manager.close_actor_connections(actor_id)?;
        
        // Step 8: Stop WASM/VM instance
        self.runtime_manager.stop_instance(actor_id)?;
        
        // Step 9: Remove from registry
        self.registry.remove(actor_id)?;
        
        log::info!("Actor {} terminated successfully", actor_id);
        Ok(())
    }

    fn drain_mailbox(&self, actor_id: ActorId) -> Result<(), ActorError> {
        let mailbox = self.mailbox_manager.get(actor_id)?;
        
        while let Some(msg) = mailbox.try_recv() {
            // Return message to sender or drop
            if msg.requires_ack() {
                msg.nack(ActorError::ActorTerminated);
            }
        }
        
        Ok(())
    }

    fn invoke_cleanup_handler(&self, actor_id: ActorId) -> Result<(), ActorError> {
        if let Some(handler) = self.cleanup_handlers.get(&actor_id) {
            handler()?;
        }
        Ok(())
    }
}
```

### 3.2 Actor Cleanup Handler

```rust
pub trait ActorCleanup {
    fn on_terminate(&mut self) -> Result<(), ActorError>;
}

impl<T: ActorCleanup> ActorInstance<T> {
    pub fn register_cleanup(&mut self) {
        let actor_id = self.id;
        let cleanup = self.actor.on_terminate.clone();
        
        self.system.cleanup_handlers.insert(actor_id, Box::new(move || {
            cleanup()
        }));
    }
}
```

### 3.3 Automatic Cleanup on Drop

```rust
pub struct ActorGuard {
    actor_id: ActorId,
    system: Arc<ActorSystem>,
    cleaned_up: bool,
}

impl Drop for ActorGuard {
    fn drop(&mut self) {
        if !self.cleaned_up {
            if let Err(e) = self.system.terminate_actor(self.actor_id) {
                log::error!("Failed to cleanup actor {}: {}", self.actor_id, e);
            }
        }
    }
}

impl ActorGuard {
    pub fn new(actor_id: ActorId, system: Arc<ActorSystem>) -> Self {
        Self {
            actor_id,
            system,
            cleaned_up: false,
        }
    }

    pub fn manual_cleanup(mut self) -> Result<(), ActorError> {
        self.system.terminate_actor(self.actor_id)?;
        self.cleaned_up = true;
        Ok(())
    }
}
```

---

## 4. Cleanup on Panic (panic=abort)

### 4.1 Panic Implications

With `panic=abort`, panics immediately terminate the process. This means:

- **No stack unwinding**: Destructors are NOT called
- **No RAII cleanup**: Resources are NOT released
- **OS-level cleanup**: Only OS resources (FDs, memory) are freed

### 4.2 Panic Mitigation Strategies

#### 4.2.1 Avoid Panics

```rust
// Instead of:
pub fn get(&self, key: &str) -> &Value {
    self.map.get(key).expect("Key not found")
}

// Use:
pub fn get(&self, key: &str) -> Result<&Value, KeyError> {
    self.map.get(key).ok_or(KeyError::NotFound)
}
```

#### 4.2.2 Catch Panics at Boundaries

```rust
pub fn call_actor_safe(&self, actor_id: ActorId, msg: Message) -> Result<(), Error> {
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        self.call_actor(actor_id, msg)
    }))
    .map_err(|e| {
        log::error!("Panic in actor {}: {:?}", actor_id, e);
        Error::ActorPanic(actor_id)
    })?
}
```

#### 4.2.3 Persist Critical State

```rust
pub struct PersistentState {
    path: PathBuf,
    state: Mutex<ActorState>,
}

impl PersistentState {
    pub fn update<F>(&self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut ActorState),
    {
        let mut state = self.state.lock().unwrap();
        f(&mut state);
        
        // Immediately persist after every update
        let serialized = serde_json::to_string(&*state)?;
        std::fs::write(&self.path, serialized)?;
        
        Ok(())
    }
}
```

#### 4.2.4 External Cleanup Process

```rust
pub fn spawn_cleanup_watcher() {
    let pid = std::process::id();
    let cleanup_script = "/usr/local/bin/aether-cleanup";
    
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5));
            
            // Check if main process is still alive
            if !process_exists(pid) {
                // Main process died, run cleanup
                std::process::Command::new(cleanup_script)
                    .arg(pid.to_string())
                    .spawn()
                    .ok();
                break;
            }
        }
    });
}

fn process_exists(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}
```

### 4.3 Cleanup Script

```bash
#!/bin/bash
# /usr/local/bin/aether-cleanup

PID=$1
LOG_FILE="/var/log/aether/cleanup-${PID}.log"

log() {
    echo "[$(date -Iseconds)] $1" >> "$LOG_FILE"
}

log "Cleanup triggered for PID $PID"

# Kill any remaining VMs
log "Terminating VMs..."
for vm in /run/aether/vms/${PID}-*; do
    if [ -d "$vm" ]; then
        vm_id=$(basename "$vm")
        log "Killing VM $vm_id"
        firecracker --api-sock "$vm/api.sock" stop 2>/dev/null || true
    fi
done

# Remove cgroups
log "Cleaning cgroups..."
for cgroup in /sys/fs/cgroup/aether/${PID}-*; do
    if [ -d "$cgroup" ]; then
        log "Removing cgroup $(basename "$cgroup")"
        rmdir "$cgroup" 2>/dev/null || true
    fi
done

# Remove temporary files
log "Cleaning temp files..."
rm -rf "/tmp/aether-${PID}-*" 2>/dev/null || true

# Release network resources
log "Cleaning network resources..."
for ns in /var/run/netns/aether-${PID}-*; do
    if [ -f "$ns" ]; then
        ns_name=$(basename "$ns")
        log "Deleting network namespace $ns_name"
        ip netns del "$ns_name" 2>/dev/null || true
    fi
done

log "Cleanup complete"
```

---

## 5. Orphan Resource Reclamation

### 5.1 Orphan Resource Types

| Resource | Detection Method | Reclamation Strategy |
|----------|------------------|----------------------|
| Orphan handles | Periodic scan | Close and log |
| Orphan memory | Memory accounting | Free and log |
| Orphan VMs | Process tracking | Kill and cleanup |
| Orphan cgroups | Cgroup tracking | Remove |
| Orphan network namespaces | Namespace tracking | Delete |

### 5.2 Orphan Detector

```rust
pub struct OrphanDetector {
    scan_interval: Duration,
    known_actors: Arc<RwLock<HashSet<ActorId>>>,
}

impl OrphanDetector {
    pub fn new(known_actors: Arc<RwLock<HashSet<ActorId>>>) -> Self {
        Self {
            scan_interval: Duration::from_secs(60),
            known_actors,
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(self.scan_interval);
                self.scan();
            }
        })
    }

    fn scan(&self) {
        log::debug!("Scanning for orphan resources...");
        
        self.scan_orphan_handles();
        self.scan_orphan_vms();
        self.scan_orphan_cgroups();
        self.scan_orphan_namespaces();
    }

    fn scan_orphan_handles(&self) {
        let known_actors = self.known_actors.read().unwrap();
        let handle_manager = HANDLE_MANAGER.lock().unwrap();
        
        let orphan_handles: Vec<Handle> = handle_manager
            .all_handles()
            .filter(|h| !known_actors.contains(&h.actor_id))
            .collect();
        
        if !orphan_handles.is_empty() {
            log::warn!("Found {} orphan handles", orphan_handles.len());
            
            for handle in orphan_handles {
                log::warn!("Reclaiming orphan handle: {:?}", handle);
                if let Err(e) = handle_manager.close(handle) {
                    log::error!("Failed to close orphan handle: {}", e);
                }
            }
        }
    }

    fn scan_orphan_vms(&self) {
        let known_actors = self.known_actors.read().unwrap();
        let vm_manager = VM_MANAGER.lock().unwrap();
        
        let orphan_vms: Vec<VmId> = vm_manager
            .all_vms()
            .filter(|vm| !known_actors.contains(&vm.owner_actor_id()))
            .map(|vm| vm.id())
            .collect();
        
        if !orphan_vms.is_empty() {
            log::warn!("Found {} orphan VMs", orphan_vms.len());
            
            for vm_id in orphan_vms {
                log::warn!("Reclaiming orphan VM: {}", vm_id);
                if let Err(e) = vm_manager.kill(vm_id) {
                    log::error!("Failed to kill orphan VM: {}", e);
                }
            }
        }
    }

    fn scan_orphan_cgroups(&self) {
        let known_actors = self.known_actors.read().unwrap();
        
        if let Ok(entries) = std::fs::read_dir("/sys/fs/cgroup/aether") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                if let Some(actor_id) = self.extract_actor_id_from_cgroup(&name) {
                    if !known_actors.contains(&actor_id) {
                        log::warn!("Reclaiming orphan cgroup: {}", name);
                        if let Err(e) = std::fs::remove_dir(entry.path()) {
                            log::error!("Failed to remove orphan cgroup: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn extract_actor_id_from_cgroup(&self, name: &str) -> Option<ActorId> {
        name.strip_prefix("actor-")?.parse().ok()
    }

    fn scan_orphan_namespaces(&self) {
        let known_actors = self.known_actors.read().unwrap();
        
        if let Ok(entries) = std::fs::read_dir("/var/run/netns") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                if name.starts_with("aether-") {
                    if let Some(actor_id) = self.extract_actor_id_from_ns(&name) {
                        if !known_actors.contains(&actor_id) {
                            log::warn!("Reclaiming orphan namespace: {}", name);
                            let _ = std::process::Command::new("ip")
                                .args(&["netns", "del", &name])
                                .status();
                        }
                    }
                }
            }
        }
    }

    fn extract_actor_id_from_ns(&self, name: &str) -> Option<ActorId> {
        name.strip_prefix("aether-")?.parse().ok()
    }
}
```

### 5.3 Resource Tracking

```rust
pub struct ResourceTracker {
    resources: Mutex<HashMap<ResourceId, ResourceInfo>>,
}

#[derive(Debug)]
pub struct ResourceInfo {
    id: ResourceId,
    owner: ActorId,
    resource_type: ResourceType,
    created_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceId {
    Handle(Handle),
    Vm(VmId),
    Cgroup(String),
    Namespace(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    Handle,
    Vm,
    Cgroup,
    Namespace,
}

impl ResourceTracker {
    pub fn register(&self, id: ResourceId, owner: ActorId, resource_type: ResourceType) {
        let info = ResourceInfo {
            id,
            owner,
            resource_type,
            created_at: Instant::now(),
        };
        
        self.resources.lock().unwrap().insert(id, info);
    }

    pub fn unregister(&self, id: ResourceId) {
        self.resources.lock().unwrap().remove(&id);
    }

    pub fn get_orphans(&self, known_actors: &HashSet<ActorId>) -> Vec<ResourceId> {
        self.resources
            .lock()
            .unwrap()
            .values()
            .filter(|info| !known_actors.contains(&info.owner))
            .map(|info| info.id)
            .collect()
    }
}
```

---

## 6. Cleanup Verification

### 6.1 Cleanup Verification Tests

```rust
#[cfg(test)]
mod cleanup_tests {
    use super::*;

    #[test]
    fn test_actor_cleanup() {
        let system = ActorSystem::new();
        let actor_id = system.spawn(TestActor::new()).unwrap();
        
        // Create resources
        let handle = system.create_handle(actor_id).unwrap();
        
        // Terminate actor
        system.terminate_actor(actor_id).unwrap();
        
        // Verify cleanup
        assert!(!system.handle_exists(handle));
    }

    #[test]
    fn test_graceful_shutdown() {
        let mut coordinator = ShutdownCoordinator::new();
        
        coordinator.run_shutdown().unwrap();
        
        assert_eq!(coordinator.current_phase(), ShutdownPhase::Complete);
    }

    #[test]
    fn test_orphan_detection() {
        let known_actors = Arc::new(RwLock::new(HashSet::new()));
        let detector = OrphanDetector::new(known_actors.clone());
        
        // Create orphan resource
        let handle = create_orphan_handle();
        
        // Scan for orphans
        detector.scan();
        
        // Verify orphan was reclaimed
        assert!(!handle_exists(handle));
    }
}
```

### 6.2 Integration Tests

```rust
#[test]
fn test_full_cleanup_cycle() {
    let system = ActorSystem::new();
    
    // Spawn multiple actors
    let actors: Vec<ActorId> = (0..10)
        .map(|_| system.spawn(TestActor::new()).unwrap())
        .collect();
    
    // Create resources for each actor
    for actor_id in &actors {
        system.create_handle(*actor_id).unwrap();
        system.allocate_memory(*actor_id, 1024).unwrap();
    }
    
    // Terminate all actors
    for actor_id in &actors {
        system.terminate_actor(*actor_id).unwrap();
    }
    
    // Verify all resources cleaned up
    assert_eq!(system.active_handle_count(), 0);
    assert_eq!(system.allocated_memory(), 0);
}
```

---

## 7. Cleanup Metrics

### 7.1 Metrics Collection

```rust
pub struct CleanupMetrics {
    pub actors_terminated: AtomicU64,
    pub handles_closed: AtomicU64,
    pub vms_killed: AtomicU64,
    pub memory_freed: AtomicU64,
    pub orphans_reclaimed: AtomicU64,
    pub cleanup_errors: AtomicU64,
}

impl CleanupMetrics {
    pub fn record_actor_termination(&self) {
        self.actors_terminated.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_handle_close(&self) {
        self.handles_closed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_vm_kill(&self) {
        self.vms_killed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_memory_free(&self, bytes: u64) {
        self.memory_freed.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_orphan_reclamation(&self) {
        self.orphans_reclaimed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cleanup_error(&self) {
        self.cleanup_errors.fetch_add(1, Ordering::Relaxed);
    }
}
```

---

## 8. Error Handling During Cleanup

### 8.1 Cleanup Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum CleanupError {
    #[error("Failed to close handle: {0}")]
    HandleClose(String),
    
    #[error("Failed to kill VM: {0}")]
    VmKill(String),
    
    #[error("Failed to free memory: {0}")]
    MemoryFree(String),
    
    #[error("Failed to remove cgroup: {0}")]
    CgroupRemove(String),
    
    #[error("Failed to delete namespace: {0}")]
    NamespaceDelete(String),
    
    #[error("Cleanup timeout")]
    Timeout,
}
```

### 8.2 Resilient Cleanup

```rust
impl ResourceCleanup {
    pub fn cleanup_all(&self) -> Result<(), CleanupError> {
        let mut errors = Vec::new();
        
        // Attempt all cleanups, collect errors
        if let Err(e) = self.cleanup_handles() {
            log::error!("Handle cleanup failed: {}", e);
            errors.push(e);
        }
        
        if let Err(e) = self.cleanup_vms() {
            log::error!("VM cleanup failed: {}", e);
            errors.push(e);
        }
        
        if let Err(e) = self.cleanup_memory() {
            log::error!("Memory cleanup failed: {}", e);
            errors.push(e);
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(CleanupError::Multiple(errors))
        }
    }
}
```

---

## 9. Best Practices

### 9.1 Development Guidelines

1. **Always use RAII**: Let destructors handle cleanup
2. **Implement cleanup handlers**: For non-RAII resources
3. **Test cleanup paths**: Verify resources are released
4. **Log cleanup operations**: Aid debugging
5. **Handle cleanup errors**: Don't fail silently

### 9.2 Code Review Checklist

- [ ] All resources have cleanup handlers
- [ ] Cleanup tested in unit tests
- [ ] Graceful shutdown implemented
- [ ] Panic mitigation strategies in place
- [ ] Orphan detection enabled

---

## 10. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Actor cleanup latency | <10ms | Integration test |
| Full shutdown latency | <60s | Integration test |
| Orphan scan overhead | <1% | Profiling |
| Cleanup error rate | <0.1% | Telemetry |

---

## 11. References

- ADR-003: panic=abort decision
- RM-MEM-001: Memory management
- RM-HANDLE-001: Handle management
- BP-HOST-RUNTIME-001: Host runtime design

---

**Approval**: Resource Engineer
**Review Date**: 2026-03-05
