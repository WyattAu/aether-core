# Resource Limits Specification
**Aether Resource Management - Phase 3.5**
**Document ID**: RM-LIMITS-001
**Version**: 1.0
**Date**: 2026-03-05
**Status**: Final

---

## 1. Overview

Aether enforces strict resource limits across CPU, memory, I/O, and network to ensure fair resource distribution, prevent denial-of-service, and maintain system stability.

---

## 2. CPU Limits

### 2.1 WASM Fuel-Based Execution

WASM actors use fuel-based execution for deterministic CPU limiting:

```rust
pub struct WasmFuelLimiter {
    initial_fuel: u64,
    remaining_fuel: AtomicU64,
    fuel_per_instruction: u64,
}

impl WasmFuelLimiter {
    pub fn new(initial_fuel: u64) -> Self {
        Self {
            initial_fuel,
            remaining_fuel: AtomicU64::new(initial_fuel),
            fuel_per_instruction: 1,
        }
    }

    pub fn consume(&self, amount: u64) -> Result<(), FuelExhausted> {
        loop {
            let current = self.remaining_fuel.load(Ordering::SeqCst);
            if current < amount {
                return Err(FuelExhausted {
                    consumed: current,
                    requested: amount,
                });
            }
            
            let new_value = current - amount;
            if self.remaining_fuel.compare_exchange(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                return Ok(());
            }
        }
    }

    pub fn remaining(&self) -> u64 {
        self.remaining_fuel.load(Ordering::SeqCst)
    }

    pub fn refuel(&self, amount: u64) -> Result<(), FuelError> {
        loop {
            let current = self.remaining_fuel.load(Ordering::SeqCst);
            let new_value = current.saturating_add(amount);
            
            if new_value > self.initial_fuel {
                return Err(FuelError::ExceedsLimit);
            }
            
            if self.remaining_fuel.compare_exchange(
                current,
                new_value,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ).is_ok() {
                return Ok(());
            }
        }
    }
}
```

### 2.2 WASM Fuel Limits by Tier

| Actor Tier | Initial Fuel | Fuel per Message | Refill Policy |
|------------|--------------|------------------|---------------|
| System | 100,000,000 | 10,000,000 | Unlimited |
| Trusted | 50,000,000 | 5,000,000 | Per-message |
| User | 10,000,000 | 1,000,000 | Per-message |
| Untrusted | 5,000,000 | 500,000 | Per-message |

### 2.3 Fuel Monitoring

```rust
pub struct FuelMonitor {
    fuel_consumed: AtomicU64,
    fuel_refilled: AtomicU64,
    fuel_exhausted_count: AtomicU64,
}

impl FuelMonitor {
    pub fn record_consumption(&self, amount: u64) {
        self.fuel_consumed.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn record_refill(&self, amount: u64) {
        self.fuel_refilled.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn record_exhaustion(&self) {
        self.fuel_exhausted_count.fetch_add(1, Ordering::Relaxed);
    }
}
```

### 2.4 VM CPU Limits (cgroups v2)

VM actors use cgroups v2 for CPU limiting:

```rust
pub struct VmCpuLimiter {
    cgroup_path: PathBuf,
    cpu_max: CpuMax,
}

pub struct CpuMax {
    quota: u64,  // microseconds per period
    period: u64, // microseconds
}

impl VmCpuLimiter {
    pub fn new(vm_id: VmId, tier: ActorTier) -> Result<Self, CgroupError> {
        let cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/aether/vm-{}", vm_id));
        
        std::fs::create_dir_all(&cgroup_path)?;
        
        let cpu_max = match tier {
            ActorTier::System => CpuMax { quota: 200_000, period: 100_000 },  // 2 CPUs
            ActorTier::Trusted => CpuMax { quota: 150_000, period: 100_000 }, // 1.5 CPUs
            ActorTier::User => CpuMax { quota: 100_000, period: 100_000 },    // 1 CPU
            ActorTier::Untrusted => CpuMax { quota: 50_000, period: 100_000 }, // 0.5 CPU
            ActorTier::VM => CpuMax { quota: 400_000, period: 100_000 },       // 4 CPUs
        };

        let cpu_max_content = format!("{} {}", cpu_max.quota, cpu_max.period);
        std::fs::write(cgroup_path.join("cpu.max"), &cpu_max_content)?;

        Ok(Self {
            cgroup_path,
            cpu_max,
        })
    }

    pub fn add_process(&self, pid: u32) -> Result<(), CgroupError> {
        std::fs::write(self.cgroup_path.join("cgroup.procs"), pid.to_string())?;
        Ok(())
    }

    pub fn get_usage(&self) -> Result<CpuUsage, CgroupError> {
        let stat = std::fs::read_to_string(self.cgroup_path.join("cpu.stat"))?;
        let mut usage = CpuUsage::default();
        
        for line in stat.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                match parts[0] {
                    "usage_usec" => usage.usage_usec = parts[1].parse().unwrap_or(0),
                    "user_usec" => usage.user_usec = parts[1].parse().unwrap_or(0),
                    "system_usec" => usage.system_usec = parts[1].parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        Ok(usage)
    }
}

pub struct CpuUsage {
    pub usage_usec: u64,
    pub user_usec: u64,
    pub system_usec: u64,
}
```

