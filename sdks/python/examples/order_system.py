"""
Event-Sourced Order System using the Aether SDK.

This example demonstrates:
  - Event sourcing for order lifecycle (OrderCreated, PaymentProcessed, Shipped, Delivered)
  - State reconstruction from events
  - Saga pattern for order processing (with compensation on failure)
  - Schema validation for events

The full order lifecycle is:
    Create -> Pay -> Ship -> Deliver

If payment fails, the saga automatically compensates by cancelling
inventory reservation and marking the order as failed.

Usage:
    python order_system.py
"""

import asyncio
from datetime import datetime, timezone

from aether_sdk.event import (
    Aggregate,
    InMemoryEventStore,
    InMemorySchemaRegistry,
    Schema,
)
from aether_sdk.event.schema import SchemaError
from aether_sdk.workflow.saga import Saga, SagaExecutor
from aether_sdk.workflow.types import (
    Duration,
    RetryConfig,
    RetryPolicy,
    SagaContext,
    SagaResult,
)

# -------------------------------------------------------------------
# Event schemas (for validation)
# -------------------------------------------------------------------

ORDER_CREATED_SCHEMA = Schema(
    name="OrderCreated",
    type="json",
    definition={
        "type": "object",
        "properties": {
            "order_id": {"type": "string"},
            "customer_id": {"type": "string"},
            "items": {
                "type": "array",
                "items": {"type": "object"},
            },
            "total": {"type": "number"},
        },
        "required": ["order_id", "customer_id", "items", "total"],
    },
)

PAYMENT_PROCESSED_SCHEMA = Schema(
    name="PaymentProcessed",
    type="json",
    definition={
        "type": "object",
        "properties": {
            "order_id": {"type": "string"},
            "amount": {"type": "number"},
            "payment_id": {"type": "string"},
        },
        "required": ["order_id", "amount", "payment_id"],
    },
)

SHIPPED_SCHEMA = Schema(
    name="OrderShipped",
    type="json",
    definition={
        "type": "object",
        "properties": {
            "order_id": {"type": "string"},
            "tracking_number": {"type": "string"},
            "carrier": {"type": "string"},
        },
        "required": ["order_id", "tracking_number"],
    },
)

DELIVERED_SCHEMA = Schema(
    name="OrderDelivered",
    type="json",
    definition={
        "type": "object",
        "properties": {
            "order_id": {"type": "string"},
            "delivered_at": {"type": "string"},
        },
        "required": ["order_id"],
    },
)


# -------------------------------------------------------------------
# Order Aggregate (event-sourced)
# -------------------------------------------------------------------


class Order(Aggregate):
    """
    Event-sourced order aggregate.

    State is rebuilt entirely from the event stream. Each event type
    has a corresponding ``apply_<event_type>`` method that mutates
    the in-memory state.
    """

    def __init__(self):
        super().__init__()
        self.status: str = "pending"
        self.customer_id: str = ""
        self.items: list = []
        self.total: float = 0.0
        self.payment_id: str | None = None
        self.tracking_number: str | None = None
        self.carrier: str | None = None
        self.delivered_at: str | None = None

    def apply_order_created(self, payload: dict) -> None:
        self.status = "created"
        self.customer_id = payload["customer_id"]
        self.items = payload["items"]
        self.total = payload["total"]

    def apply_payment_processed(self, payload: dict) -> None:
        self.status = "paid"
        self.payment_id = payload.get("payment_id")

    def apply_order_shipped(self, payload: dict) -> None:
        self.status = "shipped"
        self.tracking_number = payload.get("tracking_number")
        self.carrier = payload.get("carrier")

    def apply_order_delivered(self, payload: dict) -> None:
        self.status = "delivered"
        self.delivered_at = payload.get("delivered_at")

    def apply_order_cancelled(self, payload: dict) -> None:
        self.status = "cancelled"

    def apply_inventory_reserved(self, payload: dict) -> None:
        pass

    def apply_inventory_released(self, payload: dict) -> None:
        pass

    def __repr__(self) -> str:
        return (
            f"Order(id={self._id!r}, status={self.status!r}, "
            f"total={self.total}, items={len(self.items)})"
        )


# -------------------------------------------------------------------
# Saga step handlers (simulate external services)
# -------------------------------------------------------------------


async def reserve_inventory(ctx: SagaContext) -> dict:
    """
    Step 1: Reserve inventory for the order.

    In a real system this would call an inventory service. Here we
    simulate success and emit an event.
    """
    order_id = ctx.input["order_id"]
    print(f"  [Saga] Reserving inventory for order {order_id}...")
    ctx.set_state("inventory_reserved", True)
    ctx.set_state("step_reserve_inventory", "completed")
    return {"reserved": True}


