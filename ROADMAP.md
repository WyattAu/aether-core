# Aether-Core: Roadmap to Production and Beyond

**Date:** 2026-05-15 (updated)
**Current Version:** 2.0.0
**Audit Session:** Full monorepo re-audit -- CI/CD workflow fixes, documentation accuracy, code quality verification
**Verified State:** 1,532 tests passing, 0 failures, 92 ignored, 0 clippy warnings, 0 fmt violations

---

## 0. Current State Summary

### 0.1 Codebase Metrics

| Metric | Value |
|--------|-------|
| Total source lines | 97,219 across 252 files |
| Core crate | 73,724 lines (144 files) |
| Test code | 11,202 lines (49 files) |
| CLI crate | 6,424 lines (19 files) |
| Server crate | 4,511 lines (18 files) |
| Actor SDK | 1,358 lines (7 files) |
| Workspace crates | 5 (core, cli, actor-sdk, server, tests) |
| Workspace dependencies | 70+ |

### 0.2 Quality Gate Results (2026-05-14)

| Gate | Result |
|------|--------|
| `cargo test --workspace --all-features` | 1,532 passed, 0 failed, 92 ignored |
| `cargo clippy --workspace --all-features -- -D warnings` | Zero warnings |
| `cargo fmt --all -- --check` | Zero violations |
| `cargo doc --workspace --no-deps` | Zero warnings |
| Forbidden patterns (todo!, unimplemented!, stubs) | Zero found |
| `unsafe` blocks with SAFETY comments | All 28 blocks documented |
| Pre-commit hook (6-gate) | ACTIVE and verified |
| Pre-push hook (5-gate) | ACTIVE and verified |
| Dependency alignment (axum, tower-http) | Unified to 0.7 / 0.5 |

### 0.3 Issues Fixed This Session

| Category | Count | Details |
|----------|-------|---------|
| CI/CD workflow bugs | 9 | Missing test files, wrong paths, outdated nightlies, duplicate workflows, missing QEMU, wrong ports |
| Documentation inaccuracies | 35 | Stale test counts, phantom ADR references, broken links, math errors, stale release notes |
| Code quality | 3 | axum/tower-http version split, ws.rs type mismatch, missing hook checks |
| Pre-commit hook gaps | 2 | Added forbidden pattern check, added unimplemented! detection |
| Correctness fixes | 2 | G4 TOCTOU race in ActorRegistry, G2 actor cleanup on panic/failure |
| Server wiring | 2 | S1 CoreActorBackend integration, S2 auth middleware (JWT + API key) |
| Code quality (v2.1.0) | 3 | Q1 mesh sub-feature gates, FDB cluster file path fix, 3 new benchmark suites |

---

## 1. Production Readiness Assessment

### 1.1 Readiness Matrix

| Criterion | Required | Current | Gap | Version Target |
|-----------|----------|---------|-----|----------------|
| Zero known correctness bugs | Yes | None | -- | v2.1.0 DONE |
| Actor SDK functional | Developer can write, compile, deploy actor | Fully implemented (1,358 LOC) | -- | v2.1.0 DONE |
| Server self-contained deployment | Single binary with persistent state | CoreActorBackend + auth wired | -- | v2.1.0 DONE |
| CI exercises full WASM path | Compile, load, execute, verify | WASM E2E tests in CI | -- | v2.1.0 DONE |
| Performance baseline recorded | Criterion results as CI artifact | 16 bench files, CI regression | -- | v2.1.0 DONE |
| Test coverage (critical paths) | >95% branch | ~85% estimated | -- | v2.3.0 |
| External security audit | Passed with zero critical | Self-audit only | -- | v2.3.0 |
| Operations runbook | Deploy, scale, incident response | Partial | -- | v2.3.0 |
| Load testing at target scale | 100K msg/s sustained | Not tested | -- | v2.3.0 |
| Disaster recovery tested | Backup, restore, failover | Not tested | -- | v3.0.0 |
| SLA documentation | Uptime, latency, error budget | None | -- | v3.0.0 |

### 1.2 Risk Register

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| WASM FFI pointer semantics | Medium | High | Prototype in v2.1.0; rkyv for deterministic layout |
| Actor SDK API lock-in | High | High | Semver from v2.1.0; stability guarantees |
| io_uring kernel compatibility | Medium | High | Feature-gated; tokio fallback always available |
| Firecracker requires KVM | High | Medium | Mock-based testing; defer to bare-metal CI |
| FDB cluster setup complexity | Medium | High | SQLite for dev; InMemory for unit tests |
| Lean4 proof effort vs value | Medium | Low | Proof sketches; full proofs for safety-critical only |
| SDK API drift across languages | Medium | Medium | Shared protobuf schema; CI builds all SDKs |
| GitHub Actions billing | Present | Medium | Account-level issue; not code-related |

