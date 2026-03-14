# Aether Milestone Definitions

**Version:** 1.0.0  
**Generated:** 2026-03-06  
**Total Milestones:** 5

---

## Overview

This document defines the major milestones for Project Aether, their acceptance criteria, dependencies, and success metrics. Each milestone represents a significant capability delivery point.

---

## M1: Local WASM Execution

**Target:** Week 8  
**Dependencies:** Phase 1, Phase 2  
**Tasks:** TASK-001 through TASK-020

### Description

Single-node WASM actor execution with capability enforcement and fuel metering.

### Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| M1-AC-01 | WASM modules load and execute | Integration test with 10 sample modules |
| M1-AC-02 | Cold start < 50ms at P99 | Benchmark suite |
| M1-AC-03 | Fuel metering prevents infinite loops | Test with runaway module |
| M1-AC-04 | Capability enforcement blocks unauthorized access | Security test suite |
| M1-AC-05 | Actor scheduler handles 1000 concurrent actors | Load test |
| M1-AC-06 | All CLI commands work for local execution | CLI integration test |

### Success Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| Cold start P50 | < 20ms | TBD |
| Cold start P99 | < 50ms | TBD |
| Throughput | > 10k invocations/s | TBD |
| Memory per actor | < 5MB | TBD |
| Test coverage | > 80% | TBD |

### Technical Demonstration

```bash
# Deploy a WASM actor
aether deploy --file hello.wasm --name hello-world

# Invoke the actor
aether invoke hello-world --input '{"name": "Aether"}'

# Check status
aether status hello-world

# View logs
aether logs hello-world
```

### Dependencies

- [x] Phase 1: Core Runtime Foundation
- [ ] Phase 2: WASM Engine (TASK-011 through TASK-020)

### Risk Factors

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Cold start target missed | Medium | High | Pool-based prewarming |
| Memory leaks | Medium | Medium | Comprehensive sanitization |

### Exit Criteria

- All acceptance criteria met
- Performance benchmarks within 10% of targets
- No critical or high-severity bugs
- Documentation complete

---

## M2: Local OCI Execution

**Target:** Week 12  
**Dependencies:** M1, Phase 3  
**Tasks:** TASK-021 through TASK-030

### Description

Dual-runtime support with Firecracker-based OCI container execution alongside WASM actors.

### Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| M2-AC-01 | OCI containers execute in Firecracker VMs | Integration test with standard images |
| M2-AC-02 | VM start time < 125ms at P99 | Benchmark suite |
| M2-AC-03 | Jailer enforces seccomp and cgroup isolation | Security test suite |
| M2-AC-04 | Snapshot/restore preserves container state | State verification test |
| M2-AC-05 | Dual-runtime router selects correct backend | Routing test suite |
| M2-AC-06 | Script interpreters work via WASM shims | Python/JS execution tests |

### Success Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| VM start P50 | < 80ms | TBD |
| VM start P99 | < 125ms | TBD |
| Snapshot size | < 50MB | TBD |
| Restore time | < 150ms | TBD |
| OCI image pull | < 30s (100MB) | TBD |
| Test coverage | > 80% | TBD |

### Technical Demonstration

```bash
# Deploy a WASM actor
aether deploy --file actor.wasm --name wasm-actor

# Deploy an OCI container
aether deploy --image nginx:latest --name nginx-container

# Both coexist
aether status --all

# Invoke WASM actor
aether invoke wasm-actor --input '{}'

# Proxy to OCI container
aether proxy nginx-container --port 80
```

### Dependencies

- [ ] M1: Local WASM Execution
- [ ] Phase 3: Firecracker Integration (TASK-021 through TASK-030)

### Risk Factors

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| VM latency exceeded | Medium | High | Snapshot pre-creation |
| Jailer complexity | Low | Medium | Early integration |
| OCI compatibility | Low | Medium | Runtime-spec testing |

### Exit Criteria

- All acceptance criteria met
- Both runtimes operational simultaneously
- Security isolation verified
- Performance benchmarks within 10% of targets

