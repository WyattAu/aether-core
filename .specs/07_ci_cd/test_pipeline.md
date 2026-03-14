# Test Pipeline Specification

## Overview

This document defines the comprehensive testing strategy for Project Aether, including unit tests, integration tests, security tests, performance tests, mutation testing, and code coverage.

## Test Architecture

### Test Categories

```
┌─────────────────────────────────────────────────────────┐
│                     Test Suite                           │
├──────────────┬──────────────┬──────────────────────────┤
│  Unit Tests  │ Integration  │   End-to-End Tests       │
│              │    Tests     │                          │
├──────────────┴──────────────┴──────────────────────────┤
│              Security & Performance Tests               │
├─────────────────────────────────────────────────────────┤
│              Mutation & Coverage Analysis               │
└─────────────────────────────────────────────────────────┘
```

### Test Execution Flow

```
Unit Tests → Integration Tests → Security Tests → Performance Tests
     ↓              ↓                  ↓                ↓
Coverage ←────────────────────────────────────────────────
     ↓
Mutation Testing (PR only)
```

## Unit Tests

### Test Framework: cargo-nextest

Nextest provides faster, more reliable test execution with better output.

#### Configuration

```toml
# .nextest.toml
[profile.ci]
retries = 2
failure-output = "immediate-final"
fail-fast = false
test-threads = "num-cpus"
status-level = "pass"

[profile.ci.junit]
path = "junit.xml"
report-name = "aether-tests"
```

#### Execution

```bash
# Standard unit test run
cargo nextest run --all-features --workspace

# CI profile with JUnit output
cargo nextest run --all-features --workspace --profile ci

# Specific test
cargo nextest run test_module_name

# Parallel execution control
cargo nextest run --test-threads=8
```

### Test Organization

```
src/
├── lib.rs
├── module/
│   ├── mod.rs
│   └── mod_test.rs  # Unit tests
├── integration/
│   └── tests.rs     # Integration tests
└── e2e/
    └── scenarios.rs # E2E tests
```

### Test Naming Conventions

```rust
#[test]
fn test_<function>_<scenario>_<expected_result>()

// Examples
#[test]
fn test_spawn_actor_valid_config_returns_handle()

#[test]
fn test_execute_wasm_memory_limit_exceeded_returns_error()

#[test]
fn test_network_send_async_message_delivers_payload()
```

### Test Markers

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_test() { /* ... */ }

    #[test]
    #[ignore = "requires special setup"]
    fn integration_test() { /* ... */ }

    #[test]
    #[cfg(feature = "wasm-runtime")]
    fn wasm_specific_test() { /* ... */ }
}
```

## Integration Tests

### Test Structure

```
tests/
├── integration_tests.rs
├── common/
│   ├── mod.rs
│   └── fixtures/
└── fixtures/
    ├── test_wasm_module.wasm
    └── test_config.toml
```

### Integration Test Categories

#### 1. Runtime Integration

```rust
#[test]
fn test_actor_runtime_integration() {
    let runtime = AetherRuntime::new(Config::default()).unwrap();
    let handle = runtime.spawn_actor("test_actor").unwrap();
    let result = handle.send(Message::Ping).recv();
    assert!(result.is_ok());
}
```

#### 2. WASM Integration

```rust
#[test]
fn test_wasm_module_execution() {
    let engine = WasmEngine::new().unwrap();
    let module = engine.load_module("fixtures/test.wasm").unwrap();
    let result = engine.execute(&module, "main", &[]);
    assert!(result.is_ok());
}
```

#### 3. Network Integration

```rust
#[test]
fn test_distributed_messaging() {
    let node1 = Node::start(Config::default()).unwrap();
    let node2 = Node::start(Config::default()).unwrap();
    
    node1.connect(node2.addr()).unwrap();
    node1.send(node2.id(), Message::Test).unwrap();
    
    let received = node2.recv_timeout(Duration::from_secs(5));
    assert!(received.is_ok());
}
```

### Integration Test Execution

```bash
# Run integration tests only
cargo nextest run --test integration_tests

# With test-threads=1 for isolated execution
cargo nextest run --test integration_tests --test-threads=1

# Run specific integration test
cargo nextest run --test integration_tests test_actor_runtime_integration
```

## Security Tests

### Test Categories

1. **Vulnerability Scanning**
2. **Fuzzing**
3. **Security Boundary Tests**
4. **Penetration Tests**

### Dependency Audit

```bash
# Check for known vulnerabilities
cargo audit

# Fail on any vulnerability
cargo audit --deny warnings

# With advisory database update
cargo audit -D warnings -D unmaintained -D unsound -D yanked
```

### Fuzzing with cargo-fuzz

#### Setup

```bash
cargo install cargo-fuzz
cargo fuzz init
```

#### Fuzz Targets

```rust
// fuzz/fuzz_targets/parse_input.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = aether::parse_input(s);
    }
});
```

#### Execution

```bash
# Run fuzzing for 1 million iterations
cargo fuzz run parse_input -- -max_total_time=3600 -runs=1000000

