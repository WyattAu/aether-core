# Project Aether Performance Guide

**Version:** 1.0.0-alpha  
**Last Updated:** 2026-03-12  
**Audience:** Performance Engineers, Platform Operators

---

## Table of Contents

1. [Performance Overview](#1-performance-overview)
2. [Cold Start Optimization](#2-cold-start-optimization)
3. [Memory Tuning](#3-memory-tuning)
4. [Network Tuning](#4-network-tuning)
5. [Benchmarking](#5-benchmarking)

---

## 1. Performance Overview

### 1.1 Performance Targets

| Metric | P50 | P99 | Max | Unit |
|--------|-----|-----|-----|------|
| WASM Cold Start | 20 | 45 | 50 | µs |
| VM Cold Start | 80 | 115 | 125 | ms |
| Local Message | 0.5 | 2 | 5 | µs |
| Remote Message (same DC) | 0.5 | 1 | 2 | ms |
| State Read (local) | 0.01 | 0.1 | 0.2 | ms |
| State Write (replicated) | 1 | 5 | 10 | ms |

### 1.2 Throughput Targets

| Metric | Target | Unit |
|--------|--------|------|
| Actor Instantiations | 100,000 | inst/s/core |
| Function Invocations | 10,000,000 | inv/s/core |
| Local Messages | 10,000,000 | msg/s/node |
| Remote Messages | 1,000,000 | msg/s/node |
| State Reads | 1,000,000 | reads/s/node |
| State Writes | 500,000 | writes/s/node |

### 1.3 Resource Efficiency

| Resource | Target | Max |
|----------|--------|-----|
| CPU Utilization | >95% | 85% |
| Memory Overhead | <5% | 10% |
| Runtime Base Memory | 50MB | 200MB |
| Actor Overhead | 0.5-2MB | 5MB |

---

## 2. Cold Start Optimization

### 2.1 WASM Cold Start

#### Target: <50µs (P99)

**Cold Start Phases:**

```
Phase                    Target    Cumulative
─────────────────────────────────────────────
1. Allocation            <10µs     10µs
2. Memory Setup          <15µs     25µs
3. Data Segments         <10µs     35µs
4. Table Init            <5µs      40µs
5. Globals               <5µs      45µs
6. Capability Bind       <3µs      48µs
7. Start Function        <2µs      50µs
```

#### Optimization Strategies

##### 1. Module Pre-Compilation

```toml
# aether.toml
[settings.wasm]
precompile = true          # Enable AOT compilation
cache-modules = true       # Cache compiled modules
cache-size = "500MB"       # Module cache size
```

##### 2. Memory Pool Pre-Warming

```toml
[settings.wasm.memory-pool]
enabled = true
pre-warm-count = 100       # Pre-allocate 100 memory slots
page-size = "64KB"         # WASM page size
```

##### 3. Data Segment Optimization

```rust
// Bad: Large data segment in WASM
#[link_section = ".data"]
static LARGE_DATA: [u8; 1024*1024] = [0; 1024*1024];

// Good: Lazy load from host
fn get_data() -> &'static [u8] {
    static DATA: OnceLock<Vec<u8>> = OnceLock::new();
    DATA.get_or_init(|| host::load_data())
}
```

##### 4. Minimize Start Function

```rust
// Bad: Heavy work in start
#[no_mangle]
fn _start() {
    initialize_database();  // Slow!
    load_configuration();   // Slow!
}

// Good: Defer to first request
#[no_mangle]
fn _start() {
    // Minimal initialization
}

#[export_name = "handle"]
fn handle(msg: &[u8]) -> Vec<u8> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        initialize_database();
        load_configuration();
    });
    // Handle message
}
```

#### Benchmarking Cold Start

```bash
# Measure cold start latency
aether benchmark cold-start --actor my-actor --samples 10000

# Output:
# Cold Start Benchmark: my-actor
# ─────────────────────────────────
# Samples:    10,000
# Mean:       28.5µs
# P50:        25.1µs
# P95:        38.2µs
# P99:        44.8µs
# P999:       48.9µs
# Max:        49.5µs
```

### 2.2 VM Cold Start

#### Target: <125ms (P99)

**Cold Start Phases:**

```
Phase                    Target    Cumulative
─────────────────────────────────────────────
1. Resource Allocation   <20ms     20ms
2. VM Boot               <50ms     70ms
3. Kernel Init           <20ms     90ms
4. Container Start       <25ms     115ms
5. Health Check          <10ms     125ms
```

#### Optimization Strategies

##### 1. VM Snapshots

```toml
[actors.db]
runtime = "oci"
image = "postgres:15"

[actors.db.snapshot]
enabled = true
base-snapshot = "postgres-base.snap"
snapshot-interval = "5m"
```

##### 2. VM Pool Pre-Warming

```toml
[settings.firecracker.pool]
enabled = true
min-size = 10             # Minimum pre-warmed VMs
max-size = 100            # Maximum pooled VMs
idle-timeout = "5m"       # Recycle idle VMs
```

##### 3. Minimal Root Filesystem

```dockerfile
# Bad: Large image
FROM ubuntu:latest
RUN apt-get update && apt-get install -y postgresql

# Good: Minimal image
FROM scratch
COPY postgres /postgres
ENTRYPOINT ["/postgres"]
```

##### 4. Fast Init Systems

```bash
# Use a minimal init system
# Avoid systemd in containers
CMD ["./postgres"]
# Instead of
# CMD ["systemd", "--system"]
```

#### Benchmarking VM Cold Start

```bash
# Measure VM cold start latency
aether benchmark vm-start --actor db --samples 1000

# Output:
# VM Cold Start Benchmark: db
# ─────────────────────────────────
# Samples:    1,000
# Mean:       95.2ms
# P50:        88.5ms
# P95:        110.3ms
# P99:        118.7ms
# Max:        122.1ms
```

### 2.3 Scale-to-Zero Optimization

```toml
[actors.api]
runtime = "wasm"
module = "./api.wasm"

[actors.api.scale-to-zero]
enabled = true
idle-timeout = "60s"      # Scale to zero after 60s idle
min-instances = 0
max-instances = 100
```

**Wake Latency Optimization:**

1. **Keep module cached**: Don't evict compiled modules
2. **Pre-warm memory pool**: Maintain memory pool even at zero instances
3. **Fast routing**: Direct routing from mesh to actor

---

## 3. Memory Tuning

### 3.1 Memory Pools

Aether uses memory pools to avoid allocation overhead:

```toml
[settings.memory]
allocator = "mimalloc"    # Use mimalloc allocator

[settings.memory.pools]
enabled = true
default-size = "4KB"
large-size = "64KB"
huge-size = "1MB"
```

### 3.2 Actor Memory Configuration

```toml
[actors.worker]
runtime = "wasm"
module = "./worker.wasm"

[actors.worker.memory]
initial = "64MiB"         # Initial memory
maximum = "256MiB"        # Maximum memory
growth-factor = 2.0       # Memory growth factor
```

### 3.3 Memory Hierarchy

```
┌─────────────────────────────────────────────────────────────┐
│                     Memory Hierarchy                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  L1: Hot Cache (CPU Cache)                                  │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ - Most frequently accessed actor state                  │ │
│  │ - Size: ~32KB per core                                  │ │
│  │ - Latency: <1ns                                         │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  L2: Local Memory Pool                                      │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ - Actor linear memory                                   │ │
│  │ - Pre-allocated, lock-free                              │ │
│  │ - Size: Configured per actor                            │ │
│  │ - Latency: <100ns                                       │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  L3: Shared Page Cache                                      │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ - Module code cache                                     │ │
│  │ - Shared across instances                               │ │
│  │ - Size: Configurable (default: 500MB)                   │ │
│  │ - Latency: <1µs                                         │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  L4: Distributed State (FoundationDB)                       │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ - Persistent actor state                                │ │
│  │ - Replicated across nodes                               │ │
│  │ - Size: Unbounded                                       │ │
│  │ - Latency: <10µs (local), <10ms (remote)               │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 3.4 Memory Monitoring

```bash
# Check memory usage
aether metrics --memory

# Output:
# Memory Metrics
# ─────────────────────────────────
# Total Allocated:    8.2 GB
# Runtime Overhead:   150 MB (1.8%)
# Actor Memory:       7.8 GB
# Page Cache:         450 MB
# Memory Pools:
#   - Small (4KB):    10,000 slots
#   - Large (64KB):   1,000 slots
#   - Huge (1MB):     100 slots
```

### 3.5 Memory Optimization Tips

1. **Avoid large stack allocations**
   ```rust
   // Bad: Large stack allocation
   fn process() {
       let buffer = [0u8; 1024 * 1024]; // 1MB on stack!
   }
   
   // Good: Use heap or pool
   fn process() {
       let buffer = host::allocate_buffer(1024 * 1024);
   }
   ```

2. **Reuse buffers**
   ```rust
   // Bad: Allocate per request
   fn handle(msg: &[u8]) -> Vec<u8> {
       let mut response = Vec::with_capacity(1024);
       // ...
       response
   }
   
   // Good: Use buffer pool
   fn handle(msg: &[u8]) -> PooledBuffer {
       let mut response = BufferPool::get(1024);
       // ...
       response
   }
   ```

3. **Minimize memory growth**
   ```toml
   # Set appropriate initial memory
   [actors.worker.memory]
   initial = "128MiB"  # Match expected working set
   maximum = "256MiB"
   ```

---

## 4. Network Tuning

### 4.1 Connection Pool Tuning

```toml
[settings.mesh.connection-pool]
max-connections = 1000     # Max connections per node
idle-timeout = "60s"       # Idle connection timeout
keep-alive-interval = "10s" # Keep-alive interval
eviction-policy = "lru"    # Eviction policy
```

### 4.2 QUIC Tuning

```toml
[settings.mesh.quic]
max-streams-bidirectional = 100
max-streams-unidirectional = 100
max-idle-timeout = "30s"
keep-alive-interval = "5s"
congestion-controller = "bbr"  # bbr, cubic, new_reno

[settings.mesh.quic.stream]
receive-window = "1MB"
send-window = "1MB"
```

### 4.3 Buffer Tuning

```toml
[settings.mesh.buffers]
send-buffer-size = "64KB"
receive-buffer-size = "64KB"
max-message-size = "10MB"
backpressure-threshold = 0.8  # 80% buffer capacity
```

### 4.4 Network Performance Monitoring

```bash
# Check network metrics
aether metrics --network

# Output:
# Network Metrics
# ─────────────────────────────────
# Active Connections:    456
# Messages Sent:         1.2M/s
# Messages Received:     1.1M/s
# Bandwidth (TX):        2.5 Gbps
# Bandwidth (RX):        2.3 Gbps
# Avg Latency (local):   0.8µs
# Avg Latency (remote):  1.2ms
# Retransmits:           12/s
# Connection Errors:     0
```

### 4.5 Network Optimization Tips

1. **Use message batching**
   ```rust
   // Bad: Send individual messages
   for item in items {
       mesh.send(actor, item).await?;
   }
   
   // Good: Batch messages
   mesh.send_batch(actor, items).await?;
   ```

2. **Prefer local actors**
   ```toml
   [actors.api.placement]
   node-selector = { zone = "a" }
   
   [actors.worker.placement]
   node-selector = { zone = "a" }  # Same zone as api
   ```

3. **Tune message sizes**
   - Small messages (<1KB): Optimized path
   - Medium messages (1KB-1MB): Standard path
   - Large messages (>1MB): Streaming path

---

## 5. Benchmarking

### 5.1 Built-in Benchmarks

#### Latency Benchmark

```bash
aether benchmark latency --actor api --samples 100000

# Output:
# Latency Benchmark: api
# ─────────────────────────────────
# Samples:       100,000
# Duration:      10.2s
# Throughput:    9,804 msg/s
# 
# Latency Distribution:
#   Mean:         102µs
#   Std Dev:      45µs
#   Min:          12µs
#   P50:          95µs
#   P90:          145µs
#   P95:          178µs
#   P99:          245µs
#   P999:         412µs
#   Max:          2.1ms
```

#### Throughput Benchmark

```bash
aether benchmark throughput --actor api --concurrency 100

# Output:
# Throughput Benchmark: api
# ─────────────────────────────────
# Duration:      30s
# Concurrency:   100
# Total Ops:     15,234,567
# Throughput:    507,819 ops/s
# 
# Resource Usage:
#   CPU:         78%
#   Memory:      1.2GB
#   Network TX:  1.2 Gbps
#   Network RX:  1.1 Gbps
```

#### Cold Start Benchmark

```bash
aether benchmark cold-start --actor api --samples 10000

# Output:
# Cold Start Benchmark: api
# ─────────────────────────────────
# Samples:       10,000
# Runtime:       wasm
# 
# Cold Start Latency:
#   Mean:         28.5µs
#   P50:          25.1µs
#   P95:          38.2µs
#   P99:          44.8µs
#   Max:          49.5µs
```

### 5.2 Load Testing

```bash
# Sustained load test
aether load-test --actor api \
  --rate 10000 \
  --duration 5m \
  --payload '{"action": "test"}'

# Ramp-up test
aether load-test --actor api \
  --start-rate 100 \
  --end-rate 100000 \
  --duration 10m \
  --step-duration 1m

# Stress test
aether stress-test --actor api \
  --max-concurrency 10000 \
  --timeout 30s
```

### 5.3 Profiling

#### CPU Profiling

```bash
# Start CPU profile
aether profile cpu --duration 30s --output cpu.prof

# Analyze profile
aether profile analyze cpu.prof

# Output:
# CPU Profile Analysis
# ─────────────────────────────────
# Total Samples:     30,000
# Duration:         30s
# 
# Top Functions:
#   25.3%  actor::handle_message
#   18.7%  mesh::route_message
#   12.1%  state::read_local
#    8.4%  wasm::invoke
#    6.2%  capability::check
```

#### Memory Profiling

```bash
# Start memory profile
aether profile memory --duration 60s --output mem.prof

# Analyze profile
aether profile analyze mem.prof

# Output:
# Memory Profile Analysis
# ─────────────────────────────────
# Total Allocations:  15.2GB
# Peak RSS:           4.8GB
# 
# Top Allocators:
#   35.2%  actor::buffer_pool
#   22.1%  mesh::connection
#   15.8%  state::cache
#   12.4%  wasm::instance
#    8.3%  capability::token
```

### 5.4 Continuous Benchmarking

```yaml
# .github/workflows/benchmark.yml
name: Benchmarks
on: [push, pull_request]

jobs:
  benchmark:
    runs-on: [self-hosted, benchmark]
    steps:
      - uses: actions/checkout@v4
      
      - name: Run benchmarks
        run: |
          aether benchmark all --output bench.json
          
      - name: Compare with baseline
        run: |
          aether benchmark compare bench.json baseline.json
          
      - name: Check for regressions
        run: |
          aether benchmark check --max-regression 5%
```

### 5.5 Performance Regression Detection

```bash
# Set baseline
aether benchmark all --output baseline.json

# Compare against baseline
aether benchmark compare current.json baseline.json

# Output:
# Performance Comparison
# ─────────────────────────────────
#                    Baseline    Current    Change
# Cold Start (P99):  45.2µs      48.1µs     +6.4% ⚠️
# Latency (P99):     245µs       238µs      -2.9% ✓
# Throughput:        507K/s      512K/s     +1.0% ✓
# 
# ⚠️ 1 regression detected (threshold: 5%)
```

---

## Appendix: Performance Checklist

### Deployment Checklist

- [ ] Pre-compile all WASM modules
- [ ] Configure appropriate memory pools
- [ ] Enable VM snapshotting for OCI actors
- [ ] Tune QUIC connection parameters
- [ ] Set up monitoring and alerting
- [ ] Configure backpressure thresholds
- [ ] Enable mimalloc allocator
- [ ] Set appropriate resource limits

### Monitoring Checklist

- [ ] P99 latency < targets
- [ ] Throughput > targets
- [ ] CPU utilization 70-85%
- [ ] Memory overhead < 5%
- [ ] No memory leaks
- [ ] Network latency within bounds
- [ ] Connection pool utilization < 80%

### Optimization Checklist

- [ ] Minimized data segments
- [ ] Optimized start functions
- [ ] Using buffer pools
- [ ] Batching messages
- [ ] Local actor placement
- [ ] Appropriate message sizes
- [ ] Tuned buffer sizes

---

*For more information, visit https://aether.dev/docs/performance*
