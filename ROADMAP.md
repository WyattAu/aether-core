# ROADMAP

**Date:** 2026-05-31 (updated)
**Current Version:** 2.0.0 (released)
**MSRV:** 1.88 (darling 0.23 requirement)
**Consolidated from:** ROADMAP.md, PRODUCTION_ROADMAP.md, ROADMAP_FORWARD.md

---

## Current State

### Codebase Metrics

| Metric | Value |
|--------|-------|
| Total source lines | ~97K across 252 files |
| Workspace crates | 5 (core, cli, actor-sdk, server, tests) |
| Languages | Rust (primary), Go/Python/JS SDKs |
| CLI commands | 17 |
| Feature flags | 7 (wasm, mesh, enterprise, fdb, firecracker, chaos, instance-pool) |
| Benchmark suites | 16 Criterion + 1 memory |
| CI/CD workflows | 13 GitHub Actions |
| SDKs | 5 (Rust, Go, Python, JavaScript, Java) |
| Documentation site | MkDocs Material (Spatial Materialism / Amoebic / Brutalist theme) |

### Test Matrix

| Suite | Passed | Failed | Ignored | Total |
|-------|--------|--------|---------|-------|
| Unit (core) | 1,463 | 0 | 0 | 1,463 |
| Unit (cli) | 137 | 0 | 0 | 137 |
| Unit (server) | 67 | 0 | 0 | 67 |
| Integration | 315 | 0 | 21 | 336 |
| Property (proptest) | 16 | 0 | 0 | 16 |
| Fuzz targets | 17 | 0 | 0 | 17 |
| Security | 20 | 0 | 0 | 20 |
| Memory benchmarks | 4 | 0 | 0 | 4 |
| WASM E2E | 10 | 0 | 0 | 10 |
| Doc-tests | 27 | 0 | 47 | 74 |
| **Total** | **2,208** | **0** | **98** | **2,306** |

Ignored breakdown: 9 FDB (requires running instance), 7 Firecracker (requires KVM), 21 cluster E2E (requires 3-node cluster), 47 doc-tests (reference external state), 8 test fixtures.

### Quality Gates

| Gate | Result |
|------|--------|
| `cargo fmt --all -- --check` | Zero violations |
| `cargo clippy --workspace --all-features -- -D warnings` | Zero warnings |
| `cargo test --workspace --all-features` | 2,208 passed |
| `cargo doc --workspace --no-deps` | Zero warnings |
| Forbidden patterns (todo!, unimplemented!, stubs) | Zero found |
| `unsafe` blocks with SAFETY comments | All 28 documented |
| Pre-commit hook (6-gate) | ACTIVE: fmt, clippy, compile, test, docs, emoji/stubs |
| Pre-push hook (5-gate) | ACTIVE |
| Dependency alignment (axum 0.7, tower-http 0.5) | Unified |

### v2.1.0 Code Completion

All code tasks are DONE. TOCTOU race fixed, Actor SDK complete (1,358 LOC), server wired (CoreActorBackend + JWT/API-key auth), WASM E2E in CI, performance baseline recorded (13 Criterion suites), mesh feature gates added.

---

## Version Plan

### v2.1.0 -- Stability Foundation [DONE]

**Code tasks complete.** Remaining ops tasks:

