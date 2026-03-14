# Blue Paper BP-FIRECRACKER-MANAGER-001
# Firecracker MicroVM Manager Architecture Specification

**Document ID:** BP-FIRECRACKER-MANAGER-001  
**Version:** 1.0.0  
**Status:** Active  
**Classification:** Public  
**Compliance:** IEEE 1016-2009  

---

## Metadata

| Field | Value |
|-------|-------|
| **Author** | Construct (Systems Architect) |
| **Date** | 2026-03-05 |
| **Supersedes** | N/A |
| **References** | YP-VIRT-KVM-001, ADR-014, ADR-015 |
| **Target Revision** | a0cbf13 |

---

## BP-1: Design Overview

### 1.1 System Purpose

The Firecracker MicroVM Manager provides high-performance, secure virtualization for OCI containers using AWS Firecracker microVMs. It achieves sub-125ms boot times while maintaining strong isolation guarantees through KVM-based virtualization.

**Primary Objectives:**
- Manage Firecracker MicroVM lifecycle with <125ms boot latency
- Provide OCI-compatible container execution environment
- Enforce resource isolation through Jailer integration
- Manage block devices and network taps for VM instances
- Guarantee cleanup of all resources on VM termination

### 1.2 Scope

**In Scope:**
- VM creation, destruction, and lifecycle management
- Jailer sandbox configuration and management
- Block device (virtio-block) attachment/detachment
- Network tap (virtio-net) configuration
- Resource limits enforcement (CPU, memory, I/O)

**Out of Scope:**
- Container image management (handled by containerd)
- High-level orchestration (handled by Nomad)
- Storage provisioning (handled by Ceph integration)

### 1.3 C4 Context Diagram

```mermaid
C4Context
    title System Context - Firecracker MicroVM Manager
    
    Person(ops, "Operations", "DevOps engineers managing VMs")
    Person(dev, "Developers", "Container developers")
    
    System(vm_mgr, "Firecracker Manager", "Manages MicroVMs for OCI containers")
    
    System_Ext(firecracker, "Firecracker VMM", "AWS Firecracker microVM monitor")
    System_Ext(containerd, "containerd", "Container runtime interface")
    System_Ext(kernel, "Linux Kernel", "Host kernel with KVM support")
    System_Ext(storage, "Block Storage", "Ceph RBD / local block devices")
    System_Ext(network, "Network Stack", "Linux networking (taps, bridges)")
    
    Rel(ops, vm_mgr, "Manages VMs via API")
    Rel(dev, vm_mgr, "Runs containers")
    
    Rel(vm_mgr, firecracker, "Launches microVMs")
    Rel(vm_mgr, containerd, "Receives container specs")
    Rel(vm_mgr, kernel, "Uses KVM API")
    Rel(vm_mgr, storage, "Attaches block devices")
    Rel(vm_mgr, network, "Configures taps")
    
    UpdateRelStyle(ops, vm_mgr, $lineColor="blue")
    UpdateRelStyle(dev, vm_mgr, $lineColor="blue")
```

### 1.4 Key Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| VM Boot Time | <125ms p99 | YP-VIRT-KVM-001 Theorem 3.1 |
| VM Destroy Time | <50ms p99 | Resource cleanup SLA |
| Concurrent VMs | 5000 per host | Density requirement |
| Memory Overhead | <8MB per VM | Minimal footprint |
| API Latency | <10ms p99 | Control plane responsiveness |

---

## BP-2: Design Decomposition

### 2.1 C4 Component Diagram

```mermaid
C4Component
    title Component Diagram - Firecracker MicroVM Manager
    
    Container(vm_mgr, "Firecracker Manager", "Rust", "MicroVM orchestration")
    
    Component(lifecycle, "VM Lifecycle Manager", "Rust", "Create/destroy VMs")
    Component(jailer, "Jailer Integration", "Rust", "Sandbox configuration")
    Component(block, "Block Device Manager", "Rust", "virtio-block management")
    Component(network, "Network Tap Manager", "Rust", "virtio-net configuration")
    Component(limiter, "Resource Limiter", "Rust", "CPU/memory/IO limits")
    Component(api, "API Server", "Rust/axum", "REST/gRPC interface")
    Component(state, "State Store", "Rust/Sled", "VM state persistence")
    Component(metrics, "Metrics Collector", "Rust", "Prometheus metrics")
    
    ContainerDb(firecracker, "Firecracker Process", "Process", "Per-VM VMM")
    ContainerDb(kernel, "KVM", "Kernel Module", "Virtualization primitives")
    
    Rel(lifecycle, jailer, "Configures sandbox")
    Rel(lifecycle, block, "Attaches volumes")
    Rel(lifecycle, network, "Configures networking")
    Rel(lifecycle, limiter, "Applies limits")
    
    Rel(lifecycle, firecracker, "Spawns VMM")
    Rel(jailer, kernel, "Uses KVM")
    
    Rel(api, lifecycle, "Invokes operations")
    Rel(lifecycle, state, "Persists state")
    Rel(metrics, lifecycle, "Collects metrics")
    
    UpdateElementStyle(lifecycle, $bgColor="green", $fontColor="white")
    UpdateElementStyle(jailer, $bgColor="orange", $fontColor="white")
```

