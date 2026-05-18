# Roadmap to Production

**Date:** 2026-05-18
**Current Version:** 2.0.0 (released 2026-05-08)
**Target:** v3.0.0 Production Release
**Estimated Timeline:** ~12 months

---

## 1. Executive Summary

Aether Core v2.0.0 is a functionally complete distributed actor runtime with 1,532 passing tests, zero clippy warnings, and a full security stack (mTLS, RBAC, OPA policy, capability-based deny-by-default). The codebase spans ~97K LOC across 5 workspace crates, 5 SDKs, and 17 CLI commands.

The path to production follows four phases: hardening (v2.1.0), performance validation (v2.2.0), operational maturity (v2.3.0), and the production release (v3.0.0). The primary blockers are ops/deployment issues (Dockerfile builds wrong target), CI/CD audit findings (32 issues identified), and SDK version drift across languages.

The long-term vision targets v4.0.0 "Universal Runtime" with formal verification, WASM Component Model as universal ABI, and CHERI/RISC-V hardware sandboxing.

---

## 2. Current State Assessment

### Version: 2.0.0

| Category | Status |
|----------|--------|
| Core runtime | Production-quality (1,532 tests, zero failures) |
| CI/CD | 13 workflows, multiple audit findings |
| Documentation | Mixed: README current, .docs stale (last updated 2026-03-12) |
| SDKs | Rust native WASM; Go/Python/JS/Java are gRPC clients only |
| Security | mTLS, RBAC, OPA, capability system, 57 security/fuzz tests |
| Performance | All 5 v2.0 targets met (see table below) |

### Test Matrix

| Suite | Passed | Failed | Ignored | Total |
|-------|--------|--------|---------|-------|
| Unit (core) | 1,072 | 0 | 9 | 1,081 |
| Unit (cli) | 33 | 0 | 0 | 33 |
| Unit (server) | 59 | 0 | 0 | 59 |
| Integration | 266 | 0 | 21 | 287 |
| Property (proptest) | 16 | 0 | 0 | 16 |
| Fuzz targets | 17 | 0 | 0 | 17 |
| Security | 20 | 0 | 0 | 20 |
| WASM E2E | 10 | 0 | 0 | 10 |
| Doc-tests | 27 | 0 | 47 | 74 |
| Memory benchmarks | 4 | 0 | 0 | 4 |
| **Total** | **1,532** | **0** | **92** | **1,624** |

Ignored breakdown: 9 FDB (requires running instance), 7 Firecracker (requires KVM), 21 cluster E2E (requires 3-node cluster), 47 doc-tests (reference external state), 8 test fixtures.

### Quality Gates (All Passing)

| Gate | Result |
|------|--------|
| `cargo fmt --all -- --check` | Zero violations |
| `cargo clippy --workspace --all-features -- -D warnings` | Zero warnings |
| `cargo test --workspace --all-features` | 1,532 passed |
| `cargo doc --workspace --no-deps` | Zero warnings |
| Forbidden patterns (todo!, unimplemented!, stubs) | Zero found |
| `unsafe` blocks with SAFETY comments | All 28 documented |
| Pre-commit hook (6-gate) | ACTIVE |
| Pre-push hook (5-gate) | ACTIVE |
| Dependency alignment (axum 0.7, tower-http 0.5) | Unified |

### v2.0.0 Performance Baseline

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| WASM warm spawn (from pool) | <50us | ~61us | PASS (with pool pre-warm) |
| Actor spawn throughput | >100K/s | ~378K/s | PASS |
| Intra-node mesh latency | <1ms | ~0.5ms | PASS |
| State access (read) | <10us | ~180ns | PASS |
| Actor density | 100K/node | Tested @ 100K | PASS |

### Codebase Metrics

| Metric | Value |
|--------|-------|
| Total source lines | ~97K across 252 files |
| Workspace crates | 5 (core, cli, actor-sdk, server, tests) |
| Feature flags | 7 (wasm, mesh, enterprise, fdb, firecracker, chaos, instance-pool) |
| Benchmark suites | 16 Criterion + 1 memory |
| CI/CD workflows | 13 GitHub Actions |
| SDKs | 5 (Rust, Go, Python, JavaScript, Java) |
| Vulnerabilities | 19 remaining (all low-severity dev deps) |

---

## 3. Known Technical Debt

### CI/CD Audit Findings (32 issues)

#### Critical (P0)

