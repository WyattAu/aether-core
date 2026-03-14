# Capability Enforcement Spike

## Objective
Validate capability enforcement overhead <1µs per check.

## Implementation
- Bitflags-based capability representation
- HashMap for subject -> capability mapping
- Inline check functions for fast path

## Results

### Measurement Summary
| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| Standard check | TBD | <1µs | Pending |
| Fast check (bits) | TBD | <100ns | Pending |
| Token validation | TBD | <500ns | Pending |

### Run Instructions
```bash
cargo run --release
```

### Benchmark
```bash
cargo bench
```

## Capability Model

### Capability Flags
```
NETWORK        - Network access
FILE_READ      - File read access
FILE_WRITE     - File write access
PROCESS_SPAWN  - Spawn processes
MEMORY_ALLOC   - Allocate memory
TIME_ACCESS    - Access system time
CRYPTO         - Cryptographic operations
RANDOM         - Random number generation
```

### Enforcement Points
1. **Inline check**: Single bit operation
2. **Token validation**: Hash lookup + expiry check
3. **Batch check**: Multiple capabilities at once

## Findings

### Initial Analysis
- HashMap lookup dominates overhead
- Inline bit operations are fast
- Cache locality matters

### Mitigations
1. **Inline checks**: Force inlining for hot path
2. **Cache optimization**: Keep capability sets in L1
3. **Batch operations**: Check multiple caps in one call
4. **Pre-computation**: Cache common capability combinations

### Architecture Impact
- Capabilities stored inline where possible
- Consider flat array instead of HashMap for small N
- Capability checks should be inlined at call sites

## Conclusion
TBD after benchmark execution
