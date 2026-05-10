# Project Aether - Getting Started

**Version:** 2.0.0
**Last Updated:** 2026-03-14  
**Time to Complete:** ~15 minutes

---

## What You'll Build

In this guide, you'll:
1. Install Aether
2. Create a simple actor
3. Deploy it locally
4. Call it via the mesh network

## Prerequisites

| Requirement | Version | How to Check |
|-------------|---------|--------------|
| Linux | 5.15+ | `uname -r` |
| Rust | 1.75+ | `rustc --version` |
| FoundationDB | 7.1+ | `fdbcli --version` |
| Docker (optional) | 24+ | `docker --version` |

---

## Step 1: Install Aether (5 min)

### Option A: Binary (Recommended)

```bash
# Download latest release
curl -LO https://github.com/WyattAu/aether-core/releases/latest/download/aether-linux-amd64

# Install
chmod +x aether-linux-amd64
sudo mv aether-linux-amd64 /usr/local/bin/aether

# Verify
aether version
```

### Option B: From Source

```bash
# Clone repository
git clone https://github.com/WyattAu/aether-core.git
cd aether

# Build with Nix (recommended)
nix develop
cargo build --release

# Or build directly
cargo build --release

# Binary location
./target/release/aether version
```

---

## Step 2: Create Your First Actor (5 min)

### Initialize Project

```bash
# Create new project
aether init my-first-actor
cd my-first-actor

# Project structure
# my-first-actor/
# ├── Cargo.toml
# ├── src/
# │   └── lib.rs
# └── aether.toml
```

### Write the Actor

Edit `src/lib.rs`:

```rust
use aether_core::actor::{Actor, ActorContext, Handler, Message};
use aether_core::error::Result;
use serde::{Deserialize, Serialize};

/// A simple greeting actor
pub struct GreetingActor {
    greetings_count: u64,
}

impl GreetingActor {
    pub fn new() -> Self {
        Self { greetings_count: 0 }
    }
}

/// Greeting request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greet {
    pub name: String,
}

/// Greeting response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeting {
    pub message: String,
    pub count: u64,
}

impl Message for Greet {
    type Response = Greeting;
}

#[async_trait::async_trait]
impl Handler<Greet> for GreetingActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: Greet) -> Result<Greeting> {
        self.greetings_count += 1;
        Ok(Greeting {
            message: format!("Hello, {}! Welcome to Aether.", msg.name),
            count: self.greetings_count,
        })
    }
}

#[async_trait::async_trait]
impl Actor for GreetingActor {
    type Config = ();
    
    async fn on_start(&mut self, _ctx: &ActorContext) -> Result<()> {
        tracing::info!("Greeting actor started");
        Ok(())
    }
}
```

### Configure the Actor

Edit `aether.toml`:

```toml
[actor.greeter]
runtime = "wasm"
max_memory = "64MiB"
max_fuel = 10_000_000

[actor.greeter.config]
# Actor-specific configuration
```

---

## Step 3: Build and Deploy (3 min)

### Build the Actor

```bash
# Build as WebAssembly
aether build

# Output: ./target/wasm32-unknown-unknown/release/my_first_actor.wasm
```

### Start the Aether Runtime

```bash
# Start FoundationDB (if not running)
sudo systemctl start foundationdb

# Start Aether node
aether run --config aether.toml
```

### Deploy the Actor

```bash
# In a new terminal
aether deploy ./target/wasm32-unknown-unknown/release/my_first_actor.wasm

# Output: Actor deployed with ID: actor-abc123
```

---

## Step 4: Call Your Actor (2 min)

### Via CLI

```bash
# Call the actor
aether call actor-abc123 Greet '{"name": "World"}'

# Output:
# {"message": "Hello, World! Welcome to Aether.", "count": 1}
```

### Via HTTP API

```bash
# Get actor info
curl http://localhost:8080/api/v1/actors/actor-abc123

# Call actor
curl -X POST http://localhost:8080/api/v1/actors/actor-abc123/call \
  -H "Content-Type: application/json" \
  -d '{"type": "Greet", "payload": {"name": "Developer"}}'
```

---

## Next Steps

### Learn More

| Topic | Documentation |
|-------|---------------|
| Actor Patterns | [Code Examples](./code_examples.md) |
| Configuration | [User Guide](./user_guide.md) |
| Architecture | [Architecture Overview](./architecture_overview.md) |
| Mesh Networking | [Deployment Guide](./deployment_guide.md) |

### Try These Examples

1. **Stateful Actor** - Add persistence with FoundationDB
2. **Scheduled Actor** - Run periodic tasks
3. **Mesh Actor** - Communicate across nodes
4. **AI Actor** - Integrate with AI providers

### Join the Community

- **Discord**: [discord.gg/aether](https://discord.gg/aether)
- **GitHub Discussions**: [github.com/WyattAu/aether-core/discussions](https://github.com/WyattAu/aether-core/discussions)
- **Issues**: [github.com/WyattAu/aether-core/issues](https://github.com/WyattAu/aether-core/issues)

---

## Troubleshooting

### Common Issues

#### "Failed to connect to FoundationDB"

```bash
# Check FDB status
fdbcli status

# Start if needed
sudo systemctl start foundationdb

# Check cluster file
cat /etc/foundationdb/fdb.cluster
```

#### "Actor failed to start"

```bash
# Check logs
aether logs actor-abc123

# Verify capabilities in aether.toml
# Ensure actor has required permissions
```

#### "Build failed: wasm target not found"

```bash
# Add WASM target
rustup target add wasm32-unknown-unknown

# Rebuild
aether build
```

### Getting Help

1. Check [Troubleshooting Guide](./troubleshooting.md)
2. Search [GitHub Issues](https://github.com/WyattAu/aether-core/issues)
3. Ask in [Discord](https://discord.gg/aether)

---

## Quick Reference

### Essential Commands

| Command | Description |
|---------|-------------|
| `aether init <name>` | Create new project |
| `aether build` | Build actor to WASM |
| `aether run` | Start runtime node |
| `aether deploy <wasm>` | Deploy actor |
| `aether call <id> <msg> <json>` | Call actor |
| `aether status` | Check node status |
| `aether logs <id>` | View actor logs |

### Project Structure

```
my-actor/
├── Cargo.toml           # Rust dependencies
├── src/
│   └── lib.rs          # Actor implementation
├── aether.toml         # Aether configuration
└── tests/
    └── integration.rs  # Integration tests
```

### Configuration Snippets

```toml
# aether.toml

[node]
id = "node-1"
bind_addr = "0.0.0.0:7000"

[state]
backend = "fdb"
cluster_file = "/etc/foundationdb/fdb.cluster"

[observability]
enabled = true
metrics_port = 9090

[[actor]]
name = "my-actor"
runtime = "wasm"
path = "./target/wasm32-unknown-unknown/release/my_actor.wasm"
```
