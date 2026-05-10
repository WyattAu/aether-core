# Aether-Core: Path Forward and Roadmap

**Date:** May 10, 2026
**Current Version:** 2.0.0
**Commit:** 0d55507
**Auditor:** Full monorepo audit -- 902 tests, 0 failures, 0 warnings

---

## 1. Current State Assessment

### 1.1 Repository Composition

| Component | LOC | Files | Status | Assessment |
|-----------|-----|-------|--------|------------|
| `crates/core/` | ~66,500 | ~95 .rs | Partial | Substantial WASI, QUIC mesh, secrets, MCP, Firecracker client |
| `crates/cli/` | ~5,477 | ~16 cmds | Partial | 3/16 commands actually call server (logs, top, observability) |
| `crates/actor-sdk/` | 311 | 4 modules | Scaffold | `export_actor!` returns null; no serialization, messaging, HTTP APIs |
| `crates/server/` | ~1,190 | 5 routes | Prototype | All state in-memory HashMap; no persistence, no WASM execution |
| `server/` (Python) | ~9,848 | 19 modules | Substantial | Most complete server: real clustering, Redis/PG backends, GraphQL |
| `sdks/go/` | ~18,679 | 6 packages | Substantial | Client-side; resilience/streaming fully implemented |
| `sdks/java/` | ~11,792 | 9 packages | Substantial | Client-side; good test coverage |
| `sdks/javascript/` | ~14,004 | 13 modules | Substantial | Largest SDK; unique event/workflow modules |
| `sdks/python/` | ~16,508 | 13 packages | Substantial | Most tested SDK; gRPC stubs included |
| Specs/Reports | ~80+ .md | Formal lifecycle | Complete | Yellow papers, blue papers, TLA+, Lean4 sketches |
| CI/CD | 12 workflows | Full pipeline | Complete | Build, test, lint, SDK, benchmarks, security, release, deploy |

### 1.2 Quality Metrics (Verified 2026-05-10)

| Metric | Value |
|--------|-------|
| Rust tests passing | 902 unit + 267 integration + 16 property + 17 fuzz + 20 security + 7 fixtures + 17 doctest |
| Test failures | 0 |
| Tests ignored (external deps) | 36 (FDB: 9, Firecracker: 7, Cluster: 15, E2E: 15) |
| Clippy warnings | 0 (workspace-level deny-all) |
| `unwrap()`/`expect()` in production | 0 (denied by lints) |
| `todo!`/`unimplemented!` stubs | 0 |
| Doc warnings | 0 |
| Format violations | 0 |
| Emoji characters in docs | 0 (purged across 80+ files) |
| Broken GitHub links | 0 (28 links fixed) |
| Version consistency | Canonical 2.0.0 across all docs |

### 1.3 Pre-commit / Pre-push Hooks

| Hook | Gates | Scope |
|------|-------|-------|
| `pre-commit` | fmt, clippy, compile, test, doc build, emoji scan | Rust changes: full gate; Doc changes: emoji gate only |
| `pre-push` | fmt, clippy, full test suite, doc build | Always runs on push |

### 1.4 Critical Gap Analysis

The project has a **strategic asymmetry**: the runtime engine (66K LOC) is substantial, but the components that make it usable end-to-end are incomplete:

1. **Actor SDK is a scaffold (311 LOC)** -- developers cannot write real actors. The `export_actor!` macro returns null. No serialization, no HTTP client, no messaging API, no capability declarations.
2. **Rust server is a prototype (1,190 LOC)** -- all state is in-memory HashMap. No persistence, no WASM execution, no real message delivery. The Python server (9,848 LOC) is the only production-viable server.
3. **CLI is partially connected (5,477 LOC)** -- only 3 of 16 commands actually talk to a running server.
4. **No end-to-end WASM execution in CI** -- despite having a full WASI implementation, no CI test compiles and runs a real WASM actor through the full pipeline.
5. **Two server implementations diverge** -- Python server has clustering, Redis/PG, GraphQL; Rust server has none of these. No clear migration path documented.

---

## 2. Immediate Priorities (v2.1.0 -- Next 2 Weeks)

### 2.1 Actor SDK Completion (CRITICAL)