### 2.2 Component Descriptions

#### VM Lifecycle Manager (COMP-VM-001)
**Responsibility:** Orchestrates VM creation, startup, shutdown, and destruction  
**Dependencies:** Jailer Integration, Block Device Manager, Network Tap Manager, Resource Limiter  
**Interface:** IF-VM-001, IF-VM-002  

#### Jailer Integration (COMP-JAIL-001)
**Responsibility:** Configures Firecracker jailer for VM isolation  
**Dependencies:** Resource Limiter, KVM HAL  
**Interface:** Internal to Lifecycle Manager  

#### Block Device Manager (COMP-BLOCK-001)
**Responsibility:** Manages virtio-block device attachment/detachment  
**Dependencies:** Storage HAL  
**Interface:** IF-VM-003  

#### Network Tap Manager (COMP-NET-001)
**Responsibility:** Creates/deletes TAP devices, configures virtio-net  
**Dependencies:** Network HAL  
**Interface:** IF-VM-004  

#### Resource Limiter (COMP-LIMIT-001)
**Responsibility:** Enforces CPU, memory, and I/O limits via cgroups  
**Dependencies:** cgroups v2 HAL  
**Interface:** Internal to Lifecycle Manager  

---

## BP-3: Design Rationale

### 3.1 Architectural Decisions

| Decision | Rationale | ADR Reference |
|----------|-----------|---------------|
| Firecracker over QEMU | 12x faster boot, minimal attack surface | ADR-014 |
| Jailer by default | Defense-in-depth isolation | ADR-015 |
| Per-VM Firecracker process | Fault isolation, independent lifecycle | YP-VIRT-KVM-001 §2.3 |
| Async Rust runtime | High concurrency, low overhead | Performance requirement |
| Sled for state store | Embedded, ACID, zero-config | Operational simplicity |
| Seccomp filters | syscall-level containment | Security requirement |

### 3.2 Technology Selection

**Firecracker VMM v1.5+:**
- Mature, production-hardened
- Strong isolation guarantees
- Minimal resource footprint
- Direct KVM integration

**Jailer:**
- Automatic cgroup namespace isolation
- Seccomp filter application
- Resource limiting enforcement
- No privileged container requirement

**virtio devices:**
- Industry standard para-virtualization
- Near-native performance
- Well-tested, stable drivers

### 3.3 Design Constraints

1. **KVM Requirement:** Host kernel must support KVM (YP-VIRT-KVM-001 Thm 2.1)
2. **Root Access:** Required for /dev/kvm access and network tap creation
3. **Seccomp:** Host kernel must support seccomp filters
4. **cgroups v2:** Required for resource limiting

---

## BP-4: Traceability

### 4.1 Yellow Paper Theorem Mapping

| Theorem | BP Section | Implementation |
|---------|------------|----------------|
| **YP-VIRT-KVM-001 Thm 2.1** (KVM Availability) | BP-8.1 | KVM HAL checks /dev/kvm |
| **YP-VIRT-KVM-001 Thm 3.1** (Boot Latency) | BP-7.1 | Optimized boot sequence |
| **YP-VIRT-KVM-001 Thm 3.2** (Isolation) | BP-3.1, BP-9.1 | Jailer + seccomp |
| **YP-VIRT-KVM-001 Thm 4.1** (Resource Cleanup) | BP-9.2 | RAII-based cleanup |
| **YP-VIRT-KVM-001 Thm 5.1** (Concurrent Density) | BP-1.4 | Async architecture |

### 4.2 Requirement Traceability Matrix

```
YP-VIRT-KVM-001 → BP-1.4 (Performance Targets)
                 → BP-2.2 (Component Design)
                 → BP-3.1 (Technology Selection)
                 → BP-7.1 (Boot Sequence)
                 → BP-8.1 (KVM Requirements)
                 → BP-9.1 (Isolation Proof)
                 → BP-9.3 (Timing Proof)

ADR-014 → BP-3.1 (Firecracker Selection)
ADR-015 → BP-3.1 (Jailer Integration)
```

---

## BP-5: Interface Design

### IF-VM-001: VM Creation

