> **NOTE**: This tutorial uses SDK examples in Python and TypeScript. The Aether v2.0.0 runtime is Rust-native. The workflow patterns described here (saga, orchestration, distributed transactions) are supported via the mesh networking layer. SDK-specific code examples will be updated in a future release.

# Distributed Workflows

**Time:** ~1 hour | **Prerequisites:** [Getting Started](./01_getting_started.md), [Event-Driven Systems](./03_event_driven.md)

---

## Workflow Patterns Overview

Distributed workflows coordinate actions across multiple services. Unlike a single function call, a workflow spans time and services — it can pause for human approval, retry failed steps, and compensate partial results.

Aether supports three workflow patterns:

| Pattern | Best for | Key trait |
|---|---|---|
| **State Machine** | Fixed flows with known states | Explicit states and transitions |
| **Saga** | Multi-service transactions with rollback | Compensating actions on failure |
| **Choreography** | Loose coupling between services | No central coordinator |

This tutorial covers all three, with a focus on the **orchestrated** approach (state machines + sagas) where a central workflow engine coordinates the steps.

---

## State Machines

A state machine defines a set of states and the transitions between them. Aether's `Workflow` class provides a fluent API for building workflows.

### Defining a Workflow

=== "Python"

    ```python
    from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor
    from aether_sdk.workflow.types import Duration

    order_workflow = (
        Workflow("order-workflow")
        .state("created", is_initial=True)
        .state("validated")
        .state("processing")
        .state("shipped")
        .state("delivered", is_final=True)
        .transition("validate", "created", "validated")
        .transition("process", "validated", "processing")
        .transition("ship", "processing", "shipped")
        .transition("deliver", "shipped", "delivered")
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    import { Workflow, WorkflowExecutor } from '@aether/sdk/workflow';

    const orderWorkflow = new Workflow('order-workflow')
      .state('created', { isInitial: true })
      .state('validated')
      .state('processing')
      .state('shipped')
      .state('delivered', { isFinal: true })
      .transition('validate', 'created', 'validated')
      .transition('process', 'validated', 'processing')
      .transition('ship', 'processing', 'shipped')
      .transition('deliver', 'shipped', 'delivered')
      .build();
    ```

### Guards

Guards are predicates that determine whether a transition is allowed. If the guard returns `false`, the transition is rejected.

=== "Python"

    ```python
    def has_valid_address(ctx):
        return bool(ctx.get_variable("shipping_address"))

    def has_payment_method(ctx):
        return bool(ctx.get_variable("payment_method"))

    order_workflow = (
        Workflow("order-workflow")
        .state("created", is_initial=True)
        .state("validated")
        .state("rejected", is_final=True)
        .state("processing")
        .state("delivered", is_final=True)
        .transition("validate", "created", "validated", guard=has_valid_address)
        .transition("reject", "created", "rejected")
        .transition("process", "validated", "processing", guard=has_payment_method)
        .transition("deliver", "processing", "delivered")
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    const orderWorkflow = new Workflow('order-workflow')
      .state('created', { isInitial: true })
      .state('validated')
      .state('rejected', { isFinal: true })
      .state('processing')
      .state('delivered', { isFinal: true })
      .transition('validate', 'created', 'validated', {
        guard: (ctx) => !!ctx.getVariable('shipping_address'),
      })
      .transition('reject', 'created', 'rejected')
      .transition('process', 'validated', 'processing', {
        guard: (ctx) => !!ctx.getVariable('payment_method'),
      })
      .transition('deliver', 'processing', 'delivered')
      .build();
    ```

### Side Effects: on_enter and on_exit

Execute logic when entering or leaving a state — useful for sending notifications, updating external systems, or logging.

