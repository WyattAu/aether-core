# Deterministic Replay Architecture

**Project Aether - Phase 4: Performance Engineering**
**Status**: Draft
**Created**: 2026-05-09

---

## 1. Problem Statement

Distributed actor systems are inherently difficult to debug. When an actor
enters an unexpected state at 03:00 on a Tuesday, reproducing the exact
sequence of interleaved messages, scheduling decisions, and non-deterministic
inputs (wall-clock reads, random bytes, I/O responses) is nearly impossible.
The current aether-core runtime already provides foundational building blocks
for deterministic execution:

- `wasi::HostContext` with `deterministic` mode, injected `wall_time_ns`,
  `monotonic_time_ns`, and entropy pool (`wasi/mod.rs:171-268`)
- `wasi::Clocks` with frozen timestamps in deterministic mode
  (`wasi/clocks.rs:68-179`)
- `wasi::Random` with sequential entropy pool consumption
  (`wasi/random.rs:10-160`)
- `state::CheckpointManager` with blake3-verified snapshots
  (`state/checkpoint.rs:349-418`)
- `actor::migration::MigrationCoordinator` with two-phase state transfer
  (`actor/migration.rs`)

However, these mechanisms operate in isolation. There is no unified event log,
no scheduling decision recording, and no replay engine that can take a
checkpoint and a log of events and verify that execution reproduces identically.
This document specifies the architecture to close that gap.

### Motivation

| Use Case | Description |
|----------|-------------|
| **Post-mortem debugging** | Replay a production execution path in a local dev environment to find the root cause of a failure |
| **Crash recovery** | After a node crash, restore actor state from the last checkpoint and replay only the events that arrived after it |
| **Regression testing** | Record a flaky test's event log, then replay it deterministically in CI until the bug is fixed |
| **Audit & compliance** | Provide a cryptographically-verifiable execution log proving that actors processed messages in a specific order |
| **Chaos engineering validation** | Compare normal execution log vs. chaos-injected execution log to isolate divergence |

---

## 2. Design Goals

| # | Goal | Constraint |
|---|------|-----------|
| G-1 | **Deterministic given same inputs** | Identical WASM module + identical event log + identical initial state produces bit-exact output |
| G-2 | **Minimal overhead in recording mode** | <5% throughput degradation vs. recording disabled |
| G-3 | **Fast replay from arbitrary checkpoints** | Replay speed >= 10x real-time for CPU-bound actors (no I/O waits) |
| G-4 | **Causal ordering preservation** | Events within a single actor are totally ordered; cross-actor events carry causal metadata |
| G-5 | **Non-deterministic source isolation** | All wall-clock, monotonic, random, and external I/O results flow through recorded mediation |
| G-6 | **Pluggable log backends** | Event log is trait-based: in-memory for tests, append-only file for dev, Kafka/FDB for production |
| G-7 | **Zero-cost when disabled** | When replay is not enabled, recording code is compiled out via `#[cfg(feature = "replay")]` |
| G-8 | **Divergence detection** | Replay engine reports the exact event and actor state where replay diverged from the recording |

---

## 3. Architecture

### 3.1 Overview

