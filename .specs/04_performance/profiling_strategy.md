# Profiling Strategy
**Project Aether - Phase 4: Performance Engineering**

## Document Control
- **Version**: 1.0
- **Status**: Approved
- **Created**: 2026-03-05
- **Last Updated**: 2026-03-05
- **Author**: Performance Engineering Team
- **Review Status**: Complete

## 1. Executive Summary

This document outlines the comprehensive profiling strategy for Project Aether, covering CPU, memory, I/O, and network profiling approaches for both development and production environments. The strategy enables systematic identification of performance bottlenecks and optimization opportunities.

## 2. Profiling Philosophy

### 2.1 Guiding Principles

1. **Always-On Profiling**: Continuous low-overhead profiling in production
2. **Multi-Dimensional**: Profile CPU, memory, I/O, and network together
3. **Correlation**: Link profiling data with application metrics
4. **Minimal Overhead**: <5% performance impact in production
5. **Actionable**: Profiling must lead to concrete optimizations

### 2.2 Profiling Tiers

| Tier | Overhead | Frequency | Detail | Use Case |
|------|----------|-----------|--------|----------|
| Tier 0 | 0% | Always | Basic metrics | Production monitoring |
| Tier 1 | <1% | Always | Sampled profiling | Production continuous |
| Tier 2 | 1-5% | On-demand | Detailed profiling | Staging/debugging |
| Tier 3 | >5% | Manual | Full instrumentation | Development/benchmarks |

## 3. CPU Profiling

### 3.1 Tools and Techniques

#### 3.1.1 Linux perf

**Setup:**
```bash
# Enable perf for non-root users
sudo sysctl -w kernel.perf_event_paranoid=1
sudo sysctl -w kernel.kptr_restrict=0

# Install debug symbols
sudo apt install linux-modules-$(uname -r)-dbgsym
```

**Sampling Profiling:**
```bash
# Record CPU samples for 30 seconds
perf record -g -p $(pidof aether-runtime) -- sleep 30

# Generate report
perf report

# Generate flame graph
perf script | stackcollapse-perf.pl | flamegraph.pl > cpu_flame.svg
```

**Hardware Counters:**
```bash
# Cache misses
perf stat -e cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses \
    -p $(pidof aether-runtime) sleep 30

# Branch prediction
perf stat -e branches,branch-misses -p $(pidof aether-runtime) sleep 30

# TLB misses
perf stat -e dTLB-loads,dTLB-load-misses,iTLB-loads,iTLB-load-misses \
    -p $(pidof aether-runtime) sleep 30
```

**Integration:**
```rust
pub struct PerfProfiler {
    events: Vec<PerfEvent>,
    sample_rate: u64,
}

impl PerfProfiler {
    pub fn start_continuous(&self) -> Result<PerfSession> {
        let session = PerfSession::new()
            .events(&self.events)
            .sample_rate(self.sample_rate)
            .callchain(true)
            .start()?;
        
        Ok(session)
    }
    
    pub fn export_flamegraph(&self, session: &PerfSession) -> Result<String> {
        let stacks = session.collapsed_stacks()?;
        let flame = flamegraph::from_records(stacks)?;
        Ok(flame)
    }
}
```

#### 3.1.2 eBPF-based Profiling

**bpftrace Scripts:**

```bpftrace
// Profile CPU usage by function
profile:hz:99 /pid == PID/ {
    @stacks[ustack] = count();
}

// Trace lock contention
futex /pid == PID/ {
    @lock_contention[ustack] = count();
}

// Trace scheduling latency
sched:sched_switch /pid == PID/ {
    @sched_latency[args->prev_comm] = hist(args->prev_state);
}
```

**Custom eBPF Profiler:**
```rust
pub struct BpfProfiler {
    bpf: Bpf,
    perf_events: PerfEventArray<StackTraces>,
}

impl BpfProfiler {
    pub fn start(&mut self) -> Result<()> {
        let program: &mut Xdp = self.bpf.program_mut("profile_cpu")?.try_into()?;
        program.load()?;
        
        // Attach to CPU cycles
        let perf_event = PerfEventBuilder::new()
            .sample_rate(99)
            .cpu(Cpu::all())
            .attach(program)?;
        
        Ok(())
    }
    
    pub fn read_stacks(&self) -> Result<Vec<StackSample>> {
        let samples = self.perf_events.read()?;
        Ok(samples)
    }
}
```

