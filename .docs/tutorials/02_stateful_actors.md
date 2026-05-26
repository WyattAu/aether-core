> **NOTE**: This tutorial uses SDK examples in Python and TypeScript. The Aether v2.0.0 runtime is Rust-native. The concepts described here (state management, event sourcing, pub/sub) are fully supported in the Rust runtime via `aether_actor`. SDK-specific code examples will be updated in a future release. See the Rust API reference for current usage patterns.

# Stateful Actors

**Time:** ~25 minutes | **Prerequisites:** [Getting Started](./01_getting_started.md)

---

## State Management Deep Dive

Every Aether actor has an isolated **StateHandle** — a key-value store scoped to that actor. State survives restarts and migrations, so you can trust it as your source of truth.

=== "Python"

    ```python
    class OrderActor(Actor):
        async def on_start(self):
            raw = await self.state.read("order_count")
            self.order_count = int(raw) if raw else 0

        async def handle_message(self, sender: str, msg: Message) -> Message:
            self.order_count += 1
            await self.state.write("order_count", str(self.order_count))

            await self.state.write(f"order:{self.order_count}", msg.payload)

            exists = await self.state.exists(f"order:{self.order_count}")
            keys = await self.state.list_keys("order:")
            return Message.response({"id": self.order_count, "exists": exists})
    ```

=== "TypeScript"

    ```typescript
    class OrderActor extends Actor {
      private orderCount = 0;

      async onStart(): Promise<void> {
        const raw = await this.state.read('order_count');
        this.orderCount = raw ? parseInt(raw, 10) : 0;
      }

      async handleMessage(sender: string, msg: Message): Promise<Message> {
        this.orderCount += 1;
        await this.state.write('order_count', String(this.orderCount));
        await this.state.write(`order:${this.orderCount}`, JSON.stringify(msg.payload));

        const exists = await this.state.exists(`order:${this.orderCount}`);
        const keys = await this.state.listKeys('order:');
        return Message.response({ id: this.orderCount, exists });
      }
    }
    ```

### StateHandle API Reference

| Method | Description |
|---|---|
| `read(key)` | Read a value by key. Returns `null` if not found. |
| `write(key, value)` | Write a value. Overwrites existing. |
| `exists(key)` | Check if a key exists. |
| `list_keys(prefix)` | List all keys matching a prefix. |
| `delete(key)` | Delete a single key. |
| `clear()` | Delete all keys owned by this actor. |

---

## Event Sourcing

Aether's event system lets you record every state change as an immutable event. This gives you a full audit trail and the ability to reconstruct state from scratch.

=== "Python"

    ```python
    from aether_sdk import Actor, Message, EventStore

    class AuditActor(Actor):
        def __init__(self):
            super().__init__("audit")
            self.require("STATE_READ", "STATE_WRITE", "EVENT_EMIT")
            self.events = EventStore(self)

        async def handle_message(self, sender: str, msg: Message) -> Message:
            match msg.type:
                case "create_order":
                    await self.events.append({
                        "type": "ORDER_CREATED",
                        "order_id": msg.payload["order_id"],
                        "items": msg.payload["items"],
                        "timestamp": msg.payload["timestamp"],
                    })
                    return Message.response({"status": "recorded"})

                case "replay":
                    events = await self.events.query(
                        from_time=msg.payload.get("from"),
                        to_time=msg.payload.get("to"),
                    )
                    return Message.response({"events": events})
    ```

=== "TypeScript"

    ```typescript
    import { Actor, Message, EventStore } from '@aether/sdk';

    class AuditActor extends Actor {
      private events: EventStore;

      constructor() {
        super('audit');
        this.require('STATE_READ', 'STATE_WRITE', 'EVENT_EMIT');
        this.events = new EventStore(this);
      }

      async handleMessage(sender: string, msg: Message): Promise<Message> {
        switch (msg.type) {
          case 'create_order':
            await this.events.append({
              type: 'ORDER_CREATED',
              order_id: msg.payload.order_id,
              items: msg.payload.items,
              timestamp: msg.payload.timestamp,
            });
            return Message.response({ status: 'recorded' });

          case 'replay':
            const events = await this.events.query({
              fromTime: msg.payload.from,
              toTime: msg.payload.to,
            });
            return Message.response({ events });
        }
      }
    }
    ```

---

## Pub/Sub

Subscribe to real-time events from any actor. Pub/Sub is decoupled — publishers don't know who is listening.

