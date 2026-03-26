# Getting Started with Aether

**Time:** ~20 minutes | **Prerequisites:** None

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

Aether provides SDKs for Python, JavaScript, and Go. Install the one that matches your stack.

=== "Python"

    ```bash
    pip install aether-sdk
    ```

=== "JavaScript / TypeScript"

    ```bash
    npm install @aether/sdk
    ```

=== "Go"

    ```bash
    go get github.com/aether/aether-core/sdks/go
    ```

### Verify

=== "Python"

    ```bash
    python -c "import aether_sdk; print(aether_sdk.__version__)"
    ```

=== "JavaScript / TypeScript"

    ```bash
    node -e "const aether = require('@aether/sdk'); console.log(aether.version)"
    ```

=== "Go"

    ```bash
    go run -v <<EOF
    package main
    import (
        "fmt"
        "github.com/aether/aether-core/sdks/go/aether"
    )
    func main() { fmt.Println("Aether Go SDK:", aether.Version) }
    EOF
    ```

---

## Your First Actor

Let's build a `CounterActor` that receives messages and tracks a count in persistent state.

=== "Python"

    ```python
    import asyncio
    from aether_sdk import Actor, Message

    class CounterActor(Actor):
        def __init__(self):
            super().__init__("counter")
            self.require("STATE_READ", "STATE_WRITE", "ACTOR_MESSAGING", "LOG")

        async def on_start(self):
            raw = await self.state.read("count")
            self.count = int(raw) if raw else 0
            self.log.info(f"CounterActor started, count={self.count}")

        async def handle_message(self, sender: str, msg: Message) -> Message:
            match msg.type:
                case "increment":
                    self.count += 1
                    await self.state.write("count", str(self.count))
                    return Message.response({"count": self.count})
                case "get":
                    return Message.response({"count": self.count})
                case _:
                    return Message.error("unknown message type")

    async def main():
        actor = CounterActor()
        await actor.start()
        await actor.run()

    if __name__ == "__main__":
        asyncio.run(main())
    ```

=== "TypeScript"

    ```typescript
    import { Actor, Message, MessageType } from '@aether/sdk';

    class CounterActor extends Actor {
      private count: number = 0;

      constructor() {
        super('counter');
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG');
      }

      async onStart(): Promise<void> {
        const raw = await this.state.read('count');
        this.count = raw ? parseInt(raw, 10) : 0;
        this.log.info(`CounterActor started, count=${this.count}`);
      }

      async handleMessage(sender: string, msg: Message): Promise<Message> {
        switch (msg.type) {
          case 'increment':
            this.count += 1;
            await this.state.write('count', String(this.count));
            return Message.response({ count: this.count });
          case 'get':
            return Message.response({ count: this.count });
          default:
            return Message.error('unknown message type');
        }
      }
    }

    async function main() {
      const actor = new CounterActor();
      await actor.start();
      await actor.run();
    }

    main().catch(console.error);
    ```

### Send a Message

=== "Python"

    ```python
    response = await actor.call("counter", Message("increment"))
    print(response.payload)  # {'count': 1}

    response = await actor.call("counter", Message("increment"))
    print(response.payload)  # {'count': 2}
    ```

=== "TypeScript"

    ```typescript
    const response = await actor.call('counter', new Message('increment'));
    console.log(response.payload); // { count: 1 }

    const response2 = await actor.call('counter', new Message('increment'));
    console.log(response2.payload); // { count: 2 }
    ```

---

## Core Concepts

### Messages

Messages are the only way actors interact. Every message has a type, a payload, and optional metadata.

=== "Python"

    ```python
    from aether_sdk import Message

    msg = Message("greet", payload={"name": "Alice"})

    response = await actor.call("greeter", msg)
    ```

=== "TypeScript"

    ```typescript
    import { Message } from '@aether/sdk';

    const msg = new Message('greet', { name: 'Alice' });

    const response = await actor.call('greeter', msg);
    ```

Use `actor.call()` for request-response (waits for a reply) or `actor.send()` for fire-and-forget.

### State

Each actor gets an isolated key-value store. State persists across restarts.

=== "Python"

    ```python
    await self.state.write("last_seen", "2026-03-26")
    value = await self.state.read("last_seen")
    exists = await self.state.exists("last_seen")
    keys = await self.state.list_keys("session_")
    await self.state.delete("last_seen")
    ```

=== "TypeScript"

    ```typescript
    await this.state.write('last_seen', '2026-03-26');
    const value = await this.state.read('last_seen');
    const exists = await this.state.exists('last_seen');
    const keys = await this.state.listKeys('session_');
    await this.state.delete('last_seen');
    ```

### Capabilities

Capabilities define what an actor is allowed to do. Declare them up front — the runtime enforces them.

=== "Python"

    ```python
    class MyActor(Actor):
        def __init__(self):
            super().__init__("my-actor")
            self.require("STATE_READ", "STATE_WRITE")
            self.require("NET_OUTBOUND")
    ```

=== "TypeScript"

    ```typescript
    class MyActor extends Actor {
      constructor() {
        super('my-actor');
        this.require('STATE_READ', 'STATE_WRITE');
        this.require('NET_OUTBOUND');
      }
    }
    ```

| Capability | What it allows |
|---|---|
| `STATE_READ` / `STATE_WRITE` | Read/write persistent state |
| `ACTOR_MESSAGING` | Send messages to other actors |
| `NET_OUTBOUND` | Make outbound network calls |
| `LOG` | Write to logs |

---

## Adding Resilience

### Circuit Breaker

Wrap calls to external services with a circuit breaker so failures don't cascade.

=== "Python"

    ```python
    from aether_sdk.resilience import CircuitBreaker

    breaker = CircuitBreaker(
        name="payment-service",
        failure_threshold=5,
        recovery_timeout=30.0,
    )

    async def charge_card(amount: float):
        return await breaker.call(lambda: call_payment_gateway(amount))
    ```

=== "TypeScript"

    ```typescript
    import { CircuitBreaker } from '@aether/sdk/resilience';

    const breaker = new CircuitBreaker({
      name: 'payment-service',
      failureThreshold: 5,
      recoveryTimeout: 30_000,
    });

    async function chargeCard(amount: number) {
      return breaker.call(() => callPaymentGateway(amount));
    }
    ```

### Retry Policy

=== "Python"

    ```python
    from aether_sdk.resilience import RetryPolicy

    retry = RetryPolicy(max_attempts=3, backoff="exponential", base_delay=0.1)

    async def fetch_data(url: str):
        return await retry.call(lambda: http_get(url))
    ```

=== "TypeScript"

    ```typescript
    import { RetryPolicy } from '@aether/sdk/resilience';

    const retry = new RetryPolicy({
      maxAttempts: 3,
      backoff: 'exponential',
      baseDelay: 100,
    });

    async function fetchData(url: string) {
      return retry.call(() => httpGet(url));
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