=== "Python"

    ```python
    async def on_enter_processing(ctx):
        ctx.set_variable("processing_started_at", time.time())
        print(f"Order {ctx.input['order_id']} is now being processed")

    async def on_exit_shipped(ctx):
        await publish("order.shipped", {"order_id": ctx.input["order_id"]})

    order_workflow = (
        Workflow("order-workflow")
        .state("created", is_initial=True)
        .state("processing")
        .state("shipped")
        .state("delivered", is_final=True)
        .on_enter("processing", on_enter_processing)
        .on_exit("shipped", on_exit_shipped)
        .transition("process", "created", "processing")
        .transition("ship", "processing", "shipped")
        .transition("deliver", "shipped", "delivered")
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    async function onEnterProcessing(ctx: WorkflowContext): Promise<void> {
      ctx.setVariable('processing_started_at', Date.now());
      console.log(`Order ${ctx.input.order_id} is now being processed`);
    }

    async function onExitShipped(ctx: WorkflowContext): Promise<void> {
      await publish('order.shipped', { order_id: ctx.input.order_id });
    }

    const orderWorkflow = new Workflow('order-workflow')
      .state('created', { isInitial: true })
      .state('processing')
      .state('shipped')
      .state('delivered', { isFinal: true })
      .onEnter('processing', onEnterProcessing)
      .onExit('shipped', onExitShipped)
      .transition('process', 'created', 'processing')
      .transition('ship', 'processing', 'shipped')
      .transition('deliver', 'shipped', 'delivered')
      .build();
    ```

### Executing Transitions

=== "Python"

    ```python
    executor = WorkflowExecutor()

    result = await executor.start(order_workflow, {"order_id": "ORD-1"})
    print(f"Current state: {result.current_state}")

    await executor.transition(result.workflow_id, "process")
    await executor.transition(result.workflow_id, "ship")
    await executor.transition(result.workflow_id, "deliver")

    status = await executor.get_status(result.workflow_id)
    print(f"Final status: {status.status}")
    ```

=== "TypeScript"

    ```typescript
    const executor = new WorkflowExecutor();

    const result = await executor.start(orderWorkflow, { order_id: 'ORD-1' });
    console.log(`Current state: ${result.current_state}`);

    await executor.transition(result.workflow_id, 'process');
    await executor.transition(result.workflow_id, 'ship');
    await executor.transition(result.workflow_id, 'deliver');

    const status = await executor.getStatus(result.workflow_id);
    console.log(`Final status: ${status.status}`);
    ```

### Timeout Transitions

States can have automatic timeouts that fire a transition if the state isn't exited in time.

=== "Python"

    ```python
    from aether_sdk.workflow.types import Duration

    order_workflow = (
        Workflow("order-workflow")
        .state("payment_pending", is_initial=True, timeout=Duration.from_minutes(30))
        .state("payment_received")
        .state("payment_expired", is_final=True)
        .transition("pay", "payment_pending", "payment_received")
        .transition("expire", "payment_pending", "payment_expired")
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    const orderWorkflow = new Workflow('order-workflow')
      .state('payment_pending', {
        isInitial: true,
        timeout: Duration.fromMinutes(30),
      })
      .state('payment_received')
      .state('payment_expired', { isFinal: true })
      .transition('pay', 'payment_pending', 'payment_received')
      .transition('expire', 'payment_pending', 'payment_expired')
      .build();
    ```

---

## Saga Pattern

A saga orchestrates a multi-step transaction across services. Each step has a forward action and a compensating (undo) action. If any step fails, all previously completed steps are compensated in reverse order.

### Defining a Saga

=== "Python"

    ```python
    from aether_sdk.workflow.saga import Saga, SagaExecutor
    from aether_sdk.workflow.types import RetryConfig, Duration, RetryPolicy

    async def reserve_inventory(ctx):
        order_id = ctx.input["order_id"]
        items = ctx.input["items"]
        print(f"Reserving inventory for {order_id}: {items}")
        ctx.set_state("reservation_id", f"RES-{order_id}")
        return {"reserved": True}

    async def release_inventory(ctx):
        reservation_id = ctx.get_state("reservation_id")
        print(f"Releasing reservation {reservation_id}")

    async def process_payment(ctx):
        order_id = ctx.input["order_id"]
        amount = ctx.input["amount"]
        print(f"Charging ${amount} for {order_id}")
        ctx.set_state("charge_id", f"CHG-{order_id}")
        return {"charged": True}

    async def refund_payment(ctx):
        charge_id = ctx.get_state("charge_id")
        print(f"Refunding charge {charge_id}")

    order_saga = (
        Saga("order-processing")
        .step("reserve-inventory")
        .action(reserve_inventory)
        .compensate(release_inventory)
        .step("process-payment")
        .action(process_payment)
        .compensate(refund_payment)
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    import { Saga, SagaExecutor } from '@aether/sdk/workflow';
    import { RetryConfig, Duration, RetryPolicy } from '@aether/sdk/workflow';

    async function reserveInventory(ctx: SagaContext): Promise<any> {
      const { order_id, items } = ctx.input;
      console.log(`Reserving inventory for ${order_id}: ${items}`);
      ctx.setState('reservation_id', `RES-${order_id}`);
      return { reserved: true };
    }

    async function releaseInventory(ctx: SagaContext): Promise<void> {
      const reservationId = ctx.getState('reservation_id');
      console.log(`Releasing reservation ${reservationId}`);
    }

    async function processPayment(ctx: SagaContext): Promise<any> {
      const { order_id, amount } = ctx.input;
      console.log(`Charging $${amount} for ${order_id}`);
      ctx.setState('charge_id', `CHG-${order_id}`);
      return { charged: true };
    }

    async function refundPayment(ctx: SagaContext): Promise<void> {
      const chargeId = ctx.getState('charge_id');
      console.log(`Refunding charge ${chargeId}`);
    }

    const orderSaga = new Saga('order-processing')
      .step('reserve-inventory')
        .action(reserveInventory)
        .compensate(releaseInventory)
      .step('process-payment')
        .action(processPayment)
        .compensate(refundPayment)
      .build();
    ```

