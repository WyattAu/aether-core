# Leak Detection Strategy
**Aether Resource Management - Phase 3.5**
**Document ID**: RM-LEAK-001
**Version**: 1.0
**Date**: 2026-03-05
**Status**: Final

---

## 1. Overview

Aether implements a multi-layered leak detection strategy combining static analysis, runtime checks, and external tools to ensure zero resource leaks across memory, handles, and system resources.

---

## 2. Leak Detection Layers

```
┌─────────────────────────────────────────────────────────┐
│                    CI/CD Pipeline                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Static       │  │ Build-time   │  │ Test-time    │  │
│  │ Analysis     │  │ Checks       │  │ Detection    │  │
│  │ (Clippy)     │  │ (Compiler)   │  │ (Valgrind)   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                    Development                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Debug        │  │ Runtime      │  │ Custom       │  │
│  │ Assertions   │  │ Tracking     │  │ Detectors    │  │
│  │ (ASAN)       │  │ (Metrics)    │  │ (RAII)       │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                    Production                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Telemetry    │  │ Health       │  │ Orphan       │  │
│  │ Monitoring   │  │ Checks       │  │ Reclamation  │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

---

## 3. Static Analysis

### 3.1 Clippy Lints

Enable strict Clippy lints for leak detection:

```toml
# .clippy.toml
avoid-breaking-exported-api = true

# Cargo.toml
[lints.clippy]
undocumented_unsafe_blocks = "deny"
missing_safety_doc = "deny"
unnecessary_unwrap = "warn"
expect_used = "warn"
panic = "warn"
```

### 3.2 Custom Lints

```rust
#![deny(clippy::mem_forget)]
#![warn(clippy::arc_with_non_send_sync)]
#![warn(clippy::boxed_local)]
#![warn(clippy::rc_buffer)]
```

### 3.3 Static Analysis Tools

- **Clippy**: General Rust lints
- **Miri**: Undefined behavior detection
- **Rust Analyzer**: IDE-level checks
- **cargo-deny**: Dependency security

---

## 4. Valgrind Integration

### 4.1 Valgrind Configuration

```bash
# valgrind_aether.supp
{
   mimalloc_false_positive_1
   Memcheck:Leak
   match-leak-kinds: reachable
   fun:mi_malloc
   ...
}

{
   mimalloc_false_positive_2
   Memcheck:Leak
   match-leak-kinds: reachable
   fun:mi_zalloc
   ...
}
```

### 4.2 Valgrind Test Script

```bash
#!/bin/bash
# scripts/valgrind_test.sh

set -euo pipefail

echo "Running Valgrind leak checks..."

cargo build --release --tests

valgrind \
    --leak-check=full \
    --show-leak-kinds=all \
    --track-origins=yes \
    --verbose \
    --log-file=valgrind_report.log \
    --suppressions=valgrind_aether.supp \
    --error-exitcode=1 \
    ./target/release/deps/aether_runtime-* \
    --test-threads=1

if grep "definitely lost: 0 bytes" valgrind_report.log; then
    echo "[PASS] No memory leaks detected"
    exit 0
else
    echo "[FAIL] Memory leaks detected!"
    cat valgrind_report.log
    exit 1
fi
```

### 4.3 CI/CD Integration

```yaml
# .github/workflows/valgrind.yml
name: Valgrind Leak Check

on: [push, pull_request]

jobs:
  valgrind:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Valgrind
        run: sudo apt-get install -y valgrind
      
      - name: Run Valgrind tests
        run: ./scripts/valgrind_test.sh
      
      - name: Upload Valgrind report
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: valgrind-report
          path: valgrind_report.log
```

---

## 5. AddressSanitizer (ASAN)

### 5.1 ASAN Configuration

```bash
# scripts/asan_test.sh
#!/bin/bash

set -euo pipefail

echo "Running AddressSanitizer checks..."