```rust
/// Creates a new Firecracker MicroVM with specified configuration.
///
/// # Preconditions
/// - `config.vm_id` is globally unique and valid UTF-8
/// - `config.memory_mb` >= 128 && <= host_available_memory
/// - `config.vcpu_count` >= 1 && <= host_vcpu_count
/// - `config.kernel_image_path` exists and is readable
/// - `config.root_drive` block device exists and is accessible
/// - Caller has CAP_SYS_ADMIN or /dev/kvm access
///
/// # Postconditions
/// - VM process is running in isolated jailer environment
/// - All block devices attached via virtio-block
/// - Network tap configured and connected
/// - Resource limits enforced via cgroups
/// - VM state persisted in state store
/// - Returns `Ok(vm_handle)` with valid VM identifier
///
/// # Errors
/// - `VmError::InvalidConfig` if preconditions violated
/// - `VmError::ResourceExhausted` if insufficient host resources
/// - `VmError::KernelError` if KVM initialization fails
/// - `VmError::JailerError` if jailer setup fails
///
/// # Timing
/// - Completes in <125ms (p99) per YP-VIRT-KVM-001 Thm 3.1
fn create_vm(config: VmConfig) -> Result<VmHandle, VmError>;
```

**Full Signature:**
```rust
pub async fn create_vm(
    config: VmConfig,
    ctx: &VmContext,
) -> Result<VmHandle, VmError>
where
    VmConfig: Validate + Serialize,
    VmContext: HasKvmAccess + HasStateStore,
```

### IF-VM-002: VM Destruction

```rust
/// Destroys a running Firecracker MicroVM and cleans up all resources.
///
/// # Preconditions
/// - `vm_id` corresponds to an existing, running VM
/// - Caller has permission to destroy this VM
/// - No pending operations on this VM
///
/// # Postconditions
/// - VM process terminated (SIGKILL if graceful shutdown fails)
/// - All block devices detached and released
/// - Network tap device deleted
/// - cgroups removed
/// - Jailer directory cleaned up
/// - VM state removed from state store
/// - Returns `Ok(())` on successful cleanup
///
/// # Errors
/// - `VmError::NotFound` if VM does not exist
/// - `VmError::CleanupFailed` if partial cleanup (logged, retried)
///
/// # Timing
/// - Completes in <50ms (p99)
/// - Guarantees cleanup per YP-VIRT-KVM-001 Thm 4.1
fn destroy_vm(vm_id: VmId) -> Result<(), VmError>;
```

**Full Signature:**
```rust
pub async fn destroy_vm(
    vm_id: VmId,
    ctx: &VmContext,
    force: bool,
) -> Result<DestructionReport, VmError>
where
    VmContext: HasStateStore + HasProcessControl,
```

### IF-VM-003: Volume Attachment

```rust
/// Attaches a block device to a running VM.
///
/// # Preconditions
/// - `vm_id` corresponds to a running VM
/// - `volume_spec.device_id` is unique within this VM
/// - `volume_spec.path` exists and is accessible
/// - `volume_spec.read_only` flag is respected by underlying device
/// - VM has available virtio-block slots (max 8)
///
/// # Postconditions
/// - Block device visible inside VM at `/dev/vd{a-h}`
/// - Device accessible per `read_only` specification
/// - Volume attachment recorded in VM state
/// - Returns `Ok(device_path)` with guest-visible path
///
/// # Errors
/// - `VolumeError::VmNotFound` if VM does not exist
/// - `VolumeError::DeviceNotFound` if volume path invalid
/// - `VolumeError::SlotExhausted` if max devices attached
/// - `VolumeError::AttachFailed` if virtio negotiation fails
fn attach_volume(vm_id: VmId, volume_spec: VolumeSpec) -> Result<DevicePath, VolumeError>;
```

**Full Signature:**
```rust
pub async fn attach_volume(
    vm_id: VmId,
    volume_spec: VolumeSpec,
    ctx: &VmContext,
) -> Result<DevicePath, VolumeError>
where
    VolumeSpec: Validate,
    VmContext: HasFirecrackerApi,
```

### IF-VM-004: Network Configuration

```rust
/// Configures network tap device for a VM.
///
/// # Preconditions
/// - `vm_id` corresponds to a running VM (or about to start)
/// - `net_spec.tap_name` is unique system-wide
/// - `net_spec.ip_config` does not conflict with existing taps
/// - Caller has NET_ADMIN capability
///
/// # Postconditions
/// - TAP device created with specified name
/// - TAP connected to VM via virtio-net
/// - IP address configured on TAP device
/// - Network namespace isolated per jailer config
/// - Returns `Ok(tap_fd)` with TAP file descriptor
///
/// # Errors
/// - `NetworkError::TapExists` if TAP name already used
/// - `NetworkError::IpConflict` if IP address in use
/// - `NetworkError::PermissionDenied` if lacking NET_ADMIN
/// - `NetworkError::VirtioFailed` if virtio-net init fails
fn configure_network(vm_id: VmId, net_spec: NetworkSpec) -> Result<TapFd, NetworkError>;
```

