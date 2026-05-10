# PROJECT AETHER: MASTER ARCHITECTURAL SPECIFICATION
**Version:** 2.0.0 | **Date:** May 10, 2026

## 1. Project Philosophy
Aether is a Vertical-Integration Runtime. It deprecates the container-orchestration layer (Docker/K8s) by treating the distributed cluster as a single compiler target. It enables "Liquid Compute," where business logic (Actors) moves transparently across hardware boundaries.

## 2. The Hybrid Execution Model (The "Universal" Goal)
Aether supports a dual-engine execution strategy for both new and legacy workloads:
*   **Engine A (WASM/Native):** High-performance, memory-safe, scale-to-zero. Target: WASI Preview 2 (Component Model).
*   **Engine B (OCI/Firecracker):** Compatibility-mode for legacy monoliths/databases. Target: OCI-compliant containers running inside hardware-virtualized MicroVMs.

## 3. Core Technical Stack
*   **Language:** Rust 2024 Edition (MSRV 1.85).
*   **Host Runtime:** Wasmtime 25 (WASM) + Firecracker (OCI) + Tokio (async runtime).
*   **Networking:** Quinn (QUIC-based Mesh) with automatic TLS.
*   **State Management:** FoundationDB (Metadata/Placements) + InMemoryStore (Local Cache).
*   **Serialization:** rkyv (Zero-copy archives).
*   **CLI:** Clap with derive macros.

## 4. Engineering Invariants
All Aether source code adheres to the following invariants:
1.  **Zero-Panic Policy:** Workspace-level `#![deny(clippy::unwrap_used, clippy::expect_used)]`. No `unwrap()` or `expect()` in production code.
2.  **Deterministic Invariants:** `std::time` and `std::thread` are banned in business logic. All time/randomness must be injected via the Aether-Host capability layer.
3.  **Hardware Sympathy:** Hot-path structures are `#[repr(align(64))]` to prevent L3 cache-line invalidation.
4.  **Safety Audit:** All `unsafe` blocks require `#[deny(unsafe_op_in_unsafe_fn)]` enforcement.

## 5. The "Open-Core" IP Architecture

| Layer | Repository | Licensing | Purpose |
| :--- | :--- | :--- | :--- |
| **Core Engine** | `github.com/WyattAu/aether-core` | Apache 2.0 | Runtime, Local CLI, WASM/OCI Engines, Mesh, Security |
| **Enterprise** | `enterprise/` (subtree) | Proprietary/BSL | Multi-region Orchestrator, Audit/SSO, Time-Travel Debugger |

## 6. System Design Requirements

### 6.1 Capability-Based Security (The "Aether-Host Interface")
*   **Deny-by-Default:** No Actor has access to network sockets or file paths.
*   **Capability Injection:** Access is granted by the Orchestrator via `aether.toml` manifests.
*   **Socket Spoofing:** The Host tunnels legacy TCP connections through the QUIC mesh.

### 6.2 The "Aether.toml" Contract
Every deployment is defined by an `aether.toml` file:
*   **Capabilities:** `networking`, `volumes`, `secrets`.
*   **Deployment Policy:** `instances`, `placement`, `kind` (wasm vs oci).
*   **Contract Definition:** WIT (WASM Interface Type) bindings for Actor communication.

### 6.3 State and Storage
*   **System Actors (Legacy):** Managed via Block Volumes to support persistent DBs.
*   **WASM Actors:** Managed via Aether-State (distributed memory-mapped KV store).
*   **Zero-Copy:** Use `rkyv` for state serialization across the cluster.

## 7. Developer Experience
*   **Deterministic Replay:** The runtime can snapshot and replay distributed state to debug crashes.
*   **Introspection:** `aether inspect` allows developers to view actor memory and call stacks.

## 8. Development Roadmap

| Phase | Milestone | Focus | Status |
| :--- | :--- | :--- | :--- |
| **Phase 1** | **The Local Engine** | Replacing Docker Desktop; local `wasmtime` + `firecracker` management | COMPLETE |
| **Phase 2** | **The Cluster** | Distributed mesh (QUIC), FDB placement maps, and auto-scaling | COMPLETE |
| **Phase 3** | **The Platform** | SSO/Audit features, managed cloud SaaS, and the marketplace | IN PROGRESS |

---

*This specification is the baseline for all development. Any deviation that bypasses the capability-security model requires a manual review and ADR documentation.*
