> **NOTE**: This tutorial uses SDK examples in Python and TypeScript. The Aether v2.0.0 runtime is Rust-native. The performance patterns described here (backpressure, windowing, rate limiting) are implemented in the Rust core engine. SDK-specific code examples will be updated in a future release.

# Performance Tuning

**Time:** ~25 minutes | **Prerequisites:** [Getting Started](./01_getting_started.md)

---

## Performance Overview

Aether is designed for high throughput with predictable latency. Here are the targets:

| Metric | Target | Notes |
|---|---|---|
| Actor cold start P99 | < 50µs | Time to first message |
| Message latency P99 (local) | < 10µs | Same-node delivery |
| Message latency P99 (remote) | < 1ms | Cross-node over QUIC |
| Throughput (local) | > 1M msg/s | Per node |
| Throughput (remote) | > 100K msg/s | Cross-node |
| Memory per actor | < 2KB | Resident memory |

These targets assume proper configuration — the defaults work for most workloads, but tuning backpressure, rate limiting, and circuit breakers can make the difference between stable production behavior and cascading failures.

---

## Backpressure Strategies

When a downstream actor can't keep up, you need a strategy for handling the overflow. Aether supports four mailbox overflow policies.

| Strategy | Behavior | Best for |
|---|---|---|
| `Buffer` | Grow the mailbox (bounded by max) | Bursty traffic, when you can absorb spikes |
| `Drop` | Drop the oldest message | Telemetry, logs, non-critical events |
| `Fail` | Return an error to the sender | Critical paths where data loss is unacceptable |
| `Latest` | Keep only the newest message | Dashboards, live feeds, status updates |

=== "Python"

    ```python
    from aether_sdk import Actor, MailboxPolicy

    class TelemetryActor(Actor):
        def __init__(self):
            super().__init__("telemetry")
            self.configure_mailbox(
                capacity=10_000,
                overflow=MailboxPolicy.DROP_OLDEST,
            )

    class OrderProcessor(Actor):
        def __init__(self):
            super().__init__("orders")
            self.configure_mailbox(
                capacity=500,
                overflow=MailboxPolicy.FAIL,
            )

    class StatusFeed(Actor):
        def __init__(self):
            super().__init__("status")
            self.configure_mailbox(
                capacity=1,
                overflow=MailboxPolicy.KEEP_LATEST,
            )
    ```

=== "TypeScript"

    ```typescript
    import { Actor, MailboxPolicy } from '@aether/sdk';

    class TelemetryActor extends Actor {
      constructor() {
        super('telemetry');
        this.configureMailbox({
          capacity: 10_000,
          overflow: MailboxPolicy.DropOldest,
        });
      }
    }

    class OrderProcessor extends Actor {
      constructor() {
        super('orders');
        this.configureMailbox({
          capacity: 500,
          overflow: MailboxPolicy.Fail,
        });
      }
    }

    class StatusFeed extends Actor {
      constructor() {
        super('status');
        this.configureMailbox({
          capacity: 1,
          overflow: MailboxPolicy.KeepLatest,
        });
      }
    }
    ```

### Choosing a Strategy

- **Buffer** (default) — Use when traffic is bursty but average throughput is within the actor's capacity. Set `capacity` high enough to absorb bursts.
- **Drop** — Use for non-critical data where losing messages is acceptable (metrics, logs, real-time positions). Always pair with monitoring so you know the drop rate.
- **Fail** — Use when every message matters (orders, payments). The upstream caller gets an error and can retry.
- **Latest** — Use for "last known good" patterns where only the most recent value matters (device status, leaderboard scores).

---

## Rate Limiting

Aether provides three rate limiting algorithms. Choose based on your traffic pattern.

### Token Bucket

Best for APIs with short bursts allowed but a sustained rate limit. Tokens refill at a constant rate.

=== "Python"

    ```python
    from aether_sdk.resilience import TokenBucket

    rate_limiter = TokenBucket(rate=100, burst=20)

    class ApiActor(Actor):
        async def handle_message(self, sender: str, msg: Message) -> Message:
            if not await rate_limiter.try_acquire():
                return Message.error("rate limited", status=429)
            return await self.process(msg)
    ```

=== "TypeScript"

    ```typescript
    import { TokenBucket } from '@aether/sdk/resilience';

    const rateLimiter = new TokenBucket({ rate: 100, burst: 20 });

    class ApiActor extends Actor {
      async handleMessage(sender: string, msg: Message): Promise<Message> {
        if (!rateLimiter.tryAcquire()) {
          return Message.error('rate limited', 429);
        }
        return this.process(msg);
      }
    }
    ```

