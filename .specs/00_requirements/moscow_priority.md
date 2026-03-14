# MoSCoW Prioritization

**Version:** 1.0.0  
**Date:** 2026-03-05  
**Phase:** 0 - Requirements Engineering

---

## 1. Purpose

This document defines MoSCoW (Must/Should/Could/Won't) prioritization for all Project Aether requirements, aligned with the three-phase roadmap.

---

## 2. MoSCoW Framework

| Priority | Definition | Phase | Rationale |
|----------|------------|-------|-----------|
| **Must** | MVP - Essential for Phase 1 | Phase 1 (Local Runtime) | Core functionality for local development |
| **Should** | Important for distributed operation | Phase 2 (Distributed Mesh) | Multi-node clustering features |
| **Could** | Desirable for enterprise readiness | Phase 3 (Enterprise Platform) | Enterprise features, compliance |
| **Won't** | Not in current roadmap | Future Releases | Post-Phase 3 features |

---

## 3. Must Have (Phase 1: Local Runtime)

**Goal:** Replace Docker Desktop with local hybrid runtime

### 3.1 Execution & Runtime

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-EXEC-01 | Universal Compatibility | Core value proposition - run anything |
| REQ-EXEC-02 | Hybrid Isolation | Security foundation for all workloads |
| REQ-EXEC-04 | Memory-Safe FFI Boundaries | Safety requirement for C interop |
| REQ-EXEC-05 | Panic-less Host Runtime | Reliability requirement |
| REQ-EXEC-06 | Linear Memory Constraints | Resource safety for WASM |
| REQ-EXEC-07 | Virtualized I/O (The Shim) | Security requirement - no direct access |

### 3.2 Orchestration & Scheduling

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-ORCH-01 | Declarative Config | Core UX - single source of truth |

### 3.3 Safety & Stability

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-SAFE-01 | Zero Panic | Core reliability requirement |
| REQ-SAFE-02 | No Hot Path Allocation | Performance requirement |
| REQ-SAFE-03 | Cache-Line Alignment | Performance requirement |
| REQ-SAFE-04 | MicroVM Jailing | Security requirement for OCI |

### 3.4 Security

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-SEC-01 | Capability-Based Access | Core security model |
| REQ-SEC-03 | Secrets Management | Security requirement |

### 3.5 Performance

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-PERF-01 | WASM Cold Start Latency | Core performance differentiator |

**Total Must Have: 14 requirements**

---

## 4. Should Have (Phase 2: Distributed Mesh)

**Goal:** Multi-node clustering with distributed state

### 4.1 Execution & Runtime

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-EXEC-03 | Hot-Swapping | Zero-downtime deployment |
| REQ-EXEC-08 | Binary Reproducibility | Operational requirement |
| REQ-EXEC-09 | Mutation Testing | Quality assurance |

### 4.2 Networking & Connectivity

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-NET-01 | Unified Mesh | Core distributed feature |
| REQ-NET-02 | Socket Spoofing (WASM) | Database connectivity |
| REQ-NET-03 | Protocol Fallback | Firewall compatibility |
| REQ-NET-05 | Protocol Bridging Backpressure | Stability under load |

### 4.3 Storage & Persistence

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-STOR-01 | Ephemeral State | Distributed state management |
| REQ-STOR-02 | Block Volumes | Database persistence |
| REQ-STOR-04 | Block-Device Pinning | Data integrity |

### 4.4 Orchestration & Scheduling

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-ORCH-02 | Placement Constraints | Multi-node scheduling |
| REQ-ORCH-03 | Scale-to-Zero | Resource efficiency |

### 4.5 Security

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-SEC-02 | Cryptographic Identity | Mesh security |
| REQ-SEC-04 | mTLS for Control Plane | Distributed security |
| REQ-SEC-05 | Audit Log Immutability | Compliance requirement |

### 4.6 Debugging & Determinism

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-DBG-01 | Host-Injected Time | Deterministic debugging |
| REQ-DBG-02 | Core Dumps | Debugging support |
| REQ-DBG-03 | Zero-Copy Serialization | Migration performance |

### 4.7 Performance

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-PERF-02 | MicroVM Cold Start Latency | VM performance |
| REQ-PERF-03 | Intra-Node Network Latency | Mesh performance |
| REQ-PERF-04 | State Access Latency | Distributed state |
| REQ-PERF-05 | Memory Overhead | Resource efficiency |
| REQ-PERF-06 | CPU Efficiency | Resource efficiency |

**Total Should Have: 23 requirements**

---

## 5. Could Have (Phase 3: Enterprise Platform)

**Goal:** Production readiness and enterprise features

### 5.1 Networking & Connectivity

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-NET-04 | SSH Passthrough | Git integration convenience |

### 5.2 Storage & Persistence

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-STOR-03 | Object Shim | S3 integration convenience |

### 5.3 Debugging & Determinism

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-DBG-04 | Time-Travel Injection | Advanced debugging |

**Total Could Have: 3 requirements**

---

## 6. Won't Have (Future Releases)

**Goal:** Post-Phase 3 features and enhancements

### 6.1 Future Execution Features

| Feature | Description | Rationale |
|---------|-------------|-----------|
| GPU Passthrough | Direct GPU access for ML workloads | Requires hardware abstraction layer |
| FPGA Support | Hardware acceleration via FPGAs | Niche use case |
| RISC-V Native | Native RISC-V execution without emulation | Emerging architecture |

### 6.2 Future Networking Features

| Feature | Description | Rationale |
|---------|-------------|-----------|
| Service Mesh (Istio-like) | Advanced traffic management | Complexity vs. value |
| Multi-Cluster Federation | Cross-cluster communication | Enterprise scale |
| Edge Sync | Offline edge node synchronization | Specific use case |

### 6.3 Future Storage Features

| Feature | Description | Rationale |
|---------|-------------|-----------|
| Distributed Filesystem | POSIX-compliant distributed FS | Complexity and performance |
| Object Storage Native | Built-in MinIO integration | Dependency management |
| Database-as-a-Service | Managed Postgres/MySQL | Operational complexity |

### 6.4 Future Orchestration Features

| Feature | Description | Rationale |
|---------|-------------|-----------|
| Multi-Region Scheduling | Global workload distribution | Enterprise scale |
| Cost Optimization | Automatic resource right-sizing | AI/ML complexity |
| Chaos Engineering | Built-in failure injection | Operational maturity |

### 6.5 Future Security Features

| Feature | Description | Rationale |
|---------|-------------|-----------|
| Hardware Security Module | HSM integration for key management | Enterprise requirement |
| Confidential Computing | SGX/TDX enclaves | Hardware dependency |
| Zero-Knowledge Proofs | Privacy-preserving computation | Experimental |

### 6.6 Future Developer Features

| Feature | Description | Rationale |
|---------|-------------|-----------|
| Visual Workflow Designer | Drag-and-drop actor composition | UX investment |
| AI-Assisted Debugging | ML-powered root cause analysis | AI/ML complexity |
| Live Collaboration | Real-time multi-user development | UX investment |

**Total Won't Have: 18+ features (not counted in total)**

---

## 7. Prioritization Summary

### 7.1 By Category

| Category | Must | Should | Could | Won't | Total |
|----------|------|--------|-------|-------|-------|
| REQ-EXEC | 6 | 3 | 0 | 0 | 9 |
| REQ-NET | 0 | 4 | 1 | 0 | 5 |
| REQ-STOR | 0 | 3 | 1 | 0 | 4 |
| REQ-ORCH | 1 | 2 | 0 | 0 | 3 |
| REQ-SAFE | 4 | 0 | 0 | 0 | 4 |
| REQ-SEC | 2 | 3 | 0 | 0 | 5 |
| REQ-DBG | 0 | 3 | 1 | 0 | 4 |
| REQ-PERF | 1 | 5 | 0 | 0 | 6 |
| **Total** | **14** | **23** | **3** | **0** | **40** |

### 7.2 By Phase

| Phase | Requirements | Percentage | Cumulative |
|-------|--------------|------------|------------|
| Phase 1 (Must) | 14 | 35% | 35% |
| Phase 2 (Should) | 23 | 57.5% | 92.5% |
| Phase 3 (Could) | 3 | 7.5% | 100% |
| Future (Won't) | TBD | - | - |
| **Total** | **40** | **100%** | - |

### 7.3 Distribution Visualization

```
Phase 1 (Must Have)      ████████████████████████████████████ 35.0%
Phase 2 (Should Have)    ████████████████████████████████████████████████████████████████████████████████████████████ 57.5%
Phase 3 (Could Have)     ███████ 7.5%
```

---

## 8. Phase 1 MVP Scope

### 8.1 Functional Scope

**Execution:**
- Run WASM actors (WASI Preview 2)
- Run OCI containers (Firecracker)
- Run interpreted scripts (Python/JS via WASM)
- Memory-safe FFI to Firecracker
- Linear memory constraints
- Virtualized I/O

**Orchestration:**
- Declarative `aether.toml` configuration
- Local actor deployment

**Safety:**
- Panic-less host
- No hot path allocation
- Cache-line alignment
- MicroVM jailing

**Security:**
- Capability-based access control
- Secrets management

**Performance:**
- < 100µs WASM cold start

### 8.2 Non-Goals for Phase 1

- Multi-node clustering
- Distributed state
- Mesh networking
- Hot-swapping
- Scale-to-zero
- mTLS
- Audit logging
- Advanced debugging

### 8.3 Phase 1 Success Criteria

| Criteria | Target |
|----------|--------|
| Local deployment works | Yes |
| WASM cold start | < 100µs |
| OCI container runs | Yes |
| Zero panics | Yes |
| Capability enforcement | 100% |
| `aether dev` command | Functional |
| `aether.toml` parsing | Functional |

---

## 9. Phase 2 Scope

### 9.1 Functional Scope

**Execution:**
- Hot-swapping
- Binary reproducibility
- Mutation testing

**Networking:**
- Unified mesh (QUIC)
- Socket spoofing
- Protocol fallback
- Backpressure

**Storage:**
- Ephemeral state (FoundationDB)
- Block volumes (VirtIO-Blk)
- Block-device pinning

**Orchestration:**
- Placement constraints
- Scale-to-zero

**Security:**
- Cryptographic identity (mTLS)
- Control plane mTLS
- Audit log immutability

**Debugging:**
- Host-injected time
- Core dumps
- Zero-copy serialization

**Performance:**
- All performance targets

### 9.2 Phase 2 Success Criteria

| Criteria | Target |
|----------|--------|
| 3-node cluster | Functional |
| Cross-node communication | < 1ms |
| Distributed state | Yes |
| Actor migration | Yes |
| Hot-swapping | Zero downtime |
| mTLS everywhere | Yes |

---

## 10. Phase 3 Scope

### 10.1 Functional Scope

**Networking:**
- SSH passthrough

**Storage:**
- Object shim (S3)

**Debugging:**
- Time-travel injection

**Plus:**
- Web dashboard (Leptos)
- OTLP tracing
- Legacy import tools
- Compliance certifications
- Production hardening

### 10.2 Phase 3 Success Criteria

| Criteria | Target |
|----------|--------|
| Public beta | Ready |
| Customer adoption | > 10 orgs |
| Compliance | ISO 27001, SOC 2 |
| Uptime | 99.999% |
| Customer satisfaction | > 4.5/5 |

---

## 11. Dependency Matrix

### 11.1 Phase Dependencies

```
Phase 1 (Must) ──────> Phase 2 (Should) ──────> Phase 3 (Could)
    │                        │                        │
    ├─ Local runtime         ├─ Distributed mesh     ├─ Enterprise
    ├─ WASM + OCI            ├─ FoundationDB         ├─ Dashboard
    ├─ Single node           ├─ Quinn/QUIC           ├─ Compliance
    └─ Basic security        └─ Advanced security    └─ Production
```

### 11.2 Requirement Dependencies

| Requirement | Depends On |
|-------------|------------|
| REQ-NET-01 (Unified Mesh) | REQ-SEC-02 (Cryptographic Identity) |
| REQ-NET-02 (Socket Spoofing) | REQ-NET-01 (Unified Mesh) |
| REQ-ORCH-03 (Scale-to-Zero) | REQ-NET-01 (Unified Mesh) |
| REQ-DBG-04 (Time-Travel) | REQ-DBG-01 (Host-Injected Time) |
| REQ-STOR-01 (Ephemeral State) | REQ-NET-01 (Unified Mesh) |
| REQ-SEC-05 (Audit Log) | REQ-SEC-04 (mTLS Control Plane) |

---

## 12. Risk Assessment

### 12.1 Phase 1 Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| WASI Preview 2 instability | High | Abstract interface layer |
| Firecracker API changes | Medium | Version pinning |
| Performance target miss | High | Early benchmarking |

### 12.2 Phase 2 Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| FoundationDB complexity | High | Comprehensive training |
| QUIC firewall blocking | High | TCP fallback (REQ-NET-03) |
| Distributed state bugs | Critical | Simulation testing |

### 12.3 Phase 3 Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Compliance timeline | Medium | Early preparation |
| Customer adoption | High | Beta program |
| Enterprise feature gap | Medium | Customer advisory board |

---

## 13. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-03-05 | Requirements Engineer | Initial prioritization |