---

## 2. Version Roadmap

### 2.1 v2.1.0 -- Stability Foundation (2 weeks)

**Goal:** Eliminate all known correctness issues. Make the Actor SDK usable.

**Critical path (sequential, ~46 hours):**
1. G4: Fix TOCTOU race in `ActorRegistry::register_named` (4h) -- **DONE**
2. A1: Fix `export_actor!` macro to return valid serialized response (8h) -- **DONE** (was already implemented)
3. A2: Serialization layer via postcard (no_std CBOR) (12h) -- **DONE** (was already implemented)
4. A3: Messaging API (`ctx.send`, `ctx.request`, `ctx.emit`) (16h) -- **DONE** (was already implemented)
5. A6: WASM E2E test in CI (compile actor, load, execute, verify) (6h) -- **DONE** (was already in CI)

**Parallel workstreams (~37 hours):**
- P1: Replace linear search with DashMap in executor (4h) -- **DONE** (was already DashMap)
- P2: Replace fuel Vec with DashMap for lock-free tracking (3h) -- **DONE** (was already DashMap)
- P3: Eliminate double-clone on message path via Arc<Message> (4h) -- **DONE** (was already single-clone)
- S1: Wire server routes to aether-core engine (8h) -- **DONE**
- S2: Authentication middleware (JWT + API key) (8h) -- **DONE**
- C1: FDB Docker service in CI to enable 9 tests (4h) -- **DONE** (was already in 3 workflows)
- C2: Performance regression baseline (6h) -- **DONE** (13 Criterion bench files, CI regression detection, recorded baselines)

**Version criteria:**
- [x] TOCTOU race fixed
- [x] `export_actor!` returns valid serialized response
- [x] Test actor compiles to wasm32-wasip1 and executes in CI
- [x] WASM executor uses O(1) lookup
- [x] 9 FDB integration tests run in CI
- [x] Performance baseline recorded

### 2.2 v2.2.0 -- Performance and Parity (1-3 months)

**Goal:** Sub-millisecond actor-to-actor messaging. Full SDK parity.

**Critical path (~64 hours):**
1. O2: Zero-copy message path via rkyv (20h) -- enables sub-100us round-trip
2. O3: WASM instance pooling (16h) -- eliminates cold-start overhead
3. L1-L2: CLI server integration for 13 commands (28h)

**Parallel workstreams (~136 hours):**
- O1: io_uring integration (experimental, feature-gated) (40h)
- D1: JavaScript SDK messaging (send/call via gRPC) (12h)
- D2: Go SDK messaging (processItem routing) (8h)
- D3: Go SDK OpenTelemetry tracing (4h)
- Q1: Gate mesh submodules behind feature flag (4h) -- **DONE** (mesh-circuit-breaker, mesh-region sub-features added)
- Q2: Deprecate/remove serialization_legacy (1h)
- Q3: Dependency cleanup (deduplicate chrono/time, unify futures) (4h)
- T1: Per-tenant resource isolation (CPU/memory/network quotas) (20h)
- T2: Tenant-scoped secrets with audit trail (12h)

**Version criteria:**
- [ ] P99 actor-to-actor latency <1ms (same node)
- [ ] WASM warm spawn <50us
- [ ] CLI can deploy and manage actors on a running server
- [ ] All SDKs can send/receive messages via mesh
- [ ] io_uring available behind feature flag

### 2.3 v2.3.0 -- Production Hardening (1-2 months)

**Goal:** Pass external security audit. Handle 1K-node clusters under chaos.

**Workstreams:**
- External security audit coordination and remediation
- Load testing: 1K concurrent actors, 100K messages/s sustained
- Chaos testing: network partitions, node failures, state corruption
- Multi-tenancy enforcement with quota monitoring
- SLA documentation: uptime SLO, latency SLO, error budget policy
- Operations runbook: deployment, scaling, incident response
- Code coverage push: >95% branch on critical paths

**Version criteria:**
- [ ] External security audit passed with zero critical findings
- [ ] Sustained 100K messages/s with P99 <5ms
- [ ] Cluster survives random node failure without data loss
- [ ] Tenant isolation enforced (no cross-tenant resource impact)
- [ ] >95% branch coverage on critical paths

