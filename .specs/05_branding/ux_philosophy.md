# UX Philosophy - Project Aether

**Version:** 1.0.0  
**Last Updated:** 2026-03-06  
**Classification:** UX Design Guidelines

---

## 1. Design Principles

### 1.1 Core Principles

| Principle | Description |
|-----------|-------------|
| **Speed First** | Every interaction should feel instant |
| **Progressive Disclosure** | Simple by default, powerful when needed |
| **Zero Surprises** | Behavior matches expectations |
| **Helpful Errors** | Errors explain what went wrong and how to fix it |
| **Consistency** | Same patterns, same behavior, everywhere |

### 1.2 Principle Details

#### Speed First

**Why it matters:** Aether is about performance. The UX should reflect that.

**How we apply it:**
- CLI commands complete in <100ms or show progress
- Dashboard loads in <1s
- No spinner for operations <500ms
- Immediate feedback on user actions

**Examples:**
```bash
# Good: Instant feedback
$ aether status
Status: Running [PASS]

# Bad: No feedback
$ aether apply
# (user waits... nothing happens...)
# (eventually...)
Applied.

# Good: Progress indication for long operations
$ aether apply
Applying configuration...
  [PASS] actor-api: created
  [PASS] actor-db: created
  [PASS] network: configured
Applied in 2.3s
```

#### Progressive Disclosure

**Why it matters:** Users shouldn't be overwhelmed with options.

**How we apply it:**
- Simple commands for common cases
- Flags for advanced options
- Default values that work for 80% of cases
- Configuration for the remaining 20%

**Examples:**
```bash
# Simple case (80%)
aether apply

# With options (15%)
aether apply --wait

# Advanced (5%)
aether apply --wait --timeout 120s --force --prune
```

#### Zero Surprises

**Why it matters:** Trust is built on predictability.

**How we apply it:**
- Commands do what their names suggest
- Changes are shown before being applied
- Destructive operations require confirmation
- Behavior is consistent across commands

**Examples:**
```bash
# Good: Preview before destructive action
$ aether destroy api
This will destroy actor 'api':
  - 3 running instances
  - 256MB memory
  - Associated volumes: data-vol

Continue? [y/N]
```

#### Helpful Errors

**Why it matters:** Errors are learning opportunities.

**How we apply it:**
- Explain what went wrong
- Explain why it went wrong
- Explain how to fix it
- Include relevant documentation links

**Examples:**
```bash
# Bad: Cryptic error
Error: EACCES

# Good: Helpful error
Error: Permission denied

  You don't have permission to modify 'production' namespace.
  
  To fix this:
  1. Check your current context: aether context list
  2. Switch to a context with permission: aether context use development
  3. Or request access from your administrator

  Documentation: https://aether.dev/docs/rbac
```

#### Consistency

**Why it matters:** Consistent interfaces reduce cognitive load.

**How we apply it:**
- Same flag names across commands
- Same output formats
- Same error patterns
- Same interaction patterns

---

## 2. CLI UX Guidelines

### 2.1 Command Structure

```
aether <noun> <verb> [flags] [arguments]

Examples:
  aether actor list
  aether actor create --file actor.yaml
  aether logs api --follow
  aether invoke api --message '{"key": "value"}'
```

### 2.2 Output Formatting

#### Success Output

```
# Simple success
[PASS] Actor 'api' created

# With details
[PASS] Actor 'api' created
  Runtime: wasm
  Instances: 1
  Memory: 64MiB

# Tabular output
NAME        RUNTIME  STATUS   INSTANCES  MEMORY
api         wasm     Running  3          192MiB
db          oci      Running  1          512MiB
worker      wasm     Running  5          320MiB
```

#### Error Output

```
# Error with explanation
[FAIL] Error: Capability denied

  Actor 'worker' attempted 'net:tcp:connect:10.0.0.1:443'
  but only has capabilities:
    - compute:cpu:10%
    - compute:memory:64MiB

  To fix, add to aether.toml:
    capabilities = ["net:tcp:connect:10.0.0.0/8:443"]

  Documentation: https://aether.dev/docs/capabilities
```

### 2.3 Progress Indicators

#### Short Operations (<500ms)

No progress indicator needed. Just complete.

#### Medium Operations (500ms-5s)

Simple spinner:

```
Creating actor 'api'...
[PASS] Actor 'api' created
```

#### Long Operations (>5s)

Detailed progress:

```
Applying configuration...
  [1/5] Validating configuration...     [PASS]
  [2/5] Creating actor 'api'...         [PASS]
  [3/5] Creating actor 'db'...          [PASS]
  [4/5] Configuring network...          [PASS]
  [5/5] Waiting for readiness...        [PASS]
  
[PASS] Applied in 4.2s
```

