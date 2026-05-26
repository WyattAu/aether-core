> **NOTE**: This tutorial uses SDK examples in Python and TypeScript. The Aether v2.0.0 runtime is Rust-native. The concepts described here (event-driven architecture, pub/sub, backpressure) are fully supported in the Rust runtime via `aether_actor`. SDK-specific code examples will be updated in a future release.

# Building Event-Driven Systems

**Time:** ~1 hour | **Prerequisites:** [Getting Started](./01_getting_started.md), [Stateful Actors](./02_stateful_actors.md)

---

## Why Event-Driven?

Event-driven architecture (EDA) decouples services by having them communicate through events rather than direct calls. A producer publishes an event without knowing or caring who consumes it. Consumers subscribe to the events they care about and react independently.

This gives you three things that synchronous call chains cannot:

1. **Loose coupling** — Services can evolve independently. Add a new consumer without touching the producer.
2. **Resilience** — If a consumer is down, events queue up and process when it recovers.
3. **Auditability** — Every state change is recorded as an immutable event, giving you a complete history.

Aether provides three building blocks for event-driven systems: **Pub/Sub** for real-time messaging, **Event Sourcing** for append-only state history, and **Schema Validation** for enforcing event contracts.

---

## Setting Up Pub/Sub

Aether's pub/sub system uses topics with hierarchical names (dot-separated) and supports wildcard subscriptions.

### Creating Topics and Publishing

=== "Python"

    ```python
    from aether_sdk.event.pubsub import PubSubClient, Topic

    client = PubSubClient()

    await client.create_topic(Topic(
        name="user.events",
        partitions=4,
        retention_ms=86_400_000,  # 24 hours
    ))

    msg_id = await client.publish(
        topic="user.events",
        value={"type": "USER_CREATED", "userId": "u-123", "email": "alice@example.com"},
        key="u-123",
        headers={"origin": "registration-service"},
    )
    ```

=== "TypeScript"

    ```typescript
    import { PubSubClient, Topic } from '@aether/sdk';

    const client = new PubSubClient();

    await client.createTopic(new Topic({
      name: 'user.events',
      partitions: 4,
      retentionMs: 86_400_000,
    }));

    const msgId = await client.publish(
      'user.events',
      { type: 'USER_CREATED', userId: 'u-123', email: 'alice@example.com' },
      { key: 'u-123', headers: { origin: 'registration-service' } },
    );
    ```

### Subscribing with Handlers

=== "Python"

    ```python
    async def on_user_event(msg):
        print(f"[{msg.topic}] {msg.value}")

    sub = await client.subscribe("user.*", on_user_event)

    await client.unsubscribe(sub.id)
    ```

=== "TypeScript"

    ```typescript
    const sub = await client.subscribe('user.*', (msg) => {
      console.log(`[${msg.topic}]`, msg.value);
    });

    await client.unsubscribe(sub.id);
    ```

### Subscribing Actors

For actors that react to events, use `subscribe_actor` to route messages directly into an actor method.

=== "Python"

    ```python
    from aether_sdk import Actor
    from aether_sdk.event.pubsub import PubSubClient

    class EmailActor(Actor):
        def __init__(self):
            super().__init__("email-service")
            self.require("ACTOR_MESSAGING", "LOG")

        async def handle_event(self, msg):
            self.log.info(f"Sending email to {msg.value['email']}")

    actor = EmailActor()
    client = PubSubClient()
    await client.subscribe_actor("user.*", actor, method_name="handle_event")
    ```

=== "TypeScript"

    ```typescript
    class EmailActor extends Actor {
      constructor() {
        super({ name: 'email-service' });
        this.require(Capability.NETWORK_OUTBOUND, Capability.LOG);
      }

      async handleEvent(msg: PubSubMessage): Promise<void> {
        this.log.info(`Sending email to ${msg.value.email}`);
      }
    }

    const actor = new EmailActor();
    const client = new PubSubClient();
    await client.subscribeActor('user.*', actor, 'handleEvent');
    ```

