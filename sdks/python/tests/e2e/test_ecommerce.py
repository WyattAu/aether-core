"""E2E Scenario 1: E-Commerce Order Processing

Simulates a complete e-commerce flow with saga-based compensation:
- Order Actor manages order state (Created -> Paid -> Fulfilled -> Shipped)
- Payment Actor processes payments (succeeds or fails)
- Inventory Actor manages stock levels
- Shipping Actor schedules shipments
- Full saga with compensation on failure
"""

import random
from typing import Any, Dict, Optional

import pytest
from aether_sdk.actor import Actor
from aether_sdk.messaging import Message, MessageType
from aether_sdk.state import StateHandle
from aether_sdk.workflow.saga import Saga, SagaExecutor
from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor
from aether_sdk.workflow.types import (
    Duration,
    RetryConfig,
    RetryPolicy,
    SagaContext,
    SagaStatus,
)

random.seed(42)


@pytest.mark.e2e
class TestEcommerceOrderProcessing:
    """End-to-end tests for the e-commerce order processing pipeline."""

    @pytest.fixture
    def inventory(self):
        store: Dict[str, int] = {"widget": 100, "gadget": 50, "doohickey": 25}
        return store

    @pytest.fixture
    def payment_results(self):
        return {"success": True, "attempts": 0}

    @pytest.fixture
    def shipping_results(self):
        return {"success": True, "attempts": 0}

    @pytest.fixture
    def audit_log(self):
        return []

    def _build_order_saga(
        self,
        inventory,
        payment_results,
        shipping_results,
        audit_log,
        force_payment_fail=False,
        force_shipping_fail=False,
    ):
        async def validate_order(ctx: SagaContext) -> Dict[str, Any]:
            order = ctx.input
            audit_log.append(f"VALIDATE: order {order['order_id']}")
            if order.get("items") is None or len(order["items"]) == 0:
                raise ValueError("Order has no items")
            return {"order_id": order["order_id"], "items": order["items"]}

        async def process_payment(ctx: SagaContext) -> Dict[str, Any]:
            payment_results["attempts"] += 1
            if force_payment_fail:
                audit_log.append("PAYMENT: FAILED")
                raise RuntimeError("Payment declined")
            audit_log.append("PAYMENT: SUCCESS")
            payment_results["success"] = True
            return {"transaction_id": "txn-123", "amount": 99.99}

        async def refund_payment(ctx: SagaContext) -> None:
            audit_log.append("PAYMENT: REFUNDED")
            payment_results["success"] = False

        async def reserve_inventory(ctx: SagaContext) -> Dict[str, Any]:
            order = ctx.input
            for item in order["items"]:
                product = item["product"]
                qty = item["quantity"]
                if inventory.get(product, 0) < qty:
                    raise RuntimeError(f"Insufficient stock for {product}")
                inventory[product] -= qty
            audit_log.append(f"INVENTORY: Reserved for {order['order_id']}")
            return {"reserved": True}

        async def release_inventory(ctx: SagaContext) -> None:
            audit_log.append("INVENTORY: RELEASED")
            order = ctx.input
            for item in order["items"]:
                product = item["product"]
                qty = item["quantity"]
                inventory[product] = inventory.get(product, 0) + qty

        async def schedule_shipping(ctx: SagaContext) -> Dict[str, Any]:
            if force_shipping_fail:
                audit_log.append("SHIPPING: FAILED")
                raise RuntimeError("Carrier unavailable")
            audit_log.append("SHIPPING: SCHEDULED")
            shipping_results["success"] = True
            return {"tracking_number": "TRACK-001", "carrier": "FastShip"}

        async def cancel_shipping(ctx: SagaContext) -> None:
            audit_log.append("SHIPPING: CANCELLED")
            shipping_results["success"] = False

        saga_def = (
            Saga("order-processing")
            .step("validate-order")
            .action(validate_order)
            .step("reserve-inventory")
            .action(reserve_inventory)
            .compensate(release_inventory)
            .step("process-payment")
            .action(process_payment)
            .compensate(refund_payment)
            .step("schedule-shipping")
            .action(schedule_shipping)
            .compensate(cancel_shipping)
            .build()
        )
        return saga_def

    @pytest.mark.asyncio
    async def test_complete_order_flow(
        self, inventory, payment_results, shipping_results, audit_log
    ):
        """Test the full happy path: order placed -> paid -> shipped."""
        saga_def = self._build_order_saga(
            inventory, payment_results, shipping_results, audit_log
        )
        executor = SagaExecutor()

        order = {
            "order_id": "ORD-001",
            "items": [{"product": "widget", "quantity": 5}],
        }

        result = await executor.execute(saga_def, order)

        assert result.status == SagaStatus.COMPLETED
        assert result.completed_steps == [
            "validate-order",
            "reserve-inventory",
            "process-payment",
            "schedule-shipping",
        ]
        assert inventory["widget"] == 95
        assert payment_results["success"] is True
        assert shipping_results["success"] is True
        assert "VALIDATE: order ORD-001" in audit_log
        assert "PAYMENT: SUCCESS" in audit_log
        assert any("INVENTORY: Reserved" in entry for entry in audit_log)
        assert "SHIPPING: SCHEDULED" in audit_log

        print("\n=== Complete Order Flow Summary ===")
        print(f"  Order ID: {order['order_id']}")
        print(f"  Status: {result.status.value}")
        print(f"  Steps completed: {len(result.completed_steps)}")
        print(f"  Inventory remaining: {inventory}")
        print(f"  Audit log: {audit_log}")

    @pytest.mark.asyncio
    async def test_payment_failure_cancels_order(
        self, inventory, payment_results, shipping_results, audit_log
    ):
        """Test that payment failure triggers compensation (no shipping)."""
        saga_def = self._build_order_saga(
            inventory,
            payment_results,
            shipping_results,
            audit_log,
            force_payment_fail=True,
        )
        executor = SagaExecutor()

        order = {
            "order_id": "ORD-002",
            "items": [{"product": "gadget", "quantity": 2}],
        }

        result = await executor.execute(saga_def, order)

        assert result.status == SagaStatus.COMPENSATED
        assert "reserve-inventory" in result.completed_steps
        assert "process-payment" not in result.completed_steps
        assert "schedule-shipping" not in result.completed_steps
        assert "INVENTORY: RELEASED" in audit_log
        assert "SHIPPING: SCHEDULED" not in audit_log
        assert inventory["gadget"] == 50

        print("\n=== Payment Failure Summary ===")
        print(f"  Order ID: {order['order_id']}")
        print(f"  Status: {result.status.value}")
        print(f"  Completed before failure: {result.completed_steps}")
        print("  Compensation: inventory released, no shipping")
        print(f"  Inventory restored: {inventory}")

    @pytest.mark.asyncio
    async def test_shipping_failure_compensates_all(
        self, inventory, payment_results, shipping_results, audit_log
    ):
        """Test that shipping failure triggers full compensation: refund + restore inventory."""
        saga_def = self._build_order_saga(
            inventory,
            payment_results,
            shipping_results,
            audit_log,
            force_shipping_fail=True,
        )
        executor = SagaExecutor()

        order = {
            "order_id": "ORD-003",
            "items": [{"product": "doohickey", "quantity": 3}],
        }

        result = await executor.execute(saga_def, order)

        assert result.status == SagaStatus.COMPENSATED
        assert "validate-order" in result.completed_steps
        assert "reserve-inventory" in result.completed_steps
        assert "process-payment" in result.completed_steps
        assert "schedule-shipping" not in result.completed_steps
        assert "INVENTORY: RELEASED" in audit_log
        assert "PAYMENT: REFUNDED" in audit_log
        assert "SHIPPING: FAILED" in audit_log
        assert inventory["doohickey"] == 25
        assert payment_results["success"] is False

        print("\n=== Shipping Failure Compensation Summary ===")
        print(f"  Order ID: {order['order_id']}")
        print(f"  Status: {result.status.value}")
        print(f"  Steps completed before failure: {result.completed_steps}")
        print(
            "  Compensation chain: shipping cancelled -> payment refunded -> inventory released"
        )
        print(f"  Inventory restored: {inventory}")
        print(f"  Payment refunded: {not payment_results['success']}")

    @pytest.mark.asyncio
    async def test_multiple_items_order(
        self, inventory, payment_results, shipping_results, audit_log
    ):
        """Test ordering multiple different products in a single order."""
        saga_def = self._build_order_saga(
            inventory, payment_results, shipping_results, audit_log
        )
        executor = SagaExecutor()

        order = {
            "order_id": "ORD-004",
            "items": [
                {"product": "widget", "quantity": 10},
                {"product": "gadget", "quantity": 5},
                {"product": "doohickey", "quantity": 2},
            ],
        }

        result = await executor.execute(saga_def, order)

        assert result.status == SagaStatus.COMPLETED
        assert inventory["widget"] == 90
        assert inventory["gadget"] == 45
        assert inventory["doohickey"] == 23
        assert len(result.completed_steps) == 4

        print("\n=== Multi-Item Order Summary ===")
        print(f"  Order ID: {order['order_id']}")
        print(f"  Status: {result.status.value}")
        print(f"  Items ordered: {len(order['items'])}")
        print(f"  Final inventory: {inventory}")

    @pytest.mark.asyncio
    async def test_order_state_machine_workflow(self, inventory, audit_log):
        """Test order lifecycle using state machine: Created -> Paid -> Fulfilled -> Shipped."""
        order_wf = (
            Workflow("order-lifecycle")
            .state("created", is_initial=True)
            .state("paid")
            .state("fulfilled")
            .state("shipped", is_final=True)
            .state("cancelled", is_final=True)
            .transition("pay", "created", "paid")
            .transition("fulfill", "paid", "fulfilled")
            .transition("ship", "fulfilled", "shipped")
            .transition("cancel", "created", "cancelled")
            .transition("cancel", "paid", "cancelled")
            .build()
        )

        executor = WorkflowExecutor()

        wf_result = await executor.start(order_wf, {"order_id": "ORD-WF-001"})
        assert wf_result.current_state == "created"

        t1 = await executor.transition(wf_result.workflow_id, "pay")
        assert t1.success
        assert t1.to_state == "paid"

        status = await executor.get_status(wf_result.workflow_id)
        assert status.current_state == "paid"

        t2 = await executor.transition(wf_result.workflow_id, "fulfill")
        assert t2.success
        assert t2.to_state == "fulfilled"

        t3 = await executor.transition(wf_result.workflow_id, "ship")
        assert t3.success
        assert t3.to_state == "shipped"

        final = await executor.get_status(wf_result.workflow_id)
        assert final.status.value == "completed"
        assert final.current_state == "shipped"

        audit_log.append(
            f"WORKFLOW: Order {wf_result.workflow_id} completed full lifecycle"
        )

        print("\n=== Order State Machine Summary ===")
        print(f"  Workflow ID: {wf_result.workflow_id}")
        print(f"  Final state: {final.current_state}")
        print(f"  Status: {final.status.value}")
        print(f"  History events: {len(final.history)}")

    @pytest.mark.asyncio
    async def test_saga_with_retry_on_transient_failure(
        self, inventory, payment_results, shipping_results, audit_log
    ):
        """Test that transient payment failures are retried before compensation."""
        attempt_count = {"value": 0}

        async def flaky_payment(ctx: SagaContext) -> Dict[str, Any]:
            attempt_count["value"] += 1
            if attempt_count["value"] < 3:
                audit_log.append(
                    f"PAYMENT: ATTEMPT {attempt_count['value']} FAILED (transient)"
                )
                raise RuntimeError("Transient payment error")
            audit_log.append(f"PAYMENT: ATTEMPT {attempt_count['value']} SUCCEEDED")
            return {"transaction_id": "txn-flaky", "amount": 50.0}

        async def reserve_inv(ctx: SagaContext) -> Dict[str, Any]:
            order = ctx.input
            for item in order["items"]:
                inventory[item["product"]] -= item["quantity"]
            audit_log.append("INVENTORY: Reserved")
            return {"reserved": True}

        async def release_inv(ctx: SagaContext) -> None:
            audit_log.append("INVENTORY: RELEASED")
            order = ctx.input
            for item in order["items"]:
                inventory[item["product"]] += item["quantity"]

        saga_def = (
            Saga("flaky-payment-saga")
            .step("reserve-inventory")
            .action(reserve_inv)
            .compensate(release_inv)
            .step("process-payment")
            .action(flaky_payment)
            .retry(
                RetryConfig(
                    max_attempts=5,
                    policy=RetryPolicy.FIXED,
                    initial_delay=Duration(10),
                    max_delay=Duration(50),
                )
            )
            .build()
        )

        executor = SagaExecutor()
        order = {
            "order_id": "ORD-FLAKY",
            "items": [{"product": "widget", "quantity": 1}],
        }
        result = await executor.execute(saga_def, order)

        assert result.status == SagaStatus.COMPLETED
        assert attempt_count["value"] == 3
        assert inventory["widget"] == 99

        print("\n=== Retry on Transient Failure Summary ===")
        print(f"  Order ID: {order['order_id']}")
        print(f"  Status: {result.status.value}")
        print(f"  Payment attempts: {attempt_count['value']}")
        print(f"  Audit log: {audit_log}")