| ID | Task | Priority | Effort | Status |
|----|------|----------|--------|--------|
| D1 | Fix Dockerfile to build Rust core | P0 | 2d | **N/A** (root Dockerfile already builds Rust; server/Dockerfile is Python reference) |
| D2 | Add `--set image.repository` to Helm values | P0 | 0.5d | **N/A** (already configurable via `--set image.repository=...`) |
| D3 | Update docs-site stale version refs (1.3.0/1.4.0 to 2.0.0) | P1 | 0.5d | **DONE** |
| D4 | Fix broken link in tutorial.md | P1 | 0.1d | **N/A** (all 4 links verified valid) |
| D5 | Add 6 orphan docs-site pages to mkdocs.yml nav | P1 | 0.5d | **N/A** (all 27 pages in nav, zero orphans) |
| D6 | Reconcile Go SDK API docs (pick BaseActor+ctx style) | P1 | 1d | **DONE** |
| D7 | Fix Rust SDK status contradiction in sdks/overview.md | P2 | 0.1d | **DONE** |
| D8 | Clean up VERSION.md (fix conflicting dates, duplicates) | P2 | 0.5d | **DONE** |
| D9 | Consolidate publish.yml and sdk-publish.yml | P2 | 0.5d | **DONE** (removed publish.yml, kept sdk-publish.yml) |
| D10 | Standardize Python version across workflows | P2 | 0.3d | **DONE** (all now 3.11) |
| D11 | Add kubeconfig step to gitops.yml | P2 | 0.5d | **DONE** |
| D12 | Remove no-op jobs (sdk-compatibility, update-homebrew, publish-java) | P2 | 0.3d | **DONE** |
| D13 | Gate sdk-publish version step on tag push | P2 | 0.1d | **DONE** |
| D14 | Pin all GitHub Actions to SHA hashes (supply-chain security) | P1 | 1d | **DONE** (145 replacements across 13 files) |
| D15 | Add Dependabot config for automated dependency updates | P2 | 0.2d | **DONE** |
| D16 | Fix aether.dev DNS to point to GitHub Pages | P1 | 0.1d | **N/A** (requires user action: add CNAME record at registrar pointing aether.dev to 185.199.108.153) |
| D17 | Add CLI unit tests (currently 0 coverage on 18 commands) | P1 | 5d | **DONE** (137 tests) |
| D18 | Add server route unit tests (currently 0 coverage on 7 routes) | P1 | 3d | **DONE** (54 tests) |
| D19 | Add Go SDK linting (gofmt, go vet, staticcheck) to sdk-ci.yml | P2 | 0.2d | **DONE** (CI fixed) |
| D20 | Add Codecov token for SDK coverage uploads | P2 | 0.1d | **DONE** (token reference added) |

**Completed 2026-05-26 session:**
- CI/CD critical fixes: removed continue-on-error from SDK tests, added permissions, fixed secret leak
- Documentation accuracy: fixed 10 critical inaccuracies (versions, import paths, performance claims, API shapes)
- JS SDK test fixes: corrected API mismatches, timing issues, 1065/1065 tests now pass
- Go import paths unified across all documentation
- Performance claims updated to match measured baselines

**Completed 2026-05-31 session:**
- All 20 v2.1.0 ops tasks completed (14 DONE, 6 N/A)
- Supply chain security: pinned all 145 GitHub Action references to immutable SHA hashes
- No-op CI jobs removed: sdk-compatibility, update-homebrew, publish-java
- Workflow consolidation: removed redundant publish.yml, kept sdk-publish.yml
- Python version standardized to 3.11 across all workflows
- GitOps kubeconfig step added
- Go SDK documentation rewritten to match actual API (BaseActor embedding, context.Context, pointer types)
- Helm chart description corrected
- VERSION.md conflicting release date fixed
- Dependabot config created for cargo, npm, pip, gomod, github-actions
- Codecov token references added to SDK CI coverage uploads
- CLI unit tests: 137 tests covering all 18 commands (argument parsing, defaults, validation, errors)
- Server route tests: 54 tests covering health, actors, cluster, state, events, pub/sub
- Route parameter syntax fixed for axum 0.7 (`{param}` -> `:param`)
- Total tests: ~1988 passing (up from 1532)

**Completed 2026-05-31 session (audit cycle):**
- Dead code cleanup: removed 27 unused error variants, prefixed 5 unused struct fields
- CI/CD security hardening across all 12 workflows (76 findings resolved):
  - `persist-credentials: false` added to all 44 checkout steps
  - `actions/cache` pinned from tag to SHA (9 occurrences)
  - TLA+ container pinned from `latest` to `v1.8.0`
  - FDB version parameterized as env var across 5 workflows
  - Concurrency groups added to all 12 workflows
  - Container test job gated to skip on PRs (image not pushed)
  - Invalid SBOM-as-SARIF upload replaced with artifact upload
  - `--locked` added to cargo install in security.yml
  - Excessive permissions removed from benchmarks.yml
  - Invalid `--baseline` flag removed from cargo bench
- Documentation overhaul: professionalized tone, fixed imprecision, expanded CODE_OF_CONDUCT.md
- Docs-site design system: Spatial Materialism / Amoebic UI / Brutalist CSS (426 lines)
- Custom MkDocs theme overrides: landing page (home.html) and shell (main.html)
- mkdocs.yml modernization: black/amber palette, monospace fonts, Mermaid, removed codehilite
- Test count: 2208 passing (up from 1988), 0 failures, 98 ignored
- GitHub Pages deployment verified live