### Batch Publishing

When you need to publish multiple events atomically, use `publish_batch`.

=== "Python"

    ```python
    from aether_sdk.event.pubsub import PubSubMessage

    messages = [
        PubSubMessage(topic="user.events", value={"type": "USER_CREATED", "userId": "u-1"}),
        PubSubMessage(topic="user.events", value={"type": "USER_CREATED", "userId": "u-2"}),
    ]
    ids = await client.publish_batch("user.events", messages)
    ```

=== "TypeScript"

    ```typescript
    const messages = [
      new PubSubMessage({ topic: 'user.events', value: { type: 'USER_CREATED', userId: 'u-1' } }),
      new PubSubMessage({ topic: 'user.events', value: { type: 'USER_CREATED', userId: 'u-2' } }),
    ];
    const ids = await client.publishBatch('user.events', messages);
    ```

---

## Event Sourcing

Event sourcing stores every state change as an immutable event in an append-only log. Instead of persisting current state, you persist the *sequence of events* that led to it. To reconstruct state, you replay the events from the beginning.

### Defining Aggregates

An aggregate is the domain object whose state you're sourcing. Define `apply_*` methods for each event type.

=== "Python"

    ```python
    from aether_sdk.event.event_sourcing import Aggregate, EventSourcedActor

    class Notification(Aggregate):
        def __init__(self):
            super().__init__()
            self.user_id = ""
            self.email = ""
            self.status = "pending"
            self.sent_at = None

        def apply_notification_requested(self, event):
            self.user_id = event["user_id"]
            self.email = event["email"]
            self.status = "pending"

        def apply_notification_sent(self, event):
            self.status = "sent"
            self.sent_at = event["sent_at"]

        def apply_notification_failed(self, event):
            self.status = "failed"
    ```

=== "TypeScript"

    ```typescript
    import { Aggregate } from '@aether/sdk';

    class Notification extends Aggregate {
      userId = '';
      email = '';
      status = 'pending';
      sentAt: string | null = null;

      applyNotificationRequested(event: any): void {
        this.userId = event.user_id;
        this.email = event.email;
        this.status = 'pending';
      }

      applyNotificationSent(event: any): void {
        this.status = 'sent';
        this.sentAt = event.sent_at;
      }

      applyNotificationFailed(event: any): void {
        this.status = 'failed';
      }
    }
    ```

### Emitting and Persisting Events

=== "Python"

    ```python
    from aether_sdk.event.event_sourcing import InMemoryEventStore, EventSourcedActor

    class NotificationActor(Actor, EventSourcedActor):
        def __init__(self):
            Actor.__init__(self, "notifications")
            EventSourcedActor.__init__(self, InMemoryEventStore())
            self.require("STATE_READ", "STATE_WRITE", "EVENT_EMIT")

        async def handle_message(self, sender, msg):
            notification = Notification()
            notification.id = f"notif-{msg.payload['user_id']}"
            notification.emit_event("notification_requested", {
                "user_id": msg.payload["user_id"],
                "email": msg.payload["email"],
            })
            await self.save_aggregate(notification)
    ```

=== "TypeScript"

    ```typescript
    class NotificationActor extends Actor implements EventSourcedActor {
      private eventStore = new InMemoryEventStore();

      constructor() {
        super({ name: 'notifications' });
      }

      async handle(sender: string, msg: Message): Promise<Message | void> {
        const notification = new Notification();
        notification.id = `notif-${msg.payload.user_id}`;
        notification.emitEvent('notification_requested', {
          user_id: msg.payload.user_id,
          email: msg.payload.email,
        });
        await this.saveAggregate(notification);
      }
    }
    ```

### Snapshots

Replaying thousands of events is expensive. Snapshots save a point-in-time copy so you only replay events after the snapshot.

=== "Python"

    ```python
    aggregate = await self.load_aggregate("notif-u-123", Notification)

    if aggregate.version >= 100:
        await self.save_snapshot("notif-u-123")

    events = await self._event_store.get_events("notif-u-123", after_version=100)
    aggregate.load_from_history(events, snapshot)
    ```