RUSTFLAGS="-Z sanitizer=address" \
cargo +nightly test --lib --target x86_64-unknown-linux-gnu \
    -- --test-threads=1

echo "[PASS] AddressSanitizer checks passed"
```

### 5.2 ASAN Options

```bash
export ASAN_OPTIONS=\
detect_leaks=1:\
detect_stack_use_after_return=1:\
detect_stack_use_after_scope=1:\
detect_invalid_pointer_pairs=2:\
detect_container_overflow=1:\
symbolize=1:\
abort_on_error=1
```

### 5.3 MemorySanitizer (MSAN)

```bash
# scripts/msan_test.sh
#!/bin/bash

set -euo pipefail

echo "Running MemorySanitizer checks..."

RUSTFLAGS="-Z sanitizer=memory" \
cargo +nightly test --lib --target x86_64-unknown-linux-gnu \
    -- --test-threads=1

echo "[PASS] MemorySanitizer checks passed"
```

### 5.4 ThreadSanitizer (TSAN)

```bash
# scripts/tsan_test.sh
#!/bin/bash

set -euo pipefail

echo "Running ThreadSanitizer checks..."

RUSTFLAGS="-Z sanitizer=thread" \
cargo +nightly test --lib --target x86_64-unknown-linux-gnu \
    -- --test-threads=1

echo "[PASS] ThreadSanitizer checks passed"
```

---

## 6. Custom Leak Detectors

### 6.1 Memory Leak Detector

```rust
pub struct MemoryLeakDetector {
    allocations: Arc<Mutex<HashMap<usize, AllocationInfo>>>,
    enabled: AtomicBool,
}

#[derive(Debug)]
pub struct AllocationInfo {
    size: usize,
    location: &'static std::panic::Location<'static>,
    timestamp: Instant,
    backtrace: Backtrace,
}

impl MemoryLeakDetector {
    pub fn new() -> Self {
        Self {
            allocations: Arc::new(Mutex::new(HashMap::new())),
            enabled: AtomicBool::new(true),
        }
    }

    pub fn record_allocation(
        &self,
        ptr: *mut u8,
        size: usize,
        location: &'static std::panic::Location<'static>,
    ) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let info = AllocationInfo {
            size,
            location,
            timestamp: Instant::now(),
            backtrace: Backtrace::capture(),
        };

        self.allocations.lock().unwrap().insert(ptr as usize, info);
    }

    pub fn record_deallocation(&self, ptr: *mut u8) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        self.allocations.lock().unwrap().remove(&(ptr as usize));
    }

    pub fn detect_leaks(&self) -> Vec<LeakReport> {
        let allocations = self.allocations.lock().unwrap();
        
        allocations
            .values()
            .filter(|info| info.timestamp.elapsed() > Duration::from_secs(10))
            .map(|info| LeakReport {
                size: info.size,
                location: info.location,
                age: info.timestamp.elapsed(),
                backtrace: info.backtrace.clone(),
            })
            .collect()
    }

    pub fn report(&self) -> LeakSummary {
        let leaks = self.detect_leaks();
        
        LeakSummary {
            total_leaks: leaks.len(),
            total_bytes: leaks.iter().map(|l| l.size).sum(),
            leaks,
        }
    }
}

#[derive(Debug)]
pub struct LeakReport {
    pub size: usize,
    pub location: &'static std::panic::Location<'static>,
    pub age: Duration,
    pub backtrace: Backtrace,
}

#[derive(Debug)]
pub struct LeakSummary {
    pub total_leaks: usize,
    pub total_bytes: usize,
    pub leaks: Vec<LeakReport>,
}
```

### 6.2 Handle Leak Detector

```rust
pub struct HandleLeakDetector {
    handles: Arc<Mutex<HashMap<Handle, HandleInfo>>>,
}

#[derive(Debug)]
pub struct HandleInfo {
    handle: Handle,
    created_at: Instant,
    location: &'static std::panic::Location<'static>,
    backtrace: Backtrace,
}

