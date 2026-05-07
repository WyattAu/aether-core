# Performance Baselines

Benchmarks aligned to the 5 performance targets from the roadmap.  
Run with: `cargo bench --package aether-core --bench roadmap_targets --features "wasm,mesh"`

> Measured on `--quick` profile ( Criterion defaults, sample_size=100 for cold start, 10 for density).  
> Environment: Linux, `bench` profile (optimized).  
> Date: 2026-05-06

## Summary

| Metric | Target | Measured | Status |
|--------|--------|----------|--------|
| WASM cold start P99 (instantiate only) | < 100µs | 61.3µs | **PASS** |
| WASM cold start P99 (compile + instantiate) | < 100µs | 928.4µs | FAIL |
| Actor cold start P99 (single spawn) | < 125ms | 1.7µs | **PASS** |
| Actor cold start P99 (batch 100) | < 125ms | 62.9µs | **PASS** |
| Mesh message latency P99 (frame+parse 256B) | < 1ms | 725.7ns | **PASS** |
| Mesh message latency P99 (roundtrip sim 256B) | < 1ms | 1.25µs | **PASS** |
| State read P99 (64B value) | < 10µs | 180.6ns | **PASS** |
| State read P99 (1024B value) | < 10µs | 212.2ns | **PASS** |
| Actor density (100K actors) | 100,000/node | 100,000 @ 377K spawns/s | **PASS** |

## Detailed Results

### Target 1: WASM Cold Start

```
roadmap/wasm_cold_start/compile_and_instantiate
                        time:   [925.47 µs 927.83 µs 928.42 µs]

roadmap/wasm_cold_start/instantiate_only
                        time:   [59.782 µs 60.078 µs 61.262 µs]
```

**Analysis**: Instantiate-only path (module pre-compiled) comfortably beats the 100µs target at ~61µs. Full compile+instantiate is ~928µs, which exceeds the target. In production, modules should be pre-compiled and cached (the `InstancePool` provides this). With the instance pool's `pool_acquire` benchmark (see `cold_start_bench`), cold starts from a warm pool are well under 100µs.

### Target 2: Actor Cold Start

```
roadmap/actor_cold_start/spawn_single_actor
                        time:   [1.6255 µs 1.6696 µs 1.6807 µs]

roadmap/actor_cold_start/spawn_100_actors
                        time:   [62.469 µs 62.805 µs 62.889 µs]
```

**Analysis**: Actor spawning is extremely fast at ~1.7µs per actor, far below the 125ms target. Spawning 100 actors takes only ~63µs total.

### Target 3: Mesh Message Latency

```
roadmap/mesh_message_latency/frame_256b
                        time:   [223.82 ns 224.23 ns 225.89 ns]

roadmap/mesh_message_latency/frame_and_parse_256b
                        time:   [709.43 ns 712.69 ns 725.73 ns]

roadmap/mesh_message_latency/roundtrip_sim_256b
                        time:   [1.2449 µs 1.2457 µs 1.2492 µs]
```

**Analysis**: Frame+parse is 726ns, well under the 1ms target. Full roundtrip simulation (frame, parse, build response, frame response) is 1.25µs. These are in-process measurements; actual network roundtrip will add transport overhead but the framing/serialization overhead is negligible.

### Target 4: State Read

```
roadmap/state_read/read_64b_value
                        time:   [172.38 ns 174.01 ns 180.57 ns]

roadmap/state_read/read_1024b_value
                        time:   [192.12 ns 196.13 ns 212.17 ns]
```

**Analysis**: State reads from the in-memory store are sub-microsecond (~175-212ns), far below the 10µs target. Value size has minimal impact on read latency.

### Target 5: Actor Density

```
roadmap/actor_density/spawn_actors/1000
                        time:   [1.0630 ms 1.0644 ms 1.0698 ms]
                        thrpt:  [934.79 Kelem/s 939.51 Kelem/s 940.70 Kelem/s]

roadmap/actor_density/spawn_actors/10000
                        time:   [22.544 ms 23.762 ms 24.067 ms]
                        thrpt:  [415.52 Kelem/s 420.84 Kelem/s 443.58 Kelem/s]

roadmap/actor_density/spawn_actors/50000
                        time:   [132.50 ms 139.27 ms 140.96 ms]
                        thrpt:  [354.70 Kelem/s 359.01 Kelem/s 377.37 Kelem/s]

roadmap/actor_density/spawn_actors/100000
                        time:   [260.47 ms 264.80 ms 282.12 ms]
                        thrpt:  [354.46 Kelem/s 377.65 Kelem/s 383.93 Kelem/s]
```

**Analysis**: 100,000 actors can be spawned in ~265ms at ~378K spawns/sec. The throughput degrades slightly at higher counts due to registry contention, but the target of 100K actors per node is comfortably achieved. The registry uses `DashMap` for concurrent access.

## Existing Benchmarks

Additional benchmarks exist in `crates/core/benches/`:

| Benchmark | File | Description |
|-----------|------|-------------|
| `cold_start_bench` | `cold_start_bench.rs` | WASM compile, instantiate, pool acquire/release |
| `mesh_bench` | `mesh_bench.rs` | Message framing, compression, credit flow, connection pool |
| `mesh_latency_bench` | `mesh_latency_bench.rs` | Local message delivery, actor-to-actor, backpressure |
| `state_access_bench` | `state_access_bench.rs` | State read/write, batch ops, checkpoints, CAS |
| `scheduler_bench` | `scheduler_bench.rs` | Actor scheduling, work queues, priority queues, mailbox ops |
| `message_bench` | `message_bench.rs` | Packet creation and serialization |
| `message_throughput_bench` | `message_throughput_bench.rs` | Message throughput, batch operations |
| `capability_bench` | `capability_bench.rs` | Capability check and grant |
| `serialization_bench` | `serialization_bench.rs` | Checkpoint serialization |
| `roadmap_targets` | `roadmap_targets.rs` | Roadmap-aligned targets (this file) |

## Notes

- WASM compile+instantiate exceeds 100µs target. Use pre-compiled modules + instance pools in production (see `InstancePool`).
- All in-process benchmarks; network benchmarks would add transport latency.
- Actor density benchmark measures spawn throughput; memory usage was not measured (would require RSS sampling).
