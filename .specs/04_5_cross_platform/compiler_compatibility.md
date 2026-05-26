# Compiler Compatibility

**Document ID:** CP-COMPILER-001  
**Version:** 1.0.0  
**Status:** Draft  
**Created:** 2026-03-06  
**Author:** Compatibility Engineer  

---

## Overview

Project Aether requires specific Rust compiler features and LLVM capabilities for optimal performance and safety. This document specifies compiler requirements, version constraints, and cross-compilation support.

---

## Rust Compiler Requirements

### Version Specification

**Required:** `nightly-2026-03-01`  
**Channel:** Nightly  
**Justification:** Access to latest WASM and async features

### Installation

```bash
# Install specific nightly version
rustup toolchain install nightly-2026-03-01

# Set as default for project
rustup default nightly-2026-03-01

# Or use rust-toolchain.toml (recommended)
# Project already has this file configured
```

### rust-toolchain.toml Configuration

```toml
[toolchain]
channel = "nightly-2026-03-01"
components = ["rustc", "cargo", "rust-src", "clippy", "rustfmt"]
targets = ["wasm32-wasip1", "x86_64-unknown-linux-gnu"]
```

### Required Components

| Component | Purpose | Required |
|-----------|---------|----------|
| `rustc` | Rust compiler | [DONE] Yes |
| `cargo` | Package manager | [DONE] Yes |
| `rust-src` | Rust source code | [DONE] Yes (for WASM) |
| `clippy` | Linter | [DONE] Yes |
| `rustfmt` | Code formatter | [DONE] Yes |
| `rust-analyzer` | IDE support | [WARN] Recommended |
| `llvm-tools-preview` | LLVM tools | [WARN] Optional |

---

## Required Nightly Features

Project Aether leverages several unstable/nightly-only features:

### 1. WASM Component Model

```rust
#![feature(wasm_component_model)]

// Enables WASM component model support
// Required for: wasm32-wasip1 target
```

### 2. Async Iterator

```rust
#![feature(async_iterator)]

// Required for async streams in data plane
use async_iterator::AsyncIterator;
```

### 3. Generic Associated Types (GATs)

```rust
#![feature(generic_associated_types)]

// Required for advanced type-level programming
// Used in capability system and HAL abstractions
```

### 4. Type Alias Impl Trait

```rust
#![feature(type_alias_impl_trait)]

// Enables RPIT in trait definitions
// Used extensively in async runtimes
```

### 5. Min Specialization

```rust
#![feature(min_specialization)]

// Limited specialization for performance optimizations
// Used in serialization and zero-copy abstractions
```

### 6. Extract Ref

```rust
#![feature(extract_ref)]

// Required for safe extraction from data structures
// Used in resource management
```

### 7. Maybe Uninit Slice

```rust
#![feature(maybe_uninit_slice)]

// Required for zero-copy I/O operations
// Used in io_uring integration
```

### 8. Io Slice Advise

```rust
#![feature(io_slice_advise)]

// Provides hints for I/O operations
// Used for zero-copy optimization
```

### Complete Feature List

```rust
// lib.rs
#![feature(
    wasm_component_model,      // WASM components
    async_iterator,            // Async streams
    generic_associated_types,  // GATs
    type_alias_impl_trait,     // RPIT in traits
    min_specialization,        // Limited specialization
    extract_ref,               // Safe extraction
    maybe_uninit_slice,        // Zero-copy buffers
    io_slice_advise,           // I/O hints
    pointer_is_aligned,        // Alignment checks
    nonzero_ops,               // Non-zero optimizations
    slice_ptr_get,             // Slice pointer access
    const_fn_trait_bound,      // Const generics
)]
```

---

## LLVM Version Compatibility

### LLVM Backend

Rust nightly-2026-03-01 uses **LLVM 19.x** as the backend.

### Required LLVM Features