### Sliding Window

Best for strict per-second limits. Counts requests within a rolling time window.

=== "Python"

    ```python
    from aether_sdk.resilience import SlidingWindow

    limiter = SlidingWindow(max_requests=50, window_seconds=1)

    class StrictApiActor(Actor):
        async def handle_message(self, sender: str, msg: Message) -> Message:
            if not await limiter.try_acquire():
                return Message.error("rate limited", status=429)
            return await self.process(msg)
    ```

=== "TypeScript"

    ```typescript
    import { SlidingWindow } from '@aether/sdk/resilience';

    const limiter = new SlidingWindow({ maxRequests: 50, windowSeconds: 1 });

    class StrictApiActor extends Actor {
      async handleMessage(sender: string, msg: Message): Promise<Message> {
        if (!limiter.tryAcquire()) {
          return Message.error('rate limited', 429);
        }
        return this.process(msg);
      }
    }
    ```

### Fixed Window

Best for simplicity and quota enforcement (e.g., "1000 requests per minute").

=== "Python"

    ```python
    from aether_sdk.resilience import FixedWindow

    limiter = FixedWindow(max_requests=1000, window_seconds=60)
    ```

=== "TypeScript"

    ```typescript
    import { FixedWindow } from '@aether/sdk/resilience';

    const limiter = new FixedWindow({ maxRequests: 1000, windowSeconds: 60 });
    ```

### Comparison

| Algorithm | Burst tolerance | Memory | Precision |
|---|---|---|---|
| Token Bucket | High (burst param) | O(1) | Approximate |
| Sliding Window | Medium | O(n) | Precise |
| Fixed Window | Low (edge bursts) | O(1) | Approximate |

---

## Circuit Breaker Patterns

A circuit breaker prevents cascading failures by stopping calls to a degraded service.

### Configuration Tuning

=== "Python"

    ```python
    from aether_sdk.resilience import CircuitBreaker

    breaker = CircuitBreaker(
        name="database",
        failure_threshold=5,       # Open after 5 failures
        success_threshold=3,       # Close after 3 successes in half-open
        recovery_timeout=30.0,     # Wait 30s before trying half-open
        half_open_max_calls=2,     # Allow 2 probe requests in half-open
    )
    ```

=== "TypeScript"

    ```typescript
    import { CircuitBreaker } from '@aether/sdk/resilience';

    const breaker = new CircuitBreaker({
      name: 'database',
      failureThreshold: 5,
      successThreshold: 3,
      recoveryTimeout: 30_000,
      halfOpenMaxCalls: 2,
    });
    ```

### State Machine

```
          failure_threshold failures
  ┌──────────┐ ──────────────────────▶ ┌──────────┐
  │  CLOSED  │                         │   OPEN   │
  └──────────┘ ◀────────────────────── └────┬─────┘
       ▲          success_threshold         │
       │          successes                │ recovery_timeout
       │                                   ▼
       │                             ┌───────────┐
       └─────────────────────────────│ HALF-OPEN │
                half_open succeeds   └───────────┘
```

- **Closed** — Normal operation. Failures are counted.
- **Open** — All calls fail fast. No calls reach the downstream service.
- **Half-Open** — After `recovery_timeout`, a limited number of probe calls are allowed. If they succeed, the breaker closes. If they fail, it re-opens.

### Practical Example

=== "Python"

    ```python
    class PaymentActor(Actor):
        def __init__(self):
            super().__init__("payments")
            self.db_breaker = CircuitBreaker(
                name="payment_db",
                failure_threshold=5,
                success_threshold=2,
                recovery_timeout=15.0,
            )
            self.gateway_breaker = CircuitBreaker(
                name="payment_gateway",
                failure_threshold=3,
                success_threshold=1,
                recovery_timeout=60.0,
            )

        async def handle_message(self, sender: str, msg: Message) -> Message:
            if msg.type == "charge":
                try:
                    record = await self.db_breaker.call(
                        lambda: self.db.insert(msg.payload)
                    )
                    result = await self.gateway_breaker.call(
                        lambda: self.gateway.charge(record)
                    )
                    return Message.response(result)
                except CircuitOpenError:
                    await self.publish("payment.degraded", {"msg_id": msg.id})
                    return Message.error("service temporarily unavailable", status=503)
    ```