| # | Issue | Risk | Fix |
|---|-------|------|-----|
| 1 | Dockerfile builds Python server, not Rust core | Deployments broken | Rewrite Dockerfile (2d) |
| 2 | `--set image.repository` missing from Helm values | K8s deploys fail | Add to values.yaml (0.5d) |
| 3 | GitHub Actions billing limit blocks all workflows | CI entirely broken | Increase limit; evaluate self-hosted (external) |
| 4 | Third-party actions not pinned to SHA | Supply-chain risk | Pin all action refs (0.3d) |

#### High (P1)

| # | Issue | Risk | Fix |
|---|-------|------|-----|
| 5 | Docs-site references stale versions (1.3.0/1.4.0) | User confusion | Update to 2.0.0 (0.5d) |
| 6 | Broken link in tutorial.md | Broken docs | Fix link (0.1d) |
| 7 | 6 orphan docs-site pages not in mkdocs.yml nav | Hidden content | Add to nav (0.5d) |
| 8 | Go SDK API docs inconsistent (BaseActor vs ctx style) | SDK confusion | Reconcile (1d) |
| 9 | Rust SDK status contradiction in sdks/overview.md | Misleading docs | Fix (0.1d) |
| 10 | `|| true` in chaos/stress tests masks failures | Hidden regressions | Remove (0.3d) |
| 11 | Benchmark regression gating absent | Silent perf regressions | Add to CI (1d) |

#### Medium (P2)

| # | Issue | Risk | Fix |
|---|-------|------|-----|
| 12 | VERSION.md has duplicate section | Confusion | Clean up (0.5d) |
| 13 | publish.yml and sdk-publish.yml overlap | Maintenance burden | Consolidate (0.5d) |
| 14 | Go 1.22 and pnpm 9 not standardized across workflows | Build variance | Standardize (0.3d) |
| 15 | kubeconfig step missing in gitops.yml | Deploy fails | Add step (0.5d) |
| 16 | No-op jobs waste CI minutes (sdk-compatibility, update-homebrew, publish-java) | Billing waste | Remove (0.3d) |
| 17 | sdk-publish version step not gated on tag push | Spurious publishes | Gate on tag (0.1d) |
| 18 | No rust-toolchain.toml (toolchain not unified) | Build variance | Create file (0.1d) |

#### Low (P3)

| # | Issue | Risk | Fix |
|---|-------|------|-----|
| 19 | `.docs/` files stale (last updated 2026-03-12) | Outdated references | Refresh in v2.1.0 |
| 20 | 47 ignored doc-tests (reference external state) | Coverage gap | Fix or remove |
| 21 | `actor/zero_copy/stubs.rs` (141 LOC stub) | Incomplete feature | Implement in v2.2.0 |
| 22 | Lean4 proofs contain `sorry` (5 theorems) | Unverified safety | Complete in v3.0.0 |

### Documentation Debt

| Item | Status | Priority |
|------|--------|----------|
| `.docs/` directory (8 files) | Stale since 2026-03-12 | P2 |
| Docs-site version refs | Point to 1.3.0/1.4.0 | P1 |
| 6 orphan pages not in nav | Undiscoverable | P1 |
| Broken tutorial link | User-blocking | P1 |
| Go SDK API style inconsistent | Developer confusion | P1 |
| Rust SDK status self-contradictory | Misleading | P2 |

### SDK Gaps

| SDK | Current State | Gap |
|-----|---------------|-----|
| Rust (actor-sdk) | Native WASM, complete (1,358 LOC) | None |
| Go | gRPC client only | No native WASM actor support |
| Python | gRPC client only | No native WASM actor support |
| JavaScript | gRPC client only | No native WASM actor support |
| Java | gRPC client, 387 tests (not compiled) | Not compiled; no native WASM |

---

## 4. Phase 1: Hardening (v2.1.0)

**Status:** Code tasks DONE. Remaining: ops tasks.
**Timeline:** 1-2 weeks
**Effort:** ~60 hours

### Objectives

Fix deployment blockers, clean documentation, harden CI supply chain.

### Tasks