### Executing with Error Recovery

=== "Python"

    ```python
    executor = SagaExecutor()

    result = await executor.execute(order_saga, {
        "order_id": "ORD-1",
        "items": [{"sku": "WIDGET-1", "qty": 3}],
        "amount": 29.97,
    })

    if result.status == SagaStatus.COMPLETED:
        print("Order processed successfully")
    elif result.status == SagaStatus.COMPENSATED:
        print(f"Order failed, compensated: {result.error}")
    else:
        print(f"Order failed: {result.error}")
    ```

=== "TypeScript"

    ```typescript
    const executor = new SagaExecutor();

    const result = await executor.execute(orderSaga, {
      order_id: 'ORD-1',
      items: [{ sku: 'WIDGET-1', qty: 3 }],
      amount: 29.97,
    });

    if (result.status === SagaStatus.COMPLETED) {
      console.log('Order processed successfully');
    } else if (result.status === SagaStatus.COMPENSATED) {
      console.log(`Order failed, compensated: ${result.error}`);
    } else {
      console.log(`Order failed: ${result.error}`);
    }
    ```

### Retry and Timeout Configuration

Each step can have its own retry policy and timeout.

=== "Python"

    ```python
    from aether_sdk.workflow.types import RetryConfig, Duration, RetryPolicy

    order_saga = (
        Saga("order-processing")
        .step("reserve-inventory")
        .action(reserve_inventory)
        .compensate(release_inventory)
        .timeout(Duration.from_seconds(10))
        .retry(RetryConfig(
            max_attempts=3,
            policy=RetryPolicy.EXPONENTIAL,
            initial_delay=Duration.from_seconds(1),
            max_delay=Duration.from_seconds(10),
        ))
        .step("process-payment")
        .action(process_payment)
        .compensate(refund_payment)
        .timeout(Duration.from_seconds(30))
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    const orderSaga = new Saga('order-processing')
      .step('reserve-inventory')
        .action(reserveInventory)
        .compensate(releaseInventory)
        .timeout(Duration.fromSeconds(10))
        .retry(new RetryConfig({
          maxAttempts: 3,
          policy: RetryPolicy.EXPONENTIAL,
          initialDelay: Duration.fromSeconds(1),
          maxDelay: Duration.fromSeconds(10),
        }))
      .step('process-payment')
        .action(processPayment)
        .compensate(refundPayment)
        .timeout(Duration.fromSeconds(30))
      .build();
    ```

### Skip Conditions

Conditionally skip a step based on the saga context.

=== "Python"

    ```python
    def has_digital_items(ctx):
        return any(item.get("digital") for item in ctx.input.get("items", []))

    order_saga = (
        Saga("order-processing")
        .step("reserve-inventory")
        .action(reserve_inventory)
        .compensate(release_inventory)
        .skip_if(lambda ctx: all(item.get("digital") for item in ctx.input.get("items", [])))
        .step("process-payment")
        .action(process_payment)
        .compensate(refund_payment)
        .build()
    )
    ```

=== "TypeScript"

    ```typescript
    const orderSaga = new Saga('order-processing')
      .step('reserve-inventory')
        .action(reserveInventory)
        .compensate(releaseInventory)
        .skipIf((ctx) => ctx.input.items.every((i: any) => i.digital))
      .step('process-payment')
        .action(processPayment)
        .compensate(refundPayment)
      .build();
    ```

