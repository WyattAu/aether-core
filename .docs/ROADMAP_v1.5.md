# Project Aether v1.5.0 Roadmap

## Overview

**Version**: 1.5.0  
**Codename**: "Flow"  
**Target Date**: Q3 2026  
**Theme**: Stream Processing & Event-Driven Architecture

---

## Goals

1. **Stream Processing**: Native support for event streams and real-time data pipelines
2. **Event System**: Robust pub/sub messaging with guaranteed delivery
3. **Workflow Engine**: Saga pattern for distributed transactions
4. **Performance**: Optimize streaming throughput and latency

---

## Feature Categories

### 1. Stream Processing (Priority: Critical)

#### 1.1 Stream Actor Type
- **Description**: New actor type optimized for continuous data processing
- **Impact**: Enables real-time analytics, data pipelines, IoT ingestion
- **Effort**: High
- **SDKs**: All

```typescript
// Example API
class StreamActor extends Actor {
    // Process incoming stream events
    async onEvent(event: StreamEvent): Promise<void>;
    
    // Emit to downstream streams
    async emit(stream: string, event: any): Promise<void>;
    
    // Configure watermarks and late data handling
    configure(options: StreamConfig): void;
}

interface StreamConfig {
    watermarkStrategy: 'event-time' | 'processing-time';
    lateDataPolicy: 'drop' | 'side-output' | 'reprocess';
    bufferCapacity: number;
}
```

#### 1.2 Windowing Functions
- **Description**: Tumbling, sliding, and session windows for aggregation
- **Impact**: Time-series analytics, sessionization
- **Effort**: High
- **SDKs**: All

```typescript
interface WindowSpec {
    type: 'tumbling' | 'sliding' | 'session';
    size: Duration;
    slide?: Duration;        // For sliding windows
    gap?: Duration;          // For session windows
    lateTolerance: Duration;
}

// Example: 5-minute tumbling window
const window = new WindowSpec({
    type: 'tumbling',
    size: Duration.minutes(5),
    lateTolerance: Duration.seconds(30)
});
```

#### 1.3 Backpressure Handling
- **Description**: Reactive streams with built-in backpressure
- **Impact**: Prevents overload in high-throughput scenarios
- **Effort**: Medium
- **SDKs**: All

```typescript
interface BackpressureConfig {
    strategy: 'buffer' | 'drop' | 'fail' | 'latest';
    bufferSize: number;
    onOverflow: () => void;
}
```

#### 1.4 Stream Joins
- **Description**: Join multiple streams with time-based windows
- **Impact**: Complex event processing capabilities
- **Effort**: High
- **SDKs**: All

---

### 2. Event System (Priority: Critical)

#### 2.1 Pub/Sub Messaging
- **Description**: Topic-based publish/subscribe with actor integration
- **Impact**: Decoupled event-driven architecture
- **Effort**: High
- **SDKs**: All

```typescript
// Publisher
await actor.publish('user.events', { userId: 123, action: 'login' });

// Subscriber
class UserEventActor extends Actor {
    @Subscribe('user.events')
    async handleUserEvent(event: UserEvent): Promise<void> {
        // Process event
    }
}
```

#### 2.2 Event Sourcing
- **Description**: Persist events as source of truth for actor state
- **Impact**: Audit trail, temporal queries, state reconstruction
- **Effort**: High
- **SDKs**: All

```typescript
interface EventStore {
    append(streamId: string, events: Event[]): Promise<void>;
    read(streamId: string, fromVersion?: number): Promise<Event[]>;
    subscribe(streamId: string, handler: EventHandler): Promise<void>;
}

class EventSourcedActor extends Actor {
    // Apply event to update state
    abstract apply(event: Event): void;
    
    // Rebuild state from events
    async rebuild(fromVersion: number): Promise<void>;
}
```

#### 2.3 Guaranteed Delivery
- **Description**: At-least-once and exactly-once delivery semantics
- **Impact**: Reliable messaging for critical workflows
- **Effort**: High
- **SDKs**: All

```yaml
# Delivery guarantees
delivery:
  semantics: exactly-once  # at-most-once | at-least-once | exactly-once
  retry:
    maxAttempts: 5
    backoff: exponential
  deadLetter:
    enabled: true
    topic: dead-letter-queue
```

#### 2.4 Event Schema Registry
- **Description**: Schema management for event validation and evolution
- **Impact**: Type safety, compatibility checking
- **Effort**: Medium
- **SDKs**: All

---

### 3. Workflow Engine (Priority: High)

#### 3.1 Saga Pattern
- **Description**: Distributed transaction coordination with compensation
- **Impact**: Reliable multi-step workflows across actors
- **Effort**: High
- **SDKs**: All

```typescript
// Define saga steps
const orderSaga = new Saga('order-processing')
    .step('reserve-inventory')
        .action(async (ctx) => inventory.reserve(ctx.orderId))
        .compensate(async (ctx) => inventory.release(ctx.orderId))
    .step('process-payment')
        .action(async (ctx) => payment.charge(ctx.orderId))
        .compensate(async (ctx) => payment.refund(ctx.orderId))
    .step('ship-order')
        .action(async (ctx) => shipping.createShipment(ctx.orderId))
        .compensate(async (ctx) => shipping.cancelShipment(ctx.orderId))
    .build();

// Execute saga
const result = await sagaExecutor.execute(orderSaga, { orderId: '123' });
```

