# ADR-003: Panic Abort Policy

## Status

**Accepted** - 2026-03-05

## Context

Rust's panic handling strategy affects system reliability and performance:

1. **Panic=Unwind** (default):
   - Stack unwinding on panic
   - Enables catch_unwind
   - Binary size overhead (~10-20%)
   - Runtime overhead for unwinding tables
   - Risk of double panics

2. **Panic=Abort**:
   - Immediate process termination
   - Smaller binaries
   - No unwinding overhead
   - Simpler error handling model
   - Forces explicit error handling

For Project Aether, we must choose based on:
- Reliability requirements (99.99% uptime)
- Performance requirements (sub-microsecond latency)
- Security requirements (no undefined behavior)
- Operational requirements (fast recovery)

## Decision

We adopt **panic=abort** for the entire codebase:

### Configuration

```toml
# Cargo.toml (workspace)
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"

[profile.test]
panic = "abort"
```

### Rationale

1. **Explicit Error Handling**: Forces use of `Result<T, E>` everywhere
2. **Performance**: Eliminates unwinding overhead
3. **Binary Size**: Reduces size by 10-20%
4. **Simplicity**: Clear failure semantics
5. **Security**: No unwinding-based vulnerabilities

### Failure Isolation Strategy

Since panics abort the process, we isolate failures at the **process boundary**:

```
Host Runtime Process (supervisor)
    ├─ WASM Engine Thread (panic isolated by process)
    ├─ Network Mesh Thread (panic isolated by process)
    └─ State Manager Thread (panic isolated by process)
```

If any subsystem panics:
1. Process terminates immediately
2. Supervisor detects exit
3. Supervisor restarts process
4. State recovered from FoundationDB
5. Actor migration if needed

### Exception: WASM Traps

WASM traps are NOT panics and are handled differently:

```rust
fn execute_actor(module: &Module) -> Result<(), Trap> {
    let result = module.invoke()?;
    Ok(result)
}
```

WASM traps:
- Are caught by wasmtime
- Do NOT abort the process
- Return `Err(Trap)` to caller
- Actor instance is terminated
- Host runtime continues

## Consequences

### Positive
- **Performance**: No unwinding overhead
- **Binary Size**: 10-20% smaller
- **Explicit Errors**: Forces Result-based error handling
- **Simplicity**: Clear failure semantics
- **Security**: No unwinding exploits
- **Determinism**: No unwinding nondeterminism

### Negative
- **No catch_unwind**: Cannot recover from panics in-process
- **All-or-Nothing**: Single panic kills entire process
- **FFI Care Required**: Panics across FFI boundaries are UB
- **Testing**: Tests cannot catch panics

### Neutral
- **Supervisor Required**: External process supervision needed
- **Recovery Time**: Process restart vs thread restart

## Coding Standards

### Required

```rust
fn good_example() -> Result<(), Error> {
    let resource = acquire_resource()?;  // Explicit error propagation
    process(resource)?;                  // No panics
    Ok(())
}
```

### Forbidden

```rust
fn bad_example() {
    let resource = acquire_resource().unwrap();  // ❌ Can panic
    let config = CONFIG.clone().expect("config");  // ❌ Can panic
    process(resource);  // ❌ Should return Result
}
```

### Allowable Panics

Only in these situations:
1. **Truly unreachable code**: `unreachable!()` in dead branches
2. **Invariant violations**: `assert!()` in constructor validation
3. **OOM**: Out-of-memory is unrecoverable anyway
4. **Test code**: Panics acceptable in test assertions

## Alternatives Considered

### 1. Panic=Unwind with catch_unwind
- **Pros**: Can recover from panics in-process
- **Rejected**: Overhead too high, encourages sloppy error handling

### 2. Panic=Unwind but never use catch_unwind
- **Pros**: Option to catch if needed
- **Rejected**: Binary overhead without benefit

### 3. Mixed Mode (abort for release, unwind for dev)
- **Pros**: Easier debugging in dev
- **Rejected**: Different behavior between dev and prod is dangerous

### 4. Custom Panic Handler
- **Pros**: Could log and restart
- **Rejected**: Complexity without clear benefit

## Recovery Architecture

### Supervisor Responsibilities

```rust
struct Supervisor {
    processes: Vec<SupervisedProcess>,
    restart_policy: RestartPolicy,
}

impl Supervisor {
    fn on_process_exit(&mut self, exit: ProcessExit) {
        match exit.status {
            ExitStatus::Panic => {
                log::error!("Process {} panicked, restarting", exit.pid);
                self.restart_process(&exit.process);
            }
            ExitStatus::Graceful => {
                log::info!("Process {} exited gracefully", exit.pid);
            }
        }
    }
}
```

### State Preservation

Since processes can die at any time:
1. **All state in FoundationDB**: External durability
2. **Idempotent operations**: Safe to retry
3. **Recovery from checkpoints**: Actor state reconstruction
4. **No in-process state**: All state externalized

## References

- [Rust Panic Handling](https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html)
- [Error Handling Best Practices](https://nick.groenen.me/posts/rust-error-handling/)
- BP-HOST-RUNTIME-001: Host Runtime Blue Paper
- THM-REL-001: Recovery Completeness Theorem

## Notes

- Monitor panic rates in production
- Ensure supervisor is bulletproof
- Review error handling patterns quarterly