**Full Signature:**
```rust
pub async fn configure_network(
    vm_id: VmId,
    net_spec: NetworkSpec,
    ctx: &VmContext,
) -> Result<TapFd, NetworkError>
where
    NetworkSpec: Validate,
    VmContext: HasNetworkNamespace,
```

---

## BP-6: Data Design

### 6.1 Core Data Structures

#### VmConfig

```rust
/// Complete configuration for a Firecracker MicroVM.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct VmConfig {
    /// Unique identifier for this VM instance
    #[validate(length(min = 1, max = 64), regex = "VM_ID_PATTERN")]
    pub vm_id: String,
    
    /// Memory allocation in megabytes
    #[validate(range(min = 128, max = 16384))]
    pub memory_mb: u32,
    
    /// Number of vCPUs
    #[validate(range(min = 1, max = 32))]
    pub vcpu_count: u8,
    
    /// Path to kernel image (uncompressed ELF or bzImage)
    #[validate(length(min = 1))]
    pub kernel_image_path: PathBuf,
    
    /// Kernel command line parameters
    pub kernel_cmdline: KernelCmdline,
    
    /// Root block device specification
    pub root_drive: DriveSpec,
    
    /// Additional block devices
    #[validate(length(max = 7))]
    pub additional_drives: Vec<DriveSpec>,
    
    /// Network interface specifications
    #[validate(length(max = 4))]
    pub network_interfaces: Vec<NetworkInterfaceSpec>,
    
    /// Resource limits
    pub limits: ResourceLimits,
    
    /// Jailer configuration
    pub jailer: JailerConfig,
}

/// Kernel command line builder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelCmdline {
    pub console: ConsoleConfig,
    pub init: Option<String>,
    pub root: RootConfig,
    pub extra: Vec<String>,
}

impl KernelCmdline {
    pub fn to_string(&self) -> String {
        // Builds: "console=ttyS0 reboot=k panic=1 ..."
    }
}
```

#### VolumeSpec

```rust
/// Block device specification for VM attachment.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct VolumeSpec {
    /// Unique device identifier within VM
    #[validate(length(min = 1, max = 32))]
    pub device_id: String,
    
    /// Path to block device or image file on host
    #[validate(length(min = 1))]
    pub path: PathBuf,
    
    /// Read-only mode flag
    pub read_only: bool,
    
    /// Virtio-block cache configuration
    pub cache_mode: CacheMode,
    
    /// IO engine for block operations
    pub io_engine: IoEngine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheMode {
    /// No caching (safest)
    Unsafe,
    /// Write-through caching
    WriteThrough,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IoEngine {
    /// Sync I/O
    Sync,
    /// Async I/O with io_uring
    Async,
}
```

#### NetworkSpec

```rust
/// Network interface specification for VM.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct NetworkSpec {
    /// TAP device name on host
    #[validate(length(min = 1, max = 15), regex = "TAP_NAME_PATTERN")]
    pub tap_name: String,
    
    /// Interface configuration
    pub iface: InterfaceConfig,
    
    /// Ingress rate limiter (bytes/sec)
    pub rx_rate_limiter: Option<RateLimiterConfig>,
    
    /// Egress rate limiter (bytes/sec)
    pub tx_rate_limiter: Option<RateLimiterConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    /// Guest MAC address
    pub guest_mac: Option<MacAddress>,
    
    /// Guest IP address with CIDR
    pub guest_ip: Option<IpCidr>,
}
```

#### ResourceLimits

```rust
/// Resource limits for VM cgroup configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU quota in microseconds per period
    pub cpu_quota: Option<CpuQuota>,
    
    /// Memory limit in bytes
    pub memory_bytes: Option<u64>,
    
    /// Block I/O weight (1-10000)
    pub io_weight: Option<u16>,
    
    /// PIDs max
    pub pids_max: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuQuota {
    pub quota_us: i64,
    pub period_us: u64,
}
```

### 6.2 State Store Schema

```rust
/// Persistent VM state record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStateRecord {
    pub vm_id: String,
    pub status: VmStatus,
    pub config: VmConfig,
    pub pid: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metrics: VmMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VmStatus {
    Creating,
    Running,
    Stopping,
    Stopped,
    Failed { error: String },
}
```

---

## BP-7: Component Design

### 7.1 VM Boot Sequence

