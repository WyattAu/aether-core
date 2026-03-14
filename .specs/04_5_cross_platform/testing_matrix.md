# Testing Matrix

**Document ID:** CP-TEST-001  
**Version:** 1.0.0  
**Status:** Draft  
**Created:** 2026-03-06  
**Author:** Compatibility Engineer  

---

## Overview

This document defines the comprehensive testing matrix for Project Aether, covering OS × Architecture combinations, CI/CD coverage, manual testing requirements, and emulated testing strategies.

---

## OS × Architecture Matrix

### Tier 1: Fully Supported (Production Ready)

| OS | Architecture | Kernel | Priority | Coverage |
|----|--------------|--------|----------|----------|
| Ubuntu 22.04 LTS | x86_64 | Linux 5.15+ | P0 (Critical) | 100% |
| Ubuntu 24.04 LTS | x86_64 | Linux 6.5+ | P0 (Critical) | 100% |
| Debian 12 (Bookworm) | x86_64 | Linux 6.1+ | P1 (High) | 95% |
| RHEL 9 | x86_64 | Linux 5.14+ | P1 (High) | 95% |
| Fedora 39+ | x86_64 | Linux 6.6+ | P1 (High) | 90% |

**Requirements:**
- All unit tests pass
- All integration tests pass
- Performance benchmarks within 10% of baseline
- All documented features functional
- Full CI/CD coverage

### Tier 2: Development/Testing (Partial Support)

| OS | Architecture | Kernel | Priority | Coverage |
|----|--------------|--------|----------|----------|
| macOS 14 (Sonoma) | x86_64 | Darwin 23+ | P2 (Medium) | 60% |
| macOS 14 (Sonoma) | ARM64 | Darwin 23+ | P2 (Medium) | 60% |
| Windows 11 + WSL2 | x86_64 | Linux (WSL) | P2 (Medium) | 70% |
| Arch Linux | x86_64 | Linux 6.x | P2 (Medium) | 80% |

**Requirements:**
- Core functionality tests pass
- Control plane functional
- WASM runtime functional
- Limited performance testing
- Manual testing required

### Tier 3: Experimental (Future Support)

| OS | Architecture | Kernel | Priority | Coverage |
|----|--------------|--------|----------|----------|
| Ubuntu 22.04 LTS | ARM64 | Linux 5.15+ | P3 (Low) | 50% |
| Debian 12 | ARM64 | Linux 6.1+ | P3 (Low) | 50% |
| Alpine Linux | x86_64 | Linux 6.x | P3 (Low) | 40% |
| FreeBSD 14 | x86_64 | FreeBSD 14 | P3 (Low) | 30% |

**Requirements:**
- Basic compilation tests
- Smoke tests only
- Manual testing required
- No performance guarantees

---

## Test Categories

### 1. Unit Tests

**Scope:** Individual functions and modules  
**Coverage Target:** > 90%  
**Execution Time:** < 5 minutes

```bash
# Run all unit tests
cargo test --lib --all-features

# Run with coverage
cargo tarpaulin --out Html --output-dir target/coverage
```

**Categories:**
- Core logic tests
- Error handling tests
- Edge case tests
- Concurrency tests
- Memory safety tests

### 2. Integration Tests

**Scope:** Component interactions  
**Coverage Target:** > 80%  
**Execution Time:** < 30 minutes

```bash
# Run integration tests
cargo test --test '*' --all-features

# Run specific integration test
cargo test --test integration_wasm --all-features
```

**Categories:**
- WASM runtime integration
- KVM/Firecracker integration
- Network mesh integration
- State manager integration
- End-to-end actor lifecycle

### 3. Performance Tests

**Scope:** Performance benchmarks  
**Execution Time:** 1-2 hours

```bash
# Run benchmarks
cargo bench --all-features

# Run with specific criterion
cargo bench -- --save-baseline new
```

**Benchmarks:**
- Cold start latency: < 50µs
- I/O throughput: > 40 Gbps
- Actor invocation: < 10µs
- Memory overhead: < 10 MB per actor
- Network latency: < 100µs P99

### 4. Stress Tests

**Scope:** High-load scenarios  
**Execution Time:** 4-8 hours

**Scenarios:**
- 10,000+ concurrent actors
- 1M+ messages per second
- 100+ VMs running simultaneously
- 24-hour stability test
- Memory leak detection