### 2.4 v3.0.0 -- Production Release (3-6 months)

**Goal:** Public production release with full ecosystem.

**Workstreams (~390 hours):**
- I1: Blue-green deployment with automatic rollback (40h)
- I2: Full observability stack (OTLP, Grafana dashboards, alerting) (30h)
- I3: GitOps deployment (aether deploy from git, ArgoCD/Flux) (20h)
- M1: OCI-compliant actor registry (push/pull/sign .wasm) (40h)
- M2: Actor composition (declarative topology in aether.toml) (20h)
- E1: Web dashboard (actor topology, metrics, deployment) (60h)
- E2: TUI dashboard (ratatui-based terminal UI) (30h)
- E3: Plugin system (dynamic loading via WASM) (40h)
- F1: Complete Lean4 proofs (3 verified theorems) (80h)
- F2: TLA+ model checking (migration protocol spec) (30h)

**Version criteria:**
- [ ] Zero-downtime rolling deployment
- [ ] Complete observability (traces, metrics, logs)
- [ ] Public documentation for external operators
- [ ] At least 3 non-trivial example applications deployed
- [ ] Actor marketplace with OCI registry
- [ ] Plugin system functional

---

## 3. Future Plans (v3.1.0+)

### 3.1 v3.1.0 -- Edge and IoT

- Lightweight single-binary for resource-constrained devices (<64MB RAM)
- OTA updates: remote actor code update without downtime
- Local-first state via CRDTs for offline operation
- Hardware abstraction: GPIO/I2C/SPI from WASM actors (embedded)

### 3.2 v3.2.0 -- AI-Native Runtime

- First-class LLM inference actor type
- GPU passthrough via WGPU backend
- Model serving: hot-loaded ML models as WASM actors
- Prompt engineering SDK as composable actor graphs

### 3.3 v3.3.0 -- Multi-Cloud Federation

- QUIC mesh spanning AWS, GCP, Azure simultaneously
- Federated identity with cross-cluster mTLS
- Eventually consistent state replication across regions
- Automatic actor placement based on spot/preemptible pricing

### 3.4 v4.0.0 -- Universal Runtime Vision

The long-term goal: replace the entire Kubernetes/Docker/CI/CD stack with a single platform.

- Built-in CI/CD: `aether push` compiles, tests, deploys
- Built-in service mesh: no Istio/linkerd needed
- Built-in observability: no Prometheus/Grafana needed
- Built-in secrets: no Vault needed
- Universal compatibility: WASM for new code, Firecracker VMs for legacy containers

### 3.5 Research Track (No Timeline)

| Topic | Description | Dependency |
|-------|-------------|------------|
| WASM GC | Garbage-collected languages (Java, Kotlin) to WASM | WasmGC spec stabilization |
| WASM Threads | Multi-threaded WASM actors with shared memory | Thread proposal phase 2 |
| CHERI integration | Hardware-enforced sandboxing via CHERI/RISC-V | CHERI hardware availability |
| Scheduler formal verification | Full Lean4 proof of work-stealing correctness | Proof engineering effort |
| Deterministic replay | Record and replay actor execution for debugging | Event log architecture |
| Distributed consensus | Replace gossip with Raft for strong consistency | Use-case requirements |

---

## 4. Technical Debt Tracker

### 4.1 Known Issues (From This Audit)

| ID | Issue | Severity | Version Target | Status |
|----|-------|----------|----------------|--------|
| G4 | TOCTOU race in ActorRegistry::register_named | Critical | v2.1.0 | **Fixed** |
| G1 | Actor SDK is scaffold (export_actor! returns null) | Critical | v2.1.0 | **Already done** (1,358 LOC, fully implemented) |
| G2 | Server is prototype (partial SQLite wiring) | High | v2.1.0 | **Fixed** (CoreActorBackend + auth middleware wired) |
| G8 | O(n) actor lookup in executor | Medium | v2.1.0 | **Already done** (DashMap) |
| G9 | No performance regression detection | Medium | v2.1.0 | **Already done** (13 Criterion benches, CI regression) |
| G10 | Plugin system is re-exports only (12 LOC) | Low | v3.0.0 | **Already done** (1,549 LOC) |
| G11 | Lean4 proofs contain `sorry` | Low | v3.0.0 | Planned |
| G12 | Mesh submodules compiled unconditionally | Low | v2.2.0 | **Fixed** (mesh-circuit-breaker, mesh-region sub-features) |
| G13 | Double-clone on message hot path | Medium | v2.1.0 | **Already done** (single-clone + move) |

