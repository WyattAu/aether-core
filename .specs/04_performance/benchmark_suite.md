# Benchmark Suite Design
**Project Aether - Phase 4: Performance Engineering**

## Document Control
- **Version**: 1.0
- **Status**: Approved
- **Created**: 2026-03-05
- **Last Updated**: 2026-03-05
- **Author**: Performance Engineering Team
- **Review Status**: Complete

## 1. Overview

This document defines the comprehensive benchmark suite for validating Project Aether performance requirements. The suite covers micro-benchmarks, integration benchmarks, end-to-end scenarios, load testing, and stress testing.

## 2. Benchmark Framework Architecture

### 2.1 Framework Components

```
benchmark_suite/
├── core/
│   ├── harness.rs           # Benchmark execution harness
│   ├── metrics.rs           # Metrics collection and aggregation
│   ├── reporter.rs          # Results reporting (JSON, HTML, console)
│   ├── isolator.rs          # CPU/memory isolation for benchmarks
│   └── validator.rs         # SLO validation logic
├── micro/
│   ├── wasm/                # WASM micro-benchmarks
│   ├── vm/                  # VM micro-benchmarks
│   ├── network/             # Network micro-benchmarks
│   └── state/               # State management micro-benchmarks
├── integration/
│   ├── actor_to_actor/      # Actor communication benchmarks
│   ├── state_access/        # State access patterns
│   ├── capability/          # Capability system benchmarks
│   └── lifecycle/           # Actor lifecycle benchmarks
├── e2e/
│   ├── simple_request/      # Simple request processing
│   ├── stateful_request/    # Stateful request processing
│   ├── vm_isolation/        # VM-isolated request processing
│   └── distributed/         # Multi-node scenarios
├── load/
│   ├── steady_state/        # Sustained load testing
│   ├── burst/               # Burst traffic testing
│   ├── ramp/                # Ramp-up/ramp-down testing
│   └── mixed/               # Mixed workload testing
├── stress/
│   ├── resource_exhaustion/ # Push to resource limits
│   ├── chaos/               # Fault injection
│   ├── endurance/           # Long-running stability
│   └── boundary/            # Edge case stress
└── fixtures/
    ├── wasm_modules/        # Pre-compiled WASM modules
    ├── vm_images/           # VM disk images
    ├── test_data/           # Test datasets
    └── configurations/      # Benchmark configurations
```

### 2.2 Harness Design

```rust
pub struct BenchmarkHarness {
    config: BenchmarkConfig,
    isolator: Isolator,
    metrics: MetricsCollector,
    warmup_iterations: u64,
    measurement_iterations: u64,
}

pub struct BenchmarkConfig {
    pub name: String,
    pub category: BenchmarkCategory,
    pub isolation: IsolationLevel,
    pub sampling: SamplingStrategy,
    pub timeout: Duration,
    pub retries: u32,
}

pub enum IsolationLevel {
    None,
    Process,
    CpuSet(Vec<usize>),
    MemoryNode(usize),
    FullContainer,
}

pub enum SamplingStrategy {
    FixedIterations(u64),
    FixedDuration(Duration),
    Adaptive {
        min_iterations: u64,
        max_iterations: u64,
        target_relative_error: f64,
    },
}
```

### 2.3 Metrics Collection

```rust
pub struct BenchmarkMetrics {
    pub name: String,
    pub timestamp: DateTime<Utc>,
    pub iterations: u64,
    pub duration: Duration,
    
    pub latency: LatencyDistribution,
    pub throughput: ThroughputStats,
    pub resources: ResourceStats,
    pub errors: ErrorStats,
}

pub struct LatencyDistribution {
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub std_dev: Duration,
    pub percentiles: HashMap<Percentile, Duration>,
}

pub struct ThroughputStats {
    pub ops_per_sec: f64,
    pub bytes_per_sec: Option<f64>,
    pub msg_per_sec: Option<f64>,
}

pub struct ResourceStats {
    pub cpu_time: Duration,
    pub cpu_cycles: Option<u64>,
    pub cache_misses: Option<u64>,
    pub memory_allocated: u64,
    pub memory_peak: u64,
    pub memory_freed: u64,
    pub page_faults: u64,
    pub context_switches: u64,
}
```

## 3. Micro-Benchmarks

### 3.1 WebAssembly Micro-Benchmarks

#### 3.1.1 Cold Start Benchmark

