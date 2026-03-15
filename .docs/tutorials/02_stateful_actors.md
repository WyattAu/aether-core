# Tutorial 2: Stateful Actors

In this tutorial, you'll create an actor that persists state across restart boundaries.

**Prerequisites:** Completed [Getting Started Tutorial](./getting_started.md)

---

## Step 1: Create the stateful actor

In a new directory:

```bash
mkdir stateful-actor
cd stateful-actor

# Create Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "stateful-actor"
version = "0.1.0"
edition = "2021"

[dependencies]
aether-core = { path = "../../crates/core" }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tracing = "0.1"
EOF

# Create source file
cat > src/lib.rs << 'EOF'
use aether_core::actor::{Actor, ActorContext, Handler, Message};
use aether_core::capability::CapabilitySet;
use aether_core::error::{Error, Result};
use aether_core::state::{KeyValueStore, MemoryEntry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct StatefulActor {
    store: Arc<dyn KeyValueStore>,
    counter: u64,
}

// Define the Greeting message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greet {
    pub name: String,
}

// Define the Greeting response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeting {
    pub message: String,
}

impl Message for Greet {
    type Response = Greeting;
}

#[async_trait]
impl Actor for StatefulActor {
    type Config = ();
    
    async fn on_start(&mut self, ctx: &ActorContext) -> Result<()> {
        self.store = Some(ctx.state().await?);
        tracing::info!("StatefulActor started");
        Ok(())
    }
    
    fn capabilities() -> CapabilitySet {
        CapabilitySet::empty()
    }
}

#[async_trait]
impl Handler<Greet> for StatefulActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: Greet) -> Result<Greeting> {
        // Increment counter
        self.counter += 1;
        
        // Save greeting to state store
        let path = format!("/actors/{}", actor.id);
        let entry = MemoryEntry::new(greeting.message.as_bytes().into_vec())
            .with_ttl(std::time::Duration::from_secs(3600));
        self.store.put(&path, entry).await?;
        
        Ok(Greeting {
            message: format!("Hello, {}! Welcome to Aether."),
        })
    }
}

// Main function - handle messages
fn main() -> Result<()> {
    let mut actor = StatefulActor::new();
    
    // Process command line arguments
    let args: Vec<String> = args.iter().map(|a| a.to_string());
    
    actor.handle(Greet, args).await
}

EOF
```

---

## Step 2: Build and Deploy

```bash
# Build the actor
cargo build --target wasm32-unknown-unknown --release

# Check the the actor was built
ls -la ./target/wasm32-unknown-unknown/release/
```

---

## Step 3: Test the stateful actor

First, start the Aether node locally:

```bash
# Start FoundationDB (required for state persistence)
sudo systemctl start foundationdb

# Check that it's running
fdbcli status
```

---

## Step 4: Deploy the actor

```bash
# Deploy to actor
aether deploy ./target/wasm32-unknown-unknown/release/stateful_actor.wasm

# Note the actor ID from the output
actor_id="actor-abc123"
```

---

## Step 5: Test the state persistence

Call the actor and verify the the counter is persisted.

```bash
# Call the actor multiple times
aether call actor-abc123 Greet '{"name": "World"}'
aether call actor-abc123 Greet '{"name": "Alice"}'
aether call actor-abc123 Greet '{"name": "Bob"}'
```

---

## Step 6: Verify state persistence

```bash
# Check state files
aether state list actor-abc123

# Expected output: should show the persisted state
```

---

## Step 7: Test state expiration

```bash
# Wait 60 seconds (TTL)
aether call actor-abc123 GetState '{"key": "greeting"}'

# Expected: null (state should be expired)
aether call actor-abc123 GetState '{"key": "greeting"}'

# Expected: "Hello, World!" (or error if expired)
aether call actor-abc123 GetState '{"key": "expired"}'
```

---

## What you've learned

- **Actor lifecycle**: on_start → message handlers → state management
- **Capability security**: How to define and enforce capabilities
- **Testing**: How to test actors locally

---

## Next Steps

### Continue learning

| Topic | Documentation |
|------|-------------|
| [AI Integration](/.docs/code_examples.md#ai-integration) | Deep dive into AI capabilities |
| [Mesh Networking](/.docs/deployment_guide.md# distributed deployment |
| [State Management](/.docs/user_guide.md# state operations |

---