=== "TypeScript"

    ```typescript
    const aggregate = await this.loadAggregate('notif-u-123', Notification);

    if (aggregate.version >= 100) {
      await this.saveSnapshot('notif-u-123');
    }

    const events = await this.eventStore.getEvents('notif-u-123', 100);
    aggregate.loadFromHistory(events, snapshot);
    ```

---

## Schema Validation

A schema registry ensures events conform to a contract. Producers and consumers agree on the shape of events, and the registry catches malformed data at publish time.

### Registering Schemas

=== "Python"

    ```python
    from aether_sdk.event.schema import (
        InMemorySchemaRegistry, Schema, Compatibility,
    )

    registry = InMemorySchemaRegistry()

    await registry.register("UserCreated", Schema(
        name="UserCreated",
        type="json",
        definition={
            "type": "object",
            "properties": {
                "userId": {"type": "string"},
                "email": {"type": "string"},
                "name": {"type": "string"},
            },
            "required": ["userId", "email"],
        },
    ))
    ```

=== "TypeScript"

    ```typescript
    import { InMemorySchemaRegistry, Schema } from '@aether/sdk';

    const registry = new InMemorySchemaRegistry();

    await registry.register('UserCreated', new Schema({
      name: 'UserCreated',
      type: 'json',
      definition: {
        type: 'object',
        properties: {
          userId: { type: 'string' },
          email: { type: 'string' },
          name: { type: 'string' },
        },
        required: ['userId', 'email'],
      },
    }));
    ```

### Validating Events

=== "Python"

    ```python
    from aether_sdk.event.schema import SchemaError

    try:
        await registry.validate("UserCreated", {
            "userId": "u-123",
            "email": "alice@example.com",
        })
    except SchemaError as e:
        print(f"Invalid event: {e}")
    ```

=== "TypeScript"

    ```typescript
    try {
      await registry.validate('UserCreated', {
        userId: 'u-123',
        email: 'alice@example.com',
      });
    } catch (e) {
      console.error(`Invalid event: ${e}`);
    }
    ```

### Schema Evolution

When you need to change a schema, register a new version. The registry checks compatibility automatically.

=== "Python"

    ```python
    await registry.register("UserCreated", Schema(
        name="UserCreated",
        type="json",
        definition={
            "type": "object",
            "properties": {
                "userId": {"type": "string"},
                "email": {"type": "string"},
                "name": {"type": "string"},
                "phoneNumber": {"type": "string"},  # New optional field
            },
            "required": ["userId", "email"],
        },
    ), compatibility=Compatibility.BACKWARD)

    versions = await registry.get_versions("UserCreated")
    print(f"UserCreated has {len(versions)} versions")
    ```

=== "TypeScript"

    ```typescript
    await registry.register('UserCreated', new Schema({
      name: 'UserCreated',
      type: 'json',
      definition: {
        type: 'object',
        properties: {
          userId: { type: 'string' },
          email: { type: 'string' },
          name: { type: 'string' },
          phoneNumber: { type: 'string' },
        },
        required: ['userId', 'email'],
      },
    }), Compatibility.BACKWARD);

    const versions = await registry.getVersions('UserCreated');
    console.log(`UserCreated has ${versions.length} versions`);
    ```

| Compatibility | What it allows |
|---|---|
| `BACKWARD` | New schema can read old data (add optional fields) |
| `FORWARD` | Old schema can read new data (remove optional fields) |
| `FULL` | Both directions |
| `NONE` | Breaking change (changing types, adding required fields) |

---

## Complete Example: Real-Time Notification System

This example ties together pub/sub, event sourcing, and schema validation into a real-time notification system.

### Architecture

```
User registers
       │
       ▼
 RegistrationActor ──publish──▶ "user.registered" topic
       │                              │
       │                              ├─▶ EmailActor (sends welcome email)
       │                              └─▶ AuditActor (records event-sourced trail)
```

