# Conditional Compilation Strategy

**Document ID:** CP-CFG-001  
**Version:** 1.0.0  
**Status:** Draft  
**Created:** 2026-03-06  
**Author:** Compatibility Engineer  

---

## Overview

Project Aether uses conditional compilation to support multiple platforms, architectures, and feature sets while maintaining a single codebase. This document defines the strategy for using `cfg` flags, organizing platform-specific code, and balancing feature detection with compile-time checks.

---

## Configuration Flags

### Target OS Flags

```rust
// Operating system detection
#[cfg(target_os = "linux")]    // Linux
#[cfg(target_os = "macos")]    // macOS
#[cfg(target_os = "windows")]  // Windows
#[cfg(target_os = "freebsd")]  // FreeBSD
```

### Target Architecture Flags

```rust
// Architecture detection
#[cfg(target_arch = "x86_64")]   // x86-64 (AMD64)
#[cfg(target_arch = "aarch64")]  // ARM64
#[cfg(target_arch = "riscv64")]  // RISC-V 64-bit
```

### Target Endianness Flags

```rust
// Endianness detection
#[cfg(target_endian = "little")]  // Little-endian
#[cfg(target_endian = "big")]     // Big-endian
```

### Target Pointer Width Flags

```rust
// Pointer width detection
#[cfg(target_pointer_width = "64")]  // 64-bit
#[cfg(target_pointer_width = "32")]  // 32-bit
```

### Feature Flags

```rust
// Cargo feature flags
#[cfg(feature = "io_uring")]   // io_uring support
#[cfg(feature = "kvm")]        // KVM support
#[cfg(feature = "avx2")]       // AVX2 SIMD
#[cfg(feature = "avx512")]     // AVX-512 SIMD
```

### Custom cfg Flags

```rust
// Custom configuration flags (set in build.rs)
#[cfg(has_io_uring)]   // Runtime io_uring detection
#[cfg(has_kvm)]        // Runtime KVM detection
#[cfg(simd_avx2)]      // Runtime AVX2 detection
```

---

## Platform-Specific Code Organization

### Directory Structure

```
src/
├── lib.rs
├── runtime/
│   ├── mod.rs
│   ├── generic.rs          // Platform-independent code
│   ├── linux.rs            // Linux-specific implementation
│   ├── macos.rs            // macOS-specific implementation
│   ├── windows.rs          // Windows-specific implementation
│   └── platform.rs         // Platform detection
├── arch/
│   ├── mod.rs
│   ├── x86_64.rs           // x86_64 optimizations
│   ├── aarch64.rs          // ARM64 optimizations
│   └── detect.rs           // CPU feature detection
└── io/
    ├── mod.rs
    ├── io_uring.rs         // io_uring implementation
    ├── epoll.rs            // epoll implementation
    ├── kqueue.rs           // kqueue implementation
    └── iocp.rs             // IOCP implementation (future)
```

### Module Organization

```rust
// src/runtime/mod.rs

// Platform-independent traits and types
mod generic;

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

// Re-export platform-specific types
#[cfg(target_os = "linux")]
pub use linux::{Runtime, AsyncIo};

#[cfg(target_os = "macos")]
pub use macos::{Runtime, AsyncIo};

#[cfg(target_os = "windows")]
pub use windows::{Runtime, AsyncIo};

// Common interface
pub trait Runtime: Send + Sync {
    fn spawn<F>(&self, future: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
    
    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future;
}
```

### Implementation Example

```rust
// src/runtime/linux.rs

use super::Runtime;
use monoio::Runtime as MonoioRuntime;
use tokio::runtime::Runtime as TokioRuntime;

/// Linux runtime with dual data/control plane
pub struct Runtime {
    data_plane: MonoioRuntime,    // io_uring-based
    control_plane: TokioRuntime,  // Standard async
}

impl Runtime {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            data_plane: MonoioRuntime::builder()
                .with_entries(1024)
                .build()?,
            control_plane: TokioRuntime::new()?,
        })
    }
}

impl super::Runtime for Runtime {
    fn spawn<F>(&self, future: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // Route to appropriate runtime based on task type
        if is_data_plane_task(&future) {
            self.data_plane.spawn(future);
        } else {
            self.control_plane.spawn(future);
        }
    }
    
    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        self.data_plane.block_on(future)
    }
}
```

```rust
// src/runtime/macos.rs

use super::Runtime;
use tokio::runtime::Runtime as TokioRuntime;

/// macOS runtime (Tokio-only, no io_uring)
pub struct Runtime {
    runtime: TokioRuntime,
}

impl Runtime {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            runtime: TokioRuntime::new()?,
        })
    }
}

impl super::Runtime for Runtime {
    fn spawn<F>(&self, future: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future);
    }
    
    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        self.runtime.block_on(future)
    }
}
```