=== "Python"

    ```python
    class InventoryActor(Actor):
        async def on_start(self):
            await self.subscribe("order.created", self.on_order_created)
            await self.subscribe("order.cancelled", self.on_order_cancelled)

        async def on_order_created(self, event: dict):
            for item in event["items"]:
                stock = int(await self.state.read(f"stock:{item['sku']}") or "0")
                stock -= item["qty"]
                await self.state.write(f"stock:{item['sku']}", str(stock))
                if stock < 10:
                    await self.publish("inventory.low", {
                        "sku": item["sku"], "stock": stock,
                    })

        async def on_order_cancelled(self, event: dict):
            for item in event["items"]:
                stock = int(await self.state.read(f"stock:{item['sku']}") or "0")
                stock += item["qty"]
                await self.state.write(f"stock:{item['sku']}", str(stock))
    ```

=== "TypeScript"

    ```typescript
    class InventoryActor extends Actor {
      async onStart(): Promise<void> {
        await this.subscribe('order.created', this.onOrderCreated.bind(this));
        await this.subscribe('order.cancelled', this.onOrderCancelled.bind(this));
      }

      private async onOrderCreated(event: any): Promise<void> {
        for (const item of event.items) {
          const raw = await this.state.read(`stock:${item.sku}`) || '0';
          let stock = parseInt(raw, 10) - item.qty;
          await this.state.write(`stock:${item.sku}`, String(stock));
          if (stock < 10) {
            await this.publish('inventory.low', { sku: item.sku, stock });
          }
        }
      }

      private async onOrderCancelled(event: any): Promise<void> {
        for (const item of event.items) {
          const raw = await this.state.read(`stock:${item.sku}`) || '0';
          let stock = parseInt(raw, 10) + item.qty;
          await this.state.write(`stock:${item.sku}`, String(stock));
        }
      }
    }
    ```

---

## Complete Example: Shopping Cart

This example brings everything together — stateful operations, event sourcing for order history, and pub/sub for inventory notifications.

=== "Python"

    ```python
    import asyncio
    import time
    from aether_sdk import Actor, Message, EventStore

    class ShoppingCartActor(Actor):
        def __init__(self):
            super().__init__("cart")
            self.require(
                "STATE_READ", "STATE_WRITE",
                "EVENT_EMIT", "ACTOR_MESSAGING", "LOG",
            )
            self.events = EventStore(self)
            self.items: dict[str, dict] = {}

        async def on_start(self):
            raw = await self.state.read("items")
            if raw:
                self.items = json.loads(raw)
            self.log.info(f"Cart restored with {len(self.items)} items")

        async def _persist(self):
            await self.state.write("items", json.dumps(self.items))

        async def handle_message(self, sender: str, msg: Message) -> Message:
            match msg.type:
                case "add_item":
                    sku = msg.payload["sku"]
                    qty = msg.payload["qty"]
                    price = msg.payload["price"]

                    if sku in self.items:
                        self.items[sku]["qty"] += qty
                    else:
                        self.items[sku] = {"sku": sku, "qty": qty, "price": price}

                    await self._persist()
                    await self.events.append({
                        "type": "ITEM_ADDED",
                        "sku": sku, "qty": qty, "price": price,
                        "timestamp": time.time(),
                    })
                    await self.publish("cart.updated", {"cart_total": await self._total()})
                    return Message.response({"sku": sku, "total_qty": self.items[sku]["qty"]})

                case "remove_item":
                    sku = msg.payload["sku"]
                    if sku not in self.items:
                        return Message.error(f"item {sku} not in cart")

                    removed = self.items.pop(sku)
                    await self._persist()
                    await self.events.append({
                        "type": "ITEM_REMOVED",
                        "sku": sku, "qty": removed["qty"],
                        "timestamp": time.time(),
                    })
                    await self.publish("cart.updated", {"cart_total": await self._total()})
                    return Message.response({"removed": sku})

                case "get_cart":
                    total = await self._total()
                    return Message.response({"items": list(self.items.values()), "total": total})

                case "checkout":
                    order_id = f"ORD-{int(time.time())}"
                    await self.events.append({
                        "type": "ORDER_PLACED",
                        "order_id": order_id,
                        "items": list(self.items.values()),
                        "total": await self._total(),
                        "timestamp": time.time(),
                    })
                    await self.publish("order.created", {
                        "order_id": order_id,
                        "items": list(self.items.values()),
                    })
                    self.items = {}
                    await self._persist()
                    return Message.response({"order_id": order_id, "status": "confirmed"})

                case "history":
                    events = await self.events.query(
                        from_time=msg.payload.get("from"),
                        limit=msg.payload.get("limit", 50),
                    )
                    return Message.response({"events": events})

                case _:
                    return Message.error("unknown message type")

        async def _total(self) -> float:
            return sum(i["qty"] * i["price"] for i in self.items.values())

    async def main():
        actor = ShoppingCartActor()
        await actor.start()
        await actor.run()

    if __name__ == "__main__":
        asyncio.run(main())
    ```