#### 3.1.3 Custom Instrumentation

**Tracing Infrastructure:**
```rust
use tracing::{span, info_span, instrument};
use tracing_subscriber::layer::SubscriberExt;

pub fn init_tracing() {
    let subscriber = Registry::default()
        .with(tracing_tree::HierarchicalLayer::new(2))
        .with(tracing_timing::TimingLayer::new());
    
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[instrument(skip_all, fields(actor_id = %actor.id()))]
pub async fn process_message(actor: &Actor, msg: Message) -> Result<Response> {
    let _span = info_span!("deserialize").entered();
    let request = msg.deserialize()?;
    drop(_span);
    
    let _span = info_span!("execute").entered();
    let response = actor.execute(request).await?;
    drop(_span);
    
    Ok(response)
}
```

**Timing Macros:**
```rust
macro_rules! time_scope {
    ($name:expr, $block:block) => {{
        let _guard = scope_guard!(|| {
            let elapsed = start.elapsed();
            metrics::histogram!("scope_duration", "name" => $name)
                .record(elapsed.as_nanos() as f64);
        });
        let start = std::time::Instant::now();
        $block
    }};
}
```

### 3.2 Flame Graph Generation

**Automated Flame Graph Pipeline:**
```rust
pub struct FlameGraphGenerator {
    output_dir: PathBuf,
    sample_interval: Duration,
}

impl FlameGraphGenerator {
    pub async fn generate_continuous(&self, pid: u32) -> Result<()> {
        loop {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let output = self.output_dir.join(format!("flame_{}.svg", timestamp));
            
            // Capture samples
            let samples = self.capture_samples(pid, Duration::from_secs(60)).await?;
            
            // Generate flame graph
            let flame = self.generate_flame(&samples)?;
            
            // Write to file
            tokio::fs::write(&output, flame).await?;
            
            sleep(self.sample_interval).await;
        }
    }
    
    fn generate_flame(&self, samples: &[StackSample]) -> Result<String> {
        let mut collapsed = HashMap::new();
        
        for sample in samples {
            let stack = sample.stack.join(";");
            *collapsed.entry(stack).or_insert(0) += 1;
        }
        
        flamegraph::from_collapsed(&collapsed)
    }
}
```

**Flame Graph Types:**
- **Standard**: CPU time distribution
- **Differential**: Compare two time periods
- **Icicle**: Top-down view (caller → callee)
- **Cold**: Off-CPU time (blocking)

### 3.3 CPU Performance Counters

**Key Metrics:**
```rust
pub struct CpuCounters {
    cycles: u64,
    instructions: u64,
    cache_references: u64,
    cache_misses: u64,
    branch_instructions: u64,
    branch_misses: u64,
    stalled_cycles_frontend: u64,
    stalled_cycles_backend: u64,
}

impl CpuCounters {
    pub fn ipc(&self) -> f64 {
        self.instructions as f64 / self.cycles as f64
    }
    
    pub fn cache_miss_rate(&self) -> f64 {
        self.cache_misses as f64 / self.cache_references as f64
    }
    
    pub fn branch_miss_rate(&self) -> f64 {
        self.branch_misses as f64 / self.branch_instructions as f64
    }
    
    pub fn frontend_stall_rate(&self) -> f64 {
        self.stalled_cycles_frontend as f64 / self.cycles as f64
    }
    
    pub fn backend_stall_rate(&self) -> f64 {
        self.stalled_cycles_backend as f64 / self.cycles as f64
    }
}
```

**Target Values:**
| Metric | Target | Warning | Critical |
|--------|--------|---------|----------|
| IPC | >1.5 | 1.0-1.5 | <1.0 |
| Cache Miss Rate | <5% | 5-10% | >10% |
| Branch Miss Rate | <2% | 2-5% | >5% |
| Frontend Stalls | <10% | 10-20% | >20% |
| Backend Stalls | <20% | 20-40% | >40% |