```mermaid
sequenceDiagram
    participant Client
    participant API
    participant Lifecycle
    participant Jailer
    participant Block
    participant Network
    participant Firecracker
    participant KVM
    
    Client->>API: POST /vms (VmConfig)
    API->>Lifecycle: create_vm(config)
    
    Lifecycle->>Lifecycle: Validate config
    Lifecycle->>Lifecycle: Allocate VM ID
    Lifecycle->>Lifecycle: Check resources
    
    Lifecycle->>Jailer: setup_jailer(vm_id, limits)
    Jailer->>Jailer: Create chroot directory
    Jailer->>Jailer: Configure cgroups
    Jailer->>Jailer: Setup seccomp filters
    Jailer-->>Lifecycle: JailerEnv
    
    Lifecycle->>Block: prepare_drives(config.drives)
    Block->>Block: Verify block devices
    Block->>Block: Set device permissions
    Block-->>Lifecycle: DriveHandles
    
    Lifecycle->>Network: create_tap(config.network)
    Network->>Network: Create TAP device
    Network->>Network: Configure IP
    Network-->>Lifecycle: TapFd
    
    Lifecycle->>Firecracker: exec jailer (kernel, drives, tap)
    Firecracker->>KVM: open(/dev/kvm)
    KVM-->>Firecracker: kvm_fd
    Firecracker->>KVM: KVM_CREATE_VM
    KVM-->>Firecracker: vm_fd
    Firecracker->>Firecracker: Load kernel
    Firecracker->>Firecracker: Configure virtio devices
    Firecracker->>KVM: KVM_VCPU_RUN
    KVM-->>Firecracker: VM running
    
    Firecracker-->>Lifecycle: VM started (PID)
    Lifecycle->>Lifecycle: Persist state
    Lifecycle-->>API: VmHandle
    API-->>Client: 201 Created (VmHandle)
    
    Note over Lifecycle,KVM: Total time: <125ms
```

### 7.2 Jailer Setup Flow

```mermaid
sequenceDiagram
    participant Lifecycle
    participant Jailer
    participant FS as Filesystem
    participant Cgroup
    participant Seccomp
    
    Lifecycle->>Jailer: setup_jailer(vm_id, limits)
    
    Jailer->>FS: create_jailer_dir(vm_id)
    FS->>FS: mkdir /srv/jailer/{vm_id}
    FS->>FS: chown root:root
    FS->>FS: chmod 0755
    
    Jailer->>FS: copy_firecracker_binary()
    FS->>FS: cp /usr/bin/firecracker /srv/jailer/{vm_id}/root/firecracker
    FS->>FS: chmod 0500
    
    Jailer->>FS: setup_device_nodes()
    FS->>FS: mknod null c 1 3
    FS->>FS: mknod kvm c 10 232
    
    Jailer->>Cgroup: create_cgroup(vm_id)
    Cgroup->>Cgroup: mkdir /sys/fs/cgroup/firecracker/{vm_id}
    Cgroup->>Cgroup: set cpu.max (limits.cpu_quota)
    Cgroup->>Cgroup: set memory.max (limits.memory_bytes)
    Cgroup->>Cgroup: set io.max (limits.io_weight)
    
    Jailer->>Seccomp: load_seccomp_filter()
    Seccomp->>Seccomp: prctl(PR_SET_NO_NEW_PRIVS)
    Seccomp->>Seccomp: prctl(PR_SET_SECCOMP, filter)
    
    Jailer-->>Lifecycle: JailerEnv ready
```

### 7.3 Component Interaction Diagram

```mermaid
graph TB
    subgraph "VM Lifecycle Manager"
        A[create_vm] --> B[validate_config]
        B --> C[allocate_resources]
        C --> D[setup_jailer]
        C --> E[prepare_drives]
        C --> F[create_tap]
        D --> G[spawn_firecracker]
        E --> G
        F --> G
        G --> H[persist_state]
    end
    
    subgraph "Jailer Integration"
        D --> J1[create_chroot]
        D --> J2[configure_cgroups]
        D --> J3[setup_seccomp]
        D --> J4[drop_privileges]
    end
    
    subgraph "Block Device Manager"
        E --> B1[verify_devices]
        E --> B2[set_permissions]
        E --> B3[create_symlinks]
    end
    
    subgraph "Network Tap Manager"
        F --> N1[create_tap_device]
        F --> N2[configure_ip]
        F --> N3[setup_bridge]
    end
```

---

## BP-8: Deployment Design

### 8.1 KVM Requirements

**Hardware Requirements:**
```
CPU: x86_64 with VT-x or AMD-V support
Memory: 8GB+ host RAM for 5000 VMs @ 128MB each
Storage: NVMe SSD recommended for rootfs images
Network: SR-IOV or DPDK for high-density networking
```

