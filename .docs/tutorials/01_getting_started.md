# Getting Started with Aether

**Time:** ~20 minutes | **Prerequisites:** Rust 1.88+

---

## What is Aether?

Aether is a distributed actor framework for building scalable, resilient applications. It implements the **Actor Model** — a mathematical model for concurrent computation where "actors" are the universal primitives. Each actor is an isolated unit of computation that communicates exclusively through asynchronous messages.

In the Actor Model, every actor can:

1. **Send messages** to other actors (local or remote)
2. **Create new actors** dynamically
3. **Manage its own state** without shared memory

This eliminates entire classes of concurrency bugs — no locks, no race conditions, no deadlocks from shared mutable state. Actors process messages sequentially, so you write business logic as if it were single-threaded, while the framework handles concurrency across millions of actors.

Aether adds distributed capabilities on top: location-transparent messaging across nodes, automatic failover via mesh networking, persistent state that survives restarts, and a capability-based security model that sandboxes each actor's permissions. Whether you're building microservices, real-time event pipelines, or game servers, Aether gives you the primitives to do it reliably at scale.

---

## Installation

Add the Aether actor crate to your project:

```bash
cargo add aether-actor
```

### Verify

```bash
cargo build && echo "Aether actor crate ready"
```

Or check the version programmatically:

```rust
println!("Aether version: {}", aether_actor::version());
```

---

## Your First Actor

Let's build a `CounterActor` that receives messages and tracks a count in persistent state.

```rust
use aether_actor::{Actor, ActorContext, CapabilitySet, Handler, Message, Result};

struct CounterActor;

#[async_trait]
impl Handler for CounterActor {
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
        let raw = ctx.state().read("count").await?;
        let count: u64 = raw.parse().unwrap_or(0);
        ctx.log().info(format!("CounterActor started, count={count}"));
        Ok(())
    }

    async fn handle(&mut self, ctx: &ActorContext, msg: Message) -> Result<Message> {
        match msg.msg_type() {
            "increment" => {
                let raw = ctx.state().read("count").await?;
                let mut count: u64 = raw.parse().unwrap_or(0);
                count += 1;
                ctx.state().write("count", count.to_string()).await?;
                Ok(Message::response(&[("count", count)]))
            }
            "get" => {
                let raw = ctx.state().read("count").await?;
                let count: u64 = raw.parse().unwrap_or(0);
                Ok(Message::response(&[("count", count)]))
            }
            _ => Err(aether_actor::Error::unknown_message(msg.msg_type())),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let actor = CounterActor;
    let ctx = ActorContext::new("counter").await?;
    actor.run(ctx).await
}
```

### Send a Message

```rust
let response = ctx.call("counter", Message::new("increment")).await?;
println!("{:?}", response.payload()); // {"count": 1}

let response = ctx.call("counter", Message::new("increment")).await?;
println!("{:?}", response.payload()); // {"count": 2}
```

---

## Core Concepts

### Messages

Messages are the only way actors interact. Every message has a type, a payload, and optional metadata.

```rust
use aether_actor::Message;

let msg = Message::new("greet").with_payload("name", "Alice");

let response = ctx.call("greeter", msg).await?;
```

Use `ctx.call()` for request-response (waits for a reply) or `ctx.send()` for fire-and-forget.

### State

Each actor gets an isolated key-value store. State persists across restarts.

```rust
ctx.state().write("last_seen", "2026-03-26").await?;
let value = ctx.state().read("last_seen").await?;
let exists = ctx.state().exists("last_seen").await?;
let keys = ctx.state().list_keys("session_").await?;
ctx.state().delete("last_seen").await?;
```

### Capabilities

Capabilities define what an actor is allowed to do. Declare them up front — the runtime enforces them.

```rust
fn capabilities() -> CapabilitySet {
    CapabilitySet::builder()
        .with_state_read()
        .with_state_write()
        .with_network_outbound()
        .build()
}
```

| Capability | What it allows |
|---|---|
| `with_state_read()` / `with_state_write()` | Read/write persistent state |
| `with_actor_messaging()` | Send messages to other actors |
| `with_network_outbound()` | Make outbound network calls |
| `with_log()` | Write to logs |

---

## Adding Resilience

### Circuit Breaker

Wrap calls to external services with a circuit breaker so failures don't cascade.

```rust
use aether_actor::resilience::CircuitBreaker;

let breaker = CircuitBreaker::new("payment-service", 5, Duration::from_secs(30));

async fn charge_card(amount: f64) -> Result<PaymentResponse> {
    breaker.call(|| call_payment_gateway(amount)).await
}
```

### Retry Policy

```rust
use aether_actor::resilience::RetryPolicy;
use std::time::Duration;

let retry = RetryPolicy::new(3, Backoff::Exponential, Duration::from_millis(100));

async fn fetch_data(url: &str) -> Result<Data> {
    retry.call(|| http_get(url)).await
}
```

---

## Next Steps

| Tutorial | What you'll learn |
|---|---|
| [Stateful Actors](./02_stateful_actors.md) | Deep-dive into persistence, event sourcing, and pub/sub |
| [Performance](./04_performance.md) | Backpressure, rate limiting, circuit breakers, and windowing |
| [Examples](../../docs-site/docs/examples/overview.md) | Full example applications |

---

*Time to complete: ~20 minutes*
