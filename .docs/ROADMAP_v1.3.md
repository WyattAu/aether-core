# Project Aether Roadmap v1.3.0 and Beyond

**Version:** 1.3.0-planning  
**Date:** 2026-03-14  
**Status:** Planning  

---

## Executive Summary

This document outlines the roadmap for Project Aether following the v1.2.0-alpha release. It addresses critical gaps identified in codebase analysis, planned feature enhancements, and the strategic direction for the project.

---

## 1. Critical Issues (Must Fix Before v1.3.0)

These issues pose production risk and must be addressed:

### 1.1 WASM Execution Not Implemented
**File:** `crates/core/src/mcp/execution_tools.rs:183-192`  
**Priority:** 🔴 Critical  
**Effort:** Medium (2-3 days)

The `ExecuteWasmTool` loads and validates WASM modules but never executes them. This breaks the MCP tool contract.

**Fix:**
```rust
// Current (broken)
async fn execute(&self, args: Value) -> Result<ToolResult> {
    // ... validation ...
    ToolResult::text("WASM module validated but not executed")  // WRONG
}

// Required
async fn execute(&self, args: Value) -> Result<ToolResult> {
    let module = self.load_module(&args)?;
    let result = self.engine.execute(module, args).await?;
    ToolResult::text(result)
}
```

### 1.2 Local Mesh Requests Return Error
**File:** `crates/core/src/mesh/node.rs:169`  
**Priority:** 🔴 Critical  
**Effort:** Low (1 day)

Intra-node actor communication fails because local request handling is not implemented.

**Fix:**
```rust
pub async fn send_request(&self, target: &ActorId, request: Vec<u8>) -> Result<Vec<u8>> {
    if self.is_local(target) {
        // Route to local actor registry instead of error
        return self.local_registry.send(target, request).await;
    }
    // ... existing mesh logic
}
```

### 1.3 Vault Secrets Integration is Non-Functional
**File:** `crates/core/src/security/secrets/secrets_legacy.rs:434-467`  
**Priority:** 🔴 Critical  
**Effort:** Medium (2-3 days)

All Vault operations return errors. HashiCorp Vault integration is a documented feature.

**Fix:** Implement actual Vault API client or mark as experimental/remove from docs.

### 1.4 Panic-Prone Code Paths
**Files:** Multiple (see analysis)  
**Priority:** 🔴 Critical  
**Effort:** Medium (2-3 days)

Multiple `expect()` and `unwrap()` calls in production paths can cause runtime panics.

**Locations requiring fixes:**
| File | Line | Issue |
|------|------|-------|
| `actor/executor.rs` | 282 | `expect()` in Default impl |
| `actor/scheduler.rs` | 233 | `expect()` on thread spawn |
| `ai/providers.rs` | 335+ | Multiple `unwrap()` in HTTP parsing |
| `dashboard/server.rs` | 28+ | `unwrap()`/`expect()` in init |
| `mesh/quic.rs` | 38+ | Multiple `unwrap()` in QUIC setup |

---

## 2. High Priority (v1.3.0 Target)

### 2.1 Deadlock Prevention - MutexGuard Across Await
**Files:** `actor/rpc.rs`, `actor/supervisor.rs`, `chaos/mod.rs`, `mcp/server.rs`  
**Priority:** 🟠 High  
**Effort:** Medium (2-3 days)  
**Status:** ✅ Done

Multiple locations hold `MutexGuard` across `.await` points, which can cause deadlocks.

**Pattern to fix:**
```rust
// BAD - can deadlock
let guard = self.mutex.lock().await;
some_async_op().await;  // Guard held across await!
drop(guard);

// GOOD - drop before await
{
    let guard = self.mutex.lock().await;
    let data = guard.data.clone();
} // Guard dropped
some_async_op(data).await;
```

### 2.2 Complete Actor SDK (Currently 50%)
**Priority:** 🟠 High  
**Effort:** High (1-2 weeks)
**Status:** 🔄 In Progress

Missing SDK components:
- [ ] Full API coverage for all actor operations
- [x] Python SDK (enhanced)
- [x] JavaScript/TypeScript SDK (tests added)
- [ ] Go SDK
- [ ] Comprehensive examples

### 2.3 Test Coverage Gaps
**Priority:** 🟠 High  
**Effort:** Medium (1 week)
**Status:** ✅ Done

Files without test modules:
- `mcp/server.rs`, `mcp/tools.rs`, `mcp/actor_tools.rs`
- `engine/module.rs`
- `dashboard/handlers.rs`