## 4. Memory Profiling

### 4.1 Heap Profiling

#### 4.1.1 heaptrack

**Usage:**
```bash
# Profile heap allocations
heaptrack ./aether-runtime --config runtime.toml

# Analyze results
heaptrack_print heaptrack.aether-runtime.*.gz

# Generate visualization
heaptrack_gui heaptrack.aether-runtime.*.gz
```

**Key Metrics:**
- Total allocations
- Peak memory usage
- Allocation hotspots
- Temporary allocations
- Allocation size distribution

#### 4.1.2 Valgrind Massif

```bash
# Profile heap with detailed snapshots
valgrind --tool=massif --massif-out-file=massif.out \
    --pages-as-heap=yes \
    --threshold=0.5 \
    ./aether-runtime

# Visualize
ms_print massif.out
```

**Massif Configuration:**
```rust
pub fn run_with_massif<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    // Massif is external, but we can control its behavior
    std::env::set_var("MASSIF_DEPTH", "30");
    std::env::set_var("MASSIF_THRESHOLD", "0.5");
    f()
}
```

#### 4.1.3 Custom Allocation Tracking

```rust
use std::alloc::{GlobalAlloc, System, Layout};

pub struct TrackingAllocator {
    inner: System,
    stats: AllocStats,
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

impl TrackingAllocator {
    pub fn stats() -> &'static AllocStats {
        &ALLOCATOR.stats
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.inner.alloc(layout);
        if !ptr.is_null() {
            self.stats.record_alloc(layout.size());
        }
        ptr
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.stats.record_dealloc(layout.size());
        self.inner.dealloc(ptr, layout);
    }
}

pub struct AllocStats {
    total_allocs: AtomicU64,
    total_deallocs: AtomicU64,
    total_bytes_allocated: AtomicU64,
    total_bytes_freed: AtomicU64,
    peak_bytes: AtomicU64,
    current_bytes: AtomicU64,
    size_histogram: [AtomicU64; 20], // Buckets by power of 2
}
```

### 4.2 Memory Access Patterns

#### 4.2.1 Cache Analysis

```bash
# Use perf to analyze cache behavior
perf stat -e L1-dcache-loads,L1-dcache-load-misses,L1-dcache-stores,L1-dcache-store-misses \
    -e LLC-loads,LLC-load-misses,LLC-stores,LLC-store-misses \
    -p $(pidof aether-runtime) sleep 30
```

**Custom Cache Profiling:**
```rust
pub struct CacheProfiler {
    l1_hits: u64,
    l1_misses: u64,
    llc_hits: u64,
    llc_misses: u64,
}

impl CacheProfiler {
    pub fn profile_memory_access(&mut self, ptr: *const u8, size: usize) {
        unsafe {
            // Use prefetch to test cache state
            for i in (0..size).step_by(64) {
                let addr = ptr.add(i);
                
                // Check if in cache (prefetch instruction timing)
                let start = rdtsc();
                core::arch::x86_64::_mm_prefetch(addr as *const i8, 0);
                let end = rdtsc();
                
                if end - start < 100 {
                    self.l1_hits += 1;
                } else if end - start < 300 {
                    self.llc_hits += 1;
                } else {
                    self.llc_misses += 1;
                }
            }
        }
    }
}
```

#### 4.2.2 Memory Bandwidth Monitoring

```rust
pub struct MemoryBandwidthMonitor {
    pcm: PcmClient,
}

impl MemoryBandwidthMonitor {
    pub fn measure_bandwidth(&self) -> MemoryBandwidth {
        let before = self.pcm.read_memory_counters();
        sleep(Duration::from_millis(100));
        let after = self.pcm.read_memory_counters();
        
        MemoryBandwidth {
            read_gbps: (after.read_bytes - before.read_bytes) as f64 / 0.1 / 1e9,
            write_gbps: (after.write_bytes - before.write_bytes) as f64 / 0.1 / 1e9,
        }
    }
}
```

### 4.3 Memory Leak Detection

#### 4.3.1 AddressSanitizer (ASan)