| Feature | LLVM Version | Purpose |
|---------|--------------|---------|
| WebAssembly backend | 8.0+ | WASM compilation |
| SIMD support | 11.0+ | Vectorization |
| PGO (Profile-Guided Optimization) | 9.0+ | Performance optimization |
| LTO (Link-Time Optimization) | 9.0+ | Binary optimization |
| BOLT (Binary Optimization) | 14.0+ | Post-link optimization |

### LLVM Targets

Required LLVM targets for cross-compilation:

```bash
# Install LLVM with required targets
# Ubuntu/Debian
sudo apt install llvm-19 clang-19 lld-19

# Verify targets
llc --version | grep -E "x86-64|wasm32|aarch64"
```

| Target | Triple | Purpose |
|--------|--------|---------|
| x86-64 | `x86_64-unknown-linux-gnu` | Primary platform |
| WASM | `wasm32-wasip1` | WASM actors |
| ARM64 | `aarch64-unknown-linux-gnu` | Future support |
| RISC-V | `riscv64gc-unknown-linux-gnu` | Future support |

---

## Cross-Compilation Support

### Building for Different Targets

#### WASM Target

```bash
# Add WASM target
rustup target add wasm32-wasip1 --toolchain nightly-2026-03-01

# Build WASM actor
cargo build --target wasm32-wasip1 --release

# The output will be in:
# target/wasm32-wasip1/release/your_actor.wasm
```

#### ARM64 Target

```bash
# Add ARM64 target
rustup target add aarch64-unknown-linux-gnu --toolchain nightly-2026-03-01

# Install ARM64 linker
sudo apt install gcc-aarch64-linux-gnu

# Configure linker in .cargo/config.toml
cat > .cargo/config.toml <<EOF
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
EOF

# Build for ARM64
cargo build --target aarch64-unknown-linux-gnu --release
```

#### Cross-Compilation with Cross

For easier cross-compilation, use `cross`:

```bash
# Install cross
cargo install cross

# Build for any target with Docker
cross build --target aarch64-unknown-linux-gnu --release
cross build --target riscv64gc-unknown-linux-gnu --release
```

### Cross-Compilation Matrix

| Host | Target | Method | Status |
|------|--------|--------|--------|
| x86_64 Linux | wasm32-wasip1 | Native | [DONE] Supported |
| x86_64 Linux | aarch64-linux | Cross | [DONE] Supported |
| x86_64 Linux | riscv64-linux | Cross |  Experimental |
| x86_64 macOS | x86_64-linux | Cross | [WARN] Limited |
| ARM64 macOS | aarch64-linux | Cross | [WARN] Limited |

---

## WASM Target Details

### wasm32-wasip1 Target

The `wasm32-wasip1` target provides WASI Preview 1 support for WASM actors.

#### Features

- **WASI Preview 1:** System interface for WASM
- **Component Model:** For actor composition
- **Memory64:** Optional 64-bit memory (future)
- **SIMD:** 128-bit vector operations
- **Threads:** Shared memory (planned)

#### Compilation Flags

```toml
# Cargo.toml for WASM actors
[package]
name = "actor-example"
version = "0.1.0"

[lib]
crate-type = ["cdylib"]  # Produce .wasm file

[dependencies]
wit-bindgen = { version = "0.16", features = ["realloc"] }

[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
panic = "abort"      # Smaller binary
strip = true         # Remove symbols
```

#### WASM Optimization

```bash
# Install wasm-opt
cargo install wasm-opt

# Optimize WASM binary
wasm-opt -Oz -o actor_optimized.wasm actor.wasm

# Typical size reduction: 30-50%
```

---

## Optimization Levels

### Development Profile

```toml
[profile.dev]
opt-level = 0        # No optimization
debug = true         # Full debug info
debug-assertions = true
overflow-checks = true
lto = false
codegen-units = 256  # Fast compilation
```

### Release Profile

```toml
[profile.release]
opt-level = 3        # Maximum optimization
debug = false        # No debug info
debug-assertions = false
overflow-checks = false
lto = "fat"          # Full LTO
codegen-units = 1    # Best optimization
panic = "abort"      # No unwinding
strip = true         # Remove symbols
```

