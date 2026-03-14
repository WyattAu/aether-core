# HAL Specification for Aether Host Runtime

**Document ID:** HAL-HOST-001  
**Version:** 1.0.0  
**Status:** Draft  
**Standard:** IEEE 1016-2009  
**Created:** 2026-03-05  
**Blue Paper Reference:** BP-HOST-RUNTIME-001

---

## 1. Overview

This document specifies the Hardware Abstraction Layer (HAL) interfaces for the Aether Host Runtime. The HAL provides portable abstractions over platform-specific functionality including KVM virtualization, io_uring async I/O, and network operations.

### 1.1 Design Goals

| Goal | Description |
|------|-------------|
| Portability | Support Linux x86_64, Linux ARM64, and development environments |
| Performance | Zero-overhead abstractions where possible |
| Testability | Mock implementations for unit testing |
| Safety | Type-safe interfaces with clear error handling |

### 1.2 Platform Support Matrix

| Platform | KVM | io_uring | Network | Status |
|----------|-----|----------|---------|--------|
| Linux x86_64 | ✓ | ✓ | ✓ | Production |
| Linux ARM64 | ✓ | ✓ | ✓ | Production |
| macOS | ✗ | ✗ | ✓ | Development |
| Windows | ✗ | ✗ | ✓ | Development |

---

## 2. KVM HAL Interface (HAL-HOST-001)

### 2.1 Purpose

Abstract Linux KVM (Kernel-based Virtual Machine) operations for Firecracker microVM management.

### 2.2 Interface Definition

