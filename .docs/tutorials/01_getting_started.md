# Aether Tutorial Series

Welcome to the Aether tutorial series! This guide will walk you through building your first complete application with Aether, each tutorial builds on core concepts to fully working examples.

## Tutorial 1: Your First actor (15 minutes)

**Prerequisites:** [Rust](https://rust-lang.org), [Aether CLI](https://get.aether.dev)

---

## Part 1: Create Your first Actor

In this tutorial, you'll create a simple "Hello world" actor using Aether.

### Step 1: Initialize the project

```bash
# Create a new project
mkdir my-first-actor
cd my-first-actor

# Create actor source file
cat > src/lib.rs << 'EOF'
use aether_core::actor::{Actor, ActorContext, Handler, Message};
use aether_core::error::Result;
use serde::{Deserialize, Serialize};

// Define the HelloActor struct
pub struct HelloActor;

// Define the Greeting message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greet {
    pub name: String,
}

impl Message for Greet {
    type Response = Greeting;
}

// Implement the message handler
#[async_trait::async_trait]
impl Handler<Greet> for HelloActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: Greet) -> Result<Greeting> {
        Ok(Greeting {
            message: format!("Hello, {}!"),
        })
    }
}

// Implement the Actor trait
#[async_trait::async_trait]
impl Actor for HelloActor {
    type Config = ();
    
    async fn on_start(&mut self, _ctx: &ActorContext) -> Result<()> {
        tracing::info!("HelloActor started with id: {:?}", ctx.actor_id());
        Ok(())
    }
    
    fn capabilities() -> aether_core::capability::CapabilitySet {
        CapabilitySet::empty() // No special capabilities needed
    }
}
```

---

## Part 2: Add AI Capabilities

Now let's enhance our actor to use AI:

```bash
# Update actor configuration
cat > aether.toml

# Add AI capability
[[actor.ai]]
capabilities = ["net_outbound"]
```

(End of file - total 63 lines)