```rust
#[benchmark(
    name = "wasm_cold_start",
    category = "micro",
    warmup = 100,
    iterations = 10_000,
    isolation = "process"
)]
pub fn bench_wasm_cold_start() {
    let module = compile_wat(COUNTER_WAT);
    let store = Store::new(&engine);
    
    let start = Instant::now();
    let instance = Instance::new(&store, &module, &[]);
    let elapsed = start.elapsed();
    
    metrics.record_latency(elapsed);
}
```

**Parameters:**
- Module size: 1KB, 10KB, 100KB, 1MB
- Memory pages: 1, 10, 100, 1000
- Imports: 0, 10, 100
- Exports: 1, 10, 100

**Targets:**
- P50 < 20µs, P95 < 35µs, P99 < 45µs, P999 < 50µs

#### 3.1.2 Warm Start Benchmark

```rust
#[benchmark(
    name = "wasm_warm_start",
    category = "micro",
    warmup = 1000,
    iterations = 100_000
)]
pub fn bench_wasm_warm_start(cached_module: &CachedModule) {
    let start = Instant::now();
    let instance = cached_module.instantiate();
    let elapsed = start.elapsed();
    
    metrics.record_latency(elapsed);
}
```

**Targets:**
- P50 < 2µs, P95 < 5µs, P99 < 8µs, P999 < 10µs

#### 3.1.3 Function Invocation Benchmark

```rust
#[benchmark(
    name = "wasm_function_call",
    category = "micro",
    iterations = 1_000_000
)]
pub fn bench_wasm_function_call(instance: &Instance) {
    let func = instance.get_func("process");
    
    black_box(|| {
        let start = Instant::now();
        func.call(&[Value::I32(42)]);
        metrics.record_latency(start.elapsed());
    });
}
```

**Variants:**
- No arguments/return
- Scalar arguments (i32, i64, f32, f64)
- Memory access (read/write)
- Table access
- Host function calls

**Targets:**
- P50 < 0.5µs, P95 < 1µs, P99 < 2µs, P999 < 5µs

#### 3.1.4 Memory Operations Benchmark

```rust
#[benchmark(
    name = "wasm_memory_ops",
    category = "micro",
    iterations = 10_000_000
)]
pub fn bench_wasm_memory_ops(memory: &Memory) {
    let offset = random_offset();
    
    // Read benchmark
    black_box(|| {
        let start = Instant::now();
        let value: u64 = memory.read(offset);
        metrics.record_latency(start.elapsed());
    });
    
    // Write benchmark
    black_box(|| {
        let start = Instant::now();
        memory.write(offset, 0xDEADBEEF);
        metrics.record_latency(start.elapsed());
    });
}
```

**Targets:**
- P50 < 10ns, P95 < 20ns, P99 < 30ns, P999 < 50ns

#### 3.1.5 Serialization Benchmark

```rust
#[benchmark(
    name = "rkyv_serialization",
    category = "micro",
    iterations = 100_000
)]
pub fn bench_rkyv_serialization(message: &Message) {
    // Serialize
    black_box(|| {
        let start = Instant::now();
        let bytes = rkyv::to_bytes(message);
        metrics.record("serialize", start.elapsed());
    });
    
    // Deserialize (zero-copy)
    black_box(|| {
        let start = Instant::now();
        let archived = rkyv::check_archived_root::<Message>(&bytes);
        metrics.record("deserialize", start.elapsed());
    });
}
```

**Payload sizes:** 64B, 256B, 1KB, 4KB, 16KB, 64KB, 256KB, 1MB

**Targets:**
- P50 < 2µs (1KB), P95 < 4µs, P99 < 6µs, P999 < 10µs
- Throughput > 2GB/s

### 3.2 Virtual Machine Micro-Benchmarks

#### 3.2.1 VM Boot Benchmark

```rust
#[benchmark(
    name = "vm_boot_cold",
    category = "micro",
    iterations = 100,
    timeout = "30s",
    isolation = "cpu_set(0-3)"
)]
pub async fn bench_vm_boot_cold(vm_manager: &FirecrackerManager) {
    let start = Instant::now();
    let vm = vm_manager.create_vm(VmConfig::default()).await?;
    vm.wait_for_boot().await?;
    let elapsed = start.elapsed();
    
    metrics.record_latency(elapsed);
    vm.destroy().await?;
}
```

**Configurations:**
- vCPUs: 1, 2, 4
- Memory: 128MB, 256MB, 512MB, 1GB
- Rootfs size: 10MB, 50MB, 100MB

**Targets:**
- P50 < 80ms, P95 < 100ms, P99 < 115ms, P999 < 125ms

#### 3.2.2 VM Pause/Resume Benchmark

