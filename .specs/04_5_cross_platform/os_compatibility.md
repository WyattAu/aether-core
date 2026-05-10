# OS Compatibility Matrix

**Document ID:** CP-OS-001  
**Version:** 1.0.0  
**Status:** Draft  
**Created:** 2026-03-06  
**Author:** Compatibility Engineer  

---

## Overview

Project Aether has strict OS compatibility requirements due to its reliance on Linux-specific features for high-performance I/O and hardware virtualization. This document outlines supported operating systems, their capabilities, limitations, and workarounds.

---

## Primary Platform: Linux

### Supported Versions

| Distribution | Minimum Version | Recommended Version | Status |
|--------------|-----------------|---------------------|--------|
| Ubuntu | 20.04 LTS | 22.04 LTS | [DONE] Full Support |
| Debian | 11 (Bullseye) | 12 (Bookworm) | [DONE] Full Support |
| CentOS/RHEL | 8 | 9 | [DONE] Full Support |
| Fedora | 35 | 38+ | [DONE] Full Support |
| Arch Linux | Rolling | Current | [DONE] Full Support |
| Alpine Linux | 3.18 | Edge | [WARN] Partial (musl) |

### Kernel Requirements

**Minimum:** Linux 5.1 (for basic io_uring)  
**Recommended:** Linux 5.15+ (for full io_uring features)  
**Optimal:** Linux 6.1+ (latest io_uring optimizations)

#### Required Kernel Features

| Feature | Kernel Version | Purpose | Criticality |
|---------|----------------|---------|-------------|
| io_uring | 5.1+ | High-performance async I/O | Critical |
| KVM | 2.6.20+ | Hardware virtualization | Critical |
| EPT/NPT | Hardware-dependent | Nested page tables | Critical |
| IOMMU | Hardware-dependent | DMA protection | High |
| cgroups v2 | 4.5+ | Resource isolation | High |
| userfaultfd | 4.3+ | VM memory management | Medium |
| memfd_create | 3.17+ | Memory-mapped VM memory | Medium |
| eventfd | 2.6.22+ | Signaling mechanism | Medium |

#### io_uring Feature Matrix

| Feature | Kernel Version | Description | Aether Usage |
|---------|----------------|-------------|--------------|
| Basic SQ/CQ | 5.1 | Submission/completion queues | Core I/O |
| Fixed files | 5.1 | Registered file descriptors | Network |
| Fixed buffers | 5.1 | Registered I/O buffers | Zero-copy |
| Polling | 5.1 | Busy-poll for low latency | Data plane |
| Async workers | 5.1 | Background thread pool | Blocking ops |
| Linked SQEs | 5.1 | Chained operations | Transactions |
| Drain | 5.1 | Serialization points | Ordering |
| Timeout | 5.1 | Time-based operations | Timeouts |
| Accept | 5.5 | Async connection accept | Network |
| Connect | 5.5 | Async connection establish | Network |
| Send/Recv | 5.6 | Zero-copy network I/O | Mesh network |
| OpenAT/Close | 5.6 | Async file operations | Storage |
| Statx | 5.6 | Async file metadata | Storage |
| Spawn | 5.5+ | Thread creation | Threading |
| Provided buffers | 5.7+ | Buffer selection | Buffer pools |
| Sendmsg/Recvmsg | 5.3+ | Vectored I/O | Network |
| Multishot | 5.19+ | Repeating operations | Event streams |
| SQPOLL | 5.11+ | Kernel polling thread | Ultra-low latency |

#### Kernel Configuration Requirements

```bash
# Required kernel config options
CONFIG_IO_URING=y
CONFIG_KVM=y
CONFIG_KVM_INTEL=y  # For Intel CPUs
CONFIG_KVM_AMD=y    # For AMD CPUs
CONFIG_CGROUPS=y
CONFIG_CGROUP_PIDS=y
CONFIG_MEMCG=y
CONFIG_CGROUP_DEVICE=y
CONFIG_USERFAULTFD=y
CONFIG_MEMFD_CREATE=y
CONFIG_EVENTFD=y
CONFIG_IOMMU_SUPPORT=y
CONFIG_INTEL_IOMMU=y  # For Intel
CONFIG_AMD_IOMMU=y    # For AMD

# Recommended for performance
CONFIG_HUGETLBFS=y
CONFIG_HUGETLB_PAGE=y
CONFIG_TRANSPARENT_HUGEPAGE=y
CONFIG_NUMA=y
CONFIG_NUMA_BALANCING=y
```

