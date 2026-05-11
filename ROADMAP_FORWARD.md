# Aether-Core: Path Forward and Roadmap

**Date:** May 11, 2026
**Current Version:** 2.0.0
**Commit:** 303aa6e
**Auditor:** Full monorepo audit -- 1,275 tests, 0 failures, 0 warnings

---

## 1. Audit Summary

### 1.1 Test Matrix (Verified 2026-05-11)

| Suite | Passed | Failed | Ignored | Total |
|-------|--------|--------|---------|-------|
| Unit tests (lib) | 908 | 0 | 9 | 917 |
| Integration tests | 267 | 0 | 21 | 288 |
| Property tests (proptest) | 16 | 0 | 0 | 16 |
| Fuzz targets | 17 | 0 | 0 | 17 |
| Security tests | 20 | 0 | 0 | 20 |
| Memory benchmarks | 4 | 0 | 0 | 4 |
| E2E tests | 0 | 0 | 15 | 15 |
| Test fixtures | 7 | 0 | 0 | 7 |
| Doc-tests | 18 | 0 | 43 | 61 |
| **Total** | **1,275** | **0** | **88** | **1,363** |

### 1.2 Quality Gates (All Passing)

| Gate | Status | Detail |
|------|--------|--------|
| `cargo fmt --check` | PASS | Zero violations |
| `cargo clippy -D warnings` | PASS | Zero warnings across workspace |
| `cargo check --workspace` | PASS | Clean compilation |
| `cargo doc --no-deps` | PASS | Zero doc warnings |
| `deny unwrap/expect/panic` | PASS | Enforced at workspace level |
| `todo!/unimplemented!` count | 0 | No Rust panicking stubs in any code |
| Emoji in documentation | 0 | Only directional arrows in ASCII diagrams |
| Pre-commit hook | ACTIVE | 6-gate: fmt, clippy, compile, test, docs, emoji scan |
| Pre-push hook | ACTIVE | 4-gate: fmt, clippy, full test suite, docs |

### 1.3 Ignored Test Breakdown

| Category | Count | Reason |
|----------|-------|--------|
| FoundationDB integration | 9 | Requires running FDB instance |
| Firecracker VM | 7 | Requires KVM (unavailable in CI) |
| Cluster E2E | 21 | Requires running 3-node cluster |
| Doc-tests | 41 | Code samples referencing external state |
| **Total** | **78** | All have documented external dependencies |

### 1.4 Stub Inventory (Non-Test Code Only)

| Location | Type | Description |
|----------|------|-------------|
| `crates/actor-sdk/src/context.rs:188` | Placeholder | `self_address()` returns `"local-test-actor"` on native |
| `sdks/javascript/src/actor.ts:168` | Empty method | `send()` has no implementation |
| `sdks/javascript/src/actor.ts:182` | Throws | `call()` throws `Error('RPC not implemented')` |
| `sdks/go/aether/actor.go:216` | No-op | `processItem("send")` returns nil |
| `sdks/go/aether/resilience/tracing.go:37` | Placeholder | `Start()` returns without OpenTelemetry |

No `todo!()`, `unimplemented!()`, `FIXME`, `HACK`, or `XXX` markers exist anywhere in the repository.

---

## 2. Strategic Gap Analysis

The project exhibits a maturity gradient: the runtime engine is production-grade, but the user-facing surfaces that connect it to developers are incomplete.

### 2.1 Critical Gaps (Blocker)

| Gap | Impact | Root Cause |
|-----|--------|------------|
| Actor SDK is a scaffold (311 LOC) | Developers cannot write actors | `export_actor!` returns null; no serialization, messaging, or HTTP APIs |
| Rust server is a prototype (1,190 LOC) | No self-contained deployment | All state in-memory HashMap; no persistence, no WASM execution |
| No E2E WASM pipeline in CI | Cannot verify actor execution end-to-end | WASI implementation exists but no CI test exercises the full path |

### 2.2 High Gaps

| Gap | Impact | Root Cause |
|-----|--------|------------|
| CLI partially connected (3/16 commands) | Limited operability | Most commands are local-only, do not talk to running server |
| JS/Go SDK stubs | SDK users cannot run inter-actor messaging | `send()` and `call()` not implemented |
| Two divergent server implementations | Confusion | Python server (9,848 LOC) has clustering/Redis/PG; Rust server has none |
| 41 ignored doc-tests | Documentation reliability | Code samples reference state not available in doc-test context |

