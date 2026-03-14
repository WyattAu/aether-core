# 📄 PROJECT AETHER: PRODUCT REQUIREMENTS DOCUMENT (PRD)
**Version:** 1.0.0-draft  
**Date:** March 4, 2026  
**Status:** **FINAL ARCHITECTURE**  
**Classification:** Internal Confidential  

---

## 1. Executive Summary & Vision
**Project Aether** is a **Hybrid Application Operating System** designed to replace the Kubernetes/Docker stack. It collapses the compiler, orchestrator, and runtime into a single, vertically integrated platform.

**The Core Value Proposition:**  
Aether provides a **"Universal Runtime"** that executes:
1.  **Modern Code:** As **WebAssembly (WASM) Actors** (Microsecond cold starts, zero overhead).
2.  **Legacy Code:** As **Firecracker MicroVMs** (OCI Container compatibility).

This allows organizations to "Lift and Shift" existing Docker workloads (Keycloak, Postgres, Gitea) immediately, while incrementally optimizing high-value services to Native WASM for 90% cost reduction.

---

## 2. System Architecture: The "Final Boss" Stack

### 2.1 Host Plane (The Node Kernel)
*   **Language:** Rust (2024 Edition).
*   **Async Runtime (Data Plane):** **Monoio** (io_uring). Thread-per-core architecture with pinned memory to eliminate context switching.
*   **Async Runtime (Control Plane):** **Tokio**.
*   **Execution Engines:**
    *   **Wasmtime:** For Tier 1 (Native) and Tier 2 (Interpreted) actors.
    *   **Firecracker (KVM):** For Tier 3 (Legacy OCI/Docker) actors.
*   **Networking:** **Quinn** (QUIC) for the internal mesh. **Userspace Netstack** (smoltcp/netstack) for masquerading TCP sockets over QUIC.

### 2.2 Data Plane (The Distributed State)
*   **Cluster Metadata:** **FoundationDB**. Stores the actor placement map and cluster topology with strict ACID guarantees.
*   **Actor State:** **rkyv**. Zero-copy serialization for WASM memory hydration.
*   **Block Storage:** **VirtIO-Blk**. Interface for attaching NVMe/EBS volumes to Firecracker VMs (System Actors).

---

## 3. Functional Requirements

### 3.1 Execution & Runtime
*   **[REQ-EXEC-01] Universal Compatibility:** The runtime must accept and execute:
    *   `.wasm` binaries (WASI Preview 2 Component Model).
    *   OCI Container Images (Docker format).
    *   Interpreted Scripts (Python/JS) via pre-compiled WASM shims.
*   **[REQ-EXEC-02] Hybrid Isolation:**
    *   WASM actors must be isolated via **Linear Memory Sandboxing**.
    *   Legacy actors must be isolated via **KVM Hardware Virtualization**.
*   **[REQ-EXEC-03] Hot-Swapping:** The runtime must support updating an Actor's code without dropping active connections (using traffic shifting).

### 3.2 Networking & Connectivity
*   **[REQ-NET-01] The Unified Mesh:** Aether must provide a single overlay network where a WASM Actor can call a Firecracker VM via local DNS (e.g., `http://postgres`).
*   **[REQ-NET-02] Socket Spoofing (WASM):** The runtime must intercept standard TCP `connect()` calls from WASM modules and tunnel them over the internal QUIC mesh to support standard DB drivers.
*   **[REQ-NET-03] Protocol Fallback:** The Mesh must default to UDP/QUIC but automatically downgrade to TCP/TLS if corporate firewalls block UDP.
*   **[REQ-NET-04] SSH Passthrough:** The Ingress must support routing raw TCP traffic (Port 22) to specific System Actors (e.g., Gitea) to support git operations.

### 3.3 Storage & Persistence
*   **[REQ-STOR-01] Ephemeral State:** Support for fast, in-memory state for WASM actors backed by the distributed KV store.
*   **[REQ-STOR-02] Block Volumes:** Support for creating and attaching persistent disk images (`.img` or cloud volumes) to Firecracker VMs for legacy databases (Postgres/MySQL).
*   **[REQ-STOR-03] Object Shim:** A virtual filesystem driver that maps file operations in WASM to S3/MinIO API calls transparently.

### 3.4 Orchestration & Scheduling
*   **[REQ-ORCH-01] Declarative Config:** All deployments must be defined via `aether.toml` (Capabilities-based), not imperative scripts.
*   **[REQ-ORCH-02] Placement Constraints:** Support for **Pinning** (e.g., "Actor X must run on Node Y with the NVMe drive").
*   **[REQ-ORCH-03] Scale-to-Zero:** Stateless WASM actors must scale to zero when idle and wake in <50ms upon request.

---