### 2.4 Interactive Prompts

#### Confirmation Prompts

```
# Destructive operation
$ aether destroy production
[WARN]  Warning: You are about to destroy all actors in 'production'

This will:
  - Stop 15 running actors
  - Delete 3 persistent volumes
  - Remove all network configuration

Type 'production' to confirm: production
[PASS] Namespace 'production' destroyed
```

#### Selection Prompts

```
$ aether context use
? Select context:
  > production (default)
    staging
    development
    local
```

### 2.5 Color Usage

| Color | Meaning | Usage |
|-------|---------|-------|
| Green | Success, healthy | [PASS], Running |
| Yellow | Warning, pending | [WARN], Pending |
| Red | Error, unhealthy | [FAIL], Failed |
| Blue | Information | Links, commands |
| Cyan | Highlighting | Key values |
| Gray | Secondary | Timestamps, metadata |

### 2.6 Help Text

```
$ aether apply --help
Apply configuration from aether.toml

Usage:
  aether apply [OPTIONS]

Options:
  -f, --file <FILE>      Configuration file (default: aether.toml)
      --dry-run          Validate without applying
      --wait             Wait for actors to be ready
      --timeout <DUR>    Wait timeout (default: 60s)
      --force            Force apply changes
      --prune            Remove orphaned resources

Examples:
  # Apply default configuration
  aether apply

  # Apply with validation only
  aether apply --dry-run

  # Apply and wait for readiness
  aether apply --wait --timeout 120s

Documentation:
  https://aether.dev/docs/cli/apply
```

---

## 3. Dashboard UX Guidelines

### 3.1 Layout Structure

```
┌─────────────────────────────────────────────────────────────────────┐
│  Logo    Dashboard  Actors  Network  State  Settings    [Profile]  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │                  │  │                  │  │                  │  │
│  │   System Health  │  │   Actor Status   │  │   Throughput     │  │
│  │   ● Healthy      │  │   12 / 15        │  │   1.2M msg/s     │  │
│  │                  │  │                  │  │                  │  │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘  │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │                        Actors                                   ││
│  │  ┌────────────────────────────────────────────────────────────┐││
│  │  │  api          ● Running    3 instances    192MiB   45%    │││
│  │  │  db           ● Running    1 instance     512MiB   12%    │││
│  │  │  worker       ● Running    5 instances    320MiB   78%    │││
│  │  │  cache        ○ Stopped    0 instances    0MiB     0%     │││
│  │  └────────────────────────────────────────────────────────────┘││
│  └────────────────────────────────────────────────────────────────┘│
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Information Hierarchy

#### Level 1: Overview
- System health (single indicator)
- Total actors running
- Total throughput

#### Level 2: List View
- Actor names
- Status indicators
- Key metrics (instances, memory, CPU)

#### Level 3: Detail View
- Full configuration
- All metrics
- Recent logs
- Events timeline

#### Level 4: Deep Dive
- Metrics graphs
- Log explorer
- Trace viewer
- Debug tools

### 3.3 Status Indicators

```
● Running       (green)    - Healthy, operational
● Starting      (yellow)   - Transitioning to running
● Stopping      (yellow)   - Transitioning to stopped
○ Stopped       (gray)     - Intentionally stopped
● Warning       (yellow)   - Running but issues detected
● Degraded      (orange)   - Running with reduced capacity
● Failed        (red)      - Error state
○ Unknown       (gray)     - Status cannot be determined
```

### 3.4 Dashboard Performance

| Metric | Target | Max |
|--------|--------|-----|
| Initial Load | <1s | 2s |
| Data Refresh | <100ms | 500ms |
| Interaction Response | <50ms | 200ms |
| Chart Render | <200ms | 500ms |

### 3.5 Accessibility

- WCAG 2.1 AA compliance
- Keyboard navigation for all actions
- Screen reader compatible
- Color not sole indicator of state
- Minimum contrast ratios

---

## 4. Error Message Guidelines

### 4.1 Error Structure

```
Error: [What happened]

  [Why it happened]
  
  [How to fix it]
  
  [Documentation link]
```

### 4.2 Error Categories

#### User Errors (4xx)

```
Error: Invalid configuration

  The 'capabilities' field must be an array, but got a string.
  
  In aether.toml, line 15:
    capabilities = "network-client"
    
  Should be:
    capabilities = ["network-client"]
  
  Documentation: https://aether.dev/docs/config
