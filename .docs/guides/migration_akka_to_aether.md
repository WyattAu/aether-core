# Migrating from Akka to Aether Actors

**Last Updated:** 2026-03-26  
**Estimated Migration Time:** 2-4 weeks per service  

---

## Why Migrate?

| Concern | Akka (JVM) | Aether |
|---------|-----------|--------|
| Language | Scala / Java only | Python, JS, Rust, Go |
| Actor model | Classic, Typed, Cluster | Unified actor model |
| Message passing | JVM-serialized objects | JSON-serializable `Message` |
| Supervision | Hierarchical supervision trees | `CircuitBreaker` + `RetryPolicy` |
| Persistence | Akka Persistence (plugins) | `StateHandle` + `EventStore` |
| Remoting | Akka Cluster / ARTERY | Built-in mesh networking |
| Streaming | Akka Streams | Built-in windowing + backpressure |
| Workflow | Not built-in | Saga + state machine |
| Deployment | JVM runtime + config | Lightweight actor runtime |

Aether provides a polyglot actor framework with built-in streaming, event sourcing, and workflow orchestration. If you want to move away from JVM-only actor systems or reduce infrastructure complexity, Aether offers a lighter-weight alternative.

---

## Concept Mapping

| Akka Concept | Aether Equivalent |
|-------------|------------------|
| `ActorRef` | Actor name (string) |
| `ActorSystem` | Aether runtime |
| `Props` | Actor class + constructor |
| `tell(!)` | `actor.send(target, message)` |
| `ask(?)` | `actor.call(target, request)` |
| `Actor.receive` | `actor.handle_message()` |
| `Actor.postStop` | `actor.on_stop()` |
| `Actor.preStart` | `actor.on_start()` |
| SupervisorStrategy | `CircuitBreaker` |
| Akka Persistence | `StateHandle` + `EventStore` |
| `Stash` | Backpressure buffer |
| Akka Streams | `StreamActor` + windowing |
| `PoisonPill` | `Message(type=MessageType.STOP)` |
| `ActorSelection` | Direct name lookup |
| `Router` / `RoundRobinPool` | Multiple actor instances |
| `PersistentActor` | `EventSourcedActor` + `Aggregate` |
| `EventSourcedBehavior` | `Aggregate` with `apply_*` methods |

---

## Step-by-Step Migration

### Step 1: Actor Definition

```scala
// Akka
class Greeter extends Actor {
  def receive: Receive = {
    case Greet(name) =>
      println(s"Hello, $name!")
      sender() ! Greeted(name)
    case Stop =>
      context.stop(self)
  }
}

val system = ActorSystem("my-system")
val greeter = system.actorOf(Props[Greeter](), "greeter")
```

```python
# Aether
from aether_sdk.actor import Actor, actor
from aether_sdk.messaging import Message, MessageType

class Greeter(Actor):
    @classmethod
    def name(cls) -> str:
        return "greeter"

    async def handle_message(self, sender: str, message: Message):
        if message.payload.get("type") == "greet":
            name = message.payload["name"]
            return Message(
                type=MessageType.CUSTOM,
                payload={"type": "greeted", "name": name},
            )

# Or using the decorator
@actor
class GreeterAlt:
    _actor_name = "greeter"

    async def handle_message(self, sender, message):
        return Message(type=MessageType.CUSTOM, payload=message.payload)
```

```typescript
// Aether (JavaScript)
import { Actor, Message, MessageType } from 'aether/actor';
import { Message as Msg } from 'aether/messaging';

class Greeter extends Actor {
  static get name(): string { return 'greeter'; }

  async handle(sender: string, message: Msg): Promise<Msg | void> {
    if (message.payload.type === 'greet') {
      return Msg.custom({ type: 'greeted', name: message.payload.name });
    }
  }
}
```

### Step 2: Tell Pattern (Fire-and-Forget)

```scala
// Akka — tell
greeter ! Greet("World")
```

```python
# Aether
await greeter.send("greeter", Message(
    type=MessageType.CUSTOM,
    payload={"type": "greet", "name": "World"},
))
```

Key difference: Aether's `send` is async (`await`). Akka's `tell` is synchronous (fire-and-forget). Both are non-blocking.

### Step 3: Ask Pattern (Request-Response)

```scala
// Akka — ask
import akka.pattern.ask
import scala.concurrent.duration._
implicit val timeout: akka.util.Timeout = 30.seconds
val future = greeter ? Greet("World")
val result = Await.result(future, timeout)
```

```python
# Aether — RPC call
result = await greeter.call("greeter", {"type": "greet", "name": "World"}, timeout=30.0)
```

Key differences:
- Aether uses correlation IDs internally (no `AskTimeoutException` wrapping)
- The timeout parameter is in seconds (float), not a `Timeout` object
- The result is the payload directly, not a wrapped `Any`

### Step 4: Actor Hierarchy to Capability System

Akka uses parent-child actor hierarchies for lifecycle and supervision. Aether uses a flat capability system.