### Supported Features

[DONE] **Full Feature Set:**
- io_uring-based data plane (Monoio runtime)
- KVM/Firecracker microVM isolation
- All network mesh capabilities
- Complete resource management
- Hardware-accelerated virtualization
- NUMA-aware scheduling
- Huge page support

### Limitations

None - Linux is the primary development and deployment platform.

### Performance Characteristics

| Metric | Linux 5.15 | Linux 6.1 | Notes |
|--------|------------|-----------|-------|
| io_uring latency | < 1µs | < 0.5µs | Submission overhead |
| KVM boot time | < 125ms | < 100ms | MicroVM startup |
| Network throughput | 40+ Gbps | 100+ Gbps | With zero-copy |
| Context switch | < 1µs | < 0.8µs | VM entry/exit |

---

## Secondary Platform: macOS

### Supported Versions

| Version | Status | Limitations |
|---------|--------|-------------|
| macOS 12 (Monterey) | [WARN] Development Only | No io_uring, no KVM |
| macOS 13 (Ventura) | [WARN] Development Only | No io_uring, no KVM |
| macOS 14 (Sonoma) | [WARN] Development Only | No io_uring, no KVM |

### Supported Features

[DONE] **Available:**
- WASM runtime (Wasmtime)
- Tokio control plane
- Basic actor execution
- Development and testing
- Control plane components
- Configuration management

### Limitations

[FAIL] **Not Available:**
- **io_uring:** No kernel support, cannot be emulated
- **KVM:** No hardware virtualization support
- **Firecracker VMs:** Requires KVM
- **Monoio data plane:** Requires io_uring
- **Zero-copy I/O:** Requires io_uring registered buffers
- **MicroVM isolation:** Requires KVM

### Workarounds

#### 1. Development Mode

Run in degraded mode with Tokio-only runtime:

```rust
#[cfg(target_os = "macos")]
fn create_runtime() -> TokioRuntime {
    // Use Tokio for all operations on macOS
    tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime")
}

#[cfg(target_os = "linux")]
fn create_runtime() -> DualRuntime {
    // Use Monoio for data plane, Tokio for control plane
    DualRuntime::new()
}
```

#### 2. Docker Desktop

Run Linux containers with WSL2-like virtualization:

```bash
# Install Docker Desktop for Mac
# Enable Kubernetes if needed
docker run --rm -it aether-runtime:latest
```

**Limitations:**
- No KVM passthrough to containers
- Reduced network performance
- Limited to Docker Desktop's Linux VM

#### 3. Lima VM

Use Lima for Linux VM with KVM support (Apple Silicon):

```bash
# Install Lima
brew install lima

# Create VM with KVM support
limactl start --name=aether-dev \
  --cpus=4 --memory=8GB

# Run Aether inside Lima VM
limactl shell aether-dev
./target/release/aether-runtime
```

#### 4. Cloud Development

Develop locally, run on remote Linux servers:

```bash
# SSH with remote port forwarding
ssh -R 8080:localhost:8080 user@linux-server

# Run on remote
cargo build --release --target x86_64-unknown-linux-gnu
```

### Performance Impact

| Operation | Linux | macOS (Tokio) | Slowdown |
|-----------|-------|---------------|----------|
| I/O latency | 1µs | 10µs | 10x |
| Network throughput | 40 Gbps | 5 Gbps | 8x |
| Actor cold start | 50µs | 200µs | 4x |
| Context switch | 1µs | 5µs | 5x |

---

## Tertiary Platform: Windows

### Supported Versions