| ID | Task | Priority | Effort | Status |
|----|------|----------|--------|--------|
| D1 | Rewrite Dockerfile to build Rust core | P0 | 2d | Open |
| D2 | Add `--set image.repository` to Helm values | P0 | 0.5d | Open |
| D3 | Update docs-site stale version refs (1.3.0/1.4.0 to 2.0.0) | P1 | 0.5d | Open |
| D4 | Fix broken link in tutorial.md | P1 | 0.1d | Open |
| D5 | Add 6 orphan docs-site pages to mkdocs.yml nav | P1 | 0.5d | Open |
| D6 | Reconcile Go SDK API docs (pick BaseActor+ctx style) | P1 | 1d | Open |
| D7 | Fix Rust SDK status contradiction in sdks/overview.md | P2 | 0.1d | Open |
| D8 | Clean up VERSION.md (remove duplicate section) | P2 | 0.5d | Open |
| D9 | Consolidate publish.yml and sdk-publish.yml | P2 | 0.5d | Open |
| D10 | Standardize Go 1.22 and pnpm 9 across workflows | P2 | 0.3d | Open |
| D11 | Add kubeconfig step to gitops.yml | P2 | 0.5d | Open |
| D12 | Remove no-op jobs (sdk-compatibility, update-homebrew, publish-java) | P2 | 0.3d | Open |
| D13 | Gate sdk-publish version step on tag push | P2 | 0.1d | Open |
| D14 | Pin third-party actions to SHA | P0 | 0.3d | Open |
| D15 | Create rust-toolchain.toml | P2 | 0.1d | Open |

### Success Criteria

- [ ] `docker build` produces working Rust core image
- [ ] `helm install` succeeds with custom image repository
- [ ] Zero broken links in docs-site
- [ ] All docs-site pages reachable from nav
- [ ] CI supply-chain: all third-party actions pinned to SHA
- [ ] Zero no-op CI jobs

---

## 5. Phase 2: Performance Validation (v2.2.0)

**Timeline:** 1-3 months
**Effort:** ~200 hours
**Risk:** Medium-High (io_uring kernel compat, zero-copy correctness)

### Objectives

Validate performance against targets, implement zero-copy hot paths, add benchmark regression gating.

### Targets

| Metric | Current | Target | Gap |
|--------|---------|--------|-----|
| P99 latency (same node) | N/A | <1ms | Needs measurement |
| P99 latency (cross-node) | ~5ms | <2ms | 2.5x improvement |
| WASM warm spawn (from pool) | ~61us | <50us | 18% improvement |
| Memory per actor (idle) | ~32KB | <16KB | 2x reduction |
| Binary size (stripped) | TBD | <15MB | Needs measurement |
| Message throughput (local) | ~100K/s | 500K/s | 5x improvement |

### Tasks

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
| L1 | CLI server integration for 13 commands | P1 | 4d |
| L2 | CLI server integration (remaining commands) | P1 | 2d |
| D1 | JS SDK messaging support | P1 | 1.5d |
| D2 | Go SDK messaging support | P1 | 1.5d |
| D3 | Go OTel tracing integration | P1 | 1d |
| Q1 | Deprecate serialization_legacy module | P2 | 0.3d |
| Q2 | Dependency cleanup (unused crates) | P2 | 0.5d |
| Q3 | Remove `|| true` from chaos/stress tests | P2 | 0.3d |
| T1 | Per-tenant resource isolation | P1 | 2d |
| T2 | Scoped secrets per tenant | P1 | 2d |

### Test Trajectory

| Metric | v2.1.0 | v2.2.0 |
|--------|--------|--------|
| Tests passing | 1,600+ | 2,000+ |
| Ignored tests | 81 | 51 |
| Branch coverage (critical) | ~90% | ~92% |
| Native WASM SDKs | 1 | 2 |

### Success Criteria

- [ ] All 6 performance targets met and verified by Criterion
- [ ] Benchmark regression gating active in CI (fail on >5% regression)
- [ ] Zero-copy message path replaces stubs.rs (141 LOC stub eliminated)
- [ ] io_uring backend behind feature flag with tokio fallback
- [ ] WASM spawn <50us from pool

---

## 6. Phase 3: Production Readiness (v3.0.0)

**Timeline:** 6-9 months (includes v2.3.0 hardening sub-phase)
**Effort:** ~550 hours (160 hardening + 390 production)
**Risk:** High (formal verification, multi-region, external audit)

### Sub-Phase 3a: v2.3.0 -- Production Hardening

**Timeline:** 1-2 months
**Effort:** ~160 hours

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

**Targets:**

| Metric | Target |
|--------|--------|
| MTBF | >720h (30 days) |
| MTTR | <30s |
| Branch coverage (critical) | >95% |
| External security audit | Zero critical findings |
| Sustained throughput | 100K msg/s, P99 <5ms |

