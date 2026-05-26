# Quick Start

Build your first Aether actor in 5 minutes.

## Prerequisites

- Rust 1.88+ (MSRV)
- cargo

## Create Your First Actor

### Step 1: Initialize Project

```bash
cargo init my-actor && cd my-actor
cargo add aether-actor
```

### Step 2: Create the Actor

Replace `src/main.rs` with:

```rust
use aether_actor::{Actor, ActorContext, CapabilitySet, Handler, Message, Result};

struct GreeterActor;

#[async_trait]
impl Handler for GreeterActor {
    type Msg = Message;

    fn capabilities() -> CapabilitySet {
        CapabilitySet::builder()
            .with_actor_messaging()
            .with_log()
            .build()
    }

    async fn handle(&mut self, ctx: &ActorContext, msg: Message) -> Result<Message> {
        match msg.payload() {
            "ping" => {
                ctx.log().info("Received ping");
                Ok(Message::response_str("pong"))
            }
            name => {
                let greeting = format!("Hello, {name}!");
                ctx.log().info(format!("Greeting: {greeting}"));
                Ok(Message::response_str(&greeting))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let actor = GreeterActor;
    let ctx = ActorContext::new("greeter").await?;
    actor.run(ctx).await
}
```

### Step 3: Run the Actor

```bash
cargo run
```

You should see:

```
INFO aether_actor: Starting actor: greeter
```

## Add State Persistence

Make the actor stateful by counting greetings:

```rust
struct GreeterActor {
    count: u64,
}

#[async_trait]
impl Handler for GreeterActor {
    type Msg = Message;

    fn capabilities() -> CapabilitySet {
        CapabilitySet::builder()
            .with_state_read()
            .with_state_write()
            .with_actor_messaging()
            .with_log()
            .build()
    }

    async fn on_start(&mut self, ctx: &ActorContext) -> Result<()> {
        let raw = ctx.state().read("greeting_count").await?;
        self.count = raw.parse().unwrap_or(0);
        ctx.log().info(format!("Loaded greeting count: {}", self.count));
        Ok(())
    }

    async fn handle(&mut self, ctx: &ActorContext, msg: Message) -> Result<Message> {
        self.count += 1;
        ctx.state().write("greeting_count", self.count.to_string()).await?;

        let name = msg.payload();
        let greeting = format!("Hello, {name}! (greeting #{})", self.count);
        Ok(Message::response_str(&greeting))
    }
}
```

## Send Messages Between Actors

Create two actors that communicate:

```rust
use aether_actor::{ActorContext, Message};

// In one actor's handler:
async fn handle(&mut self, ctx: &ActorContext, msg: Message) -> Result<Message> {
    // Fire-and-forget message to another actor
    ctx.send("consumer", Message::new("hello")).await?;

    // Request-response (RPC) with timeout
    let response = ctx.call_timeout("service-actor", Message::new("get_data"), Duration::from_secs(5)).await?;
    Ok(response)
}
```

## Next Steps

- [Core Concepts](concepts.md) - Deep dive into actors, messages, and capabilities
- [Examples](../examples/overview.md) - More example applications
- [SDK Reference](../sdks/overview.md) - Detailed SDK documentation