=== "TypeScript"

    ```typescript
    class PaymentActor extends Actor {
      private dbBreaker: CircuitBreaker;
      private gatewayBreaker: CircuitBreaker;

      constructor() {
        super('payments');
        this.dbBreaker = new CircuitBreaker({
          name: 'payment_db',
          failureThreshold: 5,
          successThreshold: 2,
          recoveryTimeout: 15_000,
        });
        this.gatewayBreaker = new CircuitBreaker({
          name: 'payment_gateway',
          failureThreshold: 3,
          successThreshold: 1,
          recoveryTimeout: 60_000,
        });
      }

      async handleMessage(sender: string, msg: Message): Promise<Message> {
        if (msg.type === 'charge') {
          try {
            const record = await this.dbBreaker.call(
              () => this.db.insert(msg.payload),
            );
            const result = await this.gatewayBreaker.call(
              () => this.gateway.charge(record),
            );
            return Message.response(result);
          } catch (e) {
            if (e instanceof CircuitOpenError) {
              await this.publish('payment.degraded', { msgId: msg.id });
              return Message.error('service temporarily unavailable', 503);
            }
            throw e;
          }
        }
      }
    }
    ```

---

## Windowing Performance

Windowing groups events into time-based batches for aggregation. Aether supports three strategies.

| Window Type | Description | Overhead | Use case |
|---|---|---|---|
| Tumbling | Fixed-size, non-overlapping | Low | Per-second metrics |
| Sliding | Fixed-size, overlapping | Medium | Smoothed averages |
| Session | Dynamic, activity-gap based | Variable | User sessions |

=== "Python"

    ```python
    from aether_sdk.windowing import TumblingWindow, SlidingWindow, SessionWindow

    tumbling = TumblingWindow(duration=5.0, on_emit=self.on_window)

    sliding = SlidingWindow(duration=5.0, slide=1.0, on_emit=self.on_window)

    session = SessionWindow(gap=2.0, on_emit=self.on_window)

    async def on_window(self, events: list[dict]):
        avg = sum(e["value"] for e in events) / len(events)
        await self.publish("metrics.avg", {"value": avg, "count": len(events)})
    ```

=== "TypeScript"

    ```typescript
    import { TumblingWindow, SlidingWindow, SessionWindow } from '@aether/sdk/windowing';

    const tumbling = new TumblingWindow({
      duration: 5_000,
      onEmit: (events: any[]) => this.onWindow(events),
    });

    const sliding = new SlidingWindow({
      duration: 5_000,
      slide: 1_000,
      onEmit: (events: any[]) => this.onWindow(events),
    });

    const session = new SessionWindow({
      gap: 2_000,
      onEmit: (events: any[]) => this.onWindow(events),
    });

    private async onWindow(events: any[]): Promise<void> {
      const avg = events.reduce((s, e) => s + e.value, 0) / events.length;
      await this.publish('metrics.avg', { value: avg, count: events.length });
    }
    ```

### Trade-offs

- **Tumbling windows** are the cheapest — each event belongs to exactly one window, no duplication.
- **Sliding windows** create overlapping windows, so each event is processed multiple times. Use shorter slide intervals only when you need finer granularity.
- **Session windows** have unpredictable memory usage because a long-lived session can accumulate unbounded events. Always set a `max_duration` as a safety net.

```python
session = SessionWindow(gap=2.0, max_duration=3600.0, on_emit=self.on_window)
```

```typescript
const session = new SessionWindow({
  gap: 2_000,
  maxDuration: 3_600_000,
  onEmit: (events: any[]) => this.onWindow(events),
});
```

---

## Best Practices

### Batch Processing

Process multiple messages in a single handler call to reduce overhead.

=== "Python"

    ```python
    class BatchProcessor(Actor):
        def __init__(self):
            super().__init__("batch-processor")
            self.configure_mailbox(capacity=10_000)
            self.pending: list[Message] = []

        async def handle_message(self, sender: str, msg: Message) -> Message:
            self.pending.append(msg)
            if len(self.pending) >= 100:
                batch = self.pending[:100]
                self.pending = self.pending[100:]
                await self.flush(batch)
            return Message.response({"queued": True})

        async def flush(self, batch: list[Message]):
            keys = [f"item:{msg.payload['id']}" for msg in batch]
            values = await self.state.read_many(keys)
            await self.state.write_many({k: v for k, v in zip(keys, values) if v})
    ```