---

## 3. Memory Limits

### 3.1 Linear Memory Limits (WASM)

```rust
pub struct WasmMemoryLimiter {
    tier: ActorTier,
    current_pages: AtomicU32,
    max_pages: u32,
    page_size: usize, // 64KB for WASM
}

impl WasmMemoryLimiter {
    pub fn new(tier: ActorTier) -> Self {
        let max_pages = match tier {
            ActorTier::System => 1024,    // 64 MB
            ActorTier::Trusted => 512,    // 32 MB
            ActorTier::User => 256,       // 16 MB
            ActorTier::Untrusted => 128,  // 8 MB
            ActorTier::VM => 2048,        // 128 MB
        };

        Self {
            tier,
            current_pages: AtomicU32::new(0),
            max_pages,
            page_size: 64 * 1024,
        }
    }

    pub fn can_grow(&self, delta_pages: u32) -> Result<u32, MemoryError> {
        let current = self.current_pages.load(Ordering::SeqCst);
        let new_total = current + delta_pages;
        
        if new_total > self.max_pages {
            return Err(MemoryError::MemoryLimitExceeded {
                current: current as usize * self.page_size,
                requested: new_total as usize * self.page_size,
                limit: self.max_pages as usize * self.page_size,
            });
        }

        Ok(new_total)
    }

    pub fn grow(&self, delta_pages: u32) -> Result<u32, MemoryError> {
        let new_total = self.can_grow(delta_pages)?;
        self.current_pages.store(new_total, Ordering::SeqCst);
        Ok(new_total)
    }
}
```

### 3.2 VM Memory Limits (cgroups v2)

```rust
pub struct VmMemoryLimiter {
    cgroup_path: PathBuf,
    memory_max: u64,
}

impl VmMemoryLimiter {
    pub fn new(vm_id: VmId, tier: ActorTier) -> Result<Self, CgroupError> {
        let cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/aether/vm-{}", vm_id));
        
        std::fs::create_dir_all(&cgroup_path)?;
        
        let memory_max = match tier {
            ActorTier::System => 96 * 1024 * 1024,   // 96 MB
            ActorTier::Trusted => 48 * 1024 * 1024,  // 48 MB
            ActorTier::User => 24 * 1024 * 1024,     // 24 MB
            ActorTier::Untrusted => 12 * 1024 * 1024, // 12 MB
            ActorTier::VM => 192 * 1024 * 1024,      // 192 MB
        };

        std::fs::write(cgroup_path.join("memory.max"), memory_max.to_string())?;

        Ok(Self {
            cgroup_path,
            memory_max,
        })
    }

    pub fn get_usage(&self) -> Result<MemoryUsage, CgroupError> {
        let current = std::fs::read_to_string(self.cgroup_path.join("memory.current"))?
            .trim()
            .parse()
            .unwrap_or(0);

        let stat = std::fs::read_to_string(self.cgroup_path.join("memory.stat"))?;
        let mut usage = MemoryUsage {
            current,
            max: self.memory_max,
            ..Default::default()
        };

        for line in stat.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                match parts[0] {
                    "file" => usage.file = parts[1].parse().unwrap_or(0),
                    "anon" => usage.anon = parts[1].parse().unwrap_or(0),
                    "kernel" => usage.kernel = parts[1].parse().unwrap_or(0),
                    _ => {}
                }
            }
        }

        Ok(usage)
    }
}

pub struct MemoryUsage {
    pub current: u64,
    pub max: u64,
    pub file: u64,
    pub anon: u64,
    pub kernel: u64,
}
```

