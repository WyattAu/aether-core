# Aether Core Architecture

> **The Post-Container Application Operating System**

Aether is a next-generation runtime for distributed applications that replaces traditional container orchestration with a lightweight actor-based model powered by WebAssembly.

## Table of Contents

1. [Overview](#overview)
2. [Core Principles](#core-principles)
3. [Architecture Layers](#architecture-layers)
4. [Module Reference](#module-reference)
5. [Data Flow](#data-flow)
6. [Security Model](#security-model)
7. [Performance Characteristics](#performance-characteristics)
8. [Extension Points](#extension-points)

---

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Aether Runtime                         │
├─────────────────────────────────────────────────────────────┤
│   Actors (WASM)   │   MicroVMs   │   Legacy Containers     │
├───────────────────┴──────────────┴──────────────────────────┤
│               Actor Scheduler (Work Stealing)               │
├─────────────────────────────────────────────────────────────┤
│  Mesh Network (QUIC + mTLS)  │  State (FDB/In-Memory)       │
├─────────────────────────────────────────────────────────────┤
│              Observability & Security Layer                  │
└─────────────────────────────────────────────────────────────┘
```

### Key Metrics

| Metric | Target | Current |
|--------|--------|---------|
| Cold start (instantiate-only) | < 100us | ~61us |
| Actors per node | 100,000+ | Tested @ 100K |
| Actor spawn throughput | > 100K/sec | ~378K/sec |
| Intra-node latency | < 1ms | ~0.5ms |
| Inter-node latency | < 2ms | ~1.2ms |

---

## Core Principles

### 1. Deterministic Execution
- Time and randomness are injected by the host
- Enables time-travel debugging
- Supports replay for testing
- Critical for reproducible AI workflows

### 2. Capability-Based Security
- Actors must declare required capabilities upfront
- Host validates all capability requests at runtime
- Principle of least privilege enforced by default
- No ambient authority

### 3. Zero-Panic Policy
- All `unwrap()`, `expect()`, and `panic!()` are denied by clippy
- Errors are always propagated through `Result<T>`
- Graceful degradation is preferred over crashes

### 4. Observable by Default
- Every operation is traced
- Metrics are collected automatically
- Health checks are built-in

---

## Architecture Layers

### Layer 1: Host & Configuration

```
src/
├── host.rs           # Main entry point, orchestrates all subsystems
├── config/
│   ├── mod.rs        # Configuration structures
│   └── reload.rs     # Hot configuration reload
└── capability.rs     # Capability definitions and validation
```

**Key Types:**
- `Host` - The main runtime container
- `AetherConfig` - Configuration for all subsystems
- `CapabilitySet` - Bitflags for actor permissions

### Layer 2: Actor System

```
src/actor/
├── mod.rs            # ActorId, ActorHandle, ActorBuilder
├── executor.rs       # WASM execution engine
├── handle.rs         # Actor handle for message passing
├── mailbox.rs        # Per-actor message queue
├── queue.rs          # Global work queue
├── registry.rs       # Actor lookup by ID
├── scheduler.rs      # Work-stealing scheduler
├── supervisor.rs     # Actor lifecycle management
├── migration.rs      # Actor migration between nodes
└── rpc.rs            # Remote procedure call support
```

**Key Types:**
- `ActorId` - Unique 128-bit actor identifier
- `ActorHandle` - Reference to a running actor
- `Message` - Envelope for actor messages
- `Mailbox` - MPSC queue for incoming messages

**Message Flow:**
```
Sender → Mailbox → Scheduler → Executor → Handler
                    ↑
              Work Stealing
```

### Layer 3: Execution Engine

```
src/engine/
├── mod.rs            # Engine trait and implementations
├── module.rs         # WASM module loading and compilation
├── instance.rs       # WASM instance management
├── linker.rs         # Host function linking
├── executor.rs       # Execution context
└── pool.rs           # Instance pooling for fast cold starts
```

**Key Types:**
- `Engine` - WASM execution engine (Wasmtime)
- `Module` - Compiled WASM module
- `Instance` - Running WASM instance
- `InstancePool` - Pre-warmed instances

**Cold Start Optimization:**
1. Module compilation happens once at registration
2. Instance pooling maintains warm instances
3. Snapshot/restore for sub-50µs starts

### Layer 4: Mesh Networking

```
src/mesh/
├── mod.rs            # MeshNode, MeshConfig
├── node.rs           # Node identity and discovery
├── connection.rs     # QUIC connection management
├── message.rs        # Message framing and routing
├── resolver.rs       # Actor location resolution
├── quic.rs           # QUIC transport implementation
├── backpressure.rs   # Flow control
└── circuit_breaker.rs # Failure handling
```

**Key Types:**
- `MeshNode` - A node in the mesh
- `ActorResolver` - Resolves actor IDs to network locations
- `ConnectionPool` - Manages QUIC connections

**Routing:**
```
Local Actor → Direct delivery
Remote Actor → MeshNode → QUIC → Remote MeshNode → Local delivery
```

### Layer 5: State Management

```
src/state/
├── mod.rs            # StateBackend trait
├── kv.rs             # Key-value store interface
├── cache.rs          # In-memory caching layer
├── fdb.rs            # FoundationDB backend
├── transaction.rs    # ACID transactions
├── checkpoint.rs     # State snapshots
└── hydration.rs      # Lazy state loading
```

**Key Types:**
- `StateBackend` - Trait for state storage
- `KeyValueStore` - Simple KV operations
- `Transaction` - ACID transaction support

**Serialization:**
- Uses `rkyv` for zero-copy serialization
- State layout defined in `state-layout.rkyv`

### Layer 6: WASI Implementation

```
src/wasi/
├── mod.rs            # WasiHost trait
├── clocks.rs         # Deterministic time injection
├── random.rs         # Deterministic randomness
├── filesystem.rs     # Virtual filesystem
├── file_descriptor.rs # FD management
├── sockets.rs        # Network socket abstraction
├── sockets_tcp.rs    # TCP implementation
├── sockets_udp.rs    # UDP implementation
├── http.rs           # HTTP client support
└── virtual_fs.rs     # Sandbox filesystem
```

**Deterministic WASI:**
- `clocks::now()` returns injected time
- `random::get_random_bytes()` returns injected entropy
- Enables perfect replay of execution

### Layer 7: Security

```
src/security/
├── mod.rs            # Security subsystem
├── capability.rs     # Capability validation (in root)
├── tls.rs            # mTLS configuration
├── certs.rs          # Certificate management
├── identity.rs       # Actor/node identity
├── authorizer.rs     # Authorization decisions
├── rbac.rs           # Role-based access control
├── policy.rs         # Security policies
├── audit.rs          # Security audit logging
├── penetration.rs    # Penetration testing
├── hardening.rs      # Security hardening
├── vulnerability.rs  # Vulnerability scanning
└── secrets/          # Secret management
    ├── mod.rs
    ├── providers.rs
    ├── vault.rs
    ├── aws.rs
    ├── gcp.rs
    └── cache.rs
```

**Key Types:**
- `CertificateAuthority` - mTLS certificate management
- `ActorIdentity` / `NodeIdentity` - Identity certificates
- `SecretProvider` - Secret injection (Vault, AWS, GCP)

### Layer 8: Observability

```
src/observability/
├── mod.rs            # Observability facade
├── metrics.rs        # Prometheus metrics
└── health.rs         # Health checking
```

```
src/tracing/
├── mod.rs            # Distributed tracing
├── span.rs           # Span management
├── exporter.rs       # OTLP export
└── propagation.rs    # Context propagation
```

**Key Types:**
- `Observability` - Unified observability interface
- `MetricsCollector` - Metric collection
- `HealthChecker` - Health status
- `Tracing` - Distributed tracing

### Layer 9: Dashboard & API

```
src/dashboard/
├── mod.rs            # Dashboard configuration
├── server.rs         # HTTP/WebSocket server
├── handlers.rs       # Request handlers
├── ws.rs             # WebSocket handlers
└── static_files.rs   # Static asset serving
```

### Layer 10: Enterprise Features

```
src/enterprise/
├── mod.rs            # Enterprise subsystem
├── tenant.rs         # Multi-tenancy
└── quotas.rs         # Resource quotas
```

**Key Types:**
- `Tenant` - Isolated tenant environment
- `TenantManager` - Tenant lifecycle
- `ResourceQuotas` - Quota enforcement

### Layer 11: Chaos Engineering

```
src/chaos/
├── mod.rs            # Chaos engineering
├── fault_injector.rs # Fault injection
└── scenarios.rs      # Chaos scenarios
```

### Layer 12: MicroVM Support

```
src/vm/
├── mod.rs            # VM management
├── config.rs         # VM configuration
├── manager.rs        # VM lifecycle
├── firecracker.rs    # Firecracker integration
├── jailer.rs         # VM isolation
├── snapshot.rs       # VM snapshots
├── volume.rs         # Storage volumes
└── api.rs            # VM API
```

---

## Module Reference

### Public API Modules

| Module | Purpose | Feature Flag |
|--------|---------|--------------|
| `actor` | Actor creation and messaging | default |
| `capability` | Capability definitions | default |
| `config` | Configuration | default |
| `engine` | WASM execution | `wasm` |
| `mesh` | Mesh networking | `mesh` |
| `state` | State management | default |
| `wasi` | WASI host functions | default |
| `observability` | Metrics and tracing | default |
| `security` | TLS and certificates | default |
| `enterprise` | Multi-tenancy | `enterprise` |
| `vm` | MicroVM management | `firecracker` |
| `chaos` | Chaos engineering | `chaos` |

### Feature Flags

```toml
[features]
default = ["wasm", "mesh"]
wasm = ["wasmtime", "wasmtime-wasi"]
mesh = ["quinn", "rustls"]
enterprise = []
fdb = ["foundationdb"]
firecracker = []
chaos = ["num_cpus"]
```

---

## Data Flow

### Actor Spawn Flow

```
1. ActorBuilder::new()
2. .with_module(wasm_bytes)
3. .with_capabilities(caps)
4. .spawn()
   │
   ├── Module compilation (cached)
   ├── Capability validation
   ├── Instance creation (from pool if available)
   ├── Mailbox creation
   ├── Registration in registry
   └── Return ActorHandle
```

### Message Send Flow

```
1. actor_handle.send(message)
   │
   ├── Serialize message (rkyv)
   ├── Route to mailbox
   │   ├── Local: Direct enqueue
   │   └── Remote: Mesh routing
   ├── Scheduler picks up work
   ├── Executor runs handler
   └── Response routed back
```

### State Access Flow

```
1. Actor requests state access
   │
   ├── Capability check (STATE_READ/WRITE)
   ├── Cache lookup
   │   ├── Hit: Return cached value
   │   └── Miss: Continue
   ├── Backend lookup (FDB/In-Memory)
   ├── Cache population
   └── Return to actor
```

---

## Security Model

### Capability System

```rust
bitflags! {
    pub struct CapabilitySet: u64 {
        const FS_READ;
        const FS_WRITE;
        const STATE_READ;
        const STATE_WRITE;
        const NETWORK_OUTBOUND;
        const NETWORK_INBOUND;
        const PROCESS_SPAWN;
        const SYSTEM_INFO;
        // ... more capabilities
    }
}
```

**Enforcement Points:**
1. **Spawn time**: Capabilities declared vs granted
2. **Runtime**: Every privileged operation checks capabilities
3. **Network**: mTLS certificates encode capabilities

### mTLS Flow

```
┌─────────────┐                    ┌─────────────┐
│   Node A    │                    │   Node B    │
├─────────────┤                    ├─────────────┤
│ 1. Generate │                    │             │
│    identity │                    │             │
│ 2. Get cert │                    │             │
│    from CA  │                    │             │
│             │  3. Connect +      │             │
│             │     present cert   │             │
│             │ ─────────────────► │ 4. Validate │
│             │                    │    cert     │
│             │  5. mTLS channel   │             │
│             │ ◄─────────────────► │ established│
└─────────────┘                    └─────────────┘
```

---

## Performance Characteristics

### Cold Start Path

```
1. Module lookup (cached)         ~1µs
2. Instance allocation             ~5µs
3. Memory initialization           ~10µs
4. Host function linking           ~5µs
5. Capability injection            ~2µs
6. Ready signal                    ~1µs
────────────────────────────────────────
Total:                            ~24µs (target: <50µs)
```

### Message Latency

```
Local:
  Enqueue → Scheduler → Execute    ~0.5ms

Remote (same DC):
  Enqueue → Serialize → QUIC → 
  Deserialize → Execute            ~1.2ms

Remote (cross-region):
  + Network RTT                    ~50-100ms
```

### Memory Footprint

```
Per Actor:
  Instance: ~64KB
  Mailbox: ~4KB (configurable)
  Stack: ~32KB
  ─────────────────
  Total: ~100KB/actor

100K actors = ~10GB
```

---

## Extension Points

### Adding a New Capability

1. Add to `CapabilitySet` in `capability.rs`
2. Add validation helper method
3. Document in capability module
4. Add tests

### Adding a New WASI Function

1. Implement in `wasi/` module
2. Add to linker in `engine/linker.rs`
3. Consider determinism implications
4. Add capability check if privileged

### Adding a New State Backend

1. Implement `StateBackend` trait
2. Add to feature flags
3. Add configuration in `config/`
4. Add metrics for observability

### Adding a New Secret Provider

1. Implement `SecretProvider` trait in `security/secrets/`
2. Add provider-specific configuration
3. Add to provider registry
4. Document required permissions

---

## Testing Strategy

### Unit Tests
- All modules have `#[cfg(test)]` blocks
- Property-based testing with `proptest`
- Determinism enables perfect replay testing

### Integration Tests
- `tests/` directory for multi-module tests
- Mesh tests with multiple nodes
- State backend tests with real FDB

### Chaos Tests
- `chaos/` module for fault injection
- Network partitions, latency injection
- Resource exhaustion scenarios

---

## Configuration Reference

### Minimal Configuration

```toml
[aether]
node_id = "node-1"

[actor]
max_actors = 100000
mailbox_size = 1024

[mesh]
enabled = true
port = 9000

[state]
backend = "memory"  # or "fdb"
```

### Full Configuration

See `config/mod.rs` for all options.

---

## Contributing

When adding new code:

1. **No panics**: Use `Result<T>` everywhere
2. **Document public APIs**: `#![warn(missing_docs)]`
3. **Add capabilities**: For any privileged operation
4. **Make it observable**: Add metrics and traces
5. **Test it**: Unit tests + integration tests

---

## License

Apache License 2.0
