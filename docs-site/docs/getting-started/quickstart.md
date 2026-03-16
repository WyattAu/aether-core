# Quick Start

Build your first Aether actor in 5 minutes.

## Prerequisites

- Go 1.21+ (or Python 3.11+ or Node.js 18+)
- Aether SDK installed

## Create Your First Actor

### Step 1: Initialize Project

=== "Go"

    ```bash
    mkdir my-actor && cd my-actor
    go mod init my-actor
    go get github.com/WyattAu/aether-core/sdks/go/aether
    ```

=== "Python"

    ```bash
    mkdir my-actor && cd my-actor
    python -m venv venv
    source venv/bin/activate
    pip install aether-sdk
    ```

=== "JavaScript"

    ```bash
    mkdir my-actor && cd my-actor
    npm init -y
    npm install @aether/sdk
    ```

### Step 2: Create the Actor

Create a file named `main.go` (or `main.py` / `index.js`):

=== "Go"

    ```go
    package main

    import (
        "context"
        "fmt"
        "log"
        "os"
        "os/signal"
        "syscall"

        "github.com/WyattAu/aether-core/sdks/go/aether"
    )

    // GreeterActor responds to greeting messages
    type GreeterActor struct {
        *aether.BaseActor
    }

    func NewGreeterActor() *GreeterActor {
        return &GreeterActor{
            BaseActor: aether.NewBaseActor("greeter"),
        }
    }

    func (a *GreeterActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
        switch payload := msg.Payload.(type) {
        case string:
            if payload == "ping" {
                log.Printf("Received ping from %s", sender)
                return aether.NewResponse(msg, "pong"), nil
            }
            greeting := fmt.Sprintf("Hello, %s!", payload)
            log.Printf("Greeting %s: %s", sender, greeting)
            return aether.NewResponse(msg, greeting), nil
        default:
            return aether.NewResponse(msg, map[string]any{
                "error": "expected string payload",
            }), nil
        }
    }

    func main() {
        actor := NewGreeterActor()
        actor.Require(aether.CapabilityActorMessaging, aether.CapabilityLog)

        ctx, cancel := context.WithCancel(context.Background())
        defer cancel()

        // Handle shutdown gracefully
        sigChan := make(chan os.Signal, 1)
        signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
        go func() {
            <-sigChan
            log.Println("Shutting down...")
            actor.Stop()
            cancel()
        }()

        log.Printf("Starting actor: %s", actor.Name())
        if err := actor.Run(ctx); err != nil && err != context.Canceled {
            log.Fatalf("Actor error: %v", err)
        }
        log.Println("Actor stopped")
    }
    ```

=== "Python"

    ```python
    import asyncio
    import signal
    import logging
    from aether_sdk import Actor, Message, MessageType

    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)

    class GreeterActor(Actor):
        def __init__(self):
            super().__init__("greeter")
            self.require("ACTOR_MESSAGING", "LOG")

        async def handle_message(self, sender: str, message: Message) -> Message:
            if message.payload == "ping":
                logger.info(f"Received ping from {sender}")
                return Message.response("pong")
            
            greeting = f"Hello, {message.payload}!"
            logger.info(f"Greeting {sender}: {greeting}")
            return Message.response(greeting)

    async def main():
        actor = GreeterActor()
        
        def shutdown():
            logger.info("Shutting down...")
            asyncio.create_task(actor.stop())

        loop = asyncio.get_event_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, shutdown)

        logger.info(f"Starting actor: {actor.name}")
        await actor.start()
        await actor.run()

    if __name__ == "__main__":
        asyncio.run(main())
    ```

=== "JavaScript"

    ```javascript
    import { Actor, Message } from '@aether/sdk';
    import logger from '@aether/sdk/logging';

    class GreeterActor extends Actor {
        constructor() {
            super('greeter');
            this.require('ACTOR_MESSAGING', 'LOG');
        }

        async handleMessage(sender, message) {
            if (message.payload === 'ping') {
                logger.info(`Received ping from ${sender}`);
                return Message.response('pong');
            }

            const greeting = `Hello, ${message.payload}!`;
            logger.info(`Greeting ${sender}: ${greeting}`);
            return Message.response(greeting);
        }
    }

    async function main() {
        const actor = new GreeterActor();

        // Handle shutdown
        process.on('SIGINT', async () => {
            logger.info('Shutting down...');
            await actor.stop();
            process.exit(0);
        });

        logger.info(`Starting actor: ${actor.name}`);
        await actor.start();
        await actor.run();
    }

    main().catch(console.error);
    ```

### Step 3: Run the Actor

=== "Go"

    ```bash
    go run main.go
    ```

=== "Python"

    ```bash
    python main.py
    ```

=== "JavaScript"

    ```bash
    node index.js
    ```

You should see:

```
2026/03/16 10:00:00 Starting actor: greeter
```

## Add State Persistence

Let's make our actor stateful by counting greetings:

=== "Go"

    ```go
    type GreeterActor struct {
        *aether.BaseActor
        greetingCount int64
    }

    func (a *GreeterActor) OnStart(ctx context.Context) error {
        // Load persisted count
        data, _ := a.State().Read(ctx, "greeting_count")
        if data != nil {
            a.greetingCount = int64(binary.BigEndian.Uint64(data))
        }
        log.Printf("Loaded greeting count: %d", a.greetingCount)
        return nil
    }

    func (a *GreeterActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
        a.greetingCount++
        
        // Persist the count
        buf := make([]byte, 8)
        binary.BigEndian.PutUint64(buf, uint64(a.greetingCount))
        a.State().Write(ctx, "greeting_count", buf)

        greeting := fmt.Sprintf("Hello, %v! (greeting #%d)", msg.Payload, a.greetingCount)
        return aether.NewResponse(msg, greeting), nil
    }
    ```

## Send Messages Between Actors

Create two actors that communicate:

```go
// Producer actor
producer := aether.NewBaseActor("producer")
producer.Require(aether.CapabilityActorMessaging)

// Consumer actor
type ConsumerActor struct {
    *aether.BaseActor
}

func (a *ConsumerActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    log.Printf("Consumer received: %v from %s", msg.Payload, sender)
    return nil, nil
}

// Producer sends messages
producer.Send(ctx, "consumer", aether.NewMessage(aether.MessageTypeRequest, "hello"))
```

## Use RPC for Request-Response

```go
// Make an RPC call with timeout
response, err := actor.Call(ctx, "service-actor", map[string]any{
    "action": "get_data",
    "id":     "123",
}, 5*time.Second)

if err != nil {
    log.Printf("RPC failed: %v", err)
    return
}

log.Printf("Response: %v", response)
```

## Next Steps

- [Core Concepts](concepts.md) - Deep dive into actors, messages, and capabilities
- [Examples](../examples/overview.md) - More example applications
- [SDK Reference](../sdks/overview.md) - Detailed SDK documentation
