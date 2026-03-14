# Phase 5: Adversarial Loop - Feasibility Spike Prototypes

## Overview
This directory contains minimal compile-ready prototypes to validate critical path risks identified in the Aether architecture.

## Critical Path Risks Being Validated

| Risk ID | Risk | Target | Status |
|---------|------|--------|--------|
| CP-001 | WASM cold start | <50µs | Pending |
| CP-002 | Firecracker boot | <125ms | Pending |
| CP-003 | State hydration | <50ms | Pending |
| CP-004 | QUIC mesh connectivity | <10ms | Pending |
| CP-005 | Capability enforcement overhead | <1µs | Pending |

## Prototype Structure

```
06_prototypes/
├── cold_start_spike/      # CP-001: WASM cold start validation
├── mesh_spike/            # CP-004: QUIC mesh connectivity
├── capability_spike/      # CP-005: Capability enforcement
├── hal_mock/              # Hardware Abstraction Layer mocks
└── fuzzing/               # Fuzzing infrastructure
```

## Build Instructions

### Prerequisites
- Rust 1.75+ with nightly toolchain
- wasmtime-cli 15.0+
- cargo-fuzz

### Building All Prototypes
```bash
cd .specs/06_prototypes
cargo build --all --release
```

### Running Benchmarks
```bash
# Cold start benchmark
cd cold_start_spike && cargo bench

# Mesh connectivity test
cd mesh_spike && cargo test --release

# Capability overhead measurement
cd capability_spike && cargo bench
```

## Spike Results Summary

| Spike | Result | Measurement | Pass/Fail |
|-------|--------|-------------|-----------|
| Cold Start | Pending | - | - |
| Mesh | Pending | - | - |
| Capability | Pending | - | - |

## Risk Assessment

### Identified Risks
1. **Cold Start**: Module compilation time dominates
2. **Mesh**: TLS handshake overhead in QUIC
3. **Capability**: Cache locality affects enforcement speed

### Mitigations
1. Pre-compile modules, use pooling allocator
2. Pre-shared keys, 0-RTT connections
3. Inline capability checks, CPU cache optimization

## Next Steps
1. Complete all spike implementations
2. Run benchmarks on target hardware
3. Document findings in `.reports/phase_05_prototype_results.md`
4. Update architecture based on results
