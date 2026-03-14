# Project Aether Implementation Roadmap v2.0

**Version:** 2.0.0  
**Date:** 2026-03-09  
**Status:** ACTIVE  

---

## Executive Summary

This roadmap addresses the gaps identified in the current codebase after comprehensive analysis. The project has made significant progress with 31,153 lines of production code and 326 library tests passing. However, integration tests have 90 compilation errors due to API drift, and several components have stub implementations.

---

## Current State Analysis

### Code Statistics
| Metric | Current | Target | Gap |
|--------|---------|--------|-----|
| Core Lines of Code | 31,153 | 35,000 | 3,847 |
| Library Tests Passing | 326 | 400 | 74 |
| Integration Tests | 90 errors | 0 errors | Fix needed |
| Code Coverage | ~75% | 80% | 5% |
| Source Files | 77 | 85 | 8 |

### Component Status

| Component | Status | Lines | Tests | Notes |
|-----------|--------|-------|-------|-------|
| WASI Preview 2 | ✅ Complete | 1,320 | 27 | Full implementation |
| Actor System | ✅ Complete | 945 | 8 | Scheduler, mailbox, registry |
| WASM Engine | ✅ Complete | 1,382 | 10 | Instance, linker, pool |
| VM Manager | ✅ Complete | 700 | 6 | Firecracker, jailer, snapshot |
| Mesh Network | ✅ Complete | 715 | 8 | QUIC, connection, backpressure |
| Security | ✅ Complete | 1,190 | 5 | mTLS, RBAC, secrets |
| State Management | ✅ Complete | 2,103 | 12 | FDB, KV, transactions |
| Tracing | ✅ Complete | 450 | - | OpenTelemetry |
| Observability | ✅ Complete | 661 | 8 | Metrics, health |
| CLI | ⚠️ Partial | 658 | 16 | dev/deploy stubs |
| Dashboard | ✅ Complete | - | - | Axum-based |
| Integration Tests | ❌ Broken | - | 90 errors | API drift |

---

## Phase 1: Critical Fixes (Immediate)

### TASK-101: Fix Integration Test Compilation Errors

**Priority:** CRITICAL  
**Effort:** 8 hours  
**Status:** PENDING  

**Subtasks:**
1. Fix `CertificateValidator::validate()` - now async, needs `.await`
2. Add missing `CertificateChain` type export
3. Add missing `CertificateRotator` type export
4. Add state module constants: `MAX_CHECKPOINTS_PER_ACTOR`, `CHECKPOINT_VERSION`, `CHECKPOINT_PREFIX`
5. Fix `CapabilitySet` method naming (snake_case)
6. Fix `RoleName` From<&str> implementation
7. Fix `Permission` variant naming
8. Fix `MeshNode` import/path
9. Fix secrets API usage in tests

**Files:**
- `tests/integration/security/test_mtls_enforcement.rs`
- `tests/integration/security/test_secrets_leak.rs`
- `tests/integration/e2e_security.rs`
- `tests/integration/e2e_state_persistence.rs`
- `tests/integration/host_mesh.rs`
- `tests/integration/comprehensive.rs`
- `crates/core/src/security/mod.rs` (exports)
- `crates/core/src/state/mod.rs` (constants)

### TASK-102: Complete CLI Command Implementations

**Priority:** HIGH  
**Effort:** 4 hours  
**Status:** PENDING  

**Subtasks:**
1. Implement `aether dev` command
2. Implement `aether deploy` command

**Files:**
- `crates/cli/src/commands/dev.rs`
- `crates/cli/src/commands/deploy.rs`

### TASK-103: Complete WASI State Handle

**Priority:** HIGH  
**Effort:** 4 hours  
**Status:** PENDING  

**Subtasks:**
1. Implement state handle with FDB backend
2. Implement state handle with Redb fallback

**Files:**
- `crates/core/src/wasi/mod.rs` (lines 310, 317)

---

## Phase 2: Feature Completion

### TASK-201: Create Actor SDK Macros Crate

**Priority:** HIGH  
**Effort:** 6 hours  
**Status:** PENDING  

**Subtasks:**
1. Create `crates/actor-macros/` crate
2. Implement `#[actor]` attribute macro
3. Implement message handler derive macros
4. Update `crates/actor-sdk/src/lib.rs`

### TASK-202: Expand Chaos Testing

**Priority:** MEDIUM  
**Effort:** 8 hours  
**Status:** PENDING  

**Subtasks:**
1. Complete mesh partition tests
2. Complete cascading failure tests
3. Complete backpressure overload tests
4. Complete memory pressure tests
5. Complete actor crash recovery tests

**Files:**
- `tests/integration/chaos/*.rs`

### TASK-203: Expand Property-Based Testing

**Priority:** MEDIUM  
**Effort:** 6 hours  
**Status:** PENDING  

**Subtasks:**
1. Add proptest for capability system
2. Add proptest for message serialization
3. Add proptest for state operations

---

## Phase 3: Documentation & Quality

### TASK-301: Complete Rustdoc Coverage

**Priority:** MEDIUM  
**Effort:** 8 hours  
**Status:** PENDING  

**Subtasks:**
1. Add rustdoc to all public APIs in security module
2. Add rustdoc to all public APIs in mesh module
3. Add rustdoc to all public APIs in state module
4. Add rustdoc to all public APIs in wasi module

### TASK-302: Add Missing Module Exports

**Priority:** HIGH  
**Effort:** 2 hours  
**Status:** PENDING  

**Subtasks:**
1. Export `CertificateChain` from security module
2. Export `CertificateRotator` from security module
3. Export state constants from state module
4. Review and fix all public API exports

---

## Phase 4: Formal Verification (Future)

### TASK-401: Complete Lean4 Proofs

**Priority:** LOW  
**Effort:** 40+ hours  
**Status:** PENDING  

**Note:** All proof files are currently skeletons. This is a long-term effort.

---

## Execution Order

```
Phase 1 (Critical):
  TASK-102 ─┐
  TASK-103 ─┼─► All tests pass
  TASK-301 ─┤
  TASK-302 ─┘
       │
       ▼
  TASK-101 (Integration Tests)
       │
       ▼
Phase 2 (Features):
  TASK-201 ─┐
  TASK-202 ─┼─► Feature complete
  TASK-203 ─┘
       │
       ▼
Phase 4 (Future):
  TASK-401 (Formal Proofs)
```

---

## Success Metrics

| Metric | Current | Target | Verification |
|--------|---------|--------|--------------|
| Integration Tests | 90 errors | 0 errors | `cargo test --test integration` |
| Library Tests | 326 | 400+ | `cargo test --lib` |
| Clippy Warnings | ~20 | 0 | `cargo clippy` |
| Code Coverage | ~75% | 80% | `cargo tarpaulin` |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| API changes break more tests | Medium | Medium | Incremental fixes, run tests after each change |
| Missing types require core changes | Low | High | Add exports, minimal core changes |
| Performance regressions | Low | Medium | Benchmark after changes |

---

*Generated: 2026-03-09*  
*Next Review: After Phase 1 completion*