```bash
# Build with ASan
RUSTFLAGS="-Z sanitizer=address" cargo build --target x86_64-unknown-linux-gnu

# Run with ASan
ASAN_OPTIONS=detect_leaks=1:abort_on_error=1 \
    ./target/x86_64-unknown-linux-gnu/debug/aether-runtime
```

#### 4.3.2 LeakSanitizer Integration

```rust
#[cfg(feature = "leak_detection")]
mod leak_detection {
    use std::ffi::CString;
    
    pub fn check_for_leaks() -> bool {
        unsafe {
            // Call __lsan_do_leak_check if available
            let func_name = CString::new("__lsan_do_leak_check").unwrap();
            let func = libc::dlsym(libc::RTLD_DEFAULT, func_name.as_ptr());
            
            if !func.is_null() {
                let check: extern "C" fn() -> i32 = std::mem::transmute(func);
                check() == 0 // Returns 0 if no leaks
            } else {
                true
            }
        }
    }
}
```

#### 4.3.3 Continuous Leak Monitoring

```rust
pub struct LeakMonitor {
    baseline_memory: u64,
    samples: Vec<u64>,
    threshold_mb: u64,
}

impl LeakMonitor {
    pub fn check_for_leak(&mut self) -> Option<LeakWarning> {
        let current = self.get_current_memory();
        self.samples.push(current);
        
        // Keep last 100 samples
        if self.samples.len() > 100 {
            self.samples.remove(0);
        }
        
        // Detect trend (simple linear regression)
        let trend = self.calculate_trend();
        
        if trend > 0.1 && current > self.baseline_memory + self.threshold_mb * 1_000_000 {
            Some(LeakWarning {
                current_memory: current,
                baseline: self.baseline_memory,
                growth_rate: trend,
            })
        } else {
            None
        }
    }
    
    fn calculate_trend(&self) -> f64 {
        // Linear regression slope
        let n = self.samples.len() as f64;
        let sum_x: f64 = (0..self.samples.len()).map(|i| i as f64).sum();
        let sum_y: f64 = self.samples.iter().map(|s| *s as f64).sum();
        let sum_xy: f64 = self.samples.iter()
            .enumerate()
            .map(|(i, y)| i as f64 * *y as f64)
            .sum();
        let sum_x2: f64 = (0..self.samples.len()).map(|i| (i * i) as f64).sum();
        
        (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x)
    }
}
```

### 4.4 NUMA Profiling

```bash
# Check NUMA topology
numactl --hardware

# Profile NUMA allocations
numastat -p $(pidof aether-runtime)

# Monitor NUMA balancing
perf stat -e numa_mm_migrations,numa_pte_updates,numa_huge_pte_updates \
    -p $(pidof aether-runtime) sleep 30
```

**NUMA-Aware Memory Allocation:**
```rust
pub struct NumaAllocator {
    node_mask: Vec<bool>,
    preferred_node: usize,
}

impl NumaAllocator {
    pub fn allocate_on_node(&self, size: usize, node: usize) -> *mut u8 {
        unsafe {
            let mut ptr: *mut libc::c_void = std::ptr::null_mut();
            
            // Use numa_alloc_onnode
            libc::posix_memalign(&mut ptr, 64, size);
            
            // Bind to specific node
            let mask = 1usize << node;
            libc::mbind(
                ptr as *mut libc::c_void,
                size,
                libc::MPOL_BIND,
                &mask as *const usize as *const libc::c_ulong,
                64,
                0,
            );
            
            ptr as *mut u8
        }
    }
}
```

## 5. I/O Profiling

### 5.1 io_uring Profiling

#### 5.1.1 io_uring Statistics

```rust
pub struct IoUringProfiler {
    submissions: AtomicU64,
    completions: AtomicU64,
    sqe_latency: Histogram,
    cqe_latency: Histogram,
    batch_sizes: Histogram,
}

impl IoUringProfiler {
    pub fn profile_ring(&self, ring: &IoUring) {
        let sq = ring.submission();
        let cq = ring.completion();
        
        // Track queue depths
        metrics::gauge!("io_uring_sq_depth").set(sq.len() as f64);
        metrics::gauge!("io_uring_cq_depth").set(cq.len() as f64);
        
        // Track submission/completion rates
        metrics::counter!("io_uring_submissions").increment(sq.len() as u64);
        metrics::counter!("io_uring_completions").increment(cq.len() as u64);
    }
}
```

