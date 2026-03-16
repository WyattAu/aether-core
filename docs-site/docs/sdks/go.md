# Go SDK

The Go SDK provides a idiomatic Go interface for building Aether actors.

## Installation

```bash
go get github.com/WyattAu/aether-core/sdks/go
```

## Quick Start

```go
package main

import (
    "fmt"
    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type HelloActor struct {
    aether.Actor
}

func (a *HelloActor) HandleMessage(sender string, msg aether.Message) (aether.Message, error) {
    payload, ok := msg.Payload.(string)
    if !ok {
        return aether.Message{}, fmt.Errorf("invalid payload")
    }
    
    return aether.Message{
        Type:    aether.MessageTypeResponse,
        Payload: fmt.Sprintf("Hello, %s!", payload),
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
    
    actor.Run()
}
```

## Core Types

### Actor

The `Actor` struct is the base for all Aether actors:

```go
type Actor struct {
    Name        string
    Capabilities CapabilitySet
    State       *State
}

// Lifecycle methods
func (a *Actor) Start() error
func (a *Actor) Stop() error
func (a *Actor) Run()

// Capability management
func (a *Actor) Require(caps ...string)
func (a *Actor) Has(cap string) bool
```

### Message

Messages are the primary communication mechanism:

```go
type Message struct {
    ID        string
    Type      MessageType
    Sender    string
    Target    string
    Payload   interface{}
    Timestamp time.Time
}

type MessageType int

const (
    MessageTypeRequest    MessageType = iota
    MessageTypeResponse
    MessageTypeEvent
    MessageTypeRPCRequest
    MessageTypeRPCResponse
)
```

### Capability

Capabilities control what an actor can do:

```go
// Standard capabilities
const (
    CapStateRead      = "STATE_READ"
    CapStateWrite     = "STATE_WRITE"
    CapNetworkOutbound = "NETWORK_OUTBOUND"
    CapActorMessaging = "ACTOR_MESSAGING"
    CapLog           = "LOG"
    CapTime          = "TIME"
    CapRandom        = "RANDOM"
    CapAIUse         = "AI_USE"
)
```

### State

State provides persistent storage:

```go
type State struct {}

func (s *State) Read(key string) ([]byte, error)
func (s *State) Write(key string, value []byte) error
func (s *State) Delete(key string) error
func (s *State) ListKeys(prefix string) ([]string, error)
func (s *State) Exists(key string) (bool, error)
func (s *State) Clear() error
```

## Examples

### Counter Actor

```go
type CounterActor struct {
    aether.Actor
    count int
    stateKey string
}

func (a *CounterActor) OnStart() error {
    a.stateKey = fmt.Sprintf("counter_%s", a.Name)
    
    // Load persisted count
    data, err := a.State.Read(a.stateKey)
    if err == nil {
        a.count = bytesToInt(data)
    }
    return nil
}

func (a *CounterActor) HandleMessage(sender string, msg aether.Message) (aether.Message, error) {
    payload, _ := msg.Payload.(map[string]interface{})
    action := payload["action"].(string)
    
    switch action {
    case "increment":
        a.count++
        a.saveState()
        return aether.Message{
            Type: aether.MessageTypeResponse,
            Payload: map[string]interface{}{"count": a.count},
        }, nil
    case "get":
        return aether.Message{
            Type: aether.MessageTypeResponse,
            Payload: map[string]interface{}{"count": a.count},
        }, nil
    }
    
    return aether.Message{}, fmt.Errorf("unknown action: %s", action)
}

func (a *CounterActor) saveState() {
    a.State.Write(a.stateKey, intToBytes(a.count))
}
```

## Error Handling

```go
import "github.com/WyattAu/aether-core/sdks/go/aether"

// Use structured errors
return aether.Message{}, aether.ErrorInternal("something went wrong")
return aether.Message{}, aether.ErrorStorageRead("failed to read state")
return aether.Message{}, aether.ErrorStorageWrite("failed to write state")
```

## Best Practices

1. **Always declare capabilities**: Use `Require()` before `Start()`
2. **Handle errors gracefully**: Return errors, don't panic
3. **Persist state**: Save state changes immediately after modification
4. **Use typed payloads**: Define structs for message payloads
5. **Clean shutdown**: Always call `Stop()` when done

## API Reference

Full API documentation is available at [pkg.go.dev](https://pkg.go.dev/github.com/WyattAu/aether-core/sdks/go).