```text
                          ┌──────────────────────────────┐
                          │       Replay Engine          │
                          │  (reads log, drives executor) │
                          └──────────────┬───────────────┘
                                         │ feeds events
                          ┌──────────────▼───────────────┐
                          │    Deterministic Scheduler   │
                          │  (records/replays schedule)  │
                          └──────────────┬───────────────┘
                                         │ dispatches tasks
              ┌──────────────────────────┼──────────────────────────┐
              │                          │                          │
     ┌────────▼────────┐      ┌─────────▼─────────┐     ┌─────────▼─────────┐
     │   Worker #0     │      │   Worker #N       │     │   ND Source       │
     │  ┌───────────┐  │      │  ┌───────────┐    │     │  Mediation Layer  │
     │  │ WasmActor │  │      │  │ WasmActor │    │     │  ┌─────────────┐  │
     │  │ Executor  │  │      │  │ Executor  │    │     │  │ Clocks      │  │
     │  └─────┬─────┘  │      │  └─────┬─────┘    │     │  │ Random      │  │
     │        │        │      │        │          │     │  │ ExternalIO  │  │
     │  ┌─────▼─────┐  │      │  ┌─────▼─────┐    │     │  └──────┬──────┘  │
     │  │ ActorReg  │  │      │  │ ActorReg  │    │     │         │         │
     │  └───────────┘  │      │  └───────────┘    │     └─────────┼─────────┘
     └────────┬────────┘      └────────┬──────────┘               │
              │                        │                         │
              └────────────────────────┼─────────────────────────┘
                                       │ appends
                          ┌────────────▼───────────────┐
                          │        Event Log            │
                          │  (append-only, ordered by   │
                          │   logical time / Lamport)   │
                          └────────────┬───────────────┘
                                       │ periodic snapshots
                          ┌────────────▼───────────────┐
                          │   Checkpoint Store          │
                          │  (reuses state::Checkpoint   │
                          │   Manager, blake3 verified)  │
                          └──────────────────────────────┘
```

### 3.2 Event Log

The event log is an append-only, totally-ordered sequence of `ReplayEvent`
entries. Each entry carries a **logical timestamp** (Lamport clock) that
establishes a partial order across all actors on a node. For single-node
replay, a single Lamport counter suffices; for multi-node replay, the
existing `tracing::distributed` propagation headers (`tracing/propagation.rs`)
carry the Lamport metadata.

**Trait interface:**

```rust
#[cfg(feature = "replay")]
pub trait EventLog: Send + Sync {
    fn append(&self, event: ReplayEvent) -> Result<()>;
    fn append_batch(&self, events: Vec<ReplayEvent>) -> Result<()>;
    fn read_from(&self, sequence: u64, limit: usize) -> Result<Vec<ReplayEvent>>;
    fn read_range(&self, start: u64, end: u64) -> Result<Vec<ReplayEvent>>;
    fn latest_sequence(&self) -> u64;
    fn truncate(&self, before_sequence: u64) -> Result<()>;
    fn flush(&self) -> Result<()>;
}
```

**Backends:**

| Backend | Use Case | Latency (append) | Retention |
|---------|----------|-----------------|-----------|
| `InMemoryEventLog` | Unit tests, dev | <100ns | process lifetime |
| `AppendFileEventLog` | Local dev, CLI replay | <1µs | configurable rotation |
| `FdbEventLog` | Production (reuses `state::kv::FdbStore`) | <2ms (same DC) | configurable TTL |
| `KafkaEventLog` | Multi-node distributed replay | <5ms | topic retention policy |

### 3.3 Deterministic Scheduler

The current `ActorScheduler` (`actor/scheduler.rs`) uses a work-stealing design
with `crossbeam_deque` (`actor/queue.rs`). Work stealing introduces
non-determinism because the order in which workers steal tasks depends on
thread scheduling.

**Recording mode:** Before dispatching a task to `process_task`, the scheduler
emits a `ScheduleDecision` event recording `(worker_id, actor_id, logical_time)`.

**Replay mode:** The scheduler does not use work stealing. Instead, it reads
the next `ScheduleDecision` from the event log and dispatches the corresponding
task to the recorded worker. The `StealerRegistry` is bypassed entirely.

Integration point: `ActorScheduler::worker_loop` (`actor/scheduler.rs:397-472`)
and `ActorScheduler::process_task` (`actor/scheduler.rs:474-529`) gain
optional replay hooks gated by `#[cfg(feature = "replay")]`.

### 3.4 Non-Deterministic Source Mediation

The existing WASI layer already provides the injection points. The replay
system wraps them to record values during recording and supply recorded values
during replay.

**Mediation points:**