### Sub-Phase 3b: v3.0.0 -- Production Release

**Timeline:** 3-6 months
**Effort:** ~390 hours

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

**Targets:**

| Requirement | Target |
|-------------|--------|
| Uptime SLA | 99.95% |
| RPO | 0 (synchronous replication) |
| RTO | <60s (automated failover) |
| Scale | 100+ nodes, 1M+ actors |
| Regions | 3+ active-active |
| Support | Commercial tier with response SLA |

### Success Criteria

- [ ] External security audit: zero critical/high findings
- [ ] Branch coverage >95% on critical paths
- [ ] MTBF >720h in staging
- [ ] MTTR <30s (automated, not manual)
- [ ] Multi-region active-active deployed in staging
- [ ] 100-node load test passing (1M actors, 500K msg/s)
- [ ] All 5 SDKs at native WASM parity
- [ ] SBOM generated per release
- [ ] SLO dashboards operational

---

## 7. Phase 4: Ecosystem Growth (v3.x)

### v3.1.0 -- Edge and IoT

| Feature | Description |
|---------|-------------|
| MCU support | Lightweight single-binary for <64MB RAM devices |
| WASI preview 2 | Upgrade from preview 1 for improved capability model |
| Aggressive memory pooling | Target <8KB per actor |
| Network partition tolerance | CRDTs for local-first offline operation |
| Air-gapped deployment | Full offline install with no external dependencies |
| Hardware abstraction | GPIO/I2C/SPI access from WASM actors |

### v3.2.0 -- AI-Native Runtime

| Feature | Description |
|---------|-------------|
| Model serving actors | Hot-loaded ML models as WASM actors with automatic batching |
| Prompt injection defense | Sandboxed LLM I/O with content policy enforcement |
| Token budget enforcement | Per-actor and per-tenant token quotas |
| GPU passthrough | WGPU backend for inference acceleration |
| Semantic routing | Automatic actor placement based on workload characteristics |
| Autonomous agent patterns | Multi-step reasoning with tool-use actors |

### v3.3.0 -- Multi-Cloud Federation

| Feature | Description |
|---------|-------------|
| Cross-cloud state replication | Eventually consistent across AWS/GCP/Azure |
| Global load balancing | QUIC mesh spanning multiple providers |
| Multi-tenant isolation | Hardware-level separation via namespace isolation |
| Cost optimization | Automatic actor placement based on spot/preemptible pricing |
| Federated identity | Cross-cluster mTLS with shared CA |

---

## 8. Phase 5: Enterprise Features (v4.0.0)

**Vision:** Universal Runtime

| Feature | Description | Dependency |
|---------|-------------|------------|
| WASM Component Model | Language-agnostic actor interface as universal ABI | WASI preview 2 |
| CHERI/RISC-V integration | Hardware-enforced sandboxing | Hardware availability |
| Full Lean4 verification | Complete proofs for scheduler and consensus | v3.0 proof sketches |
| Built-in service mesh | Replace external stack (Istio/Linkerd) | v3.0 mesh maturity |
| Built-in observability | Replace external Prometheus/Grafana stack | v3.0 metrics pipeline |
| Built-in CI/CD | Actor build/test/deploy pipeline in runtime | v3.0 OCI registry |
| Universal compatibility | WASM for new code, Firecracker VMs for legacy | v3.0 MicroVM support |

---

## 9. Risk Register

| Risk | Probability | Impact | Mitigation | Owner |
|------|-------------|--------|------------|-------|
| WASM FFI pointer semantics breakage | Medium | High | rkyv for deterministic layout; pin wasmtime 25.x | Core |
| Actor SDK API lock-in | High | High | Semver from v2.1.0; stability guarantees; deprecation policy | SDK |
| io_uring kernel compatibility | Medium | High | Feature-gated; tokio fallback always available | Core |
| Firecracker requires KVM | High | Medium | Mock-based testing; defer to bare-metal CI | Infra |
| FoundationDB operational complexity | Medium | High | In-memory FDB default; SQLite for dev | Ops |
| Formal verification cost vs value | Medium | Low | Proof sketches first; full proofs for safety-critical only | Core |
| SDK API drift across languages | Medium | Medium | Shared protobuf schema; CI builds all SDKs | SDK |
| Multi-region consistency | High | High | Start active-passive; evolve to active-active | Core |
| Performance targets not met | Medium | Medium | Profile early in v2.1.0; adjust targets on data | Core |
| GitHub Actions billing exhaustion | Present | Medium | Increase spending limit; evaluate self-hosted runners | Infra |
| Dockerfile rewrite introduces bugs | Medium | Medium | Integration tests in CI; staging deploy validation | Ops |
| External security audit findings | Medium | High | Pre-audit preparation (H9); fix known issues first | Security |
| Single maintainer bus factor | High | High | Document architecture; ADRs; knowledge graph | Project |

