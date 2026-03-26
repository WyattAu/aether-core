# Migrating from Temporal to Aether Workflows

**Last Updated:** 2026-03-26  
**Estimated Migration Time:** 3-5 days per workflow  

---

## Why Migrate?

| Concern | Temporal | Aether Workflows |
|---------|----------|-----------------|
| Architecture | External Temporal server + workers | Embedded in actor runtime |
| Language support | Go, Java, TS, Python (limited) | Python, JS, Rust, Go (native) |
| Workflow definition | Deterministic code constraints | Declarative state machine / saga |
| Activity invocation | Activity stubs | Actor `call()` RPC |
| Saga support | Manual compensation | Built-in saga with auto-compensation |
| Human tasks | Signals + queries | Native `HumanTask` with forms |
| Event sourcing | Not built-in | Built-in `EventStore` + `Aggregate` |
| Resilience | Server-managed retries | `CircuitBreaker` + `RetryPolicy` |
| Deployment | Temporal server + DB + workers | Single Aether runtime |

Aether eliminates the Temporal server entirely. Workflows are defined declaratively as state machines or sagas and executed directly within the actor framework. You gain native event sourcing, streaming, and resilience patterns without a separate orchestration layer.

---

## Concept Mapping

| Temporal Concept | Aether Equivalent |
|-----------------|------------------|
| Workflow | `Workflow` (state machine) or `Saga` |
| Activity | Actor method / RPC call |
| Workflow Client | `WorkflowExecutor` |
| Activity Stub | `actor.call()` |
| Signal | `WorkflowExecutor.transition()` |
| Query | `WorkflowExecutor.get_status()` |
| Timer | `State.timeout` / `SagaStep.timeout` |
| Saga | `Saga` with compensation handlers |
| Child Workflow | `Saga` step with nested saga |
| Task Queue | Actor registration name |
| Worker | Actor instance |
| Workflow Execution | `WorkflowResult.workflow_id` |

---

## Step-by-Step Migration

### Step 1: Workflow Definition

Temporal workflows are deterministic code. Aether workflows are declarative state machines.

```python
# Temporal
from temporalio import workflow

@workflow.defn
class OrderWorkflow:
    @workflow.run
    async def run(self, order: Order) -> str:
        # Reserve inventory
        await workflow.execute_activity(
            reserve_inventory,
            order.items,
            start_to_close_timeout=timedelta(seconds=30),
            retry_policy=RetryPolicy(
                maximum_attempts=3,
                initial_interval=timedelta(seconds=1),
                backoff_coefficient=2.0,
            ),
        )

        # Process payment
        payment_result = await workflow.execute_activity(
            process_payment,
            order.total,
            start_to_close_timeout=timedelta(seconds=60),
        )

        if not payment_result.success:
            await workflow.execute_activity(
                release_inventory,
                order.items,
            )
            raise ApplicationError("Payment failed")

        # Ship order
        await workflow.execute_activity(
            ship_order,
            order.id,
            start_to_close_timeout=timedelta(hours=24),
        )

        return order.id
```

```python
# Aether
from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor
from aether_sdk.workflow.saga import Saga, SagaExecutor

# Option A: State Machine
wf = Workflow("order-workflow")
wf.state("created", is_initial=True)
wf.state("reserved")
wf.state("paid")
wf.state("shipped", is_final=True)
wf.transition("reserve", "created", "reserved")
wf.transition("pay", "reserved", "paid")
wf.transition("ship", "paid", "shipped")
wf.on_enter("reserved", reserve_inventory)
wf.on_enter("paid", process_payment)
wf.on_enter("shipped", ship_order)
wf.build()

executor = WorkflowExecutor()
result = await executor.start(wf, {"order_id": "123"})

# Option B: Saga (for transactional semantics)
order_saga = Saga("order-processing") \
    .step("reserve").action(reserve_inventory).compensate(release_inventory) \
    .step("pay").action(process_payment).compensate(refund_payment) \
    .step("ship").action(ship_order).compensate(cancel_shipment) \
    .build()

saga_executor = SagaExecutor()
result = await saga_executor.execute(order_saga, {"order_id": "123"})
```

### Step 2: Activity Migration

Temporal activities become actor methods or standalone functions called via RPC.

```python
# Temporal
@activity.defn
async def reserve_inventory(items: list[Item]) -> bool:
    for item in items:
        inventory.reserve(item.sku, item.quantity)
    return True

@activity.defn
async def process_payment(total: float) -> PaymentResult:
    return await payment_gateway.charge(total)
```

