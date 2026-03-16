# Hello World Example

This example demonstrates the basic structure of an Aether actor.

## Overview

The Hello Actor:

1. Accepts greeting requests
2. Returns personalized hello messages
3. Handles graceful startup and shutdown

## Go Implementation

```go
package main

import (
    "fmt"
    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type HelloActor struct {
    aether.Actor
    requestCount int
}

func (a *HelloActor) OnStart() error {
    fmt.Printf("[%s] Hello Actor started\n", a.Name)
    return nil
}

func (a *HelloActor) OnStop() error {
    fmt.Printf("[%s] Hello Actor stopped (requests: %d)\n", a.Name, a.requestCount)
    return nil
}

func (a *HelloActor) HandleMessage(sender string, msg aether.Message) (aether.Message, error) {
    a.requestCount++
    
    // Extract name from payload
    var name string
    switch v := msg.Payload.(type) {
    case string:
        name = v
    case map[string]interface{}:
        if n, ok := v["name"].(string); ok {
            name = n
        }
    }
    
    if name == "" {
        name = "World"
    }
    
    return aether.Message{
        Type:    aether.MessageTypeResponse,
        Payload: map[string]interface{}{"greeting": fmt.Sprintf("Hello, %s!", name)},
    }, nil
}

func main() {
    actor := &HelloActor{}
    actor.Name = "hello-actor"
    actor.Require("ACTOR_MESSAGING", "LOG")
    
    if err := actor.Start(); err != nil {
        panic(err)
    }
    defer actor.Stop()
    
    fmt.Printf("Starting %s...\n", actor.Name)
    actor.Run()
}
```

## Python Implementation

```python
import asyncio
from aether_sdk import Actor, Message, MessageType

class HelloActor(Actor):
    def __init__(self):
        super().__init__("hello-actor")
        self.request_count = 0
        self.require("ACTOR_MESSAGING", "LOG")
    
    async def on_start(self) -> None:
        print(f"[{self.name}] Hello Actor started")
    
    async def on_stop(self) -> None:
        print(f"[{self.name}] Hello Actor stopped (requests: {self.request_count})")
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type not in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return None
        
        self.request_count += 1
        
        # Extract name from payload
        payload = message.payload
        name = "World"
        
        if isinstance(payload, str):
            name = payload
        elif isinstance(payload, dict):
            name = payload.get("name", "World")
        
        return Message.response({"greeting": f"Hello, {name}!"})

async def main():
    actor = HelloActor()
    await actor.start()
    
    print(f"Starting {actor.name}...")
    await actor.run()

if __name__ == "__main__":
    asyncio.run(main())
```

## JavaScript Implementation

```typescript
import { Actor, Message, MessageType } from '@aether/sdk';

class HelloActor extends Actor {
    private requestCount: number = 0;

    constructor() {
        super('hello-actor');
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Hello Actor started`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Hello Actor stopped (requests: ${this.requestCount})`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type !== MessageType.REQUEST && message.type !== MessageType.RPC_REQUEST) {
            return null;
        }

        this.requestCount++;

        // Extract name from payload
        const payload = message.payload;
        let name = 'World';

        if (typeof payload === 'string') {
            name = payload;
        } else if (typeof payload === 'object' && payload !== null) {
            name = (payload as Record<string, any>).name || 'World';
        }

        return Message.response({ greeting: `Hello, ${name}!` });
    }
}

async function main(): Promise<void> {
    const actor = new HelloActor();

    process.on('SIGINT', async () => {
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    await actor.start();
    await actor.run();
}

main();
```

## Running the Example

### Build and Run

```bash
# Go
cd sdks/go/examples/hello_actor
go run main.go

# Python
cd sdks/python/examples
python hello_actor.py

# JavaScript
cd sdks/js/examples
npx ts-node hello_actor.ts
```

### Testing

Send a request to the actor:

```bash
# Simple greeting
aether invoke hello-actor '{"name": "Alice"}'
# Response: {"greeting": "Hello, Alice!"}

# String payload
aether invoke hello-actor '"Bob"'
# Response: {"greeting": "Hello, Bob!"}

# Default greeting
aether invoke hello-actor '{}'
# Response: {"greeting": "Hello, World!"}
```

## Key Concepts

### Actor Lifecycle

| Method | When Called |
|--------|-------------|
| `onStart()` | Actor starts |
| `handleMessage()` | Message received |
| `onStop()` | Actor stops |

### Capabilities

| Capability | Purpose |
|------------|---------|
| `ACTOR_MESSAGING` | Send/receive messages |
| `LOG` | Write to logs |

### Message Types

| Type | Description |
|------|-------------|
| `REQUEST` | Request expecting response |
| `RESPONSE` | Response to a request |
| `EVENT` | Fire-and-forget event |
| `RPC_REQUEST` | RPC method call |
| `RPC_RESPONSE` | RPC method response |

## Next Steps

- [Counter Example](counter.md) - Learn about state persistence
- [AI Actor Example](ai-actor.md) - Integrate AI capabilities
- [Mesh Example](mesh.md) - Distributed communication
- [Chat App Example](chat-app.md) - Multi-actor application