| Source | Existing Type | Mediation Strategy |
|--------|---------------|-------------------|
| Wall clock | `wasi::Clocks::clock_time_get` (`wasi/clocks.rs:112`) | Record/return injected value |
| Monotonic clock | `wasi::Clocks::clock_time_get` (`wasi/clocks.rs:112`) | Record/return injected value |
| Random bytes | `wasi::Random::random_get` (`wasi/random.rs:54`) | Record/return from entropy pool |
| Insecure random | `wasi::Random::random_insecure_get` (`wasi/random.rs:76`) | Record/return from entropy pool |
| PRNG seed | `wasi::Random::random_insecure_seed` (`wasi/random.rs:95`) | Record/return from entropy pool |
| External I/O | `wasi::sockets`, `wasi::http` | Record request/response pair |
| State reads | `wasi::StateHandle::read` (`wasi/mod.rs:309`) | Record key and returned value |

Each mediated call produces a `ReplayEvent` variant (see Section 4) appended
to the event log before returning the value to the actor.

### 3.5 Checkpoint/Restore

The existing `state::CheckpointManager` (`state/checkpoint.rs:349-418`)
already provides:

- Sequence-numbered snapshots with blake3 checksums
- Storage via pluggable `KeyValueStore` backends
- Restore to specific version with integrity verification
- Automatic old-checkpoint cleanup (max 10 per actor)

The replay system extends this with:

- **Checkpoint triggers**: automatic after N events or T wall-time
- **Checkpoint scope**: per-actor state + WasmInstance memory snapshot
- **Checkpoint tagging**: each checkpoint carries the event log sequence
  number at which it was taken, enabling "start replay from checkpoint + events
  after sequence S"

### 3.6 Causal Clock

Each node maintains a **Lamport logical clock** (scalar). For single-node
replay this is sufficient. For future multi-node replay, the clock is
compatible with the existing distributed tracing infrastructure in
`tracing::distributed` (`tracing/distributed.rs`).

```rust
pub struct LamportClock {
    counter: AtomicU64,
}

impl LamportClock {
    pub fn tick(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn merge(&self, other: u64) -> u64 {
        loop {
            let current = self.counter.load(Ordering::Acquire);
            let merged = current.max(other) + 1;
            if self.counter.compare_exchange_weak(
                current, merged, Ordering::Release, Ordering::Relaxed
            ).is_ok() {
                return merged;
            }
        }
    }
}
```

---

## 4. Event Types

```rust
/// Sequence number in the event log.
pub type EventSequence = u64;

/// Logical (Lamport) timestamp for causal ordering.
pub type LogicalTime = u64;

/// Unique identifier for a timer registration.
pub type TimerId = u64;

/// Hash of actor state at checkpoint time (blake3).
pub type StateHash = [u8; 32];

/// Discriminant for replay event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReplayEventType {
    MessageReceived   = 0,
    StateChanged      = 1,
    ActorSpawned      = 2,
    ActorKilled       = 3,
    TimerFired        = 4,
    RandomRequested   = 5,
    ExternalInput     = 6,
    Checkpoint        = 7,
    ScheduleDecision  = 8,
    ClockRead         = 9,
    StateRead         = 10,
}

/// A single entry in the deterministic replay event log.
#[derive(Debug, Clone)]
pub enum ReplayEvent {
    /// Actor received a message from the scheduler.
    MessageReceived {
        actor_id: ActorId,
        sender: Option<ActorId>,
        message: MessagePayload,
        priority: Priority,
        logical_time: LogicalTime,
    },

    /// Actor wrote to its state store.
    StateChanged {
        actor_id: ActorId,
        key: Vec<u8>,
        old_value: Option<Vec<u8>>,
        new_value: Vec<u8>,
        logical_time: LogicalTime,
    },

    /// A new actor was spawned.
    ActorSpawned {
        actor_id: ActorId,
        parent: Option<ActorId>,
        config: ActorSpawnConfig,
        logical_time: LogicalTime,
    },

    /// An actor was killed or stopped.
    ActorKilled {
        actor_id: ActorId,
        reason: KillReason,
        logical_time: LogicalTime,
    },

    /// A scheduled timer fired for an actor.
    TimerFired {
        actor_id: ActorId,
        timer_id: TimerId,
        logical_time: LogicalTime,
    },

    /// Actor requested random bytes (records the result).
    RandomRequested {
        actor_id: ActorId,
        result: Vec<u8>,
        logical_time: LogicalTime,
    },

    /// External (non-actor) input delivered to an actor.
    ExternalInput {
        actor_id: ActorId,
        source: String,
        data: Vec<u8>,
        logical_time: LogicalTime,
    },

    /// Periodic state checkpoint.
    Checkpoint {
        actor_id: ActorId,
        state_hash: StateHash,
        sequence: u64,
        logical_time: LogicalTime,
    },

    /// Scheduler assigned a task to a specific worker.
    ScheduleDecision {
        worker_id: usize,
        actor_id: ActorId,
        logical_time: LogicalTime,
    },

    /// Actor read the wall or monotonic clock.
    ClockRead {
        actor_id: ActorId,
        clock_id: u8,          // 0 = wall, 1 = monotonic
        timestamp_ns: u64,
        logical_time: LogicalTime,
    },

    /// Actor read from its state store.
    StateRead {
        actor_id: ActorId,
        key: Vec<u8>,
        value: Option<Vec<u8>>,
        logical_time: LogicalTime,
    },
}

/// Configuration captured when spawning an actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSpawnConfig {
    pub name: Option<String>,
    pub capabilities: CapabilitySet,
    pub wasm_hash: [u8; 32],
}

/// Reason an actor was killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KillReason {
    Stopped,
    Failed { error_code: u16 },
    Supervised,
    OOM,
    FuelExhausted,
}

/// Wire format for persisted events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEvent {
    pub sequence: EventSequence,
    pub event_type: ReplayEventType,
    pub logical_time: LogicalTime,
    pub timestamp_ns: u64,     // wall clock of recording, for debugging
    pub actor_id: ActorId,
    pub payload: Vec<u8>,      // rkyv-serialized variant-specific data
    pub checksum: [u8; 32],    // blake3 of payload
}
```

