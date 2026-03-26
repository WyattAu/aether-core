# Benchmark Results

## How Benchmarks Are Run

Benchmarks are executed automatically by CI on two triggers:

- **Nightly**: Scheduled at 3 AM UTC via `sdk-ci.yml` and `benchmarks.yml`
- **On push/PR**: Triggered when changes land on `main`/`develop` or against PRs targeting `main`
- **Manual**: Available via `workflow_dispatch` on the Benchmarks workflow

### Rust Core Benchmarks

Run via `cargo bench --workspace`. Results use Criterion's output format and are uploaded as artifacts.

### Python SDK Benchmarks

Run via `pytest tests/performance/ --benchmark-only` in `sdks/python`. Uses `pytest-benchmark` for timing and comparison.

### JavaScript SDK Benchmarks

Run via `pnpm exec jest tests/performance/ --verbose` in `sdks/javascript`. Uses Jest's built-in performance testing capabilities.

### Local Runs

Use the Makefile targets:

```bash
make sdk-bench-python    # Python SDK benchmarks
make sdk-bench-js        # JavaScript SDK benchmarks
make sdk-bench-all       # All SDK benchmarks
make bench               # Rust core benchmarks
```

## Where Results Are Stored

Benchmark results are uploaded as GitHub Actions artifacts after each run:

| Artifact Name | Contents |
|---|---|
| `benchmark-results` | Rust Criterion output + `benchmark-results.txt` |
| `benchmark-baseline` | Rust baseline for PR comparison (main only) |
| `memory-benchmark` | Valgrind massif output and report |
| `python-sdk-benchmark-results` | Python SDK `benchmark-results.txt` |
| `js-sdk-benchmark-results` | JavaScript SDK `benchmark-results.txt` |

Artifacts are retained for 90 days. Download them from the "Actions" tab under any benchmark workflow run.

## How to Compare Results Between Runs

1. Go to **Actions** -> **Benchmarks** workflow
2. Select two runs you want to compare
3. Download the matching artifact from each run
4. Diff the `benchmark-results.txt` files:

```bash
diff run-a/benchmark-results.txt run-b/benchmark-results.txt
```

For Rust benchmarks, the `benchmark-action/github-action-benchmark` action automatically posts a comparison comment on PRs if regressions exceed 150%.

## Performance Targets (from Roadmap)

| Metric | Target | Status |
|---|---|---|
| Actor spawn latency | < 100us | Measured |
| Message throughput | > 1M msg/s per core | Measured |
| Memory per actor (idle) | < 4 KB | Measured |
| Cold start time | < 50ms | Measured |
| Python SDK call overhead | < 1ms per call | Measured |
| JavaScript SDK call overhead | < 1ms per call | Measured |
| WASM module load time | < 10ms | Measured |