---

## Human Tasks

Some workflow steps require human intervention — approvals, manual reviews, quality checks. Aether's `HumanTask` pauses the workflow until a person completes the task.

### Creating and Managing Tasks

=== "Python"

    ```python
    from aether_sdk.workflow.human_task import (
        HumanTask, HumanTaskManager, TaskForm,
    )
    from aether_sdk.workflow.types import Duration

    manager = HumanTaskManager()

    approval_form = (
        TaskForm()
        .add_field("approved", "boolean", required=True, label="Approve Order")
        .add_field("comments", "text", label="Comments")
    )

    task = (
        HumanTask(task_type="approval", title="Review International Order")
        .with_assignee("logistics@example.com")
        .with_candidates(users=["alice@example.com", "bob@example.com"])
        .with_priority(2)
        .with_due_date(datetime(2026, 4, 1))
        .with_timeout(Duration.from_hours(48), action="escalate")
        .with_form(approval_form)
    )

    created = await manager.create_task(task, "wf-1", "shipping-review")
    ```

=== "TypeScript"

    ```typescript
    import { HumanTask, HumanTaskManager, TaskForm } from '@aether/sdk/workflow';
    import { Duration } from '@aether/sdk/workflow';

    const manager = new HumanTaskManager();

    const approvalForm = new TaskForm()
      .addField('approved', 'boolean', { required: true, label: 'Approve Order' })
      .addField('comments', 'text', { label: 'Comments' });

    const task = new HumanTask('approval', 'Review International Order')
      .withAssignee('logistics@example.com')
      .withCandidates({ users: ['alice@example.com', 'bob@example.com'] })
      .withPriority(2)
      .withDueDate(new Date('2026-04-01'))
      .withTimeout(Duration.fromHours(48), 'escalate')
      .withForm(approvalForm);

    const created = await manager.createTask(task, 'wf-1', 'shipping-review');
    ```

### Claiming and Completing

=== "Python"

    ```python
    await manager.claim_task(created.task_id, "alice@example.com")

    result = await manager.complete_task(
        created.task_id,
        {"approved": True, "comments": "Looks good, ship it."},
        user="alice@example.com",
    )
    print(f"Task status: {result.status}")
    ```

=== "TypeScript"

    ```typescript
    await manager.claimTask(created.taskId, 'alice@example.com');

    const result = await manager.completeTask(
      created.taskId,
      { approved: true, comments: 'Looks good, ship it.' },
      'alice@example.com',
    );
    console.log(`Task status: ${result.status}`);
    ```

### Waiting for Completion

The workflow can pause and wait for a human task to complete.

=== "Python"

    ```python
    result_data = await manager.wait_for_completion(created.task_id, timeout=3600)
    if result_data.get("approved"):
        print("Order approved, proceeding with shipment")
    else:
        print(f"Order rejected: {result_data.get('comments')}")
    ```

=== "TypeScript"

    ```typescript
    const resultData = await manager.waitForCompletion(created.taskId, 3600);
    if (resultData.approved) {
      console.log('Order approved, proceeding with shipment');
    } else {
      console.log(`Order rejected: ${resultData.comments}`);
    }
    ```

### Timeout and Escalation

If a human task times out, it can automatically escalate or fail.

=== "Python"

    ```python
    await manager.escalate_task(task.task_id, escalate_to="vp-logistics@example.com")

    await manager.reject_task(task.task_id, reason="Missing documentation", user="alice@example.com")
    ```

=== "TypeScript"

    ```typescript
    await manager.escalateTask(task.taskId, 'vp-logistics@example.com');

    await manager.rejectTask(task.taskId, 'Missing documentation', 'alice@example.com');
    ```

---

## Complete Example: Multi-Service Order Fulfillment

This example combines state machines, sagas, and human tasks into a complete order fulfillment workflow.

### Architecture

```
Order Created
     │
     ▼
 State Machine: Created → Validated → Processing → Shipped → Delivered
                     │            │
                     │            ▼
                     │      Saga (compensating transaction):
                     │        1. Reserve Inventory → release on failure
                     │        2. Charge Payment → refund on failure
                     │        3. Check Shipping → cancel label on failure
                     │            │
                     │            ▼ (if international)
                     │      Human Task: Manual Review
                     │            │
                     │            ▼
                     │      Ship Order
                     ▼
                Audit Trail (event sourcing)
```