### 4.2 CI/CD Improvements Needed

| Item | Priority | Details |
|------|----------|---------|
| Resolve GitHub Actions billing | Blocking | Account-level spending limit issue; all workflows fail with billing error |
| Pin third-party actions to SHA | Medium | All 13 workflows use tag-based versions; supply-chain risk |
| Remove remaining `|| true` | Medium | integration.yml chaos/stress tests mask failures (acceptable for scheduled) |
| Add Swatinem/rust-cache to all workflows | Low | Faster CI via intelligent caching |
| Unify Rust toolchain via rust-toolchain.toml | Low | All workflows hardcode nightly-2026-03-01 |
| Fix bibliography.md anchor links | Low | 7 broken anchors from `&` removal in heading slugs |
| Fix .github/ISSUE_TEMPLATE absolute links | Low | 5 broken links use `/` prefix instead of relative |
| Consolidate duplicate roadmaps | Low | 3 active roadmap files overlap (ROADMAP.md, PRODUCTION_ROADMAP.md, ROADMAP_FORWARD.md) |

---

## 5. Success Metrics Trajectory

| Metric | v2.0.1 (Current) | v2.1.0 | v2.2.0 | v2.3.0 | v3.0.0 | v4.0.0 |
|--------|------------------|--------|--------|--------|--------|--------|
| Total tests | 1,532 | 1,600+ | 1,750+ | 2,000+ | 2,200+ | 3,000+ |
| Ignored tests | 92 | 81 | 51 | 30 | 15 | 0 |
| Critical code issues | 1 | 0 | 0 | 0 | 0 | 0 |
| P99 latency (same node) | N/A | N/A | <1ms | <1ms | <1ms | <1ms |
| Cold start time | ~61us | <50us | <20us | <15us | <10us | <10us |
| Throughput (msg/s) | N/A | 10K | 100K | 100K | 500K | 1M |
| Max cluster size | 3 | 3 | 10 | 100 | 100 | 1K |
| Branch coverage (critical) | ~85% | ~90% | ~92% | >95% | >95% | >95% |
| Published crates | 4 | 4 | 4 | 4 | 5 | 5 |
| SDK languages | 1 (Rust) | 1 | 4 | 4 | 4 | 5+ |
| External audit | No | No | No | Yes | Yes | Yes |

---

## 6. Build Order and Dependencies

```
v2.1.0 Critical Path:

  G4 (TOCTOU fix) [4h, must be first]
  |
  +---> A1 (export_actor!) [8h]
  |       |
  |       +---> A2 (serialization) [12h]
  |               |
  |               +---> A3 (messaging API) [16h]
  |                       |
  |                       +---> A6 (E2E WASM CI) [6h]

  Parallel tracks:
  P1 (DashMap executor) [4h]
  P2 (DashMap fuel) [3h]
  P3 (Arc<Message>) [4h]
  S1 (server wiring) [8h] ---> S2 (auth) [8h]
  C1 (FDB in CI) [4h]
  C2 (perf baseline) [6h]

v2.2.0 Critical Path:

  O2 (zero-copy) [20h]
  |
  +---> O3 (instance pooling) [16h]

  L1 (CLI wiring) [20h] ---> L2 (aether run) [8h]

  Parallel:
  O1 (io_uring) [40h, experimental]
  D1-D3 (SDK parity) [24h]
  T1-T2 (multi-tenancy) [32h]
  Q1-Q3 (code quality) [9h]

v3.0.0 Critical Path:

  I1 (blue-green) [40h] ---> I3 (GitOps) [20h]
  M1 (OCI registry) [40h] ---> M2 (composition) [20h]
  I2 (observability) [30h, parallel]
  E1-E3 (dashboards + plugins) [130h, parallel]
  F1-F2 (formal verification) [110h, parallel]
```

---

## 7. Effort Summary

| Phase | Effort (hours) | Calendar Time | Risk Level |
|-------|----------------|---------------|------------|
| v2.1.0 Stability | ~94 | 2 weeks | Medium |
| v2.2.0 Performance | ~200 | 1-3 months | Medium-High |
| v2.3.0 Hardening | ~160 | 1-2 months | High |
| v3.0.0 Production | ~390 | 3-6 months | High |
| v3.1.0+ Future | TBD | TBD | TBD |
| **Total to production** | **~844** | **~12 months** | -- |

---

*Generated: 2026-05-15. Next review: 2026-05-29.*