```scala
// Akka — hierarchical supervision
class OrderParent extends Actor {
  override val supervisorStrategy = OneForOneStrategy() {
    case _: PaymentException => Restart
    case _: Exception => Stop
  }

  val paymentChild = context.actorOf(
    Props[PaymentActor](),
    "payment"
  )
}

// Errors bubble up to parent for handling
```

```python
# Aether — circuit breaker per dependency
from aether_sdk.resilience.circuit_breaker import CircuitBreaker, CircuitBreakerConfig
from aether_sdk.resilience.retry import RetryPolicy, RetryConfig, BackoffStrategy

class OrderActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "order"

    def __init__(self):
        self._payment_cb = CircuitBreaker(CircuitBreakerConfig(
            failure_threshold=3,
            timeout_ms=30000,
        ))
        self._payment_retry = RetryPolicy(RetryConfig(
            max_attempts=3,
            backoff=BackoffStrategy.EXPONENTIAL_JITTER,
        ))

    async def handle_message(self, sender, message):
        try:
            result = await self._payment_cb.execute(
                lambda: self._payment_retry.execute(
                    lambda: self.call("payment", message.payload)
                )
            )
            return Message(type=MessageType.CUSTOM, payload=result)
        except CircuitBreakerError:
            return Message(type=MessageType.CUSTOM, payload={"error": "payment_unavailable"})

    def require(self, *capabilities):
        super().require(Capability.NETWORK_OUTBOUND, Capability.STATE_READ)
```

| Akka Pattern | Aether Pattern |
|-------------|---------------|
| Parent `SupervisorStrategy` | `CircuitBreaker` on outbound calls |
| `OneForOneStrategy` | Per-target `CircuitBreaker` instances |
| `AllForOneStrategy` | `CircuitBreakerManager` with shared config |
| `Restart` | Circuit auto-recovers via HALF_OPEN |
| `Stop` | Circuit stays OPEN, calls rejected |
| `Resume` | Circuit reopens after timeout |
| `Escalate` | `CircuitBreakerError` propagated to caller |

### Step 5: Persistence to State Handle

```scala
// Akka Persistence
class Counter extends PersistentActor {
  var count: Int = 0

  override def persistenceId: String = "counter-1"

  override def receiveCommand: Receive = {
    case Increment =>
      persist(Incremented(1)) { event =>
        count += event.delta
      }
    case GetCount =>
      sender() ! count
  }

  override def receiveRecover: Receive = {
    case Incremented(delta) =>
      count += delta
  }
}
```

```python
# Aether — StateHandle (simple key-value)
from aether_sdk.actor import Actor
from aether_sdk.state import StateHandle

class Counter(Actor):
    @classmethod
    def name(cls) -> str:
        return "counter"

    async def handle_message(self, sender, message):
        if message.payload.get("action") == "increment":
            count = await self.state.get_json("count") or 0
            count += 1
            await self.state.set_json("count", count)
            return Message(type=MessageType.CUSTOM, payload={"count": count})
        elif message.payload.get("action") == "get":
            count = await self.state.get_json("count") or 0
            return Message(type=MessageType.CUSTOM, payload={"count": count})
```

```python
# Aether — Event Sourcing (full audit trail)
from aether_sdk.event.event_sourcing import Aggregate, EventSourcedActor, InMemoryEventStore

class CounterAggregate(Aggregate):
    def __init__(self):
        super().__init__()
        self.count = 0

    def apply_incremented(self, event):
        self.count += event["delta"]

class CounterActor(EventSourcedActor):
    @classmethod
    def name(cls) -> str:
        return "counter"

    async def handle_message(self, sender, message):
        aggregate = await self.load_aggregate("counter-1", CounterAggregate)
        aggregate.emit_event("incremented", {"delta": 1})
        await self.save_aggregate(aggregate)
        return Message(type=MessageType.CUSTOM, payload={"count": aggregate.count})
```

| Akka Persistence | Aether Equivalent |
|-----------------|------------------|
| `PersistentActor` | `EventSourcedActor` |
| `persist()` | `Aggregate.emit_event()` |
| `recover` | `Aggregate.apply_*` methods |
| `persistenceId` | `aggregate.id` |
| `saveSnapshot` | `EventStore.save_snapshot()` |
| `SnapshotOffer` | `Aggregate.load_from_history(snapshot)` |
| `RecoveryCompleted` | After `load_from_history()` returns |
| Journal plugin | `EventStore` implementation |
| Snapshot store plugin | `EventStore.save_snapshot()` |
| `eventsByPersistenceId` | `EventStore.get_events()` |

### Step 6: Akka Streams to StreamActor

```scala
// Akka Streams
val source = Source(1 to 100)
val flow = Flow[Int]
  .groupedWithin(10, 5.seconds)
  .map(_.sum)
  .async
  .withAttributes(ActorAttributes.supervisionStrategy(
    SupervisionStrategy.resumingDecider
  ))
val sink = Sink.foreach(println)
source.via(flow).runWith(sink)
```

