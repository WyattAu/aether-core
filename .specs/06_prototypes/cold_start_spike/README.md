# WASM Cold Start Spike

## Objective
Validate that WASM module cold start can achieve <50µs latency.

## Implementation
- Minimal WAT module with single exported function
- Wasmtime 15.0 with Cranelift backend
- Benchmark cold compilation, warm invocation, instance pooling

## Results

### Measurement Summary
| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| Cold start (compilation) | TBD | <50µs | Pending |
| Warm invocation | TBD | <1µs | Pending |
| Instance creation | TBD | <10µs | Pending |

### Run Instructions
```bash
cargo run --release
```

### Benchmark
```bash
cargo bench
```

## Findings

### Initial Analysis
Module compilation dominates cold start time. Pure instantiation is fast enough.

### Mitigations
1. **Pre-compilation**: Compile modules at deploy time, cache artifacts
2. **Module Pooling**: Maintain pool of pre-instantiated modules
3. **Wasmtime Cache**: Enable on-disk module cache
4. **Pooling Allocator**: Use pooling allocator for fast memory acquisition

### Architecture Impact
- Module cache required in production
- Pool size should match expected concurrency
- Consider warm-pooling strategy for hot paths

## Conclusion
TBD after benchmark execution