```rust
/// KVM Hardware Abstraction Layer
/// 
/// Provides safe abstractions over KVM ioctls for virtual machine management.
/// All operations return Result types with detailed error information.
pub trait KvmHal: Send + Sync {
    // =========================================================================
    // Device Management
    // =========================================================================
    
    /// Open the KVM device (/dev/kvm)
    /// 
    /// # Returns
    /// - `Ok(KvmFd)`: KVM file descriptor on success
    /// - `Err(HalError::DeviceNotFound)`: /dev/kvm not accessible
    /// - `Err(HalError::PermissionDenied)`: Insufficient permissions
    /// - `Err(HalError::Busy)`: Too many KVM instances
    fn open_kvm_device(&self) -> Result<KvmFd, HalError>;
    
    /// Get KVM API version
    /// 
    /// # Arguments
    /// - `kvm`: KVM file descriptor
    /// 
    /// # Returns
    /// API version number (should be 12 for KVM_API_VERSION)
    fn get_api_version(&self, kvm: &KvmFd) -> Result<u32, HalError>;
    
    /// Check for KVM extension support
    /// 
    /// # Arguments
    /// - `kvm`: KVM file descriptor
    /// - `extension`: Extension identifier (e.g., KVM_CAP_USER_MEMORY)
    /// 
    /// # Returns
    /// - `Ok(true)`: Extension supported
    /// - `Ok(false)`: Extension not supported
    fn check_extension(&self, kvm: &KvmFd, extension: KvmExtension) -> Result<bool, HalError>;
    
    // =========================================================================
    // VM Management
    // =========================================================================
    
    /// Create a new virtual machine
    /// 
    /// # Arguments
    /// - `kvm`: KVM file descriptor
    /// 
    /// # Returns
    /// - `Ok(VmFd)`: VM file descriptor
    /// - `Err(HalError::ResourceExhausted)`: Cannot allocate VM
    fn create_vm(&self, kvm: &KvmFd) -> Result<VmFd, HalError>;
    
    /// Set user memory region for VM
    /// 
    /// # Arguments
    /// - `vm`: VM file descriptor
    /// - `slot`: Memory slot identifier
    /// - `guest_addr`: Guest physical address
    /// - `size`: Size in bytes
    /// - `fd`: File descriptor for backing memory
    /// - `offset`: Offset into file
    /// 
    /// # Safety
    /// Caller must ensure:
    /// - Memory region doesn't overlap with existing regions
    /// - fd is valid and has sufficient size
    fn set_user_memory(
        &self,
        vm: &VmFd,
        slot: u32,
        guest_addr: u64,
        size: u64,
        fd: i32,
        offset: u64
    ) -> Result<(), HalError>;
    
    /// Set identity map address for VM
    /// 
    /// # Arguments
    /// - `vm`: VM file descriptor
    /// - `addr`: Physical address for identity map
    fn set_identity_map(&self, vm: &VmFd, addr: u64) -> Result<(), HalError>;
    
    /// Set TSS address for VM
    /// 
    /// # Arguments
    /// - `vm`: VM file descriptor
    /// - `addr`: Physical address for TSS
    fn set_tss_addr(&self, vm: &VmFd, addr: u32) -> Result<(), HalError>;
    
    // =========================================================================
    // vCPU Management
    // =========================================================================
    
    /// Create a virtual CPU
    /// 
    /// # Arguments
    /// - `vm`: VM file descriptor
    /// - `id`: vCPU identifier (0-indexed)
    /// 
    /// # Returns
    /// - `Ok(VcpuFd)`: vCPU file descriptor
    fn create_vcpu(&self, vm: &VmFd, id: u32) -> Result<VcpuFd, HalError>;
    
    /// Run vCPU until exit
    /// 
    /// # Arguments
    /// - `vcpu`: vCPU file descriptor
    /// 
    /// # Returns
    /// - `Ok(VcpuExit)`: Exit reason
    /// - `Err(HalError::Interrupted)`: Run was interrupted
    fn run_vcpu(&self, vcpu: &VcpuFd) -> Result<VcpuExit, HalError>;
    
    /// Get vCPU registers
    /// 
    /// # Arguments
    /// - `vcpu`: vCPU file descriptor
    fn get_regs(&self, vcpu: &VcpuFd) -> Result<Regs, HalError>;
    
    /// Set vCPU registers
    /// 
    /// # Arguments
    /// - `vcpu`: vCPU file descriptor
    /// - `regs`: Register values to set
    fn set_regs(&self, vcpu: &VcpuFd, regs: &Regs) -> Result<(), HalError>;
    
    /// Get vCPU special registers
    fn get_sregs(&self, vcpu: &VcpuFd) -> Result<Sregs, HalError>;
    
    /// Set vCPU special registers
    fn set_sregs(&self, vcpu: &VcpuFd, sregs: &Sregs) -> Result<(), HalError>;
    
    /// Get vCPU FPU state
    fn get_fpu(&self, vcpu: &VcpuFd) -> Result<FpuState, HalError>;
    
    /// Set vCPU FPU state
    fn set_fpu(&self, vcpu: &VcpuFd, fpu: &FpuState) -> Result<(), HalError>;
    
    // =========================================================================
    // Interrupt Control
    // =========================================================================
    
    /// Set interrupt line state
    fn set_irq(&self, vm: &VmFd, irq: u32, level: bool) -> Result<(), HalError>;
    
    /// Inject interrupt into vCPU
    fn interrupt_vcpu(&self, vcpu: &VcpuFd, irq: u8) -> Result<(), HalError>;
    
    // =========================================================================
    // Device Passthrough
    // =========================================================================
    
    /// Register I/O port eventfd
    fn register_io_event(
        &self,
        vm: &VmFd,
        addr: u64,
        range: IoRange,
        fd: i32
    ) -> Result<(), HalError>;
}
```

### 2.3 Data Structures