---

## 4. I/O Limits

### 4.1 Bandwidth Limits (cgroups v2)

```rust
pub struct IoBandwidthLimiter {
    cgroup_path: PathBuf,
    read_bps_max: u64,
    write_bps_max: u64,
}

impl IoBandwidthLimiter {
    pub fn new(vm_id: VmId, tier: ActorTier) -> Result<Self, CgroupError> {
        let cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/aether/vm-{}", vm_id));
        
        std::fs::create_dir_all(&cgroup_path)?;
        
        let (read_bps_max, write_bps_max) = match tier {
            ActorTier::System => (100 * 1024 * 1024, 100 * 1024 * 1024),   // 100 MB/s
            ActorTier::Trusted => (50 * 1024 * 1024, 50 * 1024 * 1024),    // 50 MB/s
            ActorTier::User => (20 * 1024 * 1024, 20 * 1024 * 1024),       // 20 MB/s
            ActorTier::Untrusted => (10 * 1024 * 1024, 10 * 1024 * 1024),  // 10 MB/s
            ActorTier::VM => (200 * 1024 * 1024, 200 * 1024 * 1024),       // 200 MB/s
        };

        let io_max_content = format!("rbps={} wbps={}", read_bps_max, write_bps_max);
        std::fs::write(cgroup_path.join("io.max"), &io_max_content)?;

        Ok(Self {
            cgroup_path,
            read_bps_max,
            write_bps_max,
        })
    }

    pub fn get_usage(&self) -> Result<IoUsage, CgroupError> {
        let stat = std::fs::read_to_string(self.cgroup_path.join("io.stat"))?;
        let mut usage = IoUsage::default();

        for line in stat.lines() {
            for field in line.split_whitespace() {
                let parts: Vec<&str> = field.split('=').collect();
                if parts.len() == 2 {
                    match parts[0] {
                        "rbytes" => usage.rbytes += parts[1].parse().unwrap_or(0),
                        "wbytes" => usage.wbytes += parts[1].parse().unwrap_or(0),
                        "rios" => usage.rios += parts[1].parse().unwrap_or(0),
                        "wios" => usage.wios += parts[1].parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
        }

        Ok(usage)
    }
}

pub struct IoUsage {
    pub rbytes: u64,
    pub wbytes: u64,
    pub rios: u64,
    pub wios: u64,
}
```

### 4.2 IOPS Limits (cgroups v2)

```rust
pub struct IoOpsLimiter {
    cgroup_path: PathBuf,
    read_iops_max: u64,
    write_iops_max: u64,
}

impl IoOpsLimiter {
    pub fn new(vm_id: VmId, tier: ActorTier) -> Result<Self, CgroupError> {
        let cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/aether/vm-{}", vm_id));
        
        std::fs::create_dir_all(&cgroup_path)?;
        
        let (read_iops_max, write_iops_max) = match tier {
            ActorTier::System => (50_000, 50_000),   // 50K IOPS
            ActorTier::Trusted => (25_000, 25_000),  // 25K IOPS
            ActorTier::User => (10_000, 10_000),     // 10K IOPS
            ActorTier::Untrusted => (5_000, 5_000),  // 5K IOPS
            ActorTier::VM => (100_000, 100_000),     // 100K IOPS
        };

        let io_max_content = format!("riops={} wiops={}", read_iops_max, write_iops_max);
        std::fs::write(cgroup_path.join("io.max"), &io_max_content)?;

        Ok(Self {
            cgroup_path,
            read_iops_max,
            write_iops_max,
        })
    }
}
```

---

## 5. Network Limits

### 5.1 Connection Limits