impl HandleLeakDetector {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn record_creation(
        &self,
        handle: Handle,
        location: &'static std::panic::Location<'static>,
    ) {
        let info = HandleInfo {
            handle,
            created_at: Instant::now(),
            location,
            backtrace: Backtrace::capture(),
        };

        self.handles.lock().unwrap().insert(handle, info);
    }

    pub fn record_closure(&self, handle: Handle) {
        self.handles.lock().unwrap().remove(&handle);
    }

    pub fn detect_leaks(&self, min_age: Duration) -> Vec<HandleLeakReport> {
        let handles = self.handles.lock().unwrap();
        
        handles
            .values()
            .filter(|info| info.created_at.elapsed() > min_age)
            .map(|info| HandleLeakReport {
                handle: info.handle,
                age: info.created_at.elapsed(),
                location: info.location,
                backtrace: info.backtrace.clone(),
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct HandleLeakReport {
    pub handle: Handle,
    pub age: Duration,
    pub location: &'static std::panic::Location<'static>,
    pub backtrace: Backtrace,
}
```

### 6.3 File Descriptor Leak Detector

```rust
pub struct FdLeakDetector {
    baseline_fds: Vec<i32>,
}

impl FdLeakDetector {
    pub fn new() -> Self {
        Self {
            baseline_fds: Self::get_open_fds(),
        }
    }

    fn get_open_fds() -> Vec<i32> {
        let mut fds = Vec::new();
        
        if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if let Ok(fd) = name.parse::<i32>() {
                        fds.push(fd);
                    }
                }
            }
        }
        
        fds.sort();
        fds
    }

    pub fn detect_leaks(&self) -> Vec<FdLeak> {
        let current_fds = Self::get_open_fds();
        let mut leaks = Vec::new();

        for fd in current_fds {
            if !self.baseline_fds.contains(&fd) {
                let path = std::fs::read_link(format!("/proc/self/fd/{}", fd))
                    .unwrap_or_else(|_| PathBuf::from("unknown"));

                leaks.push(FdLeak { fd, path });
            }
        }

        leaks
    }
}

#[derive(Debug)]
pub struct FdLeak {
    pub fd: i32,
    pub path: PathBuf,
}
```

---

## 7. CI/CD Leak Testing

### 7.1 Comprehensive Leak Test Workflow

```yaml
# .github/workflows/leak_detection.yml
name: Leak Detection

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  memory-leaks:
    name: Memory Leak Detection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y valgrind llvm-dev clang-dev
      
      - name: Run Valgrind
        run: ./scripts/valgrind_test.sh
      
      - name: Run ASAN
        run: ./scripts/asan_test.sh
      
      - name: Run MSAN
        run: ./scripts/msan_test.sh

  handle-leaks:
    name: Handle Leak Detection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run handle leak tests
        run: cargo test --release handle_leak -- --nocapture

  fd-leaks:
    name: File Descriptor Leak Detection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run FD leak tests
        run: cargo test --release fd_leak -- --nocapture

  stress-test:
    name: Stress Test with Leak Detection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run stress tests
        run: cargo test --release stress -- --nocapture
      
      - name: Check for leaks after stress test
        run: |
          cargo build --release
          valgrind --leak-check=full ./target/release/aether-runtime --stress-test
```

### 7.2 Nightly Leak Detection

```yaml
# .github/workflows/nightly_leak_detection.yml
name: Nightly Leak Detection

on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM UTC

jobs:
  long-running-leaks:
    name: Long-Running Leak Detection
    runs-on: ubuntu-latest
    timeout-minutes: 360
    steps:
      - uses: actions/checkout@v3
      
      - name: Run 6-hour stress test
        run: |
          cargo build --release
          timeout 6h valgrind \
            --leak-check=full \
            --log-file=valgrind_long_running.log \
            ./target/release/aether-runtime --long-running-test
          