```python
# Aether — actor methods
class InventoryActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "inventory"

    async def handle_message(self, sender, message):
        if message.payload.get("action") == "reserve":
            for item in message.payload["items"]:
                self._reserve(item["sku"], item["quantity"])
            return Message(type=MessageType.CUSTOM, payload={"success": True})

class PaymentActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "payment"

    async def handle_message(self, sender, message):
        if message.payload.get("action") == "charge":
            result = await self._charge(message.payload["total"])
            return Message(type=MessageType.CUSTOM, payload=result)

# Saga step using actor RPC
async def reserve_inventory(context):
    result = await context.call("inventory", {
        "action": "reserve", "items": context.input["items"]
    })

async def release_inventory(context):
    await context.call("inventory", {
        "action": "release", "items": context.input["items"]
    })
```

### Step 3: Saga Pattern Comparison

Temporal requires manual compensation. Aether sagas compensate automatically.

```python
# Temporal — manual compensation
@workflow.defn
class OrderWorkflow:
    @workflow.run
    async def run(self, order: Order):
        try:
            await workflow.execute_activity(reserve_inventory, order.items)
        except ActivityError:
            raise ApplicationError("Reservation failed")

        try:
            await workflow.execute_activity(process_payment, order.total)
        except ActivityError:
            await workflow.execute_activity(release_inventory, order.items)
            raise ApplicationError("Payment failed, inventory released")

        try:
            await workflow.execute_activity(ship_order, order.id)
        except ActivityError:
            await workflow.execute_activity(refund_payment, order.id)
            await workflow.execute_activity(release_inventory, order.items)
            raise ApplicationError("Shipping failed, payment refunded")
```

```python
# Aether — automatic compensation
saga = Saga("order") \
    .step("reserve").action(reserve_inventory).compensate(release_inventory) \
    .step("pay").action(process_payment).compensate(refund_payment) \
    .step("ship").action(ship_order).compensate(cancel_shipment) \
    .build()

result = await executor.execute(saga, {"order_id": "123"})
# If "pay" fails, "reserve" is automatically compensated
# If "ship" fails, "pay" then "reserve" are compensated in reverse order
```

### Step 4: Error Handling and Compensation

```python
# Temporal
@workflow.defn
class OrderWorkflow:
    @workflow.run
    async def run(self, order: Order):
        try:
            result = await workflow.execute_activity(
                process_payment,
                order.total,
                retry_policy=RetryPolicy(
                    maximum_attempts=3,
                    non_retryable_error_types=["InsufficientFundsError"],
                ),
            )
        except InsufficientFundsError:
            await workflow.execute_activity(cancel_order, order.id)
            return "cancelled"

# Aether
from aether_sdk.workflow.saga import SagaExecutor, RetryConfig

executor = SagaExecutor(default_retry=RetryConfig(max_attempts=3))
result = await executor.execute(saga, {"order_id": "123"})

# Check result status
if result.status == SagaStatus.COMPENSATED:
    logger.info(f"Rolled back after failure: {result.error}")
elif result.status == SagaStatus.FAILED:
    logger.error(f"Failed without compensation: {result.error}")
```

### Step 5: Timeout and Retry Comparison

| Temporal | Aether |
|----------|--------|
| `start_to_close_timeout` | `SagaStep.timeout` / `Duration` |
| `schedule_to_close_timeout` | Not needed (no scheduling queue) |
| `heartbeat_timeout` | Not applicable (actors are long-lived) |
| `RetryPolicy.maximum_attempts` | `RetryConfig.max_attempts` |
| `RetryPolicy.initial_interval` | `RetryConfig.base_delay_ms` |
| `RetryPolicy.backoff_coefficient` | `RetryConfig.multiplier` |
| `non_retryable_error_types` | `RetryConfig.is_retryable` callback |
| `workflow.sleep()` | `State.timeout` + `timeout_transition` |

```python
# Temporal — activity with timeout + retry
await workflow.execute_activity(
    process_payment,
    order.total,
    start_to_close_timeout=timedelta(seconds=30),
    retry_policy=RetryPolicy(
        maximum_attempts=5,
        initial_interval=timedelta(milliseconds=100),
        backoff_coefficient=2.0,
        non_retryable_error_types=["InsufficientFundsError"],
    ),
)
```

```python
# Aether — saga step with timeout + retry
from aether_sdk.workflow.types import Duration, RetryConfig, RetryPolicy

saga = Saga("order") \
    .step("pay") \
    .action(process_payment) \
    .compensate(refund_payment) \
    .timeout(Duration.from_seconds(30)) \
    .retry(RetryConfig(
        max_attempts=5,
        base_delay=Duration.from_millis(100),
        multiplier=2.0,
        is_retryable=lambda err, attempt: not isinstance(err, InsufficientFundsError),
    )) \
    .build()
```