=== "Python"

    ```python
    import asyncio
    import time
    from datetime import datetime
    from aether_sdk import Actor, Message
    from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor
    from aether_sdk.workflow.saga import Saga, SagaExecutor
    from aether_sdk.workflow.human_task import HumanTask, HumanTaskManager, TaskForm
    from aether_sdk.workflow.types import (
        SagaStatus, Duration, RetryConfig, RetryPolicy,
    )
    from aether_sdk.event.event_sourcing import (
        Aggregate, InMemoryEventStore, EventSourcedActor,
    )


    class Order(Aggregate):
        def __init__(self):
            super().__init__()
            self.order_id = ""
            self.status = "created"
            self.items = []
        def apply_order_created(self, event):
            self.order_id = event["order_id"]
            self.items = event["items"]
        def apply_order_status_changed(self, event):
            self.status = event["status"]


    class OrderFulfillmentActor(Actor, EventSourcedActor):
        def __init__(self):
            Actor.__init__(self, "order-fulfillment")
            EventSourcedActor.__init__(self, InMemoryEventStore())
            self.require(
                "STATE_READ", "STATE_WRITE", "EVENT_EMIT",
                "ACTOR_MESSAGING", "LOG",
            )
            self.wf_executor = WorkflowExecutor()
            self.saga_executor = SagaExecutor()
            self.task_manager = HumanTaskManager()
            self._build_workflow()
            self._build_saga()

        def _build_workflow(self):
            self.workflow = (
                Workflow("order-fulfillment")
                .state("created", is_initial=True)
                .state("validated")
                .state("processing")
                .state("shipped")
                .state("delivered", is_final=True)
                .transition("validate", "created", "validated")
                .transition("process", "validated", "processing")
                .transition("ship", "processing", "shipped")
                .transition("deliver", "shipped", "delivered")
                .build()
            )

        def _build_saga(self):
            self.saga = (
                Saga("fulfillment-saga")
                .step("reserve-inventory")
                .action(self._reserve_inventory)
                .compensate(self._release_inventory)
                .timeout(Duration.from_seconds(10))
                .retry(RetryConfig(
                    max_attempts=3,
                    policy=RetryPolicy.EXPONENTIAL,
                    initial_delay=Duration.from_milliseconds(500),
                    max_delay=Duration.from_seconds(5),
                ))
                .step("charge-payment")
                .action(self._charge_payment)
                .compensate(self._refund_payment)
                .timeout(Duration.from_seconds(30))
                .step("create-shipment")
                .action(self._create_shipment)
                .compensate(self._cancel_shipment)
                .timeout(Duration.from_seconds(15))
                .build()
            )

        async def _reserve_inventory(self, ctx):
            order_id = ctx.input["order_id"]
            items = ctx.input["items"]
            print(f"[Inventory] Reserving for {order_id}")
            ctx.set_state("reservation_id", f"RES-{order_id}")
            return {"reserved": True}

        async def _release_inventory(self, ctx):
            print(f"[Inventory] Releasing {ctx.get_state('reservation_id')}")

        async def _charge_payment(self, ctx):
            order_id = ctx.input["order_id"]
            amount = ctx.input["amount"]
            print(f"[Payment] Charging ${amount} for {order_id}")
            ctx.set_state("charge_id", f"CHG-{order_id}")
            return {"charged": True}

        async def _refund_payment(self, ctx):
            print(f"[Payment] Refunding {ctx.get_state('charge_id')}")

        async def _create_shipment(self, ctx):
            order_id = ctx.input["order_id"]
            print(f"[Shipping] Creating label for {order_id}")
            ctx.set_state("tracking_id", f"TRK-{order_id}")

        async def _cancel_shipment(self, ctx):
            print(f"[Shipping] Cancelling {ctx.get_state('tracking_id')}")

        async def _record_event(self, order_id, event_type, payload=None):
            aggregate = Order()
            aggregate.id = order_id
            aggregate.emit_event(event_type, payload or {})
            await self.save_aggregate(aggregate)

        async def handle_message(self, sender, msg):
            match msg.type:
                case "create_order":
                    order_id = msg.payload["order_id"]
                    items = msg.payload["items"]
                    amount = msg.payload["amount"]
                    international = msg.payload.get("international", False)

                    await self._record_event(order_id, "order_created", {
                        "order_id": order_id, "items": items,
                    })

                    wf_result = await self.wf_executor.start(
                        self.workflow,
                        {"order_id": order_id, "items": items, "amount": amount},
                    )
                    await self.wf_executor.transition(wf_result.workflow_id, "validate")
                    await self.wf_executor.transition(wf_result.workflow_id, "process")

                    saga_result = await self.saga_executor.execute(self.saga, {
                        "order_id": order_id, "items": items, "amount": amount,
                    })

                    if saga_result.status == SagaStatus.COMPLETED:
                        await self._record_event(order_id, "order_status_changed", {
                            "status": "processing",
                        })

                        if international:
                            form = (
                                TaskForm()
                                .add_field("approved", "boolean", required=True)
                                .add_field("notes", "text")
                            )
                            task = (
                                HumanTask("customs-review", f"Customs review for {order_id}")
                                .with_candidates(users=["customs@example.com"])
                                .with_timeout(Duration.from_hours(24), action="escalate")
                                .with_form(form)
                            )
                            created = await self.task_manager.create_task(
                                task, wf_result.workflow_id, "customs-review",
                            )
                            review = await self.task_manager.wait_for_completion(
                                created.task_id, timeout=86400,
                            )
                            if not review.get("approved"):
                                await self._record_event(order_id, "order_status_changed", {
                                    "status": "rejected",
                                })
                                return Message.response({"status": "rejected", "reason": review.get("notes")})

                        await self.wf_executor.transition(wf_result.workflow_id, "ship")
                        await self.wf_executor.transition(wf_result.workflow_id, "deliver")

                        await self._record_event(order_id, "order_status_changed", {
                            "status": "delivered",
                        })
                        return Message.response({"status": "delivered", "order_id": order_id})

                    else:
                        await self._record_event(order_id, "order_status_changed", {
                            "status": "failed",
                        })
                        return Message.response({
                            "status": "failed",
                            "error": saga_result.error,
                        })

                case _:
                    return Message.error("unknown message type")


    async def main():
        actor = OrderFulfillmentActor()
        await actor.start()

        response = await actor.call(
            "order-fulfillment",
            Message("create_order", payload={
                "order_id": "ORD-001",
                "items": [{"sku": "WIDGET-1", "qty": 3, "price": 9.99}],
                "amount": 29.97,
                "international": False,
            }),
        )
        print(f"Domestic order: {response.payload}")

        response2 = await actor.call(
            "order-fulfillment",
            Message("create_order", payload={
                "order_id": "ORD-002",
                "items": [{"sku": "WIDGET-2", "qty": 1, "price": 49.99}],
                "amount": 49.99,
                "international": True,
            }),
        )
        print(f"International order: {response2.payload}")

    if __name__ == "__main__":
        asyncio.run(main())
    ```