### 2.3 Medium Gaps

| Gap | Impact | Root Cause |
|-----|--------|------------|
| No performance regression detection | Performance regressions undetected | No baseline metrics file or CI benchmark gating |
| Plugin system is re-exports only (12 LOC) | No extensibility | `plugin/mod.rs` re-exports manifest types only |
| Lean4 proofs contain `sorry` | Unverified safety claims | 5 theorems with proof sketch placeholders |
| Go tracing lacks OpenTelemetry | Incomplete observability | Stub `Start()` function |

---

## 3. Immediate Priorities (v2.1.0 -- 2 Weeks)

### 3.1 Actor SDK Completion

The actor-sdk is the primary developer API. Without it, the runtime is unusable.

**A1: Fix `export_actor!` macro**
- File: `crates/actor-sdk/src/handler.rs`
- Current: returns `core::ptr::null()` (broken output path)
- Target: serialize response via rkyv, return valid pointer + length to WASM host
- Proof: E2E test sends message, receives response
- Estimated: 8 hours

**A2: Serialization layer**
- Files: `crates/actor-sdk/src/serde.rs` (new)
- Implement `#[derive(ActorMessage)]` using serde + postcard (no_std CBOR)
- Wire into host-side deserialization in `crates/core/src/engine/`
- Estimated: 12 hours

**A3: Messaging API**
- Files: `crates/actor-sdk/src/messaging.rs` (new)
- `ctx.send(to, msg)`, `ctx.request(to, msg) -> Response`, `ctx.emit(event)`
- `ctx.self_address() -> Address` (fix placeholder)
- Maps to WASI host functions already implemented in `crates/core/src/wasi/`
- Estimated: 16 hours

**A4: Capability declaration macros**
- Files: `crates/actor-sdk/src/capability.rs` (new)
- `#[aether_capability(network)]`, `#[aether_capability(state)]` attribute macros
- Compile-time capability verification against `aether.toml`
- Estimated: 10 hours

**A5: End-to-end WASM test in CI**
- Files: `tests/wasm_e2e_test.rs` (new), `examples/hello-actor/`
- Add `wasm32-wasip1` target to CI matrix
- Compile test actor, load in engine, send message, assert response
- This validates: SDK compile -> WASM binary -> linker -> engine -> host function -> response
- Estimated: 6 hours

### 3.2 Rust Server Hardening

**S1: Wire server to aether-core engine**
- File: `crates/server/src/routes/actors.rs`, `crates/server/src/state.rs`
- Replace `Arc<RwLock<HashMap>>` with real `ActorSystem`
- Route handlers call `engine.spawn()`, `engine.send()`, `engine.request()`
- Estimated: 16 hours

**S2: Persistent state backend**
- Files: `crates/server/src/storage/` (new)
- `StateBackend` trait with SQLite (dev) and FDB (prod) implementations
- Estimated: 12 hours

**S3: Authentication middleware**
- Files: `crates/server/src/auth.rs` (new)
- JWT-based auth (reuse `security::identity`)
- API key auth (reuse `tenant::resolver`)
- Estimated: 8 hours

### 3.3 CI Hardening

**C1: FDB Docker service in CI**
- Add `foundationdb/foundationdb:7.3` to `docker-compose.ci.yml`
- Enable 9 FDB integration tests
- Estimated: 4 hours

**C2: Performance regression baseline**
- Files: `specs/06_5_regression/baseline_metrics.toml` (new)
- Record Criterion benchmark results as CI artifact
- Gate: fail if P99 regresses >10% from baseline
- Estimated: 6 hours

### 3.4 Version Criteria

v2.1.0 ships when all of the following are true:
- [ ] `export_actor!` returns valid serialized response
- [ ] A test actor compiles to `wasm32-wasip1` and executes in CI
- [ ] Rust server routes at least one request through the real engine
- [ ] 9 FDB integration tests run in CI
- [ ] Performance baseline recorded

---

## 4. Medium-Term Goals (v2.2.0 -- 1-3 Months)

### 4.1 CLI Server Integration

**L1: Wire remaining CLI commands to server**
- 13 commands currently local-only: deploy, scale, exec, status, rollback, run, etc.
- Each command makes HTTP/gRPC call to running `aether-server`
- Estimated: 20 hours