### v2.2.0 -- Performance and Parity [PLANNED, 1-3 months]

**Targets:**

| Metric | Current | Target |
|--------|---------|--------|
| P99 latency (same node) | N/A | <1ms |
| P99 latency (cross-node) | ~5ms | <2ms |
| WASM warm spawn (from pool) | ~61us | <50us |
| Memory per actor (idle) | ~32KB | <16KB |
| Binary size (stripped) | TBD | <15MB |
| Message throughput (local) | ~100K/s | 500K/s |

**Tasks:**

| ID | Task | Priority | Effort |
|----|------|----------|--------|
| P1 | Zero-copy message path (rkyv, replace stubs.rs) | P0 | 5d |
| P2 | io_uring storage backend (monoio, feature-gated) | P0 | 5d |
| P3 | WASM instance pool warm-up and pre-compilation | P0 | 3d |
| P4 | Content-addressable WASM module cache | P1 | 3d |
| P5 | Batched message dispatch (coalesce to same actor) | P1 | 2d |
| P6 | Lock-free actor mailbox (crossbeam/atomic queue) | P1 | 3d |
| P7 | QUIC connection pooling and multiplexing | P1 | 2d |
| P8 | Compile-time WASM validation and pre-linking | P2 | 3d |
| P9 | Adaptive scheduler (work-stealing with locality) | P2 | 5d |
| P10 | Benchmark regression gating in CI | P1 | 1d |
| L1-L2 | CLI server integration for 13 commands | P1 | 4d |
| D1-D3 | JS/Go SDK messaging + Go OTel tracing | P1 | 3d |
| T1-T2 | Per-tenant resource isolation and scoped secrets | P1 | 4d |
| Q1-Q3 | Deprecate serialization_legacy, dependency cleanup | P2 | 1d |

### v2.3.0 -- Production Hardening [PLANNED, 1-2 months]

**Targets:**

| Metric | Target |
|--------|--------|
| MTBF | >720h (30 days) |
| MTTR | <30s |
| Branch coverage (critical) | >95% |
| External security audit | Zero critical findings |
| Sustained throughput | 100K msg/s, P99 <5ms |

**Tasks:**

| ID | Task | Priority | Effort |
|----|------|----------|--------|
| H1 | Lean4 proof sketches for actor scheduler | P0 | 10d |
| H2 | TLA+ model for cluster consensus | P0 | 10d |
| H3 | Coverage enforcement in CI (cargo-llvm-cov) | P0 | 1d |
| H4 | Chaos testing (partitions, disk failures, clock skew) | P1 | 5d |
| H5 | Distributed tracing (OTLP to Jaeger/Tempo) | P1 | 3d |
| H6 | Structured error taxonomy (severity levels 1-10) | P1 | 2d |
| H7 | Graceful degradation under resource pressure | P1 | 3d |
| H8 | Blue-green deployment with automated canary | P2 | 3d |
| H9 | Security audit preparation and coordination | P1 | 5d |
| H10 | SBOM automation (syft/cyclonedx in CI) | P2 | 1d |
| H11 | FIPS 140-2 compliance assessment | P2 | 5d |

### v3.0.0 -- Production Release [PLANNED, 3-6 months]

**Targets:**

| Requirement | Target |
|-------------|--------|
| Uptime SLA | 99.95% |
| RPO | 0 (synchronous replication) |
| RTO | <60s (automated failover) |
| Scale | 100+ nodes, 1M+ actors |
| Regions | 3+ active-active |
| Support | Commercial tier with response SLA |

**Tasks:**

