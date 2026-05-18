# Aether Performance Report

**Date**: March 2026
**Environment**: Linux x86_64, Python 3.14.3, Node v25.7.0

## Executive Summary

All 22 benchmarks (10 Python, 12 JavaScript) passed. The JavaScript SDK outperforms Python in streaming and validation operations by 1.5–12x, driven primarily by V8 JIT optimization of tight loops. However, Python shows lower overhead in resilience primitives (CircuitBreaker, RetryPolicy), making it competitive for async-heavy workloads. Backpressure operations are the star performer in JavaScript at ~18M ops/sec, while Python's StateHandle achieves 2.7M ops/sec for in-memory state access.

## Benchmark Results

### Python SDK

| Operation | Throughput | Unit |
|-----------|-----------|------|
| WindowAssigner | 499,407 | ops/sec |
| TumblingWindow.process | 956,225 | ops/sec |
| BackpressureController.try_push | 1,501,422 | ops/sec |
| MultiLevelBackpressure.push | 1,803,952 | ops/sec |
| CircuitBreaker.execute | 0.82 | us/call |
| CircuitBreaker overhead vs direct | 0.54 | us/call |
| RetryPolicy.execute | 0.90 | us/call |
| StateHandle set/get/delete | 2,706,123 | ops/sec |
| validate_email | 3,389,281 | ops/sec |
| validate_uuid | 1,339,863 | ops/sec |

### JavaScript SDK

| Operation | Throughput | Unit |
|-----------|-----------|------|
| WindowAssigner.assign | 924,394 | ops/sec |
| TumblingWindow.process | 1,943,215 | ops/sec |
| BackpressureController.tryPush | 18,415,738 | ops/sec |
| MultiLevelBackpressure.push | 18,407,941 | ops/sec |
| CircuitBreaker.execute | 0.53 | us/call |
| CircuitBreaker overhead vs direct | 1.13 | us/call |
| RetryPolicy.execute (maxAttempts=1) | 1.12 | us/call |
| validateEmail | 5,103,025 | ops/sec |
| validateUUID | 3,101,737 | ops/sec |
| Message.toJSON | 7,147,518 | ops/sec |
| Message.fromJSON | 12,045,987 | ops/sec |
| Message round-trip | 13,686,519 | ops/sec |

### Cross-SDK Comparison

| Operation | Python | JavaScript | JS/Python Ratio |
|-----------|--------|-----------|-----------------|
| WindowAssigner | 499,407 ops/s | 924,394 ops/s | 1.85x |
| TumblingWindow.process | 956,225 ops/s | 1,943,215 ops/s | 2.03x |
| BackpressureController | 1,501,422 ops/s | 18,415,738 ops/s | 12.26x |
| MultiLevelBackpressure | 1,803,952 ops/s | 18,407,941 ops/s | 10.20x |
| CircuitBreaker.execute | 0.82 us/call | 0.53 us/call | 1.55x faster |
| CircuitBreaker overhead | 0.54 us/call | 1.13 us/call | 0.48x (Python faster) |
| RetryPolicy.execute | 0.90 us/call | 1.12 us/call | 0.80x (Python faster) |
| validateEmail | 3,389,281 ops/s | 5,103,025 ops/s | 1.51x |
| validateUUID | 1,339,863 ops/s | 3,101,737 ops/s | 2.31x |

## Methodology

- Each benchmark runs N iterations (10K or 100K depending on operation)
- Throughput measured in operations per second (ops/sec) or microseconds per call (us/call)
- Results are single-run (not statistically significant)
- Run on development machine, not production hardware
- Python benchmarks use `time.perf_counter()` for timing
- JavaScript benchmarks use `performance.now()` for timing

## Notes

- These are SDK-side benchmarks measuring in-process operations
- Network latency and server overhead not included
- Production performance depends on deployment configuration
- JavaScript Message serialization benchmarks (toJSON/fromJSON/round-trip) have no Python equivalent
- Python StateHandle benchmark (2.7M ops/s) has no JavaScript equivalent
- Backpressure shows the largest cross-SDK gap (12x), likely due to JS engine optimization of queue operations
- Resilience primitives (CircuitBreaker, Retry) are sub-microsecond in both SDKs