=== "Python"

    ```python
    import asyncio
    import json
    import time
    from aether_sdk import Actor, Message
    from aether_sdk.event.pubsub import PubSubClient, Topic, PubSubMessage
    from aether_sdk.event.event_sourcing import (
        Aggregate, EventSourcedActor, InMemoryEventStore,
    )
    from aether_sdk.event.schema import (
        InMemorySchemaRegistry, Schema, SchemaError,
    )


    class UserRegistration(Aggregate):
        def __init__(self):
            super().__init__()
            self.user_id = ""
            self.email = ""
            self.name = ""
            self.notifications = []

        def apply_user_registered(self, event):
            self.user_id = event["user_id"]
            self.email = event["email"]
            self.name = event.get("name", "")

        def apply_notification_sent(self, event):
            self.notifications.append({
                "channel": event["channel"],
                "sent_at": event["sent_at"],
            })


    class RegistrationActor(Actor):
        def __init__(self, pubsub: PubSubClient, registry: InMemorySchemaRegistry):
            super().__init__("registration")
            self.require("STATE_READ", "STATE_WRITE", "EVENT_EMIT", "ACTOR_MESSAGING", "LOG")
            self.pubsub = pubsub
            self.registry = registry
            self.users: dict[str, dict] = {}

        async def on_start(self):
            await self.pubsub.create_topic(Topic(
                name="user.registered",
                partitions=2,
            ))

        async def handle_message(self, sender, msg):
            match msg.type:
                case "register":
                    payload = msg.payload
                    user_id = f"u-{int(time.time() * 1000)}"

                    event_data = {
                        "user_id": user_id,
                        "email": payload["email"],
                        "name": payload.get("name", ""),
                    }

                    try:
                        await self.registry.validate("UserCreated", event_data)
                    except SchemaError as e:
                        return Message.error(f"validation failed: {e}")

                    await self.state.write(f"user:{user_id}", json.dumps(event_data))
                    self.users[user_id] = event_data

                    await self.pubsub.publish(
                        "user.registered",
                        event_data,
                        key=user_id,
                    )

                    return Message.response({"user_id": user_id, "status": "registered"})

                case _:
                    return Message.error("unknown message type")


    class EmailActor(Actor):
        def __init__(self, pubsub: PubSubClient):
            super().__init__("email-service")
            self.require("ACTOR_MESSAGING", "LOG")
            self.pubsub = pubsub
            self.sent: list[dict] = []

        async def on_start(self):
            await self.pubsub.subscribe_actor(
                "user.registered", self, method_name="handle_event",
            )

        async def handle_event(self, msg):
            self.log.info(f"Sending welcome email to {msg.value['email']}")
            self.sent.append({
                "email": msg.value["email"],
                "user_id": msg.value["user_id"],
                "sent_at": time.time(),
            })


    class AuditActor(Actor, EventSourcedActor):
        def __init__(self, pubsub: PubSubClient):
            Actor.__init__(self, "audit")
            EventSourcedActor.__init__(self, InMemoryEventStore())
            self.require("STATE_READ", "STATE_WRITE", "EVENT_EMIT", "LOG")
            self.pubsub = pubsub

        async def on_start(self):
            await self.pubsub.subscribe_actor(
                "user.registered", self, method_name="handle_event",
            )

        async def handle_event(self, msg):
            aggregate = UserRegistration()
            aggregate.id = msg.value["user_id"]
            aggregate.emit_event("user_registered", msg.value)
            await self.save_aggregate(aggregate)
            self.log.info(f"Audit: user {msg.value['user_id']} registered")


    async def main():
        registry = InMemorySchemaRegistry()
        await registry.register("UserCreated", Schema(
            name="UserCreated",
            type="json",
            definition={
                "type": "object",
                "properties": {
                    "user_id": {"type": "string"},
                    "email": {"type": "string"},
                    "name": {"type": "string"},
                },
                "required": ["user_id", "email"],
            },
        ))

        pubsub = PubSubClient()

        registration = RegistrationActor(pubsub, registry)
        email = EmailActor(pubsub)
        audit = AuditActor(pubsub)

        await registration.start()
        await email.start()
        await audit.start()

        response = await registration.call(
            "registration",
            Message("register", payload={
                "email": "alice@example.com",
                "name": "Alice",
            }),
        )
        print(f"Registration: {response.payload}")

        await asyncio.sleep(0.1)

        print(f"Emails sent: {len(email.sent)}")
        events = await audit._event_store.get_all_events()
        print(f"Audit events: {len(events)}")

    if __name__ == "__main__":
        asyncio.run(main())
    ```

