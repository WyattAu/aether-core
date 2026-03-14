# ADR-005: Firecracker VMM Selection

## Status

**Accepted** - 2026-03-05

## Context

Project Aether requires strong isolation for multi-tenant actor execution. The isolation mechanism must provide:

1. **Security Requirements**:
   - Guest escape prevention
   - Hardware-level isolation
   - No information leakage between tenants
   - Resource confinement guarantee

2. **Performance Requirements**:
   - Boot time < 125ms
   - Snapshot/restore < 50ms
   - Density > 1000 VMs per host
   - Minimal overhead vs native

3. **Operational Requirements**:
   - Production-proven
   - Active maintenance
   - Good documentation
   - Simple management

4. **Compatibility Requirements**:
   - Linux KVM support
   - Standard kernel
   - Standard tooling
   - API-driven

## Decision

We select **Firecracker 1.10** as our Virtual Machine Monitor (VMM).

### Selection Criteria Matrix

| Criterion | Firecracker | gVisor | Kata | QEMU |
|-----------|-------------|--------|------|------|
| Boot Time | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ |
| Security | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| Density | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ |
| Overhead | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| Simplicity | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐ |
| Production | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

### Key Advantages

1. **Minimal VMM**:
   - Only essential devices
   - No legacy hardware emulation
   - Small attack surface
   - Fast boot

2. **Hardware Isolation**:
   - KVM-based
   - Hardware virtualization
   - Memory encryption (SEV)
   - CPU isolation

3. **Snapshot/Restore**:
   - Sub-50ms restore
   - Memory snapshot
   - Block device snapshot
   - State preservation

4. **Jailer Integration**:
   - seccomp filters
   - cgroup limits
   - Namespace isolation
   - Resource accounting

## Implementation

### Basic Setup

```rust
use firecracker::{Firecracker, MachineConfig, Jailer};

struct FirecrackerManager {
    firecracker_path: PathBuf,
    jailer_path: PathBuf,
    socket_dir: PathBuf,
}

impl FirecrackerManager {
    async fn create_vm(&self, id: &str, config: VmConfig) -> Result<VmInstance> {
        let socket = self.socket_dir.join(format!("{}.sock", id));
        
        let jailer = Jailer::new()
            .id(id)
            .uid(1000)
            .gid(1000)
            .exec_file(&self.firecracker_path)
            .chroot_base(&self.socket_dir)
            .build();
        
        jailer.start().await?;
        
        let vm = Firecracker::connect(&socket).await?;
        vm.put_machine_config(MachineConfig {
            vcpu_count: config.vcpus,
            mem_size_mib: config.memory_mb,
            ht_enabled: false,
            track_dirty_pages: true,
        }).await?;
        
        Ok(VmInstance { id: id.to_string(), vm, socket })
    }
}
```

### Snapshot/Restore

```rust
impl FirecrackerManager {
    async fn create_snapshot(&self, vm: &VmInstance) -> Result<Snapshot> {
        let snapshot_path = self.snapshot_dir.join(&vm.id);
        
        vm.vm.create_snapshot(
            &snapshot_path.join("mem"),
            &snapshot_path.join("vmstate"),
        ).await?;
        
        Ok(Snapshot {
            memory: snapshot_path.join("mem"),
            vmstate: snapshot_path.join("vmstate"),
            created: SystemTime::now(),
        })
    }
    
    async fn restore_from_snapshot(&self, snapshot: &Snapshot) -> Result<VmInstance> {
        let vm = self.create_vm(Uuid::new_v4().to_string(), VmConfig::default()).await?;
        
        vm.vm.load_snapshot(
            &snapshot.memory,
            &snapshot.vmstate,
        ).await?;
        
        Ok(vm)
    }
}
```

### Security Configuration

