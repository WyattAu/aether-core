# Quality Gates Specification

## Overview

This document defines quality gates for Project Aether CI/CD pipeline, including coverage thresholds, performance thresholds, security thresholds, and mutation score requirements.

## Quality Gate Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Quality Gates                         │
├─────────────────────────────────────────────────────────┤
│                                                          │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐          │
│   │ Coverage │   │   Perf   │   │ Security │          │
│   │  Gates   │   │  Gates   │   │  Gates   │          │
│   └──────────┘   └──────────┘   └──────────┘          │
│                                                          │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐          │
│   │ Mutation │   │  Code    │   │  Build   │          │
│   │  Score   │   │ Quality  │   │  Health  │          │
│   └──────────┘   └──────────┘   └──────────┘          │
│                                                          │
│              ┌──────────────────────┐                   │
│              │   Pass/Fail Decision  │                   │
│              └──────────────────────┘                   │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Coverage Gates

### Overall Coverage Thresholds

| Metric | Minimum | Target | Blocker |
|--------|---------|--------|---------|
| Line Coverage | 80% | 85% | < 80% |
| Branch Coverage | 75% | 80% | < 70% |
| Function Coverage | 85% | 90% | < 80% |
| Region Coverage | 75% | 80% | < 70% |

### Critical Module Coverage

Critical modules require higher coverage due to their importance:

| Module | Minimum | Target | Priority |
|--------|---------|--------|----------|
| `actor/runtime` | 95% | 98% | Critical |
| `wasm/engine` | 95% | 98% | Critical |
| `security/capabilities` | 98% | 99% | Critical |
| `network/mesh` | 90% | 95% | High |
| `state/manager` | 90% | 95% | High |
| `resource/pool` | 90% | 95% | High |

### Coverage Regression

Coverage should not decrease between builds:

| Scenario | Allowed Delta | Action |
|----------|---------------|--------|
| Overall coverage | -5% max | Block if exceeded |
| Critical module | -2% max | Block if exceeded |
| New code | 80% minimum | Block if below |

### Coverage Configuration

```yaml
# .github/workflows/coverage.yml
coverage:
  thresholds:
    overall:
      minimum: 80
      target: 85
    critical_modules:
      - path: "src/actor/runtime"
        minimum: 95
        target: 98
      - path: "src/wasm/engine"
        minimum: 95
        target: 98
      - path: "src/security/capabilities"
        minimum: 98
        target: 99
        
  regression:
    max_delta: -5
    critical_max_delta: -2
    new_code_minimum: 80
    
  fail_on_regression: true
  block_deployment: true
```

### Coverage Reporting

```bash
# Generate coverage report
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Parse and check thresholds
./scripts/check-coverage.sh lcov.info
```

## Performance Gates

### Latency Thresholds

| Operation | Target | Maximum | Blocker |
|-----------|--------|---------|---------|
| Actor spawn | 100 μs | 150 μs | > 200 μs |
| Message send | 1 μs | 2 μs | > 5 μs |
| WASM instantiate | 5 ms | 10 ms | > 20 ms |
| Network hop (local) | 1 ms | 2 ms | > 5 ms |
| Network hop (remote) | 10 ms | 20 ms | > 50 ms |

### Throughput Thresholds

| Metric | Target | Minimum | Blocker |
|--------|--------|---------|---------|
| Messages/sec | 100K | 80K | < 50K |
| Actors/sec | 10K | 8K | < 5K |
| WASM calls/sec | 50K | 40K | < 25K |
| Network MB/sec | 1000 | 800 | < 500 |

### Resource Usage Thresholds

| Resource | Target | Maximum | Blocker |
|----------|--------|---------|---------|
| CPU usage | < 70% | < 80% | > 90% |
| Memory usage | < 70% | < 80% | > 90% |
| GC pause time | < 10ms | < 20ms | > 50ms |

### Performance Regression

Performance should not regress beyond thresholds:

| Metric | Regression Limit | Action |
|--------|------------------|--------|
| Latency | +10% | Block |
| Throughput | -10% | Block |
| Critical path | +5% | Block |

### Benchmark Configuration

```yaml
# .github/workflows/benchmark.yml
benchmarks:
  thresholds:
    latency:
      actor_spawn_us: 100
      message_send_us: 1
      wasm_instantiate_ms: 5
      
    throughput:
      messages_per_sec: 100000
      actors_per_sec: 10000
      wasm_calls_per_sec: 50000
      
  regression:
    max_percent_increase: 10
    critical_path_max_increase: 5
    
  baseline_branch: main
  fail_on_regression: true
```

### Benchmark Execution

```bash
# Run benchmarks
cargo bench -- --save-baseline ${{ github.sha }}

# Compare with main
cargo bench -- --baseline main --export-json results.json

# Check for regressions
python scripts/check_benchmarks.py results.json
```

