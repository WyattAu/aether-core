# Actor Model Architecture

Deep dive into the Aether actor model implementation.

## Actor Fundamentals

### What is an Actor?

An actor is the fundamental unit of computation in Aether. It:

1. **Processes messages sequentially** - One message at a time
2. **Maintains isolated state** - No shared memory
3. **Communicates via messages** - Async message passing
4. **Has a unique identity** - Name/address for routing

### Actor Lifecycle

```
    ┌───────────────────────────────────────────┐
    │                                           │
    │   ┌─────────┐                             │
    │   │ Created │                             │
    │   └────┬────┘                             │
    │        │ spawn()                          │
    │        ▼                                  │
    │   ┌─────────┐                             │
    │   │Starting │◀─────────────┐              │
    │   └────┬────┘              │              │
    │        │ OnStart()         │ restart()    │
    │        ▼                  │              │
    │   ┌─────────┐             │              │
    │   │ Running │─────────────┤              │
    │   └────┬────┘             │              │
    │        │ error/crash      │              │
    │        ▼                  │              │
    │   ┌─────────┐             │              │
    │   │ Stopped │─────────────┘              │
    │   └────┬────┘                             │
    │        │ OnStop()                         │
    │        ▼                                  │
    │   ┌─────────┐                             │
    │   │  Dead   │                             │
    │   └─────────┘                             │
    │                                           │
    └───────────────────────────────────────────┘
```

### Actor States

| State | Description |
|-------|-------------|
| Created | Actor struct initialized but not started |
| Starting | OnStart() is being called |
| Running | Actor is processing messages |
| Stopped | Actor is stopping, OnStop() called |
| Dead | Actor has terminated |

## Mailbox Implementation

### Structure

```
                    Mailbox
    ┌─────────────────────────────────────┐
    │  ┌─────┬─────┬─────┬─────┬─────┐   │
    │  │ Msg │ Msg │ Msg │ Msg │ ... │   │  ← Messages
    │  └─────┴─────┴─────┴─────┴─────┘   │
    │    ▲                               │
    │    │                               │
    │   Head                             │
    └─────────────────────────────────────┘
         │
         │ dequeue()
         ▼
    ┌─────────┐
    │  Actor  │
    └─────────┘
```

### Bounded Mailbox

```go
type Mailbox struct {
    capacity  int
    queue     chan Message
    overflow  *list.List  // For overflow messages
    policy    OverflowPolicy
}

type OverflowPolicy int

const (
    DropOldest OverflowPolicy = iota  // Drop oldest messages
    DropNewest                        // Drop newest messages
    Block                             // Block sender
)
```

### Priority Handling

```
Priority Queue:
    ┌────────────────────────────────────┐
    │ Critical: [Msg1] [Msg2]           │ ← Processed first
    │ High:     [Msg3]                  │
    │ Normal:   [Msg4] [Msg5] [Msg6]    │
    │ Low:      [Msg7]                  │ ← Processed last
    └────────────────────────────────────┘
```

## Supervisor Hierarchies

### Supervision Tree

```
                    ┌──────────────┐
                    │   Root       │
                    │  Supervisor  │
                    └──────┬───────┘
              ┌────────────┼────────────┐
              │            │            │
        ┌─────▼─────┐ ┌────▼────┐ ┌─────▼─────┐
        │  Worker   │ │  Worker │ │  Worker   │
        │Supervisor │ │Supervisor│ │Supervisor │
        └─────┬─────┘ └────┬────┘ └─────┬─────┘
              │            │            │
         ┌────┴────┐  ┌────┴────┐  ┌────┴────┐
         │         │  │         │  │         │
      ┌──▼──┐  ┌───▼──┐ ...
      │Actor│  │Actor │
      └─────┘  └──────┘
```

### Supervision Strategies

#### OneForOne

Restart only the failed child:

```
Before:  [A] [B] [C] [D]
              ↓
B fails: [A] [X] [C] [D]
              ↓
After:   [A] [B'] [C] [D]  ← Only B restarted
```

#### OneForAll

Restart all children when one fails:

```
Before:  [A] [B] [C] [D]
              ↓
B fails: [A] [X] [C] [D]
              ↓
After:   [A'] [B'] [C'] [D']  ← All restarted
```

#### RestForOne

Restart the failed actor and all started after it:

```
Before:  [A] [B] [C] [D]
              ↓
B fails: [A] [X] [C] [D]
              ↓
After:   [A] [B'] [C'] [D']  ← B, C, D restarted
```

### Restart Policies

```go
type RestartPolicy struct {
    MaxRestarts    int           // Maximum restarts in window
    Window         time.Duration // Time window for counting
    Backoff        BackoffStrategy
}

type BackoffStrategy interface {
    NextDelay(restartCount int) time.Duration
}

// Exponential backoff
type ExponentialBackoff struct {
    Initial    time.Duration
    Max        time.Duration
    Multiplier float64
}
```

## Actor References

### Local References

```
┌───────────────────────────────────────┐
│              Local Node                │
│                                        │
│   Actor A ──┐                          │
│             │                          │
│             ▼                          │
│         Registry ──────────▶ Actor B   │
│                              (local)   │
└───────────────────────────────────────┘
```

### Remote References

```
┌─────────────┐                    ┌─────────────┐
│   Node A    │                    │   Node B    │
│             │                    │             │
│  Actor A ───┼─── Mesh Network ──┼──▶ Actor B  │
│             │                    │             │
└─────────────┘                    └─────────────┘
```

### Actor Path

```
aether://node-region.node-id/actor-path/actor-name

Example:
aether://us-east-1.node-abc123/services/payment/processor
         │           │           │          │
         │           │           │          └─ Actor name
         │           │           └─ Actor path
         │           └─ Node ID
         └─ Region
```

## Actor Context

The context provides access to actor capabilities:

```go
type Context struct {
    // Identity
    Self       ActorRef    // Reference to self
    Parent     ActorRef    // Reference to parent
    Sender     ActorRef    // Reference to message sender

    // Capabilities
    Capabilities *CapabilitySet

    // Services
    State      *StateHandle
    Logger     Logger
    Metrics    Metrics

    // Lifecycle
    Canceled   bool
    CancelFunc context.CancelFunc
}
```

## Best Practices

### 1. Keep Actors Focused

```go
// Good: Single responsibility
type OrderActor struct { /* handles orders only */ }
type PaymentActor struct { /* handles payments only */ }
type ShippingActor struct { /* handles shipping only */ }

// Bad: Too many responsibilities
type OrderPaymentShippingActor struct { /* handles everything */ }
```

### 2. Use Typed Messages

```go
// Good: Clear message types
type CreateOrder struct {
    OrderID string
    Items   []Item
}

type OrderCreated struct {
    OrderID string
    Status  string
}

// Bad: Generic messages
type GenericMessage struct {
    Type string
    Data map[string]any
}
```

### 3. Handle Errors Gracefully

```go
func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    // Return error for supervisor to handle
    if criticalError {
        return nil, err  // May trigger restart
    }

    // Return error message for sender
    if validationError {
        return aether.NewMessage(aether.MessageTypeError, "invalid input"), nil
    }

    return response, nil
}
```

### 4. Use Supervisors for Critical Actors

```go
supervisor := NewSupervisor("order-supervisor")
supervisor.SetStrategy(OneForOne)
supervisor.SetMaxRestarts(3)

// Critical actors under supervision
supervisor.Spawn(NewOrderActor())
supervisor.Spawn(NewPaymentActor())
```

## Next Steps

- [Mesh Network](mesh.md) - Distributed actor communication
- [Security](security.md) - Capability-based security
- [Examples](../examples/overview.md) - See actors in action