=== "TypeScript"

    ```typescript
    import { Actor, Message, EventStore } from '@aether/sdk';

    interface CartItem {
      sku: string;
      qty: number;
      price: number;
    }

    class ShoppingCartActor extends Actor {
      private events: EventStore;
      private items: Map<string, CartItem> = new Map();

      constructor() {
        super('cart');
        this.require(
          'STATE_READ', 'STATE_WRITE',
          'EVENT_EMIT', 'ACTOR_MESSAGING', 'LOG',
        );
        this.events = new EventStore(this);
      }

      async onStart(): Promise<void> {
        const raw = await this.state.read('items');
        if (raw) {
          const parsed = JSON.parse(raw) as CartItem[];
          for (const item of parsed) this.items.set(item.sku, item);
        }
        this.log.info(`Cart restored with ${this.items.size} items`);
      }

      private async persist(): Promise<void> {
        await this.state.write('items', JSON.stringify([...this.items.values()]));
      }

      private total(): number {
        let sum = 0;
        for (const item of this.items.values()) sum += item.qty * item.price;
        return sum;
      }

      async handleMessage(sender: string, msg: Message): Promise<Message> {
        switch (msg.type) {
          case 'add_item': {
            const { sku, qty, price } = msg.payload;
            const existing = this.items.get(sku);
            if (existing) {
              existing.qty += qty;
            } else {
              this.items.set(sku, { sku, qty, price });
            }
            await this.persist();
            await this.events.append({
              type: 'ITEM_ADDED', sku, qty, price, timestamp: Date.now(),
            });
            await this.publish('cart.updated', { cartTotal: this.total() });
            return Message.response({ sku, totalQty: this.items.get(sku)!.qty });
          }

          case 'remove_item': {
            const { sku } = msg.payload;
            const removed = this.items.get(sku);
            if (!removed) return Message.error(`item ${sku} not in cart`);

            this.items.delete(sku);
            await this.persist();
            await this.events.append({
              type: 'ITEM_REMOVED', sku, qty: removed.qty, timestamp: Date.now(),
            });
            await this.publish('cart.updated', { cartTotal: this.total() });
            return Message.response({ removed: sku });
          }

          case 'get_cart': {
            return Message.response({
              items: [...this.items.values()],
              total: this.total(),
            });
          }

          case 'checkout': {
            const orderId = `ORD-${Date.now()}`;
            await this.events.append({
              type: 'ORDER_PLACED',
              orderId,
              items: [...this.items.values()],
              total: this.total(),
              timestamp: Date.now(),
            });
            await this.publish('order.created', {
              orderId,
              items: [...this.items.values()],
            });
            this.items.clear();
            await this.persist();
            return Message.response({ orderId, status: 'confirmed' });
          }

          case 'history': {
            const events = await this.events.query({
              fromTime: msg.payload.from,
              limit: msg.payload.limit ?? 50,
            });
            return Message.response({ events });
          }

          default:
            return Message.error('unknown message type');
        }
      }
    }

    async function main() {
      const actor = new ShoppingCartActor();
      await actor.start();
      await actor.run();
    }

    main().catch(console.error);
    ```

### Walkthrough

1. **`add_item`** — Upserts the item in the in-memory map, persists to state, records an `ITEM_ADDED` event, and publishes a `cart.updated` notification.
2. **`remove_item`** — Deletes the item, persists, records `ITEM_REMOVED`, publishes the new total.
3. **`get_cart`** — Returns the current cart contents and total. No side effects.
4. **`checkout`** — Records an `ORDER_PLACED` event for the full order history, publishes `order.created` so inventory actors can react, then clears the cart.
5. **`history`** — Queries the event store to reconstruct what happened over any time range.

### Try It

```bash
# Add items
aether call cart add_item '{"sku":"WIDGET-1","qty":3,"price":9.99}'
aether call cart add_item '{"sku":"WIDGET-2","qty":1,"price":24.99}'

# Check total
aether call cart get_cart '{}'

# Checkout
aether call cart checkout '{}'

# View event history
aether call cart history '{"limit":10}'
```

---

## What You Learned

- **StateHandle API** — Isolated key-value persistence per actor
- **Event sourcing** — Immutable event log for audit trails and state reconstruction
- **Pub/Sub** — Decoupled real-time event subscriptions
- **Full example** — A shopping cart with state, events, and notifications

---

## Next Steps

| Tutorial | What you'll learn |
|---|---|
| [Performance](./04_performance.md) | Backpressure, rate limiting, circuit breakers, and windowing |
| [Examples](../../docs-site/docs/examples/overview.md) | More complete applications |

---

*Time to complete: ~25 minutes*