# With corpus
cargo fuzz run parse_input -- -dict=fuzz/dict.txt corpus/
```

### Security Boundary Tests

```rust
#[test]
fn test_memory_limit_enforcement() {
    let config = Config {
        memory_limit: 1024 * 1024, // 1 MB
        ..Default::default()
    };
    
    let runtime = AetherRuntime::new(config).unwrap();
    let result = runtime.execute_memory_intensive_operation();
    
    assert!(matches!(result, Err(Error::MemoryLimitExceeded)));
}

#[test]
fn test_capability_enforcement() {
    let actor = Actor::new_with_caps(Capabilities::none());
    
    // Should fail without network capability
    let result = actor.network_send("host", Message::Test);
    assert!(matches!(result, Err(Error::CapabilityDenied)));
}

#[test]
fn test_seccomp_filter() {
    let process = spawn_sandboxed_process();
    
    // Attempt forbidden syscall
    let result = process.execute(|| unsafe { libc::fork() });
    assert!(result.is_err());
}
```

### Security Test Execution

```bash
# Run security test suite
cargo test --test security_tests

# Run fuzzing targets
cargo fuzz run parse_input -- -max_total_time=300
```

## Performance Tests

### Benchmark Framework: cargo-criterion

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "actor_spawn"
harness = false
```

### Benchmark Structure

```
benches/
├── actor_spawn.rs
├── wasm_execution.rs
├── network_throughput.rs
└── memory_operations.rs
```

### Benchmark Examples

```rust
// benches/actor_spawn.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_actor_spawn(c: &mut Criterion) {
    let runtime = AetherRuntime::new(Config::default()).unwrap();
    
    c.bench_function("spawn_actor", |b| {
        b.iter(|| {
            black_box(runtime.spawn_actor("test_actor").unwrap())
        })
    });
}

fn bench_message_throughput(c: &mut Criterion) {
    let runtime = AetherRuntime::new(Config::default()).unwrap();
    let actor = runtime.spawn_actor("test_actor").unwrap();
    
    c.bench_function("message_send", |b| {
        b.iter(|| {
            black_box(actor.send(Message::Ping).unwrap())
        })
    });
}

criterion_group!(benches, bench_actor_spawn, bench_message_throughput);
criterion_main!(benches);
```

### Performance Test Execution

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench actor_spawn

# Save baseline for comparison
cargo bench -- --save-baseline main

# Compare with baseline
cargo bench -- --baseline main
```

### Performance Thresholds

| Operation | Target | Regression Threshold |
|-----------|--------|---------------------|
| Actor spawn | < 100 μs | > 10% |
| Message send | < 1 μs | > 5% |
| WASM instantiation | < 5 ms | > 10% |
| Network hop | < 1 ms | > 5% |

### Performance Regression Detection

```yaml
- name: Check Performance Regressions
  run: |
    cargo bench -- --save-baseline ${{ github.sha }}
    cargo bench -- --baseline main --export-json perf-results.json
    
    # Parse and check for regressions
    python scripts/check_performance.py perf-results.json
```

## Mutation Testing

### Tool: cargo-mutants

Mutation testing validates test suite effectiveness.

#### Configuration

```toml
# .cargo/mutants.toml
timeout_multiplier = 3
test_tool = "nextest"
minimum_test_coverage = 85
exclude_globs = ["**/generated/**", "**/tests/**"]
```

#### Execution

```bash
# Run mutation testing
cargo mutants

# With specific options
cargo mutants --in-place --timeout 300 --jobs 4

# Generate report
cargo mutants --output mutants.out
```

#### Mutation Operators

| Operator | Description |
|----------|-------------|
| `replace_arithmetic_op` | +, -, *, / ↔ |
| `replace_comparison_op` | <, >, == ↔ |
| `replace_logical_op` | &&, ||, ! ↔ |
| `replace_assignment` | =, +=, -= ↔ |
| `replace_const` | 0 ↔ 1, true ↔ false |
| `delete_statement` | Remove statement |
| `replace_return` | Replace return value |

### Mutation Score Requirements

| Category | Minimum Score | Target Score |
|----------|---------------|--------------|
| Overall | 85% | 90% |
| Critical modules | 95% | 98% |
| Security modules | 98% | 99% |

## Code Coverage

### Tool: cargo-llvm-cov

#### Configuration

```bash
# Install
cargo install cargo-llvm-cov

# Generate coverage
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
```

#### Coverage Execution

```bash
# Standard coverage
cargo llvm-cov --all-features --workspace

# HTML report
cargo llvm-cov --all-features --workspace --html