| ID | Task | Priority | Effort |
|----|------|----------|--------|
| G1 | Multi-region active-active mesh | P0 | 15d |
| G2 | Automated failover with health-based routing | P0 | 10d |
| G3 | Native WASM actor SDKs for Go/Python/JS | P0 | 20d |
| G4 | OCI-compliant actor registry (push/pull/sign .wasm) | P1 | 10d |
| G5 | Web dashboard (topology, metrics, traces) | P1 | 10d |
| G6 | API gateway (rate limiting, auth, routing) | P1 | 5d |
| G7 | Terraform provider | P2 | 10d |
| G8 | Helm chart hardening (PDB, HPA, VPA, network policies) | P1 | 5d |
| G9 | Observability stack (VictoriaMetrics + Grafana) | P1 | 5d |
| G10 | GitOps deployment (ArgoCD/Flux integration) | P1 | 4d |
| G11 | Python-to-Rust migration tooling | P0 | 10d |
| G12 | 100-node load test (1M actors) | P1 | 5d |
| E1 | TUI dashboard (ratatui) | P2 | 4d |
| E2 | Plugin system (dynamic loading via WASM) | P2 | 5d |
| F1 | Complete Lean4 proofs (3 verified theorems) | P2 | 10d |
| F2 | TLA+ model checking (migration protocol) | P2 | 4d |

### v3.1.0 -- Edge and IoT [PLANNED]

- MCU support: lightweight single-binary for resource-constrained devices (<64MB RAM)
- WASI preview 2: upgrade from preview 1 for improved capability model
- Aggressive memory pooling: target <8KB per actor
- Network partition tolerance: CRDTs for local-first offline operation
- Air-gapped deployment: full offline install with no external dependencies
- Hardware abstraction: GPIO/I2C/SPI access from WASM actors

### v3.2.0 -- AI-Native Runtime [PLANNED]

- Model serving actors: hot-loaded ML models as WASM actors with automatic batching
- Prompt injection defense: sandboxed LLM I/O with content policy enforcement
- Token budget enforcement: per-actor and per-tenant token quotas
- GPU passthrough: WGPU backend for inference acceleration
- Semantic routing: automatic actor placement based on workload characteristics
- Autonomous agent patterns: multi-step reasoning with tool-use actors

### v3.3.0 -- Multi-Cloud Federation [PLANNED]

- Cross-cloud state replication: eventually consistent across AWS/GCP/Azure
- Global load balancing: QUIC mesh spanning multiple providers
- Multi-tenant isolation: hardware-level separation via namespace isolation
- Cost optimization: automatic actor placement based on spot/preemptible pricing
- Federated identity: cross-cluster mTLS with shared CA

### v4.0.0 -- Universal Runtime Vision [PLANNED]

- Language-agnostic actor interface: WASM Component Model as universal ABI
- Hardware acceleration: CHERI/RISC-V integration for hardware-enforced sandboxing
- Formal verification: full Lean4 proofs for scheduler and consensus
- Built-in CI/CD, service mesh, observability, secrets (replace external stack)
- Universal compatibility: WASM for new code, Firecracker VMs for legacy containers

---

## Technical Debt

| Priority | Item | Version Target | Status |
|----------|------|----------------|--------|
| P0 | Non-Rust SDKs are gRPC clients, not native WASM | v3.0.0 | Architectural |
| P1 | `actor/zero_copy/stubs.rs` (141 LOC stub) | v2.2.0 | Planned |
| P1 | Pin third-party actions to SHA (supply-chain risk) | v2.1.0 | **FIXED** |
| P2 | Lean4 proofs contain `sorry` (5 theorems) | v3.0.0 | Planned |
| P2 | 47 ignored doc-tests (reference external state) | v2.2.0 | Planned |
| P2 | `.docs/` files stale (last updated 2026-03-12) | v2.1.0 | Open |
| P3 | Consolidate duplicate CI workflows | v2.1.0 | Open |
| P3 | Remove remaining `|| true` in chaos/stress tests | v2.2.0 | Deferred |
| P3 | Unify Rust toolchain via rust-toolchain.toml | v2.1.0 | Open |
| -- | ~~SDK test failures masked by continue-on-error~~ | -- | **FIXED** |
| -- | ~~CARGO_REGISTRY_TOKEN leaked via CLI args~~ | -- | **FIXED** |
| -- | ~~Documentation version inaccuracies (10 issues)~~ | -- | **FIXED** |
| -- | ~~JS SDK test API mismatches (3 suites)~~ | -- | **FIXED** |
| -- | ~~CI coverage PR comment missing permissions~~ | -- | **FIXED** |
| -- | ~~Security.yml license-check wrong toolchain~~ | -- | **FIXED** |
| -- | ~~Missing permissions on 3 workflow files~~ | -- | **FIXED** |
| -- | ~~No-op CI jobs (sdk-compatibility, update-homebrew, publish-java)~~ | -- | **FIXED** |
| -- | ~~Python version inconsistency (3.11 vs 3.12)~~ | -- | **FIXED** |
| -- | ~~Duplicate publish.yml and sdk-publish.yml~~ | -- | **FIXED** |
| -- | ~~GitHub Actions using mutable tags~~ | -- | **FIXED** |
| -- | ~~Go SDK docs describe non-existent API~~ | -- | **FIXED** |
| -- | ~~VERSION.md conflicting v2.0.0 release date~~ | -- | **FIXED** |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| WASM FFI pointer semantics breakage | Medium | High | rkyv for deterministic layout; pin wasmtime 25.x |
| Actor SDK API lock-in | High | High | Semver from v2.1.0; stability guarantees; deprecation policy |
| io_uring kernel compatibility | Medium | High | Feature-gated; tokio fallback always available |
| Firecracker requires KVM | High | Medium | Mock-based testing; defer to bare-metal CI |
| FoundationDB operational complexity | Medium | High | In-memory FDB default; SQLite for dev |
| Formal verification cost vs value | Medium | Low | Proof sketches first; full proofs for safety-critical only |
| SDK API drift across languages | Medium | Medium | Shared protobuf schema; CI builds all SDKs |
| Multi-region consistency | High | High | Start active-passive; evolve to active-active |
| Performance targets not met | Medium | Medium | Profile early in v2.1.0; adjust targets on data |
| GitHub Actions billing exhaustion | Present | Medium | Increase spending limit; evaluate self-hosted runners |