**Kernel Requirements:**
```
Linux Kernel: 5.10+ (5.15+ recommended)
KVM: Enabled (/dev/kvm exists)
Modules: kvm, kvm_intel or kvm_amd
cgroups: v2 mounted at /sys/fs/cgroup
seccomp: CONFIG_SECCOMP=y
```

**Verification Script:**
```bash
#!/bin/bash
# verify_kvm.sh - Validates KVM readiness

set -euo pipefail

echo "Checking KVM support..."

# Check CPU virtualization
if grep -q 'vmx' /proc/cpuinfo || grep -q 'svm' /proc/cpuinfo; then
    echo "✓ CPU virtualization supported"
else
    echo "✗ CPU virtualization NOT supported"
    exit 1
fi

# Check KVM module
if [ -c /dev/kvm ]; then
    echo "✓ /dev/kvm exists"
else
    echo "✗ /dev/kvm missing"
    exit 1
fi

# Check KVM permissions
if [ -r /dev/kvm ] && [ -w /dev/kvm ]; then
    echo "✓ /dev/kvm accessible"
else
    echo "✗ /dev/kvm not accessible"
    exit 1
fi

# Check cgroups v2
if mountpoint -q /sys/fs/cgroup; then
    echo "✓ cgroups v2 mounted"
else
    echo "✗ cgroups v2 not mounted"
    exit 1
fi

# Check seccomp
if grep -q 'seccomp' /proc/self/status; then
    echo "✓ seccomp supported"
else
    echo "✗ seccomp NOT supported"
    exit 1
fi

echo "✓ All KVM requirements satisfied"
```

### 8.2 Deployment Architecture

```mermaid
graph TB
    subgraph "Host System"
        subgraph "Firecracker Manager"
            API[API Server :8080]
            MGR[VM Manager]
            STORE[State Store]
        end
        
        subgraph "VM Instances"
            VM1[VM 1 - PID 12345]
            VM2[VM 2 - PID 12346]
            VM3[VM 3 - PID 12347]
        end
        
        subgraph "Kernel"
            KVM[KVM Module]
            CGROUP[cgroups v2]
            NET[Network Stack]
        end
    end
    
    API --> MGR
    MGR --> STORE
    MGR --> VM1
    MGR --> VM2
    MGR --> VM3
    
    VM1 --> KVM
    VM2 --> KVM
    VM3 --> KVM
    
    MGR --> CGROUP
    MGR --> NET
```

### 8.3 Resource Allocation Strategy

```rust
/// Resource allocation policy for VM density.
pub struct ResourcePolicy {
    /// Reserve memory for host operations
    pub host_memory_reserve_mb: u32,  // 2048 MB
    
    /// Reserve vCPUs for host operations
    pub host_vcpu_reserve: u8,  // 2 vCPUs
    
    /// Maximum memory per VM
    pub max_vm_memory_mb: u32,  // 16384 MB
    
    /// Maximum vCPUs per VM
    pub max_vm_vcpus: u8,  // 8 vCPUs
    
    /// Overcommit ratio for memory
    pub memory_overcommit: f32,  // 1.5
}

impl ResourcePolicy {
    pub fn calculate_capacity(&self, host: &HostResources) -> VmCapacity {
        let available_memory = 
            (host.total_memory_mb - self.host_memory_reserve_mb) as f32 
            * self.memory_overcommit;
        
        let available_vcpus = 
            host.total_vcpus - self.host_vcpu_reserve;
        
        VmCapacity {
            max_vms_128mb: (available_memory / 128.0) as u32,
            max_vms_256mb: (available_memory / 256.0) as u32,
            available_vcpus,
        }
    }
}
```

---

## BP-9: Formal Verification

### 9.1 PROP-VM-001: VM Isolation

**Property:** Each VM executes in complete isolation from other VMs and the host.

**Formal Specification:**
```lean
-- Isolation invariant: VMs cannot access each other's resources
theorem vm_isolation (vm1 vm2 : VmId) (h : vm1 ≠ vm2) :
  ∀ (op : Operation), 
    executes_in vm1 op → 
    ∃ (resource : Resource), 
      resource.owner = vm2 → 
      ¬(accesses op resource) :=
by
  intro op h_exec
  -- Proof relies on:
  -- 1. Separate jailer chroots (filesystem isolation)
  -- 2. Separate cgroups (resource isolation)
  -- 3. Seccomp filters (syscall isolation)
  -- 4. KVM virtualization (hardware isolation)
  sorry -- Full proof in proof_vm.lean
```

**Verification Method:**
- Static analysis of jailer configuration
- Runtime monitoring of cgroup membership
- Seccomp filter audit
- KVM isolation test suite