```rust
/// KVM file descriptor wrapper
pub struct KvmFd {
    fd: i32,
}

/// VM file descriptor wrapper
pub struct VmFd {
    fd: i32,
}

/// vCPU file descriptor wrapper
pub struct VcpuFd {
    fd: i32,
    id: u32,
}

/// Standard registers
#[derive(Debug, Clone)]
pub struct Regs {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rsp: u64,
    pub rflags: u64,
}

/// Special registers
#[derive(Debug, Clone)]
pub struct Sregs {
    pub cs: Segment,
    pub ds: Segment,
    pub es: Segment,
    pub fs: Segment,
    pub gs: Segment,
    pub ss: Segment,
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub cr8: u64,
    pub efer: u64,
    pub apic_base: u64,
}

/// Segment descriptor
#[derive(Debug, Clone)]
pub struct Segment {
    pub base: u64,
    pub limit: u32,
    pub selector: u16,
    pub type_: u8,
    pub present: bool,
    pub dpl: u8,
    pub db: bool,
    pub s: bool,
    pub l: bool,
    pub g: bool,
    pub avl: bool,
}

/// FPU state
#[derive(Debug, Clone)]
pub struct FpuState {
    pub fpr: [[u8; 16]; 8],
    pub fcw: u16,
    pub fsw: u16,
    pub ftwx: u8,
    pub last_opcode: u16,
    pub last_ip: u64,
    pub last_dp: u64,
    pub xmm: [[u8; 16]; 16],
    pub mxcsr: u32,
}

/// vCPU exit reasons
#[derive(Debug, Clone)]
pub enum VcpuExit {
    Unknown,
    Exception,
    IoOut { port: u16, data: Vec<u8> },
    IoIn { port: u16, size: usize },
    MmioWrite { addr: u64, data: Vec<u8> },
    MmioRead { addr: u64, size: usize },
    Halt,
    Shutdown,
    FailEntry { hardware_entry_failure_reason: u64 },
    Intr,
    SystemEvent { type_: u32, flags: u64 },
    Debug,
}

/// KVM extension identifiers
#[derive(Debug, Clone, Copy)]
pub enum KvmExtension {
    UserMemory,
    SetTssAddr,
    Vapic,
    Nmi,
    PvClock,
    Irqchip,
    IrqRouting,
    UserNmi,
    MaxVcpus,
    MaxExtCpus,
    MaxVcpuId,
}

/// I/O range specification
#[derive(Debug, Clone)]
pub struct IoRange {
    pub addr: u64,
    pub len: u64,
    pub is_write: bool,
}
```

### 2.4 Error Codes

```rust
#[derive(Debug, thiserror::Error)]
pub enum HalError {
    #[error("KVM device not found")]
    DeviceNotFound,
    
    #[error("Permission denied")]
    PermissionDenied,
    
    #[error("Resource exhausted")]
    ResourceExhausted,
    
    #[error("Device busy")]
    Busy,
    
    #[error("Operation interrupted")]
    Interrupted,
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("Operation not supported")]
    NotSupported,
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Unknown error: {0}")]
    Unknown(i32),
}
```

### 2.5 Platform Implementations

#### Linux Implementation

```rust
#[cfg(target_os = "linux")]
pub struct LinuxKvmHal;

#[cfg(target_os = "linux")]
impl KvmHal for LinuxKvmHal {
    fn open_kvm_device(&self) -> Result<KvmFd, HalError> {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open("/dev/kvm")?;
        
        Ok(KvmFd { fd: file.into_raw_fd() })
    }
    
    // ... other implementations using ioctls
}
```

#### Mock Implementation (Testing)

```rust
#[cfg(test)]
pub struct MockKvmHal {
    vms: RefCell<Vec<MockVm>>,
}

#[cfg(test)]
impl KvmHal for MockKvmHal {
    fn open_kvm_device(&self) -> Result<KvmFd, HalError> {
        Ok(KvmFd { fd: -1 }) // Mock FD
    }
    
    fn create_vm(&self, _kvm: &KvmFd) -> Result<VmFd, HalError> {
        let vm_id = self.vms.borrow().len() as i32;
        self.vms.borrow_mut().push(MockVm::default());
        Ok(VmFd { fd: vm_id })
    }
    
    // ... simplified mock implementations
}
```

---

## 3. io_uring HAL Interface (HAL-HOST-002)

### 3.1 Purpose

Abstract Linux io_uring operations for high-performance async I/O in the data plane.

### 3.2 Interface Definition

