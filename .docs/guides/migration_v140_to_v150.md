# Migrating from Aether v1.4.0 to v1.5.0

**Version:** 1.5.0  
**Last Updated:** 2026-03-26  
**Estimated Migration Time:** 2-4 hours per service  

---

## Why Migrate?

Aether v1.5.0 introduces four major new modules that significantly expand the framework's capabilities:

| Feature | v1.4.0 | v1.5.0 |
|---------|--------|--------|
| Streaming | Manual event processing | Built-in windowing, backpressure, watermarks |
| Events | Basic message passing | Pub/Sub, event sourcing, schema validation |
| Workflows | Not available | Saga, state machine, human tasks |
| Resilience | Manual retry logic | Circuit breaker, retry policies with backoff |
| Actor API | `Actor` base class only | `StreamActor` with typed state access |

v1.5.0 is fully backward compatible with v1.4.0. Existing actors continue to work without modification, but new APIs unlock powerful patterns for stream processing, event-driven architecture, and workflow orchestration.

---

## Breaking Changes

### 1. New Message Types

The `MessageType` enum gained three new members:

```python
# v1.4.0
class MessageType(Enum):
    START = "start"
    STOP = "stop"
    SIGNAL = "signal"
    RPC_REQUEST = "rpc_request"
    RPC_RESPONSE = "rpc_response"
    CUSTOM = "custom"

# v1.5.0 — added:
    STREAM_EVENT = "stream_event"
    WATERMARK = "watermark"
    CHECKPOINT = "checkpoint"
    CHECKPOINT_ACK = "checkpoint_ack"
```

If you switch on `message.type` exhaustively, add cases for the new types.

### 2. Actor Decorator Behavior

The `@actor` decorator now inherits from `Actor` using multiple inheritance. If you manually inherit from `Actor` **and** use the decorator, remove the explicit base class:

```python
# v1.4.0
@actor
class MyActor(Actor):
    ...

# v1.5.0 — remove Actor from the class declaration
@actor
class MyActor:
    ...
```

### 3. State Handle Initialization

`StateHandle` is now lazily initialized on first access via the `state` property. Direct `__init__` calls still work, but the recommended pattern is to use the property:

```python
# v1.4.0
def __init__(self):
    self._state = StateHandle()

# v1.5.0
async def handle_message(self, sender, message):
    await self.state.set_json("key", "value")  # auto-created
```

---

## New Module: Streaming

### Overview

The streaming module (`aether_sdk.streaming`) provides Flink-style stream processing with event-time semantics.

### Windowing

Three window types are available: tumbling, sliding, and session.

```python
# v1.4.0 — manual aggregation
class Aggregator(Actor):
    def __init__(self):
        self.buffer = []
        self.last_flush = time.time()

    async def handle_message(self, sender, message):
        self.buffer.append(message.payload)
        if time.time() - self.last_flush > 300:  # 5 minutes
            result = sum(self.buffer)
            self.buffer.clear()
            await self.send("downstream", Message(type=MessageType.CUSTOM, payload=result))

# v1.5.0 — using TumblingWindow
from aether_sdk.streaming import StreamActor, TumblingWindow, Duration, StreamEvent

class Aggregator(StreamActor[str, float]):
    @classmethod
    def name(cls) -> str:
        return "aggregator"

    async def on_start(self):
        self.configure_window(
            WindowSpec(type=WindowType.TUMBLING, size=Duration.from_minutes(5)),
            self._aggregate,
        )

    async def _aggregate(self, events, info):
        total = sum(e.value for e in events)
        await self.emit("output", {"total": total, "window": info.window_id})

    async def process_event(self, event: StreamEvent[float]):
        pass  # windowing handles batching
```

### Backpressure

```python
# v1.4.0 — manual queue management
class Processor(Actor):
    def __init__(self):
        self.queue = asyncio.Queue(maxsize=1000)

    async def handle_message(self, sender, message):
        if self.queue.full():
            return  # drop
        await self.queue.put(message)

# v1.5.0 — built-in backpressure
from aether_sdk.streaming import StreamActor, BackpressureConfig
from aether_sdk.streaming.types import BackpressureStrategy

class Processor(StreamActor):
    @classmethod
    def name(cls) -> str:
        return "processor"

    def __init__(self):
        super().__init__(
            backpressure_config=BackpressureConfig(
                strategy=BackpressureStrategy.LATEST,
                buffer_size=1000,
            )
        )

    async def process_event(self, event):
        await self.emit("output", transform(event.value))
```

### Watermarks

```python
# v1.5.0 — watermark tracking
async def advance_watermark(self, watermark: Watermark):
    await self.advance_watermark(watermark)  # fires completed windows
```

---

## New Module: Event System

### Pub/Sub

```python
# v1.4.0 — direct actor messaging only
await actor.send("order-service", Message(type=MessageType.CUSTOM, payload=order))

# v1.5.0 — topic-based pub/sub
from aether_sdk.event.pubsub import PubSubClient, Topic, subscribe, publish

client = PubSubClient()
await client.create_topic(Topic(name="orders", partitions=4))
await client.publish("orders", {"orderId": "123", "action": "created"})

@subscribe("orders.*")
async def handle_order(self, msg):
    process(msg.value)
```

### Event Sourcing