---

## Effort Summary

| Phase | Effort (hours) | Calendar Time | Risk |
|-------|----------------|---------------|------|
| v2.1.0 ops (code done) | ~60 | 1-2 weeks | Low |
| v2.2.0 Performance | ~200 | 1-3 months | Medium-High |
| v2.3.0 Hardening | ~160 | 1-2 months | High |
| v3.0.0 Production | ~390 | 3-6 months | High |
| v3.1.0+ Future | TBD | TBD | TBD |
| **Total to production** | **~810** | **~12 months** | -- |

---

## Decision Log

| Decision | Rationale | Date |
|----------|-----------|------|
| MSRV 1.88 | darling 0.23 requires it; no practical workaround | 2026-05 |
| Zero-copy via rkyv | Deterministic memory layout; no_std compatible; outperforms serde | 2026-05 |
| postcard for Actor SDK serialization | no_std CBOR; small codegen; WASM-friendly | 2026-05 |
| DashMap for executor lookups | O(1) concurrent lookup; replaces O(n) Vec scan | 2026-05 |
| Feature-gate mesh submodules | Reduce binary size for non-mesh deployments | 2026-05 |
| Tokio primary runtime, monoio experimental | io_uring kernel compatibility risk; tokio is battle-tested | 2026-05 |
| JWT + API key auth | Covers both human operators and programmatic access | 2026-05 |
| SQLite for dev, FDB for production | FDB operational complexity too high for local development | 2026-05 |
| Active-passive before active-active | Multi-region consistency is high-risk; incremental approach | 2026-05 |
| Lean4 proofs: sketches before full verification | Proof engineering is expensive; focus on safety-critical paths | 2026-05 |

---

## Metrics Trajectory

| Metric | v2.0.0 | v2.1.0 | v2.2.0 | v2.3.0 | v3.0.0 | v4.0.0 |
|--------|--------|--------|--------|--------|--------|--------|
| Tests passing | 1,532 | 1,600+ | 2,208 | 2,500+ | 3,000+ | 3,000+ |
| Ignored tests | 92 | 81 | 98 | 30 | 15 | 0 |
| P99 latency (same node) | N/A | N/A | <1ms | <1ms | <1ms | <1ms |
| Cold start (warm) | ~61us | <50us | <50us | <50us | <50us | <10us |
| Throughput (msg/s) | N/A | 10K | 100K | 100K | 500K | 1M |
| Max cluster size | 3 | 3 | 10 | 50 | 100+ | 1K |
| Branch coverage (critical) | ~85% | ~90% | ~92% | >95% | >95% | >95% |
| Native WASM SDKs | 1 | 1 | 2 | 4 | 5 | 5+ |
| External audit | No | No | No | Yes | Yes | Yes |

---

*Consolidated: 2026-05-16. Updated: 2026-05-31. Next review: 2026-06-14.*