### Event Ordering Invariant

Events are assigned a monotonically increasing `EventSequence` by the log.
Within a single actor, events are ordered by `logical_time`. Across actors,
the Lamport clock provides a **partial order**: if event A *happened-before*
event B, then `A.logical_time < B.logical_time`. Concurrent events may have
the same `logical_time` but different `EventSequence` values.

---

## 5. Replay Protocol

### 5.1 Modes of Operation

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  RECORDING   │     │    REPLAY    │     │    LIVE      │
│  (prod/dev)  │     │  (debugging) │     │  (disabled)  │
├──────────────┤     ├──────────────┤     ├──────────────┤
│ Log appends  │     │ Log reads    │     │ No replay    │
│ ND sources   │     │ ND sources   │     │ code at all  │
│   recorded   │     │   replayed   │     │              │
│ Scheduler    │     │ Scheduler    │     │ Standard     │
│   decisions  │     │   enforced   │     │ work steal   │
│ Checkpoints  │     │ Checkpoints  │     │              │
│   periodic   │     │   used as    │     │              │
│              │     │   start pt   │     │              │
└──────────────┘     └──────────────┘     └──────────────┘
```

### 5.2 Recording Flow

```
1. Scheduler selects task for worker
   ├── emit ScheduleDecision { worker_id, actor_id }
   └── advance Lamport clock

2. Worker begins processing message
   └── emit MessageReceived { actor_id, sender, message, priority }

3. WASM execution begins
   ├── Actor calls clock_time_get()
   │   ├── RECORD: emit ClockRead { actor_id, clock_id, timestamp_ns }
   │   └── return real time (or HostContext-injected time)
   ├── Actor calls random_get(n)
   │   ├── RECORD: emit RandomRequested { actor_id, result }
   │   └── return result from CSPRNG (or entropy pool)
   ├── Actor writes state
   │   ├── RECORD: emit StateChanged { actor_id, key, old, new }
   │   └── write to KeyValueStore
   └── Actor reads state
       ├── RECORD: emit StateRead { actor_id, key, value }
       └── return value from KeyValueStore

4. Execution completes
   └── if checkpoint threshold reached:
       └── emit Checkpoint { actor_id, state_hash, sequence }
