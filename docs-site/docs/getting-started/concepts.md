# Core Concepts

Understanding the fundamental concepts of Aether.

## Actors

An **actor** is the fundamental unit of computation in Aether. Each actor:

- Has a unique **name/identifier**
- Processes messages **sequentially** (one at a time)
- Maintains **isolated state** (not shared with other actors)
- Can **send messages** to other actors
- Can **spawn child actors**
- Can **designate behavior** for the next message

### Actor Lifecycle

```
  ┌─────────┐
  │ Created │
  └────┬────┘
       │
       ▼
  ┌─────────┐     OnStart()
  │ Starting├──────────────▶ [Initialize state]
  └────┬────┘
       │
       ▼
  ┌─────────┐
  │ Running │◀──────┐
  └────┬────┘       │
       │            │
       │ HandleMessage()
       │            │
       └────────────┘
       │
       │ OnStop()
       ▼
  ┌─────────┐
  │ Stopped │
  └─────────┘
```

### Actor Interface

```go
type Actor interface {
    // Name returns the actor's unique identifier
    Name() string

    // HandleMessage processes incoming messages
    HandleMessage(ctx context.Context, sender string, message *Message) (*Message, error)

    // OnStart is called when the actor starts
    OnStart(ctx context.Context) error

    // OnStop is called when the actor stops
    OnStop(ctx context.Context) error
}
```

### BaseActor

The `BaseActor` provides a default implementation:

```go
type MyActor struct {
    *aether.BaseActor
}

func NewMyActor() *MyActor {
    return &MyActor{
        BaseActor: aether.NewBaseActor("my-actor"),
    }
}
```

## Messages

Actors communicate exclusively through **messages**. A message contains:

| Field | Description |
|-------|-------------|
| `Type` | Message type (request, response, event, etc.) |
| `Payload` | The message content (any type) |
| `Sender` | The sender's actor ID |
| `CorrelationID` | For request-response correlation |
| `Priority` | Message priority level |
| `Timestamp` | When the message was created |
| `Metadata` | Additional key-value metadata |

### Message Types

```go
const (
    MessageTypeRequest      // A request message
    MessageTypeResponse     // A response message
    MessageTypeEvent        // An event notification
    MessageTypeRPCRequest   // An RPC request
    MessageTypeRPCResponse  // An RPC response
    MessageTypeError        // An error message
)
```

### Creating Messages

```go
// Simple message
msg := aether.NewMessage(aether.MessageTypeRequest, "hello")

// With options
msg := aether.NewMessage(aether.MessageTypeRequest, payload).
    WithPriority(aether.PriorityHigh).
    WithMetadata("trace_id", "abc123")

// Response message
response := aether.NewResponse(request, result)

// RPC request
rpcMsg := aether.NewRPCRequest(sender, payload, correlationID)
```

## Mailboxes

Each actor has a **mailbox** - a queue that holds incoming messages. The actor processes messages from its mailbox one at a time.

```
                    Mailbox
┌─────────────────────────────────────┐
│  Msg1 │ Msg2 │ Msg3 │ Msg4 │ ...   │
└───────┴──────┴──────┴──────┴───────┘
         │
         ▼
    ┌─────────┐
    │  Actor  │ ──── HandleMessage()
    └─────────┘
```

### Mailbox Properties

- **Bounded capacity** - Prevents memory exhaustion
- **FIFO ordering** - Messages processed in order received
- **Priority support** - High-priority messages processed first

## Capabilities

**Capabilities** define what an actor is allowed to do. This is Aether's security model.

### Declaring Capabilities

```go
actor.Require(
    aether.CapabilityStateRead,       // Can read from state
    aether.CapabilityStateWrite,      // Can write to state
    aether.CapabilityNetworkOutbound, // Can make network calls
)
```

### Available Capabilities