#### 3.2 Workflow State Machine
- **Description**: Visual workflow definitions with state transitions
- **Impact**: Clear workflow visibility, debugging
- **Effort**: Medium
- **SDKs**: All

```typescript
const workflow = new Workflow('approval-flow')
    .state('draft')
    .state('pending_approval')
    .state('approved')
    .state('rejected')
    .transition('submit', { from: 'draft', to: 'pending_approval' })
    .transition('approve', { from: 'pending_approval', to: 'approved' })
    .transition('reject', { from: 'pending_approval', to: 'rejected' })
    .onEnter('approved', async (ctx) => notifyApproved(ctx));
```

#### 3.3 Workflow Persistence
- **Description**: Durable workflow state with checkpointing
- **Impact**: Resume workflows after failures
- **Effort**: Medium
- **SDKs**: All

#### 3.4 Human Task Integration
- **Description**: External task completion (approval, review)
- **Impact**: Human-in-the-loop workflows
- **Effort**: Medium
- **SDKs**: All

```typescript
// Human task step in workflow
.step('manager-approval')
    .humanTask({
        assignTo: 'manager',
        timeout: Duration.days(3),
        form: ApprovalForm,
        onTimeout: 'escalate'
    })
```

---

### 4. Performance & Optimization (Priority: High)

#### 4.1 Zero-Copy Messaging
- **Description**: Minimize serialization for high-throughput streams
- **Impact**: Reduced CPU, lower latency
- **Effort**: High

#### 4.2 Batch Processing
- **Description**: Efficient batch operations for stream processing
- **Impact**: Higher throughput for bulk operations
- **Effort**: Medium
- **SDKs**: All

```typescript
class BatchProcessor extends StreamActor {
    @Window({ type: 'tumbling', size: Duration.seconds(5) })
    async processBatch(events: Event[]): Promise<void> {
        // Process all events in window as a batch
        await this.batchInsert(events);
    }
}
```

#### 4.3 Partitioning
- **Description**: Parallel stream processing with partitioning
- **Impact**: Horizontal scalability
- **Effort**: Medium
- **SDKs**: All

```typescript
interface PartitionConfig {
    strategy: 'key' | 'range' | 'hash' | 'random';
    keyExtractor: (event: any) => string;
    partitions: number;
}
```

#### 4.4 Flow Control
- **Description**: Rate limiting and throttling for streams
- **Impact**: Protect downstream systems
- **Effort**: Low
- **SDKs**: All

---

### 5. SDK Improvements (Priority: Medium)

#### 5.1 Python SDK v0.3.0
- [ ] Async generators for stream consumption
- [ ] Type hints for stream types
- [ ] Pydantic models for event schemas
- [ ] Decorator-based stream definitions

```python
from aether_sdk import StreamActor, stream, window

class OrderStreamActor(StreamActor):
    @stream.subscribe("orders")
    @window.tumbling(size=timedelta(minutes=5))
    async def process_orders(self, events: list[OrderEvent]):
        # Process batch of orders
        pass
```

#### 5.2 Go SDK v0.3.0
- [ ] Channel-based stream API
- [ ] Generic event types
- [ ] Context propagation for streams
- [ ] Middleware for stream processing

```go
type OrderStreamActor struct {
    aether.StreamActor
}

func (a *OrderStreamActor) ProcessOrders(ctx context.Context, events []OrderEvent) error {
    // Process batch of orders
    return nil
}
```

#### 5.3 JavaScript SDK v0.3.0
- [ ] Async iterable streams
- [ ] RxJS integration
- [ ] Event schema validation with Zod
- [ ] React hooks for stream state

```typescript
import { StreamActor, window } from '@aether/sdk';

class OrderStreamActor extends StreamActor {
    @subscribe('orders')
    @window.tumbling({ size: 5 * 60 * 1000 })
    async processOrders(events: OrderEvent[]): Promise<void> {
        // Process batch
    }
}
```

#### 5.4 Java SDK v0.3.0
- [ ] Reactive Streams (Project Reactor) integration
- [ ] Event schema with Jackson
- [ ] Annotation-based stream definitions
- [ ] Spring Boot auto-configuration

```java
@StreamActor
public class OrderStreamActor {
    @Subscribe("orders")
    @Window(type = WindowType.TUMBLING, size = "5m")
    public Flux<Void> processOrders(Flux<OrderEvent> events) {
        return events.map(this::processOrder);
    }
}
```

---

### 6. Developer Experience (Priority: Medium)

#### 6.1 Stream Debugger
- [ ] Real-time stream visualization
- [ ] Event flow tracing
- [ ] Throughput monitoring
- [ ] Latency histograms

#### 6.2 Workflow Designer
- [ ] Visual workflow editor
- [ ] State machine diagrams
- [ ] Saga step visualization
- [ ] Execution history

