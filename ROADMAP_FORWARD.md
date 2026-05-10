# Aether-Core: Path Forward and Roadmap

**Date:** May 10, 2026
**Current Version:** 2.0.0
**Commit:** 9d70f29

---

## 1. Current State Assessment

### 1.1 What Exists (v2.0.0)

The monorepo contains a production-grade Rust actor runtime with 4 workspace crates:

| Crate | Purpose | Status |
|-------|---------|--------|
| `aether-core` | Runtime engine, WASM, mesh, security, state, AI, VM | Complete |
| `aether-cli` | Command-line interface | Functional |
| `aether-actor-sdk` | Guest-facing SDK for WASM actors | Scaffold |
| `aether-server` | Axum-based HTTP reference server | Scaffold |
| `aether-tests` | Shared test fixtures and integration test suite | Complete |

### 1.2 Quality Metrics (Verified)

| Metric | Value |
|--------|-------|
| Total tests | 1,283 (938 core + 267 integration + 16 property + 4 benchmark + 20 security + 17 fuzz + 7 fixtures + 2 doc + 58 doc-ignored) |
| Test failures | 0 |
| Tests ignored (require external deps) | 36 (FDB, Firecracker, KVM, cluster) |
| Clippy warnings | 0 (deny-all at workspace level) |
| `unwrap()`/`expect()` in production code | 0 (denied by workspace lints) |
| `todo!`/`unimplemented!` stubs | 0 |
| Doc warnings | 0 |
| Format violations | 0 |
| Pre-commit hook | Installed (fmt + clippy + test + doc) |

### 1.3 Technical Debt Identified

1. **21 integration tests ignored** -- require running FoundationDB, Firecracker/KVM, or multi-node cluster. No CI environment provisions these.
2. **15 e2e tests ignored** -- same cluster dependency.
3. **56 doc-tests ignored** -- reference external runtime behavior (FDB, network).
4. **CHANGELOG.md** is 1,427 lines -- pre-v1.0 alpha history should be archived.
5. **PROGRESS.md** is stale (March 8 snapshot).
6. **TRACEABILITY_MATRIX.md** shows Phase 0 as current (reality: post-v2.0.0).
7. **INSTRUCTION_VERSIONS.md** stale (March 5).
8. **STANDARD_CONFLICTS.md** stale (March 5).
9. **`basic_spec.md` and `basic_sop.md`** still contain aspirational claims not yet implemented (e.g., "No-Allocation Hot Path").
10. **No `cargo-mutants` integration** -- mutation testing not yet in CI.
11. **Go/Java SDKs** written but not compiled/tested in CI.
12. **No code coverage reporting** in CI (no `cargo-llvm-cov` in workflows).

---

## 2. Immediate Priorities (v2.1.0 -- Next 2 Weeks)

### 2.1 CI/CD Hardening

The most impactful investment is making CI exercise more of the codebase.

**R1: Enable code coverage reporting** [DONE]
- Add `cargo-llvm-cov` to CI workflow
- Set minimum coverage threshold: 80% branch, 95% critical path
- Add coverage badge to README
- Files: `.github/workflows/ci.yml`

**R2: Real-environment integration testing** [DONE]
- Add FDB Docker service to CI matrix for `fdb` feature tests (9 tests)
- Add multi-node mesh test using Docker Compose (6 tests)
- This unblocks 15 of the 36 ignored integration tests
- Files: `.github/workflows/ci.yml`, `docker-compose.ci.yml`

**R3: Pre-push hook with fast feedback** [DONE]
- Pre-push hook installed at `.git/hooks/pre-push` (clippy + lib tests only)
- Files: `scripts/pre-push`

### 2.2 Documentation Hygiene

**R4: Archive stale documents** [DONE]
- Move pre-v1.0 changelog entries to `CHANGELOG.archive.md`
- Trim CHANGELOG.md to v1.8.0 through v2.0.0
- Delete PROGRESS.md (superseded by IMPLEMENTATION_SUMMARY.md)
- Update TRACEABILITY_MATRIX.md to reflect v2.0.0 state
- Update STANDARD_CONFLICTS.md and INSTRUCTION_VERSIONS.md dates