=== "TypeScript"

    ```typescript
    import { Actor, Message, MessageType } from '@aether/sdk';
    import { Workflow, WorkflowExecutor } from '@aether/sdk/workflow';
    import { Saga, SagaExecutor } from '@aether/sdk/workflow';
    import {
      HumanTask, HumanTaskManager, TaskForm,
    } from '@aether/sdk/workflow';
    import {
      SagaStatus, Duration, RetryConfig, RetryPolicy,
    } from '@aether/sdk/workflow';
    import { Aggregate, InMemoryEventStore } from '@aether/sdk/event-sourcing';

    class Order extends Aggregate {
      orderId = '';
      status = 'created';
      items: any[] = [];

      applyOrderCreated(event: any): void {
        this.orderId = event.order_id;
        this.items = event.items;
      }

      applyOrderStatusChanged(event: any): void {
        this.status = event.status;
      }
    }

    class OrderFulfillmentActor extends Actor {
      private eventStore = new InMemoryEventStore();
      private wfExecutor = new WorkflowExecutor();
      private sagaExecutor = new SagaExecutor();
      private taskManager = new HumanTaskManager();
      private workflow: Workflow;
      private saga: Saga;

      constructor() {
        super({ name: 'order-fulfillment' });
        this.workflow = new Workflow('order-fulfillment')
          .state('created', { isInitial: true })
          .state('validated')
          .state('processing')
          .state('shipped')
          .state('delivered', { isFinal: true })
          .transition('validate', 'created', 'validated')
          .transition('process', 'validated', 'processing')
          .transition('ship', 'processing', 'shipped')
          .transition('deliver', 'shipped', 'delivered')
          .build();

        this.saga = new Saga('fulfillment-saga')
          .step('reserve-inventory')
            .action(this.reserveInventory.bind(this))
            .compensate(this.releaseInventory.bind(this))
            .timeout(Duration.fromSeconds(10))
            .retry(new RetryConfig({
              maxAttempts: 3,
              policy: RetryPolicy.EXPONENTIAL,
              initialDelay: Duration.fromMilliseconds(500),
              maxDelay: Duration.fromSeconds(5),
            }))
          .step('charge-payment')
            .action(this.chargePayment.bind(this))
            .compensate(this.refundPayment.bind(this))
            .timeout(Duration.fromSeconds(30))
          .step('create-shipment')
            .action(this.createShipment.bind(this))
            .compensate(this.cancelShipment.bind(this))
            .timeout(Duration.fromSeconds(15))
          .build();
      }

      private async reserveInventory(ctx: any): Promise<any> {
        const { order_id, items } = ctx.input;
        console.log(`[Inventory] Reserving for ${order_id}`);
        ctx.setState('reservation_id', `RES-${order_id}`);
        return { reserved: true };
      }

      private async releaseInventory(ctx: any): Promise<void> {
        console.log(`[Inventory] Releasing ${ctx.getState('reservation_id')}`);
      }

      private async chargePayment(ctx: any): Promise<any> {
        const { order_id, amount } = ctx.input;
        console.log(`[Payment] Charging $${amount} for ${order_id}`);
        ctx.setState('charge_id', `CHG-${order_id}`);
        return { charged: true };
      }

      private async refundPayment(ctx: any): Promise<void> {
        console.log(`[Payment] Refunding ${ctx.getState('charge_id')}`);
      }

      private async createShipment(ctx: any): Promise<any> {
        const { order_id } = ctx.input;
        console.log(`[Shipping] Creating label for ${order_id}`);
        ctx.setState('tracking_id', `TRK-${order_id}`);
      }

      private async cancelShipment(ctx: any): Promise<void> {
        console.log(`[Shipping] Cancelling ${ctx.getState('tracking_id')}`);
      }

      private async recordEvent(orderId: string, eventType: string, payload?: any): Promise<void> {
        const aggregate = new Order();
        aggregate.id = orderId;
        aggregate.emitEvent(eventType, payload || {});
        const events = aggregate.uncommittedEvents;
        if (events.length > 0) {
          await this.eventStore.append(orderId, events.map(e => ({
            type: e.eventType, ...e.payload,
          })));
          aggregate.markEventsCommitted();
        }
      }

      async handle(sender: string, msg: Message): Promise<Message | void> {
        if (msg.type === MessageType.CUSTOM && msg.payload.action === 'create_order') {
          const { order_id, items, amount, international } = msg.payload;

          await this.recordEvent(order_id, 'order_created', { order_id, items });

          const wfResult = await this.wfExecutor.start(this.workflow, { order_id, items, amount });
          await this.wfExecutor.transition(wfResult.workflow_id, 'validate');
          await this.wfExecutor.transition(wfResult.workflow_id, 'process');

          const sagaResult = await this.sagaExecutor.execute(this.saga, { order_id, items, amount });

          if (sagaResult.status === SagaStatus.COMPLETED) {
            await this.recordEvent(order_id, 'order_status_changed', { status: 'processing' });

            if (international) {
              const form = new TaskForm()
                .addField('approved', 'boolean', { required: true })
                .addField('notes', 'text');
              const task = new HumanTask('customs-review', `Customs review for ${order_id}`)
                .withCandidates({ users: ['customs@example.com'] })
                .withTimeout(Duration.fromHours(24), 'escalate')
                .withForm(form);
              const created = await this.taskManager.createTask(task, wfResult.workflow_id, 'customs-review');
              const review = await this.taskManager.waitForCompletion(created.taskId, 86400);
              if (!review.approved) {
                await this.recordEvent(order_id, 'order_status_changed', { status: 'rejected' });
                return Message.custom({ status: 'rejected', reason: review.notes });
              }
            }

            await this.wfExecutor.transition(wfResult.workflow_id, 'ship');
            await this.wfExecutor.transition(wfResult.workflow_id, 'deliver');
            await this.recordEvent(order_id, 'order_status_changed', { status: 'delivered' });
            return Message.custom({ status: 'delivered', order_id });
          } else {
            await this.recordEvent(order_id, 'order_status_changed', { status: 'failed' });
            return Message.custom({ status: 'failed', error: sagaResult.error });
          }
        }
      }
    }

    async function main() {
      const actor = new OrderFulfillmentActor();
      await actor.start();

      const res1 = await actor.call('order-fulfillment', Message.custom({
        action: 'create_order',
        order_id: 'ORD-001',
        items: [{ sku: 'WIDGET-1', qty: 3, price: 9.99 }],
        amount: 29.97,
        international: false,
      }));
      console.log('Domestic order:', res1);

      const res2 = await actor.call('order-fulfillment', Message.custom({
        action: 'create_order',
        order_id: 'ORD-002',
        items: [{ sku: 'WIDGET-2', qty: 1, price: 49.99 }],
        amount: 49.99,
        international: true,
      }));
      console.log('International order:', res2);
    }

    main().catch(console.error);
    ```