The actor-sdk is the primary user-facing API. Without it, nobody can write actors.

**A1: Fix `export_actor!` macro**
- Current: returns `core::ptr::null()` (broken output path)
- Target: serialize response via rkyv/cbor, return valid pointer + length
- Files: `crates/actor-sdk/src/handler.rs`

**A2: Add serialization layer**
- Implement `aether-serde` using serde + postcard (no_std compatible, CBOR)
- Provide `#[derive(ActorMessage)]` proc macro for auto-serialization
- Target: `#[derive(ActorMessage)] struct Greet { name: String }` just works
- Files: new `crates/actor-sdk/src/serde.rs`, `crates/actor-macros/`

**A3: Add messaging API**
- `ctx.send(to: Address, msg: impl ActorMessage)` -- send message to another actor
- `ctx.request(to: Address, msg: impl ActorMessage) -> Response` -- request-response
- `ctx.emit(event: impl ActorMessage)` -- publish event
- `ctx.self_address() -> Address`
- Files: `crates/actor-sdk/src/messaging.rs`

**A4: Add capability declaration API**
- `#[aether_capability(network)]` attribute macro
- `#[aether_capability(state)]` attribute macro
- Compile-time capability verification
- Files: `crates/actor-macros/src/lib.rs`

**A5: End-to-end WASM test in CI**
- Compile a test actor to `wasm32-wasip1`
- Load it in `aether-core` engine
- Send a message, assert response
- This validates the entire pipeline: SDK -> WASM -> linker -> engine -> response
- Files: `tests/wasm_e2e_test.rs`, `examples/hello-actor/`

### 2.2 Rust Server Completion

The Rust server must replace the Python server for the project to be self-contained.

**S1: Wire server to aether-core engine**
- Replace `Arc<RwLock<HashMap>>` with real `ActorSystem` from `aether-core`
- Route handlers call `engine.spawn()`, `engine.send()`, `engine.request()`
- Files: `crates/server/src/routes/actors.rs`, `crates/server/src/state.rs`

**S2: Add persistent state backend**
- Implement `StateBackend` trait with FDB and SQLite backends
- SQLite for development/testing (no external dependency)
- FDB for production
- Files: new `crates/server/src/storage/`

**S3: Add WebSocket transport for real-time**
- WebSocket endpoint at `/ws` for bidirectional actor communication
- Client SDKs can connect via WebSocket instead of HTTP polling
- Files: `crates/server/src/routes/ws.rs`

**S4: Add authentication middleware**
- JWT-based authentication (reuse `security::identity` module)
- API key authentication (reuse `tenant::resolver`)
- Files: `crates/server/src/auth.rs`

### 2.3 CI Hardening

**C1: FDB Docker service in CI**
- Add `foundationdb/foundationdb:7.3` to `docker-compose.ci.yml`
- Enable 9 FDB integration tests in CI
- Estimated time: 4 hours

**C2: WASM end-to-end test in CI**
- Add `wasm32-wasip1` target to CI
- Build `examples/hello-actor`, run through engine
- Estimated time: 2 hours

**C3: Mutation testing**
- Add `cargo-mutants` to CI for `crates/core/src/actor/` module
- Target: >90% mutation score
- Estimated time: 8 hours (initial setup + triage)

---

## 3. Medium-Term Goals (v2.2.0 -- 1-3 Months)

### 3.1 CLI Server Integration

**L1: Wire remaining CLI commands to server**
- `deploy`: POST to `/api/v1/actors` with WASM module
- `status`: GET `/api/v1/cluster/status`
- `scale`: POST `/api/v1/actors/{id}/scale`
- `exec`: POST `/api/v1/actors/{id}/exec`
- Files: `crates/cli/src/commands/`

**L2: Add `aether run` command**
- Start local engine, load WASM module, serve API
- Single-command development experience
- Files: `crates/cli/src/commands/run.rs`

### 3.2 Python Server Migration

**P1: Document migration path from Python to Rust server**
- Feature parity matrix: Python server vs Rust server
- Migration guide for existing Python server users
- Files: `.docs/guides/migration_python_to_rust_server.md`

