# Capability Matrix: Project Aether

## 1. Build Toolchain

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Rust nightly | nightly-2026-03-01 | ⬜ | - | Missing | Install via rustup |
| Cargo | 1.85+ | ⬜ | - | Missing | Bundled with Rust |
| wasm-tools | 1.220+ | ⬜ | - | Missing | `cargo install wasm-tools` |
| wit-bindgen | 0.33+ | ⬜ | - | Missing | `cargo install wit-bindgen-cli` |
| protoc | 28+ | ⬜ | - | Missing | System package |

## 2. Runtime Dependencies

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Wasmtime | 27+ | ⬜ | - | Missing | System install or bundled |
| Firecracker | 1.10+ | ⬜ | - | Missing | Download from GitHub releases |
| jailer | bundled | ⬜ | - | Missing | Bundled with Firecracker |
| FoundationDB | 7.3+ | ⬜ | - | Missing | System package |

## 3. Platform Features

| Capability | Required | Available | Status | Notes |
|------------|----------|-----------|--------|-------|
| KVM access | Yes | ⬜ | Unknown | Check `/dev/kvm` permissions |
| io_uring | Linux 5.19+ | ⬜ | Unknown | Check kernel version |
| VT-x/AMD-V | Yes | ⬜ | Unknown | Check `/proc/cpuinfo` |
| NVMe storage | Recommended | ⬜ | Unknown | Check available storage |

## 4. Development Tools

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| clippy | bundled | ⬜ | - | Missing | Bundled with Rust |
| rustfmt | bundled | ⬜ | - | Missing | Bundled with Rust |
| cargo-nextest | 0.9+ | ⬜ | - | Missing | `cargo install cargo-nextest` |
| proptest | 1.6+ | ⬜ | - | Missing | Cargo dependency |

## 5. Formal Verification

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Lean4 | 4.12+ | ⬜ | - | Missing | Optional for Phase 0+ |
| Coq | 8.20+ | ⬜ | - | Missing | Optional for Phase 0+ |
| TLA+ | 1.8+ | ⬜ | - | Missing | Optional for Phase 1+ |

## 6. Network & Security

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| rustls | 0.23+ | ⬜ | - | N/A | Cargo dependency |
| Quinn | 0.11+ | ⬜ | - | N/A | Cargo dependency |
| ring | 0.17+ | ⬜ | - | N/A | Cargo dependency |

## 7. Status Summary

| Category | Total | Available | Missing | Unknown |
|----------|-------|-----------|---------|---------|
| Build Toolchain | 5 | 0 | 5 | 0 |
| Runtime Dependencies | 5 | 0 | 5 | 0 |
| Platform Features | 4 | 0 | 0 | 4 |
| Development Tools | 4 | 0 | 4 | 0 |
| Formal Verification | 3 | 0 | 3 | 0 |
| **Total** | **21** | **0** | **17** | **4** |

## 8. Capability Acquisition Plan

### Phase -1 (Immediate)
1. Install Rust toolchain: `rustup install nightly-2026-03-01`
2. Install WASM tools: `cargo install wasm-tools wit-bindgen-cli`
3. Install protoc: System package manager
4. Verify KVM access: `ls -la /dev/kvm`
5. Verify kernel version: `uname -r`

### Phase 0 (Architecture)
1. Install Wasmtime: Download or build from source
2. Install Firecracker: Download from GitHub releases
3. Install FoundationDB: System package or Docker

### Phase 1+ (Development)
1. Install formal verification tools as needed
2. Install profiling tools
3. Configure CI/CD infrastructure

## 9. Verification Checklist

Run these commands to verify capabilities:

```bash
# Rust toolchain
rustup show active-toolchain
rustc --version

# WASM tools
wasm-tools --version
wit-bindgen --version

# Protobuf
protoc --version

# Platform
ls -la /dev/kvm
uname -r
cat /proc/cpuinfo | grep -E "vmx|svm"

# Storage
lsblk | grep -i nvme
```

---
Last Updated: 2026-03-05
Status: Pending Initial Assessment
