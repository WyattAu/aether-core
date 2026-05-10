# Project Aether

**A distributed actor framework for building scalable, resilient applications.**

[![GitHub](https://img.shields.io/github/stars/WyattAu/aether-core?style=social)](https://github.com/WyattAu/aether-core)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Go Reference](https://pkg.go.dev/badge/github.com/WyattAu/aether-core/sdks/go/aether.svg)](https://pkg.go.dev/github.com/WyattAu/aether-core/sdks/go/aether)

## What is Aether?

Aether is a distributed actor framework that enables you to build highly scalable, fault-tolerant applications using the actor model. It provides:

- ** Lightweight Actors** - Spawn millions of actors with minimal overhead
- **[IN PROGRESS] Automatic Recovery** - Self-healing with supervisor hierarchies
- ** Mesh Networking** - Seamless distributed communication
- ** Capability-based Security** - Fine-grained permission control
- ** Persistent State** - State survives actor restarts
- ** Multi-language SDKs** - Go, Python, JavaScript, Rust

## Quick Start

### Go

```go
package main

import (
    "context"
    "fmt"
    "log"

    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type HelloActor struct {
    *aether.BaseActor
}

func (a *HelloActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    return aether.NewResponse(msg, fmt.Sprintf("Hello, %v!", msg.Payload)), nil
}

func main() {
    actor := aether.NewBaseActor("hello")
    actor.Require(aether.CapabilityActorMessaging)
    
    if err := actor.Run(context.Background()); err != nil {
        log.Fatal(err)
    }
}
```

### Python

```python
import asyncio
from aether_sdk import Actor, Message

class HelloActor(Actor):
    async def handle_message(self, sender: str, message: Message) -> Message:
        return Message.response(f"Hello, {message.payload}!")

async def main():
    actor = HelloActor("hello")
    await actor.start()
    await actor.run()

asyncio.run(main())
```

### JavaScript

```javascript
import { Actor, Message } from '@aether/sdk';

class HelloActor extends Actor {
    async handleMessage(sender, message) {
        return Message.response(`Hello, ${message.payload}!`);
    }
}

const actor = new HelloActor('hello');
await actor.start();
```

## Key Features

### Actor Model

Build concurrent applications using isolated actors that communicate via message passing:

```
┌─────────┐     ┌─────────┐     ┌─────────┐
│ Actor A │────▶│ Actor B │────▶│ Actor C │
└─────────┘     └─────────┘     └─────────┘
     │                              │
     └──────────────────────────────┘
```

### Supervisor Hierarchies

Let-it-crash philosophy with automatic recovery:

```
        ┌────────────┐
        │ Supervisor │
        └─────┬──────┘
         ┌────┴────┐
         │         │
    ┌────▼───┐ ┌───▼────┐
    │ Actor1 │ │ Actor2 │
    └────────┘ └────────┘
```

### Mesh Networking

Seamless distributed communication:

```
    Node A          Node B          Node C
   ┌───────┐       ┌───────┐       ┌───────┐
   │Actor 1│◄─────▶│Actor 2│◄─────▶│Actor 3│
   └───────┘       └───────┘       └───────┘
        ▲               │               │
        └───────────────┴───────────────┘
                    Mesh Network
```

### Capability Security

Fine-grained permissions:

```go
actor.Require(
    aether.CapabilityStateRead,
    aether.CapabilityStateWrite,
    aether.CapabilityNetworkOutbound,
)
```

## Use Cases

- **Microservices** - Build distributed services with actors
- **IoT Edge Computing** - Lightweight actors for constrained devices
- **Game Servers** - Manage game state with actor isolation
- **Real-time Systems** - Event-driven processing pipelines
- **AI/ML Workloads** - Distributed inference with AI actors

## Performance

| Metric | Value |
|--------|-------|
| Cold Start P99 | < 50µs |
| Actors per Node | 50,000+ |
| Message Latency P99 | < 10µs |
| Memory per Actor | ~2KB |

## Get Started

Ready to build with Aether? Check out the documentation:

- [Installation Guide](getting-started/installation.md)
- [Quick Start Tutorial](getting-started/quickstart.md)
- [Core Concepts](getting-started/concepts.md)

## Community

- [GitHub](https://github.com/WyattAu/aether-core)
- [Discord](https://discord.gg/aether)
- [Twitter](https://twitter.com/aether_dev)

## License

Aether is licensed under the [MIT License](https://opensource.org/licenses/MIT).