# LCOV format for CI
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Summary only
cargo llvm-cov --all-features --workspace --summary-only
```

### Coverage Thresholds

| Metric | Minimum | Target | Critical |
|--------|---------|--------|----------|
| Line coverage | 80% | 85% | 95% |
| Branch coverage | 75% | 80% | 90% |
| Function coverage | 85% | 90% | 95% |
| Region coverage | 75% | 80% | 90% |

### Coverage Exclusions

```rust
// Exclude from coverage
#[cfg(test)]
mod tests { }

#[derive(Debug)]  // Auto-generated code
struct Data { }
```

### Coverage Report

```
Filename                    | Regions | Missed | Coverage | Functions
----------------------------|---------|--------|----------|-----------
src/lib.rs                  |   95.2% |  4.8%  |   95.2%  |   100.0%
src/actor/mod.rs            |   92.1% |  7.9%  |   92.1%  |    96.5%
src/wasm/engine.rs          |   88.3% | 11.7%  |   88.3%  |    94.2%
src/network/mesh.rs         |   85.6% | 14.4%  |   85.6%  |    91.8%
----------------------------|---------|--------|----------|-----------
TOTAL                       |   85.4% | 14.6%  |   85.4%  |    94.8%
```

## Test Environment

### Test Configuration

```toml
# tests/config.toml
[general]
timeout_seconds = 300
parallel_jobs = 8

[integration]
network_timeout_seconds = 10
actor_startup_ms = 100

[performance]
warmup_iterations = 10
measurement_iterations = 100
```

### Test Fixtures

```
tests/fixtures/
├── wasm/
│   ├── minimal.wasm
│   ├── complex.wasm
│   └── memory_test.wasm
├── configs/
│   ├── default.toml
│   ├── high_performance.toml
│   └── secure.toml
└── data/
    ├── test_messages.bin
    └── test_state.bin
```

## Test Reports

### JUnit XML Format

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="aether-tests" tests="245" failures="0" errors="0">
    <testcase name="test_actor_spawn" classname="actor::tests" time="0.002"/>
    <testcase name="test_message_send" classname="actor::tests" time="0.001"/>
  </testsuite>
</testsuites>
```

### HTML Report

Generated by cargo-llvm-cov and cargo-criterion.

### CI Integration

```yaml
- name: Run Tests
  run: cargo nextest run --profile ci

- name: Upload Test Results
  uses: actions/upload-artifact@v4
  with:
    name: test-results
    path: target/nextest/ci/junit.xml

- name: Publish Test Report
  uses: mikepenz/action-junit-report@v4
  with:
    report_paths: target/nextest/ci/junit.xml
```

## Test Best Practices

### 1. Test Independence

```rust
// GOOD: Each test is independent
#[test]
fn test_feature_a() {
    let context = TestContext::new();
    // Test implementation
}

// BAD: Shared mutable state
static mut STATE: i32 = 0;
#[test]
fn test_feature_b() {
    unsafe { STATE += 1; }
}
```

### 2. Deterministic Tests

```rust
// GOOD: Deterministic
#[test]
fn test_deterministic() {
    let result = compute(42);
    assert_eq!(result, 84);
}

// BAD: Non-deterministic
#[test]
fn test_nondeterministic() {
    let delay = rand::thread_rng().gen_range(0..100);
    std::thread::sleep(Duration::from_millis(delay));
}
```

### 3. Clear Assertions

```rust
// GOOD: Clear assertion
assert_eq!(
    result.status,
    Status::Success,
    "Expected success after valid input, got {:?}",
    result.status
);

// BAD: Vague assertion
assert!(result.is_ok());
```

### 4. Proper Cleanup

```rust
#[test]
fn test_with_cleanup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path();
    
    // Test implementation
    
    // Automatic cleanup via Drop
}
```

## Test Metrics

### Collected Metrics

- Test count by category
- Test duration
- Pass/fail rate
- Coverage percentage
- Mutation score
- Flaky test incidents

### Quality Targets

| Metric | Target | Alert Threshold |
|--------|--------|-----------------|
| Test count | > 500 | < 300 |
| Test duration (total) | < 10 min | > 20 min |
| Pass rate | 100% | < 99% |
| Coverage | > 85% | < 80% |
| Mutation score | > 85% | < 80% |
| Flaky tests | 0 | > 2/week |

## Troubleshooting

### Flaky Tests

1. Identify with test retries
2. Add logging
3. Check for race conditions
4. Increase timeouts
5. Isolate shared state

### Slow Tests

1. Profile with `cargo test -- --test-threads=1 --nocapture`
2. Reduce setup/teardown
3. Use fixtures
4. Parallelize independent tests

### Coverage Gaps

1. Review uncovered code
2. Add targeted tests
3. Exclude unreachable code
4. Update coverage configuration

## References

- [cargo-nextest](https://nexte.st/)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [cargo-mutants](https://github.com/sourcefrog/cargo-mutants)
- [Criterion.rs](https://bheisler.github.io/criterion.rs/book/)
- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
