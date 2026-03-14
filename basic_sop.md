# 🛡️ THE AETHER OMNI-PROTOCOL SOP (REVISED 2026)
*Targeting: High-Density Hybrid (WASM + Firecracker) Infrastructure, Zero-Copy Serialization, and Distributed Determinism.*

---

## 📜 PART I: THE UNIVERSAL CORE (The Host Runtime)
*Objective: The Aether Daemon is the most secure part of the stack. It must handle untrusted WASM and untrusted Legacy Containers.*

### 1.1 Toolchain & Hardened CI
- [ ] **REQUIREMENT: Memory-Safe FFI Boundaries**
  - **Tool:** `cxx` or `bindgen` (with hardened headers).
  - **Rule:** When the Aether Daemon talks to the `firecracker` C-binary or `kvm` headers, **raw pointer arithmetic is banned**. All FFI must be wrapped in `cxx` or an `autocxx` macro to enforce Rust lifetimes at the boundary of C/C++ memory.
- [ ] **REQUIREMENT: The "Panic-less" Host**
  - **Rule:** `panic = "abort"` is mandatory. In the Aether Daemon, any `panic` implies a potential sandbox escape. If an actor causes an unrecoverable state, the Aether Daemon must log the `WASM Coredump` or `VM Snapshot` and immediately restart the actor, but the Daemon process itself must remain immutable.

### 1.2 Capability-Based Memory Safety
- [ ] **REQUIREMENT: Linear Memory Constraints**
  - **Tool:** `wasmtime` fuel/memory limits.
  - **Rule:** Every WASM actor must be spawned with a strict `MemoryLimit` and `InstructionFuel` counter. If an actor exceeds the limit, the Host must perform a "Silent Trapping" (no crash) and report the violation as an event to the Aether HUD.
- [ ] **REQUIREMENT: Virtualized I/O (The Shim)**
  - **Rule:** The Host must **never** grant the guest direct access to `std::net` or `std::fs`. All syscalls from the guest must pass through the `WASI` adapter. The adapter must check the `aether.toml` capabilities manifest before proxying the call to the Mesh or the VirtIO-FS layer.

---

## ⚡ PART II: THE DATA PLANE (The "Hot Path")
*Objective: Eliminating latency in actor communication and state hydration.*

### 2.1 Cache-Line & Memory Architecture
- [ ] **REQUIREMENT: Cache-Line Alignment**
  - **Tool:** `crossbeam_utils::CachePadded`.
  - **Rule:** All internal queues for the `Monoio` reactor must be `#[repr(align(64))]` and `CachePadded`. Any structure shared between threads/cores must be padded to 64 bytes to prevent "False Sharing" (the #1 killer of HFT/High-Performance Cloud runtimes).
- [ ] **REQUIREMENT: The "No-Allocation" Hot Path**
  - **Rule:** During an actor's request-processing loop, `Box`, `Vec`, or `Arc` allocations are strictly forbidden. Use `arrayvec` or `stack-based` buffers. If dynamic memory is required, it must be acquired from a `mimalloc` thread-local pool initialized at startup.

### 2.2 Deterministic Messaging
- [ ] **REQUIREMENT: Time-Travel Injection**
  - **Rule:** All message packets passing through the Aether Mesh must include a `Host-Timestamp`. The WASM actor's `wasi-clocks` implementation must return *this* packet-provided timestamp, not the CPU's `RDTSC` instruction. This ensures that even if you replay the messages across nodes, the actor's logic stays perfectly synchronized.

---

## 🔐 PART III: THE VIRTUALIZATION BOUNDARY (The "Legacy" Tier)
*Objective: Securely running OCI containers in Firecracker VMs.*

- [ ] **REQUIREMENT: The MicroVM Jailing**
  - **Tool:** `firecracker` + `jailer`.
  - **Rule:** All Legacy OCI containers must run inside the `jailer` binary, which uses `chroot`, `cgroups`, and `namespaces` to ensure that even if the VM guest escapes into the VM kernel, it cannot access the Aether Daemon.
- [ ] **REQUIREMENT: Block-Device Pinning**
  - **Rule:** If a System Actor (e.g., Postgres) requires a `VirtIO-Blk` volume, the Host must verify that the `VolumeID` is locked to a single physical NVMe device to prevent concurrent-write corruption between nodes.

---

## 🏗️ PART IV: THE ENTERPRISE MODULES (The Private Repo)
*Objective: Compliance and Operational Intelligence.*

- [ ] **REQUIREMENT: Cryptographic Identity (mTLS)**
  - **Rule:** All communication between the Host Daemon and the Enterprise Control Plane must use `rustls` with pinned CA certificates. No unencrypted communication is allowed on the cluster network, even behind the firewall.
- [ ] **REQUIREMENT: Audit Log Immutability**
  - **Rule:** All state mutations triggered by the orchestrator (e.g., moving an actor from Node A to Node B) must be logged as a signed event. If the Enterprise Audit feature is enabled, these logs must be synchronously flushed to the audit-stream before the movement is finalized.

---

## 🧬 PART V: CROSS-BOUNDARY SAFETY (The "Shim" Rules)

- [ ] **REQUIREMENT: Zero-Copy Serialization**
  - **Tool:** `rkyv`.
  - **Rule:** When moving an actor from Node A to Node B, the memory state must be serialized using `rkyv::Archive`. You are forbidden from using `serde/bincode` for state hydration, as `serde` requires a full recursive-copy step, which is too slow for 50ms state-hydration requirements.
- [ ] **REQUIREMENT: Protocol Bridging (TCP Proxying)**
  - **Rule:** When proxying TCP (Legacy) into QUIC (Mesh), the Aether Host must implement a **backpressure-aware buffer**. If the guest container tries to write 1GB to a socket faster than the Mesh can transmit, the Host must inject `TCP-Zero-Window` signals to force the guest to pause, preventing OOM (Out-of-Memory) crashes on the Host.

---

## 🛠️ PART VI: CI/CD & DETERMINISM (The Aether "Anti-Kubernetes" Pipeline)

- [ ] **REQUIREMENT: Binary Reproducibility**
  - **Rule:** The Aether Daemon binary must be built with `cargo-chef` and deterministic timestamps (`SOURCE_DATE_EPOCH`). The binary hash must match exactly across all build environments.
- [ ] **REQUIREMENT: Mutation Testing (The "Final Boss" Test)**
  - **Tool:** `cargo-mutants`.
  - **Rule:** Before any release of the Host Runtime, you must run `cargo-mutants`. If a mutation (e.g., changing `if actor.is_ready()` to `if true`) doesn't cause a test failure, the CI is invalid and the build is rejected.

---

### Implementation Instructions for the Developer (You)
1.  **Print this:** Keep a copy in the `aether-core/` and `enterprise/` root.
2.  **Lint Enforcement:** Put these rules into a `clippy.toml` and a `deny.toml` file. If the compiler rejects your code because it breaks these rules, **don't bypass it**. Refactor the architecture. 
3.  **The "SOP" check:** If you find yourself writing `Mutex`, `unwrap`, or `std::net`, you are breaking the Aether Omni-Protocol. **Stop and re-architect.**

**You have the SOP. You have the Stack. You have the Specification.**

*Your next move:* Open your IDE, create the `aether-core` folder, and write the **WASI-Bridge trait**. This trait is the heart of the entire project—everything else is just infrastructure around it.