```rust
/// io_uring Hardware Abstraction Layer
/// 
/// Provides zero-copy async I/O using Linux io_uring.
/// Designed for thread-per-core architectures.
pub trait IoUringHal: Send + Sync {
    // =========================================================================
    // Ring Setup
    // =========================================================================
    
    /// Create a new io_uring instance
    /// 
    /// # Arguments
    /// - `entries`: Number of SQ entries (must be power of 2)
    /// - `params`: Setup parameters
    /// 
    /// # Returns
    /// - `Ok(IoUring)`: Configured io_uring instance
    fn setup(&self, entries: u32, params: IoUringParams) -> Result<IoUring, HalError>;
    
    /// Register file descriptors for zero-copy
    /// 
    /// # Arguments
    /// - `ring`: io_uring instance
    /// - `files`: File descriptors to register
    fn register_files(&self, ring: &mut IoUring, files: &[i32]) -> Result<(), HalError>;
    
    /// Register buffers for zero-copy
    /// 
    /// # Arguments
    /// - `ring`: io_uring instance
    /// - `buffers`: I/O vectors to register
    fn register_buffers(&self, ring: &mut IoUring, buffers: &[IoSlice]) -> Result<(), HalError>;
    
    // =========================================================================
    // Submission
    // =========================================================================
    
    /// Prepare a read operation
    /// 
    /// # Arguments
    /// - `ring`: io_uring instance
    /// - `fd`: File descriptor (or registered file index)
    /// - `buf`: Destination buffer
    /// - `offset`: File offset
    /// - `user_data`: User-provided identifier
    fn prepare_read(
        &self,
        ring: &mut IoUring,
        fd: i32,
        buf: &mut [u8],
        offset: u64,
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare a write operation
    fn prepare_write(
        &self,
        ring: &mut IoUring,
        fd: i32,
        buf: &[u8],
        offset: u64,
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare a read into registered buffer (zero-copy)
    fn prepare_read_fixed(
        &self,
        ring: &mut IoUring,
        fd: i32,
        buf_index: u16,
        len: u32,
        offset: u64,
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare a write from registered buffer (zero-copy)
    fn prepare_write_fixed(
        &self,
        ring: &mut IoUring,
        fd: i32,
        buf_index: u16,
        len: u32,
        offset: u64,
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare accept operation
    fn prepare_accept(
        &self,
        ring: &mut IoUring,
        fd: i32,
        addr: &mut SockAddrStorage,
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare connect operation
    fn prepare_connect(
        &self,
        ring: &mut IoUring,
        fd: i32,
        addr: &SockAddr,
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare send operation
    fn prepare_send(
        &self,
        ring: &mut IoUring,
        fd: i32,
        buf: &[u8],
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare recv operation
    fn prepare_recv(
        &self,
        ring: &mut IoUring,
        fd: i32,
        buf: &mut [u8],
        user_data: u64
    ) -> Result<(), HalError>;
    
    /// Prepare timeout operation
    fn prepare_timeout(
        &self,
        ring: &mut IoUring,
        duration: Duration,
        user_data: u64
    ) -> Result<(), HalError>;
    
    // =========================================================================
    // Submission Control
    // =========================================================================
    
    /// Submit all prepared operations
    /// 
    /// # Returns
    /// Number of submissions processed
    fn submit(&self, ring: &IoUring) -> Result<u32, HalError>;
    
    /// Submit and wait for at least `min_complete` completions
    fn submit_and_wait(&self, ring: &IoUring, min_complete: u32) -> Result<u32, HalError>;
    
    /// Get number of pending submissions
    fn pending_submissions(&self, ring: &IoUring) -> u32;
    
    // =========================================================================
    // Completion
    // =========================================================================
    
    /// Peek at completion queue (non-blocking)
    /// 
    /// # Returns
    /// Iterator over available completions
    fn peek_cq<'a>(&'a self, ring: &'a IoUring) -> CqIterator<'a>;
    
    /// Wait for at least one completion
    fn wait_cq(&self, ring: &IoUring) -> Result<Cqe, HalError>;
    
    /// Advance completion queue head
    /// 
    /// # Arguments
    /// - `ring`: io_uring instance
    /// - `count`: Number of completions consumed
    fn advance_cq(&self, ring: &IoUring, count: u32);
    
    /// Get number of available completions
    fn available_completions(&self, ring: &IoUring) -> u32;
}

/// Completion queue entry
#[derive(Debug, Clone, Copy)]
pub struct Cqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

/// Completion queue iterator
pub struct CqIterator<'a> {
    ring: &'a IoUring,
    head: u32,
    tail: u32,
}
```

### 3.3 Configuration

