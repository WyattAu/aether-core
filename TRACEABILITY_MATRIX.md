# Traceability Matrix: Project Aether

**Version:** 2.0.0  
**Last Updated:** 2026-05-10  
**Phase:** Post-v2.0.0 Hardening / v2.1.0 Development

---

## 1. Executive Summary

This document consolidates all traceability information for Project Aether, linking requirements to standards, test cases, components, and architecture decisions. Complete traceability is maintained from source documents through implementation.

**Total Requirements:** 40  
**Standards Referenced:** 17  
**Test Cases Defined:** 1,623 (1,072 core + 23 cli + 10 cli-lib + 59 server + 267 integration + 16 property + 4 benchmark + 20 security + 17 fuzz + 7 fixtures + 10 wasm-e2e + 27 doc + 92 doc-ignored)  
**Components Identified:** 75  
**Tests Passing:** 1,531 / 1,623 (0 failures, 92 ignored requiring external deps)

---

## 2. Requirements to Standards Mapping

### 2.1 Execution & Runtime (REQ-EXEC)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-EXEC-01 | Universal Compatibility | WASI Preview 2, OCI Runtime Spec, Component Model | Must | TC-EXEC-01-* | Defined |
| REQ-EXEC-02 | Hybrid Isolation | IEC 62443 (SL 3), NIST SP 800-53 SC-3 | Must | TC-EXEC-02-* | Defined |
| REQ-EXEC-03 | Hot-Swapping | ISO 27001 A.12.1.2 | Should | TC-EXEC-03-* | Defined |
| REQ-EXEC-04 | Memory-Safe FFI | IEC 61508 Part 3, ISO 26262 Part 6 | Must | TC-EXEC-04-* | Defined |
| REQ-EXEC-05 | Panic-less Host | IEC 61508 Part 3, ISO 26262 Part 6 | Must | TC-EXEC-05-* | Defined |
| REQ-EXEC-06 | Linear Memory Constraints | WASI Preview 2, OWASP ASVS V1 | Must | TC-EXEC-06-* | Defined |
| REQ-EXEC-07 | Virtualized I/O | WASI Preview 2, NIST SP 800-53 AC-3 | Must | TC-EXEC-07-* | Defined |
| REQ-EXEC-08 | Binary Reproducibility | ISO 27001 A.14.2.6, NIST SP 800-53 SA-12 | Should | TC-EXEC-08-* | Defined |
| REQ-EXEC-09 | Mutation Testing | IEC 61508 Part 7, IEEE 829 | Should | TC-EXEC-09-* | Defined |

### 2.2 Networking & Connectivity (REQ-NET)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-NET-01 | Unified Mesh | RFC 9000, RFC 1035, RFC 8446 | Should | TC-NET-01-* | Defined |
| REQ-NET-02 | Socket Spoofing | WASI Sockets, RFC 9000, RFC 793 | Should | TC-NET-02-* | Defined |
| REQ-NET-03 | Protocol Fallback | RFC 9000, RFC 8446 | Should | TC-NET-03-* | Defined |
| REQ-NET-04 | SSH Passthrough | RFC 4251, RFC 4254 | Could | TC-NET-04-* | Defined |
| REQ-NET-05 | Backpressure | RFC 793, RFC 9000, RFC 5681 | Should | TC-NET-05-* | Defined |

### 2.3 Storage & Persistence (REQ-STOR)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-STOR-01 | Ephemeral State | ACID (FoundationDB) | Should | TC-STOR-01-* | Defined |
| REQ-STOR-02 | Block Volumes | VirtIO Spec, NVMe Spec | Should | TC-STOR-02-* | Defined |
| REQ-STOR-03 | Object Shim | AWS S3 API, WASI Filesystem | Could | TC-STOR-03-* | Defined |
| REQ-STOR-04 | Block-Device Pinning | IEC 61508 (Data Integrity) | Should | TC-STOR-04-* | Defined |

### 2.4 Orchestration & Scheduling (REQ-ORCH)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-ORCH-01 | Declarative Config | IEEE 1016, TOML Spec | Must | TC-ORCH-01-* | Defined |
| REQ-ORCH-02 | Placement Constraints | NIST SP 800-53 SC-5 | Should | TC-ORCH-02-* | Defined |
| REQ-ORCH-03 | Scale-to-Zero | NIST SP 800-53 SC-5 | Should | TC-ORCH-03-* | Defined |