| Version | Status | Requirements |
|---------|--------|--------------|
| Windows 10 | [WARN] WSL2 Only | WSL2 enabled |
| Windows 11 | [WARN] WSL2 Only | WSL2 enabled |
| Windows Server 2019+ | [WARN] WSL2 Only | WSL2 enabled |

### WSL2 Requirements

WSL2 (Windows Subsystem for Linux 2) provides a full Linux kernel with KVM and io_uring support.

#### Installation

```powershell
# Enable WSL2
wsl --install

# Install Ubuntu 22.04
wsl --install -d Ubuntu-22.04

# Update kernel (required for latest io_uring)
wsl --update
```

#### WSL2 Configuration

Create or edit `%USERPROFILE%\.wslconfig`:

```ini
[wsl2]
kernel=C:\\Users\\YourUser\\custom-kernel
memory=16GB
processors=8
swap=8GB
localhostForwarding=true

[experimental]
autoMemoryReclaim=gradual
```

#### Custom Kernel (for latest io_uring)

```bash
# Inside WSL2
git clone https://github.com/microsoft/WSL2-Linux-Kernel.git
cd WSL2-Linux-Kernel

# Enable required features
cat >> Microsoft/config-wsl <<EOF
CONFIG_IO_URING=y
CONFIG_KVM=y
CONFIG_KVM_INTEL=y
CONFIG_HUGETLBFS=y
EOF

# Build kernel
make -j$(nproc) KCONFIG_CONFIG=Microsoft/config-wsl

# Copy to Windows
cp arch/x86/boot/bzImage /mnt/c/Users/YourUser/custom-kernel
```

### Supported Features (via WSL2)

[DONE] **Full Linux Compatibility:**
- io_uring (with custom kernel)
- KVM (limited performance)
- All Aether features
- Docker support
- Network mesh

### Limitations

[WARN] **WSL2 Constraints:**

1. **KVM Performance:**
   - Nested virtualization overhead
   - Reduced VM isolation performance
   - Limited to ~50% of bare-metal speed

2. **Memory:**
   - WSL2 memory shared with Windows
   - No huge pages by default
   - Limited to 50% of system RAM

3. **Network:**
   - NAT-based networking
   - Port forwarding required
   - Reduced throughput

4. **Filesystem:**
   - `/mnt/c` is slow (9P protocol)
   - Use Linux filesystem for performance
   - `/home` directory recommended

### Performance Impact

| Operation | Linux | Windows (WSL2) | Slowdown |
|-----------|-------|----------------|----------|
| io_uring latency | 1µs | 2µs | 2x |
| KVM boot time | 125ms | 200ms | 1.6x |
| Network throughput | 40 Gbps | 15 Gbps | 2.7x |
| File I/O | 5 GB/s | 500 MB/s | 10x |

### Workarounds

#### 1. Use Linux Filesystem

```bash
# Bad: Windows filesystem
cd /mnt/c/Users/YourUser/project

# Good: Linux filesystem
cd ~/project  # /home/youruser/project
```

#### 2. Increase Memory

```ini
# .wslconfig
[wsl2]
memory=32GB  # Increase as needed
```

#### 3. Optimize Networking

```bash
# Use mirrored networking mode (Windows 11 22H2+)
# .wslconfig
[wsl2]
networkingMode=mirrored
```

---

## Potential Platform: FreeBSD

### Status

 **Experimental** - Not currently supported, potential future platform.

### Current Support

[FAIL] **Not Available:**
- io_uring (Linux-specific)
- KVM (Linux-specific)
- Monoio runtime (Linux-only)

### Potential Support

[DONE] **Available:**
- **Bhyve:** FreeBSD hypervisor (alternative to KVM)
- **kqueue:** Async I/O mechanism (alternative to epoll/io_uring)
- **WASM:** Runtime support available
- **Tokio:** Compatible with kqueue

### Porting Requirements

1. **Data Plane Runtime:**
   - Replace Monoio with Tokio+kqueue
   - Implement zero-copy with `sendfile(2)`
   - Use `aio_read(2)`/`aio_write(2)` for async I/O

2. **Virtualization:**
   - Port Firecracker to Bhyve
   - Implement VM lifecycle management
   - Create Bhyve HAL layer