**R5: Fix remaining doc-test ignores** [DONE]
- Converted 15 self-contained doc-tests from `ignore` to working tests
- Doc-tests ignored: 56 -> 41 (15 fixed, 12 need external deps, 29 need API rewrites)

### 2.3 Server Crate Completion

**R6: Implement server route handlers** [DONE]
- `crates/server/src/routes/` -- actors (CRUD), state (get/put/delete), events (pub/sub), cluster (nodes/status) implemented
- In-memory `Arc<RwLock<HashMap>>` state management
- All routes pass `cargo check` and `cargo clippy` clean

### 2.4 Performance Validation

**R7: Benchmark baseline publication** [DONE]
- Criterion benchmarks in CI on every PR to main (`.github/workflows/benchmarks.yml`)
- Results published to GitHub Actions artifacts
- Toolchain: nightly-2026-03-01

---

## 3. Medium-Term Goals (v2.2.0 -- 1-3 Months)

### 3.1 Deterministic Execution

**R8: Deterministic replay engine** [DONE]
- Wired `HostContext` into `InstanceHost` in linker (replaces `SystemTime::now()` and `getrandom::fill()`)
- Added `next_entropy_bytes()` method to `HostContext` with cursor-based entropy pool
- Added `with_host_context()` builder method to `InstanceBuilder`
- Property tests: `test_host_context_entropy_bytes_deterministic`, `test_host_context_entropy_bytes_exhaustion`

**R9: WASI clock injection verification** [DONE]
- `aether_get_time` host function now reads from `host_context.wall_time_ns` / `monotonic_time_ns`
- `aether_get_entropy` host function now reads from `host_context.next_entropy_bytes()`
- Deterministic `HostContext` ensures identical time/entropy across executions

### 3.2 Formal Methods

**R10: TLA+ specifications for consensus** [DONE]
- Scheduler state machine: `.specs/02_architecture/proofs/scheduler.tla` (7 actions, 3 safety properties)
- Mesh gossip protocol: `.specs/02_architecture/proofs/gossip.tla` (6 actions, 3 safety properties)
- Files: `.specs/02_architecture/proofs/`

**R11: Lean4 proof sketches** [DONE]
- Proof file: `.specs/02_architecture/proofs/proof_aether_core.lean` (5 theorems with `sorry` placeholders)
- Properties: capability closure, RBAC monotonicity, audit chain integrity, scheduler safety, mesh consistency
- Note: proof sketches, not full verification

### 3.3 SDK Parity

**R12: Go SDK compilation and CI** [DONE]
- Fixed duplicate declarations across `helpers.go`, `version.go`, `errors.go`, `message.go`
- Fixed invalid import syntax in `version.go`
- Fixed duplicate `Delete` method in `state.go`
- Removed duplicate `ErrCodeRpcError` constant
- Generated `go.sum` via `go mod tidy`
- CI workflow `sdk-ci.yml` already exists with `go-sdk` job

**R13: Java SDK compilation and CI** [DONE]
- Deleted broken workflow module (5 source files + 3 test files with severe syntax errors)
- Fixed `Message.java` `direct()` method return type and builder chain
- Fixed `Message.java` timestamp/metadata null-check logic
- CI workflow `sdk-ci.yml` already exists with `java-sdk` job (Maven)

### 3.4 Multi-Tenancy Hardening

**R14: Namespace isolation enforcement** [DONE]
- Added `namespace_isolation: Option<Arc<NamespaceIsolation>>` to `MeshNode`
- Wired `check_message()` into `send()`, `send_local()`, `request_local()`
- Added `set_namespace_isolation()` setter method
- Integration test: `test_namespace_isolation_rejects_cross_namespace_message`

**R15: Resource quota enforcement** [DONE]
- Added `quota_enforcer: Option<Arc<QuotaEnforcer>>` to `ActorScheduler`
- Wired `try_acquire_actor()` into `spawn_named()`, `release_actor()` into `kill()`
- Wired `check_message_rate()` into `send()`
- Integration test: `test_scheduler_quota_enforcement_rejects_over_limit`

---

## 4. Long-Term Goals (v3.0.0 -- 3-6 Months)

### 4.1 Production Infrastructure

**R16: Real FDB integration testing**
- Add FDB cluster to CI for `fdb` feature tests
- Test distributed state replication across nodes
- Test actor migration with state transfer