## Security Gates

### Vulnerability Thresholds

| Severity | Allowed | Action |
|----------|---------|--------|
| Critical | 0 | Block immediately |
| High | 0 | Block within 24h |
| Medium | < 3 | Fix within 7 days |
| Low | < 10 | Fix within 30 days |

### Dependency Security

| Check | Threshold | Action |
|-------|-----------|--------|
| CVEs (critical) | 0 | Block |
| CVEs (high) | 0 | Block |
| Unmaintained | 0 | Review |
| Unsound | 0 | Block |
| Yanked | 0 | Block |

### Secret Detection

| Finding | Action |
|---------|--------|
| Verified secret | Block immediately |
| Potential secret | Review required |
| False positive | Allowlist entry |

### CodeQL Alerts

| Severity | Allowed | Action |
|----------|---------|--------|
| Critical | 0 | Block |
| High | 0 | Block |
| Medium | < 5 | Track |
| Low | < 20 | Track |

### Container Scanning

| Check | Threshold | Action |
|-------|-----------|--------|
| CVEs (critical) | 0 | Block |
| CVEs (high) | 0 | Block |
| Misconfigurations | 0 | Block |

### Security Configuration

```yaml
# .github/workflows/security.yml
security:
  vulnerabilities:
    critical: 0
    high: 0
    medium_max: 3
    low_max: 10
    
  dependency_audit:
    block_on_critical: true
    block_on_high: true
    block_on_unmaintained: true
    block_on_unsound: true
    block_on_yanked: true
    
  codeql:
    block_on_critical: true
    block_on_high: true
    medium_max: 5
    
  container_scan:
    block_on_critical: true
    block_on_high: true
    block_on_misconfiguration: true
    
  secret_scan:
    block_on_verified: true
    review_on_potential: true
```

## Mutation Score Gates

### Overall Mutation Score

| Category | Minimum | Target | Blocker |
|----------|---------|--------|---------|
| Overall | 85% | 90% | < 80% |
| Critical modules | 95% | 98% | < 90% |
| Security modules | 98% | 99% | < 95% |

### Mutation Score by Module

| Module | Minimum | Target |
|--------|---------|--------|
| `actor/runtime` | 95% | 98% |
| `wasm/engine` | 95% | 98% |
| `security/capabilities` | 98% | 99% |
| `network/mesh` | 90% | 95% |
| `state/manager` | 90% | 95% |

### Mutation Configuration

```yaml
# .cargo/mutants.toml
[mutation]
minimum_test_coverage = 85
target_test_coverage = 90

[[module_thresholds]]
path = "src/actor/runtime"
minimum = 95
target = 98

[[module_thresholds]]
path = "src/wasm/engine"
minimum = 95
target = 98

[[module_thresholds]]
path = "src/security/capabilities"
minimum = 98
target = 99

[execution]
timeout_multiplier = 3.0
test_tool = "nextest"
jobs = 4
```

### Mutation Execution

```bash
# Run mutation testing
cargo mutants --in-place --timeout 300

# Check score
./scripts/check-mutation-score.sh mutants.out/
```

## Code Quality Gates

### Clippy Lints

| Category | Action |
|----------|--------|
| Errors | Block |
| Warnings | Block |
| Pedantic | Track |

### Code Style

| Check | Action |
|-------|--------|
| rustfmt | Block on deviation |
| taplo (TOML) | Block on deviation |
| Line length | Track if > 100 |

### Documentation

| Check | Threshold | Action |
|-------|-----------|--------|
| Public items documented | 100% | Block |
| Examples in docs | > 80% | Track |
| Doc tests passing | 100% | Block |

### Complexity Metrics

| Metric | Threshold | Action |
|--------|-----------|--------|
| Cognitive complexity | < 25 | Review |
| Function length | < 50 lines | Review |
| File length | < 500 lines | Review |

### Code Quality Configuration

```yaml
# .github/workflows/quality.yml
quality:
  clippy:
    deny_warnings: true
    deny_errors: true
    
  formatting:
    rustfmt_check: true
    taplo_check: true
    
  documentation:
    public_items_required: 100
    doc_tests_required: 100
    
  complexity:
    cognitive_max: 25
    function_length_max: 50
    file_length_max: 500
```

## Build Health Gates

### Build Success

| Check | Action |
|-------|--------|
| Debug build | Block on failure |
| Release build | Block on failure |
| WASM build | Block on failure |

### Binary Size

| Target | Maximum | Action |
|--------|---------|--------|
| Linux x86_64 | 50 MB | Block if exceeded |
| WASM module | 10 MB | Block if exceeded |
| Stripped binary | 30 MB | Block if exceeded |