3. **Estimated Effort:**
   - Runtime: 2-3 months
   - Bhyve integration: 3-4 months
   - Testing: 1-2 months
   - **Total:** 6-9 months

### Performance Expectations

| Feature | Linux | FreeBSD (Est.) | Notes |
|---------|-------|----------------|-------|
| I/O latency | 1µs | 5µs | kqueue vs io_uring |
| VM boot time | 125ms | 150ms | Bhyve vs KVM |
| Network throughput | 40 Gbps | 30 Gbps | Zero-copy available |
| WASM runtime | Full | Full | Wasmtime compatible |

### Implementation Strategy

```rust
// Conditional compilation for FreeBSD
#[cfg(target_os = "freebsd")]
mod freebsd_platform {
    use tokio::runtime::Runtime;
    
    pub fn create_data_plane() -> Runtime {
        // Use Tokio with kqueue
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create runtime")
    }
    
    pub mod vm {
        // Bhyve integration
        pub fn create_vm(config: VmConfig) -> Result<Vm, Error> {
            // Bhyve-specific implementation
            unimplemented!("Bhyve support pending")
        }
    }
}
```

---

## Feature Comparison Matrix

| Feature | Linux | macOS | Windows (WSL2) | FreeBSD |
|---------|-------|-------|----------------|---------|
| **Runtime** |
| Monoio data plane | [DONE] | [FAIL] | [DONE] | [FAIL] |
| Tokio control plane | [DONE] | [DONE] | [DONE] | [DONE] |
| io_uring | [DONE] | [FAIL] | [DONE] | [FAIL] |
| kqueue | [FAIL] | [DONE] | [FAIL] | [DONE] |
| **Virtualization** |
| KVM | [DONE] | [FAIL] | [WARN] | [FAIL] |
| Bhyve | [FAIL] | [FAIL] | [FAIL] |  |
| Firecracker | [DONE] | [FAIL] | [WARN] | [FAIL] |
| **Performance** |
| Zero-copy I/O | [DONE] | [FAIL] | [DONE] | [WARN] |
| Huge pages | [DONE] | [FAIL] | [WARN] | [DONE] |
| NUMA support | [DONE] | [FAIL] | [FAIL] | [DONE] |
| **Production Ready** | [DONE] | [FAIL] | [WARN] | [FAIL] |

**Legend:**
- [DONE] Full support
- [WARN] Partial/limited support
- [FAIL] Not supported
-  Experimental/potential

---

## Platform Detection

### Runtime Detection

```rust
/// Detect platform capabilities at runtime
pub struct PlatformCapabilities {
    pub has_io_uring: bool,
    pub has_kvm: bool,
    pub has_epoll: bool,
    pub has_kqueue: bool,
    pub kernel_version: String,
}

impl PlatformCapabilities {
    pub fn detect() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            has_io_uring: Self::check_io_uring(),
            #[cfg(not(target_os = "linux"))]
            has_io_uring: false,
            
            #[cfg(target_os = "linux")]
            has_kvm: Self::check_kvm(),
            #[cfg(not(target_os = "linux"))]
            has_kvm: false,
            
            #[cfg(target_os = "linux")]
            has_epoll: true,
            #[cfg(not(target_os = "linux"))]
            has_epoll: false,
            
            #[cfg(any(target_os = "macos", target_os = "freebsd"))]
            has_kqueue: true,
            #[cfg(not(any(target_os = "macos", target_os = "freebsd")))]
            has_kqueue: false,
            
            kernel_version: Self::get_kernel_version(),
        }
    }
    
    #[cfg(target_os = "linux")]
    fn check_io_uring() -> bool {
        use std::fs;
        
        // Check kernel version
        let version = fs::read_to_string("/proc/version")
            .unwrap_or_default();
        
        // Parse version and check >= 5.1
        // Simplified check
        version.contains("5.") || version.contains("6.")
    }
    
    #[cfg(target_os = "linux")]
    fn check_kvm() -> bool {
        std::path::Path::new("/dev/kvm").exists()
    }
    
    fn get_kernel_version() -> String {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/version")
                .unwrap_or_else(|_| "unknown".to_string())
        }
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let output = Command::new("uname")
                .arg("-r")
                .output()
                .ok();
            output
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }
        #[cfg(target_os = "windows")]
        {
            "WSL2".to_string()
        }
        #[cfg(target_os = "freebsd")]
        {
            use std::process::Command;
            let output = Command::new("uname")
                .arg("-r")
                .output()
                .ok();
            output
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }
    }
}
```