```rust
fn configure_security(jailer: &mut Jailer, config: &SecurityConfig) {
    jailer
        .seccomp_level(config.seccomp_level)
        .cgroup_version(CgroupVersion::V2)
        .cgroup_args([
            "cpu.max=50000",      // 50ms per 100ms
            "memory.max=1G",      // 1GB limit
            "io.max=1000000",     // I/O limit
        ]);
}

fn configure_network(vm: &mut Firecracker, config: &NetworkConfig) {
    vm.put_network_interface(NetworkInterface {
        iface_id: "eth0".to_string(),
        guest_mac: Some(config.mac_address),
        host_dev_name: config.tap_device,
    }).unwrap();
}
```

## Consequences

### Positive
- **Fast Boot**: <125ms from API call
- **High Density**: >1000 VMs per host
- **Strong Isolation**: Hardware-level security
- **Snapshot Speed**: <50ms restore
- **Minimal Attack Surface**: No unnecessary devices
- **Production Proven**: AWS Lambda, Fargate

### Negative
- **Linux Only**: No Windows support
- **x86_64/ARM64 Only**: Limited architecture support
- **No GPU**: No device passthrough
- **No Migration**: Cannot live migrate VMs
- **API Complexity**: REST API requires careful state management

### Neutral
- **KVM Required**: Needs hardware virtualization
- **Root Required**: For KVM access and jailer
- **Network Setup**: Requires tap devices

## Alternatives Considered

### 1. gVisor
- **Pros**: No hardware virtualization, fast startup
- **Cons**: Software isolation, lower performance, larger attack surface
- **Rejected**: Hardware isolation preferred

### 2. Kata Containers
- **Pros**: OCI compatible, good security
- **Cons**: Heavier than Firecracker, slower boot
- **Rejected**: Firecracker better for high density

### 3. QEMU/KVM
- **Pros**: Full features, proven
- **Cons**: Large attack surface, slow boot, complex
- **Rejected**: Overkill for microVMs

### 4. Native Containers (runc)
- **Pros**: Fast, lightweight
- **Cons**: Kernel sharing, weaker isolation
- **Rejected**: Insufficient isolation for multi-tenant

## Performance Characteristics

| Metric | Value | Notes |
|--------|-------|-------|
| Boot time | 125ms | From API call to ready |
| Snapshot create | 50ms | Memory + state |
| Snapshot restore | 50ms | From snapshot to ready |
| Memory overhead | <5% | vs bare metal |
| CPU overhead | <2% | vs bare metal |
| Max VMs/host | 1000+ | Depends on resources |

## Security Properties

| Property | Mechanism | Status |
|----------|-----------|--------|
| Memory isolation | KVM + EPT | ✅ Hardware enforced |
| CPU isolation | KVM + VMX/SVM | ✅ Hardware enforced |
| Device isolation | Minimal devices | ✅ Reduced attack surface |
| Seccomp | Jailer | ✅ Configurable |
| Cgroups | Jailer | ✅ Resource limits |
| Namespaces | Jailer | ✅ Process isolation |

## Operational Considerations

### Resource Limits

```yaml
vm_defaults:
  vcpus: 1
  memory_mb: 128
  max_vcpus: 4
  max_memory_mb: 1024

security:
  seccomp_level: 2  # Advanced filtering
  jailer_enabled: true
  namespace_isolation: true
```

### Monitoring

- VM lifecycle events
- Resource usage (CPU, memory, I/O)
- Boot/restore latencies
- Error rates

### Backup/Recovery

- Snapshot to persistent storage
- Periodic checkpointing
- Cross-host snapshot replication

## References

- [Firecracker Documentation](https://github.com/firecracker-microvm/firecracker)
- [Firecracker Security](https://github.com/firecracker-microvm/firecracker/blob/main/docs/security.md)
- [Jailer Documentation](https://github.com/firecracker-microvm/firecracker/blob/main/docs/jailer.md)
- YP-VIRT-KVM-001: Virtualization Yellow Paper
- BP-FIRECRACKER-MANAGER-001: Firecracker Manager Blue Paper

## Notes

- Monitor Firecracker releases for security updates
- Benchmark boot/restore times quarterly
- Evaluate SEV support for confidential computing