### Build Time

| Build Type | Target | Maximum | Action |
|------------|--------|---------|--------|
| Clean build | 10 min | 15 min | Alert |
| Incremental | 2 min | 5 min | Alert |
| Release build | 12 min | 20 min | Alert |

### Build Configuration

```yaml
# .github/workflows/build.yml
build:
  targets:
    - x86_64-unknown-linux-gnu
    - wasm32-wasip1
    
  size_limits:
    linux_x86_64_mb: 50
    wasm_mb: 10
    stripped_mb: 30
    
  time_limits:
    clean_build_minutes: 10
    incremental_minutes: 2
    release_minutes: 12
```

## Gate Enforcement

### Pre-merge Gates

Gates that must pass before merging to main:

```yaml
pre_merge_gates:
  - name: Unit Tests
    required: true
    
  - name: Integration Tests
    required: true
    
  - name: Security Audit
    required: true
    
  - name: Code Coverage
    required: true
    minimum: 80
    
  - name: Clippy
    required: true
    
  - name: Formatting
    required: true
```

### Pre-deploy Gates

Gates that must pass before deployment:

```yaml
pre_deploy_gates:
  - name: All Tests Passing
    required: true
    
  - name: Coverage Thresholds
    required: true
    
  - name: Security Scan Clean
    required: true
    
  - name: Performance Benchmarks
    required: true
    
  - name: Mutation Score
    required: false  # PR only
```

### Quality Gate Report

```markdown
# Quality Gate Report

## Overall Status: ✓ PASS

### Coverage
- Overall: 85.4% ✓ (target: 85%)
- Critical Modules: 96.2% ✓ (target: 95%)
- Regression: -1.2% ✓ (max: -5%)

### Performance
- Latency: All within limits ✓
- Throughput: All within limits ✓
- Regression: None detected ✓

### Security
- Critical CVEs: 0 ✓
- High CVEs: 0 ✓
- Medium CVEs: 2 (tracked)
- Secrets: 0 ✓
- CodeQL: 0 alerts ✓

### Mutation Score
- Overall: 87.5% ✓ (minimum: 85%)
- Critical: 96.8% ✓ (minimum: 95%)

### Code Quality
- Clippy: 0 warnings ✓
- Formatting: Pass ✓
- Documentation: 100% ✓

### Build Health
- All builds: Pass ✓
- Binary size: 42 MB ✓ (max: 50 MB)
- Build time: 8 min ✓ (target: 10 min)
```

## Quality Gate Script

```bash
#!/bin/bash
# scripts/quality-gates.sh

set -e

echo "Running quality gates..."

# Coverage gate
COVERAGE=$(cargo llvm-cov --summary-only 2>&1 | grep "TOTAL" | awk '{print $4}' | tr -d '%')
if (( $(echo "$COVERAGE < 80" | bc -l) )); then
    echo "❌ Coverage gate failed: $COVERAGE% < 80%"
    exit 1
fi
echo "✓ Coverage gate passed: $COVERAGE%"

# Security gate
CRITICAL=$(cargo audit 2>&1 | grep "Critical" | wc -l)
if [ "$CRITICAL" -gt 0 ]; then
    echo "❌ Security gate failed: $CRITICAL critical vulnerabilities"
    exit 1
fi
echo "✓ Security gate passed: 0 critical vulnerabilities"

# Performance gate
# ... similar checks

echo "All quality gates passed ✓"
```

## Metrics and Monitoring

### Quality Metrics Dashboard

- Coverage trends over time
- Performance trends over time
- Security vulnerability trends
- Mutation score trends
- Build health trends

### Alerting

```yaml
alerts:
  - name: coverage_below_threshold
    condition: coverage < 80
    severity: critical
    
  - name: performance_regression
    condition: latency_p99 > baseline * 1.1
    severity: warning
    
  - name: security_vulnerability
    condition: critical_cves > 0
    severity: critical
    
  - name: mutation_score_low
    condition: mutation_score < 85
    severity: warning
```

## Continuous Improvement

### Quality Gate Reviews

- Monthly review of thresholds
- Adjust based on team capacity
- Track false positive rates
- Measure impact on velocity

### Quality Metrics

| Metric | Target | Current |
|--------|--------|---------|
| Gate pass rate | > 95% | TBD |
| False positive rate | < 5% | TBD |
| Time to fix failures | < 2 hours | TBD |
| Deployment success rate | > 99% | TBD |

## References

- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [cargo-mutants](https://github.com/sourcefrog/cargo-mutants)
- [cargo-audit](https://github.com/RustSec/rustsec/tree/main/cargo-audit)
- [Criterion.rs](https://bheisler.github.io/criterion.rs/book/)
- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)
