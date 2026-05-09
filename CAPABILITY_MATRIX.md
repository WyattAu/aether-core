# Capability Matrix: Project Aether

## 1. Build Toolchain

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Rust nightly | nightly-2026-03-01 | ✅ | 1.96.0-nightly (38c0de8dc 2026-02-28) | ✅ Available | Close to target nightly |
| Cargo | 1.85+ | ✅ | 1.96.0-nightly (f298b8c82 2026-02-24) | ✅ Available | |
| wasm-tools | 1.220+ | ⬜ | - | ⬜ Missing | `cargo install wasm-tools` |
| wit-bindgen | 0.33+ | ⬜ | - | ⬜ Missing | `cargo install wit-bindgen-cli` |
| protoc | 28+ | ✅ | libprotoc 33.5 | ✅ Available | |

## 2. Runtime Dependencies

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Wasmtime | 25+ | ✅ | 25.0 (Cargo dep) | ✅ Available | Built as library dependency |
| Firecracker | 1.10+ | ⬜ | - | ⬜ Missing | Download from GitHub releases |
| jailer | bundled | ⬜ | - | ⬜ Missing | Bundled with Firecracker |
| FoundationDB | 7.3+ | ⬜ | - | ⬜ Missing | System package |

## 3. Platform Features

| Capability | Required | Available | Version/Detail | Status | Notes |
|------------|----------|-----------|----------------|--------|-------|
| KVM access | Yes | ✅ | /dev/kvm present | ✅ Available | |
| io_uring | Linux 5.19+ | ✅ | 7.0.3 (CachyOS) | ✅ Available | |
| VT-x/AMD-V | Yes | ✅ | vmx detected | ✅ Available | Intel VT-x |
| NVMe storage | Recommended | ✅ | nvme0n1 (1.8T) | ✅ Available | |

## 4. Development Tools

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| clippy | bundled | ✅ | 0.1.95 (38c0de8dcb 2026-02-28) | ✅ Available | |
| rustfmt | bundled | ✅ | 1.9.0-nightly (38c0de8dcb 2026-02-28) | ✅ Available | |
| cargo-nextest | 0.9+ | ✅ | 0.9.129 | ✅ Available | |
| proptest | 1.6+ | ✅ | 1.6 (Cargo dep) | ✅ Available | |

## 5. Formal Verification

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Lean4 | 4.12+ | ✅ | 4.29.1 | ✅ Available | Optional for Phase 0+ |
| Coq | 8.20+ | ⬜ | - | ⬜ Missing | Optional for Phase 0+ |
| TLA+ | 1.8+ | ⬜ | - | ⬜ Missing | Optional for Phase 1+ |

## 6. Network & Security (Cargo Dependencies)

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| rustls | 0.23+ | ✅ | 0.23 (Cargo dep) | ✅ Available | |
| Quinn | 0.11+ | ✅ | 0.11 (Cargo dep) | ✅ Available | |
| ring | 0.17+ | ✅ | 0.17 (Cargo dep) | ✅ Available | |

## 7. Status Summary

| Category | Total | Available | Missing |
|----------|-------|-----------|---------|
| Build Toolchain | 5 | 3 | 2 |
| Runtime Dependencies | 4 | 1 | 3 |
| Platform Features | 4 | 4 | 0 |
| Development Tools | 4 | 4 | 0 |
| Formal Verification | 3 | 1 | 2 |
| Network & Security | 3 | 3 | 0 |
| **Total** | **23** | **16** | **7** |

---
Last Updated: 2026-05-09
