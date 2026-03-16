# Introduction

Welcome to Project Aether, a distributed actor framework for building scalable, resilient applications.

## What is the Actor Model?

The actor model is a mathematical model for concurrent computation that treats "actors" as the universal primitives of concurrent digital computation. In response to a message that it receives, an actor can:

1. **Send messages** to other actors
2. **Create new actors**
3. **Designate behavior** for the next message it receives

### Key Principles

#### Isolation
Each actor is isolated from others - they don't share state. The only way to interact with an actor is by sending messages.

```go
// Actors don't share memory
// Communication is ONLY via messages
actor.Send(ctx, "target-actor", message)
```

#### Location Transparency
Actors can be local or remote - the code doesn't need to know where another actor is located.

```go
// Same API whether actor is local or on another node
actor.Send(ctx, "actor@remote-node", message)
```

#### Let It Crash
Instead of defensive programming, actors are designed to fail and be restarted by supervisors.

```go
// Supervisor automatically restarts failed actors
supervisor := NewSupervisor("main-supervisor")
supervisor.Spawn(childActor)
```

## Why Aether?

### Performance

| Operation | Latency |
|-----------|---------|
| Actor spawn | < 1µs |
| Local message | < 10µs |
| Remote message | < 1ms |

### Scalability

- **Horizontal**: Add more nodes to the mesh
- **Vertical**: Run millions of actors per node
- **Elastic**: Auto-scale based on load

### Reliability

- **Self-healing**: Automatic recovery from failures
- **Persistence**: State survives restarts
- **Mesh networking**: Automatic failover

### Security

- **Capability-based**: Fine-grained permissions
- **Sandboxed**: Isolated execution
- **mTLS**: Encrypted mesh communication

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                           │
├─────────────────────────────────────────────────────────────┤
│                      SDK Layer (Go/Python/JS)                │
├─────────────────────────────────────────────────────────────┤
│                      Actor Runtime                           │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  │
│  │Scheduler│  │Mailbox    │  │Supervisor │  │Registry   │  │
│  └─────────┘  └───────────┘  └───────────┘  └───────────┘  │
├─────────────────────────────────────────────────────────────┤
│                      Core Services                           │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  │
│  │State    │  │Security   │  │Mesh       │  │Observability│ │
│  └─────────┘  └───────────┘  └───────────┘  └───────────┘  │
├─────────────────────────────────────────────────────────────┤
│                      Infrastructure                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │WASM Runtime │  │Firecracker  │  │Network      │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

## Use Cases

### Microservices

Replace traditional microservices with actors for better isolation and scalability.

```go
// Order service as actors
orderActor := NewOrderActor()
paymentActor := NewPaymentActor()
shippingActor := NewShippingActor()

// Communication via messages
orderActor.Send(ctx, "payment", NewPaymentRequest(order))
```

### Real-time Systems

Build event-driven systems that react to real-time data.

```go
// Stream processing actors
sourceActor := NewSourceActor()
transformActor := NewTransformActor()
sinkActor := NewSinkActor()

// Pipeline via message passing
sourceActor.StreamTo("transform")
transformActor.StreamTo("sink")
```

### Game Servers

Manage game state with isolated actors.

```go
// Each player is an actor
playerActor := NewPlayerActor(playerID)
roomActor := NewRoomActor(roomID)

// Game logic via messages
playerActor.Send(ctx, "room-1", NewMoveCommand(x, y))
```

### AI/ML Workloads

Distribute inference across actors.

```go
// AI inference actors
preprocessor := NewPreprocessorActor()
inference := NewInferenceActor(modelID)
postprocessor := NewPostprocessorActor()

// Pipeline inference
preprocessor.Send(ctx, "inference", input)
```

## Next Steps

- [Install Aether](installation.md)
- [Quick Start Guide](quickstart.md)
- [Core Concepts](concepts.md)