=== "TypeScript"

    ```typescript
    import { Actor, Message, MessageType } from '@aether/sdk';
    import {
      PubSubClient,
      Topic,
      PubSubMessage,
    } from '@aether/sdk/pubsub';
    import {
      Aggregate,
      InMemoryEventStore,
    } from '@aether/sdk/event-sourcing';
    import {
      InMemorySchemaRegistry,
      Schema,
      SchemaError,
    } from '@aether/sdk/schema';

    class UserRegistration extends Aggregate {
      userId = '';
      email = '';
      name = '';
      notifications: Array<{ channel: string; sentAt: number }> = [];

      applyUserRegistered(event: any): void {
        this.userId = event.user_id;
        this.email = event.email;
        this.name = event.name ?? '';
      }

      applyNotificationSent(event: any): void {
        this.notifications.push({
          channel: event.channel,
          sentAt: event.sent_at,
        });
      }
    }

    class RegistrationActor extends Actor {
      private users: Map<string, any> = new Map();

      constructor(
        private pubsub: PubSubClient,
        private registry: InMemorySchemaRegistry,
      ) {
        super({ name: 'registration' });
      }

      async onStart(): Promise<void> {
        await this.pubsub.createTopic(new Topic({
          name: 'user.registered',
          partitions: 2,
        }));
      }

      async handle(sender: string, msg: Message): Promise<Message | void> {
        if (msg.type === MessageType.CUSTOM) {
          const payload = msg.payload;
          const userId = `u-${Date.now()}`;

          const eventData = {
            user_id: userId,
            email: payload.email,
            name: payload.name ?? '',
          };

          try {
            await this.registry.validate('UserCreated', eventData);
          } catch (e) {
            if (e instanceof SchemaError) {
              return Message.custom({ error: `validation failed: ${e}` });
            }
            throw e;
          }

          this.users.set(userId, eventData);
          await this.pubsub.publish('user.registered', eventData, { key: userId });

          return Message.custom({ user_id: userId, status: 'registered' });
        }
      }
    }

    class EmailActor extends Actor {
      sent: Array<{ email: string; userId: string; sentAt: number }> = [];

      constructor(private pubsub: PubSubClient) {
        super({ name: 'email-service' });
      }

      async onStart(): Promise<void> {
        await this.pubsub.subscribeActor(
          'user.registered',
          this,
          'handleEvent',
        );
      }

      async handleEvent(msg: PubSubMessage): Promise<void> {
        this.sent.push({
          email: msg.value.email,
          userId: msg.value.user_id,
          sentAt: Date.now(),
        });
      }
    }

    class AuditActor extends Actor {
      private eventStore = new InMemoryEventStore();

      constructor(private pubsub: PubSubClient) {
        super({ name: 'audit' });
      }

      async onStart(): Promise<void> {
        await this.pubsub.subscribeActor(
          'user.registered',
          this,
          'handleEvent',
        );
      }

      async handleEvent(msg: PubSubMessage): Promise<void> {
        const aggregate = new UserRegistration();
        aggregate.id = msg.value.user_id;
        aggregate.emitEvent('user_registered', msg.value);
        await this.saveAggregate(aggregate);
      }

      private async saveAggregate(aggregate: UserRegistration): Promise<void> {
        const events = aggregate.uncommittedEvents;
        if (events.length === 0) return;
        const eventDicts = events.map(e => ({
          type: e.eventType,
          ...e.payload,
        }));
        await this.eventStore.append(aggregate.id, eventDicts);
        aggregate.markEventsCommitted();
      }
    }

    async function main() {
      const registry = new InMemorySchemaRegistry();
      await registry.register('UserCreated', new Schema({
        name: 'UserCreated',
        type: 'json',
        definition: {
          type: 'object',
          properties: {
            user_id: { type: 'string' },
            email: { type: 'string' },
            name: { type: 'string' },
          },
          required: ['user_id', 'email'],
        },
      }));

      const pubsub = new PubSubClient();

      const registration = new RegistrationActor(pubsub, registry);
      const email = new EmailActor(pubsub);
      const audit = new AuditActor(pubsub);

      await registration.start();
      await email.start();
      await audit.start();

      const response = await registration.call('registration', Message.custom({
        email: 'alice@example.com',
        name: 'Alice',
      }));
      console.log('Registration:', response);

      await new Promise(r => setTimeout(r, 100));

      console.log(`Emails sent: ${email.sent.length}`);
    }

    main().catch(console.error);
    ```