### 2.5 Safety & Stability (REQ-SAFE)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-SAFE-01 | Zero Panic | IEC 61508 Part 3, ISO 26262 Part 6 | Must | TC-SAFE-01-* | Defined |
| REQ-SAFE-02 | No Hot Path Allocation | IEC 61508 Part 7 | Must | TC-SAFE-02-* | Defined |
| REQ-SAFE-03 | Cache-Line Alignment | IEC 61508 Part 7 | Must | TC-SAFE-03-* | Defined |
| REQ-SAFE-04 | MicroVM Jailing | IEC 62443 (SL 4), NIST SP 800-53 SC-3 | Must | TC-SAFE-04-* | Defined |

### 2.6 Security (REQ-SEC)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-SEC-01 | Capability-Based Access | NIST SP 800-53 AC-3, IEC 62443 | Must | TC-SEC-01-* | Defined |
| REQ-SEC-02 | Cryptographic Identity | RFC 8446, FIPS 140-2/3 | Should | TC-SEC-02-* | Defined |
| REQ-SEC-03 | Secrets Management | NIST SP 800-53 SC-12, ISO 27001 A.10.1.2 | Must | TC-SEC-03-* | Defined |
| REQ-SEC-04 | mTLS Control Plane | RFC 8446, NIST SP 800-52 | Should | TC-SEC-04-* | Defined |
| REQ-SEC-05 | Audit Log Immutability | NIST SP 800-53 AU-9, ISO 27001 A.12.4 | Should | TC-SEC-05-* | Defined |

### 2.7 Debugging & Determinism (REQ-DBG)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-DBG-01 | Host-Injected Time | WASI Clocks, IEC 61508 | Should | TC-DBG-01-* | Defined |
| REQ-DBG-02 | Core Dumps | WASI Coredump Spec, ELF | Should | TC-DBG-02-* | Defined |
| REQ-DBG-03 | Zero-Copy Serialization | IEC 61508 (Performance) | Should | TC-DBG-03-* | Defined |
| REQ-DBG-04 | Time-Travel Injection | IEC 61508, WASI Preview 2 | Could | TC-DBG-04-* | Defined |

### 2.8 Performance (REQ-PERF)

| Req ID | Requirement | Standards | Priority | Test Cases | Status |
|--------|-------------|-----------|----------|------------|--------|
| REQ-PERF-01 | WASM Cold Start | NIST SP 800-53 SC-5 | Must | TC-PERF-01-* | Defined |
| REQ-PERF-02 | MicroVM Cold Start | NIST SP 800-53 SC-5 | Should | TC-PERF-02-* | Defined |
| REQ-PERF-03 | Intra-Node Latency | RFC 9000 | Should | TC-PERF-03-* | Defined |
| REQ-PERF-04 | State Access Latency | IEC 61508 | Should | TC-PERF-04-* | Defined |
| REQ-PERF-05 | Memory Overhead | IEC 61508 | Should | TC-PERF-05-* | Defined |
| REQ-PERF-06 | CPU Efficiency | IEC 61508 | Should | TC-PERF-06-* | Defined |

---

## 3. Standards to Requirements Coverage

| Standard | Requirements Covered | Coverage % | Priority |
|----------|---------------------|------------|----------|
| WASI Preview 2 | 8 | 100% | Critical |
| IEC 61508 | 12 | 100% | Critical |
| NIST SP 800-53 | 10 | 100% | High |
| ISO 27001 | 6 | 100% | High |
| RFC 9000 (QUIC) | 4 | 100% | High |
| IEC 62443 | 3 | 100% | High |
| FIPS 140-2/3 | 3 | 100% | High |
| RFC 8446 (TLS 1.3) | 4 | 100% | High |
| ISO 26262 | 3 | 100% | Medium |
| IEEE 1016 | 1 | 100% | Medium |
| IEEE 829 | 1 | 100% | Medium |
| OWASP ASVS | 1 | 100% | Medium |
| RFC 793 (TCP) | 2 | 100% | Medium |
| RFC 4251 (SSH) | 1 | 100% | Low |
| GDPR | Referenced via ISO 27001 | - | High |
| CCPA | Referenced via ISO 27001 | - | High |
| VirtIO Spec | 2 | 100% | Medium |

---

## 4. Architecture Decisions to Requirements