```rust
#[benchmark(
    name = "vm_pause_resume",
    category = "micro",
    iterations = 1_000
)]
pub async fn bench_vm_pause_resume(vm: &VmHandle) {
    // Pause
    let start = Instant::now();
    vm.pause().await?;
    metrics.record("pause", start.elapsed());
    
    // Resume
    let start = Instant::now();
    vm.resume().await?;
    metrics.record("resume", start.elapsed());
}
```

**Targets:**
- Pause P50 < 3ms, P95 < 5ms, P99 < 8ms, P999 < 10ms
- Resume P50 < 5ms, P95 < 8ms, P99 < 12ms, P999 < 15ms

#### 3.2.3 VM Snapshot Benchmark

```rust
#[benchmark(
    name = "vm_snapshot",
    category = "micro",
    iterations = 100
)]
pub async fn bench_vm_snapshot(vm: &VmHandle) {
    // Create snapshot
    let start = Instant::now();
    let snapshot = vm.create_snapshot().await?;
    metrics.record("create", start.elapsed());
    
    // Restore snapshot
    let start = Instant::now();
    let restored = FirecrackerManager::restore_from_snapshot(&snapshot).await?;
    metrics.record("restore", start.elapsed());
}
```

**Targets:**
- Create P50 < 50ms, P95 < 80ms, P99 < 100ms, P999 < 120ms
- Restore P50 < 40ms, P95 < 60ms, P99 < 80ms, P999 < 100ms

### 3.3 Network Micro-Benchmarks

#### 3.3.1 Message Passing Benchmark

```rust
#[benchmark(
    name = "message_passing",
    category = "micro",
    iterations = 10_000_000
)]
pub fn bench_message_passing(channel: &Channel<Message>) {
    let msg = Message::ping();
    
    black_box(|| {
        let start = Instant::now();
        channel.send(msg.clone());
        metrics.record("send", start.elapsed());
    });
    
    black_box(|| {
        let start = Instant::now();
        let _ = channel.recv();
        metrics.record("recv", start.elapsed());
    });
}
```

**Variants:**
- Local (same thread)
- Local (cross-thread, same core)
- Local (cross-core)
- Remote (same node)
- Remote (cross-node, same rack)
- Remote (cross-rack)

**Targets:**
- Local P50 < 0.5µs, P95 < 1µs, P99 < 2µs, P999 < 5µs
- Remote (same node) P50 < 10µs, P95 < 20µs, P99 < 50µs, P999 < 100µs
- Remote (cross-node) P50 < 0.5ms, P95 < 0.8ms, P99 < 1ms, P999 < 2ms

#### 3.3.2 io_uring Benchmark

```rust
#[benchmark(
    name = "io_uring_ops",
    category = "micro",
    iterations = 1_000_000
)]
pub fn bench_io_uring(ring: &IoUring, fd: RawFd) {
    let buf = vec![0u8; 4096];
    
    // Read
    black_box(|| {
        let start = Instant::now();
        let entry = opcode::Read::new(fd, buf.as_mut_ptr(), buf.len() as _);
        ring.submission().push(&entry.build()).unwrap();
        ring.submit_and_wait(1).unwrap();
        metrics.record("read", start.elapsed());
    });
    
    // Write
    black_box(|| {
        let start = Instant::now();
        let entry = opcode::Write::new(fd, buf.as_ptr(), buf.len() as _);
        ring.submission().push(&entry.build()).unwrap();
        ring.submit_and_wait(1).unwrap();
        metrics.record("write", start.elapsed());
    });
}
```

**Variants:**
- SQPoll enabled/disabled
- Batch sizes: 1, 8, 32, 128
- Buffer sizes: 512B, 4KB, 64KB, 1MB

**Targets:**
- Single op P50 < 2µs, P95 < 5µs, P99 < 10µs
- Batch (32) P50 < 20µs, P95 < 40µs, P99 < 80µs

### 3.4 State Management Micro-Benchmarks

#### 3.4.1 KV Store Benchmark

```rust
#[benchmark(
    name = "kv_store",
    category = "micro",
    iterations = 1_000_000
)]
pub fn bench_kv_store(store: &KVStore) {
    let key = random_key();
    let value = random_value(1024);
    
    // Get
    black_box(|| {
        let start = Instant::now();
        let _ = store.get(&key);
        metrics.record("get", start.elapsed());
    });
    
    // Put
    black_box(|| {
        let start = Instant::now();
        store.put(&key, &value);
        metrics.record("put", start.elapsed());
    });
}
```

**Variants:**
- Local vs distributed
- Synchronous vs asynchronous
- Value sizes: 64B, 256B, 1KB, 4KB, 16KB
- Read/write ratios: 100/0, 90/10, 50/50, 10/90

