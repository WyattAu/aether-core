# ADR-002: Deny-by-Default Capability Model

## Status

**Accepted** - 2026-03-05

## Context

Project Aether executes untrusted WebAssembly actors that must be securely isolated while allowing controlled access to system resources. Traditional security models face challenges:

1. **Ambient Authority**: Programs inherit user permissions, leading to privilege escalation
2. **Coarse-Grained Permissions**: "Read access" doesn't specify which files
3. **Implicit Dependencies**: Unclear what resources code actually needs
4. **Difficult Auditing**: Hard to enumerate what a program can do

WASM's capability-based security model provides an opportunity to enforce fine-grained, explicit permissions.

## Decision

We adopt a **deny-by-default capability model**:

### Core Principles

1. **No Ambient Authority**: Actors start with zero capabilities
2. **Explicit Grants**: All capabilities must be explicitly granted
3. **Unforgeable Capabilities**: Actors cannot create or modify capabilities
4. **Least Privilege**: Grant minimum necessary capabilities
5. **Auditable**: All capabilities explicitly declared

### Capability Types

```rust
enum Capability {
    Network {
        allowed_hosts: Vec<HostPattern>,
        max_connections: usize,
        max_bandwidth: Bandwidth,
    },
    Storage {
        paths: Vec<PathPattern>,
        operations: Vec<OpType>,
        quota: ByteSize,
    },
    Compute {
        max_fuel: u64,
        max_memory: ByteSize,
        max_cpu_percent: f32,
    },
    Time {
        resolution: Duration,
        max_drift: Duration,
    },
}
```

### Capability Enforcement

```rust
struct CapabilityEnforcer {
    capabilities: Arc<[Capability]>,
    usage: Arc<AtomicU64>,
}

impl CapabilityEnforcer {
    fn check(&self, op: &Operation) -> Result<(), CapabilityError> {
        for cap in self.capabilities.iter() {
            if cap.allows(op) {
                return Ok(());
            }
        }
        Err(CapabilityError::Denied {
            required: op.required_capability(),
            granted: self.capabilities.clone(),
        })
    }
}
```

### Capability Declaration

Actors declare required capabilities in their manifest:

```toml
[actor]
name = "my-actor"
version = "1.0.0"

[[capabilities]]
type = "network"
hosts = ["api.example.com:443"]
max_connections = 10

[[capabilities]]
type = "storage"
paths = ["/data/actor-state/*"]
operations = ["read", "write"]
quota = "100MB"
```

## Consequences

### Positive
- **Strong Isolation**: Actors cannot exceed granted permissions
- **Explicit Dependencies**: Clear what resources each actor needs
- **Easy Auditing**: Enumerate all capabilities statically
- **Defense in Depth**: Even compromised actors are contained
- **Principle of Least Privilege**: Default is minimum access
- **O(1) Checks**: Capability verification is constant time

### Negative
- **Developer Friction**: Must explicitly request all capabilities
- **Over-Granting Risk**: Developers might request excessive permissions
- **Migration Effort**: Existing code requires capability annotations
- **Granularity Trade-offs**: Too fine = complex, too coarse = insecure

### Neutral
- **Configuration Complexity**: More manifests to manage
- **Runtime Overhead**: Capability checks (minimal, O(1))

## Enforcement Points

| Resource | Check Point | Failure Mode |
|----------|-------------|--------------|
| Network I/O | Before socket creation | Trap |
| File I/O | Before file open | Trap |
| Memory | On allocation | Trap |
| CPU | On fuel exhaustion | Trap |
| Time | On clock access | Return error |

## Capability Lifecycle

```
Declaration → Validation → Grant → Enforcement → Revocation
     ↓            ↓          ↓          ↓            ↓
  Manifest    Schema     Runtime    WASM       Actor
   File       Check      Install    Trap      Termination
```

## Alternatives Considered

### 1. Allow-by-Default
- **Pros**: Easier development, less friction
- **Cons**: Insecure, violates least privilege
- **Rejected**: Security is paramount

### 2. POSIX Capabilities
- **Pros**: Standard, well-understood
- **Cons**: Coarse-grained, ambient authority
- **Rejected**: Not suitable for WASM actors

### 3. SELinux/AppArmor
- **Pros**: Proven, kernel-level
- **Cons**: System-wide, not per-actor
- **Rejected**: Doesn't fit actor model

### 4. Custom ACL System
- **Pros**: Flexible
- **Cons**: Complex, reinventing capability model
- **Rejected**: Capabilities more elegant

## Implementation Notes

### Capability Store
- Immutable after grant
- Reference-counted for sharing
- Atomic for thread-safety

### Performance
- Capability check: <100ns (O(1))
- No syscalls in fast path
- Cached in WASM instance

### Auditing
- All grants logged
- Manifest versioned
- Changes require redeployment

## References

- [Capability-Based Security](https://en.wikipedia.org/wiki/Capability-based_security)
- [WASI Capabilities](https://github.com/WebAssembly/WASI)
- YP-WASM-RUNTIME-001: WASM Runtime Yellow Paper
- THM-WASM-003: Capability Confinement Theorem
- BP-WASM-ENGINE-001: WASM Engine Blue Paper

## Notes

- Review capability granularity quarterly
- Monitor for over-granting patterns
- Consider capability composition operators