```

### 5.3 Replay Flow

```
1. Load checkpoint from CheckpointStore (or start from sequence 0)
   ├── Restore actor state from checkpoint data
   ├── Restore WasmInstance memory from checkpoint
   └── Set replay cursor to checkpoint.sequence + 1

2. Read next event from EventLog
   ├── if ScheduleDecision:
   │   └── dispatch recorded task to recorded worker_id
   ├── if MessageReceived:
   │   └── deliver message to actor mailbox
   ├── if ClockRead:
   │   └── inject recorded timestamp into HostContext
   ├── if RandomRequested:
   │   └── feed recorded bytes into entropy pool
   ├── if StateChanged:
   │   └── write new_value to state store, verify against log
   ├── if StateRead:
   │   └── verify actual state matches recorded value
   ├── if TimerFired:
   │   └── deliver timer event to actor
   └── if Checkpoint:
       └── take checkpoint, compare state_hash to recorded

3. Execute WASM with mediated inputs
   └── compare all outputs (state writes, sent messages) against log

4. If mismatch detected:
   ├── Report DivergencePoint { expected_event, actual_event, actor_state }
   └── Halt or continue (configurable)
```

### 5.4 Divergence Detection

```rust
#[derive(Debug, Clone)]
pub struct DivergencePoint {
    pub event_sequence: EventSequence,
    pub actor_id: ActorId,
    pub expected: ReplayEvent,
    pub actual: ReplayEvent,
    pub actor_state_at_divergence: ActorState,
    pub state_snapshot: Option<Vec<u8>>,
}
```

The replay engine can operate in two divergence policies:

| Policy | Behavior |
|--------|----------|
| `HaltOnDiverge` | Stop replay, return `DivergencePoint` |
| `RecordDivergence` | Log divergence but continue replay (useful for comparing two recordings) |

---

## 6. Implementation Strategy

### Phase 1: Event Logging Layer

**Scope:** Trait definitions, in-memory backend, serialization, basic recording.

- Define `EventLog` trait and `ReplayEvent` enum (Section 4)
- Implement `InMemoryEventLog` with `Vec<PersistedEvent>` + sequence counter
- Implement `AppendFileEventLog` using lock-free append with `io_uring`-style
  buffered writes
- Add `#[cfg(feature = "replay")]` gates throughout
- Add `ReplayConfig` to `AetherConfig`
- Wire event log into `Host` (`host.rs`)

**Integration points:**
- `crates/core/src/state/mod.rs` - new `replay` submodule
- `crates/core/src/config.rs` - add `ReplayConfig`
- `crates/core/Cargo.toml` - add `replay` feature flag

### Phase 2: Deterministic Scheduler Mode

**Scope:** Record and replay scheduling decisions.

- Add `ScheduleDecision` recording to `ActorScheduler::process_task`
  (`actor/scheduler.rs:474`)
- Implement `DeterministicScheduler` wrapper that bypasses work stealing
  during replay
- Record task source (priority queue vs. global queue vs. stolen) in events
- Add `ReplayScheduler` that reads `ScheduleDecision` events and dispatches
  to the correct worker

**Integration points:**
- `actor/scheduler.rs` - add conditional recording in `worker_loop` and `process_task`
- `actor/queue.rs` - no changes (replay bypasses queue entirely)

### Phase 3: Non-Deterministic Source Mediation

**Scope:** Wrap all WASI host functions with recording/replay logic.

- Create `MediatedClocks` wrapping `wasi::Clocks` (`wasi/clocks.rs`)
  - Records `ClockRead` events on each `clock_time_get` call
  - In replay mode, returns recorded timestamp instead of live value
- Create `MediatedRandom` wrapping `wasi::Random` (`wasi/random.rs`)
  - Records `RandomRequested` events on each `random_get` call
  - In replay mode, feeds recorded bytes into entropy pool
- Create `MediatedStateHandle` wrapping `wasi::StateHandle` (`wasi/mod.rs:277`)
  - Records `StateChanged` and `StateRead` events
  - In replay mode, verifies state reads match recorded values
