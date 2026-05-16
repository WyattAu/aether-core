# SDKs Overview

Aether provides SDKs for multiple programming languages, all sharing the same core concepts and API patterns.

## Available SDKs

| SDK | Status | Package | Documentation |
|-----|--------|---------|---------------|
| Go | [DONE] Stable | `github.com/WyattAu/aether-core/sdks/go/aether` | [Go SDK](go.md) |
| Python | [DONE] Stable | `aether-sdk` | [Python SDK](python.md) |
| JavaScript | [DONE] Stable | `@aether/sdk` | [JavaScript SDK](javascript.md) |
| Rust | [DONE] Stable | `aether` | [Rust SDK](rust.md) |

## Common API Patterns

All SDKs follow the same design patterns:

### Actor Definition

=== "Go"

    ```go
    type MyActor struct {
        *aether.BaseActor
    }

    func NewMyActor() *MyActor {
        return &MyActor{
            BaseActor: aether.NewBaseActor("my-actor"),
        }
    }

    func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
        // Handle message
        return response, nil
    }
    ```

=== "Python"

    ```python
    from aether_sdk import Actor, Message

    class MyActor(Actor):
        def __init__(self):
            super().__init__("my-actor")

        async def handle_message(self, sender: str, message: Message) -> Message:
            # Handle message
            return Message.response(result)
    ```

=== "JavaScript"

    ```javascript
    import { Actor, Message } from '@aether/sdk';

    class MyActor extends Actor {
        constructor() {
            super('my-actor');
        }

        async handleMessage(sender, message) {
            // Handle message
            return Message.response(result);
        }
    }
    ```

=== "Rust"

    ```rust
    use aether::{Actor, Message, Context};

    struct MyActor;

    #[async_trait]
    impl Actor for MyActor {
        async fn handle_message(&mut self, ctx: &Context, sender: &str, msg: Message) -> Result<Option<Message>> {
            // Handle message
            Ok(Some(response))
        }
    }
    ```

### Capabilities

All SDKs use the same capability model:

=== "Go"

    ```go
    actor.Require(
        aether.CapabilityStateRead,
        aether.CapabilityStateWrite,
        aether.CapabilityNetworkOutbound,
    )
    ```

=== "Python"

    ```python
    actor.require(
        "STATE_READ",
        "STATE_WRITE",
        "NETWORK_OUTBOUND",
    )
    ```

=== "JavaScript"

    ```javascript
    actor.require(
        'STATE_READ',
        'STATE_WRITE',
        'NETWORK_OUTBOUND',
    );
    ```

=== "Rust"

    ```rust
    actor.require(&[
        Capability::StateRead,
        Capability::StateWrite,
        Capability::NetworkOutbound,
    ]);
    ```

### State Management

=== "Go"

    ```go
    // Write
    actor.State().Write(ctx, "key", []byte("value"))

    // Read
    value, _ := actor.State().Read(ctx, "key")
    ```

=== "Python"

    ```python
    # Write
    await actor.state.write("key", b"value")

    # Read
    value = await actor.state.read("key")
    ```

=== "JavaScript"

    ```javascript
    // Write
    await actor.state.write('key', Buffer.from('value'));

    // Read
    const value = await actor.state.read('key');
    ```

=== "Rust"

    ```rust
    // Write
    actor.state().write("key", b"value").await?;

    // Read
    let value = actor.state().read("key").await?;
    ```

### Messaging

=== "Go"

    ```go
    // Fire-and-forget
    actor.Send(ctx, "target", message)

    // Request-response
    response, _ := actor.Call(ctx, "target", payload, timeout)
    ```

=== "Python"

    ```python
    # Fire-and-forget
    await actor.send("target", message)

    # Request-response
    response = await actor.call("target", payload, timeout)
    ```

=== "JavaScript"

    ```javascript
    // Fire-and-forget
    await actor.send('target', message);

    // Request-response
    const response = await actor.call('target', payload, timeout);
    ```

=== "Rust"

    ```rust
    // Fire-and-forget
    actor.send("target", message).await?;

    // Request-response
    let response = actor.call("target", payload, timeout).await?;
    ```

## Version Compatibility

| Aether Core | Go SDK | Python SDK | JS SDK | Rust SDK |
|-------------|--------|------------|--------|----------|
| 1.3.0 | 0.1.0 | 0.1.0 | 0.1.0 | 0.1.0 |
| 1.2.0 | - | - | - | - |

## Choosing an SDK

### Go SDK

[DONE] **Best for:**
- High-performance services
- Microservices architecture
- Infrastructure tooling
- Production backends

### Python SDK

[DONE] **Best for:**
- AI/ML applications
- Data processing
- Rapid prototyping
- Scripting and automation

### JavaScript SDK

[DONE] **Best for:**
- Web applications
- Node.js backends
- Serverless functions
- Real-time applications

### Rust SDK

[DONE] **Best for:**
- System programming
- Maximum performance
- Safety-critical systems
- WASM compilation

## Next Steps

- [Go SDK Documentation](go.md)
- [Python SDK Documentation](python.md)
- [JavaScript SDK Documentation](javascript.md)
- [Rust SDK Documentation](rust.md)
