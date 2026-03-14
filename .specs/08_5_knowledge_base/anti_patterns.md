# Aether Anti-Patterns

**Version:** 1.0.0  
**Generated:** 2026-03-06  
**Total Anti-Patterns:** 15

---

## Overview

This document catalogs anti-patterns identified during the R&D phase that must be avoided in the implementation. Each anti-pattern includes detection methods, consequences, and proper alternatives.

---

## 1. unwrap/expect in Hot Path

### Description

Using `.unwrap()` or `.expect()` in performance-critical code paths creates hidden panics that bypass error handling.

### Example (Anti-Pattern)

```rust
pub async fn process_message(&self, msg: Message) -> Response {
    let parsed: Request = serde_json::from_str(&msg.data).unwrap(); // PANIC RISK
    let result = self.handler.handle(parsed).await.unwrap(); // PANIC RISK
    serde_json::to_string(&result).unwrap() // PANIC RISK
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Actor crash on malformed input | Critical |
| Denial of service vector | Critical |
| No graceful degradation | High |
| Debugging difficulty | Medium |

### Detection

```bash
# Static analysis
rg "\.unwrap\(\)" --type rust core/src/
rg "\.expect\(" --type rust core/src/

# Runtime detection
RUST_BACKTRACE=1 RUST_LIB_BACKTRACE=1 cargo test
```

### Proper Alternative

```rust
pub async fn process_message(&self, msg: Message) -> Result<Response, ActorError> {
    let parsed: Request = serde_json::from_str(&msg.data)
        .map_err(|e| ActorError::ParseError(e))?;
    
    let result = self.handler.handle(parsed).await
        .map_err(|e| ActorError::HandlerError(e))?;
    
    let response = serde_json::to_string(&result)
        .map_err(|e| ActorError::SerializeError(e))?;
    
    Ok(response)
}
```

### Enforcement

```rust
// Add to CI pipeline
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
```

---

## 2. Dynamic Allocation in Hot Path

### Description

Heap allocations (Box, Vec, String) in hot paths cause cache misses and non-deterministic latency.

### Example (Anti-Pattern)

```rust
pub fn handle_packet(&self, data: &[u8]) -> Vec<u8> { // HEAP ALLOCATION
    let mut response = Vec::with_capacity(1024); // HEAP ALLOCATION
    response.extend_from_slice(data);
    response
}

