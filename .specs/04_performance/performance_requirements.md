# Performance Requirements Specification
**Project Aether - Phase 4: Performance Engineering**

## Document Control
- **Version**: 1.0
- **Status**: Approved
- **Created**: 2026-03-05
- **Last Updated**: 2026-03-05
- **Author**: Performance Engineering Team
- **Review Status**: Complete

## 1. Executive Summary

This document defines quantitative performance requirements for Project Aether, establishing latency, throughput, resource utilization, and scalability targets across all system components. These requirements ensure the system meets real-time constraints for distributed actor orchestration.

## 2. Latency Targets

### 2.1 WebAssembly Runtime Latency

| Operation | P50 | P95 | P99 | P999 | Max | Unit |
|-----------|-----|-----|-----|------|-----|------|
| Cold Start (module instantiation) | <20 | <35 | <45 | <50 | 50 | µs |
| Warm Start (cached module) | <2 | <5 | <8 | <10 | 10 | µs |
| Function Call (no state) | <0.5 | <1 | <2 | <5 | 5 | µs |
| Function Call (with state) | <5 | <10 | <15 | <25 | 25 | µs |
| Memory Allocation (4KB) | <0.1 | <0.3 | <0.5 | <1 | 1 | µs |
| Memory Boundary Check | <0.01 | <0.02 | <0.03 | <0.05 | 0.05 | µs |
| Capability Check | <0.1 | <0.2 | <0.3 | <0.5 | 0.5 | µs |
| Message Serialization (1KB) | <2 | <4 | <6 | <10 | 10 | µs |
| Message Deserialization (1KB) | <2 | <4 | <6 | <10 | 10 | µs |

### 2.2 Virtual Machine (Firecracker) Latency

| Operation | P50 | P95 | P99 | P999 | Max | Unit |
|-----------|-----|-----|-----|------|-----|------|
| VM Boot (cold) | <80 | <100 | <115 | <125 | 125 | ms |
| VM Boot (warm/cache) | <15 | <25 | <35 | <50 | 50 | ms |
| VM Pause | <3 | <5 | <8 | <10 | 10 | ms |
| VM Resume | <5 | <8 | <12 | <15 | 15 | ms |
| VM Snapshot Create | <50 | <80 | <100 | <120 | 120 | ms |
| VM Snapshot Restore | <40 | <60 | <80 | <100 | 100 | ms |
| Context Switch (guest→host) | <5 | <10 | <15 | <20 | 20 | µs |

### 2.3 Network Mesh Latency

| Operation | P50 | P95 | P99 | P999 | Max | Unit |
|-----------|-----|-----|-----|------|-----|------|
| Local Actor-to-Actor (same node) | <0.5 | <1 | <2 | <5 | 5 | µs |
| Remote Actor-to-Actor (same datacenter) | <0.5 | <0.8 | <1 | <2 | 2 | ms |
| Remote Actor-to-Actor (cross-region) | <50 | <80 | <100 | <150 | 150 | ms |
| Service Discovery Lookup | <0.1 | <0.3 | <0.5 | <1 | 1 | ms |
| Routing Table Update | <1 | <3 | <5 | <10 | 10 | ms |
| Health Check (ping) | <0.1 | <0.2 | <0.3 | <0.5 | 0.5 | ms |
| Encryption/Decryption (TLS 1.3) | <0.05 | <0.1 | <0.15 | <0.2 | 0.2 | ms |

### 2.4 State Management Latency

| Operation | P50 | P95 | P99 | P999 | Max | Unit |
|-----------|-----|-----|-----|------|-----|------|
| State Hydration (cold, 1MB) | <20 | <35 | <45 | <50 | 50 | ms |
| State Hydration (warm, 1MB) | <5 | <10 | <15 | <20 | 20 | ms |
| State Snapshot (1MB) | <10 | <20 | <30 | <40 | 40 | ms |
| State Diff Apply (100KB) | <2 | <5 | <8 | <10 | 10 | ms |
| KV Store Get (local) | <0.01 | <0.05 | <0.1 | <0.2 | 0.2 | ms |
| KV Store Put (local) | <0.02 | <0.08 | <0.15 | <0.3 | 0.3 | ms |
| KV Store Get (distributed) | <1 | <3 | <5 | <10 | 10 | ms |
| KV Store Put (distributed, quorum) | <5 | <10 | <15 | <20 | 20 | ms |