          if grep "definitely lost: 0 bytes" valgrind_long_running.log; then
            echo "[PASS] No leaks in long-running test"
          else
            echo "[FAIL] Leaks detected!"
            cat valgrind_long_running.log
            exit 1
          fi
```

---

## 8. Runtime Leak Detection

### 8.1 Periodic Leak Check

```rust
pub struct LeakChecker {
    memory_detector: Arc<MemoryLeakDetector>,
    handle_detector: Arc<HandleLeakDetector>,
    fd_detector: Arc<FdLeakDetector>,
    interval: Duration,
}

impl LeakChecker {
    pub fn new(
        memory_detector: Arc<MemoryLeakDetector>,
        handle_detector: Arc<HandleLeakDetector>,
        fd_detector: Arc<FdLeakDetector>,
    ) -> Self {
        Self {
            memory_detector,
            handle_detector,
            fd_detector,
            interval: Duration::from_secs(60),
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(self.interval);
                self.check();
            }
        })
    }

    fn check(&self) {
        let memory_report = self.memory_detector.report();
        if memory_report.total_leaks > 0 {
            log::warn!(
                "Memory leaks detected: {} leaks, {} bytes",
                memory_report.total_leaks,
                memory_report.total_bytes
            );
        }

        let handle_leaks = self.handle_detector.detect_leaks(Duration::from_secs(300));
        if !handle_leaks.is_empty() {
            log::warn!("Handle leaks detected: {} handles", handle_leaks.len());
        }

        let fd_leaks = self.fd_detector.detect_leaks();
        if !fd_leaks.is_empty() {
            log::warn!("FD leaks detected: {} descriptors", fd_leaks.len());
        }
    }
}
```

### 8.2 Health Check Endpoint

```rust
pub struct LeakHealthCheck {
    memory_detector: Arc<MemoryLeakDetector>,
    handle_detector: Arc<HandleLeakDetector>,
    fd_detector: Arc<FdLeakDetector>,
}

impl LeakHealthCheck {
    pub fn check(&self) -> HealthStatus {
        let memory_report = self.memory_detector.report();
        let handle_leaks = self.handle_detector.detect_leaks(Duration::from_secs(300));
        let fd_leaks = self.fd_detector.detect_leaks();

        let has_critical_leaks = memory_report.total_bytes > 10 * 1024 * 1024
            || handle_leaks.len() > 100
            || fd_leaks.len() > 50;

        HealthStatus {
            healthy: !has_critical_leaks,
            memory_leaks: memory_report.total_leaks,
            memory_leaked_bytes: memory_report.total_bytes,
            handle_leaks: handle_leaks.len(),
            fd_leaks: fd_leaks.len(),
        }
    }
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub memory_leaks: usize,
    pub memory_leaked_bytes: usize,
    pub handle_leaks: usize,
    pub fd_leaks: usize,
}
```

---

## 9. Testing for Leaks

### 9.1 Unit Tests

```rust
#[cfg(test)]
mod leak_tests {
    use super::*;

    #[test]
    fn test_no_memory_leak() {
        let detector = MemoryLeakDetector::new();
        
        {
            let _handle = create_handle();
            // Handle should be cleaned up by RAII
        }
        
        std::thread::sleep(Duration::from_millis(100));
        
        let leaks = detector.detect_leaks();
        assert!(leaks.is_empty(), "Memory leaks detected: {:?}", leaks);
    }

    #[test]
    fn test_no_handle_leak() {
        let detector = HandleLeakDetector::new();
        
        {
            let handle = create_file_handle();
            // Handle should be closed by RAII
        }
        
        std::thread::sleep(Duration::from_millis(100));
        
        let leaks = detector.detect_leaks(Duration::from_millis(50));
        assert!(leaks.is_empty(), "Handle leaks detected: {:?}", leaks);
    }