pub fn parse_header(&self, data: &[u8]) -> String { // HEAP ALLOCATION
    std::str::from_utf8(&data[0..10]).unwrap().to_string() // HEAP ALLOCATION
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Cache misses | High |
| Non-deterministic latency | Critical |
| Memory fragmentation | Medium |
| GC pressure (if applicable) | Medium |

### Detection

```bash
# Profiling
valgrind --tool=massif ./target/release/aether-daemon

# Heap tracking
MALLOC_CONF="prof:true,prof_prefix:jeprof.out" ./target/release/aether-daemon
```

### Proper Alternative

```rust
pub fn handle_packet<'a>(&self, data: &[u8], output: &'a mut [u8]) -> &'a [u8] {
    let len = data.len().min(output.len());
    output[0..len].copy_from_slice(&data[0..len]);
    &output[0..len]
}

pub fn parse_header<'a>(&self, data: &'a [u8]) -> &'a str {
    std::str::from_utf8(&data[0..10]).unwrap_or("")
}
```

### Guidelines

| Context | Allocation Policy |
|---------|-------------------|
| WASM invocation | Zero allocation |
| Mesh message parsing | Zero allocation |
| Actor scheduling | Minimal allocation |
| Configuration | Allocation allowed |

---

## 3. Mutex in Data Plane

### Description

Using `std::sync::Mutex` or `tokio::sync::Mutex` in high-throughput data paths causes contention and latency spikes.

### Example (Anti-Pattern)

```rust
pub struct ActorRegistry {
    actors: Arc<Mutex<HashMap<ActorId, ActorHandle>>>, // CONTENTION POINT
}

impl ActorRegistry {
    pub async fn route(&self, msg: Message) -> Result<(), Error> {
        let actors = self.actors.lock().await; // BLOCKS ALL ROUTING
        let actor = actors.get(&msg.to).ok_or(Error::NotFound)?;
        actor.send(msg).await
    }
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Lock contention | Critical |
| Priority inversion | High |
| Latency spikes | Critical |
| Throughput degradation | High |

### Detection

```bash
# Lock analysis
perf record -e lock:contention_begin ./target/release/aether-daemon

# Tracing
tokio-console
```

### Proper Alternative

```rust
// Option 1: DashMap (sharded concurrent map)
pub struct ActorRegistry {
    actors: DashMap<ActorId, ActorHandle>,
}

impl ActorRegistry {
    pub async fn route(&self, msg: Message) -> Result<(), Error> {
        let actor = self.actors.get(&msg.to).ok_or(Error::NotFound)?;
        actor.send(msg).await
    }
}

// Option 2: Actor-per-shard
pub struct ShardedRegistry {
    shards: Vec<ActorRegistry>, // Each shard is single-threaded
}

impl ShardedRegistry {
    fn shard(&self, id: &ActorId) -> &ActorRegistry {
        &self.shards[id.hash() % self.shards.len()]
    }
}

// Option 3: RCU (Read-Copy-Update)
pub struct ActorRegistry {
    actors: Arc<RwLock<Arc<HashMap<ActorId, ActorHandle>>>>,
}
```

### Guidelines

| Context | Lock Type |
|---------|-----------|
| Data plane (hot) | Lock-free or sharded |
| Control plane | Mutex acceptable |
| Read-heavy | RwLock |
| Write-heavy | DashMap |

---

## 4. Blocking I/O in Async Context

### Description

Calling blocking I/O operations (std::fs, std::net) from async code blocks the entire executor thread.

### Example (Anti-Pattern)

```rust
pub async fn load_module(&self, path: &Path) -> Result<Module, Error> {
    let bytes = std::fs::read(path)?; // BLOCKS EXECUTOR
    self.compile(&bytes).await
}

pub async fn send_response(&self, response: Response) -> Result<(), Error> {
    let socket = std::net::TcpStream::connect(self.addr)?; // BLOCKS EXECUTOR
    // ...
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Executor thread blocked | Critical |
| Other tasks starved | Critical |
| Latency spikes | High |
| Throughput collapse | Critical |

### Detection

```bash
# Tokio console shows blocking
tokio-console

# Tracing
tokio::task::spawn_blocking instrumentation
```

### Proper Alternative

```rust
pub async fn load_module(&self, path: &Path) -> Result<Module, Error> {
    let path = path.to_owned();
    let bytes = tokio::task::spawn_blocking(move || {
        std::fs::read(path)
    }).await??;
    
    self.compile(&bytes).await
}

pub async fn send_response(&self, response: Response) -> Result<(), Error> {
    tokio::net::TcpStream::connect(self.addr).await?;
    // ...
}
```

### Guidelines

| Operation | Correct Approach |
|-----------|-----------------|
| File I/O | tokio::fs or spawn_blocking |
| Network I/O | tokio::net |
| CPU-intensive | spawn_blocking |
| Sleep | tokio::time::sleep |

---

## 5. Clone in Hot Path

### Description

Excessive cloning of large data structures in performance-critical code causes allocation pressure.

### Example (Anti-Pattern)

```rust
pub fn process_batch(&self, messages: Vec<Message>) -> Vec<Response> {
    messages.iter().map(|msg| {
        let config = self.config.clone(); // CLONES LARGE CONFIG
        self.handle_with_config(msg, &config)
    }).collect()
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Allocation overhead | High |
| Cache pollution | Medium |
| Copy cost | High |

### Detection

```bash
# Clippy
cargo clippy -- -W clippy::clone_on_copy

# Profiling
cargo flamegraph
```

### Proper Alternative

```rust
pub fn process_batch(&self, messages: Vec<Message>) -> Vec<Response> {
    messages.iter().map(|msg| {
        self.handle_with_config(msg, &self.config) // Borrow, don't clone
    }).collect()
}

// Or use Arc for shared ownership
pub struct Handler {
    config: Arc<Config>, // Cheap to clone
}
```

---

## 6. God Object

### Description

Creating a single struct or module that handles too many responsibilities.

### Example (Anti-Pattern)

```rust
pub struct Runtime {
    // Everything in one struct
    actors: HashMap<ActorId, Actor>,
    vms: HashMap<VmId, Vm>,
    network: NetworkStack,
    storage: StorageEngine,
    config: Config,
    metrics: Metrics,
    logs: Logger,
    security: SecurityContext,
    // ... 50 more fields
}

impl Runtime {
    pub fn handle_everything(&mut self, request: Request) -> Response {
        // 1000+ line function
    }
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Untestable code | Critical |
| Merge conflicts | High |
| Cognitive overload | High |
| Coupling | Critical |

### Detection

```bash
# Line count
wc -l src/runtime.rs

# Complexity
cargo clippy -- -W clippy::cognitive_complexity
```

### Proper Alternative

```rust
pub struct Runtime {
    engine: Arc<WasmEngine>,
    vm_manager: Arc<VmManager>,
    mesh: Arc<MeshNetwork>,
    state: Arc<StateManager>,
}

// Each component has single responsibility
pub struct WasmEngine { /* ... */ }
pub struct VmManager { /* ... */ }
pub struct MeshNetwork { /* ... */ }
pub struct StateManager { /* ... */ }
```

---

## 7. Premature Optimization

### Description

Optimizing code before measuring and understanding actual bottlenecks.

### Example (Anti-Pattern)

```rust
// Complex "optimized" code that doesn't help
pub fn parse_byte(b: u8) -> u32 {
    // Hand-rolled lookup table for simple parsing
    static TABLE: [u32; 256] = [/* ... */];
    unsafe { *TABLE.get_unchecked(b as usize) }
}

// Simple version is actually faster due to CPU optimizations
pub fn parse_byte_simple(b: u8) -> u32 {
    b as u32
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Wasted time | Medium |
| Bug-prone code | High |
| Harder to maintain | High |
| May be slower | Medium |

### Detection

```bash
# Always benchmark before and after
cargo bench -- baseline
cargo bench -- optimized
```

### Proper Alternative

```rust
// 1. Write clear code first
pub fn parse_byte(b: u8) -> u32 {
    b as u32
}

// 2. Profile to find actual bottlenecks
// cargo flamegraph

// 3. Optimize only proven hotspots
// 4. Verify with benchmarks
```

---

## 8. Magic Numbers

### Description

Using unexplained numeric constants without documentation.

### Example (Anti-Pattern)

```rust
pub fn allocate_buffer(&self) -> Vec<u8> {
    Vec::with_capacity(65536) // Why 65536?
}

pub fn timeout(&self) -> Duration {
    Duration::from_millis(30000) // Why 30000?
}

if response.status == 429 { // Why 429?
    self.backoff(2.0); // Why 2.0?
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Unclear intent | Medium |
| Maintenance burden | Medium |
| Bugs from changes | Medium |

### Detection

```bash
# Find numeric literals
rg '\b\d+\b' --type rust
```

### Proper Alternative

```rust
const BUFFER_SIZE: usize = 64 * 1024; // 64 KiB - matches page size
const DEFAULT_TIMEOUT_MS: u64 = 30_000; // 30 seconds - AWS Lambda limit
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
const BACKOFF_MULTIPLIER: f64 = 2.0; // Exponential backoff

pub fn allocate_buffer(&self) -> Vec<u8> {
    Vec::with_capacity(BUFFER_SIZE)
}
```

---

## 9. Error Swallowing

### Description

Ignoring or silently discarding errors without proper handling or logging.

### Example (Anti-Pattern)

```rust
pub async fn process(&self, msg: Message) {
    let _ = self.send_to_downstream(msg).await; // Error ignored
    self.cache.invalidate().ok(); // Error ignored
}

pub fn parse_config(&self, data: &[u8]) -> Option<Config> {
    Some(serde_json::from_slice(data).ok()?) // Error details lost
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Silent failures | Critical |
| Debugging nightmare | Critical |
| Data loss | High |

### Detection

```bash
# Find error ignoring
rg 'let _ = ' --type rust
rg '\.ok\(\)' --type rust
```

### Proper Alternative

```rust
pub async fn process(&self, msg: Message) -> Result<(), ProcessError> {
    if let Err(e) = self.send_to_downstream(msg.clone()).await {
        log::warn!("Downstream send failed: {}", e);
        metrics::counter!("process_downstream_errors").increment(1);
    }
    
    self.cache.invalidate().await
        .map_err(|e| ProcessError::CacheError(e))?;
    
    Ok(())
}
```

---

## 10. Global Mutable State

### Description

Using global static mutable variables that break testability and cause race conditions.

### Example (Anti-Pattern)

```rust
static mut CONFIG: Option<Config> = None; // DANGEROUS

pub fn get_config() -> &'static Config {
    unsafe { CONFIG.as_ref().unwrap() }
}

lazy_static! {
    static ref METRICS: Mutex<Metrics> = Mutex::new(Metrics::new());
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Race conditions | Critical |
| Untestable | Critical |
| Hidden dependencies | High |

### Detection

```bash
# Find global state
rg 'static mut' --type rust
rg 'lazy_static!' --type rust
rg 'once_cell::sync::Lazy' --type rust
```

### Proper Alternative

```rust
// Dependency injection
pub struct Runtime {
    config: Arc<Config>,
    metrics: Arc<Metrics>,
}

impl Runtime {
    pub fn new(config: Arc<Config>, metrics: Arc<Metrics>) -> Self {
        Self { config, metrics }
    }
}

// Or use context pattern
pub struct Context {
    config: Arc<Config>,
    metrics: Arc<Metrics>,
}
```

---

## 11. Stringly Typed APIs

### Description

Using strings where structured types would be more appropriate.

### Example (Anti-Pattern)

```rust
pub fn invoke(&self, actor: &str, method: &str, args: &str) -> Result<String, String> {
    // Everything is strings
}

pub fn route(&self, address: &str) -> Result<(), String> {
    let parts: Vec<&str> = address.split(':').collect();
    // Manual parsing
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Runtime errors | High |
| No type safety | High |
| Poor documentation | Medium |

### Detection

```bash
# Find stringly typed parameters
rg 'fn.*&str.*-> Result<String' --type rust
```

### Proper Alternative

```rust
#[derive(Debug, Clone)]
pub struct ActorId(Uuid);

#[derive(Debug, Clone)]
pub struct MethodName(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    pub actor: ActorId,
    pub method: MethodName,
    pub args: serde_json::Value,
}

pub fn invoke(&self, request: InvokeRequest) -> Result<InvokeResponse, InvokeError> {
    // Type-safe throughout
}
```

---

## 12. Deeply Nested Code

### Description

Excessive nesting levels that make code hard to read and maintain.

### Example (Anti-Pattern)

```rust
pub async fn process(&self, msg: Message) -> Result<Response, Error> {
    if let Some(actor) = self.registry.get(&msg.to) {
        if actor.is_active() {
            if let Some(cap) = actor.capabilities().get(&msg.capability) {
                if cap.allows(&msg.action) {
                    if let Some(state) = actor.state() {
                        // 5 levels deep, still going
                        if state.is_ready() {
                            // ...
                        }
                    }
                }
            }
        }
    }
    Ok(Response::default())
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Cognitive load | High |
| Bug hiding | High |
| Hard to test | Medium |

### Detection

```bash
# Complexity analysis
cargo clippy -- -W clippy::cognitive_complexity
```

### Proper Alternative

```rust
pub async fn process(&self, msg: Message) -> Result<Response, Error> {
    let actor = self.registry.get(&msg.to)
        .ok_or(Error::ActorNotFound)?;
    
    if !actor.is_active() {
        return Err(Error::ActorInactive);
    }
    
    let cap = actor.capabilities().get(&msg.capability)
        .ok_or(Error::CapabilityNotFound)?;
    
    if !cap.allows(&msg.action) {
        return Err(Error::PermissionDenied);
    }
    
    let state = actor.state().ok_or(Error::StateNotReady)?;
    
    // Main logic at low nesting level
    self.execute(actor, state, msg).await
}
```

---

## 13. Synchronous Channel Misuse

### Description

Using bounded channels without backpressure handling, leading to deadlocks or memory exhaustion.

### Example (Anti-Pattern)

```rust
pub struct ActorSystem {
    sender: mpsc::Sender<Message>, // Unbounded!
}

// Or bounded without handling
let (tx, rx) = mpsc::channel(100);
tx.send(msg).await.unwrap(); // Will block or panic when full
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Deadlock | Critical |
| Memory exhaustion | Critical |
| Unbounded growth | High |

### Detection

```bash
# Find unbounded channels
rg 'mpsc::channel\(\)' --type rust
rg 'mpsc::unbounded' --type rust
```

### Proper Alternative

```rust
pub struct ActorSystem {
    sender: mpsc::Sender<Message>,
}

impl ActorSystem {
    pub async fn send(&self, msg: Message) -> Result<(), SendError> {
        match self.sender.try_send(msg) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(msg)) => {
                // Apply backpressure
                metrics::counter!("channel_full").increment(1);
                Err(SendError::ChannelFull)
            }
            Err(mpsc::error::TrySendError::Closed(msg)) => {
                Err(SendError::ChannelClosed)
            }
        }
    }
}
```

---

## 14. Copy-Paste Error Handling

### Description

Duplicating error handling code instead of centralizing it.

### Example (Anti-Pattern)

```rust
// Repeated in 20 places
pub async fn operation1(&self) -> Result<(), Error> {
    match self.client.call().await {
        Ok(r) => Ok(r),
        Err(e) => {
            log::error!("Failed: {}", e);
            metrics::counter!("errors").increment(1);
            Err(Error::from(e))
        }
    }
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| DRY violation | Medium |
| Inconsistent handling | High |
| Maintenance burden | Medium |

### Detection

```bash
# Find similar error patterns
rg 'log::error!' --type rust -C 3
```

### Proper Alternative

```rust
pub trait ResultExt<T, E> {
    fn log_and_metric(self, context: &str) -> Result<T, Error>;
}

impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn log_and_metric(self, context: &str) -> Result<T, Error> {
        self.map_err(|e| {
            log::error!("{} failed: {}", context, e);
            metrics::counter!("errors", "context" => context).increment(1);
            Error::from(e)
        })
    }
}

// Usage
pub async fn operation1(&self) -> Result<(), Error> {
    self.client.call().await
        .log_and_metric("operation1")
}
```

---

## 15. Testing Implementation Details

### Description

Writing tests that depend on internal implementation rather than public behavior.

### Example (Anti-Pattern)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_internal_field() {
        let actor = Actor::new();
        assert_eq!(actor.internal_counter, 0); // Tests implementation
        actor.increment();
        assert_eq!(actor.internal_counter, 1); // Breaks on refactor
    }
}
```

### Consequences

| Consequence | Severity |
|-------------|----------|
| Fragile tests | High |
| Refactor resistance | High |
| False confidence | Medium |

### Detection

```bash
# Find tests accessing private fields
rg '#\[test\]' -A 20 --type rust | grep '\.\w+_\w+'
```

### Proper Alternative

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_behavior() {
        let actor = Actor::new();
        assert_eq!(actor.count(), 0); // Tests behavior
        actor.increment();
        assert_eq!(actor.count(), 1); // Stable across refactors
    }
}
```

---

## Anti-Pattern Detection Checklist

### CI Pipeline Integration

```yaml
# .github/workflows/anti-patterns.yml
- name: Check anti-patterns
  run: |
    # unwrap/expect check
    cargo clippy -- -D clippy::unwrap_used -D clippy::expect_used
    
    # Complexity check
    cargo clippy -- -W clippy::cognitive_complexity
    
    # Cognitive complexity threshold
    cargo complexity --max-complexity 20
    
    # Dead code
    cargo clippy -- -W clippy::dead_code
    
    # Documentation
    cargo doc --no-deps
```

### Code Review Checklist

- [ ] No unwrap/expect in hot paths
- [ ] No blocking I/O in async
- [ ] No Mutex in data plane
- [ ] No dynamic allocation in hot paths
- [ ] No global mutable state
- [ ] No magic numbers
- [ ] No error swallowing
- [ ] No deeply nested code (>4 levels)
- [ ] No copy-paste error handling
- [ ] Tests verify behavior, not implementation

---

## Severity Classification

| Severity | Definition | Action |
|----------|------------|--------|
| Critical | Will cause production incidents | Block merge |
| High | Significant impact on reliability/performance | Fix before release |
| Medium | Maintainability concern | Fix in next sprint |
| Low | Minor code quality issue | Fix opportunistically |

---

## Related Resources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Effective Rust](https://www.lurklurk.org/effective-rust/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