**Targets:**
- Local Get P50 < 0.01ms, P95 < 0.05ms, P99 < 0.1ms
- Local Put P50 < 0.02ms, P95 < 0.08ms, P99 < 0.15ms
- Distributed Get P50 < 1ms, P95 < 3ms, P99 < 5ms

## 4. Integration Benchmarks

### 4.1 Actor-to-Actor Communication

#### 4.1.1 Local Actor Communication

```rust
#[benchmark(
    name = "actor_to_actor_local",
    category = "integration",
    iterations = 1_000_000
)]
pub async fn bench_actor_to_actor_local(runtime: &Runtime) {
    let actor_a = runtime.spawn_actor("actor_a", ActorConfig::wasm()).await?;
    let actor_b = runtime.spawn_actor("actor_b", ActorConfig::wasm()).await?;
    
    let msg = Message::request("ping");
    
    black_box(|| async {
        let start = Instant::now();
        let response = actor_a.send(actor_b.id(), msg.clone()).await?;
        metrics.record_latency(start.elapsed());
        Ok(response)
    });
}
```

**Variants:**
- Sync vs async messaging
- Request/response vs fire-and-forget
- Message sizes: 64B, 1KB, 64KB, 1MB

**Targets:**
- P50 < 0.5µs, P95 < 1µs, P99 < 2µs, P999 < 5µs

#### 4.1.2 Remote Actor Communication

```rust
#[benchmark(
    name = "actor_to_actor_remote",
    category = "integration",
    iterations = 100_000,
    cluster_size = 2
)]
pub async fn bench_actor_to_actor_remote(cluster: &Cluster) {
    let node_a = cluster.nodes()[0];
    let node_b = cluster.nodes()[1];
    
    let actor_a = node_a.spawn_actor("actor_a", ActorConfig::wasm()).await?;
    let actor_b = node_b.spawn_actor("actor_b", ActorConfig::wasm()).await?;
    
    let msg = Message::request("ping");
    
    black_box(|| async {
        let start = Instant::now();
        let response = actor_a.send(actor_b.id(), msg.clone()).await?;
        metrics.record_latency(start.elapsed());
        Ok(response)
    });
}
```

**Targets:**
- P50 < 0.5ms, P95 < 0.8ms, P99 < 1ms, P999 < 2ms

### 4.2 State Access Patterns

#### 4.2.1 State Hydration Benchmark

```rust
#[benchmark(
    name = "state_hydration",
    category = "integration",
    iterations = 10_000
)]
pub async fn bench_state_hydration(runtime: &Runtime, state_store: &StateStore) {
    let state = generate_state(1_000_000); // 1MB
    
    // Cold hydration
    black_box(|| async {
        let actor = runtime.spawn_actor("test", ActorConfig::wasm()).await?;
        state_store.store(actor.id(), &state).await?;
        
        let start = Instant::now();
        actor.hydrate_state().await?;
        metrics.record("cold", start.elapsed());
        Ok(())
    });
    
    // Warm hydration (cached)
    black_box(|| async {
        let actor = runtime.spawn_actor("test", ActorConfig::wasm()).await?;
        
        let start = Instant::now();
        actor.hydrate_state().await?;
        metrics.record("warm", start.elapsed());
        Ok(())
    });
}
```

**State sizes:** 10KB, 100KB, 1MB, 10MB

**Targets:**
- Cold (1MB) P50 < 20ms, P95 < 35ms, P99 < 45ms, P999 < 50ms
- Warm (1MB) P50 < 5ms, P95 < 10ms, P99 < 15ms, P999 < 20ms

### 4.3 Capability System Benchmark

#### 4.3.1 Capability Check Benchmark

```rust
#[benchmark(
    name = "capability_check",
    category = "integration",
    iterations = 10_000_000
)]
pub fn bench_capability_check(actor: &Actor, cap_system: &CapabilitySystem) {
    let caps = vec![
        Capability::FileRead("/data".into()),
        Capability::NetworkConnect("10.0.0.0/8".into()),
        Capability::TimeRead,
    ];
    
    actor.set_capabilities(caps);
    
    black_box(|| {
        let start = Instant::now();
        let allowed = cap_system.check(actor.id(), &CapabilityRequest::FileRead("/data/file.txt"));
        metrics.record_latency(start.elapsed());
        allowed
    });
}
```

**Variants:**
- Single capability check
- Multiple capability checks (batch)
- Cached vs uncached
- Complex policy evaluation

**Targets:**
- P50 < 0.1µs, P95 < 0.2µs, P99 < 0.3µs, P999 < 0.5µs