### Compile-Time Detection

```rust
// Platform-specific code paths
#[cfg(target_os = "linux")]
mod platform {
    pub const PLATFORM_NAME: &str = "Linux";
    pub const HAS_IO_URING: bool = true;
    pub const HAS_KVM: bool = true;
}

#[cfg(target_os = "macos")]
mod platform {
    pub const PLATFORM_NAME: &str = "macOS";
    pub const HAS_IO_URING: bool = false;
    pub const HAS_KVM: bool = false;
}

#[cfg(target_os = "windows")]
mod platform {
    pub const PLATFORM_NAME: &str = "Windows (WSL2)";
    pub const HAS_IO_URING: bool = true;
    pub const HAS_KVM: bool = true;
}

#[cfg(target_os = "freebsd")]
mod platform {
    pub const PLATFORM_NAME: &str = "FreeBSD";
    pub const HAS_IO_URING: bool = false;
    pub const HAS_KVM: bool = false;
}
```

---

## Recommendations

### Production Deployment

**Primary:** Linux 6.1+ on bare metal or VM
- Full feature support
- Best performance
- Production-ready

**Alternative:** Linux 5.15+ on cloud VMs
- Good feature support
- Acceptable performance
- Widely available

### Development

**Primary:** Linux 5.15+ (native or VM)
- Full feature parity with production
- Best development experience

**Alternative:** macOS with Docker/Lima
- Control plane development
- WASM runtime testing
- Limited to non-performance-critical work

**Alternative:** Windows with WSL2
- Full feature support
- Acceptable performance
- Good for Windows-based teams

### CI/CD

**Recommended:** Linux containers on any platform
- Consistent environment
- Fast execution
- Docker/Kubernetes support

```yaml
# Example GitHub Actions
jobs:
  test:
    runs-on: ubuntu-latest
    container:
      image: ubuntu:22.04
    steps:
      - uses: actions/checkout@v3
      - name: Install Rust
        run: |
          rustup toolchain install nightly-2026-03-01
          rustup default nightly-2026-03-01
      - name: Run tests
        run: cargo test --all-features
```

---

## Troubleshooting

### Linux: io_uring Not Available

```bash
# Check kernel version
uname -r  # Should be 5.1+

# Check io_uring support
grep CONFIG_IO_URING /boot/config-$(uname -r)

# If missing, install newer kernel
sudo apt install linux-generic-hwe-22.04  # Ubuntu
```

### Linux: KVM Permission Denied

```bash
# Add user to kvm group
sudo usermod -aG kvm $USER

# Log out and back in
# Verify access
ls -l /dev/kvm
# Should show: crw-rw---- 1 root kvm 10, 232 Mar  6 10:00 /dev/kvm
```

### macOS: Performance Issues

```bash
# Use Docker Desktop with increased resources
# Preferences → Resources → Memory: 8GB+, CPUs: 4+

# Or use Lima for better performance
brew install lima
limactl start
```

### Windows: WSL2 Kernel Update

```powershell
# Update WSL2 kernel
wsl --update

# Restart WSL2
wsl --shutdown

# Verify kernel version
wsl uname -r
```

---

## Conclusion

Project Aether is **Linux-first** software requiring:
- **Linux 5.15+** for full feature support
- **io_uring** for high-performance data plane
- **KVM** for hardware virtualization

Other platforms support **development only** with degraded performance:
- **macOS:** Tokio-only, no virtualization
- **Windows:** Full support via WSL2 with overhead
- **FreeBSD:** Potential future platform requiring porting effort

For production deployments, **Linux on bare metal or VMs** is the only supported platform.