```rust
pub struct NetworkLimiter {
    actor_id: ActorId,
    tier: ActorTier,
    current_connections: AtomicU64,
    max_connections: u64,
    bandwidth_tracker: BandwidthTracker,
}

impl NetworkLimiter {
    pub fn new(actor_id: ActorId, tier: ActorTier) -> Self {
        let max_connections = match tier {
            ActorTier::System => 1000,
            ActorTier::Trusted => 500,
            ActorTier::User => 100,
            ActorTier::Untrusted => 50,
            ActorTier::VM => 2000,
        };

        Self {
            actor_id,
            tier,
            current_connections: AtomicU64::new(0),
            max_connections,
            bandwidth_tracker: BandwidthTracker::new(tier),
        }
    }

    pub fn can_open_connection(&self) -> Result<(), NetworkError> {
        let current = self.current_connections.load(Ordering::SeqCst);
        
        if current >= self.max_connections {
            return Err(NetworkError::ConnectionLimitExceeded {
                current,
                max: self.max_connections,
            });
        }

        Ok(())
    }

    pub fn open_connection(&self) -> Result<(), NetworkError> {
        self.can_open_connection()?;
        self.current_connections.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn close_connection(&self) {
        self.current_connections.fetch_sub(1, Ordering::SeqCst);
    }
}
```

### 5.2 Bandwidth Limits (tc - Traffic Control)

```rust
pub struct BandwidthLimiter {
    interface: String,
    actor_id: ActorId,
    rate_limit: u64, // bytes per second
}

impl BandwidthLimiter {
    pub fn new(interface: String, actor_id: ActorId, tier: ActorTier) -> Result<Self, NetworkError> {
        let rate_limit = match tier {
            ActorTier::System => 100 * 1024 * 1024,   // 100 MB/s
            ActorTier::Trusted => 50 * 1024 * 1024,   // 50 MB/s
            ActorTier::User => 20 * 1024 * 1024,      // 20 MB/s
            ActorTier::Untrusted => 10 * 1024 * 1024, // 10 MB/s
            ActorTier::VM => 200 * 1024 * 1024,       // 200 MB/s
        };

        let class_id = actor_id.as_u32();
        
        std::process::Command::new("tc")
            .args(&[
                "class", "add", "dev", &interface,
                "parent", "1:1", "classid", &format!("1:{}", class_id),
                "htb", "rate", &format!("{}bit", rate_limit * 8),
            ])
            .status()?;

        Ok(Self {
            interface,
            actor_id,
            rate_limit,
        })
    }

    pub fn get_stats(&self) -> Result<BandwidthStats, NetworkError> {
        let output = std::process::Command::new("tc")
            .args(&["-s", "class", "show", "dev", &self.interface])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut stats = BandwidthStats::default();

        for line in stdout.lines() {
            if line.contains(&format!("1:{}", self.actor_id.as_u32())) {
                for field in line.split_whitespace() {
                    if field.starts_with("rate") {
                        stats.rate = field.split(':').nth(1).unwrap_or("0").parse().unwrap_or(0);
                    }
                }
            }
        }

        Ok(stats)
    }
}

pub struct BandwidthStats {
    pub rate: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
```

### 5.3 Bandwidth Tracker (Application-Level)

```rust
pub struct BandwidthTracker {
    tier: ActorTier,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    window_start: AtomicI64,
    window_size_secs: u64,
}

impl BandwidthTracker {
    pub fn new(tier: ActorTier) -> Self {
        Self {
            tier,
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            window_start: AtomicI64::new(chrono::Utc::now().timestamp()),
            window_size_secs: 1,
        }
    }

    pub fn record_send(&self, bytes: u64) -> Result<(), NetworkError> {
        let rate_limit = match self.tier {
            ActorTier::System => 100 * 1024 * 1024,
            ActorTier::Trusted => 50 * 1024 * 1024,
            ActorTier::User => 20 * 1024 * 1024,
            ActorTier::Untrusted => 10 * 1024 * 1024,
            ActorTier::VM => 200 * 1024 * 1024,
        };

        let current = self.bytes_sent.load(Ordering::SeqCst);
        let window_start = self.window_start.load(Ordering::SeqCst);
        let now = chrono::Utc::now().timestamp();

        if now - window_start >= self.window_size_secs as i64 {
            self.bytes_sent.store(bytes, Ordering::SeqCst);
            self.window_start.store(now, Ordering::SeqCst);
        } else {
            let new_total = current + bytes;
            if new_total > rate_limit {
                return Err(NetworkError::BandwidthLimitExceeded);
            }
            self.bytes_sent.store(new_total, Ordering::SeqCst);
        }

        Ok(())
    }

    pub fn record_receive(&self, bytes: u64) -> Result<(), NetworkError> {
        let rate_limit = match self.tier {
            ActorTier::System => 100 * 1024 * 1024,
            ActorTier::Trusted => 50 * 1024 * 1024,
            ActorTier::User => 20 * 1024 * 1024,
            ActorTier::Untrusted => 10 * 1024 * 1024,
            ActorTier::VM => 200 * 1024 * 1024,
        };

        let current = self.bytes_received.load(Ordering::SeqCst);
        let window_start = self.window_start.load(Ordering::SeqCst);
        let now = chrono::Utc::now().timestamp();

        if now - window_start >= self.window_size_secs as i64 {
            self.bytes_received.store(bytes, Ordering::SeqCst);
            self.window_start.store(now, Ordering::SeqCst);
        } else {
            let new_total = current + bytes;
            if new_total > rate_limit {
                return Err(NetworkError::BandwidthLimitExceeded);
            }
            self.bytes_received.store(new_total, Ordering::SeqCst);
        }

        Ok(())
    }
}
```