## 4. Non-Functional Requirements (The SOP)

### 4.1 Stability & Safety
*   **[SOP-SAFE-01] Zero Panic:** The Host Runtime must compile with `#![deny(clippy::unwrap_used)]`. No thread panics allowed.
*   **[SOP-SAFE-02] Memory Safety:** The Data Plane (Hot Path) must not perform dynamic heap allocations (`malloc`) during request processing. Use `mimalloc` and pooling.

### 4.2 Security
*   **[SOP-SEC-01] Capability-Based Access:** By default, actors have **Zero Trust**. They cannot access network or disk unless explicitly granted in `aether.toml`.
*   **[SOP-SEC-02] Cryptographic Identity:** Every actor instance receives an ephemeral mTLS certificate upon boot. All mesh traffic is encrypted.
*   **[SOP-SEC-03] Secrets Management:** Secrets are injected directly into process memory (env vars or memory mapped). They are never written to disk in plaintext.

### 4.3 Determinism & Debugging
*   **[SOP-DBG-01] Host-Injected Time:** WASM actors must rely on the Host for time and randomness to enable **Deterministic Replay** debugging.
*   **[SOP-DBG-02] Core Dumps:** On crash, the runtime must export a standard WASM Coredump or VM Snapshot for offline analysis.

---

## 5. Developer Experience (DevEx)

### 5.1 The `aether.toml` Specification
The configuration file acts as the contract between the code and the cluster.

```toml
[project]
name = "enterprise-stack"

# Tier 1: Native WASM (Stateless API)
[[actor]]
name = "api-gateway"
kind = "wasm"
image = "build/api.wasm"
instances = "autoscaling"
[actor.capabilities]
  networking = "public"

# Tier 3: Legacy OCI (Stateful DB)
[[actor]]
name = "database"
kind = "oci"
image = "postgres:15"
instances = 1
[actor.capabilities]
  networking = "private"
  [actor.capabilities.volumes]
    data = { path = "/var/lib/postgresql/data", size = "50GB" }
```

### 5.2 The CLI Tooling
*   **`aether dev`:** Starts a local environment. WASM runs native; OCI runs in lightweight local VMs.
*   **`aether deploy`:** Pushes artifacts to the registry and applies the config to the cluster.
*   **`aether inspect <id>`:** Opens a live debugger/introspection session into a running actor.
*   **`aether import docker-compose.yml`:** Automigration tool to convert legacy stacks to `aether.toml`.

---

## 6. Business & Licensing Strategy

### 6.1 Licensing Model: Open Core
*   **Aether Engine (Apache 2.0):** The CLI, The Runtime (Wasmtime/Firecracker integration), Local Execution, Basic Mesh.
*   **Aether Enterprise (Proprietary):** The Global Orchestrator (Multi-region scheduling), The SSO/Audit Modules, The "Time Travel" Debugger backend.

### 6.2 Monetization
1.  **Managed Aether Cloud:** SaaS platform charging for compute + data.
2.  **Enterprise Appliance:** Self-hosted license for Banks/Govs needing air-gapped deployments.

---

## 7. Implementation Roadmap

### Phase 1: The "Local-First" Runtime (Months 0-6)
*   **Goal:** Replace Docker Desktop.
*   **Tech:** Build the Rust Host that can launch both `wasmtime` modules and `firecracker` VMs locally using a unified `aether.toml`.
*   **Milestone:** Running `aether dev` brings up a Keycloak (VM) + Rust API (WASM) stack on a laptop.

### Phase 2: The Distributed Mesh (Months 6-12)
*   **Goal:** Multi-Node Clustering.
*   **Tech:** Implement FoundationDB integration for state and Quinn (QUIC) for the overlay network.
*   **Milestone:** A 3-node cluster where a WASM actor on Node A can query a Postgres VM on Node B.

### Phase 3: The Enterprise Platform (Months 12-18)
*   **Goal:** Production Readiness.
*   **Tech:** Build the Web Dashboard (Leptos), OTLP Tracing integration, and the "Legacy Import" tools.
*   **Milestone:** Public Beta Launch.

---

## 8. Risk Analysis & Mitigation

| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| **Java/C# Perf in WASM** | High | **Strategy:** Default managed languages to Firecracker VMs until WasmGC matures. |
| **UDP Blocking (Corp Firewalls)** | High | **Strategy:** Implement automatic TCP/TLS fallback in the mesh. |
| **Storage Latency** | Med | **Strategy:** Use pinned NVMe volumes for Databases; do not attempt distributed filesystems. |
| **Adoption Friction** | High | **Strategy:** Ensure `aether import docker-compose` works flawlessly for "Day 1" migration. |

---

**Approved By:**  
Senior Systems Architect  
Date: 04/03/2026