# Project Aether Architecture Overview

**Version:** 2.0.0
**Last Updated:** 2026-03-12  
**Audience:** System Architects, Platform Engineers

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [System Components](#2-system-components)
3. [Execution Model](#3-execution-model)
4. [Networking Model](#4-networking-model)
5. [Security Model](#5-security-model)
6. [State Management](#6-state-management)

---

## 1. System Overview

### 1.1 What is Aether?

Aether is a distributed computing platform designed for high-performance, secure execution of workloads at the edge. It combines the benefits of WebAssembly's fast startup and sandboxing with hardware virtualization for legacy workloads.

### 1.2 Key Design Principles

| Principle | Description |
|-----------|-------------|
| **Deny-by-Default** | All operations require explicit capability grants |
| **Panic-Free Operation** | `panic=abort` with explicit error handling |
| **Zero-Copy Data Path** | io_uring-based I/O for maximum throughput |
| **Dual Runtime** | WASM for fast cold starts, VMs for isolation |
| **Unified Mesh** | Single overlay network for all actors |

### 1.3 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Aether Platform                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                      Host Runtime Daemon                         │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │    │
│  │  │   Config    │  │ Capability  │  │    Health Monitor       │  │    │
│  │  │   Loader    │  │   Manager   │  │                         │  │    │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  ┌──────────────────────┐  ┌──────────────────────────────────────────┐ │
│  │   WASM Engine        │  │        Firecracker Manager               │ │
│  │  ┌────────────────┐  │  │  ┌────────────────┐  ┌────────────────┐  │ │
│  │  │ Module Loader  │  │  │  │   VM Pool      │  │   Jailer       │  │ │
│  │  │ (Wasmtime)     │  │  │  │                │  │                │  │ │
│  │  └────────────────┘  │  │  └────────────────┘  └────────────────┘  │ │
│  │  ┌────────────────┐  │  │  ┌────────────────┐  ┌────────────────┐  │ │
│  │  │ Instance Pool  │  │  │  │   Snapshot     │  │   Restore      │  │ │
│  │  │ (<50µs start)  │  │  │  │   Manager      │  │   Manager      │  │ │
│  │  └────────────────┘  │  │  └────────────────┘  └────────────────┘  │ │
│  └──────────────────────┘  └──────────────────────────────────────────┘ │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                        Mesh Network Layer                           │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │ │
│  │  │ Connection   │  │ Actor        │  │ Flow Controller          │  │ │
│  │  │ Pool (QUIC)  │  │ Resolver     │  │ (Backpressure)           │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                        State Manager                                │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │ │
│  │  │ Local Cache  │  │ Checkpoint   │  │ FDB Client               │  │ │
│  │  │              │  │ Manager      │  │                          │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                          Async Runtimes                                  │
│  ┌──────────────────────────────┐  ┌──────────────────────────────────┐ │
│  │    Data Plane (Monoio)       │  │    Control Plane (Tokio)         │ │
│  │    - io_uring native         │  │    - Standard ecosystem          │ │
│  │    - Zero-copy I/O           │  │    - Service discovery           │ │
│  │    - Thread-per-core         │  │    - Configuration               │ │
│  └──────────────────────────────┘  └──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. System Components

### 2.1 Host Runtime Daemon

The central orchestrator managing all subsystems.

**Responsibilities:**
- Initialize subsystems in dependency order
- Coordinate graceful shutdown
- Route external requests
- Maintain health status
- Enforce resource limits

**Key Interfaces:**
- `Initialize(config) → Result<Runtime, Error>`
- `Shutdown(mode) → Result<(), Error>`
- `GetStatus() → RuntimeStatus`

### 2.2 WASM Engine

High-performance WebAssembly runtime using Wasmtime.

| Feature | Specification |
|---------|---------------|
| Cold Start | <50µs (P99) |
| Memory Isolation | Linear memory sandboxing |
| Execution Model | Fuel-based deterministic execution |
| WASI Compliance | Preview 2 Component Model |

**Components:**
- **Module Loader**: AOT compilation, validation, caching
- **Instance Manager**: Lifecycle management, cold start optimization
- **Fuel Counter**: Instruction counting for bounded execution
- **Memory Manager**: Linear memory allocation, bounds checking
- **WASI Bridge**: System call dispatch, capability enforcement
- **Capability Enforcer**: Deny-by-default access control

### 2.3 Firecracker Manager

KVM-based microVM management for legacy containers.

| Feature | Specification |
|---------|---------------|
| VM Boot | <125ms (P99) |
| Isolation | Hardware-enforced (KVM) |
| Snapshot/Restore | <100ms |
| Network | VirtIO-net |

**Components:**
- **VM Pool**: Instance management with pre-warming
- **Jailer**: Security sandboxing (chroot, cgroups, namespaces)
- **Snapshot Manager**: State capture and restoration
- **Network Config**: Tap device management

### 2.4 Mesh Network Layer

QUIC-based overlay network for actor communication.

| Feature | Specification |
|---------|---------------|
| Protocol | QUIC (RFC 9000) |
| Local Latency | <1ms (P99) |
| Remote Latency | <2ms (P99 same DC) |
| Security | mTLS on all connections |

**Components:**
- **Connection Pool**: Bounded connection management with LRU eviction
- **Actor Resolver**: DHT-based actor location
- **Flow Controller**: Credit-based backpressure
- **mTLS Handler**: Certificate management

### 2.5 State Manager

Distributed state persistence with FoundationDB.

| Feature | Specification |
|---------|---------------|
| Local Read | <10µs |
| Local Write | <100µs |
| Consistency | Linearizable writes |
| Replication | Configurable (default: 3) |

---

## 3. Execution Model

### 3.1 WASM Actor Lifecycle

```
┌─────────┐     ┌─────────┐     ┌────────────┐     ┌─────────┐
│ Pending │────▶│ Loading │────▶│ Initializing│────▶│ Running │
└─────────┘     └─────────┘     └────────────┘     └─────────┘
     │               │                  │                │
     │               │                  │                │
     │               │                  │                ▼
     │               │                  │         ┌───────────┐
     │               │                  │         │ Suspended │
     │               │                  │         └───────────┘
     │               │                  │                │
     │               │                  │                ▼
     │               │                  │         ┌─────────────┐
     │               │                  │         │ Migrating   │
     │               │                  │         └─────────────┘
     │               │                  │
     ▼               ▼                  ▼
┌─────────┐    ┌─────────┐       ┌─────────┐
│ Failed  │    │ Failed  │       │ Failed  │
└─────────┘    └─────────┘       └─────────┘
```

### 3.2 Cold Start Optimization

WASM actors achieve sub-50µs cold starts through:

1. **AOT Compilation**: Modules pre-compiled to native code
2. **Memory Pre-allocation**: Pre-warmed memory pools
3. **Lazy Initialization**: Defer non-critical setup
4. **Data Segment Optimization**: Parallelized data copying

```
Timeline: t=0 ──────────────────────────────────────────▶ t=50µs

Phase 1: Allocation (<10µs)
├─ Allocate InstanceState struct
├─ Initialize atomic counters
└─ Set up capability bitmap

Phase 2: Memory Setup (<15µs)
├─ mmap(min_pages * 64KB)
├─ Set up guard pages
└─ Configure memory protection

Phase 3: Data Segments (<10µs)
├─ memcpy for each segment (parallelized)
└─ Verify copy integrity

Phase 4: Table Init (<5µs)
├─ Allocate function table
└─ Initialize element segments

Phase 5: Globals (<5µs)
├─ Copy global initializers
└─ Link imported globals

Phase 6: Capability Bind (<3µs)
├─ Set capability bitmap
└─ Validate capability subset

Phase 7: Start Function (<2µs)
└─ Invoke if present (trivial only)
```

### 3.3 Fuel-Based Execution

Deterministic execution through instruction counting:

```rust
// Fuel consumption model
struct FuelCounter {
    remaining: AtomicU64,
}

impl FuelCounter {
    fn consume(&self, cost: u64) -> Result<(), Trap> {
        loop {
            let current = self.remaining.load(Ordering::Relaxed);
            if current < cost {
                return Err(Trap::OutOfFuel);
            }
            if self.remaining.compare_exchange_weak(
                current,
                current - cost,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ).is_ok() {
                return Ok(());
            }
        }
    }
}
```

**Instruction Costs:**

| Instruction Type | Cost |
|-----------------|------|
| Simple (i32.add, etc.) | 1 |
| Memory access | 2 |
| Branch | 3 |
| Call | 5 |
| Host function call | 100+ |

### 3.4 VM Execution Model

Firecracker VMs provide hardware-enforced isolation:

```
┌─────────────────────────────────────────────────────────────┐
│                       Host System                            │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Aether Daemon                        ││
│  │  ┌───────────────────────────────────────────────────┐  ││
│  │  │              Firecracker Manager                  │  ││
│  │  │  ┌─────────────┐  ┌─────────────────────────────┐│  ││
│  │  │  │   Jailer    │  │       microVM               ││  ││
│  │  │  │             │  │  ┌───────────────────────┐  ││  ││
│  │  │  │ chroot      │  │  │   Guest Kernel        │  ││  ││
│  │  │  │ cgroups     │  │  │  ┌─────────────────┐  │  ││  ││
│  │  │  │ namespaces  │  │  │  │ Container       │  │  ││  ││
│  │  │  │ seccomp     │  │  │  │  Application    │  │  ││  ││
│  │  │  │             │  │  │  └─────────────────┘  │  ││  ││
│  │  │  │             │  │  └───────────────────────┘  ││  ││
│  │  │  └─────────────┘  └─────────────────────────────┘│  ││
│  │  └───────────────────────────────────────────────────┘  ││
│  └─────────────────────────────────────────────────────────┘│
│                              │                               │
│                        Linux Kernel                          │
│                              │                               │
│                         KVM Module                           │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Networking Model

### 4.1 Unified Mesh Network

All actors communicate over a unified QUIC-based mesh:

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Mesh Network                                │
│                                                                      │
│  ┌───────────┐     QUIC      ┌───────────┐     QUIC      ┌───────────┐
│  │  Node A   │◄─────────────▶│  Node B   │◄─────────────▶│  Node C   │
│  │           │               │           │               │           │
│  │ ┌───────┐ │               │ ┌───────┐ │               │ ┌───────┐ │
│  │ │Actor 1│ │               │ │Actor 3│ │               │ │Actor 5│ │
│  │ └───────┘ │               │ └───────┘ │               │ └───────┘ │
│  │ ┌───────┐ │               │ ┌───────┐ │               │           │
│  │ │Actor 2│ │               │ │Actor 4│ │               │           │
│  │ └───────┘ │               │ └───────┘ │               │           │
│  └───────────┘               └───────────┘               └───────────┘
│       ▲                           ▲                           ▲       │
│       │                           │                           │       │
│       └───────────────────────────┴───────────────────────────┘       │
│                        mTLS Encrypted                                 │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Actor Addressing

Actors are addressed using a hierarchical identifier:

```
actor://<namespace>/<actor-name>/<instance-id>

Examples:
  actor://default/api/0
  actor://production/db/primary
  actor://staging/worker/3
```

### 4.3 Message Flow

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  Client  │────▶│  Mesh    │────▶│  Target  │────▶│  Actor   │
│          │     │  Node    │     │  Node    │     │          │
└──────────┘     └──────────┘     └──────────┘     └──────────┘
     │                │                │                │
     │ 1. Resolve actor               │                │
     │ ◀──────────────┤               │                │
     │                │               │                │
     │ 2. Send message (QUIC)         │                │
     │ ───────────────▶               │                │
     │                │ 3. Route      │                │
     │                │ ─────────────▶│                │
     │                │               │ 4. Deliver     │
     │                │               │ ──────────────▶│
     │                │               │                │
     │                │               │ 5. Response    │
     │                │               │◀───────────────│
     │                │ 6. Route back │                │
     │                │◀──────────────│                │
     │ 7. Response    │               │                │
     │◀───────────────│               │                │
```

### 4.4 TCP Bridge

Legacy TCP clients connect via transparent proxy:

```
┌──────────┐     TCP      ┌──────────┐     QUIC     ┌──────────┐
│  Legacy  │─────────────▶│   TCP    │─────────────▶│  Actor   │
│  Client  │              │  Bridge  │              │          │
└──────────┘              └──────────┘              └──────────┘
```

### 4.5 Backpressure Flow Control

Credit-based flow control prevents buffer overflow:

```
Sender                              Receiver
  │                                    │
  │ 1. Request credit (1000 bytes)    │
  │ ─────────────────────────────────▶│
  │                                    │
  │ 2. Grant credit (800 bytes)       │
  │◀───────────────────────────────── │
  │                                    │
  │ 3. Send data (800 bytes)          │
  │ ─────────────────────────────────▶│
  │                                    │
  │                    4. Buffer 80% full
  │                                    │
  │ 5. Send backpressure signal       │
  │◀───────────────────────────────── │
  │                                    │
  │ 6. Pause sending                   │
```

---

## 5. Security Model

### 5.1 Capability-Based Security

Aether implements deny-by-default access control:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Capability Security Model                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐                                                │
│  │   Actor     │                                                │
│  │  Request    │                                                │
│  └──────┬──────┘                                                │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Capability Enforcer                         │    │
│  │  ┌────────────────────────────────────────────────────┐ │    │
│  │  │  1. Check capability bitmap (O(1))                 │ │    │
│  │  │  2. Verify token signature                         │ │    │
│  │  │  3. Check revocation status                        │ │    │
│  │  │  4. Apply constraints                              │ │    │
│  │  └────────────────────────────────────────────────────┘ │    │
│  └─────────────────────────────────────────────────────────┘    │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────┐    ┌─────────────────┐                     │
│  │    GRANTED      │    │    DENIED       │                     │
│  │  Execute op     │    │  Log & return   │                     │
│  │  Audit success  │    │  error          │                     │
│  └─────────────────┘    └─────────────────┘                     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Capability Types

| Domain | Capability | Example |
|--------|------------|---------|
| **Filesystem** | `fs:read`, `fs:write`, `fs:delete` | `fs:read:/data/*` |
| **Network** | `net:tcp:connect`, `net:tcp:listen` | `net:tcp:connect:10.0.0.0/8:443` |
| **Compute** | `compute:cpu`, `compute:memory` | `compute:cpu:50%` |
| **System** | `sys:clock`, `sys:random` | `sys:clock` |
| **Crypto** | `crypto:hash`, `crypto:encrypt` | `crypto:hash` |

### 5.3 mTLS Architecture

All mesh communication uses mutual TLS:

```
┌──────────────────────────────────────────────────────────────────┐
│                        mTLS Architecture                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌───────────────┐                    ┌───────────────┐          │
│  │   Node A      │                    │   Node B      │          │
│  │               │                    │               │          │
│  │ ┌───────────┐ │   QUIC + mTLS     │ ┌───────────┐ │          │
│  │ │  Cert A   │ │◄──────────────────▶│ │  Cert B   │ │          │
│  │ │  (Ed25519)│ │                    │ │  (Ed25519)│ │          │
│  │ └───────────┘ │                    │ └───────────┘ │          │
│  │               │                    │               │          │
│  │ Verify:       │                    │ Verify:       │          │
│  │ - Cert chain  │                    │ - Cert chain  │          │
│  │ - Expiration  │                    │ - Expiration  │          │
│  │ - Revocation  │                    │ - Revocation  │          │
│  └───────────────┘                    └───────────────┘          │
│                                                                   │
│  Certificate Properties:                                         │
│  - Algorithm: Ed25519                                           │
│  - Lifetime: 24 hours (auto-rotation)                           │
│  - Revocation: OCSP with 30s cache                              │
│  - Transparency: All certs logged to CT                         │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 5.4 Isolation Boundaries

| Boundary | Mechanism | Strength |
|----------|-----------|----------|
| WASM-to-Host | Linear memory sandboxing | Strong |
| WASM-to-WASM | Separate memory instances | Strong |
| VM-to-Host | KVM hardware virtualization | Very Strong |
| VM-to-VM | Separate VMs + cgroups | Very Strong |
| Node-to-Node | mTLS + capability tokens | Strong |

---

## 6. State Management

### 6.1 State Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                       State Management                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                     Local State Cache                         │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐  │   │
│  │  │  Hot Cache │  │ Warm Cache │  │ Actor State (rkyv)     │  │   │
│  │  │  (<10µs)   │  │ (<100µs)   │  │  (zero-copy)           │  │   │
│  │  └────────────┘  └────────────┘  └────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                   FoundationDB Cluster                        │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐  │   │
│  │  │   Node 1   │  │   Node 2   │  │   Node 3               │  │   │
│  │  │ (Leader)   │  │ (Follower) │  │ (Follower)             │  │   │
│  │  └────────────┘  └────────────┘  └────────────────────────┘  │   │
│  │                                                               │   │
│  │  Properties:                                                  │   │
│  │  - ACID transactions                                          │   │
│  │  - Linearizable writes                                        │   │
│  │  - Configurable replication (default: 3)                      │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 State Hydration

Actor state is serialized using rkyv for zero-copy deserialization:

```rust
// State serialization
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct ActorState {
    counters: HashMap<String, u64>,
    data: Vec<u8>,
    config: Config,
}

// Zero-copy hydration
fn hydrate(archive: &[u8]) -> &ArchivedActorState {
    rkyv::check_archived_root::<ActorState>(archive)
        .expect("Invalid archive")
}
```

### 6.3 Checkpoint Protocol

```
┌──────────────┐                    ┌──────────────┐
│  Source Node │                    │ Target Node  │
└──────────────┘                    └──────────────┘
       │                                   │
       │ 1. Suspend actor                  │
       ├──────────────────────────────────▶│
       │                                   │
       │ 2. Serialize state (rkyv)         │
       │    < 50ms                         │
       │                                   │
       │ 3. Transfer archive               │
       │ ─────────────────────────────────▶│
       │                                   │
       │                                   │ 4. Validate archive
       │                                   │    - Checksum
       │                                   │    - Structure
       │                                   │
       │                                   │ 5. Hydrate state
       │                                   │    - Zero-copy
       │                                   │    - < 50ms
       │                                   │
       │ 6. Confirm hydration              │
       │◀───────────────────────────────── │
       │                                   │
       │ 7. Cleanup source                 │
       │                                   │
```

---

## 7. Deployment Architecture

### 7.1 Deployment Philosophy

Aether is designed as a **Post-Container Application OS** that replaces traditional container orchestration. However, we recognize organizations have existing infrastructure investments.

**Two deployment models are supported:**

| Model | Purpose | Recommendation |
|-------|---------|----------------|
| **Native** | Primary production | [DONE] Recommended for all production |
| **Kubernetes** | Transitional/adoption | [WARN] Evaluation only |

### 7.2 Native Deployment Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Bare Metal / VM                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Aether Host Runtime                       │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  │  │
│  │  │  WASM   │  │   VM    │  │  Mesh   │  │  State  │  │  │
│  │  │ Engine  │  │ Manager │  │ Network │  │ Manager │  │  │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                    Linux Kernel (5.15+)                     │
│  io_uring │ KVM │ eBPF │ cgroups │ namespaces              │
└─────────────────────────────────────────────────────────────┘
```

**Benefits:**
- Direct hardware access (io_uring, KVM)
- Minimal latency (~30µs cold start)
- Reduced attack surface
- Full Liquid Compute capabilities

### 7.3 Kubernetes Deployment Architecture (Transitional)

```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Cluster                        │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Pod: Aether                         │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │              Aether Host Runtime                 │  │  │
│  │  │  ┌─────────┐  ┌─────────┐  ┌─────────┐         │  │  │
│  │  │  │  WASM   │  │   VM*   │  │  Mesh   │         │  │  │
│  │  │  │ Engine  │  │ Manager │  │ Network │         │  │  │
│  │  │  └─────────┘  └─────────┘  └─────────┘         │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                              │
│  * KVM access requires privileged mode or device plugins     │
├─────────────────────────────────────────────────────────────┤
│              Container Runtime (containerd/cri-o)           │
├─────────────────────────────────────────────────────────────┤
│                    Linux Kernel + K8s Services              │
└─────────────────────────────────────────────────────────────┘
```

**Limitations:**
- Additional network hop through K8s networking
- Resource overhead from container runtime
- Reduced hardware access (requires device plugins)
- Larger attack surface

**Use Cases:**
- Initial evaluation and testing
- Organizations with existing K8s investment
- Gradual migration path to native

### 7.4 Choosing a Deployment Model

```
                    ┌─────────────────────────────┐
                    │ Do you have existing        │
                    │ bare-metal/VM infra?       │
                    └───────────┬───────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                   YES                     NO
                    │                       │
                    ▼                       ▼
          ┌─────────────┐       ┌─────────────────────────┐
          │ NATIVE      │       │ Can you provision       │
          │ DEPLOY      │       │ bare-metal/VM?          │
          └─────────────┘       └───────────┬─────────────┘
                                              │
                                  ┌───────────┴───────────┐
                                  │                       │
                                 YES                     NO
                                  │                       │
                                  ▼                       ▼
                        ┌─────────────┐       ┌─────────────────┐
                        │ NATIVE      │       │ K8s (plan        │
                        │ DEPLOY      │       │ migration later) │
                        └─────────────┘       └─────────────────┘
```

**For detailed deployment instructions, see [Deployment Guide](deployment_guide.md).**

---

## Appendix: Performance Targets

| Metric | Target | Unit |
|--------|--------|------|
| WASM Cold Start (P99) | <50 | µs |
| VM Cold Start (P99) | <125 | ms |
| Local Message Latency (P99) | <1 | ms |
| Remote Message Latency (P99) | <2 | ms (same DC) |
| State Read (local) | <10 | µs |
| State Write (replicated) | <100 | µs |
| Actors per Node | 100,000 | - |
| Throughput per Node | 10M | msg/s |

---

*For more information, visit https://aether.dev/docs*
