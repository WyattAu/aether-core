# Build Pipeline Specification

## Overview

This document defines the build pipeline for Project Aether, focusing on multi-target compilation, dependency caching, and binary reproducibility.

## Pipeline Architecture

### Stage Dependencies

```
Validate → Build → Test → Package → Deploy
   ↓         ↓
Security → Coverage
```

## Dependency Caching Strategy

### Cargo Chef Integration

Cargo Chef is used for Docker layer caching to optimize build times in containerized environments.

#### Dockerfile Pattern

```dockerfile
FROM rust:1.76 as chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin aether

FROM debian:bookworm-slim as runtime
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/aether /usr/local/bin
ENTRYPOINT ["aether"]
```

### Cache Keys

Primary cache key structure:
```
v1-rust-{os}-{arch}-{rustc-version}-{hash(Cargo.lock)}
```

Secondary fallback:
```
v1-rust-{os}-{arch}-{rustc-version}-
```

### Cache Volumes

- `~/.cargo/registry` - Registry index and cached crates
- `~/.cargo/git` - Git dependencies
- `target/` - Build artifacts (conditional)

## Multi-Target Builds

### Target Matrix

| Target | OS | Architecture | Use Case |
|--------|-----|--------------|----------|
| x86_64-unknown-linux-gnu | Linux | x86_64 | Primary server deployment |
| x86_64-apple-darwin | macOS | x86_64 | Development environments |
| aarch64-apple-darwin | macOS | ARM64 | Apple Silicon development |
| x86_64-pc-windows-msvc | Windows | x86_64 | Windows deployment |
| wasm32-wasip1 | WASI | WASM | WASM runtime components |

### Build Configuration

#### Linux Native Build

```bash
cargo build --release --target x86_64-unknown-linux-gnu --all-features
```

#### WASM Build

```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 --no-default-features --features wasm-runtime
```

#### Cross-Compilation

For targets requiring cross-compilation toolchains:

```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

### Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| default | Standard runtime features | ✓ |
| wasm-runtime | WASM execution engine | ✓ |
| firecracker | Firecracker VM support | ✓ |
| mesh-network | Distributed networking | ✓ |
| all-features | All available features | ✗ |

## Release Builds

### Build Profile

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
debug = false
debug-assertions = false
overflow-checks = false
rpath = false

[profile.release.package."*"]
opt-level = 3
```

### LTO Configuration

Link-Time Optimization (LTO) settings:
- **Mode**: Thin LTO for faster builds with good optimization
- **Codegen Units**: 1 for maximum optimization
- **Target**: ~10% performance improvement, ~20% size reduction

### Binary Stripping

Automated stripping for size optimization:
```bash
strip --strip-unneeded target/release/aether
```

Expected size reduction: 30-40%

## Binary Reproducibility

### Reproducibility Requirements

All release builds must be reproducible byte-for-byte given the same source and environment.

### Techniques

#### 1. Fixed Timestamps

```bash
export SOURCE_DATE_EPOCH=$(git log -1 --pretty=%ct)
```

#### 2. Deterministic Ordering

```toml
# .cargo/config.toml
[build]
rustflags = ["--remap-path-prefix", "${PWD}=/build"]
```

#### 3. Consistent Dependencies

- Lock file committed: `Cargo.lock`
- Dependency versions pinned
- Registry sources verified

### Verification Process

```bash
# Build 1
cargo build --release
sha256sum target/release/aether > build1.sha256

# Clean and rebuild
cargo clean
cargo build --release
sha256sum target/release/aether > build2.sha256

# Verify
diff build1.sha256 build2.sha256
```

### Reproducibility CI Check

```yaml
- name: Verify Reproducibility
  run: |
    cargo build --release
    sha256sum target/release/aether > /tmp/build1.sha256
    
    cargo clean
    cargo build --release
    sha256sum target/release/aether > /tmp/build2.sha256
    
    diff /tmp/build1.sha256 /tmp/build2.sha256
```

## Build Artifacts

### Artifact Structure