#### 5.1.2 I/O Latency Distribution

```rust
pub struct IoLatencyTracker {
    reads: Histogram,
    writes: Histogram,
    fsyncs: Histogram,
}

impl IoLatencyTracker {
    pub fn record_io(&self, op: IoOp, size: usize, latency: Duration) {
        match op {
            IoOp::Read => {
                self.reads.record(latency.as_nanos() as f64);
                metrics::histogram!("io_read_latency", "size" => size.to_string())
                    .record(latency.as_nanos() as f64);
            }
            IoOp::Write => {
                self.writes.record(latency.as_nanos() as f64);
                metrics::histogram!("io_write_latency", "size" => size.to_string())
                    .record(latency.as_nanos() as f64);
            }
            IoOp::Fsync => {
                self.fsyncs.record(latency.as_nanos() as f64);
            }
        }
    }
}
```

### 5.2 Block I/O Profiling

```bash
# Monitor block I/O with bpftrace
bpftrace -e 'kprobe:submit_bio { @[comm] = count(); }'

# Trace I/O latency
biolatency -Q

# Trace I/O sizes
biosnoop -Q
```

**Custom Block I/O Profiler:**
```rust
pub struct BlockIoProfiler {
    device: String,
}

impl BlockIoProfiler {
    pub fn read_stats(&self) -> BlockIoStats {
        let path = format!("/sys/block/{}/stat", self.device);
        let contents = std::fs::read_to_string(&path).unwrap();
        
        let fields: Vec<u64> = contents.split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        
        BlockIoStats {
            reads_completed: fields[0],
            reads_merged: fields[1],
            sectors_read: fields[2],
            read_time_ms: fields[3],
            writes_completed: fields[4],
            writes_merged: fields[5],
            sectors_written: fields[6],
            write_time_ms: fields[7],
            io_in_progress: fields[8],
            io_time_ms: fields[9],
            weighted_io_time_ms: fields[10],
        }
    }
}
```

### 5.3 Filesystem Profiling

```rust
pub struct FilesystemProfiler {
    mount_point: PathBuf,
}

impl FilesystemProfiler {
    pub fn profile_operations(&self) {
        // Track file operations
        metrics::counter!("fs_open_calls").increment(1);
        metrics::counter!("fs_read_calls").increment(1);
        metrics::counter!("fs_write_calls").increment(1);
        metrics::counter!("fs_close_calls").increment(1);
        
        // Track file descriptor usage
        let fd_count = self.count_open_fds();
        metrics::gauge!("fs_open_fds").set(fd_count as f64);
    }
    
    fn count_open_fds(&self) -> usize {
        std::fs::read_dir("/proc/self/fd").unwrap().count()
    }
}
```

## 6. Network Profiling

### 6.1 Network Latency Profiling

```rust
pub struct NetworkProfiler {
    local_histogram: Histogram,
    remote_histogram: Histogram,
}

impl NetworkProfiler {
    pub async fn measure_mesh_latency(&self, cluster: &Cluster) -> LatencyMatrix {
        let mut matrix = LatencyMatrix::new();
        
        for node_a in cluster.nodes() {
            for node_b in cluster.nodes() {
                if node_a.id() != node_b.id() {
                    let latency = self.ping(node_a, node_b).await;
                    matrix.set(node_a.id(), node_b.id(), latency);
                }
            }
        }
        
        matrix
    }
    
    async fn ping(&self, from: &Node, to: &Node) -> Duration {
        let start = Instant::now();
        from.send_ping(to.id()).await.unwrap();
        start.elapsed()
    }
}
```

### 6.2 Network Throughput Profiling

```bash
# Measure throughput with iperf3
iperf3 -s &  # On server
iperf3 -c <server> -t 30  # On client

# Monitor network traffic
iftop -i eth0

# Packet capture
tcpdump -i eth0 -w capture.pcap
```