### Performance Profile (Custom)

```toml
[profile.perf]
inherits = "release"
opt-level = 3
lto = "fat"
codegen-units = 1
debug = true         # Keep debug for profiling
debug-assertions = false
```

### Size Profile (Custom)

```toml
[profile.min-size]
inherits = "release"
opt-level = "z"      # Optimize for size
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

---

## Profile-Guided Optimization (PGO)

### Overview

PGO improves performance by profiling real workloads and feeding that data back to the compiler.

### PGO Workflow

```bash
# 1. Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" \
  cargo build --release --target x86_64-unknown-linux-gnu

# 2. Run representative workload
./target/release/aether-runtime --test-workload

# 3. Merge profile data
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data/*.profraw

# 4. Build optimized binary with PGO
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" \
  cargo build --release --target x86_64-unknown-linux-gnu
```

### Expected Improvement

| Metric | Without PGO | With PGO | Improvement |
|--------|-------------|----------|-------------|
| Cold start | 50µs | 40µs | 20% |
| Throughput | 40 Gbps | 48 Gbps | 20% |
| Latency P99 | 100µs | 75µs | 25% |

---

## Link-Time Optimization (LTO)

### LTO Modes

| Mode | Description | Build Time | Performance | Binary Size |
|------|-------------|------------|-------------|-------------|
| `none` | No LTO | Fast | Baseline | Large |
| `thin` | Thin LTO | Medium | +5-10% | Medium |
| `fat` | Full LTO | Slow | +10-15% | Small |

### Configuration

```toml
# Cargo.toml
[profile.release]
lto = "fat"  # Use full LTO for maximum performance

# For faster builds during development
[profile.dev]
lto = false

# For CI/release builds
[profile.release]
lto = "fat"
```

### LTO with Cross-Compilation

```bash
# Ensure LLD linker is used for LTO
RUSTFLAGS="-Clinker-plugin-lto -Clinker=clang-19" \
  cargo build --release --target x86_64-unknown-linux-gnu
```

---

## Build Configuration

### .cargo/config.toml

```toml
# Build configuration
[build]
# Use all CPU cores
jobs = 8

# Target-specific settings
[target.x86_64-unknown-linux-gnu]
linker = "clang-19"
rustflags = [
  "-C", "link-arg=-fuse-ld=lld-19",  # Use LLD linker
  "-C", "target-cpu=native",         # Use native CPU features
  "-C", "link-arg=-Wl,--no-rosegment",  # Allow modifying read-only segments
]

[target.wasm32-wasip1]
rustflags = [
  "-C", "target-feature=+simd128,+bulk-memory,+sign-ext",  # Enable WASM features
]

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"

# Environment variables
[env]
# Set during build
RUST_LOG = "info"
RUST_BACKTRACE = "1"

# Future: RISC-V support
[target.riscv64gc-unknown-linux-gnu]
linker = "riscv64-linux-gnu-gcc"
```

### Environment Variables

```bash
# Build optimizations
export RUSTFLAGS="-C target-cpu=native -C opt-level=3"

# Linker
export RUSTFLAGS="-C linker=clang-19 -C link-arg=-fuse-ld=lld-19"

# Debug info (for profiling)
export RUSTFLAGS="-C debuginfo=2"

# Codegen options
export RUSTFLAGS="-C codegen-units=1 -C lto=fat"

# Target CPU
export RUSTFLAGS="-C target-cpu=skylake-avx512"

# Combine all
export RUSTFLAGS="-C target-cpu=native -C linker=clang-19 -C link-arg=-fuse-ld=lld-19 -C codegen-units=1 -C lto=fat"
```

---

## Compiler Checks

### Clippy Lints

```toml
# .clippy.toml
msrv = "1.88.0"

# Cargo.toml
[lints.clippy]
# Performance lints
inefficient_to_string = "warn"
large_types_passed_by_value = "warn"
linkedlist = "warn"
unnecessary_box_returns = "warn"

# Correctness lints
unwrap_used = "deny"  # Important for panic=abort
expect_used = "deny"
panic = "deny"

# Style lints
all = "warn"
pedantic = "warn"
nursery = "allow"
```

### Running Clippy

```bash
# Run all lints
cargo clippy --all-targets --all-features -- -D warnings

# Run specific lints
cargo clippy -- -W clippy::perf -W clippy::correctness
```

---

## Debugging Support

### Debug Info Levels

| Level | Size | Info | Use Case |
|-------|------|------|----------|
| 0 | Smallest | None | Production |
| 1 | Small | Line tables | Stack traces |
| 2 | Medium | Full debug | Development |
| 3 | Large | Full + macro expansion | Debugging |

### Configuration

```toml
# Development: Full debug info
[profile.dev]
debug = 2

# Release: Minimal debug info for stack traces
[profile.release]
debug = 1  # Line tables only

# Profiling: Full debug for profiling tools
[profile.perf]
debug = 2
```

### Debugging Tools

```bash
# Install debugging tools
rustup component add llvm-tools-preview

# Install cargo-binutils
cargo install cargo-binutils

# View assembly
cargo asm --release your_function

# View LLVM IR
cargo rustc --release -- --emit=llvm-ir

# View optimized LLVM IR
cargo rustc --release -- --emit=llvm-ir=optimized.ll
```

---

## Version Compatibility Matrix

| Component | Minimum Version | Recommended | Notes |
|-----------|-----------------|-------------|-------|
| Rust | nightly-2026-03-01 | nightly-2026-03-01 | Pinned version |
| LLVM | 19.0 | 19.0 | Rust backend |
| Clang | 19.0 | 19.0 | For LTO |
| LLD | 19.0 | 19.0 | Fast linker |
| Cargo | 1.77+ | Latest | Package manager |
| Rustup | 1.26+ | Latest | Toolchain manager |

---

## Troubleshooting

### Error: "feature is unstable"

```bash
# Ensure nightly toolchain
rustup default nightly-2026-03-01

# Check current toolchain
rustup show
```

### Error: "linker 'cc' not found"

```bash
# Install build essentials
sudo apt install build-essential  # Ubuntu/Debian
sudo dnf install gcc clang        # Fedora/RHEL
```

### Error: "wasm32-wasip1 target not found"

```bash
# Add WASM target
rustup target add wasm32-wasip1 --toolchain nightly-2026-03-01
```

### Error: "LLVM version mismatch"

```bash
# Check LLVM version
llc --version

# Install correct LLVM version
# Ubuntu/Debian
wget https://apt.llvm.org/llvm.sh
chmod +x llvm.sh
sudo ./llvm.sh 19
```

### Slow Compilation

```bash
# Use faster linker
sudo apt install lld-19
export RUSTFLAGS="-C link-arg=-fuse-ld=lld-19"

# Increase parallel jobs
# .cargo/config.toml
[build]
jobs = 16  # Or number of CPU cores
```

---

## Best Practices

1. **Pin Toolchain:** Always use `rust-toolchain.toml` with exact version
2. **Enable LTO:** Use `lto = "fat"` for release builds
3. **Profile-Guided:** Use PGO for production builds (10-20% improvement)
4. **Size Optimization:** Use `opt-level = "z"` for WASM actors
5. **Lint Strictly:** Enable all clippy lints and treat warnings as errors
6. **Test Thoroughly:** Test on all target platforms before release
7. **Document Features:** Keep track of required nightly features

---

## Conclusion

Project Aether requires:
- **Rust nightly-2026-03-01** with specific unstable features
- **LLVM 19.x** for optimal code generation
- **LTO and PGO** for production builds
- **wasm32-wasip1** target for WASM actors

The compiler configuration is critical for achieving performance targets and maintaining safety guarantees. All builds should use the pinned toolchain version specified in `rust-toolchain.toml`.