| ADR | Decision | Requirements | Standards | Status |
|-----|----------|--------------|-----------|--------|
| ADR-001 | Dual Runtime Architecture | REQ-EXEC-01, REQ-EXEC-06 | IEC 61508, NIST SP 800-53 | Accepted |
| ADR-002 | Deny-by-Default Capability Model | REQ-SEC-01, REQ-SAFE-01 | IEC 61508 | Accepted |
| ADR-003 | Panic Abort Policy | REQ-EXEC-05, REQ-SAFE-01 | IEC 61508 | Accepted |
| ADR-004 | Wasmtime WASM Runtime Selection | REQ-EXEC-01, REQ-EXEC-07 | WASI Preview 2 | Accepted |
| ADR-005 | Firecracker VMM Selection | REQ-SAFE-04, REQ-EXEC-02 | IEC 62443 | Accepted |
| ADR-006 | Multi-Language SDK Strategy | REQ-EXEC-06, REQ-EXEC-07 | ISO 12207 | Accepted |
| ADR-007 | MkDocs for Documentation Site | REQ-SEC-05 | ISO 12207 | Accepted |

---

## 5. Test Cases to Requirements

### 5.1 Test Coverage Summary

| Category | Unit Tests | Integration | System | Performance | Total |
|----------|------------|-------------|--------|-------------|-------|
| REQ-EXEC | 9 | 9 | 5 | 4 | 27 |
| REQ-NET | 5 | 5 | 3 | 2 | 15 |
| REQ-STOR | 4 | 4 | 2 | 2 | 12 |
| REQ-ORCH | 3 | 3 | 2 | 1 | 9 |
| REQ-SAFE | 4 | 4 | 2 | 2 | 12 |
| REQ-SEC | 5 | 5 | 3 | 2 | 15 |
| REQ-DBG | 4 | 4 | 2 | 2 | 12 |
| REQ-PERF | 6 | 6 | 2 | 4 | 18 |
| **Total** | **40** | **40** | **21** | **19** | **120** |

### 5.2 Critical Test Cases

| Test ID | Test Case | Requirements | Type | Priority |
|---------|-----------|--------------|------|----------|
| TC-EXEC-05-01 | Panic prevention | REQ-EXEC-05 | Unit | Critical |
| TC-SAFE-01-01 | Zero panic validation | REQ-SAFE-01 | Unit | Critical |
| TC-SEC-01-01 | Default capability denial | REQ-SEC-01 | Security | Critical |
| TC-PERF-01-01 | WASM cold start latency | REQ-PERF-01 | Performance | Critical |
| TC-EXEC-02-01 | WASM escape prevention | REQ-EXEC-02 | Security | Critical |
| TC-SAFE-04-01 | MicroVM jailing | REQ-SAFE-04 | Security | Critical |

---

## 6. Documentation to Standards

| Document | Content | Standards | Status |
|----------|---------|-----------|--------|
| domain_analysis.md | Domain model, terminology | ISO 12207 | Complete |
| applicable_standards.md | Standards mapping | ISO 12207 | Complete |
| requirements.md | EARS-compliant requirements | ISO 12207, IEEE 1016 | Complete |
| acceptance_criteria.md | Measurable criteria | IEEE 829 | Complete |
| stakeholder_analysis.md | Stakeholder matrix | ISO 12207 | Complete |
| moscow_priority.md | Prioritization | ISO 12207 | Complete |
| traceability_matrix.md | Full traceability | ISO 12207 | Complete |
| privacy_impact_assessment.md | GDPR analysis | GDPR | Not yet created |
| safety_case.md | Safety argumentation | IEC 61508, ISO 26262 | Not yet created |

---

## 7. Phase Deliverables

| Phase | Deliverable | Requirements | Standards | Status |
|-------|-------------|--------------|-----------|--------|
| -1 | Context Discovery | Initial scope | ISO 12207 | Complete |
| 0 | Requirements Engineering | All 40 | All 17 | Complete |
| 1 | Core Runtime (Local) | 14 Must | IEC 61508, WASI, NIST | Implemented |
| 2 | Distributed Mesh | 23 Should | RFC 9000, IEC 62443 | Implemented |
| 3 | Enterprise Platform | 3 Could | ISO 27001, FIPS | Partial |
| 5 | Adversarial Loop | Prototype validation | All | In Progress |

---

## 8. Risk to Requirements Traceability

| Risk | Impacted Requirements | Mitigation | Status |
|------|----------------------|------------|--------|
| WASI Preview 2 instability | REQ-EXEC-01, REQ-EXEC-06, REQ-EXEC-07 | Abstraction layer, version locking | Mitigated |
| Performance targets | REQ-PERF-* | Incremental optimization | Active |
| Security certification | REQ-SEC-* | Early engagement with auditor | Planned |
| FoundationDB complexity | REQ-STOR-01, REQ-ORCH-* | Comprehensive training | Planned |
| Firecracker API changes | REQ-EXEC-02, REQ-SAFE-04 | Version pinning | Mitigated |
| UDP blocking (firewalls) | REQ-NET-01, REQ-NET-03 | TCP fallback | Designed |