**P2: Deprecate Python server**
- Mark `server/` as deprecated in README
- Stop adding features to Python server
- Remove Python server from default Docker image

### 3.3 Performance Optimization

**O1: io_uring integration**
- `monoio` is already in Cargo.toml as optional dep
- Implement `io_uring` feature flag for state operations
- Benchmark against tokio baseline
- Target: 2x throughput for state-heavy workloads
- Files: `crates/core/src/state/`, `crates/core/src/engine/`

**O2: Zero-copy message path**
- Use `rkyv` for zero-deserialization message passing
- Current: serde_json serialization on every message
- Target: <10us per message round-trip (actor to actor, same node)
- Files: `crates/core/src/mesh/message.rs`, `crates/core/src/actor/`

**O3: WASM instance pooling**
- Pool pre-compiled WASM instances to avoid cold-start latency
- Current: ~2ms cold start per actor spawn
- Target: <100us warm spawn from pool
- Files: `crates/core/src/engine/instance.rs`

### 3.4 Multi-Tenancy Production

**T1: Per-tenant resource isolation**
- CPU quotas via cgroups (when running in Firecracker)
- Memory quotas via WASM fuel metering
- Network quotas via token bucket
- Files: `crates/core/src/tenant/`

**T2: Tenant-scoped secrets**
- Each tenant gets isolated secret provider
- No cross-tenant secret access
- Audit log for all secret access
- Files: `crates/core/src/security/secrets/`

---

## 4. Long-Term Goals (v3.0.0 -- 3-6 Months)

### 4.1 Production Infrastructure

**I1: Blue-green deployment**
- Rolling update with zero-downtime actor migration
- Canary deployment with automatic rollback on error spike
- Files: `crates/server/src/deployment/`

**I2: Observability stack**
- OTLP export to VictoriaMetrics/VictoriaLogs (already scaffolded)
- Distributed trace correlation across mesh nodes
- Custom dashboards for actor lifecycle, mesh topology, state operations
- Files: `crates/core/src/observability/`, `crates/core/src/tracing/`

**I3: GitOps deployment**
- `aether deploy` reads from git repo
- ArgoCD/Flux integration via Helm chart
- Files: `deploy/helm/`, `.github/workflows/gitops.yml`

### 4.2 Actor Marketplace

**M1: OCI-compliant actor registry**
- Push/pull `.wasm` modules to OCI registry
- Versioning, dependency resolution, signature verification
- `aether push` and `aether pull` CLI commands
- Files: new `crates/registry/`

**M2: Actor composition**
- Declarative actor graph in YAML/TOML
- `aether.toml` specifies actor topology, routing, scaling
- Files: `crates/cli/src/config/`

### 4.3 Ecosystem

**E1: Web dashboard**
- React/Vue frontend consuming REST API
- Actor topology graph, metrics charts, deployment management
- Files: new `web/` directory

**E2: TUI dashboard**
- Ratatui-based terminal dashboard (ratatui in Cargo.toml)
- Real-time actor status, mesh topology, metrics
- `aether dashboard` command
- Files: `crates/cli/src/commands/dashboard.rs`

**E3: Plugin system**
- Current: `plugin/mod.rs` is 12 LOC (re-exports only)
- Target: dynamic plugin loading via WASM
- Plugins can provide custom schedulers, state backends, auth providers
- Files: `crates/core/src/plugin/`

### 4.4 Formal Verification

**F1: Complete Lean4 proofs**
- Current: 5 theorems with `sorry` placeholders
- Target: at least 3 fully verified theorems
- Priority: capability safety, audit chain integrity, scheduler safety
- Files: `.specs/02_architecture/proofs/`

**F2: TLA+ model checking**
- Current: 2 specs with safety properties (scheduler, gossip)
- Target: run TLC model checker on all specs
- Add actor migration protocol spec
- Files: `.specs/02_architecture/proofs/`

---

