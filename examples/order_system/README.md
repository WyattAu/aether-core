# Real-Time Order Processing System

An end-to-end demo that shows how to use the Aether framework to build a distributed, event-driven order processing system.

## What This Demo Shows

- **Actor registration** — five service actors registered with the Aether server
- **Message passing** — actors communicate via directed messages
- **State management** — each service maintains its own isolated state
- **Pub/sub events** — an event bus broadcasts order lifecycle events
- **Event sourcing** — every order records a full, immutable event stream
- **Saga-like orchestration** — a simplified orchestrator steps through the order pipeline

## Architecture

```
Customer
  |
  v
+----------------+     +-------------------+     +----------------+
| Order Service  |---->| Inventory Service |---->| Payment Service|
| (orchestrator) |     | (stock checks,    |     | (charges,      |
|                |     |  reservations)    |     |  refunds)      |
+----------------+     +-------------------+     +----------------+
        |                      |                        |
        |                      |                        |
        +----------------------+------------------------+
                               |
                    Event Bus (pub/sub)
                               |
              +----------------+-----------------+
              |                                  |
   +--------------------+            +--------------------+
   | Notification Svc   |            | Analytics Svc      |
   | (order confirmations           | (totals, metrics,  |
   |  shipping updates) |            |  per-order stats)  |
   +--------------------+            +--------------------+
```

### Data Flow

1. **Customer** sends a `create_order` message to the Order Service
2. **Order Service** asks Inventory Service to check and reserve stock
3. **Order Service** asks Payment Service to charge the customer
4. On success, Inventory Service deducts the reserved stock
5. Order Service marks the order as *completed*
6. Events are published to the bus: `orders.completed`, `inventory.updated`, `payments.processed`
7. Notification Service and Analytics Service consume events from the bus
8. The full lifecycle is recorded as an immutable event stream (event sourcing)

## Services

| Actor ID | Role | Capabilities |
|---|---|---|
| `order-service` | Orchestrates the order pipeline | `orders`, `events` |
| `inventory-service` | Manages stock reservations and deductions | `inventory` |
| `payment-service` | Processes charges and refunds | `payments` |
| `notification-service` | Sends order confirmations | `notifications` |
| `analytics-service` | Aggregates order metrics | `analytics` |

### Pub/Sub Topics

| Topic | Subscribers |
|---|---|
| `orders.*` | notification-service, analytics-service |
| `inventory.*` | analytics-service |
| `payments.*` | notification-service |

## Running

```bash
# From the repo root
python examples/order_system/demo.py
```

### Prerequisites

- Python 3.10+
- Dependencies: `httpx`, `uvicorn`, `fastapi`, `pydantic`

Install them if needed:

```bash
pip install httpx uvicorn fastapi pydantic
```

### What You'll See

```
Starting Aether reference server...
Server is ready

==============================================================
  Aether Demo: Real-Time Order Processing System
==============================================================

Setting up services...
  Registered: order-service (service, caps=['orders', 'events'])
  Registered: inventory-service (service, caps=['inventory'])
  ...

--- Order #1 ---
============================================================
  Order ORD-171...-001 — COMPLETED
============================================================
  1. [order-service            ] received (order: ORD-...)
  2. [inventory-service        ] checking (order: ORD-...)
  3. [payment-service          ] processing (order: ORD-...)
  4. [inventory-service        ] deducting (order: ORD-...)
  5. [analytics-service        ] recording (order: ORD-...)
  ...

============================================================
  SUMMARY
============================================================
  Orders processed: 3
  Total items:      6
  Total revenue:    $179.95
  Events recorded:  15
  Active actors:    5
  Active topics:    4
```

## How It Works

### Server Lifecycle

The demo starts the Aether reference server as a subprocess, polls `/health` until it's ready, then connects the SDK client. On exit (including `Ctrl+C`), the server is terminated cleanly.

### Order Pipeline

Each order flows through these steps:

1. **Order Service** receives the order, stores initial state
2. **Inventory Service** checks availability, stores a reservation hold
3. **Payment Service** charges the card, stores payment record
4. **Inventory Service** deducts stock from the hold
5. **Order Service** updates state to `completed`
6. Three events are published to the bus
7. **Notification Service** records a confirmation
8. **Analytics Service** increments aggregate counters

### Event Sourcing

Every order appends five events to an immutable log:

| # | Event Type | Payload |
|---|---|---|
| 1 | `OrderCreated` | items, total |
| 2 | `InventoryChecked` | available: true |
| 3 | `PaymentProcessed` | amount |
| 4 | `InventoryDeducted` | items |
| 5 | `OrderCompleted` | status |

These can be replayed to reconstruct the full order state at any point in time.

## Extending the Demo

### Add a New Service

```python
await client.register_actor("fraud-service", "service", capabilities=["fraud"])
await client.subscribe("payments.*", "fraud-service")
```

### Add Failure Handling

Modify `process_order` to simulate payment failures and roll back inventory reservations (a full saga with compensating actions).

### Add Streaming Consumers

Use the WebSocket endpoint (`ws://localhost:8080/ws/{actor_id}`) to stream messages and events in real-time instead of polling.

### Run Against a Remote Server

```python
demo = OrderProcessingDemo(base_url="https://your-aether-server.com")
```

Just skip `start_server()` and `stop_server()` if the server is already running.