#### 6.3 Schema Registry UI
- [ ] Schema browser
- [ ] Compatibility checker
- [ ] Migration tools
- [ ] Documentation generator

#### 6.4 Testing Utilities
- [ ] Stream test fixtures
- [ ] Event recorder/replayer
- [ ] Workflow simulation
- [ ] Time manipulation for windows

---

## Milestones

### M1: Streaming Foundation (Week 1-4)
- Stream actor type
- Basic backpressure
- Stream configuration
- Buffer management

### M2: Event System (Week 5-8)
- Pub/sub messaging
- Event sourcing basics
- Delivery guarantees
- Schema registry

### M3: Workflow Engine (Week 9-12)
- Saga pattern implementation
- Workflow state machine
- Persistence and checkpointing
- Human task integration

### M4: Performance & SDKs (Week 13-16)
- Windowing functions
- Batch processing
- Partitioning
- SDK v0.3.0 releases

---

## Breaking Changes

### Deprecations in v1.5.0
- `Actor.onMessage()` → `Actor.handle()` for consistency with streams
- Direct state access → Event-sourced state patterns

### Migration Guide
Will be provided for all breaking changes with automated migration scripts where possible.

---

## Success Metrics

| Metric | Current (v1.4.0) | Target (v1.5.0) |
|--------|------------------|-----------------|
| Stream throughput | N/A | 500K events/s |
| Event latency (P99) | N/A | <10ms |
| Saga success rate | N/A | 99.9% |
| Window accuracy | N/A | 99.99% |
| Backpressure recovery | N/A | <100ms |
| Test coverage | 90% | 92% |
| Documentation | 6000 lines | 8000 lines |

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Exactly-once complexity | High | High | Start with at-least-once |
| Event ordering issues | Medium | High | Partitioning strategy |
| Schema evolution | Medium | Medium | Backward compatibility |
| Performance regression | Low | High | Continuous benchmarking |
| SDK API changes | Medium | Medium | Design review |

---

## Dependencies

### External
- Apache Kafka (optional, for persistent event streams)
- Redis Streams (optional, for lightweight streams)
- Schema Registry (Confluent or custom)

### Internal
- v1.4.0 Resilience patterns (circuit breaker, retry)
- Mesh network layer
- State management layer
- Observability stack

---

## Appendix: API Previews

### Stream Actor API

```typescript
interface StreamActorConfig {
    inputStreams: string[];
    outputStreams: string[];
    parallelism: number;
    checkpointing: boolean;
    checkpointInterval: Duration;
}

abstract class StreamActor extends Actor {
    // Lifecycle
    abstract onEvent(event: StreamEvent): Promise<void>;
    onWatermark(watermark: Timestamp): Promise<void>;
    onError(error: Error): Promise<void>;
    
    // Output
    emit(stream: string, event: any): Promise<void>;
    emitWithTimestamp(stream: string, event: any, timestamp: Timestamp): Promise<void>;
    
    // State
    getState<K>(key: K): Promise<ValueState<K>>;
    getListState<K>(key: K): Promise<ListState<K>>;
    getMapState<K, V>(key: K): Promise<MapState<K, V>>;
}
```

### Saga API

```typescript
interface SagaStep<T> {
    name: string;
    action: (context: SagaContext<T>) => Promise<void>;
    compensate: (context: SagaContext<T>) => Promise<void>;
    retry: RetryPolicy;
    timeout: Duration;
}

interface SagaContext<T> {
    sagaId: string;
    input: T;
    state: Record<string, any>;
    completedSteps: string[];
}

class SagaExecutor {
    execute<T>(saga: Saga<T>, input: T): Promise<SagaResult<T>>;
    getStatus(sagaId: string): Promise<SagaStatus>;
    compensate(sagaId: string): Promise<void>;
}
```

### Event Store API

```typescript
interface EventStore {
    // Write
    append(streamId: string, event: Event | Event[]): Promise<AppendResult>;
    
    // Read
    read(streamId: string, options?: ReadOptions): Promise<Event[]>;
    readAll(options?: ReadOptions): Promise<Event[]>;
    
    // Subscribe
    subscribe(streamId: string, handler: EventHandler): Promise<Subscription>;
    subscribeAll(handler: EventHandler): Promise<Subscription>;
    
    // Management
    createStream(streamId: string, options?: StreamOptions): Promise<void>;
    deleteStream(streamId: string): Promise<void>;
    getStreamMetadata(streamId: string): Promise<StreamMetadata>;
}
```

### Window Function API

```typescript
interface WindowOperator<T, R> {
    // Trigger when window closes
    onTrigger(events: T[], window: WindowInfo): Promise<R[]>;
    
    // Early results (optional)
    onEarlyResult?(events: T[], window: WindowInfo): Promise<R[] | null>;
    
    // Late data handling
    onLateData?(event: T, window: WindowInfo): Promise<void>;
}

interface WindowInfo {
    start: Timestamp;
    end: Timestamp;
    maxTimestamp: Timestamp;
    pane: 'early' | 'on-time' | 'late';
}
```

---

**Document Version**: 1.0.0  
**Last Updated**: 2026-03-19  
**Author**: Aether Core Team