## 5. Technical Risk Register

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Actor SDK design lock-in | High | High | Design API with stability guarantees; version from day 1 |
| WASM execution performance | Medium | High | Instance pooling; benchmark early; rkyv zero-copy |
| Python/Rust server divergence | High | Medium | Document migration path; deprecate Python server in v2.2 |
| io_uring kernel compatibility | Medium | High | Feature-gate behind `io_uring` feature; fallback to tokio |
| Firecracker requires KVM (no CI) | High | Medium | Mock-based testing; defer to bare-metal CI |
| FDB cluster setup complexity | Medium | High | SQLite for dev; InMemoryFdb for unit tests; Docker CI for integration |
| Lean4 proof investment vs value | Medium | Low | Proof sketches only; full proofs only for safety-critical paths |
| SDK API drift across languages | Medium | Medium | Shared protobuf schema; CI builds all SDKs; API compatibility tests |
| Two server implementations confuse users | High | High | Deprecate Python server clearly; migration guide |

---

## 6. Version Success Criteria

### v2.1.0 (Next 2 Weeks)

- [ ] Actor SDK: `export_actor!` returns valid response (not null)
- [ ] Actor SDK: serialization layer (postcard/CBOR)
- [ ] Actor SDK: messaging API (send, request, emit)
- [ ] Actor SDK: capability declaration macros
- [ ] End-to-end WASM test in CI (compile -> load -> message -> response)
- [ ] Rust server: wired to aether-core engine
- [ ] Rust server: persistent state backend (SQLite)
- [ ] CLI: `aether run` command for local development
- [ ] CI: FDB Docker service enabling 9 integration tests
- [ ] CI: mutation testing for core/actor module

### v2.2.0 (1-3 Months)

- [ ] CLI: all 16 commands connected to server
- [ ] Python server: deprecated, migration guide published
- [ ] Rust server: WebSocket transport, JWT auth
- [ ] Performance: io_uring feature flag, benchmarked
- [ ] Performance: zero-copy message path (rkyv)
- [ ] Performance: WASM instance pooling (<100us warm spawn)
- [ ] Multi-tenancy: per-tenant resource isolation
- [ ] Multi-tenancy: tenant-scoped secrets
- [ ] Doc-tests: ignored count reduced from 41 to <20

### v3.0.0 (3-6 Months)

- [ ] Blue-green deployment with zero-downtime migration
- [ ] Observability: OTLP export, distributed traces, dashboards
- [ ] Actor marketplace: OCI registry, push/pull
- [ ] Actor composition: declarative topology in aether.toml
- [ ] Web dashboard
- [ ] TUI dashboard
- [ ] Plugin system: dynamic WASM plugin loading
- [ ] Lean4: 3 fully verified theorems
- [ ] TLA+: TLC model checking on all specs
- [ ] External security audit completed
- [ ] No-allocation hot path verified (<100 allocs per message)

---

## 7. Dependency Graph (Build Order)

```
v2.1.0 Critical Path:
  A1 (export_actor! fix)
    -> A2 (serialization)
      -> A3 (messaging API)
        -> A5 (E2E WASM test)
  S1 (server -> engine wiring)
    -> S2 (persistent state)
    -> S3 (WebSocket)
    -> S4 (auth)

v2.2.0 Critical Path:
  L1 (CLI wiring) -> L2 (aether run)
  O1 (io_uring) [parallel with O2, O3]
  O2 (zero-copy) [parallel with O1, O3]
  O3 (instance pooling) [parallel with O1, O2]
  P1 (migration docs) -> P2 (deprecate Python)
```

---

## 8. Resource Estimates

| Phase | Effort | Risk | Blockers |
|-------|--------|------|----------|
| v2.1.0 Actor SDK | 40-60 hours | Medium | WASM FFI pointer semantics |
| v2.1.0 Rust Server | 20-30 hours | Low | Engine API stability |
| v2.1.0 CI | 10-15 hours | Low | Docker image sizes |
| v2.2.0 Performance | 60-80 hours | High | io_uring kernel version |
| v2.2.0 Multi-tenancy | 30-40 hours | Medium | Quota enforcement edge cases |
| v3.0.0 Infrastructure | 80-120 hours | High | Deployment complexity |
| v3.0.0 Marketplace | 40-60 hours | Medium | OCI registry design |
| v3.0.0 Formal Verification | 60-100 hours | High | Lean4 proof complexity |

---

*Generated: 2026-05-10. Next review: 2026-05-24.*