---

## 10. Success Metrics

### Phase 1 (v2.1.0) -- Hardening

| KPI | Target | Measurement |
|-----|--------|-------------|
| Dockerfile builds Rust core | Yes | `docker build` produces working image |
| Helm deploy success rate | 100% | `helm install` + smoke test |
| Broken docs links | 0 | Link checker in CI |
| CI supply-chain (SHA-pinned actions) | 100% | Audit action refs |
| No-op CI jobs removed | All | Workflow inspection |

### Phase 2 (v2.2.0) -- Performance

| KPI | Target | Measurement |
|-----|--------|-------------|
| P99 latency (same node) | <1ms | Criterion benchmark |
| P99 latency (cross-node) | <2ms | Criterion benchmark |
| WASM warm spawn | <50us | Criterion benchmark |
| Message throughput (local) | 500K/s | Criterion benchmark |
| Memory per actor (idle) | <16KB | Memory benchmark suite |
| Benchmark regression CI | Active | Fails on >5% regression |

### Phase 3 (v3.0.0) -- Production

| KPI | Target | Measurement |
|-----|--------|-------------|
| Uptime SLA | 99.95% | SLO monitoring |
| MTBF | >720h | Incident tracking |
| MTTR | <30s | Automated recovery metrics |
| Branch coverage (critical) | >95% | cargo-llvm-cov |
| External audit critical findings | 0 | Audit report |
| Max cluster size | 100+ nodes | Load test |
| Actor count at scale | 1M+ | Load test |
| Native WASM SDKs | 5/5 | SDK feature matrix |
| SBOM per release | Generated | CI artifact |
| Sustained throughput | 100K msg/s, P99 <5ms | Load test |

### Phase 4 (v3.x) -- Ecosystem

| KPI | Target | Measurement |
|-----|--------|-------------|
| MCU binary size | <5MB | Build artifact |
| Actor memory (edge) | <8KB | Memory benchmark |
| Offline deployment | Full | Air-gapped test |
| SDK ecosystem packages | 10+ | Registry count |
| Community contributors | 20+ | GitHub stats |

### Phase 5 (v4.0.0) -- Enterprise

| KPI | Target | Measurement |
|-----|--------|-------------|
| Formal verification | Complete | Lean4 proof checker |
| WASM Component Model | Adopted | Component Model conformance |
| Compliance certifications | SOC 2, FIPS 140-2 | Audit reports |
| SLA guarantee | 99.99% | Commercial SLA |
| Language-agnostic ABI | Universal | Component Model interface |

---

## Effort Summary

| Phase | Effort (hours) | Calendar Time | Risk |
|-------|----------------|---------------|------|
| v2.1.0 Hardening | ~60 | 1-2 weeks | Low |
| v2.2.0 Performance | ~200 | 1-3 months | Medium-High |
| v2.3.0 Hardening | ~160 | 1-2 months | High |
| v3.0.0 Production | ~390 | 3-6 months | High |
| v3.1.0+ Ecosystem | TBD | TBD | TBD |
| **Total to production (v3.0.0)** | **~810** | **~12 months** | -- |

---

## Decision Log (Active)

| Decision | Rationale | Date |
|----------|-----------|------|
| MSRV 1.88 | darling 0.23 requires it; no practical workaround | 2026-05 |
| Zero-copy via rkyv | Deterministic memory layout; no_std compatible; outperforms serde | 2026-05 |
| postcard for Actor SDK serialization | no_std CBOR; small codegen; WASM-friendly | 2026-05 |
| Tokio primary runtime, monoio experimental | io_uring kernel compatibility risk; tokio is battle-tested | 2026-05 |
| Active-passive before active-active | Multi-region consistency is high-risk; incremental approach | 2026-05 |
| SQLite for dev, FDB for production | FDB operational complexity too high for local development | 2026-05 |

---

*Generated: 2026-05-18. Next review: 2026-06-01. Source: ROADMAP.md, VERSION.md, CHANGELOG.md, ARCHITECTURE.md.*