### Walkthrough

1. **RegistrationActor** receives a `register` message, validates the payload against the `UserCreated` schema, persists the user to state, and publishes a `user.registered` event.
2. **EmailActor** subscribes to `user.registered` and sends a welcome email when the event arrives.
3. **AuditActor** subscribes to the same topic and records every registration as an event-sourced aggregate, providing a full audit trail.
4. If the schema validation fails (e.g., missing required `email` field), the registration is rejected before any event is published.

---

## Best Practices

### Event Versioning

Always use `Compatibility.BACKWARD` (the default) when evolving schemas. This allows new optional fields to be added without breaking existing consumers.

=== "Python"

    ```python
    await registry.register("UserCreated", new_schema, compatibility=Compatibility.BACKWARD)
    ```

=== "TypeScript"

    ```typescript
    await registry.register('UserCreated', newSchema, Compatibility.BACKWARD);
    ```

Reserve `Compatibility.NONE` for major breaking changes, and plan a coordinated migration of all consumers.

### Idempotency

Consumers may receive the same event more than once (at-least-once delivery). Design handlers to be idempotent.

=== "Python"

    ```python
    async def handle_event(self, msg):
        event_id = msg.id
        processed = await self.state.read(f"processed:{event_id}")
        if processed:
            return
        await self._do_work(msg.value)
        await self.state.write(f"processed:{event_id}", "1")
    ```

=== "TypeScript"

    ```typescript
    async handleEvent(msg: PubSubMessage): Promise<void> {
      const processed = await this.state.getString(`processed:${msg.id}`);
      if (processed) return;
      await this.doWork(msg.value);
      await this.state.setString(`processed:${msg.id}`, '1');
    }
    ```

### Ordering

Use the `key` parameter when publishing to ensure events for the same entity go to the same partition, preserving order.

=== "Python"

    ```python
    await client.publish("order.events", order_event, key=order_id)
    ```

=== "TypeScript"

    ```typescript
    await client.publish('order.events', orderEvent, { key: orderId });
    ```

Events with the same key are always delivered in order. Events with different keys may be interleaved — design your consumers to handle this.

---

## What You Learned

- **Pub/Sub** — Topic-based messaging with wildcard subscriptions and actor integration
- **Event Sourcing** — Append-only event stores, aggregate reconstruction, and snapshots
- **Schema Validation** — Contract enforcement with versioned schemas and compatibility checking
- **Complete example** — A real-time notification system with pub/sub, event sourcing, and schema validation

---

## Next Steps

| Tutorial | What you'll learn |
|---|---|
| [Performance](./04_performance.md) | Backpressure, rate limiting, circuit breakers, and windowing |
| [Distributed Workflows](./05_distributed_workflows.md) | Sagas, state machines, and human tasks |
| [Examples](../../docs-site/docs/examples/overview.md) | Full example applications |

---

*Time to complete: ~1 hour*