---

## M3: Multi-Node Mesh

**Target:** Week 16  
**Dependencies:** M2, Phase 4, Phase 5  
**Tasks:** TASK-031 through TASK-048

### Description

Distributed mesh networking with actor addressing, state replication, and automatic failover.

### Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| M3-AC-01 | Nodes discover each other automatically | 5-node cluster test |
| M3-AC-02 | Actor-to-actor communication across nodes | Cross-node messaging test |
| M3-AC-03 | Message latency < 10ms at P99 | Network benchmark |
| M3-AC-04 | Failover within 100ms of node failure | Chaos test |
| M3-AC-05 | Actor state persists across restarts | State persistence test |
| M3-AC-06 | Actor migration preserves state | Migration test suite |

### Success Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| Message latency P50 | < 5ms | TBD |
| Message latency P99 | < 10ms | TBD |
| Failover time | < 100ms | TBD |
| Discovery convergence | < 30s | TBD |
| Throughput per connection | > 100k msg/s | TBD |
| Test coverage | > 80% | TBD |

### Technical Demonstration

```bash
# Initialize cluster
aether mesh init --node-id node-1

# Join additional nodes
aether mesh join --bootstrap node-1:9000

# List nodes
aether mesh nodes

# Deploy actor to specific node
aether deploy --file actor.wasm --name my-actor --node node-2

# Invoke remote actor
aether invoke my-actor --input '{}' --from node-1

# Migrate actor
aether migrate my-actor --to node-3

# Simulate failure (chaos test)
aether chaos kill node-2
# Actor should restart on surviving node
```

### Dependencies

- [ ] M2: Local OCI Execution
- [ ] Phase 4: Mesh Networking (TASK-031 through TASK-040)
- [ ] Phase 5: State Management (TASK-041 through TASK-048)

### Risk Factors

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Network partitions | Medium | High | Split-brain detection |
| State consistency | Medium | High | ACID transactions |
| Discovery failures | Low | Medium | Manual node addition |

### Exit Criteria

- All acceptance criteria met
- 5-node cluster operational
- Failover tested and working
- State replication verified

---

## M4: Production Ready

**Target:** Week 16  
**Dependencies:** M3, Phase 6, Phase 7  
**Tasks:** TASK-049 through TASK-076

### Description

Complete platform with CLI, observability, testing, and security hardening.

### Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| M4-AC-01 | All CLI commands documented and working | CLI documentation review |
| M4-AC-02 | Metrics export to Prometheus | Integration test |
| M4-AC-03 | Traces export to Jaeger | Integration test |
| M4-AC-04 | Health checks respond correctly | Endpoint test |
| M4-AC-05 | Security fuzzing finds no critical issues | Fuzzing campaign |
| M4-AC-06 | Performance benchmarks meet targets | Benchmark comparison |
| M4-AC-07 | End-to-end tests pass | E2E test suite |

### Success Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| Test coverage | > 85% | TBD |
| Critical CVEs | 0 | TBD |
| High CVEs | 0 | TBD |
| Documentation completeness | 100% | TBD |
| E2E test pass rate | 100% | TBD |
| Benchmark variance | < 10% | TBD |

### Technical Demonstration

```bash
# Full deployment workflow
aether config init --output aether.toml
aether deploy --file app.wasm --name production-app --replicas 3

# Monitoring
aether status --namespace production
aether logs production-app --follow

# Scaling
aether scale production-app --replicas 5

# Observability
curl http://localhost:9090/metrics  # Prometheus
curl http://localhost:8080/health   # Health check

# Security
aether capability grant production-app network:http:outbound
```

### Dependencies

- [ ] M3: Multi-Node Mesh
- [ ] Phase 6: CLI & Tooling (TASK-049 through TASK-056)
- [ ] Phase 7: Dashboard & Observability (TASK-057 through TASK-063)
- [ ] Integration tests (TASK-070 through TASK-076)