### 4.4 Actor Lifecycle Benchmark

#### 4.4.1 Actor Creation Benchmark

```rust
#[benchmark(
    name = "actor_lifecycle",
    category = "integration",
    iterations = 100_000
)]
pub async fn bench_actor_lifecycle(runtime: &Runtime) {
    // Create
    let start = Instant::now();
    let actor = runtime.spawn_actor("test", ActorConfig::wasm()).await?;
    metrics.record("create", start.elapsed());
    
    // Activate
    let start = Instant::now();
    actor.activate().await?;
    metrics.record("activate", start.elapsed());
    
    // Deactivate
    let start = Instant::now();
    actor.deactivate().await?;
    metrics.record("deactivate", start.elapsed());
    
    // Destroy
    let start = Instant::now();
    actor.destroy().await?;
    metrics.record("destroy", start.elapsed());
}
```

**Targets:**
- Create P50 < 50µs, P95 < 100µs, P99 < 200µs
- Activate P50 < 10ms, P95 < 20ms, P99 < 50ms
- Deactivate P50 < 5ms, P95 < 10ms, P99 < 20ms
- Destroy P50 < 5ms, P95 < 10ms, P99 < 20ms

## 5. End-to-End Benchmarks

### 5.1 Simple Request Processing

```rust
#[benchmark(
    name = "e2e_simple_request",
    category = "e2e",
    iterations = 100_000,
    scenario = "simple"
)]
pub async fn bench_e2e_simple_request(cluster: &Cluster) {
    let gateway = cluster.gateway();
    let client = TestClient::new();
    
    black_box(|| async {
        let start = Instant::now();
        let response = client
            .post(&gateway.url("/api/process"))
            .json(&simple_request())
            .send()
            .await?;
        metrics.record_latency(start.elapsed());
        assert_eq!(response.status(), 200);
        Ok(())
    });
}
```

**Scenario:**
1. Request received at gateway
2. Actor activated (WASM)
3. Message processed (no state)
4. Response returned

**Targets:**
- P50 < 0.1ms, P95 < 0.5ms, P99 < 1ms, P999 < 5ms

### 5.2 Stateful Request Processing

```rust
#[benchmark(
    name = "e2e_stateful_request",
    category = "e2e",
    iterations = 10_000,
    scenario = "stateful"
)]
pub async fn bench_e2e_stateful_request(cluster: &Cluster) {
    let gateway = cluster.gateway();
    let client = TestClient::new();
    
    // Pre-populate state
    let actor_id = create_actor_with_state(cluster, 1_000_000).await?;
    
    black_box(|| async {
        let start = Instant::now();
        let response = client
            .post(&gateway.url(&format!("/api/actors/{}/process", actor_id)))
            .json(&stateful_request())
            .send()
            .await?;
        metrics.record_latency(start.elapsed());
        assert_eq!(response.status(), 200);
        Ok(())
    });
}
```

**Scenario:**
1. Request received at gateway
2. Actor activated with state hydration
3. State accessed and modified
4. State persisted (async)
5. Response returned

**Targets:**
- P50 < 5ms, P95 < 15ms, P99 < 30ms, P999 < 50ms

### 5.3 VM-Isolated Request Processing

```rust
#[benchmark(
    name = "e2e_vm_isolation",
    category = "e2e",
    iterations = 1_000,
    scenario = "vm_isolation"
)]
pub async fn bench_e2e_vm_isolation(cluster: &Cluster) {
    let gateway = cluster.gateway();
    let client = TestClient::new();
    
    black_box(|| async {
        let start = Instant::now();
        let response = client
            .post(&gateway.url("/api/vm/process"))
            .json(&vm_request())
            .send()
            .await?;
        metrics.record_latency(start.elapsed());
        assert_eq!(response.status(), 200);
        Ok(())
    });
}
```

**Scenario:**
1. Request received at gateway
2. VM created and booted
3. Request forwarded to VM
4. VM processes request
5. Response returned
6. VM destroyed (or returned to pool)

**Targets:**
- P50 < 100ms, P95 < 150ms, P99 < 200ms, P999 < 300ms

### 5.4 Distributed Workflow

```rust
#[benchmark(
    name = "e2e_distributed_workflow",
    category = "e2e",
    iterations = 10_000,
    scenario = "distributed",
    cluster_size = 3
)]
pub async fn bench_e2e_distributed_workflow(cluster: &Cluster) {
    let gateway = cluster.gateway();
    let client = TestClient::new();
    
    black_box(|| async {
        let start = Instant::now();
        let response = client
            .post(&gateway.url("/api/workflow"))
            .json(&workflow_request())
            .send()
            .await?;
        metrics.record_latency(start.elapsed());
        assert_eq!(response.status(), 200);
        Ok(())
    });
}
```