### 5. Security Tests

**Scope:** Security properties  
**Execution Time:** 1-2 hours

**Categories:**
- Capability enforcement
- Memory isolation
- Escape attack prevention
- Side-channel resistance
- Fuzzing

---

## CI/CD Platform Coverage

### GitHub Actions (Primary)

**Configuration:** `.github/workflows/test.yml`

```yaml
name: Test Matrix

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test-matrix:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-22.04, ubuntu-24.04]
        rust: [nightly-2026-03-01]
        include:
          - os: ubuntu-22.04
            tier: 1
            full-test: true
          - os: ubuntu-24.04
            tier: 1
            full-test: true
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
          components: clippy, rustfmt
      
      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Check formatting
        run: cargo fmt -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings
      
      - name: Run unit tests
        run: cargo test --lib --all-features
      
      - name: Run integration tests
        if: matrix.full-test
        run: cargo test --test '*' --all-features
      
      - name: Run benchmarks
        if: matrix.full-test
        run: cargo bench --no-run
      
      - name: Upload coverage
        if: matrix.tier == 1
        uses: codecov/codecov-action@v3
        with:
          files: ./target/coverage/coverage.json

  test-arm64:
    runs-on: ubuntu-22.04
    continue-on-error: true  # Experimental
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3
        with:
          platforms: arm64
      
      - name: Run tests on ARM64
        run: |
          docker run --rm -v $PWD:/workspace \
            -w /workspace \
            --platform linux/arm64 \
            rust:latest \
            cargo test --lib

  test-macos:
    runs-on: macos-14
    continue-on-error: true  # Tier 2
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: nightly-2026-03-01
      
      - name: Run tests
        run: |
          cargo test --lib
          # Skip integration tests requiring io_uring/KVM

  test-windows:
    runs-on: windows-latest
    continue-on-error: true  # Tier 2
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: nightly-2026-03-01
      
      - name: Enable WSL2
        shell: pwsh
        run: |
          wsl --install -d Ubuntu-22.04
          wsl --shutdown
      
      - name: Run tests in WSL2
        shell: pwsh
        run: |
          wsl -d Ubuntu-22.04 -e bash -c "
            rustup toolchain install nightly-2026-03-01 &&
            rustup default nightly-2026-03-01 &&
            cargo test --lib
          "
```

### CI Pipeline Stages

1. **Lint** (5 min)
   - Format check (rustfmt)
   - Lint check (clippy)
   - Security audit (cargo-audit)

2. **Unit Tests** (10 min)
   - All platforms
   - All features
   - Coverage collection

3. **Integration Tests** (30 min)
   - Tier 1 platforms only
   - Requires KVM access
   - Performance baseline check

4. **Performance Tests** (60 min)
   - Critical benchmarks only
   - Regression detection
   - PGO profile generation

5. **Stress Tests** (4 hours)
   - Nightly builds only
   - Extended duration tests
   - Memory leak detection

6. **Security Tests** (2 hours)
   - Daily builds
   - Fuzzing campaigns
   - Static analysis

---

## Manual Testing Requirements

### Prerequisites

- Access to physical hardware for each target platform
- Understanding of test procedures
- Ability to diagnose and report issues

### Manual Test Scenarios

#### Scenario 1: Full Platform Validation (Tier 1)

**Platforms:**
- Ubuntu 22.04 LTS on bare metal x86_64
- Ubuntu 24.04 LTS on bare metal x86_64

**Duration:** 4 hours

**Steps:**
1. Install Rust toolchain (nightly-2026-03-01)
2. Clone repository and build release
3. Run full test suite
4. Run performance benchmarks
5. Deploy sample application
6. Monitor for 24 hours
7. Document any issues

**Acceptance Criteria:**
- All tests pass
- Performance within 10% of baseline
- No crashes or memory leaks
- Logs show no errors

#### Scenario 2: KVM Functionality (Tier 1)

**Platforms:**
- Ubuntu 22.04 LTS with KVM enabled
- Check Intel VT-x and AMD-V separately

**Duration:** 2 hours

**Steps:**
1. Verify KVM access (`ls -l /dev/kvm`)
2. Create Firecracker VM
3. Run VM lifecycle tests (create, snapshot, restore, destroy)
4. Run 100+ VMs concurrently
5. Verify isolation (no cross-VM access)