### Risk Factors

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Security vulnerabilities | Medium | Critical | Extensive fuzzing |
| Performance regression | Low | High | Continuous benchmarking |
| Documentation gaps | Low | Medium | Technical writer review |

### Exit Criteria

- All acceptance criteria met
- Security audit passed
- Performance validated
- Documentation complete
- E2E tests passing

---

## M5: Enterprise Features

**Target:** Week 16  
**Dependencies:** M4, Phase 8  
**Tasks:** TASK-064 through TASK-085

### Description

Enterprise-grade features including RBAC, audit logging, secrets management, and multi-tenancy.

### Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| M5-AC-01 | RBAC enforces role-based permissions | RBAC test suite |
| M5-AC-02 | Audit logs capture all operations | Audit verification |
| M5-AC-03 | Secrets encrypted at rest and in transit | Security test |
| M5-AC-04 | Resource quotas enforced per tenant | Quota test suite |
| M5-AC-05 | Multi-tenant isolation verified | Isolation test |
| M5-AC-06 | External security audit passed | Audit report |
| M5-AC-07 | Kubernetes deployment works | K8s integration test |

### Success Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| RBAC test coverage | 100% | TBD |
| Audit log completeness | 100% | TBD |
| Secret encryption strength | AES-256 | TBD |
| Tenant isolation | Complete | TBD |
| K8s deployment time | < 5 min | TBD |
| External audit findings | 0 critical | TBD |

### Technical Demonstration

```bash
# RBAC
aether role create admin --permissions "*"
aether role create developer --permissions "actors:*,logs:read"
aether user create alice --role developer

# Secrets
aether secret create db-password --value "s3cr3t"
aether deploy --file app.wasm --secret db-password:DB_PASS

# Multi-tenancy
aether tenant create team-a --quota cpu=4,memory=8Gi
aether deploy --file app.wasm --tenant team-a

# Kubernetes deployment
helm install aether ./helm/aether \
  --set replicas=3 \
  --set storage.class=fast-ssd

# Audit
aether audit list --user alice --since 1h
```

### Dependencies

- [ ] M4: Production Ready
- [ ] Phase 8: Enterprise Features (TASK-064 through TASK-069)
- [ ] Deployment artifacts (TASK-080 through TASK-082)
- [ ] Security audit (TASK-083)

### Risk Factors

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Audit findings | Medium | High | Pre-audit remediation |
| K8s compatibility | Low | Medium | Multi-version testing |
| Secret leakage | Low | Critical | Encryption at rest |

### Exit Criteria

- All acceptance criteria met
- External security audit passed
- Kubernetes deployment verified
- Multi-tenancy tested

---

## Milestone Timeline

```
Week 1-4:   Phase 1 (Foundation)
            └── No milestone

Week 5-8:   Phase 2 (WASM Engine)
            └── M1: Local WASM Execution ✓

Week 9-12:  Phase 3 (Firecracker)
            └── M2: Local OCI Execution ✓

Week 13-16: Phase 4, 5, 6, 7, 8 (Parallel)
            ├── M3: Multi-Node Mesh ✓
            ├── M4: Production Ready ✓
            └── M5: Enterprise Features ✓
```

## Milestone Dependencies

```
M1 (Week 8)
 └── M2 (Week 12)
      └── M3 (Week 16)
           ├── M4 (Week 16)
           └── M5 (Week 16)
```

## Go/No-Go Decisions

Each milestone requires a formal go/no-go decision:

### Go Criteria

- All acceptance criteria met
- No critical or high-severity bugs
- Performance within 10% of targets
- Documentation complete
- Test coverage > 80%

### No-Go Response

- Identify blocking issues
- Create remediation plan
- Set new target date
- Communicate to stakeholders

## Milestone Celebrations

| Milestone | Celebration |
|-----------|-------------|
| M1 | Team lunch |
| M2 | Team outing |
| M3 | Team dinner |
| M4 | Company announcement |
| M5 | Release party |