**L2: `aether run` single-command development**
- Start local engine, load WASM module from `aether.toml`, serve API
- Files: `crates/cli/src/commands/run.rs`
- Estimated: 8 hours

### 4.2 SDK Parity

**D1: Complete JavaScript SDK messaging**
- Files: `sdks/javascript/src/actor.ts`
- Implement `send()` and `call()` using gRPC transport
- Add E2E test: JS actor sends message to Rust actor via mesh
- Estimated: 12 hours

**D2: Complete Go SDK messaging**
- Files: `sdks/go/aether/actor.go`
- Implement `processItem("send")` routing logic
- Estimated: 8 hours

**D3: Go SDK OpenTelemetry tracing**
- Files: `sdks/go/aether/resilience/tracing.go`
- Replace stub with real OTel span creation
- Estimated: 4 hours

### 4.3 Python Server Deprecation

**P1: Feature parity matrix**
- Document: Python server features vs Rust server features
- Identify gaps in Rust server
- Files: `.docs/guides/migration_python_to_rust_server.md`
- Estimated: 4 hours

**P2: Deprecation**
- Mark `server/` as deprecated in README and CONTRIBUTING
- Stop adding features to Python server
- Remove from default Docker image
- Estimated: 2 hours

### 4.4 Performance Optimization

**O1: io_uring integration**
- `monoio` already in Cargo.toml as optional dependency
- Implement `io_uring` feature flag for state operations
- Benchmark against tokio baseline
- Target: 2x throughput for state-heavy workloads
- Estimated: 40 hours

**O2: Zero-copy message path**
- Use `rkyv` for zero-deserialization message passing between actors
- Current: serde_json on every message
- Target: <10us per actor-to-actor round-trip (same node)
- Estimated: 20 hours

**O3: WASM instance pooling**
- Pool pre-compiled WASM instances
- Current: ~2ms cold start per actor spawn
- Target: <100us warm spawn from pool
- Estimated: 16 hours

### 4.5 Multi-Tenancy Production

**T1: Per-tenant resource isolation**
- CPU quotas via WASM fuel metering
- Memory quotas via WASM memory limits
- Network quotas via token bucket in mesh layer
- Files: `crates/core/src/tenant/`
- Estimated: 20 hours

**T2: Tenant-scoped secrets**
- Isolated secret provider per tenant
- Cross-tenant secret access prohibition
- Audit log for all secret access
- Estimated: 12 hours

---

## 5. Long-Term Goals (v3.0.0 -- 3-6 Months)

### 5.1 Production Infrastructure

**I1: Blue-green deployment**
- Rolling update with zero-downtime actor migration
- Canary deployment with automatic rollback on error spike
- Files: `crates/server/src/deployment/` (new)
- Estimated: 40 hours

**I2: Full observability stack**
- OTLP export to VictoriaMetrics/VictoriaLogs (scaffolding exists)
- Distributed trace correlation across mesh nodes
- Custom Grafana dashboards for actor lifecycle, mesh topology
- Estimated: 30 hours

**I3: GitOps deployment**
- `aether deploy` reads actor topology from git repo
- ArgoCD/Flux integration via Helm chart
- Estimated: 20 hours

### 5.2 Actor Marketplace

**M1: OCI-compliant actor registry**
- Push/pull `.wasm` modules to OCI registry
- Versioning, dependency resolution, signature verification
- `aether push` / `aether pull` CLI commands
- Estimated: 40 hours

**M2: Actor composition**
- Declarative actor graph in `aether.toml`
- Topology, routing, scaling, and capability declarations
- Estimated: 20 hours

### 5.3 Ecosystem

**E1: Web dashboard**
- React/Vue frontend consuming REST API
- Actor topology graph, metrics charts, deployment management
- Estimated: 60 hours

**E2: TUI dashboard**
- Ratatui-based terminal UI (already in Cargo.toml)
- Real-time actor status, mesh topology, metrics
- `aether dashboard` command
- Estimated: 30 hours

**E3: Plugin system**
- Current: 12 LOC re-exports
- Target: dynamic plugin loading via WASM
- Plugins provide custom schedulers, state backends, auth providers
- Estimated: 40 hours

### 5.4 Formal Verification

**F1: Complete Lean4 proofs**
- Current: 5 theorems with `sorry` placeholders
- Target: 3 fully verified theorems (capability safety, audit chain, scheduler)
- Estimated: 80 hours