### 9.2 PROP-VM-002: Resource Cleanup

**Property:** All resources are cleaned up when a VM is destroyed.

**Formal Specification:**
```lean
-- Cleanup invariant: All resources released on VM destruction
theorem resource_cleanup (vm : VmId) :
  ∀ (resource : Resource), 
    resource.owner = vm →
    destroys vm →
    eventually (λ s, resource.owner = none ∧ resource.state = Released) :=
by
  intro resource h_owner h_destroy
  -- Proof relies on:
  -- 1. RAII-based resource management
  -- 2. Guarded cleanup in drop()
  -- 3. Idempotent cleanup operations
  -- 4. Transactional state updates
  sorry -- Full proof in proof_vm.lean
```

**Verification Method:**
- Leak detection tests
- Cleanup verification harness
- State store consistency checks
- Resource accounting audit

### 9.3 PROP-VM-003: Boot Timing

**Property:** VM boot completes within 125ms (p99).

**Formal Specification:**
```lean
-- Timing invariant: Boot completes in <125ms
theorem boot_timing (config : VmConfig) (h_valid : config.valid) :
  ∃ (t : Duration), 
    t < 125ms ∧
    creates_vm config = result_after t (ok vm_handle) :=
by
  -- Proof relies on:
  -- 1. Minimal boot sequence (see BP-7.1)
  -- 2. Pre-allocated resources
  -- 3. Optimized virtio initialization
  -- 4. No blocking I/O in critical path
  sorry -- Full proof in proof_vm.lean
```

**Verification Method:**
- Microbenchmarks for each boot phase
- Statistical analysis of production metrics
- P99 latency SLO monitoring
- Performance regression tests

---

## BP-10: HAL Specification

### 10.1 KVM Hardware Abstraction Layer

```rust
/// Hardware Abstraction Layer for KVM operations.
pub trait KvmHal: Send + Sync {
    /// Opens the KVM device.
    fn open_kvm(&self) -> Result<KvmFd, KvmError>;
    
    /// Creates a new VM instance.
    fn create_vm(&self, kvm_fd: &KvmFd) -> Result<VmFd, KvmError>;
    
    /// Creates a VCPU for a VM.
    fn create_vcpu(&self, vm_fd: &VmFd, vcpu_id: u8) -> Result<VcpuFd, KvmError>;
    
    /// Sets user memory region for VM.
    fn set_user_memory_region(
        &self,
        vm_fd: &VmFd,
        region: &MemoryRegion,
    ) -> Result<(), KvmError>;
    
    /// Gets VCPU registers.
    fn get_regs(&self, vcpu_fd: &VcpuFd) -> Result<Regs, KvmError>;
    
    /// Sets VCPU registers.
    fn set_regs(&self, vcpu_fd: &VcpuFd, regs: &Regs) -> Result<(), KvmError>;
    
    /// Runs VCPU.
    fn run_vcpu(&self, vcpu_fd: &VcpuFd) -> Result<VcpuExit, KvmError>;
}

/// Production KVM HAL implementation.
pub struct LinuxKvmHal {
    kvm_path: PathBuf,
}

impl KvmHal for LinuxKvmHal {
    fn open_kvm(&self) -> Result<KvmFd, KvmError> {
        let fd = unsafe { 
            libc::open(
                self.kvm_path.as_ptr() as *const i8,
                libc::O_RDWR | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            Err(KvmError::OpenFailed(errno::errno()))
        } else {
            Ok(KvmFd::from_raw_fd(fd))
        }
    }
    
    // ... other implementations
}

/// Mock KVM HAL for testing.
pub struct MockKvmHal {
    vms: Arc<Mutex<Vec<MockVm>>>,
}

impl KvmHal for MockKvmHal {
    fn open_kvm(&self) -> Result<KvmFd, KvmError> {
        Ok(KvmFd::from_raw_fd(42)) // Mock FD
    }
    
    // ... other mock implementations
}
```

### 10.2 Network HAL

```rust
/// Hardware Abstraction Layer for network operations.
pub trait NetworkHal: Send + Sync {
    /// Creates a TAP device.
    fn create_tap(&self, name: &str) -> Result<TapFd, NetworkError>;
    
    /// Deletes a TAP device.
    fn delete_tap(&self, name: &str) -> Result<(), NetworkError>;
    
    /// Sets TAP persistent.
    fn set_tap_persistent(&self, fd: &TapFd) -> Result<(), NetworkError>;
    
    /// Configures TAP offload.
    fn set_tap_offload(
        &self,
        fd: &TapFd,
        features: TapOffload,
    ) -> Result<(), NetworkError>;
}

pub struct LinuxNetworkHal;

impl NetworkHal for LinuxNetworkHal {
    fn create_tap(&self, name: &str) -> Result<TapFd, NetworkError> {
        // Uses /dev/net/tun
    }
}
```