---

## Conditional Compilation Patterns

### Pattern 1: Module-Level Conditional

Use when entire modules are platform-specific:

```rust
// src/lib.rs

#[cfg(target_os = "linux")]
mod linux_platform;

#[cfg(target_os = "linux")]
pub use linux_platform::{create_runtime, create_vm};

#[cfg(target_os = "macos")]
mod macos_platform;

#[cfg(target_os = "macos")]
pub use macos_platform::{create_runtime};
```

### Pattern 2: Function-Level Conditional

Use for platform-specific implementations of the same interface:

```rust
// src/io/async_io.rs

pub trait AsyncIo {
    async fn read(&self, buf: &mut [u8]) -> Result<usize>;
    async fn write(&self, buf: &[u8]) -> Result<usize>;
}

// Linux implementation with io_uring
#[cfg(target_os = "linux")]
pub async fn async_read(fd: RawFd, buf: &mut [u8]) -> Result<usize> {
    // io_uring-based implementation
    use io_uring::IoUring;
    let mut ring = IoUring::new(256)?;
    // ...
}

// macOS implementation with kqueue
#[cfg(target_os = "macos")]
pub async fn async_read(fd: RawFd, buf: &mut [u8]) -> Result<usize> {
    // kqueue-based implementation
    use tokio::io::unix::AsyncFd;
    // ...
}
```

### Pattern 3: Type-Level Conditional

Use for platform-specific types:

```rust
// src/vm/mod.rs

#[cfg(target_os = "linux")]
mod firecracker;

#[cfg(target_os = "linux")]
pub use firecracker::Vm as MicroVm;

#[cfg(target_os = "macos")]
mod stub;

#[cfg(target_os = "macos")]
pub use stub::Vm as MicroVm;

// Common interface
pub trait Vm {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}
```

### Pattern 4: Feature-Gated Conditional

Use for optional features:

```rust
// Cargo.toml
[features]
default = ["io_uring"]
io_uring = []
avx2 = []
avx512 = []

// src/lib.rs
#[cfg(feature = "io_uring")]
mod io_uring_backend;

#[cfg(not(feature = "io_uring"))]
mod generic_backend;
```

### Pattern 5: Combined Conditions

Use for complex platform/feature combinations:

```rust
// Linux with io_uring
#[cfg(all(target_os = "linux", feature = "io_uring"))]
pub fn create_runtime() -> impl Runtime {
    monoio::Runtime::new().unwrap()
}

// Linux without io_uring
#[cfg(all(target_os = "linux", not(feature = "io_uring")))]
pub fn create_runtime() -> impl Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

// Non-Linux
#[cfg(not(target_os = "linux"))]
pub fn create_runtime() -> impl Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
```

---

## Feature Detection vs Compile-Time Checks

### Compile-Time Checks (Preferred)

**Use when:**
- Feature is always available on target platform
- Feature can be determined from `target_os` or `target_arch`
- Performance is critical

**Advantages:**
- Zero runtime overhead
- Compile-time error detection
- Optimized binary size

**Example:**

```rust
// Compile-time check (preferred)
#[cfg(target_os = "linux")]
fn use_io_uring() {
    // io_uring is always available on Linux 5.1+
}

// Usage
#[cfg(target_os = "linux")]
let io_backend = IoUringBackend::new();

#[cfg(not(target_os = "linux"))]
let io_backend = GenericBackend::new();
```

### Runtime Feature Detection (When Necessary)

**Use when:**
- Feature availability varies within same platform
- Feature depends on hardware (CPU features)
- Feature depends on kernel version

**Advantages:**
- Flexible deployment
- Single binary for multiple configurations
- Graceful degradation

**Disadvantages:**
- Runtime overhead (minimal)
- More complex code
- Requires testing multiple paths

**Example:**

```rust
use std::sync::OnceLock;

static HAS_AVX2: OnceLock<bool> = OnceLock::new();

fn has_avx2() -> bool {
    *HAS_AVX2.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        {
            is_x86_feature_detected!("avx2")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    })
}

// Usage
pub fn process_data(data: &[u8]) -> Vec<u8> {
    if has_avx2() {
        #[cfg(target_arch = "x86_64")]
        {
            unsafe { process_avx2(data) }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            process_generic(data)
        }
    } else {
        process_generic(data)
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn process_avx2(data: &[u8]) -> Vec<u8> {
    // AVX2-optimized implementation
    vec![]
}

fn process_generic(data: &[u8]) -> Vec<u8> {
    // Generic implementation
    vec![]
}
```

### Hybrid Approach

Combine compile-time and runtime checks for optimal results:

```rust
// src/arch/detect.rs

pub struct CpuCapabilities {
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub has_neon: bool,
}

impl CpuCapabilities {
    pub fn detect() -> Self {
        Self {
            #[cfg(target_arch = "x86_64")]
            {
                has_avx2: is_x86_feature_detected!("avx2"),
                has_avx512: is_x86_feature_detected!("avx512f"),
                has_neon: false,
            }
            #[cfg(target_arch = "aarch64")]
            {
                has_avx2: false,
                has_avx512: false,
                has_neon: is_aarch64_feature_detected!("neon"),
            }
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            {
                has_avx2: false,
                has_avx512: false,
                has_neon: false,
            }
        }
    }
}

// Usage
lazy_static! {
    static ref CPU_CAPS: CpuCapabilities = CpuCapabilities::detect();
}

pub fn hash(data: &[u8]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if CPU_CAPS.has_avx512 {
            unsafe { hash_avx512(data) }
        } else if CPU_CAPS.has_avx2 {
            unsafe { hash_avx2(data) }
        } else {
            hash_generic(data)
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if CPU_CAPS.has_neon {
            unsafe { hash_neon(data) }
        } else {
            hash_generic(data)
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        hash_generic(data)
    }
}
```

---

## Build Script Configuration

### build.rs

Use build scripts for advanced feature detection:

```rust
// build.rs

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("platform_config.rs");
    
    let mut config = String::new();
    
    // Check for io_uring support
    #[cfg(target_os = "linux")]
    {
        if check_io_uring() {
            config.push_str("pub const HAS_IO_URING: bool = true;\n");
            println!("cargo:rustc-cfg=has_io_uring");
        } else {
            config.push_str("pub const HAS_IO_URING: bool = false;\n");
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        config.push_str("pub const HAS_IO_URING: bool = false;\n");
    }
    
    // Check for KVM support
    #[cfg(target_os = "linux")]
    {
        if check_kvm() {
            config.push_str("pub const HAS_KVM: bool = true;\n");
            println!("cargo:rustc-cfg=has_kvm");
        } else {
            config.push_str("pub const HAS_KVM: bool = false;\n");
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        config.push_str("pub const HAS_KVM: bool = false;\n");
    }
    
    // Write config
    fs::write(&dest_path, config).unwrap();
    
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(target_os = "linux")]
fn check_io_uring() -> bool {
    // Check kernel version
    if let Ok(version) = fs::read_to_string("/proc/version") {
        // Parse version string
        // Simplified check: look for 5.x or 6.x
        version.contains("5.") || version.contains("6.")
    } else {
        false
    }
}

#[cfg(target_os = "linux")]
fn check_kvm() -> bool {
    Path::new("/dev/kvm").exists()
}
```

### Usage

```rust
// src/lib.rs

mod platform_config {
    include!(concat!(env!("OUT_DIR"), "/platform_config.rs"));
}

// Use generated config
#[cfg(has_io_uring)]
fn create_io_backend() -> Box<dyn IoBackend> {
    Box::new(IoUringBackend::new())
}

#[cfg(not(has_io_uring))]
fn create_io_backend() -> Box<dyn IoBackend> {
    Box::new(GenericBackend::new())
}
```

---

## Cargo Configuration

### .cargo/config.toml

```toml
# Target-specific configurations
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native"]

[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native"]

[target.wasm32-wasip1]
rustflags = [
  "-C",
  "target-feature=+simd128,+bulk-memory,+sign-ext",
]

# Feature flags for different platforms
[build]
# Default features for development
# Use --no-default-features and --features for production builds

[target.'cfg(target_os = "linux")'.dependencies]
io-uring = "0.6"
kvm-bindings = "0.7"

[target.'cfg(target_os = "macos")'.dependencies]
# macOS-specific dependencies
```

### Cargo.toml

```toml
[package]
name = "aether-runtime"
version = "0.1.0"
edition = "2021"

[features]
default = ["io_uring"]

# Platform-specific features
io_uring = ["io-uring", "monoio"]
kvm = ["kvm-bindings", "kvm-ioctls"]
avx2 = []
avx512 = []

# Development features
debug-io = []
trace = ["tracing"]

[dependencies]
# Common dependencies
tokio = { version = "1.35", features = ["full"] }
tracing = { version = "0.1", optional = true }

# Platform-specific dependencies
[target.'cfg(target_os = "linux")'.dependencies]
io-uring = { version = "0.6", optional = true }
monoio = { version = "0.2", optional = true }
kvm-bindings = { version = "0.7", optional = true }
kvm-ioctls = { version = "0.16", optional = true }

[target.'cfg(target_os = "macos")'.dependencies]
# macOS doesn't support io_uring or KVM

[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "runtime_benchmark"
harness = false
```

---

