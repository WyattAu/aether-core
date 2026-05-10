# Project Aether Troubleshooting Guide

**Version:** 2.0.0
**Last Updated:** 2026-03-12  
**Audience:** Platform Operators, Developers

---

## Table of Contents

1. [Common Errors](#1-common-errors)
2. [Debug Techniques](#2-debug-techniques)
3. [Logging](#3-logging)
4. [Time-Travel Debugging](#4-time-travel-debugging)

---

## 1. Common Errors

### 1.1 Capability Errors

#### CAPABILITY_DENIED (Code 200)

**Symptoms:**
```
Error: Capability denied
  Actor: my-actor
  Required: net:tcp:connect:10.0.0.0/8:443
  Granted: [compute:cpu:10%, compute:memory:64MiB]
```

**Cause:** Actor attempted network operation without network capability.

**Solution:**
```toml
# aether.toml
[actors.my-actor]
capabilities = [
    "compute:cpu:10%",
    "compute:memory:64MiB",
    "net:tcp:connect:10.0.0.0/8:443",  # Add this
    "net:resolve:*"                     # DNS resolution
]
```

#### CAPABILITY_REVOKED (Code 201)

**Symptoms:**
```
Error: Capability revoked
  Token ID: tok-abc123
  Revoked at: 2026-03-06T12:00:00Z
  Reason: Security policy violation
```

**Cause:** Capability was revoked during execution.

**Solution:**
1. Check audit logs for revocation reason
2. Re-grant capability if appropriate
3. Fix security policy violation

```bash
# Check audit logs
aether logs --level warn --grep "revoked"

# Re-grant capability
aether grant my-actor net:tcp:connect:10.0.0.0/8:443
```

### 1.2 Actor Errors

#### ACTOR_NOT_FOUND (Code 100)

**Symptoms:**
```
Error: Actor not found
  Actor ID: actor://default/missing-actor/0
```

**Cause:** Target actor does not exist or has been destroyed.

**Solution:**
```bash
# Check actor status
aether status

# Check if actor exists
aether status missing-actor

# Recreate actor
aether apply
```

#### ACTOR_CRASHED (Code 101)

**Symptoms:**
```
Error: Actor crashed
  Actor: my-actor
  Instance: 0
  Exit code: 1
  Coredump: /var/lib/aether/cores/my-actor-12345.core
```

**Cause:** Actor process terminated unexpectedly.

**Solution:**
```bash
# View crash logs
aether logs my-actor

# Analyze coredump
aether debug coredump /var/lib/aether/cores/my-actor-12345.core

# Check for memory issues
aether metrics my-actor --memory

# Restart actor
aether restart my-actor
```

#### ACTOR_TIMEOUT (Code 102)

**Symptoms:**
```
Error: Actor timeout
  Actor: slow-actor
  Timeout: 30s
  Elapsed: 30.05s
```

**Cause:** Actor did not respond within timeout.

**Solution:**
```toml
# Increase timeout
[actors.slow-actor]
timeout = "60s"  # Increase from default
```

Or optimize actor performance:
```bash
# Profile actor
aether profile slow-actor --duration 30s

# Check for bottlenecks
aether metrics slow-actor
```

#### ACTOR_OUT_OF_MEMORY (Code 103)

**Symptoms:**
```
Error: Actor out of memory
  Actor: memory-hungry
  Limit: 64MiB
  Requested: 128MiB
```

**Cause:** Actor exceeded memory limit.

**Solution:**
```toml
# Increase memory limit
[actors.memory-hungry]
memory = "256MiB"  # Increase from 64MiB
```

Or optimize memory usage:
```bash
# Profile memory
aether profile memory-hungry --memory --duration 60s

# Find memory leaks
aether debug leak memory-hungry
```

#### ACTOR_OUT_OF_FUEL (Code 104)

**Symptoms:**
```
Error: Actor out of fuel
  Actor: compute-heavy
  Fuel consumed: 10,000,000
  Fuel limit: 10,000,000
```

**Cause:** Actor consumed all fuel (instruction budget).

**Solution:**
```toml
# Increase fuel limit
[actors.compute-heavy]
fuel = "100000000"  # 100M instructions

# Or disable fuel (not recommended for production)
[actors.compute-heavy]
fuel = "unlimited"
```

### 1.3 Network Errors

#### CONNECTION_REFUSED (Code 300)

**Symptoms:**
```
Error: Connection refused
  Target: db:5432
  Actor: api
```

**Cause:** Target actor not accepting connections.

**Solution:**
```bash
# Check if target actor is running
aether status db

# Check if target actor is listening
aether exec db -- netstat -tlnp | grep 5432

# Check network capabilities
aether status db --capabilities
```

#### TLS_HANDSHAKE_FAILED (Code 306)

**Symptoms:**
```
Error: TLS handshake failed
  Peer: node-2.aether.local:4200
  Reason: certificate expired
```

**Cause:** Certificate expired or invalid.

**Solution:**
```bash
# Check certificate status
aether certs list

# Rotate certificates
aether certs rotate

# Check certificate expiry
aether certs check --expiry
```

### 1.4 State Errors

#### STATE_CORRUPTED (Code 405)

**Symptoms:**
```
Error: State corrupted
  Actor: stateful-actor
  Checksum: expected abc123, got def456
```

**Cause:** State data corrupted.

**Solution:**
```bash
# Verify state integrity
aether state verify stateful-actor

# Restore from checkpoint
aether state restore stateful-actor --checkpoint cp-123

# Last resort: reset state
aether state reset stateful-actor
```

---

## 2. Debug Techniques

### 2.1 Health Checks

```bash
# System health
aether status

# Actor health
aether status my-actor

# Detailed health
aether status --detail

# Continuous monitoring
aether status --watch
```

### 2.2 Actor Inspection

```bash
# Actor details
aether inspect my-actor

# Output:
# Actor: my-actor
# ─────────────────────────────────
# Runtime:         wasm
# Module:          my-actor.wasm
# Hash:            sha256:abc123...
# Instances:       3
# Memory:          128MB / 256MB
# CPU:             15%
# Uptime:          2h30m
# 
# Capabilities:
#   - compute:cpu:25%
#   - compute:memory:256MiB
#   - net:tcp:listen:0.0.0.0:8080
#   - net:tcp:connect:10.0.0.0/8:5432
# 
# Environment:
#   LOG_LEVEL: debug
#   DB_HOST: db
```

### 2.3 Tracing

```bash
# Enable tracing for actor
aether trace my-actor --enable

# Invoke with trace
aether invoke my-actor --message "test" --trace

# View trace
aether trace view

# Output:
# Trace: req-abc123
# ─────────────────────────────────
# 0.000ms  Start request
# 0.012ms  Capability check: net:tcp:connect [PASS]
# 0.045ms  Connect to db:5432
# 0.089ms  Send query
# 0.156ms  Receive response
# 0.162ms  Return response
# 
# Total: 0.162ms
```

### 2.4 Debug Mode

```bash
# Enable debug mode
aether debug enable my-actor

# Debug invocation
aether debug invoke my-actor --message "test"

# Interactive debug
aether debug attach my-actor

# (gdb-like interface)
(aether-dbg) break handle
Breakpoint 1 at handle
(aether-dbg) continue
Continuing...
Breakpoint 1, handle (msg="test")
(aether-dbg) print msg
msg = "test"
(aether-dbg) step
(aether-dbg) backtrace
#0  handle (msg="test")
#1  _start ()
(aether-dbg) continue
```

### 2.5 Resource Analysis

```bash
# Memory analysis
aether analyze memory my-actor

# Output:
# Memory Analysis: my-actor
# ─────────────────────────────────
# Linear Memory:
#   Total:      256MB
#   Used:       128MB (50%)
#   Free:       128MB (50%)
#   Peak:       200MB
# 
# Allocation Breakdown:
#   Heap:       100MB (78%)
#   Stack:      20MB (16%)
#   Data:       8MB (6%)
# 
# Top Allocators:
#   45%  HashMap<K,V>
#   25%  Vec<u8>
#   15%  String
#   10%  Other
# 
# Potential Leaks:
#   [WARN] 5MB in static HashMap (growing)
```

---

## 3. Logging

### 3.1 Log Levels

| Level | Description |
|-------|-------------|
| `trace` | Very detailed debugging |
| `debug` | Detailed debugging |
| `info` | General information |
| `warn` | Warning conditions |
| `error` | Error conditions |

### 3.2 Configuring Logging

```toml
# aether.toml
[settings]
log-level = "info"

[settings.logging]
format = "json"              # json or text
output = "/var/log/aether/aether.log"
max-size = "100MB"
max-files = 10
```

### 3.3 Viewing Logs

```bash
# System logs
aether logs

# Actor logs
aether logs my-actor

# Follow logs
aether logs my-actor -f

# Filter by level
aether logs my-actor --level error

# Filter by time
aether logs my-actor --since 1h
aether logs my-actor --since "2026-03-06T10:00:00Z"

# Search logs
aether logs my-actor --grep "error"

# JSON output
aether logs my-actor -o json | jq '.level == "error"'
```

### 3.4 Log Format

**Text Format:**
```
2026-03-06T12:00:00.123Z INFO  my-actor: Request processed request_id=req-123 duration=45ms
```

**JSON Format:**
```json
{
  "timestamp": "2026-03-06T12:00:00.123Z",
  "level": "INFO",
  "actor": "my-actor",
  "message": "Request processed",
  "request_id": "req-123",
  "duration_ms": 45
}
```

### 3.5 Structured Logging in Actors

```rust
// Rust actor
use log::{info, warn, error};

fn handle(msg: Message) -> Response {
    info!(
        request_id = %msg.id,
        action = %msg.action,
        "Processing request"
    );
    
    match process(msg) {
        Ok(result) => {
            info!(
                request_id = %msg.id,
                duration_us = result.duration.as_micros(),
                "Request completed"
            );
            result
        }
        Err(e) => {
            error!(
                request_id = %msg.id,
                error = %e,
                "Request failed"
            );
            Response::error(e)
        }
    }
}
```

### 3.6 Log Aggregation

```yaml
# fluent-bit.conf
[INPUT]
    Name              forward
    Listen            0.0.0.0
    Port              24224

[FILTER]
    Name              record_modifier
    Match             aether.*
    Record            hostname ${HOSTNAME}

[OUTPUT]
    Name              elasticsearch
    Match             aether.*
    Host              elasticsearch
    Port              9200
    Index             aether-logs
```

---

## 4. Time-Travel Debugging

### 4.1 Overview

Aether supports time-travel debugging through deterministic execution. By recording all inputs (messages, time, randomness), you can replay execution to debug issues.

### 4.2 Enabling Recording

```toml
# aether.toml
[settings.debug]
recording-enabled = true
recording-path = "/var/lib/aether/recordings"
max-recording-size = "1GB"
```

### 4.3 Recording Sessions

```bash
# Start recording
aether record start my-actor

# Invoke actor (recorded)
aether invoke my-actor --message "test"

# Stop recording
aether record stop my-actor

# List recordings
aether record list my-actor

# Output:
# Recordings for my-actor
# ─────────────────────────────────
# ID          Started                 Duration  Events
# rec-abc123  2026-03-06T12:00:00Z   5.2s      1,234
# rec-def456  2026-03-06T12:05:00Z   3.1s      876
```

### 4.4 Replaying Sessions

```bash
# Replay recording
aether replay rec-abc123

# Output:
# Replaying rec-abc123
# ─────────────────────────────────
# 0.000s  Message received: "test"
# 0.001s  Capability check: compute [PASS]
# 0.002s  Processing message
# 0.003s  State read: key="counter"
# 0.004s  State write: key="counter" value=42
# 0.005s  Response sent: "ok"
# 
# Replay complete: 5 events in 5ms
```

### 4.5 Interactive Replay

```bash
# Interactive replay
aether replay rec-abc123 --interactive

# (debugger interface)
(aether-replay) step
Step 1: Message received: "test"
(aether-replay) print message
message = "test"
(aether-replay) step
Step 2: Capability check: compute [PASS]
(aether-replay) step
Step 3: Processing message
(aether-replay) backtrace
#0  handle (msg="test")
#1  _start ()
(aether-replay) continue
Replay complete
```

### 4.6 Debugging with Recordings

```bash
# Find issue in recording
aether replay rec-abc123 --find-error

# Output:
# Error found at event 1,234
# ─────────────────────────────────
# Error: Capability denied
#   Required: net:tcp:connect
#   Granted: [compute:cpu:10%]
# 
# Context:
#   1,230  Message received
#   1,231  Processing request
#   1,232  Attempting network call
#   1,233  Capability check failed
#   1,234  Error returned

# Jump to specific event
aether replay rec-abc123 --event 1230

# Diff recordings
aether replay diff rec-abc123 rec-def456
```

### 4.7 Recording Best Practices

1. **Record selectively**: Only record actors you're debugging
2. **Limit recording size**: Set appropriate max-recording-size
3. **Clean up old recordings**: Implement retention policy
4. **Secure recordings**: Recordings may contain sensitive data

```bash
# Clean up old recordings
aether record prune --older-than 7d

# Export recording
aether record export rec-abc123 --output recording.tar.gz

# Import recording
aether record import recording.tar.gz
```

### 4.8 Determinism Requirements

For time-travel debugging to work, execution must be deterministic:

1. **Time**: Host-injected, not from system clock
2. **Randomness**: Host-injected, not from system RNG
3. **Network**: Replay network responses from recording
4. **State**: Start from known state

```rust
// Good: Use host time
fn get_time() -> u64 {
    host::clock::now()  // Deterministic
}

// Bad: Use system time
fn get_time() -> u64 {
    std::time::SystemTime::now()  // Non-deterministic
}
```

---

## Appendix: Diagnostic Commands

### Quick Diagnostics

```bash
# Full system diagnostic
aether diagnostic

# Output:
# Aether Diagnostic Report
# ─────────────────────────────────
# System Status:     Healthy
# Uptime:            2d 5h 30m
# 
# Components:
#   Host Runtime:    [PASS] Running
#   WASM Engine:     [PASS] Running (1,234 actors)
#   Firecracker:     [PASS] Running (5 VMs)
#   Mesh Network:    [PASS] Running (10 peers)
#   State Manager:   [PASS] Running
# 
# Resources:
#   CPU:             45% (healthy)
#   Memory:          8.2GB / 32GB (healthy)
#   Disk:            120GB / 500GB (healthy)
#   Network:         2.5Gbps (healthy)
# 
# Errors (last 24h):
#   Total:           12
#   Critical:        0
#   Warning:         3
# 
# Recommendations:
#   - Consider increasing memory for actor 'worker'
#   - 3 capability denials in last hour
```

### Support Bundle

```bash
# Generate support bundle
aether support-bundle --output support.tar.gz

# Contains:
# - System logs
# - Actor logs
# - Configuration
# - Metrics snapshot
# - Network status
# - Coredumps (if any)
# - Recordings (recent)
```

---

*For more information, visit https://aether.dev/docs/troubleshooting*