### 2.5 End-to-End Request Latency

| Scenario | P50 | P95 | P99 | P999 | Max | Unit |
|----------|-----|-----|-----|------|-----|------|
| Simple Request (no state) | <0.1 | <0.5 | <1 | <5 | 5 | ms |
| Request with State Access | <5 | <15 | <30 | <50 | 50 | ms |
| Request with VM Isolation | <100 | <150 | <200 | <300 | 300 | ms |
| Complex Workflow (3 actors) | <10 | <30 | <50 | <100 | 100 | ms |
| Distributed Transaction (2PC) | <50 | <100 | <200 | <500 | 500 | ms |

## 3. Throughput Targets

### 3.1 WebAssembly Runtime Throughput

| Metric | Target | Unit | Notes |
|--------|--------|------|-------|
| Module Instantiations/sec | >100,000 | inst/s | Cold start per core |
| Function Invocations/sec | >10,000,000 | inv/s | Per core, no I/O |
| Actor Messages/sec | >5,000,000 | msg/s | Per core, local |
| Memory Operations/sec | >50,000,000 | ops/s | Per core |
| Serialization Throughput | >2 | GB/s | Per core |
| Deserialization Throughput | >2 | GB/s | Per core |

### 3.2 Virtual Machine Throughput

| Metric | Target | Unit | Notes |
|--------|--------|------|-------|
| VM Boots/sec | >50 | vm/s | Per host, cold start |
| VM Boots/sec (cached) | >500 | vm/s | Per host, warm cache |
| Concurrent VMs | >1,000 | VMs | Per host |
| VM Network Throughput | >10 | Gbps | Per VM |
| VM Disk I/O | >100,000 | IOPS | Per VM |

### 3.3 Network Mesh Throughput

| Metric | Target | Unit | Notes |
|--------|--------|------|-------|
| Messages/sec (local) | >10,000,000 | msg/s | Per node |
| Messages/sec (remote) | >1,000,000 | msg/s | Per node |
| Bandwidth (inter-node) | >25 | Gbps | Per node |
| Concurrent Connections | >100,000 | conns | Per node |
| Service Registrations/sec | >10,000 | reg/s | Cluster-wide |
| Health Checks/sec | >100,000 | checks/s | Cluster-wide |

### 3.4 State Management Throughput

| Metric | Target | Unit | Notes |
|--------|--------|------|-------|
| KV Reads/sec (local) | >1,000,000 | reads/s | Per node |
| KV Writes/sec (local) | >500,000 | writes/s | Per node |
| KV Reads/sec (distributed) | >100,000 | reads/s | Cluster-wide |
| KV Writes/sec (distributed) | >50,000 | writes/s | Cluster-wide |
| Snapshot Throughput | >1 | GB/s | Per node |
| Replication Bandwidth | >10 | Gbps | Cross-node |

## 4. Resource Utilization Targets

### 4.1 CPU Utilization

| Component | Idle | Normal | Peak | Max Acceptable |
|-----------|------|--------|------|----------------|
| Host Runtime | <2% | 10-40% | 60-80% | 85% |
| WASM Engine (per actor) | <0.1% | 5-20% | 40-60% | 70% |
| Network Mesh | <1% | 5-15% | 20-40% | 50% |
| State Manager | <1% | 5-20% | 30-50% | 60% |
| VM Host (per VM) | <5% | 10-40% | 50-70% | 80% |

### 4.2 Memory Utilization

| Component | Base | Per Actor | Per VM | Max | Unit |
|-----------|------|-----------|--------|-----|------|
| Host Runtime | 50 | - | - | 200 | MB |
| WASM Engine (per actor) | - | 0.5-2 | - | 5 | MB |
| Actor Code (avg) | - | 0.1-0.5 | - | 2 | MB |
| Actor State (avg) | - | 1-10 | - | 100 | MB |
| Firecracker VM (per VM) | - | - | 8 | 128 | MB |
| Network Mesh | 100 | - | - | 500 | MB |
| State Manager | 200 | - | - | 1,000 | MB |
| Page Cache | - | - | - | 4,000 | MB |

### 4.3 I/O Utilization