| Capability | Description |
|------------|-------------|
| `NetworkOutbound` | Make outbound network connections |
| `NetworkInbound` | Accept inbound network connections |
| `StateRead` | Read from state storage |
| `StateWrite` | Write to state storage |
| `FSRead` | Read from filesystem |
| `FSWrite` | Write to filesystem |
| `ActorMessaging` | Send messages to other actors |
| `Log` | Write to logs |
| `Time` | Access system time |
| `Random` | Generate random numbers |
| `Environment` | Access environment variables |
| `HTTPClient` | HTTP client operations |
| `HTTPServer` | HTTP server operations |
| `ProcessSpawn` | Spawn child processes |

### Capability Enforcement

```go
// Check before performing an action
if !actor.Capabilities().Has(aether.CapabilityStateRead) {
    return aether.PermissionDenied("state read not allowed")
}
```

## State

Actors can maintain **persistent state** that survives restarts.

### StateHandle

```go
// Write state
err := actor.State().Write(ctx, "key", []byte("value"))

// Read state
value, err := actor.State().Read(ctx, "key")

// Check existence
exists, err := actor.State().Exists(ctx, "key")

// List keys with prefix
keys, err := actor.State().ListKeys(ctx, "prefix_")

// Delete a key
err := actor.State().Delete(ctx, "key")

// Clear all state
err := actor.State().Clear(ctx)
```

### State Isolation

Each actor's state is **isolated** - actors cannot access each other's state directly.

```
Actor A              Actor B              Actor C
┌─────────┐          ┌─────────┐          ┌─────────┐
│ State:  │          │ State:  │          │ State:  │
│ key1    │          │ keyA    │          │ keyX    │
│ key2    │          │ keyB    │          │ keyY    │
└─────────┘          └─────────┘          └─────────┘
    │                    │                    │
    ▼                    ▼                    ▼
  Isolated            Isolated            Isolated
```

## Supervisors

**Supervisors** manage child actors and handle failures.

### Supervision Strategies

```
                ┌──────────────┐
                │  Supervisor  │
                └──────┬───────┘
           ┌───────────┼───────────┐
           │           │           │
      ┌────▼───┐  ┌────▼───┐  ┌────▼───┐
      │ Actor1 │  │ Actor2 │  │ Actor3 │
      └────────┘  └────────┘  └────────┘
           │
           │ Error!
           ▼
      ┌─────────┐
      │Restart? │
      └────┬────┘
           │
    ┌──────┼──────┐
    │      │      │
  OneForOne OneForAll RestForOne
```

### Strategies

1. **OneForOne** - Restart only the failed actor
2. **OneForAll** - Restart all children when one fails
3. **RestForOne** - Restart the failed actor and all started after it

## Mesh Network

Aether supports **distributed actors** across a mesh network.

### Location Transparency

Actors communicate the same way regardless of location:

```go
// Local actor
actor.Send(ctx, "local-actor", message)

// Remote actor (same API!)
actor.Send(ctx, "remote-actor@node-2", message)
```

### Mesh Topology

```
     Node A                Node B                Node C
    ┌──────┐              ┌──────┐              ┌──────┐
    │Actor1│◄────────────▶│Actor2│◄────────────▶│Actor3│
    └──────┘              └──────┘              └──────┘
         ▲                    │                    │
         │                    │                    │
         └────────────────────┴────────────────────┘
                        Mesh Network
                      (QUIC + mTLS)
```

## Error Handling

Aether follows the **"Let It Crash"** philosophy.

### Error Types

```go
// Predefined errors
err := aether.InvalidArgument("field required")
err := aether.NotFound("actor not found")
err := aether.Timeout("operation timed out")
err := aether.PermissionDenied("capability missing")
err := aether.Internal("unexpected error")

// Custom error
err := aether.NewError("CUSTOM_ERROR", "description", cause)
```

### Error Propagation

```go
func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    // Return error to let supervisor handle it
    if err := doSomething(); err != nil {
        return nil, err  // Supervisor may restart this actor
    }

    // Return error message to sender
    if invalidInput {
        return aether.NewMessage(aether.MessageTypeError, "invalid input"), nil
    }

    return response, nil
}
```

## Next Steps

- [Examples](../examples/overview.md) - See these concepts in action
- [Architecture](../architecture/overview.md) - Learn about the system design
- [SDK Reference](../sdks/overview.md) - Detailed API documentation
