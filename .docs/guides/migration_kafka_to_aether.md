# Migrating from Apache Kafka to Aether Event System

**Last Updated:** 2026-03-26  
**Estimated Migration Time:** 3-5 days per service  

---

## Why Migrate?

| Concern | Apache Kafka | Aether Event System |
|---------|-------------|-------------------|
| Architecture | External broker cluster | Embedded in actor framework |
| Language model | Java-centric clients | Native Python / JS / Rust SDKs |
| Stream processing | Kafka Streams (Java only) | Built-in windowing in all SDKs |
| Backpressure | Client-side only | Framework-level with strategies |
| Workflow integration | Requires external orchestrator | Native saga and state machine |
| Resilience | Producer/consumer configs | Circuit breaker + retry policies |
| State management | External (RocksDB, etc.) | Per-actor state handles |
| Deployment | Separate ZooKeeper + Kafka cluster | Single Aether runtime |

Aether replaces the Kafka broker, Kafka Streams, and a separate workflow engine with a single unified framework. You gain native streaming, event sourcing, and workflow orchestration without the operational complexity of managing a Kafka cluster.

---

## Concept Mapping

| Kafka Concept | Aether Equivalent |
|---------------|------------------|
| Topic | `Topic` / actor stream name |
| Partition | `PartitionConfig` / keyed `StreamActor` |
| Consumer Group | Actor subscription via `PubSubClient` |
| Producer | `PubSubClient.publish()` / `StreamActor.emit()` |
| Consumer | `PubSubClient.subscribe()` / `StreamActor.process_event()` |
| Kafka Streams | `StreamActor` with windowing |
| Offset | `Watermark` / event `offset` field |
| Consumer Lag | `BackpressureController` stats |
| Exactly-once | `DeliverySemantics.EXACTLY_ONCE` + checkpointing |
| Schema Registry | `EventEnvelope.schema_version` |
| Compacted Topic | `Topic(compacted=True)` |

---

## Step-by-Step Migration

### Step 1: Replace the Producer

```python
# Kafka
from kafka import KafkaProducer
import json

producer = KafkaProducer(
    bootstrap_servers='kafka:9092',
    value_serializer=lambda v: json.dumps(v).encode(),
)
producer.send('orders', {'orderId': '123', 'action': 'created'})
producer.flush()
```

```python
# Aether
from aether_sdk.event.pubsub import PubSubClient, Topic

client = PubSubClient()
await client.create_topic(Topic(name="orders", partitions=4))
await client.publish("orders", {"orderId": "123", "action": "created"})
```

Key differences:
- No bootstrap servers to configure
- Topics are created programmatically, not via CLI
- `publish` returns a message ID for tracking

### Step 2: Replace the Consumer

```python
# Kafka
from kafka import KafkaConsumer

consumer = KafkaConsumer(
    'orders',
    bootstrap_servers='kafka:9092',
    group_id='order-processor',
    auto_offset_reset='earliest',
    value_deserializer=lambda m: json.loads(m.decode()),
)

for message in consumer:
    order = message.value
    process(order)
    consumer.commit()
```

```python
# Aether
from aether_sdk.event.pubsub import PubSubClient

client = PubSubClient()

async def handle_order(msg):
    order = msg.value
    process(order)
    await client.acknowledge(msg.id)

await client.subscribe("orders", handle_order)
```

Key differences:
- No consumer group ID needed (each actor is its own consumer)
- Acknowledgement is explicit via `acknowledge(msg.id)`
- Use `@subscribe("orders.*")` decorator for actor integration

### Step 3: Replace Kafka Streams

This is the most significant migration. Kafka Streams DSL maps to Aether's `StreamActor`.

```python
# Kafka Streams (Java)
KStream<String, Order> orders = builder.stream("orders");
KTable<String, Long> counts = orders
    .groupBy((key, order) -> order.getCategory())
    .windowedBy(TimeWindows.of(Duration.ofMinutes(5)))
    .count(Materialized.as("counts"));
counts.toStream().to("order-counts");
```

```python
# Aether
from aether_sdk.streaming import StreamActor, TumblingWindow, Duration
from aether_sdk.streaming.types import StreamEvent, WindowSpec, WindowType

class OrderCounter(StreamActor[str, dict]):
    @classmethod
    def name(cls) -> str:
        return "order-counter"

    async def on_start(self):
        self.configure_window(
            WindowSpec(type=WindowType.TUMBLING, size=Duration.from_minutes(5)),
            self._count_orders,
        )

    async def _count_orders(self, events: list, info):
        counts = {}
        for e in events:
            category = e.value.get("category", "unknown")
            counts[category] = counts.get(category, 0) + 1
        await self.emit("order-counts", counts)

    async def process_event(self, event: StreamEvent[dict]):
        pass  # windowing handles batching
```

### Step 4: Replace Offset Management with Watermarks

```python
# Kafka — manual offset management
consumer.seek_to_beginning(partition)
last_offset = consumer.position(partition)
consumer.commit(offsets={partition: OffsetAndMetadata(new_offset)})
```

```python
# Aether — watermark-based progress tracking
from aether_sdk.streaming.types import Watermark, Timestamp

class MyProcessor(StreamActor):
    async def process_event(self, event: StreamEvent):
        await self.emit("output", event.value)

    async def advance_watermark(self, watermark: Watermark):
        # Fires completed windows automatically
        await super().advance_watermark(watermark)

# Get current watermark position
wm = self.get_watermark("orders")
```

### Step 5: Replace Exactly-Once Semantics

