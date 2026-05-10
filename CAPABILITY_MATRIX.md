# Capability Matrix: Project Aether

## 1. Build Toolchain

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Rust nightly | nightly-2026-03-01 | [DONE] | 1.96.0-nightly (38c0de8dc 2026-02-28) | [DONE] Available | Close to target nightly |
| Cargo | 1.85+ | [DONE] | 1.96.0-nightly (f298b8c82 2026-02-24) | [DONE] Available | |
| wasm-tools | 1.220+ | [PENDING] | - | [PENDING] Missing | `cargo install wasm-tools` |
| wit-bindgen | 0.33+ | [PENDING] | - | [PENDING] Missing | `cargo install wit-bindgen-cli` |
| protoc | 28+ | [DONE] | libprotoc 33.5 | [DONE] Available | |

## 2. Runtime Dependencies

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Wasmtime | 25+ | [DONE] | 25.0 (Cargo dep) | [DONE] Available | Built as library dependency |
| Firecracker | 1.10+ | [PENDING] | - | [PENDING] Missing | Download from GitHub releases |
| jailer | bundled | [PENDING] | - | [PENDING] Missing | Bundled with Firecracker |
| FoundationDB | 7.3+ | [PENDING] | - | [PENDING] Missing | System package |

## 3. Platform Features

| Capability | Required | Available | Version/Detail | Status | Notes |
|------------|----------|-----------|----------------|--------|-------|
| KVM access | Yes | [DONE] | /dev/kvm present | [DONE] Available | |
| io_uring | Linux 5.19+ | [DONE] | 7.0.3 (CachyOS) | [DONE] Available | |
| VT-x/AMD-V | Yes | [DONE] | vmx detected | [DONE] Available | Intel VT-x |
| NVMe storage | Recommended | [DONE] | nvme0n1 (1.8T) | [DONE] Available | |

## 4. Development Tools

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| clippy | bundled | [DONE] | 0.1.95 (38c0de8dcb 2026-02-28) | [DONE] Available | |
| rustfmt | bundled | [DONE] | 1.9.0-nightly (38c0de8dcb 2026-02-28) | [DONE] Available | |
| cargo-nextest | 0.9+ | [DONE] | 0.9.129 | [DONE] Available | |
| proptest | 1.6+ | [DONE] | 1.6 (Cargo dep) | [DONE] Available | |

## 5. Formal Verification

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| Lean4 | 4.12+ | [DONE] | 4.29.1 | [DONE] Available | Optional for Phase 0+ |
| Coq | 8.20+ | [PENDING] | - | [PENDING] Missing | Optional for Phase 0+ |
| TLA+ | 1.8+ | [PENDING] | - | [PENDING] Missing | Optional for Phase 1+ |

## 6. Network & Security (Cargo Dependencies)

| Capability | Required | Available | Version | Status | Notes |
|------------|----------|-----------|---------|--------|-------|
| rustls | 0.23+ | [DONE] | 0.23 (Cargo dep) | [DONE] Available | |
| Quinn | 0.11+ | [DONE] | 0.11 (Cargo dep) | [DONE] Available | |
| ring | 0.17+ | [DONE] | 0.17 (Cargo dep) | [DONE] Available | |

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