```
artifacts/
├── binaries/
│   ├── aether-x86_64-unknown-linux-gnu
│   ├── aether-x86_64-apple-darwin
│   ├── aether-x86_64-pc-windows-msvc.exe
│   └── aether-wasm32-wasip1.wasm
├── docs/
│   └── api/
│       └── index.html
├── sbom/
│   └── sbom.spdx.json
└── checksums.sha256
```

### Artifact Naming Convention

```
aether-{version}-{target}.{ext}
```

Example:
```
aether-0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

### Checksum Generation

```bash
find artifacts/ -type f -exec sha256sum {} \; > checksums.sha256
```

## Build Performance Optimization

### Parallel Compilation

```bash
export CARGO_BUILD_JOBS=$(nproc)
```

### Incremental Builds

Enabled for development, disabled for CI:
```toml
[profile.dev]
incremental = true

[profile.release]
incremental = false
```

### sccache Integration

Shared compilation cache across CI runs:

```bash
export RUSTC_WRAPPER=sccache
export SCCACHE_CACHE_SIZE=10G
export SCCACHE_DIR=/var/cache/sccache
```

### Expected Build Times

| Build Type | Clean Build | Incremental |
|------------|-------------|-------------|
| Debug (dev) | ~3 min | ~30 sec |
| Release | ~8 min | ~2 min |
| Release + LTO | ~12 min | ~4 min |

## Build Environment

### Required Tools

- Rust: stable, beta, nightly (tested)
- Cargo: 1.76+
- rustfmt, clippy
- Platform-specific linkers

### Environment Variables

```bash
RUST_BACKTRACE=1
RUST_LOG=info
CARGO_TERM_COLOR=always
CARGO_INCREMENTAL=0  # CI only
CARGO_NET_RETRY=10
CARGO_NET_GIT_FETCH_WITH_CLI=true
```

### Container Build

Docker-based build for consistency:

```bash
docker build \
  --build-arg RUST_VERSION=1.76 \
  --target runtime \
  -t aether:latest \
  .
```

## Build Verification

### Smoke Tests

Post-build verification:
```bash
./target/release/aether --version
./target/release/aether --help
./target/release/aether doctor
```

### Size Validation

Maximum binary sizes:
- Linux x86_64: < 50 MB
- WASM module: < 10 MB
- Stripped binary: < 30 MB

### Symbol Verification

```bash
nm target/release/aether | grep -i "GLIBC"
objdump -T target/release/aether
```

## Troubleshooting

### Common Issues

1. **Linker Errors**
   - Install missing system dependencies
   - Verify target toolchain

2. **Cache Miss**
   - Invalidate cache key
   - Update cache version

3. **Out of Memory**
   - Reduce parallel jobs
   - Enable swap

4. **Long Build Times**
   - Enable sccache
   - Use cargo-chef
   - Optimize dependencies

### Debug Commands

```bash
# Build with verbose output
cargo build -vv

# Show dependency tree
cargo tree

# Check for duplicate dependencies
cargo tree --duplicates

# Analyze build time
cargo build --timings
```

## Metrics and Monitoring

### Build Metrics Collected

- Build duration
- Binary size
- Dependency count
- Cache hit rate
- LTO effectiveness

### Performance Baselines

| Metric | Target | Alert Threshold |
|--------|--------|-----------------|
| Clean build time | < 10 min | > 15 min |
| Binary size | < 40 MB | > 50 MB |
| Dependency count | < 300 | > 400 |
| Cache hit rate | > 80% | < 60% |

## Security Considerations

### Build Isolation

- Use containers for builds
- No network access during build
- Verify all dependencies

### Artifact Signing

```bash
gpg --armor --detach-sign aether-0.1.0-linux-x86_64.tar.gz
```

### Supply Chain Verification

- Verify dependency signatures
- Check SBOM completeness
- Scan for known vulnerabilities

## References

- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [cargo-chef](https://github.com/LukeMathWalker/cargo-chef)
- [Reproducible Builds](https://reproducible-builds.org/)
- [Rust LTO](https://doc.rust-lang.org/cargo/reference/profiles.html#lto)