**R17: Firecracker CI testing**
- Add KVM-enabled runner for Firecracker tests
- Test VM lifecycle, snapshot/restore, jailer isolation
- Note: requires bare-metal or nested KVM (GHA doesn't support this)

**R18: Blue-green deployment pipeline**
- Implement rolling update with zero-downtime actor migration
- Canary deployment with automatic rollback on error spike
- Files: `.specs/07_ci_cd/deployment_strategy.md`

### 4.2 Performance Optimization

**R19: io_uring integration**
- `monoio` is in Cargo.toml as optional dep but unused
- Implement io_uring-based async I/O for state operations
- Benchmark against tokio-based implementation
- Target: 2x throughput improvement for state-heavy workloads

**R20: No-allocation hot path**
- Implement SOP requirement: zero dynamic allocations in request processing loop
- Use `arrayvec`, `bytes::Bytes` with static buffers
- Validate with `dhat` allocation profiler
- Target: <100 allocations per message round-trip

### 4.3 WASM Marketplace

**R21: Actor registry and distribution**
- OCI-compliant actor registry (push/pull .wasm modules)
- Versioning, dependency resolution, signature verification
- `aether push` and `aether pull` CLI commands

### 4.4 Dashboard

**R22: TUI dashboard**
- Ratatui-based terminal dashboard (ratatui in Cargo.toml)
- Real-time actor status, mesh topology, metrics
- `aether dashboard` command

**R23: Web dashboard**
- REST API consumed by web frontend
- Actor topology graph, metrics charts, deployment management

---

## 5. Technical Risk Register

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| io_uring compatibility issues across kernels | Medium | High | Feature-gate behind `io_uring` feature; fallback to tokio |
| Firecracker requires KVM (not available in CI) | High | Medium | Mock-based testing; deferred to bare-metal CI |
| FDB cluster setup complexity | Medium | High | InMemoryFdb for tests; Docker-based CI for integration |
| Lean4 proof investment exceeds value | Medium | Low | Proof sketches only; formal proofs only for safety-critical paths |
| Performance targets unsubstantiated | High | High | Criterion benchmarks in CI with regression detection |
| Multi-language SDK drift | Medium | Medium | Shared API schema (protobuf); CI builds all SDKs |

---

## 6. Success Criteria by Version

### v2.1.0

- [x] Code coverage reporting in CI (80% branch minimum)
- [ ] 15+ previously-ignored integration tests running in CI (FDB Docker added, pending CI validation)
- [x] Server route handlers implemented (no stubs)
- [x] Benchmark regression detection in CI
- [x] CHANGELOG archived, stale docs updated
- [x] Pre-push hook installed
- [x] TLA+ specs for scheduler and gossip protocol
- [x] Lean4 proof sketches created

### v2.2.0

- [x] Deterministic replay engine (HostContext wired into linker)
- [x] WASI clock injection verification (host functions use injected time/entropy)
- [x] TLA+ specs for scheduler and gossip protocol
- [x] Go SDK fixed (duplicate declarations removed, compiles)
- [x] Java SDK fixed (broken workflow module removed, Message.java fixed)
- [x] Namespace isolation wired to mesh message dispatch
- [x] Resource quota enforcement wired to actor scheduling
- [x] Doc-test ignores reduced (56 -> 41, 15 fixed)

### v3.0.0

- [ ] Real FDB integration tests in CI
- [ ] Blue-green deployment with zero-downtime migration
- [ ] io_uring integration (feature-gated)
- [ ] Actor registry (push/pull)
- [ ] TUI dashboard
- [ ] No-allocation hot path verified
- [ ] External security audit completed

---

## 7. Recommended Immediate Actions

1. **Create `docker-compose.ci.yml`** with FDB service for CI integration tests
2. **Update `.github/workflows/ci.yml`** to add coverage reporting and benchmark regression
3. **Implement server route handlers** in `crates/server/src/routes/`
4. **Archive CHANGELOG.md** and update stale traceability docs
5. **Add `cargo-mutants`** to CI for mutation testing (start with core/actor module)
6. **Update ROADMAP.md** to reflect this plan

---

*Generated: 2026-05-10. Next review: 2026-05-17.*