## Testing Conditional Code

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_specific() {
        // Test Linux-specific code
        let runtime = create_runtime();
        assert!(runtime.is_ok());
    }
    
    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_specific() {
        // Test macOS-specific code
        let runtime = create_runtime();
        assert!(runtime.is_ok());
    }
    
    #[test]
    fn test_platform_independent() {
        // Test code that works on all platforms
        let result = generic_function();
        assert!(result.is_ok());
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs

#[cfg(target_os = "linux")]
mod linux_tests {
    use aether_runtime::*;
    
    #[test]
    fn test_io_uring() {
        // Test io_uring functionality
        let backend = IoUringBackend::new(256).unwrap();
        // ...
    }
    
    #[test]
    fn test_kvm() {
        // Test KVM functionality
        let vm = create_vm().unwrap();
        // ...
    }
}

#[cfg(target_os = "macos")]
mod macos_tests {
    use aether_runtime::*;
    
    #[test]
    fn test_tokio_runtime() {
        // Test Tokio-based runtime
        let runtime = create_runtime().unwrap();
        // ...
    }
}
```

### CI Testing Matrix

```yaml
# .github/workflows/test.yml
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        features:
          - ""
          - "--no-default-features"
          - "--features avx2"
          - "--features avx512"
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - name: Run tests
        run: cargo test ${{ matrix.features }}
```

---

## Best Practices

### 1. Prefer Traits Over Conditional Code

```rust
// Bad: Conditional code everywhere
pub fn process(data: &[u8]) -> Vec<u8> {
    #[cfg(target_os = "linux")]
    {
        // Linux code
    }
    #[cfg(target_os = "macos")]
    {
        // macOS code
    }
}

// Good: Trait-based abstraction
pub trait Processor {
    fn process(&self, data: &[u8]) -> Vec<u8>;
}

pub fn create_processor() -> Box<dyn Processor> {
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxProcessor::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSProcessor::new())
    }
}
```

### 2. Use cfg_attr for Metadata

```rust
// Bad
#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct Config {
    // ...
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
pub struct Config {
    // ...
}

// Good
#[derive(Debug, Clone)]
#[cfg_attr(target_os = "linux", derive(Serialize))]
pub struct Config {
    // ...
}
```

### 3. Minimize Conditional Nesting

```rust
// Bad: Nested conditionals
#[cfg(target_os = "linux")]
{
    #[cfg(target_arch = "x86_64")]
    {
        #[cfg(feature = "avx2")]
        {
            // Deeply nested
        }
    }
}

// Good: Combine conditions
#[cfg(all(target_os = "linux", target_arch = "x86_64", feature = "avx2"))]
fn optimized_function() {
    // Clear condition
}
```

### 4. Document Conditional Code

```rust
/// Process data with platform-specific optimizations
///
/// # Platform-Specific Behavior
///
/// - **Linux x86_64 with AVX2**: Uses AVX2 SIMD for 2-3x speedup
/// - **Linux ARM64 with NEON**: Uses NEON SIMD for 1.5-2x speedup
/// - **macOS/Windows**: Falls back to generic implementation
#[cfg(target_os = "linux")]
pub fn process_optimized(data: &[u8]) -> Vec<u8> {
    // ...
}
```

### 5. Test All Paths

```rust
#[cfg(test)]
mod tests {
    // Test all conditional paths
    
    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_path() { /* ... */ }
    
    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_path() { /* ... */ }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    #[test]
    fn test_generic_path() { /* ... */ }
}
```

---

## Common Patterns

### Platform-Specific Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(target_os = "linux")]
    #[error("io_uring error: {0}")]
    IoUring(#[from] io_uring::Error),
    
    #[cfg(target_os = "linux")]
    #[error("KVM error: {0}")]
    Kvm(#[from] kvm_ioctls::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Platform not supported: {0}")]
    UnsupportedPlatform(String),
}
```

### Platform-Specific Initialization

```rust
pub struct RuntimeConfig {
    pub thread_count: usize,
    pub memory_limit: usize,
}

impl RuntimeConfig {
    pub fn default_for_platform() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                thread_count: num_cpus::get(),
                memory_limit: 1024 * 1024 * 1024,  // 1 GB
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            Self {
                thread_count: num_cpus::get(),
                memory_limit: 512 * 1024 * 1024,  // 512 MB (no VMs)
            }
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self {
                thread_count: 4,
                memory_limit: 256 * 1024 * 1024,  // 256 MB
            }
        }
    }
}
```

---

## Conclusion

Conditional compilation in Project Aether follows these principles:

1. **Prefer compile-time checks** for platform-specific features
2. **Use runtime detection** for hardware-dependent optimizations
3. **Abstract platform differences** behind traits
4. **Document conditional behavior** clearly
5. **Test all code paths** in CI/CD

This strategy ensures:
- Single codebase for multiple platforms
- Optimal performance on each platform
- Clear separation of platform-specific code
- Maintainable and testable codebase