**F2: TLA+ model checking**
- Current: 2 specs with safety properties
- Target: run TLC model checker, add migration protocol spec
- Estimated: 30 hours

---

## 6. Technical Risk Register

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Actor SDK API design lock-in | High | High | Stability guarantees from v2.1.0; semver from day 1 |
| WASM FFI pointer semantics | Medium | High | Prototype in v2.1.0; rkyv for deterministic layout |
| Python/Rust server divergence | High | Medium | Migration guide; deprecate Python in v2.2.0 |
| io_uring kernel compatibility | Medium | High | Feature-gated; tokio fallback |
| Firecracker requires KVM | High | Medium | Mock-based testing; defer to bare-metal CI |
| FDB cluster setup complexity | Medium | High | SQLite for dev; InMemoryFdb for unit tests |
| Lean4 proof effort vs value | Medium | Low | Proof sketches only; full proofs for safety-critical paths only |
| SDK API drift across languages | Medium | Medium | Shared protobuf schema; CI builds all SDKs |

---

## 7. Dependency Graph (Build Order)

```
v2.1.0 Critical Path:

  A1 (export_actor! fix)
    -> A2 (serialization)
      -> A3 (messaging API)
        -> A5 (E2E WASM test in CI)

  S1 (server -> engine wiring)
    -> S2 (persistent state)
    -> S3 (auth middleware)

  Parallel:
  C1 (FDB in CI)
  C2 (performance baseline)

v2.2.0 Critical Path:

  L1 (CLI wiring) -> L2 (aether run)
  D1 (JS SDK send/call) [parallel with D2, D3]
  D2 (Go SDK messaging) [parallel with D1, D3]
  D3 (Go OTel tracing) [parallel with D1, D2]
  O1 (io_uring) [parallel with O2, O3]
  O2 (zero-copy) [parallel with O1, O3]
  O3 (instance pooling) [parallel with O1, O2]
  P1 (migration docs) -> P2 (deprecate Python)

v3.0.0 Critical Path:

  I1 (blue-green deploy) -> I3 (GitOps)
  I2 (observability) [parallel with I1]
  M1 (OCI registry) -> M2 (composition)
  E1 (web dashboard) [parallel with E2, E3]
  E2 (TUI dashboard) [parallel with E1, E3]
  E3 (plugin system) [parallel with E1, E2]
  F1 (Lean4 proofs) [parallel with F2]
  F2 (TLA+ model checking) [parallel with F1]
```

---

## 8. Resource Estimates

| Phase | Effort | Risk | Blockers |
|-------|--------|------|----------|
| v2.1.0 Actor SDK | 52 hours | Medium | WASM FFI pointer semantics |
| v2.1.0 Rust Server | 36 hours | Low | Engine API stability |
| v2.1.0 CI | 10 hours | Low | Docker image sizes |
| v2.2.0 CLI + SDK | 52 hours | Low | Server API completeness |
| v2.2.0 Performance | 76 hours | High | io_uring kernel version |
| v2.2.0 Multi-tenancy | 32 hours | Medium | Quota enforcement edge cases |
| v3.0.0 Infrastructure | 90 hours | High | Deployment complexity |
| v3.0.0 Marketplace | 60 hours | Medium | OCI registry design |
| v3.0.0 Ecosystem | 130 hours | Medium | Frontend framework choice |
| v3.0.0 Formal Verification | 110 hours | High | Proof complexity |

---

## 9. Quality Trajectory

| Metric | v2.0.0 (Current) | v2.1.0 Target | v2.2.0 Target | v3.0.0 Target |
|--------|------------------|----------------|----------------|----------------|
| Total tests | 1,257 | 1,300+ | 1,500+ | 2,000+ |
| Ignored tests | 86 | 77 (FDB enabled) | 41 (doc-tests fixed) | 15 (Firecracker only) |
| Clippy warnings | 0 | 0 | 0 | 0 |
| Stubs (production) | 5 | 3 | 1 | 0 |
| SDK messaging | Stub | JS/Go partial | Full parity | Full parity |
| Server persistence | None | SQLite | FDB | FDB + migration |
| Performance baseline | None | Recorded | Regressed | Regressed |
| Lean4 proofs | Sketches | 1 verified | 2 verified | 3 verified |
| Mutation score | N/A | N/A | >80% core/actor | >90% critical paths |

---

*Generated: 2026-05-11. Next review: 2026-05-25.*