| Component | Read | Write | Unit |
|-----------|------|-------|------|
| Disk I/O (baseline) | <10 | <5 | MB/s |
| Disk I/O (peak) | <500 | <250 | MB/s |
| Network I/O (baseline) | <100 | <100 | Mbps |
| Network I/O (peak) | <10,000 | <10,000 | Mbps |

### 4.4 Kernel Resources

| Resource | Per Node | Per Actor | Per VM | Unit |
|----------|----------|-----------|--------|------|
| File Descriptors | <100,000 | <10 | <100 | fds |
| Threads | <1,000 | 0-1 | 1-4 | threads |
| epoll/kqueue instances | <100 | <1 | <1 | instances |
| io_uring Submissions | <65,536 | <64 | <256 | entries |
| Memory Mappings | <10,000 | <5 | <50 | mappings |

## 5. Scalability Targets

### 5.1 Actor Density

| Configuration | Target | Max | Notes |
|---------------|--------|-----|-------|
| Actors per Node (WASM only) | 100,000 | 150,000 | Light actors (<5MB each) |
| Actors per Node (mixed) | 50,000 | 75,000 | Mix of WASM and VMs |
| Actors per Node (VM heavy) | 1,000 | 2,000 | VM-isolated actors |
| Actors per Cluster | 10,000,000 | 50,000,000 | 100+ node cluster |

### 5.2 Node Scalability

| Metric | Target | Max | Notes |
|--------|--------|-----|-------|
| Nodes per Cluster | 1,000 | 10,000 | Full mesh overlay |
| Nodes per Datacenter | 500 | 5,000 | |
| Cross-DC Latency Penalty | <2x | 3x | vs intra-DC |

### 5.3 Message Rate Scalability

| Scale | Message Rate | Bandwidth | Notes |
|-------|--------------|-----------|-------|
| Single Node | 10M msg/s | 10 Gbps | Local actors |
| Small Cluster (10 nodes) | 50M msg/s | 25 Gbps | Mixed local/remote |
| Medium Cluster (100 nodes) | 200M msg/s | 100 Gbps | |
| Large Cluster (1000 nodes) | 500M msg/s | 500 Gbps | Aggregated |

### 5.4 State Scalability

| Metric | Target | Max | Notes |
|--------|--------|-----|-------|
| Total State Size (cluster) | 10 TB | 100 TB | Distributed |
| State per Actor | 10 MB | 1 GB | |
| KV Entries (cluster) | 1 billion | 10 billion | |
| Snapshot Size (single) | 100 GB | 500 GB | |

## 6. Real-Time Constraints

### 6.1 Hard Real-Time Operations (Must Never Exceed)

| Operation | Deadline | Consequence | Mitigation |
|-----------|----------|-------------|------------|
| Actor Scheduling Decision | 10 µs | Missed deadline | Pre-computed scheduling |
| Memory Boundary Check | 50 ns | Safety violation | Inline bounds check |
| Capability Check | 500 ns | Security bypass | Cached capabilities |
| Message Routing | 1 ms | Timeout | Fast path routing |
| Interrupt Handling | 100 µs | System instability | Dedicated interrupt thread |

### 6.2 Soft Real-Time Operations (Target with Degradation)

| Operation | Target | Degraded | Fallback |
|-----------|--------|----------|----------|
| Actor Activation | 50 ms | 100 ms | Queue request |
| State Hydration | 50 ms | 200 ms | Partial load |
| VM Boot | 125 ms | 250 ms | Use cached VM |
| Service Discovery | 1 ms | 10 ms | Use cache |
| Health Check | 1 ms | 10 ms | Assume healthy |

## 7. Performance Isolation

### 7.1 Noisy Neighbor Protection

| Isolation Dimension | Mechanism | Target |
|---------------------|-----------|--------|
| CPU Time | cgroups v2 + cpu.max | <5% impact from neighbor |
| Memory Bandwidth | RDT (CAT) | <10% impact from neighbor |
| Cache Space | RDT (CAT) | <10% impact from neighbor |
| Network Bandwidth | tc htb | <5% impact from neighbor |
| Disk I/O | io_uring + cgroups | <10% impact from neighbor |

### 7.2 Quality of Service Tiers