### 2.4 Dashboard Module Issues
**Priority:** 🟠 High  
**Effort:** Medium (1 week)
**Status:** ✅ Done

- Axum version conflict (0.7.9 from tonic vs 0.8.8)
- Missing `ui/dist` folder for static files
- TUI uses mock data instead of real metrics

### 2.5 Deprecated Field Usage
**File:** `crates/core/src/wasi/mod.rs:381`  
**Priority:** 🟡 Medium  
**Effort:** Low (1 hour)
**Status:** ✅ Done

Using deprecated `timestamp_ns` instead of `wall_time_ns`.

### 2.6 Clippy Warning Cleanup
**Priority:** 🟡 Medium  
**Effort:** Low (1 day)
**Status:** ✅ Done

Fixed:
- Identical blocks in conditionals
- Complex type definitions (type aliases)
- Field reassignment with Default
- Vec init then push pattern

---

## 3. Medium Priority (v1.4.0 Target)

### 3.1 Dead Code Cleanup
**Effort:** Low (1-2 days)

Remove unused code:
- `MailboxBuilder` and associated methods
- Unused constants in `context/loader.rs`
- Unused fields in `security/secrets/` structs
- Unused methods in `chaos/fault_injector.rs`

### 3.2 Expose CLI Commands
**Effort:** Medium (3-5 days)

Implemented but not exposed:
- `aether import docker-compose` (747 lines)
- `aether rollback` (288 lines)
- `aether logs --follow`
- Shell completion generation

### 3.3 Integration Test Infrastructure
**Effort:** High (1-2 weeks)

- Multi-node mesh tests (currently mocked)
- Actual Firecracker integration tests
- End-to-end actor lifecycle tests
- CI pipeline for integration tests

### 3.4 Stub Implementation Completion
**Effort:** Medium (1 week)

| Component | Issue |
|-----------|-------|
| `actor/migration.rs` | Checkpoint creation is stub |
| `security/secret_injector.rs` | Memory locking is placeholder |
| `context/loader.rs` | Size limits not enforced |

---

## 4. Feature Roadmap

### v1.3.0 (Stabilization Release)
**Target:** 4 weeks  
**Theme:** Production Readiness

| Feature | Priority | Status |
|---------|----------|--------|
| WASM execution fix | Critical | ✅ Done |
| Local mesh fix | Critical | ✅ Done |
| Vault secrets fix | Critical | ✅ Done |
| Panic elimination | Critical | ✅ Done |
| Deadlock fixes | High | ✅ Done |
| MutexGuard across await | High | ✅ Done |
| Test coverage (MCP tools) | High | ✅ Done |
| Python SDK | High | ✅ Enhanced |
| JavaScript SDK | High | ✅ Tests added |
| Dashboard fixes | High | ✅ Done |
| Unused code cleanup | Low | ✅ Done |
| Missing documentation | Low | In Progress |

### v1.4.0 (SDK Release)
**Target:** 8 weeks  
**Theme:** Developer Experience

| Feature | Priority | Status |
|---------|----------|--------|
| Python SDK | High | ✅ Enhanced |
| JavaScript SDK | High | ✅ Tests added |
| Go SDK | High | ✅ Done |
| CLI improvements | Medium | Partial |
| Examples library | Medium | Partial |
| Documentation site | Medium | Partial |

### v1.5.0 (Scale Release)
**Target:** 12 weeks  
**Theme:** Production Scale

| Feature | Priority | Status |
|---------|----------|--------|
| Horizontal scaling | High | Design |
| Auto-scaling actors | High | Design |
| Global mesh routing | Medium | Design |
| Multi-region support | Medium | Design |
| Federation | Low | Research |

### v2.0.0 (Platform Release)
**Target:** 6 months  
**Theme:** Complete Platform

| Feature | Priority | Status |
|---------|----------|--------|
| Web dashboard | High | Design |
| Visual actor designer | Medium | Research |
| Marketplace/registry | Medium | Research |
| Enterprise features | Medium | Partial |
| Certification program | Low | Planning |

---

## 5. Technical Debt Register

| ID | Description | Impact | Effort | Priority |
|----|-------------|--------|--------|----------|
| TD-001 | Panic-prone code paths | Production stability | Medium | Critical |
| TD-002 | MutexGuard across await | Deadlock risk | Medium | High |
| TD-003 | Dead code accumulation | Maintainability | Low | Medium |
| TD-004 | Missing test coverage | Quality assurance | Medium | High |
| TD-005 | Deprecated API usage | Future compatibility | Low | Medium |
| TD-006 | Axum version conflict | Dependency management | Medium | High |
| TD-007 | Mock data in production | Incorrect metrics | Medium | High |
| TD-008 | Stub implementations | Feature completeness | Medium | High |