### Step 6: State Machine with Timers

```python
# Temporal — timer-based escalation
@workflow.defn
class ApprovalWorkflow:
    @workflow.run
    async def run(self, request: ApprovalRequest):
        await workflow.execute_activity(send_approval_request, request)
        try:
            result = await workflow.wait_condition(
                lambda: self.approved is not None,
                timeout=timedelta(hours=24),
            )
        except asyncio.TimeoutError:
            await workflow.execute_activity(escalate, request)
```

```python
# Aether — state timeout + transition
from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor

wf = Workflow("approval-flow")
wf.state("pending", is_initial=True, timeout=Duration.from_hours(24),
         timeout_transition="escalate")
wf.state("approved", is_final=True)
wf.state("rejected", is_final=True)
wf.state("escalated", is_final=True)
wf.transition("approve", "pending", "approved")
wf.transition("reject", "pending", "rejected")
wf.transition("escalate", "pending", "escalated")
wf.on_enter("pending", send_approval_request)
wf.on_enter("escalated", escalate_request)
wf.build()

executor = WorkflowExecutor()
result = await executor.start(wf, {"request_id": "123"})
# State auto-transitions to "escalated" after 24 hours
```

---

## Human Tasks

Temporal uses signals and queries for human interaction. Aether has native human task support.

```python
# Temporal — signal-based approval
@workflow.defn
class ApprovalWorkflow:
    def __init__(self):
        self.approved = None

    @workflow.run
    async def run(self, request):
        await workflow.execute_activity(notify_reviewer, request)
        await workflow.wait_condition(lambda: self.approved is not None)
        if self.approved:
            return "approved"
        return "rejected"

    @workflow.signal
    async def approve(self):
        self.approved = True

    @workflow.signal
    async def reject(self):
        self.approved = False
```

```python
# Aether — native human tasks
from aether_sdk.workflow.human_task import HumanTask, HumanTaskManager, TaskForm

task = HumanTask(
    task_type="approval",
    title="Approve Purchase Order",
    description=f"PO #{request['id']} for ${request['total']}",
) \
    .with_candidates(users=["reviewer@company.com"]) \
    .with_priority(3) \
    .with_timeout(Duration.from_hours(24), action="escalate") \
    .with_form(TaskForm()
        .add_field("approved", "boolean", required=True)
        .add_field("comments", "text"))

manager = HumanTaskManager()
await manager.create_task(task, workflow_id, "review-step")

# Wait for completion (blocks the workflow)
result = await manager.wait_for_completion(task.task_id, timeout=86400)
if result.get("approved"):
    await executor.transition(workflow_id, "approve")
else:
    await executor.transition(workflow_id, "reject")
```

---

## Gotchas and Common Pitfalls

1. **Workflows are declarative, not imperative**. You cannot use loops, conditionals, or non-deterministic code inside workflow definitions. Express logic via guards, skip conditions, and compensation handlers.

2. **No `workflow.wait_condition`**. Instead, use `WorkflowExecutor.transition()` triggered by external events, or `HumanTaskManager.wait_for_completion()` for human tasks.

3. **Saga compensation is automatic**. You do not need to write try/except blocks. Just define compensation handlers and the executor handles rollback.

4. **State timeouts are declarative**. Set `timeout` and `timeout_transition` on the `State`, not inside transition handlers.

5. **No activity heartbeats**. Aether actors are long-lived processes. Heartbeating is not needed because actors maintain their own event loop.

6. **No task queue concept**. Actors are identified by name. Deploy multiple instances with the same name for parallelism.

7. **Workflow state is in-memory**. For persistence across restarts, use the `state` handle or integrate with the event sourcing module.

---

## Migration Checklist

- [ ] Inventory all Temporal workflows and their activity dependencies
- [ ] For each workflow, decide: state machine or saga
- [ ] Map workflow states to `Workflow.state()` calls
- [ ] Map transitions to `Workflow.transition()` calls
- [ ] Map activities to actor methods or standalone functions
- [ ] Define compensation handlers for saga steps
- [ ] Replace `start_to_close_timeout` with `SagaStep.timeout`
- [ ] Replace `RetryPolicy` with `RetryConfig`
- [ ] Replace `workflow.wait_condition` with state transitions or human tasks
- [ ] Replace timer-based escalation with `State.timeout`
- [ ] Replace signals with `WorkflowExecutor.transition()`
- [ ] Replace queries with `WorkflowExecutor.get_status()`
- [ ] Add circuit breakers for activity (actor) calls
- [ ] Decommission Temporal server and database
- [ ] Run workflow integration tests
- [ ] Monitor workflow metrics via `WorkflowResult` status