```python
# Aether
from aether_sdk.streaming import StreamActor, TumblingWindow, Duration
from aether_sdk.streaming.types import WindowSpec, WindowType, StreamEvent

class SumProcessor(StreamActor[str, int]):
    @classmethod
    def name(cls) -> str:
        return "sum-processor"

    async def on_start(self):
        self.configure_window(
            WindowSpec(type=WindowType.TUMBLING, size=Duration.from_seconds(5)),
            self._sum_window,
        )

    async def _sum_window(self, events, info):
        total = sum(e.value for e in events)
        await self.emit("output", total)

    async def process_event(self, event: StreamEvent[int]):
        pass  # windowing handles batching
```

| Akka Streams | Aether |
|-------------|--------|
| `Source` | Actor sending `STREAM_EVENT` messages |
| `Sink` | `StreamActor.emit()` output |
| `Flow` | `StreamActor.process_event()` |
| `groupedWithin` | `TumblingWindow` / `SlidingWindow` |
| `async` boundary | Built-in backpressure |
| `SupervisionStrategy.resumingDecider` | `BackpressureStrategy.LATEST` |
| `Buffer` | `BackpressureController` |
| `Throttle` | `RateBasedBackpressure` |

### Step 7: Actor Lifecycle

```scala
// Akka
class MyActor extends Actor {
  override def preStart(): Unit = {
    println("Starting")
  }

  override def postStop(): Unit = {
    println("Stopped")
  }

  def receive: Receive = { case _ => }
}
```

```python
# Aether
class MyActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "my-actor"

    async def on_start(self):
        print("Starting")

    async def on_stop(self):
        print("Stopped")

    async def handle_message(self, sender, message):
        pass
```

| Akka Lifecycle | Aether Lifecycle |
|---------------|-----------------|
| `preStart()` | `on_start()` |
| `postStop()` | `on_stop()` |
| `context.stop(self)` | `self.stop()` |
| `PoisonPill` | `Message(type=MessageType.STOP)` |
| `GracefulStop` | `await self.stop()` |

---

## Message Serialization

Akka serializes messages using Java serialization or custom serializers. Aether uses JSON serialization built into the `Message` class.

```scala
// Akka — custom serializer
class OrderSerializer extends JSerializer {
  def toBinary(o: AnyRef): Array[Byte] = ...
  def fromBinary(bytes: Array[Byte], manifest: Option[Class[_]]): AnyRef = ...
  def manifest(o: AnyRef): Option[Class[_]] = Some(o.getClass)
}
```

```python
# Aether — JSON by default
msg = Message(type=MessageType.CUSTOM, payload={"orderId": "123", "total": 42.0})
json_str = msg.to_json()
restored = Message.from_json(json_str)
```

Messages must have JSON-serializable payloads. For binary data, use base64 encoding in the payload or store in `StateHandle` with raw bytes.

---

## Gotchas and Common Pitfalls

1. **No actor hierarchy**. Aether uses a flat actor model. If you relied on parent-child relationships for lifecycle management, use the capability system and circuit breakers instead.

2. **No `become/unbecome`**. Akka's hot-swappable behavior (`context.become`) has no direct equivalent. Use the `state` handle to track actor mode and branch in `handle_message`.

3. **Messages are JSON, not typed objects**. You lose compile-time type checking on message payloads. Consider using typed dicts or dataclass serialization for structure.

4. **`ask` returns the payload, not a wrapper**. `actor.call()` returns the response payload directly, not a `Future[Any]` or `CompletionStage`.

5. **No mailbox priority by default**. The Python SDK processes messages FIFO. For priority, use `MultiLevelBackpressure` on the receiving side.

6. **No distributed actor references**. Actors are identified by name within a single Aether runtime. Cross-node communication is handled by the mesh networking layer transparently.

7. **State is per-actor, not shared**. Unlike Akka Cluster Sharding, there is no automatic distribution of state across nodes. Use `EventStore` for shared persistent state.

8. **No `Stash`**. For deferred message processing, use the `BackpressureController` buffer or the `state` handle to queue work items.

---

## Migration Checklist

- [ ] Inventory all Akka actor classes and their message protocols
- [ ] Map each Akka `ActorRef` to an Aether actor name
- [ ] Replace `tell(!)` with `actor.send()`
- [ ] Replace `ask(?)` with `actor.call()`
- [ ] Replace `SupervisorStrategy` with `CircuitBreaker` per dependency
- [ ] Replace `PersistentActor` with `EventSourcedActor` + `Aggregate`
- [ ] Replace simple state with `StateHandle` (key-value)
- [ ] Replace `context.become()` with state-mode branching
- [ ] Replace `Akka Streams` with `StreamActor` + windowing
- [ ] Replace `groupedWithin` with `TumblingWindow` / `SlidingWindow`
- [ ] Replace `async` boundaries with `BackpressureController`
- [ ] Replace `Buffer` with `BackpressureConfig.buffer_size`
- [ ] Replace `Throttle` with `RateBasedBackpressure`
- [ ] Ensure all message payloads are JSON-serializable
- [ ] Add `RetryPolicy` for transient failures
- [ ] Add `CircuitBreakerManager` for monitoring
- [ ] Run actor unit tests
- [ ] Run integration tests with mesh networking
- [ ] Monitor circuit breaker stats in production