    #[test]
    fn test_no_fd_leak() {
        let detector = FdLeakDetector::new();
        
        {
            let _file = File::open("/dev/null").unwrap();
        }
        
        let leaks = detector.detect_leaks();
        assert!(leaks.is_empty(), "FD leaks detected: {:?}", leaks);
    }
}
```

### 9.2 Integration Tests

```rust
#[test]
fn test_actor_termination_cleanup() {
    let system = ActorSystem::new();
    let actor_id = system.spawn(TestActor::new()).unwrap();
    
    let detector = HandleLeakDetector::new();
    
    system.terminate(actor_id).unwrap();
    
    std::thread::sleep(Duration::from_millis(100));
    
    let leaks = detector.detect_leaks(Duration::from_millis(50));
    assert!(leaks.is_empty(), "Handles leaked after actor termination");
}

#[test]
fn test_stress_no_leaks() {
    let detector = MemoryLeakDetector::new();
    
    for _ in 0..10000 {
        let system = ActorSystem::new();
        let actor_id = system.spawn(TestActor::new()).unwrap();
        system.terminate(actor_id).unwrap();
    }
    
    std::thread::sleep(Duration::from_secs(1));
    
    let leaks = detector.detect_leaks();
    assert!(leaks.is_empty(), "Memory leaks under stress");
}
```

---

## 10. Leak Detection Metrics

### 10.1 Metrics Collection

```rust
pub struct LeakMetrics {
    pub memory_leak_count: u64,
    pub memory_leaked_bytes: u64,
    pub handle_leak_count: u64,
    pub fd_leak_count: u64,
    pub last_check_time: Instant,
    pub checks_performed: u64,
}

impl LeakMetrics {
    pub fn to_prometheus(&self) -> String {
        format!(
            "# HELP aether_memory_leak_count Number of memory leaks detected\n\
             # TYPE aether_memory_leak_count gauge\n\
             aether_memory_leak_count {}\n\
             \n\
             # HELP aether_memory_leaked_bytes Bytes leaked\n\
             # TYPE aether_memory_leaked_bytes gauge\n\
             aether_memory_leaked_bytes {}\n\
             \n\
             # HELP aether_handle_leak_count Number of handle leaks\n\
             # TYPE aether_handle_leak_count gauge\n\
             aether_handle_leak_count {}\n\
             \n\
             # HELP aether_fd_leak_count Number of FD leaks\n\
             # TYPE aether_fd_leak_count gauge\n\
             aether_fd_leak_count {}",
            self.memory_leak_count,
            self.memory_leaked_bytes,
            self.handle_leak_count,
            self.fd_leak_count
        )
    }
}
```

---

## 11. Leak Detection Best Practices

### 11.1 Development Guidelines

1. **Always use RAII**: Wrap resources in RAII types
2. **Avoid `mem::forget`**: Let RAII handle cleanup
3. **Test for leaks**: Write leak detection tests
4. **Run sanitizers**: Use ASAN/MSAN/TSAN regularly
5. **Monitor in production**: Enable runtime leak checks

### 11.2 Code Review Checklist

- [ ] All resources wrapped in RAII types
- [ ] No manual `free`/`close` calls without RAII
- [ ] Handle cleanup in error paths
- [ ] Test coverage for resource cleanup
- [ ] Leak detection tests pass

---

## 12. Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Leak detection overhead (dev) | <5% | Profiling |
| Leak detection overhead (prod) | <1% | Telemetry |
| False positive rate | <1% | Manual review |
| Detection latency | <60s | Periodic check |
| CI leak test duration | <10min | CI/CD metrics |

---

## 13. References

- Valgrind: https://valgrind.org/
- AddressSanitizer: https://clang.llvm.org/docs/AddressSanitizer.html
- MemorySanitizer: https://clang.llvm.org/docs/MemorySanitizer.html
- ThreadSanitizer: https://clang.llvm.org/docs/ThreadSanitizer.html
- Rust sanitizers: https://github.com/japaric/rust-san
- RM-MEM-001: Memory management
- RM-HANDLE-001: Handle management

---

**Approval**: Resource Engineer
**Review Date**: 2026-03-05