**Acceptance Criteria:**
- VM creation < 125ms
- Snapshot/restore < 50ms
- No VM escape vulnerabilities
- All VMs properly isolated

#### Scenario 3: macOS Development Environment (Tier 2)

**Platforms:**
- macOS 14 (Sonoma) on Apple Silicon
- macOS 14 (Sonoma) on Intel

**Duration:** 2 hours

**Steps:**
1. Install dependencies
2. Build project (Tokio-only mode)
3. Run unit tests
4. Run WASM runtime tests
5. Test with Docker Desktop
6. Test with Lima VM

**Acceptance Criteria:**
- Core functionality works
- WASM actors execute correctly
- No KVM/io_uring expected errors
- Performance degradation documented

#### Scenario 4: Windows WSL2 Validation (Tier 2)

**Platforms:**
- Windows 11 with WSL2
- Ubuntu 22.04 in WSL2

**Duration:** 2 hours

**Steps:**
1. Enable WSL2 and install Ubuntu
2. Install Rust toolchain
3. Build project
4. Run tests with KVM (nested)
5. Test networking
6. Test filesystem performance

**Acceptance Criteria:**
- Tests pass with acceptable performance
- KVM works (nested virtualization)
- Network connectivity functional
- Performance degradation documented

#### Scenario 5: ARM64 Experimental (Tier 3)

**Platforms:**
- AWS Graviton3 (c7g instance)
- Ampere Altra (cloud or bare metal)

**Duration:** 4 hours

**Steps:**
1. Provision ARM64 instance
2. Install dependencies
3. Build project
4. Run available tests
5. Run benchmarks
6. Compare with x86_64

**Acceptance Criteria:**
- Compilation succeeds
- Core tests pass
- Performance documented
- Issues identified for future work

---

## Emulated Testing (QEMU)

### QEMU Setup

```bash
# Install QEMU
sudo apt install qemu-system-x86 qemu-system-arm qemu-utils

# Create disk image
qemu-img create -f qcow2 aether-test.qcow2 20G

# Install Ubuntu in QEMU
qemu-system-x86_64 \
  -m 4096 \
  -smp 4 \
  -cdrom ubuntu-22.04-live-server-amd64.iso \
  -drive file=aether-test.qcow2,format=qcow2 \
  -enable-kvm \
  -cpu host \
  -net nic -net user
```

### QEMU Test Matrix

| Architecture | QEMU Command | Use Case | Status |
|--------------|--------------|----------|--------|
| x86_64 | `qemu-system-x86_64 -enable-kvm` | Primary testing | ✅ Supported |
| ARM64 | `qemu-system-aarch64 -M virt -cpu cortex-a57` | ARM64 testing | ⚠️ Slow |
| RISC-V | `qemu-system-riscv64 -M virt` | Future testing | 🔍 Experimental |

### ARM64 Emulation

```bash
# Run ARM64 tests in QEMU
qemu-system-aarch64 \
  -M virt \
  -cpu cortex-a57 \
  -m 4096 \
  -smp 4 \
  -kernel vmlinuz \
  -initrd initrd.img \
  -drive file=rootfs.ext4,if=virtio \
  -append "root=/dev/vda rw console=ttyAMA0" \
  -nographic

# Inside QEMU
cargo test --target aarch64-unknown-linux-gnu
```

### Cross-Architecture Testing

```bash
# Build for ARM64 on x86_64
cargo build --target aarch64-unknown-linux-gnu --release

# Run in QEMU
qemu-aarch64 -L /usr/aarch64-linux-gnu \
  ./target/aarch64-unknown-linux-gnu/release/aether-runtime
```

---

## Performance Baselines

### Tier 1 Platforms

| Metric | Ubuntu 22.04 x86_64 | Ubuntu 24.04 x86_64 | Tolerance |
|--------|---------------------|---------------------|-----------|
| Cold start | 50µs | 45µs | ±10% |
| I/O throughput | 40 Gbps | 45 Gbps | ±10% |
| Actor invocation | 10µs | 8µs | ±10% |
| VM boot time | 125ms | 110ms | ±10% |
| Memory per actor | 8 MB | 7 MB | ±20% |

### Tier 2 Platforms

| Metric | macOS (Tokio) | Windows WSL2 | Degradation |
|--------|---------------|--------------|-------------|
| Cold start | 200µs | 100µs | 4x / 2x |
| I/O throughput | 5 Gbps | 15 Gbps | 8x / 2.7x |
| Actor invocation | 40µs | 15µs | 4x / 1.5x |
| VM boot time | N/A | 200ms | N/A / 1.6x |

