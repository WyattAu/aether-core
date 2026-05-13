# Aether Go SDK

[![Go Reference](https://pkg.go.dev/badge/github.com/WyattAu/aether-core/sdks/go/aether.svg)](https://pkg.go.dev/github.com/WyattAu/aether-core/sdks/go/aether)
[![Go Report Card](https://goreportcard.com/badge/github.com/WyattAu/aether-core/sdks/go/aether)](https://goreportcard.com/report/github.com/WyattAu/aether-core/sdks/go/aether)

The official Go SDK for [Project Aether](https://github.com/WyattAu/aether-core) - a distributed actor framework for building scalable, resilient applications.

## Features

- **Actor Model**: Build concurrent applications using the actor pattern
- **Persistent State**: State survives actor restarts with built-in storage
- **Capability-based Security**: Fine-grained permissions for actors
- **Mesh Communication**: Distributed messaging across nodes
- **Type-safe Messages**: Strongly typed message handling
- **Zero-panic Design**: Production-ready error handling

## Installation

```bash
go get github.com/WyattAu/aether-core/sdks/go/aether
```

## Quick Start

### Hello World Actor

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

type HelloActor struct {
    *aether.BaseActor
}

func NewHelloActor() *HelloActor {
    return &HelloActor{
        BaseActor: aether.NewBaseActor("hello-actor"),
    }
}

func (a *HelloActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    switch payload := msg.Payload.(type) {
    case string:
        if payload == "ping" {
            return aether.NewResponse(msg, "pong"), nil
        }
        return aether.NewResponse(msg, fmt.Sprintf("Hello, %s!", payload)), nil
    }
    return nil, nil
}

func main() {
    actor := NewHelloActor()
    actor.Require(aether.CapabilityActorMessaging, aether.CapabilityLog)

    ctx, cancel := context.WithCancel(context.Background())
    defer cancel()

    sigChan := make(chan os.Signal, 1)
    signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
    go func() {
        <-sigChan
        actor.Stop()
        cancel()
    }()

    log.Printf("Starting %s...", actor.Name())
    if err := actor.Run(ctx); err != nil && err != context.Canceled {
        log.Fatalf("Actor error: %v", err)
    }
}
```

## Core Concepts

### Actor

An actor is the fundamental unit of computation in Aether. Each actor:
- Has a unique name/ID
- Processes messages sequentially
- Maintains isolated state
- Can spawn child actors
- Can send messages to other actors

```go
type Actor interface {
    Name() string
    HandleMessage(ctx context.Context, sender string, message *Message) (*Message, error)
    OnStart(ctx context.Context) error
    OnStop(ctx context.Context) error
}
```

### BaseActor

`BaseActor` provides a default implementation that you can embed:

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

### Messages

Messages are how actors communicate:

```go
// Create a simple message
msg := aether.NewMessage(aether.MessageTypeRequest, "hello")

// Create with priority
msg := aether.NewMessage(aether.MessageTypeRequest, payload).
    WithPriority(aether.PriorityHigh).
    WithMetadata("trace_id", "abc123")

// Serialize to JSON
data, err := msg.ToJSON()

// Deserialize from JSON
msg, err := aether.FromJSON(data)
```

### Capabilities

Capabilities define what an actor can do:

```go
actor.Require(
    aether.CapabilityStateRead,      // Read from state storage
    aether.CapabilityStateWrite,     // Write to state storage
    aether.CapabilityNetworkOutbound, // Make outbound network calls
    aether.CapabilityActorMessaging,  // Send messages to other actors
    aether.CapabilityLog,             // Write to logs
    aether.CapabilityTime,            // Access system time
)
```

Available capabilities:
- `CapabilityNetworkOutbound` - Outbound network connections
- `CapabilityNetworkInbound` - Inbound network connections
- `CapabilityStateRead` - Read from state storage
- `CapabilityStateWrite` - Write to state storage
- `CapabilityFSRead` - Filesystem read operations
- `CapabilityFSWrite` - Filesystem write operations
- `CapabilityActorMessaging` - Send messages to actors
- `CapabilityLog` - Write to logs
- `CapabilityTime` - Access system time
- `CapabilityRandom` - Generate random numbers
- `CapabilityEnvironment` - Access environment variables
- `CapabilityHTTPClient` - HTTP client operations
- `CapabilityHTTPServer` - HTTP server operations
- `CapabilityProcessSpawn` - Spawn child processes

### State Management

Actors can persist state that survives restarts:

```go
// Write state
err := actor.State().Write(ctx, "counter", []byte("42"))

// Read state
value, err := actor.State().Read(ctx, "counter")

// Check if key exists
exists, err := actor.State().Exists(ctx, "counter")

// List keys with prefix
keys, err := actor.State().ListKeys(ctx, "user_")

// Delete a key
err := actor.State().Delete(ctx, "counter")

// Clear all state
err := actor.State().Clear(ctx)
```

### RPC Calls

Make request-response calls to other actors:

```go
// Call with timeout
response, err := actor.Call(ctx, "target-actor", requestData, 5*time.Second)
if err != nil {
    // Handle timeout or error
}
```

## Examples

The SDK includes comprehensive examples:

### 1. Hello World
Basic actor that responds to greeting messages.

```bash
go run ./examples/hello_actor
```

### 2. Stateful Counter
Demonstrates persistent state that survives actor restarts.

```bash
go run ./examples/counter_actor
```

### 3. AI-Powered Actor
Shows how to integrate AI capabilities with the actor model.

```bash
go run ./examples/ai_actor
```

### 4. Mesh Communication
Demonstrates distributed messaging across mesh nodes.

```bash
go run ./examples/mesh_actor
```

### 5. Chat Application
A complete multi-actor chat room application.

```bash
go run ./examples/chat_app
```

## API Reference

### Message Types

| Type | Description |
|------|-------------|
| `MessageTypeRequest` | A request message |
| `MessageTypeResponse` | A response message |
| `MessageTypeEvent` | An event notification |
| `MessageTypeRPCRequest` | An RPC request |
| `MessageTypeRPCResponse` | An RPC response |
| `MessageTypeError` | An error message |

### Message Priority

| Priority | Description |
|----------|-------------|
| `PriorityLow` | Low priority |
| `PriorityNormal` | Normal priority (default) |
| `PriorityHigh` | High priority |
| `PriorityCritical` | Critical priority |

### Error Types

```go
// Create an error
err := aether.NewError(aether.ErrCodeInvalidArgument, "value cannot be negative", nil)

// Predefined error helpers
err := aether.InvalidArgument("field is required")
err := aether.NotFound("actor not found")
err := aether.Timeout("operation timed out")
err := aether.PermissionDenied("insufficient capabilities")
```

## Best Practices

### 1. Always Use Capabilities

Declare required capabilities upfront:

```go
func NewSecureActor() *SecureActor {
    a := &SecureActor{BaseActor: aether.NewBaseActor("secure")}
    a.Require(
        aether.CapabilityStateRead,
        aether.CapabilityStateWrite,
        aether.CapabilityNetworkOutbound,
    )
    return a
}
```

### 2. Handle Context Cancellation

Always respect context cancellation:

```go
func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    select {
    case <-ctx.Done():
        return nil, ctx.Err()
    default:
        // Process message
    }
    // ...
}
```

### 3. Use Typed Payloads

Define clear payload structures:

```go
type CreateUserRequest struct {
    Name  string `json:"name"`
    Email string `json:"email"`
}

type CreateUserResponse struct {
    ID    string `json:"id"`
    Error string `json:"error,omitempty"`
}
```

### 4. Clean Up in OnStop

Release resources when stopping:

```go
func (a *MyActor) OnStop(ctx context.Context) error {
    // Close connections
    // Flush buffers
    // Save state
    return nil
}
```

## Testing

```go
func TestMyActor(t *testing.T) {
    actor := NewMyActor()
    ctx := context.Background()

    // Start the actor
    go actor.Run(ctx)
    defer actor.Stop()

    // Wait for startup
    time.Sleep(100 * time.Millisecond)

    // Test message handling
    msg := aether.NewMessage(aether.MessageTypeRequest, "test")
    response, err := actor.HandleMessage(ctx, "test-sender", msg)

    assert.NoError(t, err)
    assert.NotNil(t, response)
}
```

## Versioning

The SDK follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

Current version: `v0.1.0`

## Contributing

We welcome contributions! Please see the [Contributing Guide](../../CONTRIBUTING.md) for details.

## License

Licensed under the [Apache 2.0 License](../../LICENSE).

## Links

- [Project Aether](https://github.com/WyattAu/aether-core)
- [Documentation](https://aether.dev/docs)
- [Examples](./examples)
- [API Reference](https://pkg.go.dev/github.com/WyattAu/aether-core/sdks/go/aether)