- Extend `HostContext` (`wasi/mod.rs:172`) with a `ReplayMode` enum:
  `Live | Recording | Replaying`

**Integration points:**
- `wasi/clocks.rs:112` - wrap `clock_time_get`
- `wasi/random.rs:54` - wrap `random_get`, `random_insecure_get`, `random_insecure_seed`
- `wasi/mod.rs:309-342` - wrap `StateHandle::read`, `write`, `delete`
- `engine/linker.rs` - update linker to use mediated WASI implementations

### Phase 4: Replay Engine with Divergence Detection

**Scope:** The actual replay driver that reads events and drives execution.

- Implement `ReplayEngine` struct:
  ```rust
  pub struct ReplayEngine {
      event_log: Arc<dyn EventLog>,
      checkpoint_store: CheckpointManager<impl KeyValueStore>,
      scheduler: DeterministicScheduler,
      divergence_policy: DivergencePolicy,
  }
  ```
- Implement `ReplayEngine::replay_from_checkpoint(actor_id, sequence)` method
- Implement `ReplayEngine::replay_all()` method (from beginning)
- Implement divergence detection with `DivergencePoint` reporting
- Add replay speed controls (max events/second, real-time pacing)
- Add CLI command `aether replay <log-file> [--from-checkpoint N] [--speed 10x]`
  in `cli/src/commands/`

**Integration points:**
- New `crates/core/src/replay/mod.rs` module
- `cli/src/commands/` - new `replay.rs` command
- `crates/core/src/lib.rs` - expose `pub mod replay`

### Phase 5: WASM Host Integration

**Scope:** Wire mediation into the actual Wasmtime host functions.

- Update `engine::linker::create_linker` (`engine/linker.rs`) to conditionally
  use mediated WASI implementations when replay is active
- Store `ReplayMode` in `InstanceHost` (`engine/linker.rs`) alongside
  `HostContext`
- Ensure `WasmInstance::builder` (`engine/instance.rs:47`) accepts replay
  configuration
- Handle `ExternalInput` events for actors that make HTTP calls or socket
  connections: record the request and response, replay the recorded response
- Verify fuel consumption matches between recording and replay

**Integration points:**
- `engine/instance.rs` - add replay mode to `InstanceBuilder`
- `engine/linker.rs` - conditional mediation in host functions
- `wasi/http.rs` - record/replay HTTP request-response pairs
- `wasi/sockets_tcp.rs`, `wasi/sockets_udp.rs` - record/replay network I/O

---

## 7. Performance Impact Analysis

### 7.1 Event Log Write Path

| Operation | Estimated Cost | Basis |
|-----------|---------------|-------|
| `InMemoryEventLog::append` | ~50-100ns | `Vec::push` + atomic increment |
| `AppendFileEventLog::append` | ~500ns-1µs | buffered write, no fsync per event |
| `FdbEventLog::append` | ~1-2ms | FDB write latency (same DC) |
| `PersistedEvent::serialize` | ~200ns | rkyv serialization of ~100-500 byte payload |
| blake3 checksum | ~50ns | blake3 of small payload |
| Lamport clock tick | ~10ns | `AtomicU64::fetch_add` |

**Recording mode total overhead per event:** ~300ns-1.5µs (in-memory or file backend)

**Estimated throughput impact:** With ~10 events per message (1 schedule + 1
message + 2 clock reads + 2 random calls + 2 state ops + 2 state reads),
recording adds ~3-15µs per message. At a target of <1µs per message
(`performance_requirements.md`), this is significant but acceptable because:

1. Events can be batched (write every N events or every M microseconds)
2. File backend uses buffered I/O; actual flush is amortized
3. Feature flag allows disabling entirely in production if needed
4. The overhead is dominated by serialization, not allocation (rkyv is zero-copy)

**Batched recording** (append every 100µs or 50 events, whichever comes first):

| Scenario | Events/sec | Batched overhead | Throughput impact |
|----------|-----------|------------------|-------------------|
| 10M msg/sec, 10 events/msg | 100M events/sec | ~30µs per batch (50 events) | ~3% |
| 1M msg/sec, 10 events/msg | 10M events/sec | ~5µs per batch (50 events) | <1% |

