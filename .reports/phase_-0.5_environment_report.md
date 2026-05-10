# Phase -0.5: Environment Materialization Report

**Generated:** 2026-03-05
**Status:** Complete
**Phase:** -0.5

## Overview

This phase establishes the immutable build environment and capability detection infrastructure for the Aether runtime. All build artifacts are designed to be bit-for-bit reproducible.

## Created Artifacts

### 1. Dockerfile
- **Location:** `/Dockerfile`
- **Multi-stage build with:**
  - `chef`: Base image with cargo-chef for dependency caching
  - `planner`: Dependency planning stage
  - `builder`: Compilation stage with deterministic flags
  - `runtime`: Minimal production image (~50MB)
  - `wasm-builder`: WebAssembly compilation target
  - `dev`: Development environment with all tooling

### 2. reproducibility.nix
- **Location:** `/reproducibility.nix`
- **Features:**
  - Flake-based Nix configuration
  - Rust nightly-2026-03-01 toolchain
  - crane for reproducible Cargo builds
  - Multi-target support (native + WASM)
  - Integrated devShell with all dependencies

### 3. tool_requirements.toml
- **Location:** `/.specs/00_requirements/tool_requirements.toml`
- **Specifies:**
  - Rust nightly-2026-03-01
  - Wasmtime >=25.0.0
  - Firecracker >=1.9.0
  - FoundationDB >=7.3.0
  - Lean4 >=4.12.0, Coq >=8.19.0
  - Protobuf >=3.25.0
  - All supporting tool versions

### 4. .envrc.example
- **Location:** `/.envrc.example`
- **Configures:**
  - Development environment variables
  - PATH settings
  - Nix flake integration
  - Tool availability checks

## Deterministic Build Configuration

### SOURCE_DATE_EPOCH
```
1733097600  # 2024-12-03T00:00:00Z
```

### RUSTFLAGS (for bit-for-bit reproducibility)
```
--remap-path-prefix ${HOME}=/ 
--remap-path-prefix ${PWD}=/aether
```

### Cargo Configuration
- `CARGO_INCREMENTAL=0` (disables incremental for reproducibility)
- `--locked` flag enforced
- Offline mode available via `CARGO_NET_OFFLINE=true`

## Capability Detection

### Required Capabilities
| Capability | Tool | Version | Status |
|------------|------|---------|--------|
| wasm-component-model | Wasmtime | >=25.0.0 | [WARN] Not detected |
| microvm-isolation | Firecracker | >=1.9.0 | [WARN] Not detected |
| distributed-tx-log | FoundationDB | >=7.3.0 | [WARN] Not detected |
| formal-verification | Lean4/Coq | >=4.12/8.19 | [WARN] Not detected |
| capability-based-auth | Native | N/A | [DONE] Implemented |

### Optional Capabilities
| Capability | Purpose | Status |
|------------|---------|--------|
| gpu-acceleration | Parallel proof verification | [WARN] Not detected |
| sgx-enclave | Hardware TEE | [WARN] Not detected |
| sev-snp | AMD secure encryption | [WARN] Not detected |

## Build Instructions

### Using Docker (Recommended for CI)
```bash
docker build -t aether-core:latest .
docker run --rm aether-core:latest --version
```

### Using Nix (Recommended for Development)
```bash
nix develop
cargo build --release
```

### Verify Reproducibility
```bash
# Build twice and compare hashes
nix build .#aether-core
sha256sum ./result/bin/aether-core

nix build .#aether-core --rebuild
sha256sum ./result/bin/aether-core
```

## Missing Capabilities

The following system-level tools require manual installation or host access:

1. **Firecracker** - MicroVM hypervisor (requires KVM access)
2. **FoundationDB** - Distributed database (requires cluster setup)
3. **Wasmtime** - WASM runtime (installable via Nix)
4. **Lean4/Coq** - Proof assistants (installable via Nix)

## Next Steps (Phase 0)

1. Install system dependencies on target infrastructure
2. Configure FoundationDB cluster
3. Set up Firecracker with KVM access
4. Validate formal verification toolchain
5. Run capability detection tests

## Verification Checklist

- [x] Dockerfile created with multi-stage build
- [x] reproducibility.nix created with flake
- [x] tool_requirements.toml specifies all versions
- [x] .envrc.example created for direnv
- [x] SOURCE_DATE_EPOCH set for determinism
- [x] RUSTFLAGS configured for path remapping
- [ ] System tools installed (requires host access)
- [ ] FoundationDB cluster running
- [ ] Firecracker available with KVM
- [ ] Formal verification tools available

## Environment Variables Summary

| Variable | Value | Purpose |
|----------|-------|---------|
| `SOURCE_DATE_EPOCH` | `1733097600` | Deterministic timestamps |
| `RUSTFLAGS` | `--remap-path-prefix...` | Reproducible paths |
| `CARGO_INCREMENTAL` | `0` | Disable incremental builds |
| `AETHER_RUNTIME_MODE` | `development/production` | Runtime behavior |
| `AETHER_WASM_ENGINE` | `wasmtime` | WASM runtime selection |

---

**Phase -0.5 Status:** [DONE] COMPLETE
**Ready for Phase 0:** Pending system tool installation