**Scenario:**
1. Orchestrator receives workflow request
2. Actor A processes on node 1
3. Actor B processes on node 2 (parallel)
4. Actor C aggregates on node 3
5. Response returned

**Targets:**
- P50 < 10ms, P95 < 30ms, P99 < 50ms, P999 < 100ms

## 6. Load Testing

### 6.1 Steady-State Load Test

```rust
#[load_test(
    name = "steady_state_load",
    duration = "1h",
    target_throughput = "100_000 req/s",
    workers = 100
)]
pub async fn load_steady_state(cluster: &Cluster) {
    let gateway = cluster.gateway();
    let client = TestClient::new();
    
    loop {
        let start = Instant::now();
        match client.post(&gateway.url("/api/process")).send().await {
            Ok(response) => {
                metrics.record_latency(start.elapsed());
                metrics.record_success();
            }
            Err(e) => {
                metrics.record_error(e);
            }
        }
    }
}
```

**Success Criteria:**
- P99 latency < 10ms
- Error rate < 0.1%
- CPU utilization < 70%
- Memory utilization < 80%

### 6.2 Burst Load Test

```rust
#[load_test(
    name = "burst_load",
    pattern = "burst",
    burst_intervals = [
        (0, "10_000 req/s", "30s"),
        (30s, "100_000 req/s", "10s"),
        (40s, "10_000 req/s", "30s"),
    ],
    repeat = 10
)]
pub async fn load_burst(cluster: &Cluster) {
    // Burst load implementation
}
```

**Success Criteria:**
- P99 latency during burst < 2x steady state
- No request failures during burst
- Recovery to steady state within 5s

### 6.3 Ramp-Up/Ramp-Down Test

```rust
#[load_test(
    name = "ramp_test",
    pattern = "ramp",
    ramp_up = "5 min",
    steady = "30 min",
    ramp_down = "5 min",
    max_throughput = "200_000 req/s"
)]
pub async fn load_ramp(cluster: &Cluster) {
    // Ramp test implementation
}
```

**Success Criteria:**
- Linear throughput scaling during ramp-up
- No latency degradation during ramp-up
- Graceful degradation during ramp-down

### 6.4 Mixed Workload Test

```rust
#[load_test(
    name = "mixed_workload",
    duration = "2h",
    workload_mix = {
        "simple": 60%,
        "stateful": 30%,
        "vm_isolation": 5%,
        "workflow": 5%,
    },
    total_throughput = "50_000 req/s"
)]
pub async fn load_mixed(cluster: &Cluster) {
    // Mixed workload implementation
}
```

**Success Criteria:**
- All workload types meet their individual SLOs
- No resource starvation
- Fair scheduling observed

## 7. Stress Testing

### 7.1 Resource Exhaustion Tests

#### 7.1.1 CPU Saturation

```rust
#[stress_test(
    name = "cpu_saturation",
    target = "cpu_100%",
    duration = "30 min"
)]
pub async fn stress_cpu_saturation(cluster: &Cluster) {
    // Spawn CPU-intensive actors until saturation
    let mut actors = vec![];
    
    while cpu_utilization() < 99.0 {
        let actor = cluster.spawn_actor(
            &format!("cpu_stress_{}", actors.len()),
            ActorConfig::cpu_intensive(),
        ).await?;
        actors.push(actor);
    }
    
    // Monitor system stability
    monitor_for(Duration::from_secs(1800)).await;
}
```

**Success Criteria:**
- System remains responsive (P99 < 500ms)
- No actor starvation
- Graceful degradation observed
- No OOM kills

#### 7.1.2 Memory Exhaustion

```rust
#[stress_test(
    name = "memory_exhaustion",
    target = "memory_95%",
    duration = "30 min"
)]
pub async fn stress_memory_exhaustion(cluster: &Cluster) {
    // Spawn memory-intensive actors
    let mut actors = vec![];
    
    while memory_utilization() < 95.0 {
        let actor = cluster.spawn_actor(
            &format!("mem_stress_{}", actors.len()),
            ActorConfig::memory_intensive(100_000_000), // 100MB each
        ).await?;
        actors.push(actor);
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // Monitor for OOM and eviction
    monitor_for(Duration::from_secs(1800)).await;
}
```

**Success Criteria:**
- OOM killer not triggered
- Actor eviction functions correctly
- System remains operational
- Recovery when memory freed

#### 7.1.3 Network Saturation