**Custom Throughput Profiler:**
```rust
pub struct NetworkThroughputProfiler {
    interface: String,
}

impl NetworkThroughputProfiler {
    pub fn measure_throughput(&self) -> NetworkThroughput {
        let before = self.read_interface_stats();
        sleep(Duration::from_secs(1));
        let after = self.read_interface_stats();
        
        NetworkThroughput {
            rx_bytes_per_sec: (after.rx_bytes - before.rx_bytes) as f64,
            tx_bytes_per_sec: (after.tx_bytes - before.tx_bytes) as f64,
            rx_packets_per_sec: (after.rx_packets - before.rx_packets) as f64,
            tx_packets_per_sec: (after.tx_packets - before.tx_packets) as f64,
        }
    }
    
    fn read_interface_stats(&self) -> InterfaceStats {
        let path = format!("/sys/class/net/{}/statistics/", self.interface);
        InterfaceStats {
            rx_bytes: read_counter(&format!("{}rx_bytes", path)),
            tx_bytes: read_counter(&format!("{}tx_bytes", path)),
            rx_packets: read_counter(&format!("{}rx_packets", path)),
            tx_packets: read_counter(&format!("{}tx_packets", path)),
        }
    }
}
```

### 6.3 TCP/UDP Profiling

```bash
# Monitor TCP connections
ss -tunap

# Track TCP retransmits
netstat -s | grep retransmit

# Monitor socket buffers
cat /proc/net/sockstat
```

**TCP Performance Profiling:**
```rust
pub struct TcpProfiler {
    retransmits: AtomicU64,
    zero_windows: AtomicU64,
    connection_resets: AtomicU64,
}

impl TcpProfiler {
    pub fn read_tcp_stats(&self) -> TcpStats {
        let contents = std::fs::read_to_string("/proc/net/snmp").unwrap();
        let tcp_line = contents.lines()
            .find(|l| l.starts_with("Tcp:"))
            .unwrap();
        
        let fields: Vec<u64> = tcp_line.split(':').nth(2).unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        
        TcpStats {
            active_opens: fields[4],
            passive_opens: fields[5],
            attempt_fails: fields[6],
            estab_resets: fields[7],
            curr_estab: fields[8],
            in_segs: fields[9],
            out_segs: fields[10],
            retrans_segs: fields[11],
        }
    }
}
```

## 7. Continuous Profiling in Production

### 7.1 Always-On Profiling

#### 7.1.1 Async-profiler Integration

```rust
pub struct AsyncProfiler {
    output_dir: PathBuf,
    interval: Duration,
}

impl AsyncProfiler {
    pub async fn start_continuous(&self, pid: u32) -> Result<()> {
        loop {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let output = self.output_dir.join(format!("profile_{}.html", timestamp));
            
            // Start async-profiler
            let mut cmd = Command::new("async-profiler/profiler.sh");
            cmd.arg("-d").arg("60")  // 60 second sample
                .arg("-f").arg(&output)
                .arg("-e").arg("cpu")
                .arg(pid.to_string());
            
            cmd.status().await?;
            
            sleep(self.interval).await;
        }
    }
}
```

#### 7.1.2 Continuous Flame Graphs

```rust
pub struct ContinuousFlameGraph {
    aggregator: StackAggregator,
    window_size: Duration,
}

impl ContinuousFlameGraph {
    pub fn add_sample(&mut self, stack: Vec<String>) {
        self.aggregator.add(stack);
    }
    
    pub fn generate_differential(&self, before: &Self) -> String {
        let current = self.aggregator.collapsed();
        let previous = before.aggregator.collapsed();
        
        let diff = DifferentialFlameGraph::new(&current, &previous);
        diff.to_svg()
    }
}
```

### 7.2 Profiling Data Pipeline

```
┌─────────────────┐
│  Application    │
│  (Instrumented) │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Profiling      │
│  Agent          │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Kafka/Queue    │
│  (Raw Samples)  │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Stream         │
│  Processor      │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Time-Series    │
│  DB (InfluxDB)  │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Visualization  │
│  (Grafana)      │
└─────────────────┘
```

### 7.3 Profiling Alerts

