# THE AETHER ENGINEERING SOP (REVISED 2026)
*Targeting: High-Density Hybrid (WASM + Firecracker) Infrastructure, Zero-Copy Serialization, and Distributed Determinism.*

---

## PART I: THE UNIVERSAL CORE (The Host Runtime)
*Objective: The Aether Daemon is the most secure part of the stack. It handles untrusted WASM and untrusted Legacy Containers.*

### 1.1 Toolchain and Hardened CI
- [x] **REQUIREMENT: Memory-Safe FFI Boundaries**
  - **Tool:** `bindgen` (with hardened headers).
  - **Rule:** When the Aether Daemon talks to the `firecracker` C-binary or `kvm` headers, raw pointer arithmetic is banned. All FFI must be wrapped to enforce Rust lifetimes at the boundary of C/C++ memory.
- [x] **REQUIREMENT: The "Panic-less" Host**
  - **Rule:** `panic = "abort"` is mandatory in all profiles. If an actor causes an unrecoverable state, the Daemon logs the WASM Coredump or VM Snapshot and restarts the actor. The Daemon process itself must remain immutable.

### 1.2 Capability-Based Memory Safety
- [x] **REQUIREMENT: Linear Memory Constraints**
  - **Tool:** `wasmtime` fuel/memory limits.
  - **Rule:** Every WASM actor is spawned with a strict `MemoryLimit` and `InstructionFuel` counter. If exceeded, the Host performs "Silent Trapping" (no crash) and reports the violation as an event.
- [x] **REQUIREMENT: Virtualized I/O (The Shim)**
  - **Rule:** The Host never grants the guest direct access to `std::net` or `std::fs`. All syscalls pass through the `WASI` adapter, which checks the `aether.toml` capabilities manifest before proxying.

---

## PART II: THE DATA PLANE (The "Hot Path")
*Objective: Eliminating latency in actor communication and state hydration.*

### 2.1 Cache-Line and Memory Architecture
- [x] **REQUIREMENT: Cache-Line Alignment**
  - **Tool:** `crossbeam_utils::CachePadded`.
  - **Rule:** All internal queues for the reactor are `#[repr(align(64))]` and `CachePadded`. Structures shared between threads/cores are padded to 64 bytes to prevent false sharing.
- [ ] **REQUIREMENT: The "No-Allocation" Hot Path**
  - **Rule:** During an actor's request-processing loop, `Box`, `Vec`, or `Arc` allocations are forbidden. Use `arrayvec` or stack-based buffers. Dynamic memory must be acquired from thread-local pools initialized at startup.

### 2.2 Deterministic Messaging
- [x] **REQUIREMENT: Time-Travel Injection**
  - **Rule:** All message packets through the Aether Mesh include a `Host-Timestamp`. The WASM actor's `wasi-clocks` implementation returns this packet-provided timestamp, not the CPU's `RDTSC`. This ensures replay across nodes produces identical actor logic.

---

## PART III: THE VIRTUALIZATION BOUNDARY (The "Legacy" Tier)
*Objective: Securely running OCI containers in Firecracker VMs.*

- [x] **REQUIREMENT: The MicroVM Jailing**
  - **Tool:** `firecracker` + `jailer`.
  - **Rule:** All Legacy OCI containers run inside the `jailer` binary, which uses `chroot`, `cgroups`, and `namespaces` to ensure VM guest escape cannot access the Aether Daemon.
- [x] **REQUIREMENT: Block-Device Pinning**
  - **Rule:** If a System Actor requires a block volume, the Host verifies that the `VolumeID` is locked to a single physical NVMe device to prevent concurrent-write corruption.

---

## PART IV: THE ENTERPRISE MODULES (The Private Repo)
*Objective: Compliance and Operational Intelligence.*

- [x] **REQUIREMENT: Cryptographic Identity (mTLS)**
  - **Rule:** All communication between the Host Daemon and the Enterprise Control Plane uses `rustls` with pinned CA certificates. No unencrypted communication is allowed on the cluster network.
- [x] **REQUIREMENT: Audit Log Immutability**
  - **Rule:** All state mutations triggered by the orchestrator are logged as signed events. If the Enterprise Audit feature is enabled, logs are synchronously flushed to the audit-stream before the movement is finalized.

---

## PART V: CROSS-BOUNDARY SAFETY (The "Shim" Rules)

- [x] **REQUIREMENT: Zero-Copy Serialization**
  - **Tool:** `rkyv`.
  - **Rule:** Actor state movement uses `rkyv::Archive`. `serde/bincode` is forbidden for state hydration due to the full recursive-copy overhead.
- [x] **REQUIREMENT: Protocol Bridging (TCP Proxying)**
  - **Rule:** When proxying TCP (Legacy) into QUIC (Mesh), the Aether Host implements a backpressure-aware buffer. TCP-Zero-Window signals force the guest to pause, preventing OOM crashes on the Host.

---

## PART VI: CI/CD AND DETERMINISM (The Aether Pipeline)

- [x] **REQUIREMENT: Binary Reproducibility**
  - **Rule:** The Aether Daemon binary is built with deterministic timestamps (`SOURCE_DATE_EPOCH`). The binary hash must match across all build environments.
- [ ] **REQUIREMENT: Mutation Testing**
  - **Tool:** `cargo-mutants`.
  - **Rule:** Before any release, run `cargo-mutants`. Mutations that do not cause test failures invalidate the build.

---

### Implementation Instructions
1.  **Lint Enforcement:** These rules are enforced via workspace-level clippy lints (`Cargo.toml`) and `deny.toml`. If the compiler rejects code, refactor the architecture rather than bypassing the lint.
2.  **The "SOP" check:** If writing `unwrap`, `expect`, or `std::net` in production code, the Aether Omni-Protocol is being violated. Stop and re-architect.