```rust
#[stress_test(
    name = "network_saturation",
    target = "network_10Gbps",
    duration = "30 min"
)]
pub async fn stress_network_saturation(cluster: &Cluster) {
    // Generate maximum network traffic
    let actors: Vec<_> = (0..1000)
        .map(|i| cluster.spawn_actor(&format!("net_stress_{}", i), ActorConfig::network_intensive()))
        .collect();
    
    // Monitor network metrics
    monitor_network_bandwidth().await;
}
```

**Success Criteria:**
- No packet loss > 0.01%
- Latency degradation < 10x
- TCP retransmits < 0.1%

### 7.2 Chaos Testing

#### 7.2.1 Node Failure

```rust
#[chaos_test(
    name = "node_failure",
    fault = "kill_node",
    target_nodes = 1,
    cluster_size = 5
)]
pub async fn chaos_node_failure(cluster: &Cluster) {
    // Establish baseline
    let baseline_latency = measure_latency().await;
    
    // Kill random node
    let victim = cluster.random_node();
    victim.kill().await;
    
    // Measure recovery
    let recovery_start = Instant::now();
    wait_for_cluster_stable().await;
    let recovery_time = recovery_start.elapsed();
    
    // Verify functionality
    let post_failure_latency = measure_latency().await;
    
    metrics.record("recovery_time", recovery_time);
    metrics.record("latency_impact", post_failure_latency / baseline_latency);
}
```

**Success Criteria:**
- Recovery time < 30s
- Latency impact < 2x
- No data loss
- No actor corruption

#### 7.2.2 Network Partition

```rust
#[chaos_test(
    name = "network_partition",
    fault = "network_partition",
    partition_size = "2_of_5_nodes"
)]
pub async fn chaos_network_partition(cluster: &Cluster) {
    // Create partition
    cluster.partition(vec![0, 1], vec![2, 3, 4]).await;
    
    // Test partition tolerance
    test_partition_tolerance().await;
    
    // Heal partition
    cluster.heal_partition().await;
    
    // Test recovery
    test_partition_recovery().await;
}
```

**Success Criteria:**
- Majority partition remains operational
- Minority partition becomes read-only
- Split-brain prevention works
- Automatic recovery on heal

#### 7.2.3 Disk Failure

```rust
#[chaos_test(
    name = "disk_failure",
    fault = "disk_full",
    target = "state_disk"
)]
pub async fn chaos_disk_failure(cluster: &Cluster) {
    // Fill disk to 100%
    fill_disk(cluster.state_disk(), 100).await;
    
    // Test degradation
    test_disk_full_handling().await;
    
    // Free space
    free_disk(cluster.state_disk(), 20).await;
    
    // Test recovery
    test_disk_recovery().await;
}
```

**Success Criteria:**
- Graceful degradation (reject writes, allow reads)
- Alerts generated
- Automatic recovery when space available

### 7.3 Endurance Testing

```rust
#[endurance_test(
    name = "72h_endurance",
    duration = "72h",
    throughput = "50_000 req/s",
    checkpoints = ["6h", "12h", "24h", "48h", "72h"]
)]
pub async fn endurance_72h(cluster: &Cluster) {
    let start = Instant::now();
    let duration = Duration::from_secs(72 * 3600);
    
    while start.elapsed() < duration {
        // Run mixed workload
        run_mixed_workload().await;
        
        // Checkpoint validation
        if is_checkpoint(start.elapsed()) {
            validate_checkpoint().await;
        }
        
        sleep(Duration::from_secs(1)).await;
    }
}
```

**Success Criteria:**
- No memory leaks (RSS growth < 5% over 72h)
- No performance degradation (P99 within 10% of start)
- No resource exhaustion
- No crash/restart required

### 7.4 Boundary Testing

```rust
#[stress_test(
    name = "boundary_max_actors",
    target = "max_actor_density"
)]
pub async fn stress_max_actors(cluster: &Cluster) {
    let target = 150_000; // Max target
    let mut actors = vec![];
    
    for i in 0..target {
        match cluster.spawn_actor(&format!("boundary_{}", i), ActorConfig::minimal()).await {
            Ok(actor) => actors.push(actor),
            Err(e) => {
                metrics.record("max_actors", i);
                break;
            }
        }
        
        if i % 10_000 == 0 {
            metrics.record("actors_spawned", i);
        }
    }
}
```

**Success Criteria:**
- Reach at least 100,000 actors
- Graceful rejection when limit reached
- No system instability

## 8. Benchmark Execution

### 8.1 Execution Matrix