```rust
/// io_uring setup parameters
#[derive(Debug, Clone)]
pub struct IoUringParams {
    /// SQ entries (power of 2)
    pub sq_entries: u32,
    /// CQ entries (power of 2, typically 2x SQ)
    pub cq_entries: u32,
    /// Enable SQPOLL (kernel thread polling)
    pub sq_poll: bool,
    /// SQPOLL CPU affinity
    pub sq_poll_cpu: Option<u32>,
    /// Enable IOPOLL for storage
    pub io_poll: bool,
    /// Defer task runs
    pub defer_taskrun: bool,
    /// Maximum registered buffers
    pub max_buffers: u32,
}

impl Default for IoUringParams {
    fn default() -> Self {
        Self {
            sq_entries: 256,
            cq_entries: 512,
            sq_poll: false,
            sq_poll_cpu: None,
            io_poll: false,
            defer_taskrun: true,
            max_buffers: 32768,
        }
    }
}
```

---

## 4. Network HAL Interface (HAL-HOST-003)

### 4.1 Purpose

Abstract network socket operations for cross-platform compatibility.

### 4.2 Interface Definition

```rust
/// Network Hardware Abstraction Layer
pub trait NetworkHal: Send + Sync {
    // =========================================================================
    // Socket Creation
    // =========================================================================
    
    /// Create a socket
    fn socket(&self, domain: AddressFamily, ty: SocketType, protocol: Protocol) -> Result<i32, HalError>;
    
    /// Create a non-blocking socket
    fn nonblocking_socket(&self, domain: AddressFamily, ty: SocketType, protocol: Protocol) -> Result<i32, HalError>;
    
    // =========================================================================
    // Socket Options
    // =========================================================================
    
    /// Set socket to non-blocking mode
    fn set_nonblocking(&self, fd: i32, nonblocking: bool) -> Result<(), HalError>;
    
    /// Set socket reuse address
    fn set_reuseaddr(&self, fd: i32, reuse: bool) -> Result<(), HalError>;
    
    /// Set socket reuse port
    fn set_reuseport(&self, fd: i32, reuse: bool) -> Result<(), HalError>;
    
    /// Set socket receive buffer size
    fn set_recv_buffer(&self, fd: i32, size: usize) -> Result<(), HalError>;
    
    /// Set socket send buffer size
    fn set_send_buffer(&self, fd: i32, size: usize) -> Result<(), HalError>;
    
    /// Enable TCP fast open
    fn set_tcp_fastopen(&self, fd: i32, enable: bool) -> Result<(), HalError>;
    
    /// Set TCP no delay
    fn set_tcp_nodelay(&self, fd: i32, nodelay: bool) -> Result<(), HalError>;
    
    // =========================================================================
    // Socket Operations
    // =========================================================================
    
    /// Bind socket to address
    fn bind(&self, fd: i32, addr: &SocketAddr) -> Result<(), HalError>;
    
    /// Listen for connections
    fn listen(&self, fd: i32, backlog: i32) -> Result<(), HalError>;
    
    /// Accept a connection
    fn accept(&self, fd: i32) -> Result<(i32, SocketAddr), HalError>;
    
    /// Connect to remote address
    fn connect(&self, fd: i32, addr: &SocketAddr) -> Result<(), HalError>;
    
    /// Close socket
    fn close(&self, fd: i32) -> Result<(), HalError>;
    
    // =========================================================================
    // Data Transfer
    // =========================================================================
    
    /// Send data
    fn send(&self, fd: i32, buf: &[u8]) -> Result<usize, HalError>;
    
    /// Receive data
    fn recv(&self, fd: i32, buf: &mut [u8]) -> Result<usize, HalError>;
    
    /// Send to address (UDP)
    fn send_to(&self, fd: i32, buf: &[u8], addr: &SocketAddr) -> Result<usize, HalError>;
    
    /// Receive from address (UDP)
    fn recv_from(&self, fd: i32, buf: &mut [u8]) -> Result<(usize, SocketAddr), HalError>;
    
    // =========================================================================
    // Address Resolution
    // =========================================================================
    
    /// Resolve hostname to addresses
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, HalError>;
    
    /// Get socket name
    fn getsockname(&self, fd: i32) -> Result<SocketAddr, HalError>;
    
    /// Get peer name
    fn getpeername(&self, fd: i32) -> Result<SocketAddr, HalError>;
    
    // =========================================================================
    // Error Handling
    // =========================================================================
    
    /// Get socket error
    fn get_socket_error(&self, fd: i32) -> Result<i32, HalError>;
}

/// Address family
#[derive(Debug, Clone, Copy)]
pub enum AddressFamily {
    Inet,
    Inet6,
    Unix,
}

/// Socket type
#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    Stream,
    Datagram,
    SeqPacket,
    Raw,
}

/// Protocol
#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
    Raw,
}
```