```python
# v1.4.0 — direct state mutation
await self.state.set_json("status", "shipped")

# v1.5.0 — event-sourced state
from aether_sdk.event.event_sourcing import Aggregate, EventStore, InMemoryEventStore

class Order(Aggregate):
    def __init__(self):
        super().__init__()
        self.status = "pending"
        self.items = []

    def apply_order_created(self, event):
        self.status = "created"
        self.items = event["items"]

    def apply_order_shipped(self, event):
        self.status = "shipped"

store = InMemoryEventStore()
await store.append("order-123", [{"type": "order_created", "items": ["widget"]}])
events = await store.get_events("order-123")
```

---

## New Module: Workflows

### State Machine

```python
# v1.5.0 — new workflow engine
from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor

wf = Workflow("order-workflow")
wf.state("created", is_initial=True)
wf.state("paid")
wf.state("shipped", is_final=True)
wf.transition("pay", "created", "paid")
wf.transition("ship", "paid", "shipped")
wf.on_enter("shipped", notify_customer)
wf.build()

executor = WorkflowExecutor()
result = await executor.start(wf, {"order_id": "123"})
await executor.transition(result.workflow_id, "pay")
await executor.transition(result.workflow_id, "ship")
```

### Saga

```python
# v1.5.0 — distributed transaction with compensation
from aether_sdk.workflow.saga import Saga, SagaExecutor, RetryConfig

order_saga = Saga("order-processing") \
    .step("reserve-inventory").action(reserve).compensate(release) \
    .step("process-payment").action(charge).compensate(refund) \
    .step("ship-order").action(ship).compensate(cancel_shipment) \
    .build()

executor = SagaExecutor()
result = await executor.execute(order_saga, {"order_id": "123"})
```

### Human Tasks

```python
# v1.5.0 — human-in-the-loop workflows
from aether_sdk.workflow.human_task import HumanTask, HumanTaskManager, TaskForm

task = HumanTask(task_type="approval", title="Approve Order") \
    .with_assignee("manager@company.com") \
    .with_form(TaskForm().add_field("approved", "boolean", required=True))

manager = HumanTaskManager()
await manager.create_task(task, "wf-1", "approval-step")
result = await manager.wait_for_completion(task.task_id, timeout=86400)
```

---

## New Module: Resilience

### Circuit Breaker

```python
# v1.4.0 — manual failure tracking
class ServiceCaller(Actor):
    def __init__(self):
        self.failure_count = 0

    async def call_service(self):
        if self.failure_count > 5:
            return  # skip call
        try:
            result = await external_call()
            self.failure_count = 0
        except Exception:
            self.failure_count += 1

# v1.5.0 — circuit breaker
from aether_sdk.resilience.circuit_breaker import CircuitBreaker, CircuitBreakerConfig

cb = CircuitBreaker(CircuitBreakerConfig(
    failure_threshold=5,
    timeout_ms=30000,
    success_threshold=3,
))
try:
    result = await cb.execute(external_call)
except CircuitBreakerError:
    await self.send("alert", Message(type=MessageType.CUSTOM, payload="Service down"))
```

### Retry Policy

```python
# v1.4.0 — manual retry loop
for attempt in range(3):
    try:
        result = await external_call()
        break
    except Exception:
        await asyncio.sleep(2 ** attempt)

# v1.5.0 — declarative retry
from aether_sdk.resilience.retry import RetryPolicy, RetryConfig, BackoffStrategy

policy = RetryPolicy(RetryConfig(
    max_attempts=5,
    backoff=BackoffStrategy.EXPONENTIAL_JITTER,
    base_delay_ms=100,
    max_delay_ms=5000,
))
result = await policy.execute(external_call)
```

Predefined policies are also available:

```python
from aether_sdk.resilience.retry import network_retry_policy, database_retry_policy

network_policy = network_retry_policy(max_attempts=5)
db_policy = database_retry_policy(max_attempts=5)
```

---

## Migration Checklist

- [ ] Update `aether-sdk` dependency to `>=1.5.0`
- [ ] Audit `message.type` switch statements for new `STREAM_EVENT` / `WATERMARK` types
- [ ] Remove explicit `Actor` base class from `@actor`-decorated classes
- [ ] Replace manual state initialization with `self.state` property access
- [ ] Identify actors that could benefit from `StreamActor` instead of `Actor`
- [ ] Identify direct `actor.send()` patterns that could use `PubSubClient`
- [ ] Identify manual retry loops that could use `RetryPolicy`
- [ ] Identify manual failure tracking that could use `CircuitBreaker`
- [ ] Identify multi-step processes that could use `Saga` or `Workflow`
- [ ] Run existing test suite against v1.5.0
- [ ] Add tests for any new resilience patterns introduced

---

## Gotchas and Common Pitfalls

1. **StreamActor requires `process_event`, not `handle_message`**. If you extend `StreamActor`, implement `process_event` instead of `handle_message`. The base class handles message routing internally.

2. **Window handlers fire with copies of event lists**. Do not assume the list is mutable across firings; each call receives a copy.

3. **Circuit breakers are not shared by default**. Use `CircuitBreakerManager` if multiple actors need the same breaker instance.

4. **Event sourcing requires `apply_*` method naming**. The `Aggregate.apply_event` method looks for `apply_{event_type}` on the subclass. Missing handlers are silently ignored.

5. **Saga compensation runs in reverse order**. Only completed steps are compensated. If step 1 of 3 fails, nothing is rolled back.

6. **Backpressure `LATEST` strategy drops old events**. If you need to keep all events, use `BUFFER` or `FAIL` strategy instead.

7. **Watermarks are per-stream**. Call `advance_watermark` for each input stream independently.