```python
# Kafka — exactly-once configuration
producer = KafkaProducer(
    bootstrap_servers='kafka:9092',
    enable_idempotence=True,
    transactional_id='order-producer-1',
)
producer.init_transactions()
producer.begin_transaction()
producer.send('orders', order)
producer.send('inventory', update)
producer.commit_transaction()
```

```python
# Aether — exactly-once via checkpointing
from aether_sdk.streaming.types import StreamConfig, DeliverySemantics

class OrderProcessor(StreamActor):
    def __init__(self):
        super().__init__(config=StreamConfig(
            checkpointing_enabled=True,
            checkpoint_interval=Duration.from_minutes(1),
        ))
        self.require(Capability.STATE_READ, Capability.STATE_WRITE)
```

With checkpointing enabled, the framework coordinates `CHECKPOINT` / `CHECKPOINT_ACK` messages between actors to ensure atomic progress.

---

## Comparison: Windowing

| Feature | Kafka Streams | Aether |
|---------|--------------|--------|
| Tumbling windows | `TimeWindows.of(size)` | `TumblingWindow(size)` |
| Sliding windows | `TimeWindows.of(size).advanceBy(slide)` | `SlidingWindow(size, slide)` |
| Session windows | `SessionWindows.withGap(gap)` | `SessionWindow(gap)` |
| Late data handling | `allowedLateness()` | `LateDataPolicy.SIDE_OUTPUT` |
| Custom triggers | `Trigger` class | `WindowTrigger` with early firing |
| Watermark strategy | `WatermarkStrategy` | `WatermarkStrategy` enum |
| Event time vs processing time | Both | Both via `WatermarkStrategy` |

```python
# Kafka Streams — sliding window
KTable<Windowed<String>, Long> counts = orders
    .groupByKey()
    .windowedBy(TimeWindows.of(Duration.ofMinutes(10)).advanceBy(Duration.ofMinutes(1)))
    .count();

# Aether — sliding window
sw = SlidingWindow(
    size=Duration.from_minutes(10),
    slide=Duration.from_minutes(1),
    handler=count_fn,
)
```

---

## Comparison: Backpressure

| Feature | Kafka | Aether |
|---------|-------|--------|
| Flow control | `max.poll.records` + `fetch.max.bytes` | `BackpressureController` with strategies |
| Buffer overflow | Blocks consumer | DROP / BUFFER / FAIL / LATEST strategies |
| Priority handling | Not available | `MultiLevelBackpressure` with priority queues |
| Rate limiting | Quota configs | `RateBasedBackpressure` |
| Metrics | JMX / consumer lag | `BackpressureStats` per actor |

```python
# Kafka — consumer throttle
consumer = KafkaConsumer(max_poll_records=500)

# Aether — configurable backpressure
from aether_sdk.streaming.types import BackpressureConfig, BackpressureStrategy

processor = MyProcessor(backpressure_config=BackpressureConfig(
    strategy=BackpressureStrategy.BUFFER,
    buffer_size=10000,
    high_watermark=0.9,
    low_watermark=0.5,
))
```

---

## Comparison: Exactly-Once Semantics

| Aspect | Kafka EOS | Aether EOS |
|--------|-----------|------------|
| Mechanism | Transactional producer + consumer | Checkpoint barriers |
| Scope | Topic partition | Actor graph |
| Coordination | Transaction coordinator | Actor-to-actor messages |
| Isolation | Read-committed | In-flight + committed |
| Overhead | Higher (full transactions) | Lower (barrier-based) |
| Alignment | Manual (pause/resume) | Automatic (framework) |

---

## Gotchas and Common Pitfalls

1. **No ZooKeeper equivalent**. Aether uses its own mesh networking for discovery. You do not need to manage a separate coordination service.

2. **Partitions are logical, not physical**. In Kafka, partitions map to broker disks. In Aether, partitioning is handled by `PartitionConfig` and `StreamActor` key extraction within a single runtime.

3. **Consumer groups don't exist**. Each `StreamActor` instance processes its own stream. For parallelism, deploy multiple actor instances with the same name.

4. **Topic compaction is declarative**. Use `Topic(compacted=True)` to retain only the latest value per key, similar to Kafka's `cleanup.policy=compact`.

5. **No schema registry integration**. Use `EventEnvelope.schema_version` with `EventVersion` to track event schema evolution manually.

6. **Watermarks are actor-local**. Unlike Kafka's broker-side offset tracking, each actor maintains its own watermark state. Use the `state` handle to persist watermark positions across restarts.

---

## Migration Checklist

- [ ] Inventory all Kafka topics and their partition counts
- [ ] Map Kafka topics to Aether `Topic` definitions
- [ ] Replace `KafkaProducer` with `PubSubClient.publish()`
- [ ] Replace `KafkaConsumer` with `PubSubClient.subscribe()` or `StreamActor`
- [ ] Replace Kafka Streams applications with `StreamActor` subclasses
- [ ] Replace `TimeWindows` / `SessionWindows` with Aether window classes
- [ ] Replace offset management with watermark tracking
- [ ] Replace `enable_idempotence` with checkpointing configuration
- [ ] Configure backpressure strategies for each stream processor
- [ ] Add circuit breakers for downstream service calls
- [ ] Replace Kafka consumer group monitoring with `BackpressureStats`
- [ ] Decommission ZooKeeper and Kafka brokers
- [ ] Run integration tests with production-like event volumes
- [ ] Monitor `BackpressureStats` and watermark lag in production