@pytest.mark.e2e
class TestEcommerceMessagePassing:
    """Test e-commerce actors communicating via messages."""

    @pytest.mark.asyncio
    async def test_actor_message_based_order_processing(self):
        """Test actors processing order via message passing with StateHandle persistence."""

        class OrderActor(Actor):
            def __init__(self):
                super().__init__()
                self._state = StateHandle()
                self.processed_orders: list = []

            @classmethod
            def name(cls) -> str:
                return "order-actor"

            async def handle_message(
                self, sender: str, message: Message
            ) -> Optional[Message]:
                if message.payload.get("action") == "create":
                    order = message.payload["order"]
                    await self._state.set_json(
                        f"order:{order['id']}",
                        {
                            "id": order["id"],
                            "status": "created",
                            "items": order["items"],
                        },
                    )
                    self.processed_orders.append(order["id"])
                    return Message(
                        type=MessageType.CUSTOM, payload={"status": "created"}
                    )
                return None

        class PaymentActor(Actor):
            def __init__(self):
                super().__init__()
                self._state = StateHandle()
                self.payments: list = []

            @classmethod
            def name(cls) -> str:
                return "payment-actor"

            async def handle_message(
                self, sender: str, message: Message
            ) -> Optional[Message]:
                if message.payload.get("action") == "charge":
                    order_id = message.payload["order_id"]
                    amount = message.payload["amount"]
                    txn_id = f"txn-{order_id}"
                    self.payments.append(
                        {"order_id": order_id, "amount": amount, "txn_id": txn_id}
                    )
                    await self._state.set_json(
                        f"payment:{txn_id}",
                        {
                            "order_id": order_id,
                            "amount": amount,
                            "status": "completed",
                        },
                    )
                    return Message(
                        type=MessageType.CUSTOM,
                        payload={"txn_id": txn_id, "status": "success"},
                    )
                return None

        order_actor = OrderActor()
        payment_actor = PaymentActor()

        order_msg = Message(
            type=MessageType.CUSTOM,
            payload={
                "action": "create",
                "order": {"id": "ORD-MSG-001", "items": ["widget"]},
            },
        )
        response = await order_actor.handle_message("customer", order_msg)
        assert response is not None
        assert response.payload["status"] == "created"
        assert "ORD-MSG-001" in order_actor.processed_orders

        saved_order = await order_actor._state.get_json("order:ORD-MSG-001")
        assert saved_order is not None
        assert saved_order["status"] == "created"

        payment_msg = Message(
            type=MessageType.CUSTOM,
            payload={"action": "charge", "order_id": "ORD-MSG-001", "amount": 49.99},
        )
        payment_response = await payment_actor.handle_message(
            "order-actor", payment_msg
        )
        assert payment_response.payload["status"] == "success"
        assert len(payment_actor.payments) == 1

        saved_payment = await payment_actor._state.get_json("payment:txn-ORD-MSG-001")
        assert saved_payment["status"] == "completed"

        print("\n=== Actor Message-Based Order Summary ===")
        print(f"  Orders processed: {order_actor.processed_orders}")
        print(f"  Payments completed: {len(payment_actor.payments)}")
        print("  StateHandle persistence verified for both actors")
