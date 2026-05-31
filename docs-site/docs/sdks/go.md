# Go SDK

The Go SDK provides an idiomatic Go interface for building Aether actors using the `BaseActor` embedding pattern with `context.Context` throughout.

## Installation

```bash
go get github.com/WyattAu/aether-core/sdks/go
```

## Quick Start

```go
package main

import (
    "context"
    "fmt"
    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type HelloActor struct {
    *aether.BaseActor
}

func (a *HelloActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    payload, ok := msg.Payload.(string)
    if !ok {
        return nil, aether.InvalidMessage("expected string payload", nil)
    }

    return aether.NewResponse(msg, fmt.Sprintf("Hello, %s!", payload)), nil
}

func main() {
    actor := aether.NewBaseActor("hello-actor")
    hello := &HelloActor{BaseActor: actor}
    hello.Require(aether.CapabilityNetworkOutbound, aether.CapabilityLog)

    ctx := context.Background()
    if err := hello.Run(ctx); err != nil {
        fmt.Printf("actor error: %v\n", err)
    }
}
```

## Core Types

### BaseActor

`BaseActor` is the embeddable base struct implementing the `Actor` interface. Override methods by defining them on your embedding struct:

```go
// Actor is the interface all actors implement.
type Actor interface {
    Name() string
    HandleMessage(ctx context.Context, sender string, message *Message) (*Message, error)
    OnStart(ctx context.Context) error
    OnStop(ctx context.Context) error
}

// BaseActor provides default no-op implementations.
type BaseActor struct {
    // Function hooks for optional override without full struct embedding.
    OnStartFunc        func(ctx context.Context) error
    OnStopFunc         func(ctx context.Context) error
    HandleMessageFunc  func(ctx context.Context, sender string, msg *Message) (*Message, error)
}

func NewBaseActor(name string) *BaseActor
func (a *BaseActor) Require(caps ...Capability)
func (a *BaseActor) Capabilities() *CapabilitySet
func (a *BaseActor) State() *StateHandle
func (a *BaseActor) Send(ctx context.Context, target string, msg *Message) error
func (a *BaseActor) Call(ctx context.Context, target string, request *Message, timeout time.Duration) (any, error)
func (a *BaseActor) Run(ctx context.Context) error
func (a *BaseActor) Stop()
func (a *BaseActor) IsRunning() bool
```

### Message

Messages are passed as pointers. Use constructor functions for creation:

```go
type Message struct {
    Type         MessageType
    Payload      any
    Sender       string
    CorrelationID string
    Priority     Priority
    Timestamp    time.Time
    Metadata     map[string]string
}

type MessageType string

const (
    MessageTypeRequest     MessageType = "request"
    MessageTypeResponse    MessageType = "response"
    MessageTypeEvent      MessageType = "event"
    MessageTypeRPCRequest  MessageType = "rpc_request"
    MessageTypeRPCResponse MessageType = "rpc_response"
    MessageTypeError      MessageType = "error"
)

// Constructors
func NewMessage(msgType MessageType, payload any) *Message
func NewResponse(request *Message, payload any) *Message
func NewRPCRequest(sender string, payload any, correlationID string) *Message
func NewRPCResponse(request *Message, payload any) *Message
```

### Capability

Capabilities are typed constants, not strings:

```go
type Capability int

const (
    CapabilityNetworkOutbound Capability = iota
    CapabilityNetworkInbound
    CapabilityActorMessaging
    CapabilityStateRead
    CapabilityStateWrite
    CapabilityLog
    CapabilityTime
    CapabilityRandom
    CapabilityFSRead
    CapabilityFSWrite
    CapabilityEnvironment
    CapabilityHTTPClient
    CapabilityHTTPServer
    CapabilityProcessSpawn
)

func NewCapabilitySet(caps ...Capability) *CapabilitySet
func (cs *CapabilitySet) Has(cap Capability) bool
func (cs *CapabilitySet) HasNetwork() bool
func (cs *CapabilitySet) HasState() bool
```

### StateHandle

All state methods require `context.Context`:

```go
type StateHandle struct{}

func (s *StateHandle) Read(ctx context.Context, key string) ([]byte, error)
func (s *StateHandle) Write(ctx context.Context, key string, value []byte) error
func (s *StateHandle) Delete(ctx context.Context, key string) error
func (s *StateHandle) ListKeys(ctx context.Context, prefix string) ([]string, error)
func (s *StateHandle) Exists(ctx context.Context, key string) (bool, error)
func (s *StateHandle) Clear(ctx context.Context) error
```

## Examples

### Counter Actor

```go
type CounterActor struct {
    *aether.BaseActor
    count    int
    stateKey string
}

func (a *CounterActor) OnStart(ctx context.Context) error {
    a.stateKey = fmt.Sprintf("counter_%s", a.Name())

    data, err := a.State().Read(ctx, a.stateKey)
    if err == nil {
        a.count = int(binary.BigEndian.Uint64(data))
    }
    return nil
}

func (a *CounterActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    payload, _ := msg.Payload.(map[string]any)
    action, _ := payload["action"].(string)

    switch action {
    case "increment":
        a.count++
        a.saveState(ctx)
        return aether.NewResponse(msg, map[string]any{"count": a.count}), nil
    case "get":
        return aether.NewResponse(msg, map[string]any{"count": a.count}), nil
    default:
        return nil, fmt.Errorf("unknown action: %s", action)
    }
}

func (a *CounterActor) saveState(ctx context.Context) {
    buf := make([]byte, 8)
    binary.BigEndian.PutUint64(buf, uint64(a.count))
    a.State().Write(ctx, a.stateKey, buf)
}
```

## Error Handling

```go
import "github.com/WyattAu/aether-core/sdks/go/aether"

// Structured errors with code, message, and optional cause
return nil, aether.InternalError("something went wrong", err)
return nil, aether.StateError("read", err)

// Error type checking
var e *aether.Error
if errors.As(err, &e) && e.IsCapabilityDenied() {
    // handle missing capability
}
```

## Sub-Packages

| Package | Description |
|---------|-------------|
| `aether/streaming` | Stream processing, tumbling/sliding/session windows, backpressure, batching, zero-copy buffers |
| `aether/resilience` | Circuit breaker, retry with backoff, rate limiter, bulkhead, health checks, resilient executor |
| `aether/validation` | Fluent validators, sanitization, schema validation |
| `aether/workflow` | Saga orchestration, state machines, human tasks |
| `aether.Client` | HTTP client for the Aether server REST API with functional options |

## Best Practices

1. **Embed `*aether.BaseActor`**: Override only the methods you need; default implementations are no-op
2. **Always declare capabilities**: Call `Require()` before `Run()`
3. **Pass `context.Context`**: All methods accept context as the first parameter
4. **Use message constructors**: `NewResponse`, `NewMessage` instead of struct literals
5. **Persist state immediately**: Call `State().Write()` after state mutations
6. **Use typed payloads**: Define structs for message payloads instead of `map[string]any`
7. **Clean shutdown**: The `Run()` method blocks; call `Stop()` from a signal handler

## API Reference

Full API documentation is available at [pkg.go.dev](https://pkg.go.dev/github.com/WyattAu/aether-core/sdks/go).