---

## 5. HAL Factory

### 5.1 Purpose

Provide platform-appropriate HAL implementations.

### 5.2 Interface

```rust
/// HAL factory for creating platform-specific implementations
pub struct HalFactory;

impl HalFactory {
    /// Create KVM HAL for current platform
    pub fn create_kvm_hal() -> Box<dyn KvmHal> {
        #[cfg(target_os = "linux")]
        {
            Box::new(LinuxKvmHal)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Box::new(UnsupportedKvmHal)
        }
    }
    
    /// Create io_uring HAL for current platform
    pub fn create_io_uring_hal() -> Box<dyn IoUringHal> {
        #[cfg(target_os = "linux")]
        {
            Box::new(LinuxIoUringHal)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Box::new(FallbackIoHal) // Falls back to epoll/kqueue
        }
    }
    
    /// Create network HAL for current platform
    pub fn create_network_hal() -> Box<dyn NetworkHal> {
        #[cfg(target_os = "linux")]
        {
            Box::new(LinuxNetworkHal)
        }
        #[cfg(target_os = "macos")]
        {
            Box::new(MacosNetworkHal)
        }
        #[cfg(target_os = "windows")]
        {
            Box::new(WindowsNetworkHal)
        }
    }
}
```

---

## 6. Testing Strategy

### 6.1 Unit Testing

Each HAL interface should have a mock implementation for unit testing:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capability_check() {
        let hal = MockKvmHal::new();
        let kvm = hal.open_kvm_device().unwrap();
        assert!(hal.check_extension(&kvm, KvmExtension::UserMemory).unwrap());
    }
    
    #[test]
    fn test_io_uring_submit_complete() {
        let hal = MockIoUringHal::new();
        let mut ring = hal.setup(256, IoUringParams::default()).unwrap();
        
        let mut buf = [0u8; 1024];
        hal.prepare_read(&mut ring, 0, &mut buf, 0, 1).unwrap();
        
        let submitted = hal.submit(&ring).unwrap();
        assert_eq!(submitted, 1);
    }
}
```

### 6.2 Integration Testing

Platform-specific tests run on appropriate platforms:

```rust
#[cfg(target_os = "linux")]
#[test]
fn test_linux_kvm_create_vm() {
    let hal = LinuxKvmHal;
    let kvm = hal.open_kvm_device().expect("KVM not available");
    let vm = hal.create_vm(&kvm).expect("Failed to create VM");
    // ... assertions
}
```

---

## 7. Performance Characteristics

| HAL Operation | Linux x86_64 | Linux ARM64 | Notes |
|---------------|--------------|-------------|-------|
| KVM ioctl | ~1µs | ~1µs | System call overhead |
| io_uring submit | ~50ns | ~50ns | Ring buffer write |
| io_uring peek_cq | ~10ns | ~10ns | Memory read |
| Network socket | ~5µs | ~5µs | System call |
| Capability check | ~10ns | ~10ns | Bitmap test |

---

## 8. Security Considerations

### 8.1 File Descriptor Management

- All file descriptors are CLOEXEC by default
- Fds are closed on drop via RAII wrappers
- No fd leaks on error paths

### 8.2 Memory Safety

- All buffers are bounds-checked
- Registered buffers are pinned and DMA-safe
- No use-after-free in ring buffer access

### 8.3 Error Handling

- All fallible operations return Result
- No panics in HAL implementations
- Detailed error information for debugging

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-03-05 | Construct | Initial HAL specification |

---

*End of HAL Specification HAL-HOST-001*