---

## 6. Architecture Improvements

### 6.1 Error Handling Standardization
Current: Mix of `Error::internal()`, custom errors, and `anyhow`  
Target: Unified error hierarchy with:
- Structured error codes
- Severity levels
- Retry guidance
- Context chaining

### 6.2 Configuration Management
Current: TOML only  
Target: Multiple backends:
- Environment variables
- Consul/etcd integration
- Kubernetes ConfigMaps
- Dynamic reload

### 6.3 Observability Enhancement
Current: OTLP traces, basic metrics  
Target: Full observability:
- Distributed tracing with baggage
- Custom metrics dashboard
- Log aggregation
- Alerting rules

### 6.4 Security Hardening
Current: mTLS, RBAC, secrets  
Target: Enhanced security:
- Secret rotation automation
- Audit logging
- Compliance reporting
- Security scanning in CI

---

## 7. Community & Ecosystem

### 7.1 Documentation
- [x] MkDocs documentation site (docs-site/)
- [x] API reference documentation (in README files)
- [ ] Architecture decision records (ADRs)
- [ ] Runbook for operations
- [ ] Security documentation
- [ ] Performance tuning guide

### 7.2 Examples
- [x] Hello World actor (5 languages)
- [x] Stateful counter actor
- [x] AI-powered actor
- [x] Mesh communication actor
- [x] Full application example

### 7.3 Tooling
- [ ] VS Code extension
- [ ] Language servers
- [ ] Debug adapters
- [ ] Performance profiler
- [ ] Migration tools

---

## 8. Release Cadence

| Release | Cadence | Content |
|---------|---------|---------|
| Patch (1.2.1) | As needed | Bug fixes, security |
| Minor (1.3.0) | Monthly | Features, improvements |
| Major (2.0.0) | Quarterly | Breaking changes, major features |

---

## 9. Success Metrics

| Metric | v1.2.0 | v1.3.0 Target | v2.0.0 Target |
|--------|--------|---------------|---------------|
| Test Coverage | 80% | 85% | 90% |
| Clippy Warnings | ~50 | 0 | 0 |
| Documentation | 90% | 95% | 100% |
| Cold Start P99 | <50µs | <30µs | <20µs |
| Actors/Node | TBD | 50,000 | 100,000 |
| GitHub Stars | TBD | 500 | 2,000 |
| Contributors | 5 | 20 | 50 |

---

## 10. Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| WASM engine complexity | Medium | High | Incremental fixes, extensive testing |
| Security vulnerabilities | Medium | Critical | Regular audits, dependency scanning |
| Performance regression | Low | High | Benchmarking in CI |
| Community adoption | Medium | Medium | Documentation, examples, outreach |
| Dependency conflicts | Medium | Medium | Regular updates, version pinning |

---

*Last Updated: 2026-03-16*  
*Next Review: 2026-03-23*

## Changelog

### 2026-03-16 (Session 3)
- **Go SDK v0.1.0**: Complete implementation with comprehensive examples
  - Created stateful counter actor example
  - Created AI-powered actor example
  - Created mesh communication actor example
  - Created chat application example (multi-actor)
- **Documentation**: Created MkDocs documentation site structure
  - Getting started guide (introduction, installation, quickstart, concepts)
  - SDKs overview and language-specific guides
  - Examples overview with learning path
  - Architecture documentation stubs
- **README**: Fixed and expanded Go SDK README with full API reference

### 2026-03-16 (Session 2)
- **Deprecated Field**: Replaced `timestamp_ns` with `wall_time_ns` in WasiHost trait
- **Clippy Warnings**: Fixed identical blocks, complex types, field reassignment patterns
- **Type Aliases**: Added `WorkerStealer` type alias in scheduler.rs
- **Code Quality**: Converted Vec::new() + push to vec![] macro in health.rs
- **Test Code**: Fixed field assignment patterns in test code

### 2026-03-16 (Session 1)
- **Dashboard Module**: Fixed panic patterns (unwrap/expect) in server.rs, static_files.rs
- **Documentation**: Added comprehensive docs to dashboard module (handlers, server, ws, static_files)
- **Dead Code**: Addressed all dead code warnings with #[allow(dead_code)] for reserved APIs
- **MutexGuard**: Fixed remaining MutexGuard across await in mcp/server.rs and secrets/registry.rs
- **Clippy Warnings**: Reduced from ~2280 to ~2045 (mostly documentation)