---

## 6. Resource Limit Summary by Tier

### 6.1 WASM Actors

| Resource | System | Trusted | User | Untrusted |
|----------|--------|---------|------|-----------|
| **CPU (Fuel)** | 100M | 50M | 10M | 5M |
| **Linear Memory** | 64 MB | 32 MB | 16 MB | 8 MB |
| **Heap Memory** | 32 MB | 16 MB | 8 MB | 4 MB |
| **Connections** | 1000 | 500 | 100 | 50 |
| **Bandwidth** | 100 MB/s | 50 MB/s | 20 MB/s | 10 MB/s |
| **I/O Bandwidth** | 100 MB/s | 50 MB/s | 20 MB/s | 10 MB/s |
| **IOPS** | 50K | 25K | 10K | 5K |

### 6.2 VM Actors

| Resource | Limit |
|----------|-------|
| **CPU** | 4 cores |
| **Memory** | 192 MB |
| **Connections** | 2000 |
| **Bandwidth** | 200 MB/s |
| **I/O Bandwidth** | 200 MB/s |
| **IOPS** | 100K |

---

## 7. Resource Limit Enforcement

### 7.1 Enforcement Points

```rust
pub struct ResourceEnforcer {
    cpu_limiter: CpuLimiter,
    memory_limiter: MemoryLimiter,
    io_limiter: IoLimiter,
    network_limiter: NetworkLimiter,
}

impl ResourceEnforcer {
    pub fn check_before_operation(&self, op: &Operation) -> Result<(), ResourceError> {
        match op {
            Operation::CpuIntensive { fuel } => {
                self.cpu_limiter.check_fuel(*fuel)?;
            }
            Operation::MemoryAllocation { size } => {
                self.memory_limiter.check_available(*size)?;
            }
            Operation::IoOperation { bytes } => {
                self.io_limiter.check_bandwidth(*bytes)?;
            }
            Operation::NetworkOperation { bytes } => {
                self.network_limiter.check_bandwidth(*bytes)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn record_usage(&self, op: &Operation, actual: u64) {
        match op {
            Operation::CpuIntensive { .. } => {
                self.cpu_limiter.record_consumption(actual);
            }
            Operation::MemoryAllocation { .. } => {
                self.memory_limiter.record_allocation(actual);
            }
            Operation::IoOperation { .. } => {
                self.io_limiter.record_bytes(actual);
            }
            Operation::NetworkOperation { .. } => {
                self.network_limiter.record_bytes(actual);
            }
            _ => {}
        }
    }
}
```

### 7.2 Soft vs Hard Limits

```rust
pub enum LimitType {
    Soft, // Throttle when exceeded
    Hard, // Reject when exceeded
}

pub struct ResourceLimit {
    soft_limit: u64,
    hard_limit: u64,
    limit_type: LimitType,
}

impl ResourceLimit {
    pub fn check(&self, current: u64, requested: u64) -> Result<LimitAction, ResourceError> {
        let new_total = current + requested;
        
        if new_total > self.hard_limit {
            return Err(ResourceError::HardLimitExceeded);
        }
        
        if new_total > self.soft_limit {
            return Ok(LimitAction::Throttle);
        }
        
        Ok(LimitAction::Allow)
    }
}

pub enum LimitAction {
    Allow,
    Throttle,
}
```