async def release_inventory(ctx: SagaContext) -> None:
    """
    Compensation for Step 1: Release reserved inventory.
    """
    order_id = ctx.input["order_id"]
    print(f"  [Saga] COMPENSATE: Releasing inventory for order {order_id}...")
    ctx.set_state("inventory_reserved", False)
    ctx.set_state("step_reserve_inventory", "compensated")


async def process_payment(ctx: SagaContext, simulate_failure: bool = False) -> dict:
    """
    Step 2: Process payment for the order.

    If ``simulate_failure`` is True the payment is rejected, triggering
    saga compensation (inventory release).
    """
    order_id = ctx.input["order_id"]
    amount = ctx.input["total"]

    print(f"  [Saga] Processing payment of ${amount:.2f} for order {order_id}...")

    if simulate_failure:
        print(f"  [Saga] PAYMENT FAILED for order {order_id}!")
        raise RuntimeError(f"Payment declined for order {order_id}")

    payment_id = f"pay-{order_id}"
    print(f"  [Saga] Payment successful ({payment_id}).")
    ctx.set_state("payment_id", payment_id)
    ctx.set_state("step_process_payment", "completed")
    return {"payment_id": payment_id, "amount": amount}


async def refund_payment(ctx: SagaContext) -> None:
    """
    Compensation for Step 2: Refund the payment.
    """
    order_id = ctx.input["order_id"]
    print(f"  [Saga] COMPENSATE: Refunding payment for order {order_id}...")
    ctx.set_state("payment_id", None)
    ctx.set_state("step_process_payment", "compensated")


async def ship_order(ctx: SagaContext) -> dict:
    """
    Step 3: Ship the order.
    """
    order_id = ctx.input["order_id"]
    carrier = ctx.input.get("carrier", "ACME Express")
    tracking = f"TRK-{order_id.upper()}"

    print(f"  [Saga] Shipping order {order_id} via {carrier} ({tracking})...")
    ctx.set_state("tracking_number", tracking)
    ctx.set_state("carrier", carrier)
    ctx.set_state("step_ship_order", "completed")
    return {"tracking_number": tracking, "carrier": carrier}


async def cancel_shipment(ctx: SagaContext) -> None:
    """
    Compensation for Step 3: Cancel the shipment.
    """
    order_id = ctx.input["order_id"]
    print(f"  [Saga] COMPENSATE: Cancelling shipment for order {order_id}...")
    ctx.set_state("tracking_number", None)
    ctx.set_state("step_ship_order", "compensated")


# -------------------------------------------------------------------
# Helpers
# -------------------------------------------------------------------


def print_order_state(order: Order, label: str) -> None:
    print(f"  [{label}] {order}")


def print_separator(title: str) -> None:
    print()
    print(f"--- {title} ---")


# -------------------------------------------------------------------
# Main demo
# -------------------------------------------------------------------