```

#### System Errors (5xx)

```
Error: Connection failed

  Could not connect to Aether daemon at /var/run/aether.sock
  
  Possible causes:
  - Daemon is not running: `aether daemon`
  - Socket path is wrong: check AETHER_RUN_DIR
  - Permission denied: check socket permissions
  
  To diagnose:
    aether diagnostic
  
  Documentation: https://aether.dev/docs/troubleshooting
```

#### Capability Errors

```
Error: Capability denied

  Actor 'api' cannot perform 'fs:write:/data/output.json'
  
  Current capabilities:
    [PASS] compute:cpu:25%
    [PASS] compute:memory:256MiB
    [PASS] net:tcp:listen:0.0.0.0:8080
    [FAIL] fs:write (not granted)
  
  To grant this capability, add to aether.toml:
    [actors.api]
    capabilities = [
      "compute:cpu:25%",
      "compute:memory:256MiB",
      "net:tcp:listen:0.0.0.0:8080",
      "fs:write:/data/*"  # Add this
    ]
  
  Documentation: https://aether.dev/docs/capabilities
```

### 4.3 Error Tone

| Do | Don't |
|----|----|
| Be specific | Be vague |
| Explain why | Just say "error" |
| Offer solutions | Blame the user |
| Link to docs | Leave user stranded |
| Use plain language | Use jargon |

### 4.4 Error Examples

#### Good Error

```
Error: Actor 'worker' out of memory

  The actor exceeded its 64MiB memory limit while processing
  a 10MB message. Peak memory usage was 78MiB.
  
  Solutions:
  1. Increase memory limit in aether.toml:
     [actors.worker]
     memory = "128MiB"
  
  2. Process smaller messages
  3. Stream large data instead of buffering
  
  Memory profile: aether profile worker --memory
  Documentation: https://aether.dev/docs/memory
```

#### Bad Error

```
Error: OOM
```

---

## 5. Interaction Patterns

### 5.1 Common Patterns

#### Create Flow

```
1. User runs create command
2. Validate input locally
3. Show preview of what will be created
4. Ask for confirmation (if destructive)
5. Execute
6. Show result
```

#### Status Flow

```
1. User runs status command
2. Fetch current state
3. Format and display
4. (Optional) Watch for changes
```

#### Debug Flow

```
1. User encounters error
2. Error message provides guidance
3. User runs diagnostic command
4. Diagnostic provides detailed analysis
5. User applies fix
6. Verify fix resolved issue
```

### 5.2 Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+C` | Cancel current operation |
| `Ctrl+D` | Exit interactive mode |
| `Tab` | Auto-complete |
| `↑/↓` | Navigate history |
| `?` | Show help |

### 5.3 Autocomplete

```
$ aether act<Tab>
actor   actors  

$ aether actor <Tab>
create   destroy   list   logs   restart   status

$ aether actor logs <Tab>
api   db   worker   cache
```

---

## 6. Documentation UX

### 6.1 Documentation Principles

1. **Task-focused**: Organize by what users want to do
2. **Example-heavy**: Show, don't just tell
3. **Searchable**: Easy to find what you need
4. **Versioned**: Match documentation to software version
5. **Interactive**: Try it yourself links

### 6.2 Documentation Structure

```
Getting Started
  Quick Start
  Installation
  First Actor
  
Tutorials
  Building a Web Service
  Running Databases
  Distributed Processing
  Edge Deployment

Reference
  CLI Reference
  Configuration
  API Reference
  Capabilities

Concepts
  Architecture
  Security Model
  Networking
  State Management

Troubleshooting
  Common Errors
  Debug Guide
  Performance Tuning
  FAQ
```

### 6.3 Code Examples

````markdown
### Create a Simple Actor

Create a new Rust project:

```bash
mkdir my-actor && cd my-actor
cargo init --lib
```

Add the Aether SDK:

```toml
# Cargo.toml
[dependencies]
aether-sdk = "0.4"
```

Write your actor:

```rust
use aether_sdk::prelude::*;

#[actor]
pub fn handle(msg: String) -> String {
    format!("Hello, {}!", msg)
}
```

Build and deploy:

```bash
cargo build --target wasm32-wasip2 --release
aether apply
```

[Try it yourself](/playground?example=hello)
````

---

## 7. Success Metrics

### UX Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Time to first success | <5 minutes | New user study |
| CLI command success rate | >95% | Analytics |
| Error recovery rate | >90% | User surveys |
| Documentation helpfulness | 4.5/5 | Feedback forms |
| Dashboard task completion | >95% | Usability testing |

### Satisfaction Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| NPS | >50 | Quarterly survey |
| Developer satisfaction | 4.5/5 | GitHub discussions |
| Support ticket reduction | -50% YoY | Ticket tracking |

---

*Document Classification: Internal / Design*