### 7.2 Checkpoint Cost

| Component | Estimated Cost | Basis |
|-----------|---------------|-------|
| Actor state serialization | ~1-10µs | rkyv of typical actor state (1KB-1MB) |
| WasmInstance memory snapshot | ~10-50µs | memcpy of 64MB worst case; typically much less |
| blake3 hash of state | ~5-20µs | blake3 at ~1GB/s |
| FDB write (1MB checkpoint) | ~2-5ms | FDB write latency |
| Total (in-memory) | ~15-80µs | |
| Total (FDB) | ~2-5ms | |

With checkpoint interval of every 10,000 events or 1 second (whichever comes
first), the amortized cost per event is negligible.

### 7.3 Replay Speed

Replay is expected to be **faster than real-time** for CPU-bound actors:

| Factor | Effect on replay speed |
|--------|----------------------|
| No network I/O waits | Timer events replayed instantly |
| No wall-clock sleeps | Monotonic clock advanced without blocking |
| No real random generation | Entropy served from log |
| No real filesystem I/O | State reads/writes against in-memory store |
| WASM execution | Same cost as recording (fuel-metered) |
| Event deserialization | ~200ns per event (rkyv) |

**Estimated replay speedup:** 10x-100x for typical actor workloads. For
I/O-heavy actors (many `ExternalInput` events), replay speed approaches 1x
since the replay engine still needs to process each event.

### 7.4 Memory Overhead

| Component | Size | Notes |
|-----------|------|-------|
| `InMemoryEventLog` (1M events) | ~200-500MB | Depends on average event size |
| `LamportClock` | 8 bytes | Single `AtomicU64` |
| `ReplayEngine` (per node) | ~1KB | Config + handles |
| Mediated wrappers | ~64 bytes each | One per actor |

---

## 8. Open Questions / Future Work

### Open Questions

1. **Multi-node replay ordering**: Should we use Lamport clocks (simpler,
   single scalar) or vector clocks (captures full causal history) for
   cross-node events? Lamport is sufficient for single-node and likely
   sufficient for multi-node if we replay one node at a time.

2. **External I/O replay boundary**: For HTTP calls to external services,
   recording the full response may be impractical (large payloads). Should
   we record only a content hash and require the test environment to serve
   the same response? This aligns with the existing `wasi::http` abstraction.

3. **Checkpoint granularity**: Should checkpoints be per-actor or
   per-node? Per-actor is simpler but requires replaying all actors from
   their individual checkpoints. Per-node captures the full system state but
   is more expensive.

4. **Event log compaction**: Long-running systems will accumulate large event
   logs. Should we support log compaction (keeping only the latest checkpoint
   + events after it)? This trades storage for replay start-up cost.

5. **Wasmtime epoch-based interruption**: The current `Executor` uses fuel
   metering (`engine/executor.rs:64`). During replay, should fuel limits be
   enforced identically, or relaxed to avoid spurious `FuelExhausted`
   divergence?

### Future Work

- **Time-travel debugger UI**: Integrate with the existing dashboard
  (`dashboard/`) to provide a web UI for stepping through events, inspecting
  actor state at any point, and setting "time breakpoints"
- **Recorded test extraction**: CLI command to extract a test case from a
  production event log: `aether extract-test --actor X --from S --to E`
- **Chaos testing integration**: Compare normal recording vs. chaos-injected
  recording to validate resilience properties automatically
- **Formal verification**: Leverage existing TLA+ proofs
  (`02_architecture/proofs/scheduler_work_stealing.tla`) to formally verify
  that the replay protocol preserves all invariants
- **WASI Preview 2 component model**: As the WASI component model matures,
  ensure mediated host functions comply with the standard component interface
- **Incremental replay**: For large logs, support replaying only the delta
  between two checkpoints, not the entire history
- **Event log streaming**: For multi-node replay, stream events from all
  nodes into a single merged log using the causal clock for ordering