---

## 8. Monitoring and Telemetry

### 8.1 Resource Metrics

```rust
pub struct ResourceMetrics {
    pub actor_id: ActorId,
    pub tier: ActorTier,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub io: IoMetrics,
    pub network: NetworkMetrics,
}

pub struct CpuMetrics {
    pub fuel_consumed: u64,
    pub fuel_remaining: u64,
    pub fuel_exhausted_count: u64,
}

pub struct MemoryMetrics {
    pub linear_memory_used: usize,
    pub heap_memory_used: usize,
    pub allocations: u64,
    pub deallocations: u64,
}

pub struct IoMetrics {
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub read_ops: u64,
    pub write_ops: u64,
}

pub struct NetworkMetrics {
    pub connections_active: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}
```

### 8.2 Metrics Collection

```rust
impl ResourceManager {
    pub fn collect_metrics(&self, actor_id: ActorId) -> Option<ResourceMetrics> {
        let enforcer = self.enforcers.get(&actor_id)?;
        
        Some(ResourceMetrics {
            actor_id,
            tier: enforcer.tier,
            cpu: CpuMetrics {
                fuel_consumed: enforcer.cpu_limiter.consumed(),
                fuel_remaining: enforcer.cpu_limiter.remaining(),
                fuel_exhausted_count: enforcer.cpu_limiter.exhausted_count(),
            },
            memory: MemoryMetrics {
                linear_memory_used: enforcer.memory_limiter.linear_used(),
                heap_memory_used: enforcer.memory_limiter.heap_used(),
                allocations: enforcer.memory_limiter.allocations(),
                deallocations: enforcer.memory_limiter.deallocations(),
            },
            io: IoMetrics {
                bytes_read: enforcer.io_limiter.bytes_read(),
                bytes_written: enforcer.io_limiter.bytes_written(),
                read_ops: enforcer.io_limiter.read_ops(),
                write_ops: enforcer.io_limiter.write_ops(),
            },
            network: NetworkMetrics {
                connections_active: enforcer.network_limiter.active_connections(),
                bytes_sent: enforcer.network_limiter.bytes_sent(),
                bytes_received: enforcer.network_limiter.bytes_received(),
            },
        })
    }
}
```

---

## 9. Error Handling

### 9.1 Resource Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("CPU fuel exhausted: consumed {consumed}, requested {requested}")]
    FuelExhausted { consumed: u64, requested: u64 },
    
    #[error("Memory limit exceeded: used {used}, requested {requested}, limit {limit}")]
    MemoryLimitExceeded { used: usize, requested: usize, limit: usize },
    
    #[error("Connection limit exceeded: current {current}, max {max}")]
    ConnectionLimitExceeded { current: u64, max: u64 },
    
    #[error("Bandwidth limit exceeded")]
    BandwidthLimitExceeded,
    
    #[error("I/O limit exceeded")]
    IoLimitExceeded,
    
    #[error("Hard limit exceeded")]
    HardLimitExceeded,
}
```

---

## 10. Testing Requirements

### 10.1 Unit Tests

- Fuel consumption and refilling
- Memory limit enforcement
- Connection limit enforcement
- Bandwidth tracking

### 10.2 Integration Tests

- Resource exhaustion handling
- Throttling behavior
- Multi-actor resource competition
- cgroup enforcement

### 10.3 Stress Tests

- Resource exhaustion scenarios
- High-frequency limit checks
- Long-running resource monitoring

---

## 11. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Fuel check overhead | <10ns | Inline atomic |
| Memory limit check | <50ns | Inline atomic |
| Connection limit check | <20ns | Inline atomic |
| cgroup operation latency | <1ms | System call |
| Metrics collection overhead | <1% | Profiling |

---

## 12. References

- WASM fuel: https://docs.wasmtime.dev/examples-fuel.html
- cgroups v2: https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html
- Traffic Control: https://man7.org/linux/man-pages/man8/tc.8.html
- RM-MEM-001: Memory management

---

**Approval**: Resource Engineer
**Review Date**: 2026-03-05