### Walkthrough

1. **State Machine** tracks the order through `Created → Validated → Processing → Shipped → Delivered`.
2. **Saga** runs inside the `Processing` state. It reserves inventory, charges payment, and creates a shipment label. If any step fails, previous steps are compensated in reverse (refund payment, release inventory).
3. **Human Task** pauses international orders for customs review. The task has a 24-hour timeout with auto-escalation. If rejected, the order fails.
4. **Event Sourcing** records every status change as an immutable event, providing a full audit trail for each order.

---

## Best Practices

### Timeout Handling

Always set timeouts on saga steps and human tasks. A step that hangs forever blocks the entire workflow.

=== "Python"

    ```python
    .timeout(Duration.from_seconds(30))
    ```

=== "TypeScript"

    ```typescript
    .timeout(Duration.fromSeconds(30));
    ```

Use shorter timeouts for steps that call external services and longer ones for human tasks.

### Idempotent Steps

Saga actions and compensations may be retried. Design every step to be safe to call multiple times with the same input.

=== "Python"

    ```python
    async def charge_payment(ctx):
        charge_id = ctx.get_state("charge_id")
        if charge_id:
            return {"already_charged": True, "charge_id": charge_id}
        new_id = await payment_gateway.charge(ctx.input["amount"])
        ctx.set_state("charge_id", new_id)
        return {"charged": True, "charge_id": new_id}
    ```