=== "TypeScript"

    ```typescript
    class BatchProcessor extends Actor {
      private pending: Message[] = [];

      constructor() {
        super('batch-processor');
        this.configureMailbox({ capacity: 10_000 });
      }

      async handleMessage(sender: string, msg: Message): Promise<Message> {
        this.pending.push(msg);
        if (this.pending.length >= 100) {
          const batch = this.pending.splice(0, 100);
          await this.flush(batch);
        }
        return Message.response({ queued: true });
      }

      private async flush(batch: Message[]): Promise<void> {
        const keys = batch.map(m => `item:${m.payload.id}`);
        const values = await this.state.readMany(keys);
        const writes: Record<string, string> = {};
        for (const [k, v] of Object.entries(values)) {
          if (v) writes[k] = v;
        }
        await this.state.writeMany(writes);
      }
    }
    ```

### Zero-Copy Where Possible

Avoid serializing and deserializing large payloads. Use references when the data stays on the same node.

=== "Python"

    ```python
    import numpy as np

    class MLActor(Actor):
        async def handle_message(self, sender: str, msg: Message) -> Message:
            payload = msg.payload
            if isinstance(payload, bytes):
                array = np.frombuffer(payload, dtype=np.float32)
            elif hasattr(payload, "shared_buffer"):
                array = payload.shared_buffer
            return Message.response({"result": array.sum()})
    ```

=== "TypeScript"

    ```typescript
    class MLActor extends Actor {
      async handleMessage(sender: string, msg: Message): Promise<Message> {
        const payload = msg.payload;
        if (payload instanceof ArrayBuffer) {
          const view = new Float32Array(payload);
          return Message.response({ result: view.reduce((a, b) => a + b, 0) });
        }
        if (payload.sharedBuffer) {
          const view = new Float32Array(payload.sharedBuffer);
          return Message.response({ result: view.reduce((a, b) => a + b, 0) });
        }
        return Message.error('unsupported payload type');
      }
    }
    ```

### Memory-Efficient Event Handling

Long-lived actors can accumulate memory if events aren't cleaned up. Use TTLs and periodic compaction.

=== "Python"

    ```python
    class StreamActor(Actor):
        async def on_start(self):
            await self.events.configure(
                max_age_seconds=86400,    # Keep 24h of events
                compaction_interval=300,   # Compact every 5 minutes
            )
    ```

=== "TypeScript"

    ```typescript
    class StreamActor extends Actor {
      async onStart(): Promise<void> {
        await this.events.configure({
          maxAgeSeconds: 86_400,
          compactionInterval: 300,
        });
      }
    }
    ```

### Monitoring and Alerting

Export metrics from every critical actor so you can spot issues before they become incidents.

=== "Python"

    ```python
    from aether_sdk.metrics import counter, gauge, histogram

    class MonitoredActor(Actor):
        async def handle_message(self, sender: str, msg: Message) -> Message:
            counter("actor.messages.total").inc()
            with histogram("actor.latency").time():
                result = await self.process(msg)
            gauge("actor.mailbox.size").set(len(self.mailbox))
            return result
    ```

=== "TypeScript"

    ```typescript
    import { counter, gauge, histogram } from '@aether/sdk/metrics';

    class MonitoredActor extends Actor {
      async handleMessage(sender: string, msg: Message): Promise<Message> {
        counter('actor.messages.total').inc();
        const result = await histogram('actor.latency').time(() => this.process(msg));
        gauge('actor.mailbox.size').set(this.mailboxSize);
        return result;
      }
    }
    ```

Key metrics to watch:

| Metric | Alert threshold | Why |
|---|---|---|
| `actor.mailbox.size` | > 80% capacity | Backpressure building |
| `actor.latency.p99` | > 10ms (local) | Handler too slow |
| `resilience.circuit_open` | > 0 | Downstream degradation |
| `window.events.dropped` | > 0 | Event loss |
| `actor.restarts.total` | > 3 in 5 min | Crash loop |

---

## What You Learned

- **Backpressure** — Four overflow strategies and when to use each
- **Rate limiting** — Token bucket, sliding window, and fixed window
- **Circuit breakers** — State machine tuning and practical configuration
- **Windowing** — Tumbling vs sliding vs session with performance trade-offs
- **Best practices** — Batching, zero-copy, memory management, and monitoring

---

## Next Steps

| Topic | Resource |
|---|---|
| Architecture deep-dive | [Architecture Overview](../../docs-site/docs/architecture/overview.md) |
| Full examples | [Examples Gallery](../../docs-site/docs/examples/overview.md) |
| Performance targets | [Performance Overview](../../docs-site/docs/performance/overview.md) |

---

*Time to complete: ~25 minutes*