| Category | Frequency | Environment | Duration |
|----------|-----------|-------------|----------|
| Micro | Every commit | CI (bare metal) | 5 min |
| Integration | Every PR | CI (bare metal) | 15 min |
| E2E | Daily | Staging | 30 min |
| Load | Weekly | Production-like | 4 hours |
| Stress | Monthly | Production-like | 8 hours |
| Endurance | Quarterly | Production-like | 72 hours |

### 8.2 CI Integration

```yaml
# .github/workflows/benchmark.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  micro-benchmarks:
    runs-on: [self-hosted, benchmark]
    steps:
      - uses: actions/checkout@v4
      - name: Run micro-benchmarks
        run: cargo bench --bench micro -- --save-baseline main
      - name: Compare with baseline
        run: cargo bench --bench micro -- --baseline main
      - name: Check for regressions
        run: scripts/check_benchmark_regression.py
  
  integration-benchmarks:
    runs-on: [self-hosted, benchmark]
    steps:
      - uses: actions/checkout@v4
      - name: Run integration benchmarks
        run: cargo bench --bench integration
```

### 8.3 Reporting

```rust
pub struct BenchmarkReport {
    pub timestamp: DateTime<Utc>,
    pub commit_sha: String,
    pub branch: String,
    pub environment: EnvironmentInfo,
    pub results: Vec<BenchmarkResult>,
    pub regressions: Vec<Regression>,
    pub summary: ReportSummary,
}

pub struct BenchmarkResult {
    pub name: String,
    pub category: String,
    pub metrics: BenchmarkMetrics,
    pub baseline_comparison: Option<Comparison>,
    pub slo_status: SLOStatus,
}

pub enum SLOStatus {
    Met,
    Warning { percent_over: f64 },
    Failed { percent_over: f64 },
}
```

## 9. Test Fixtures

### 9.1 WASM Modules

| Module | Size | Purpose |
|--------|------|---------|
| minimal.wasm | 1KB | Minimal actor (no imports) |
| counter.wasm | 5KB | Simple stateful actor |
| compute.wasm | 50KB | CPU-intensive operations |
| memory.wasm | 100KB | Memory-intensive operations |
| network.wasm | 200KB | Network I/O operations |
| complex.wasm | 1MB | Complex actor with many imports |

### 9.2 VM Images

| Image | Size | Boot Time | Purpose |
|-------|------|-----------|---------|
| micro.rootfs | 5MB | 50ms | Minimal Linux |
| standard.rootfs | 50MB | 80ms | Standard utilities |
| full.rootfs | 200MB | 120ms | Full runtime |

### 9.3 Test Data

| Dataset | Size | Records | Purpose |
|---------|------|---------|---------|
| small.json | 1KB | 10 | Small payloads |
| medium.json | 100KB | 1,000 | Medium payloads |
| large.json | 10MB | 100,000 | Large payloads |
| huge.json | 1GB | 10,000,000 | Stress testing |

## 10. Continuous Benchmarking

### 10.1 Performance Regression Detection

```rust
pub struct RegressionDetector {
    baseline_window: Duration,
    significance_threshold: f64,
    z_score_threshold: f64,
}

impl RegressionDetector {
    pub fn detect_regression(
        &self,
        current: &BenchmarkMetrics,
        history: &[BenchmarkMetrics],
    ) -> Option<Regression> {
        let baseline = self.compute_baseline(history);
        
        if current.latency.p99 > baseline.latency.p99 * (1.0 + self.significance_threshold) {
            return Some(Regression {
                metric: "latency_p99".into(),
                baseline: baseline.latency.p99,
                current: current.latency.p99,
                percent_change: (current.latency.p99 - baseline.latency.p99) / baseline.latency.p99 * 100.0,
            });
        }
        
        None
    }
}
```

### 10.2 Historical Tracking

All benchmark results stored in time-series database:
- InfluxDB for metrics
- Grafana for visualization
- Automated alerting on regression

## 11. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-05 | Performance Team | Initial version |

## 12. Appendices

### Appendix A: Benchmark CLI

```bash
# Run all micro-benchmarks
aether bench --category micro

# Run specific benchmark
aether bench --name wasm_cold_start

# Run with custom configuration
aether bench --config benchmark_config.toml

# Compare with baseline
aether bench --baseline main --compare

# Generate report
aether bench --report html --output report.html
```

### Appendix B: Environment Setup

```bash
# Isolate CPUs for benchmarking
sudo cset shield --cpu=0-63 --exec -- aether bench

# Set CPU frequency to performance
sudo cpupower frequency-set -g performance

# Disable turbo boost for consistency
echo 0 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo

# Clear page cache
sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'
```