| Tier | CPU Shares | Memory | Network | Disk | Eviction Priority |
|------|------------|--------|---------|------|-------------------|
| Critical | 1024 | Guaranteed | High | High | Never |
| High | 512 | Guaranteed | Medium | Medium | Low |
| Normal | 256 | Best-effort | Low | Low | Medium |
| Best-effort | 128 | Best-effort | Minimal | Minimal | High |

## 8. Performance Monitoring Thresholds

### 8.1 Alert Thresholds

| Metric | Warning | Critical | Auto-remediation |
|--------|---------|----------|------------------|
| P99 Latency (actor) | >80% target | >100% target | Scale out |
| P99 Latency (mesh) | >80% target | >100% target | Add routes |
| CPU Utilization | >70% | >85% | Throttle best-effort |
| Memory Utilization | >80% | >90% | Evict actors |
| Message Queue Depth | >1000 | >10000 | Backpressure |
| Actor Density | >80% target | >90% target | Reject new actors |

### 8.2 SLO Targets

| SLO | Target | Measurement Window |
|-----|--------|-------------------|
| Availability | 99.99% | Monthly |
| Latency SLO (P99 < target) | 99.9% | 5 min window |
| Throughput SLO | 99% | 5 min window |
| Error Rate | <0.1% | 5 min window |

## 9. Test Conditions

### 9.1 Baseline Environment

| Component | Specification |
|-----------|---------------|
| CPU | AMD EPYC 7763 (64 cores, 2.45 GHz) |
| Memory | 512 GB DDR4-3200 |
| Storage | NVMe SSD (7GB/s read, 5GB/s write) |
| Network | 25 Gbps NIC (Mellanox ConnectX-6) |
| OS | Linux 6.1+ (fully preemptible kernel) |
| Kernel Config | PREEMPT_DYNAMIC, io_uring enabled |

### 9.2 Degraded Conditions

| Condition | Degradation Factor | Must Still Meet |
|-----------|-------------------|-----------------|
| CPU Reduced 50% | 0.5x | 50% throughput target |
| Memory Reduced 25% | 0.75x | 75% actor density |
| Network Congestion 50% | 0.5x | 50% mesh throughput |
| Disk Slow (100 MB/s) | 0.1x | P99 latency within 2x |

## 10. Validation Criteria

### 10.1 Benchmark Pass Criteria

- All P50 targets met: **Mandatory**
- All P95 targets met: **Mandatory**
- All P99 targets met: **Mandatory**
- All P999 targets met: **Strongly Recommended** (≥95% of benchmarks)
- All Max targets met: **Recommended** (≥90% of benchmarks)

### 10.2 Regression Thresholds

| Metric | Allowable Regression | Action Required |
|--------|---------------------|-----------------|
| Latency (any percentile) | <5% | None |
| Latency (any percentile) | 5-10% | Review required |
| Latency (any percentile) | >10% | Block merge |
| Throughput | <5% | None |
| Throughput | 5-10% | Review required |
| Throughput | >10% | Block merge |

## 11. Compliance

### 11.1 Standards Compliance

| Standard | Requirement | Compliance Level |
|----------|-------------|------------------|
| POSIX.1-2017 | Real-time extensions | Partial (see notes) |
| AUTOSAR | Timing constraints | Guideline |
| DO-178C | Determinism | For critical actors |
| IEC 61508 | Response time | For safety actors |

### 11.2 Traceability

All performance requirements trace to:
- PRD Section 3.2 (Performance Requirements)
- Architecture Decision ADR-006 (Performance Targets)
- Yellow Paper YP-PERF-001 (Performance Model)

## 12. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-05 | Performance Team | Initial version |

## 13. Appendices

### Appendix A: Measurement Methodology

All latencies measured using:
- High-resolution timers (clock_gettime(CLOCK_MONOTONIC))
- Statistical analysis with ≥1,000,000 samples
- Outlier detection and removal (3σ rule)
- Calibrated measurement overhead subtraction

### Appendix B: Hardware Dependencies

Performance targets assume:
- x86_64 architecture with AVX2
- Hardware virtualization (SVM/VMX)
- IOMMU for device passthrough
- NUMA-aware memory allocation
- Turbo boost enabled

### Appendix C: Software Dependencies

- Linux kernel 6.1+ with io_uring
- Rust 1.75+ with optimizations
- wasmtime 18+ with cranelift
- Firecracker v1.5+
- DPDK 23.11+ (optional fast path)
