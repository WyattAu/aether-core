# Deterministic Replay Support

## Overview

Aether provides deterministic execution for time-travel debugging and replay. All time and randomness values are injected by the host, ensuring reproducible execution across multiple runs.

## Architecture

### HostContext

The `HostContext` is the central mechanism for injecting deterministic values:

```rust
use aether_core::{HostContext, CapabilitySet};

// Create a deterministic context for replay
let ctx = HostContext::deterministic()
    .with_wall_time(1_234_567_890_000_000_000)  // Unix timestamp in nanoseconds
    .with_monotonic_time(1_000_000)              // Monotonic time in nanoseconds
    .with_entropy(vec![1, 2, 3, 4, 5]);          // Entropy pool for randomness

// Create interfaces with capability checks
let clocks = ctx.create_clocks(CapabilitySet::TIME);
let random = ctx.create_random(CapabilitySet::RANDOM);
```

### Clocks API

The clocks API implements WASI Preview 2 clock interfaces:

```rust
use aether_core::wasi::{Clocks, ClockId, CapabilitySet};

let clocks = Clocks::new(
    CapabilitySet::TIME,
    wall_time_ns: 1_000_000_000,
    monotonic_time_ns: 500_000,
    deterministic: true,
);

// Get wall clock time
let wall = clocks.clock_wall()?;
assert_eq!(wall.to_nanos(), 1_000_000_000);

// Get monotonic clock time
let mono = clocks.clock_monotonic()?;
assert_eq!(mono.to_nanos(), 500_000);

// Get clock resolution
let res = clocks.clock_res_get(ClockId::WallClock)?;
// In deterministic mode: 1 second for wall clock, 1ms for monotonic
```

### Random API

The random API implements WASI Preview 2 random interfaces:

```rust
use aether_core::wasi::{Random, CapabilitySet};

let mut random = Random::new(
    CapabilitySet::RANDOM,
    vec![1, 2, 3, 4, 5, 6, 7, 8],
    deterministic: true,
);

// Get random bytes (wraps around entropy pool)
let bytes = random.random_get(4)?;
assert_eq!(bytes, vec![1, 2, 3, 4]);

let more = random.random_get(4)?;
assert_eq!(more, vec![5, 6, 7, 8]);

// Get insecure random (same pool in deterministic mode)
let insecure = random.random_insecure_get(4)?;

// Get PRNG seed
let seed = random.random_insecure_seed()?;
```

## Time-Travel Debugging

### Recording Execution

To enable time-travel debugging, capture the HostContext during execution:

```rust
// During recording
let ctx = host.get_context();
let recording = RecordedContext {
    wall_time_ns: ctx.wall_time_ns,
    monotonic_time_ns: ctx.monotonic_time_ns,
    entropy: ctx.entropy.clone(),
};

// Save recording
save_recording("execution_001.json", &recording)?;
```

### Replaying Execution

To replay a recorded execution:

```rust
// Load recording
let recording = load_recording("execution_001.json")?;

// Create deterministic context
let ctx = HostContext::deterministic()
    .with_wall_time(recording.wall_time_ns)
    .with_monotonic_time(recording.monotonic_time_ns)
    .with_entropy(recording.entropy);

// Re-run with exact same time/randomness
let result = actor.run_with_context(ctx)?;
```

### Stepping Through Time

You can manipulate time for debugging:

```rust
let mut clocks = ctx.create_clocks(CapabilitySet::TIME);

// Get current time
let t1 = clocks.clock_wall()?.to_nanos();

// Advance time by 1 second
clocks.set_wall_time(t1 + 1_000_000_000);

// Now wall clock shows 1 second later
let t2 = clocks.clock_wall()?.to_nanos();
assert_eq!(t2 - t1, 1_000_000_000);
```

## Entropy Management

### Entropy Pool

In deterministic mode, the entropy pool is used sequentially and wraps around:

```rust
let mut random = Random::new(
    CapabilitySet::RANDOM,
    vec![1, 2, 3],  // Small pool
    deterministic: true,
);

// Uses pool: [1, 2, 3]
random.random_get(3)?;

// Wraps around: [1, 2, 3, 1, 2, 3, 1]
random.random_get(7)?;
```

### Resetting Entropy Position

For precise replay control:

```rust
let mut random = ctx.create_random(CapabilitySet::RANDOM);

// Use some entropy
random.random_get(5)?;

// Reset to beginning of pool
random.reset_position();

// Next call starts from beginning again
random.random_get(5)?;  // Same bytes as before
```

### Updating Entropy Pool

For replaying specific scenarios:

```rust
let mut random = ctx.create_random(CapabilitySet::RANDOM);

// Use original entropy
random.random_get(10)?;

// Switch to different entropy for different scenario
random.set_entropy(vec![10, 20, 30, 40]);

// Now uses new entropy
random.random_get(4)?;  // [10, 20, 30, 40]
```

## Capability Checks

All operations require appropriate capabilities:

```rust
// Without TIME capability
let clocks = Clocks::new(CapabilitySet::empty(), 0, 0, true);
assert!(clocks.clock_wall().is_err());

// With TIME capability
let clocks = Clocks::new(CapabilitySet::TIME, 0, 0, true);
assert!(clocks.clock_wall().is_ok());

// Without RANDOM capability
let mut random = Random::new(CapabilitySet::empty(), vec![], true);
assert!(random.random_get(1).is_err());

// With RANDOM capability
let mut random = Random::new(CapabilitySet::RANDOM, vec![1], true);
assert!(random.random_get(1).is_ok());
```

## Production vs Deterministic Mode

### Production Mode (Non-Deterministic)

```rust
let ctx = HostContext::new();  // Uses system time and real randomness
assert!(!ctx.deterministic);

let clocks = ctx.create_clocks(CapabilitySet::TIME);
let res = clocks.clock_res_get(ClockId::MonotonicClock)?;
assert_eq!(res.nanoseconds, 1);  // High resolution

let mut random = ctx.create_random(CapabilitySet::RANDOM);
let bytes = random.random_get(32)?;  // Real CSPRNG
```

### Deterministic Mode (Replay)

```rust
let ctx = HostContext::deterministic()
    .with_wall_time(0)
    .with_entropy(vec![]);
assert!(ctx.deterministic);

let clocks = ctx.create_clocks(CapabilitySet::TIME);
let res = clocks.clock_res_get(ClockId::MonotonicClock)?;
assert_eq!(res.nanoseconds, 1_000_000);  // Lower resolution for replay

let mut random = ctx.create_random(CapabilitySet::RANDOM);
let bytes = random.random_get(32)?;  // From entropy pool
```

## Performance Considerations

- **Deterministic mode** uses lower clock resolution (1s wall, 1ms monotonic) to reduce timing sensitivity
- **Entropy pool** is used sequentially with wraparound for O(1) access
- **Capability checks** are O(1) bitflag operations
- **No system calls** in deterministic mode - all values are injected

## API Reference

### HostContext Methods

- `new()` - Create with current system time/entropy
- `deterministic()` - Create for replay/debugging
- `with_wall_time(nanos)` - Set wall clock time
- `with_monotonic_time(nanos)` - Set monotonic time
- `with_entropy(bytes)` - Set entropy pool
- `create_clocks(caps)` - Create Clocks interface
- `create_random(caps)` - Create Random interface

### Clocks Methods

- `clock_time_get(id, precision)` - Get time from clock
- `clock_res_get(id)` - Get clock resolution
- `clock_wall()` - Get wall clock time
- `clock_monotonic()` - Get monotonic time
- `set_wall_time(nanos)` - Update wall time
- `set_monotonic_time(nanos)` - Update monotonic time

### Random Methods

- `random_get(len)` - Get secure random bytes
- `random_insecure_get(len)` - Get fast random bytes
- `random_insecure_seed()` - Get 128-bit PRNG seed
- `set_entropy(bytes)` - Update entropy pool
- `reset_position()` - Reset entropy position
- `remaining_entropy()` - Get remaining bytes in pool

## Testing

All functionality is fully tested:

```bash
# Run all tests
cargo test -p aether-core wasi::

# Run specific module tests
cargo test -p aether-core wasi::clocks::
cargo test -p aether-core wasi::random::
```

## Future Enhancements

- [ ] Entropy pool expansion via PRNG when depleted
- [ ] Snapshot/restore for full state replay
- [ ] Time compression for faster replay
- [ ] Integration with debugger for time-travel breakpoints