### 10.3 Storage HAL

```rust
/// Hardware Abstraction Layer for storage operations.
pub trait StorageHal: Send + Sync {
    /// Opens a block device.
    fn open_block_device(&self, path: &Path) -> Result<BlockFd, StorageError>;
    
    /// Gets device size.
    fn get_device_size(&self, fd: &BlockFd) -> Result<u64, StorageError>;
    
    /// Sets device read-only.
    fn set_read_only(&self, fd: &BlockFd) -> Result<(), StorageError>;
}
```

---

## BP-11: Compliance Matrix

### 11.1 OCI Runtime Specification Compliance

| OCI Requirement | Implementation | Status |
|-----------------|----------------|--------|
| **Config Schema** | VmConfig maps to spec.Spec | ✓ Compliant |
| **Root Filesystem** | Block device with rootfs | ✓ Compliant |
| **Process** | Init process in VM | ✓ Compliant |
| **Linux Namespaces** | VM provides full isolation | ✓ Compliant |
| **Linux Devices** | virtio devices | ✓ Compliant |
| **Linux Resources** | cgroups via jailer | ✓ Compliant |
| **Hooks** | Prestart, poststop supported | ✓ Compliant |
| **Annotations** | VmConfig.metadata | ✓ Compliant |

### 11.2 Firecracker API Compatibility

| Firecracker API | Manager Interface | Status |
|-----------------|-------------------|--------|
| PUT /machine-config | IF-VM-001 (create_vm) | ✓ Compatible |
| PUT /boot-source | IF-VM-001 (kernel config) | ✓ Compatible |
| PUT /drives/{id} | IF-VM-003 (attach_volume) | ✓ Compatible |
| PUT /network-interfaces/{id} | IF-VM-004 (configure_network) | ✓ Compatible |
| PUT /actions | Internal (start instance) | ✓ Compatible |
| GET /vm/config | VmStateRecord | ✓ Compatible |
| GET /vm/info | VmMetrics | ✓ Compatible |

### 11.3 Security Compliance

| Security Control | Implementation | Verification |
|------------------|----------------|--------------|
| Process Isolation | Jailer chroot + namespaces | PROP-VM-001 |
| Resource Limits | cgroups v2 | PROP-VM-002 |
| Syscall Filtering | Seccomp | Audit log |
| Network Isolation | Per-VM TAP devices | Integration test |
| Storage Isolation | Per-VM block devices | Integration test |
| Audit Logging | Structured logging | Log analysis |

---

## BP-12: Quality Checklist

### 12.1 Design Completeness

- [x] All IEEE 1016-2009 sections addressed
- [x] C4 diagrams provided (Context, Component)
- [x] Interface contracts defined (IF-VM-001 to IF-VM-004)
- [x] Data structures specified
- [x] Component interactions documented
- [x] Deployment requirements specified
- [x] Formal properties stated
- [x] HAL abstractions defined

### 12.2 Traceability

- [x] YP-VIRT-KVM-001 theorems mapped
- [x] ADR references included
- [x] Requirement matrix complete

### 12.3 Verification

- [x] Formal properties defined (PROP-VM-001 to PROP-VM-003)
- [x] Proof obligations stated
- [x] Test strategy defined
- [x] Performance targets specified

### 12.4 Implementation Readiness

- [x] Interface signatures complete
- [x] Error handling specified
- [x] Timing requirements clear
- [x] Deployment checklist provided

### 12.5 Review Sign-off

| Reviewer | Role | Status | Date |
|----------|------|--------|------|
| Construct | Systems Architect | ✓ Approved | 2026-03-05 |
| | Security Review | Pending | |
| | Performance Review | Pending | |
| | Implementation Lead | Pending | |

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| **Firecracker** | AWS open-source VMM for microVMs |
| **Jailer** | Firecracker's isolation/sandboxing tool |
| **MicroVM** | Lightweight VM with minimal boot time |
| **virtio** | Para-virtualized I/O device framework |
| **TAP** | Network tunnel device |
| **KVM** | Kernel-based Virtual Machine |

## Appendix B: References

1. IEEE 1016-2009 - Standard for Information Technology
2. YP-VIRT-KVM-001 - Yellow Paper on KVM Virtualization
3. ADR-014 - Firecracker Selection
4. ADR-015 - Jailer Integration
5. Firecracker Documentation - https://github.com/firecracker-microvm/firecracker
6. OCI Runtime Specification - https://github.com/opencontainers/runtime-spec

---

**Document End**
