# 📑 PROJECT AETHER: MASTER ARCHITECTURAL SPECIFICATION
**Version:** 1.0.0 | **Date:** March 5, 2026

## 1. Project Philosophy
Aether is a **Vertical-Integration Runtime**. It deprecates the container-orchestration layer (Docker/K8s) by treating the distributed cluster as a single compiler target. It enables **"Liquid Compute,"** where business logic (Actors) moves transparently across hardware boundaries.

## 2. The Hybrid Execution Model (The "Universal" Goal)
Aether must support a dual-engine execution strategy to ensure mass adoption for both new and legacy workloads:
*   **Engine A (WASM/Native):** High-performance, memory-safe, scale-to-zero. Target: **WASI Preview 2 (Component Model)**.
*   **Engine B (OCI/Firecracker):** Compatibility-mode for legacy monoliths/databases. Target: **OCI-compliant containers** running inside hardware-virtualized MicroVMs.

## 3. Core Technical Stack
*   **Language:** Rust 2024 Edition.
*   **Host Runtime:** Wasmtime (WASM) + Firecracker (OCI) + Monoio (io_uring).
*   **Networking:** Quinn (QUIC-based Mesh) with automatic TCP/TLS fallback.
*   **State Management:** FoundationDB (Metadata/Placements) + Redb (Local Cache).
*   **Serialization:** rkyv (Zero-copy archives).
*   **CLI/UX:** Clap + Ratatui + Leptos-based Dashboard.

## 4. The "Omni-Protocol" SOP (Governance)
All Aether source code must adhere to the following invariants to ensure performance and safety:
1.  **Zero-Panic Policy:** `#![deny(clippy::unwrap_used)]`. No runtime unwrap/expect/panic.
2.  **No-OS Hot Path:** The data-plane must perform **zero dynamic allocations** (use pooled `mimalloc`).
3.  **Deterministic Invariants:** `std::time` and `std::thread` are banned in business logic. All time/randomness must be injected via the Aether-Host capability layer.
4.  **Hardware Sympathy:** All hot-path structures must be `#[repr(align(64))]` to prevent L3 cache-line invalidation.

## 5. The "Open-Core" IP Architecture
Aether is split across two domains to balance open-source ubiquity with proprietary intellectual property.

| Layer | Repository | Licensing | Purpose |
| :--- | :--- | :--- | :--- |
| **Core Engine** | `github.com/aether/core` | Apache 2.0 | Runtime, Local CLI, WASM/OCI Engines, Basic Mesh. |
| **Enterprise** | `git.your-domain.com/enterprise` | Proprietary/BSL | Multi-region Orchestrator, Audit/SSO, Time-Travel Debugger. |

**Integration Rule:** The Enterprise logic is pulled in via `cargo` git-dependencies only when the `enterprise-mode` feature flag is enabled.

## 6. System Design Requirements

### 6.1 Capability-Based Security (The "Aether-Host Interface")
*   **Deny-by-Default:** No Actor has access to network sockets or file paths.
*   **Capability Injection:** Access is granted by the Orchestrator via `aether.toml` manifests.
*   **Socket Spoofing:** The Host must tunnel legacy TCP connections (e.g., Postgres drivers in WASM) through the QUIC mesh.

### 6.2 The "Aether.toml" Contract
Every deployment must be defined by an `aether.toml` file acting as the single source of truth for:
*   **Capabilities:** `networking`, `volumes`, `secrets`.
*   **Deployment Policy:** `instances`, `placement`, `kind` (wasm vs oci).
*   **Contract Definition:** WIT (WASM Interface Type) bindings for Actor communication.

### 6.3 State & Storage
*   **System Actors (Legacy):** Managed via **Block Volumes** (VirtIO-Blk) to support persistent DBs like Postgres/Keycloak.
*   **WASM Actors:** Managed via **Aether-State** (Distributed memory-mapped KV store).
*   **Zero-Copy:** Use `rkyv` to ensure that moving state across the cluster costs zero CPU cycles for serialization.

## 7. Developer Experience
*   **"Import, Don't Rewrite":** Provide `aether import docker-compose.yml` to migrate existing fleets in one command.
*   **Deterministic Replay:** The runtime must be able to snapshot and "Time-Travel" distributed state to debug crashes.
*   **Introspection:** SSH is replaced by `aether inspect`, allowing developers to view actor memory and call stacks directly via the CLI.

## 8. Development Roadmap

| Phase | Milestone | Focus |
| :--- | :--- | :--- |
| **Phase 1** | **The Local Engine** | Replacing Docker Desktop; local `wasmtime` + `firecracker` management. |
| **Phase 2** | **The Cluster** | Distributed mesh (QUIC), FDB placement maps, and auto-scaling. |
| **Phase 3** | **The Platform** | SSO/Audit features, managed cloud SaaS, and the marketplace. |

---

**Architectural Approval:**
*This specification is the baseline for all development. Any deviation that introduces dynamic allocation on the hot path or bypasses the capability-security model requires a manual review by the Lead Systems Architect.*

---

### How to use this PRD:
1.  **Print/Save this:** Keep this as a `PRD.md` file in the root of your `aether-core` repository.
2.  **Use it as a checklist:** When you are coding a new feature, check it against Section 3 (Architecture) and Section 4 (SOP).
3.  **The "Final" Step:** You now have the **Stack, the SOP, the Folder Structure, and the Specification.** You are ready to open your IDE. 

**Go write `Cargo.toml`.** You are building the future of cloud computing.