async def main() -> None:
    print("=" * 60)
    print("  Aether SDK - Event-Sourced Order System Example")
    print("=" * 60)

    # ----------------------------------------------------------------
    # 1. Register event schemas
    # ----------------------------------------------------------------
    print_separator("Schema Registration")
    registry = InMemorySchemaRegistry()
    await registry.register("OrderCreated", ORDER_CREATED_SCHEMA)
    await registry.register("PaymentProcessed", PAYMENT_PROCESSED_SCHEMA)
    await registry.register("OrderShipped", SHIPPED_SCHEMA)
    await registry.register("OrderDelivered", DELIVERED_SCHEMA)
    print(
        "  Registered schemas: OrderCreated, PaymentProcessed, OrderShipped, OrderDelivered"
    )

    # Validate a correct event
    valid_event = {
        "order_id": "ord-001",
        "customer_id": "cust-1",
        "items": [],
        "total": 29.99,
    }
    is_valid = await registry.validate("OrderCreated", valid_event)
    print(f"  Validation (valid event): {is_valid}")

    # Validate an incorrect event (missing required fields)
    try:
        await registry.validate("OrderCreated", {"order_id": "ord-002"})
    except SchemaError as e:
        print(f"  Validation (invalid event): SchemaError -> {e}")

    # ----------------------------------------------------------------
    # 2. Event sourcing: build order from events
    # ----------------------------------------------------------------
    print_separator("Event Sourcing - Build Order from Events")
    event_store = InMemoryEventStore()

    order_id = "ord-100"
    await event_store.append(
        order_id,
        [
            {"type": "inventory_reserved", "order_id": order_id},
            {
                "type": "order_created",
                "order_id": order_id,
                "customer_id": "cust-42",
                "items": [
                    {"sku": "WIDGET-1", "name": "Widget", "qty": 2, "price": 9.99},
                    {"sku": "GEAR-7", "name": "Gear", "qty": 1, "price": 10.01},
                ],
                "total": 29.99,
            },
            {
                "type": "payment_processed",
                "order_id": order_id,
                "amount": 29.99,
                "payment_id": "pay-ord-100",
            },
            {
                "type": "order_shipped",
                "order_id": order_id,
                "tracking_number": "TRK-ORD100",
                "carrier": "ACME Express",
            },
            {
                "type": "order_delivered",
                "order_id": order_id,
                "delivered_at": datetime.now(timezone.utc).isoformat(),
            },
        ],
    )

    # Reconstruct the order from its event stream
    order = Order()
    order.id = order_id
    events = await event_store.get_events(order_id)
    order.load_from_history(events)
    print_order_state(order, "Reconstructed")

    # ----------------------------------------------------------------
    # 3. Saga: successful order processing
    # ----------------------------------------------------------------
    print_separator("Saga - Successful Order (create -> pay -> ship)")
    saga = (
        Saga("order-processing")
        .step("reserve-inventory")
        .action(reserve_inventory)
        .compensate(release_inventory)
        .step("process-payment")
        .action(process_payment)
        .compensate(refund_payment)
        .step("ship-order")
        .action(ship_order)
        .compensate(cancel_shipment)
        .build()
    )

    executor = SagaExecutor(
        default_retry=RetryConfig(
            max_attempts=1,
            policy=RetryPolicy.NONE,
            initial_delay=Duration.from_seconds(0),
        ),
        default_timeout=Duration.from_seconds(10),
    )

    order_input = {
        "order_id": "ord-200",
        "customer_id": "cust-77",
        "items": [{"sku": "GIZMO-3", "name": "Gizmo", "qty": 1, "price": 49.99}],
        "total": 49.99,
        "carrier": "FastShip",
    }

    result: SagaResult = await executor.execute(saga, order_input)
    print(
        f"  [Saga Result] status={result.status.value}, "
        f"steps={result.completed_steps}, "
        f"duration={result.duration_ms}ms"
    )

    # ----------------------------------------------------------------
    # 4. Saga: payment failure triggers compensation
    # ----------------------------------------------------------------
    print_separator("Saga - Payment Failure (compensation flow)")

    failed_saga = (
        Saga("order-processing-failed")
        .step("reserve-inventory")
        .action(reserve_inventory)
        .compensate(release_inventory)
        .step("process-payment")
        .action(lambda ctx: process_payment(ctx, simulate_failure=True))
        .compensate(refund_payment)
        .step("ship-order")
        .action(ship_order)
        .compensate(cancel_shipment)
        .build()
    )

    failed_order_input = {
        "order_id": "ord-300",
        "customer_id": "cust-88",
        "items": [{"sku": "THING-9", "name": "Thing", "qty": 3, "price": 15.00}],
        "total": 45.00,
        "carrier": "SlowMail",
    }

    failed_result = await executor.execute(failed_saga, failed_order_input)
    print(
        f"  [Saga Result] status={failed_result.status.value}, "
        f"error={failed_result.error}, "
        f"compensated_steps={failed_result.compensated_steps}"
    )

    # ----------------------------------------------------------------
    # 5. State reconstruction after saga events
    # ----------------------------------------------------------------
    print_separator("State Reconstruction after Saga Events")

    # Emit events from the successful saga into the event store
    saga_order_id = order_input["order_id"]
    saga_events = [
        {"type": "inventory_reserved", "order_id": saga_order_id},
        {"type": "order_created", **order_input},
        {
            "type": "payment_processed",
            "order_id": saga_order_id,
            "amount": order_input["total"],
            "payment_id": "pay-ord-200",
        },
        {
            "type": "order_shipped",
            "order_id": saga_order_id,
            "tracking_number": "TRK-ORD200",
            "carrier": "FastShip",
        },
    ]
    await event_store.append(saga_order_id, saga_events)

    # Rebuild the order from the event store
    saga_order = Order()
    saga_order.id = saga_order_id
    saga_order_events = await event_store.get_events(saga_order_id)
    saga_order.load_from_history(saga_order_events)
    print_order_state(saga_order, "Reconstructed from saga events")

    # ----------------------------------------------------------------
    # 6. Snapshot optimization
    # ----------------------------------------------------------------
    print_separator("Snapshot Optimization")

    # Take a snapshot of the current order state
    snapshot = saga_order.create_snapshot()
    print(
        f"  Snapshot taken at version {snapshot.version}: "
        f"status={snapshot.state.get('status')}"
    )

    # Save and reload from snapshot
    await event_store.save_snapshot(snapshot)

    # Create a fresh order and load from snapshot + remaining events
    reloaded = Order()
    reloaded.id = saga_order_id
    saved_snapshot = await event_store.load_snapshot(saga_order_id)
    remaining_events = await event_store.get_events(
        saga_order_id, after_version=saved_snapshot.version
    )
    reloaded.load_from_history(remaining_events, saved_snapshot)
    print_order_state(reloaded, "Reloaded from snapshot")

    print()
    print("=" * 60)
    print("  Order system demo complete!")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