### Tier 3 Platforms

| Metric | ARM64 (Experimental) | Notes |
|--------|---------------------|-------|
| Cold start | 60µs | Expected |
| I/O throughput | 30 Gbps | Expected |
| Actor invocation | 12µs | Expected |
| VM boot time | 150ms | Expected |

---

## Test Reporting

### Automated Reports

**Daily Reports:**
- Test pass/fail rates
- Coverage metrics
- Performance regressions
- New issues detected

**Weekly Reports:**
- Trend analysis
- Platform comparison
- Resource usage
- Test stability

**Monthly Reports:**
- Comprehensive analysis
- Tier promotion/demotion
- Resource planning
- Risk assessment

### Report Format

```markdown
# Test Report - YYYY-MM-DD

## Summary
- Overall Status: ✅ PASS
- Total Tests: 1,234
- Passed: 1,200
- Failed: 10
- Skipped: 24

## Platform Breakdown

| Platform | Tests | Pass Rate | Coverage |
|----------|-------|-----------|----------|
| Ubuntu 22.04 x86_64 | 1,234 | 97.2% | 92% |
| Ubuntu 24.04 x86_64 | 1,234 | 97.5% | 93% |
| macOS 14 ARM64 | 740 | 95.4% | 60% |
| Windows WSL2 | 864 | 94.6% | 70% |

## Performance

| Metric | Baseline | Current | Status |
|--------|----------|---------|--------|
| Cold start | 50µs | 48µs | ✅ |
| Throughput | 40 Gbps | 42 Gbps | ✅ |
| VM boot | 125ms | 122ms | ✅ |

## Issues
- #123: Intermittent failure on ARM64
- #124: Performance regression on macOS

## Recommendations
- Investigate ARM64 test failures
- Optimize macOS I/O path
```

---

## Test Environment Setup

### Hardware Requirements

**Tier 1 Testing:**
- CPU: 8+ cores (Intel/AMD with VT-x/AMD-V)
- RAM: 32 GB
- Storage: 500 GB SSD
- Network: 10 Gbps

**Tier 2 Testing:**
- CPU: 4+ cores
- RAM: 16 GB
- Storage: 200 GB SSD
- Network: 1 Gbps

**Tier 3 Testing:**
- CPU: 2+ cores
- RAM: 8 GB
- Storage: 100 GB
- Network: Basic

### Software Requirements

**All Platforms:**
- Rust nightly-2026-03-01
- Git
- Docker (for containerized tests)

**Linux-specific:**
- KVM enabled
- io_uring support
- cgroups v2

**macOS-specific:**
- Xcode Command Line Tools
- Docker Desktop or Lima

**Windows-specific:**
- WSL2 enabled
- Docker Desktop

---

## Troubleshooting Test Failures

### Common Issues

#### 1. KVM Permission Denied

```bash
# Solution: Add user to kvm group
sudo usermod -aG kvm $USER
# Log out and back in
```

#### 2. io_uring Not Available

```bash
# Solution: Check kernel version
uname -r  # Should be 5.1+

# Check io_uring support
grep CONFIG_IO_URING /boot/config-$(uname -r)
```

#### 3. Test Timeout

```bash
# Solution: Increase timeout
cargo test -- --test-threads=1
# Or set environment variable
export RUST_TEST_THREADS=1
```

#### 4. Out of Memory

```bash
# Solution: Reduce parallelism
cargo test -- --test-threads=2

# Or increase system memory
```

#### 5. Network Test Failures

```bash
# Solution: Check firewall
sudo ufw allow 8080:9000/tcp

# Check network namespace
ip netns list
```

---

## Conclusion

The testing matrix ensures comprehensive coverage across:
- **Tier 1:** Production-ready platforms with 95-100% coverage
- **Tier 2:** Development platforms with 60-70% coverage
- **Tier 3:** Experimental platforms with 30-50% coverage

Automated CI/CD handles routine testing, while manual testing validates complex scenarios and platform-specific features. QEMU enables testing on architectures without physical hardware.

**Next Steps:**
1. Set up CI/CD pipelines
2. Provision test hardware
3. Document manual test procedures
4. Train team on testing protocols
5. Establish baseline metrics