---

## 9. Component to Requirements Mapping

### 9.1 Runtime Components

| Component | Requirements | Priority | Status |
|-----------|--------------|----------|--------|
| aether-runtime::wasmtime_engine | REQ-EXEC-01, REQ-PERF-01 | Must | Implemented |
| aether-runtime::firecracker_engine | REQ-EXEC-01, REQ-PERF-02 | Must | Implemented |
| aether-runtime::host | REQ-EXEC-05, REQ-SAFE-01 | Must | Implemented |
| aether-runtime::wasi_shim | REQ-EXEC-07, REQ-SEC-01 | Must | Implemented |
| aether-runtime::ffi_bridge | REQ-EXEC-04 | Must | Implemented |

### 9.2 Security Components

| Component | Requirements | Priority | Status |
|-----------|--------------|----------|--------|
| aether-security::capability_engine | REQ-SEC-01 | Must | Implemented |
| aether-security::secrets | REQ-SEC-03 | Must | Implemented |
| aether-security::identity | REQ-SEC-02 | Should | Implemented |
| aether-security::mtls | REQ-SEC-04 | Should | Implemented |
| aether-audit::logger | REQ-SEC-05 | Should | Implemented |

### 9.3 Mesh Components

| Component | Requirements | Priority | Status |
|-----------|--------------|----------|--------|
| aether-mesh::quinn_mesh | REQ-NET-01, REQ-PERF-03 | Should | Implemented |
| aether-mesh::socket_shim | REQ-NET-02 | Should | Implemented |
| aether-mesh::protocol_bridge | REQ-NET-05 | Should | Implemented |
| aether-mesh::dns_resolver | REQ-NET-01 | Should | Implemented |

### 9.4 State Components

| Component | Requirements | Priority | Status |
|-----------|--------------|----------|--------|
| aether-state::fdb_client | REQ-STOR-01 | Should | Implemented |
| aether-state::serializer | REQ-DBG-03 | Should | Implemented |
| aether-storage::volume_manager | REQ-STOR-02 | Should | Implemented |

### 9.5 Orchestration Components

| Component | Requirements | Priority | Status |
|-----------|--------------|----------|--------|
| aether-config::parser | REQ-ORCH-01 | Must | Implemented |
| aether-orch::scheduler | REQ-ORCH-02, REQ-ORCH-03 | Should | Implemented |
| aether-orch::deployment_controller | REQ-EXEC-03 | Should | Implemented |

---

## 10. Change Impact Matrix

| Change Type | Impact Assessment | Required Updates |
|-------------|-------------------|------------------|
| New standard | Add to standards mapping, assess conflicts | Matrix, conflicts doc |
| Requirement change | Update all trace links | Matrix, test cases, components |
| Architecture change | Verify requirement coverage | ADR, requirements |
| Test failure | Trace to requirement, standard | Test case, requirement |
| Component change | Verify requirement implementation | Component, tests |

---

## 11. Verification Status

| Category | Defined | Implemented | Tested | Verified |
|----------|---------|-------------|--------|----------|
| REQ-EXEC (9) | 9 | 9 | 8 | 0 |
| REQ-NET (5) | 5 | 5 | 4 | 0 |
| REQ-STOR (4) | 4 | 4 | 3 | 0 |
| REQ-ORCH (3) | 3 | 3 | 3 | 0 |
| REQ-SAFE (4) | 4 | 4 | 4 | 0 |
| REQ-SEC (5) | 5 | 5 | 5 | 0 |
| REQ-DBG (4) | 4 | 4 | 3 | 0 |
| REQ-PERF (6) | 6 | 6 | 5 | 0 |
| **Total (40)** | **40** | **40** | **35** | **0** |

---

## 12. Status Summary

| Category | Status |
|----------|--------|
| Requirements | 40/40 defined, 40/40 implemented |
| Tests | 1,531 passing (92 ignored: FDB/Firecracker/cluster/doc-tests) |
| Standards | 17 mapped, compliance matrix active |
| ADRs | 7 documented (ADR-001 through ADR-007) |
| Quality | Zero clippy warnings, deny-all safety lints |
| Security | 20 security tests, 17 fuzz tests, vulnerability scanning via cargo-deny |

---

Last Updated: 2026-05-13
Phase: Post-v2.0.0 Hardening
Next Phase: v2.1.0 Development