```rust
pub struct ProfilingAlertManager {
    thresholds: ProfilingThresholds,
}

impl ProfilingAlertManager {
    pub fn check_and_alert(&self, metrics: &ProfilingMetrics) {
        // CPU alerts
        if metrics.cpu_l1_miss_rate > self.thresholds.l1_miss_rate {
            self.send_alert(Alert::CpuL1MissRateHigh {
                value: metrics.cpu_l1_miss_rate,
                threshold: self.thresholds.l1_miss_rate,
            });
        }
        
        // Memory alerts
        if metrics.memory_growth_rate > self.thresholds.memory_growth_rate {
            self.send_alert(Alert::MemoryLeakSuspected {
                growth_rate: metrics.memory_growth_rate,
            });
        }
        
        // I/O alerts
        if metrics.io_p99_latency > self.thresholds.io_p99_latency {
            self.send_alert(Alert::IoLatencyHigh {
                value: metrics.io_p99_latency,
            });
        }
    }
}
```

## 8. Profiling Workflows

### 8.1 Development Workflow

```
1. Write code
2. Run benchmarks
3. Profile if regression detected
4. Identify hotspot
5. Optimize
6. Re-run benchmarks
7. Compare flame graphs
8. Commit if improved
```

### 8.2 Production Debugging Workflow

```
1. Alert triggered
2. Identify affected component
3. Enable Tier 2 profiling
4. Collect 5-10 minutes of data
5. Generate flame graphs
6. Correlate with logs/metrics
7. Identify root cause
8. Disable Tier 2 profiling
9. Apply fix
10. Monitor recovery
```

### 8.3 Optimization Workflow

```
1. Establish baseline
2. Profile representative workload
3. Identify top N hotspots
4. For each hotspot:
   a. Understand the code
   b. Hypothesize optimization
   c. Implement optimization
   d. Profile again
   e. Compare results
   f. Keep or revert
5. Document optimizations
6. Update baseline
```

## 9. Profiling Tools Matrix

| Tool | Type | Overhead | Use Case | Tier |
|------|------|----------|----------|------|
| perf | CPU | <5% | General CPU profiling | 1-3 |
| async-profiler | CPU | <2% | Java/continuous | 1-2 |
| heaptrack | Memory | 10-20% | Heap analysis | 2-3 |
| valgrind/massif | Memory | 20-50x | Detailed heap | 3 |
| valgrind/cachegrind | CPU/Mem | 20-50x | Cache simulation | 3 |
| bpftrace | General | <1% | Kernel tracing | 1-2 |
| strace | Syscall | 2-10x | Syscall tracing | 3 |
| ltrace | Library | 2-10x | Library tracing | 3 |
| tcpdump | Network | <1% | Packet capture | 1-2 |
| perf+eBPF | I/O | <1% | I/O latency | 1-2 |

## 10. Security Considerations

### 10.1 Profiling Data Sensitivity

- Stack traces may contain sensitive information
- Memory dumps must be handled carefully
- Network captures require encryption keys removed
- Access control for profiling interfaces

### 10.2 Profiling Access Control

```rust
pub struct ProfilingAccessControl {
    allowed_users: HashSet<String>,
    allowed_groups: HashSet<String>,
}

impl ProfilingAccessControl {
    pub fn can_enable_profiling(&self, user: &str, groups: &[String]) -> bool {
        self.allowed_users.contains(user) ||
            groups.iter().any(|g| self.allowed_groups.contains(g))
    }
}
```

## 11. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-05 | Performance Team | Initial version |

## 12. Appendices

### Appendix A: Profiling Commands Reference

```bash
# Quick CPU profile (60s)
perf record -g -p PID -- sleep 60 && perf script | stackcollapse-perf.pl | flamegraph.pl > flame.svg

# Memory leak check
valgrind --leak-check=full --show-leak-kinds=all ./aether-runtime

# I/O latency
biolatency -Q 60

# Network connections
ss -tunap

# Thread analysis
perf top -t TID
```

### Appendix B: Grafana Dashboards

- CPU Profiling Dashboard
- Memory Profiling Dashboard
- I/O Profiling Dashboard
- Network Profiling Dashboard
- Differential Flame Graphs