=== "TypeScript"

    ```typescript
    async function chargePayment(ctx: SagaContext): Promise<any> {
      const chargeId = ctx.getState('charge_id');
      if (chargeId) return { alreadyCharged: true, chargeId };
      const newId = await paymentGateway.charge(ctx.input.amount);
      ctx.setState('charge_id', newId);
      return { charged: true, chargeId: newId };
    }
    ```

### Monitoring

Track saga and workflow status in production. Log every transition and saga result.

=== "Python"

    ```python
    result = await executor.execute(saga, input_data)
    print(f"Saga {result.saga_id}: {result.status.value} "
          f"({result.duration_ms}ms, steps: {result.completed_steps})")
    ```

=== "TypeScript"

    ```typescript
    const result = await executor.execute(saga, inputData);
    console.log(`Saga ${result.sagaId}: ${result.status} ` +
      `(${result.durationMs}ms, steps: ${result.completedSteps.join(', ')})`);
    ```

Key metrics to monitor:

| Metric | Alert threshold | Why |
|---|---|---|
| Saga completion rate | < 95% | Compensation storms |
| Step duration P99 | > 2x timeout | Service degradation |
| Human task backlog | > 50 pending | Bottleneck |
| Compensation failures | > 0 | Data inconsistency risk |

---

## What You Learned

- **State Machines** — Defining workflows with states, transitions, guards, and side effects
- **Saga Pattern** — Multi-service transactions with compensating actions and retry
- **Human Tasks** — Approval workflows with assignment, timeouts, and escalation
- **Complete example** — Order fulfillment combining all three patterns with event sourcing

---

## Next Steps

| Topic | Resource |
|---|---|
| Event-Driven Systems | [Event-Driven Tutorial](./03_event_driven.md) |
| Performance | [Performance Tutorial](./04_performance.md) |
| Examples | [Examples Gallery](../../docs-site/docs/examples/overview.md) |

---

*Time to complete: ~1 hour*
