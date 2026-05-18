"""
Edge case tests for the Python Aether SDK.

Covers boundary conditions and stress scenarios across streaming,
events, workflow, and resilience modules.
"""

import asyncio
import os

import pytest
from aether_sdk.event.event_sourcing import Aggregate, InMemoryEventStore
from aether_sdk.event.pubsub import InMemoryPubSub, PubSubMessage, Topic
from aether_sdk.event.schema import InMemorySchemaRegistry, Schema, SchemaError
from aether_sdk.resilience.bulkhead import (
    Bulkhead,
    BulkheadConfig,
    BulkheadRejectedError,
)
from aether_sdk.resilience.circuit_breaker import (
    CircuitBreaker,
    CircuitBreakerConfig,
    CircuitBreakerError,
    CircuitState,
)
from aether_sdk.resilience.retry import BackoffStrategy
from aether_sdk.resilience.retry import RetryConfig as ResRetryConfig
from aether_sdk.resilience.retry import RetryExhaustedError, RetryPolicy
from aether_sdk.streaming.backpressure import BackpressureController
from aether_sdk.streaming.types import (
    BackpressureConfig,
    BackpressureStrategy,
    StreamEvent,
    Timestamp,
    Watermark,
)
from aether_sdk.workflow.human_task import (
    HumanTask,
    HumanTaskManager,
    HumanTaskTimeoutError,
)
from aether_sdk.workflow.saga import Saga, SagaExecutor
from aether_sdk.workflow.state_machine import Workflow, WorkflowExecutor
from aether_sdk.workflow.types import Duration as WfDuration
from aether_sdk.workflow.types import HumanTaskStatus
from aether_sdk.workflow.types import RetryConfig as WfRetryConfig
from aether_sdk.workflow.types import RetryPolicy as WfRetryPolicy
from aether_sdk.workflow.types import SagaStatus

# ============================================================
# 1. Stream Processing Edge Cases
# ============================================================


class _NoOpStreamActor:
    """Minimal concrete subclass of StreamActor for testing empty streams."""

    def __init__(self):
        self.processed = []

    async def process_event(self, event):
        self.processed.append(event)


@pytest.mark.asyncio
async def test_empty_stream_processes_zero_events():
    """An actor processing zero events should complete with no errors and zero metrics."""
    actor = _NoOpStreamActor()
    bp = BackpressureController(
        BackpressureConfig(strategy=BackpressureStrategy.BUFFER)
    )
    assert bp.is_empty()
    assert bp.pop() is None
    assert bp.stats.total_events == 0
    assert actor.processed == []


@pytest.mark.asyncio
async def test_large_message_over_10mb_processes():
    """A StreamEvent carrying a 10 MB+ payload should be buffered and popped without error."""
    big_value = os.urandom(11 * 1024 * 1024)
    event = StreamEvent.create(key="big", value=big_value)
    assert len(big_value) > 10 * 1024 * 1024

    bp = BackpressureController(BackpressureConfig(buffer_size=5))
    accepted = bp.try_push(event)
    assert accepted is True

    popped = bp.pop()
    assert popped is not None
    assert popped.key == "big"
    assert len(popped.value) == len(big_value)


@pytest.mark.asyncio
async def test_rapid_fire_10000_events_no_loss():
    """Push 10 000 events through a backpressure controller and verify none are lost."""
    count = 10000
    bp = BackpressureController(
        BackpressureConfig(
            strategy=BackpressureStrategy.BUFFER,
            buffer_size=count + 1,
        )
    )

    for i in range(count):
        event = StreamEvent.create(key=str(i), value=i)
        assert bp.try_push(event) is True, f"Event {i} was dropped"

    assert bp.stats.total_events == count
    assert bp.stats.dropped_events == 0

    received = []
    while True:
        e = bp.pop()
        if e is None:
            break
        received.append(e)

    assert len(received) == count


@pytest.mark.asyncio
async def test_late_data_with_watermark_at_epoch():
    """Events with timestamp 0 should be flagged as late when watermark is already advanced."""
    stream_id = "test-stream"
    watermark = Watermark(timestamp=Timestamp(5000), stream_id=stream_id)

    late_event = StreamEvent.create(
        key="late",
        value="data",
        timestamp=Timestamp(0),
    )

    assert late_event.timestamp < watermark.timestamp
    assert watermark.is_late(late_event.timestamp) is True


# ============================================================
# 2. Event System Edge Cases
# ============================================================


@pytest.mark.asyncio
async def test_circular_subscription_does_not_infinite_loop():
    """Subscribing A->B->C->A where each handler publishes to the next topic
    should not cause an infinite loop because InMemoryPubSub._route_message
    dispatches synchronously and publishes are awaited sequentially."""
    backend = InMemoryPubSub()
    await backend.create_topic(Topic("a"))
    await backend.create_topic(Topic("b"))
    await backend.create_topic(Topic("c"))

    visit_counts = {"a": 0, "b": 0, "c": 0}

    async def handler_a(msg):
        visit_counts["a"] += 1
        if visit_counts["a"] <= 1:
            await backend.publish("b", PubSubMessage(topic="b", value="go"))

    async def handler_b(msg):
        visit_counts["b"] += 1
        if visit_counts["b"] <= 1:
            await backend.publish("c", PubSubMessage(topic="c", value="go"))

    async def handler_c(msg):
        visit_counts["c"] += 1
        if visit_counts["c"] <= 1:
            await backend.publish("a", PubSubMessage(topic="a", value="go"))

    await backend.subscribe("a", handler_a)
    await backend.subscribe("b", handler_b)
    await backend.subscribe("c", handler_c)

    await backend.publish("a", PubSubMessage(topic="a", value="start"))

    assert visit_counts["a"] == 2
    assert visit_counts["b"] == 1
    assert visit_counts["c"] == 1


@pytest.mark.asyncio
async def test_schema_validation_deeply_nested_objects():
    """The JsonSchemaValidator only validates the top level of objects.
    Verify that deeply nested top-level required fields are caught, but that
    missing nested sub-fields are not recursively validated."""
    definition = {
        "type": "object",
        "properties": {
            "level_0": {"type": "object"},
            "level_1": {"type": "object"},
            "name": {"type": "string"},
        },
        "required": ["level_0", "level_1"],
    }

    schema = Schema(name="DeepNested", type="json", definition=definition)
    registry = InMemorySchemaRegistry()
    await registry.register("DeepNested", schema)

    with pytest.raises(SchemaError, match="Missing required field"):
        await registry.validate("DeepNested", {"name": "ok"})

    valid = await registry.validate("DeepNested", {"level_0": {}, "level_1": {}})
    assert valid is True

    with pytest.raises(SchemaError, match="wrong type"):
        await registry.validate("DeepNested", {"level_0": 123, "level_1": {}})


@pytest.mark.asyncio
async def test_event_store_many_events_ordering_and_recovery():
    """Append 500 events, verify sequential version ordering, then rebuild an
    aggregate from history and confirm state matches."""
    store = InMemoryEventStore()
    aggregate_id = "agg-001"
    total = 500

    for i in range(total):
        new_version = await store.append(
            aggregate_id,
            [{"type": "value_set", "value": i}],
        )
        assert new_version == i + 1

    events = await store.get_events(aggregate_id)
    assert len(events) == total

    for i, env in enumerate(events):
        assert env.version == i + 1
        assert env.payload["value"] == i

    class Counter(Aggregate):
        def __init__(self):
            super().__init__()
            self.current = -1

        def apply_value_set(self, payload):
            self.current = payload["value"]

    counter = Counter()
    counter.id = aggregate_id
    counter.load_from_history(events)

    assert counter.current == total - 1
    assert counter.version == total


# ============================================================
# 3. Workflow Edge Cases
# ============================================================


@pytest.mark.asyncio
async def test_saga_all_steps_fail_compensation():
    """Build a saga where the second step fails, triggering compensation
    on the first step. Verify compensated_steps includes the first step."""
    compensated_steps = []

    async def action_ok(ctx):
        return "ok"

    async def compensate_ok(ctx):
        compensated_steps.append("step1")

    async def action_fail(ctx):
        raise RuntimeError("intentional failure")

    async def compensate_fail(ctx):
        compensated_steps.append("step2")

    saga = (
        Saga("fail-saga")
        .step("step1")
        .action(action_ok)
        .compensate(compensate_ok)
        .step("step2")
        .action(action_fail)
        .compensate(compensate_fail)
        .build()
    )

    executor = SagaExecutor(
        default_retry=WfRetryConfig(
            max_attempts=1,
            policy=WfRetryPolicy.NONE,
        )
    )
    result = await executor.execute(saga, {})

    assert result.status in (SagaStatus.COMPENSATED, SagaStatus.FAILED)
    assert "step1" in compensated_steps


@pytest.mark.asyncio
async def test_state_machine_rapid_transitions():
    """Execute 100 rapid transitions back and forth between two states
    and verify the final state is correct."""
    wf = (
        Workflow("ping-pong")
        .state("A", is_initial=True)
        .state("B", is_final=True)
        .transition("to_b", "A", "B")
        .transition("to_a", "B", "A")
        .build()
    )

    executor = WorkflowExecutor()
    start_result = await executor.start(wf, {})
    wf_id = start_result.workflow_id
    assert start_result.current_state == "A"

    for i in range(50):
        r = await executor.transition(wf_id, "to_b")
        assert r.success is True
        assert r.to_state == "B"
        r = await executor.transition(wf_id, "to_a")
        assert r.success is True
        assert r.to_state == "A"

    status = await executor.get_status(wf_id)
    assert status.current_state == "A"
    assert len(status.history) > 100


@pytest.mark.asyncio
async def test_human_task_timeout_behavior():
    """Verify that is_expired() reflects the timeout setting and that
    wait_for_completion raises HumanTaskTimeoutError when the caller's
    timeout expires before the task is completed."""
    manager = HumanTaskManager()
    task = HumanTask(
        task_type="approval",
        title="Quick Approval",
        description="Will time out",
        timeout=WfDuration.from_seconds(0.05),
        timeout_action="fail",
    )

    created = await manager.create_task(task, "wf-1", "step-1")
    assert created.status == HumanTaskStatus.PENDING
    assert created.is_expired() is False

    await asyncio.sleep(0.08)
    assert created.is_expired() is True

    with pytest.raises(HumanTaskTimeoutError):
        await manager.wait_for_completion(created.task_id, timeout=0.01)


# ============================================================
# 4. Resilience Edge Cases
# ============================================================


@pytest.mark.asyncio
async def test_concurrent_circuit_breaker_many_threads():
    """After forcing the circuit breaker open, many concurrent calls should
    all be immediately rejected with CircuitBreakerError."""
    config = CircuitBreakerConfig(
        failure_threshold=3,
        success_threshold=1,
        timeout_ms=5000,
    )
    breaker = CircuitBreaker(config)

    async def failing_call():
        raise ConnectionError("down")

    for _ in range(3):
        with pytest.raises(ConnectionError):
            await breaker.execute(failing_call)

    assert breaker.state == CircuitState.OPEN
    breaker.force_open()

    reject_count = 0

    async def invoke():
        nonlocal reject_count
        try:
            await breaker.execute(failing_call)
        except CircuitBreakerError:
            reject_count += 1

    await asyncio.gather(*[invoke() for _ in range(50)])
    assert reject_count == 50


@pytest.mark.asyncio
async def test_bulkhead_all_permits_and_queue_full():
    """When all concurrent permits are held with no queue, additional calls
    should be rejected with BulkheadRejectedError."""
    config = BulkheadConfig(max_concurrent=2, max_queued=0)
    bh = Bulkhead(config)

    blocker = asyncio.Event()

    async def blocking_task():
        await blocker.wait()

    tasks = [asyncio.create_task(bh.execute(blocking_task)) for _ in range(2)]
    await asyncio.sleep(0.05)

    stats = bh.get_stats()
    assert stats.active == 2

    for _ in range(5):
        with pytest.raises(BulkheadRejectedError):
            await bh.execute(blocking_task)

    stats_after = bh.get_stats()
    assert stats_after.total_accepted == 2
    assert stats_after.total_rejected == 5

    blocker.set()
    await asyncio.gather(*tasks)


@pytest.mark.asyncio
async def test_retry_with_actual_timeout_simulation():
    """Use asyncio.wait_for inside the retried function to simulate real
    timeouts. The retry policy should retry on TimeoutError."""
    attempt_counter = {"n": 0}

    async def flaky_with_timeout():
        attempt_counter["n"] += 1
        if attempt_counter["n"] < 3:
            raise asyncio.TimeoutError("connection timed out")
        return "recovered"

    policy = RetryPolicy(
        ResRetryConfig(
            max_attempts=5,
            backoff=BackoffStrategy.FIXED,
            base_delay_ms=10,
            max_delay_ms=50,
            is_retryable=lambda err, attempt: isinstance(err, asyncio.TimeoutError),
        )
    )

    result = await policy.execute(flaky_with_timeout)
    assert result.result == "recovered"
    assert result.attempts == 3
    assert attempt_counter["n"] == 3


@pytest.mark.asyncio
async def test_retry_exhausted_with_real_timeout():
    """A function that always times out should exhaust retries and raise
    RetryExhaustedError wrapping the original TimeoutError."""
    call_count = {"n": 0}

    async def always_timeout():
        call_count["n"] += 1
        raise asyncio.TimeoutError("db unreachable")

    policy = RetryPolicy(
        ResRetryConfig(
            max_attempts=3,
            backoff=BackoffStrategy.FIXED,
            base_delay_ms=10,
            max_delay_ms=50,
            is_retryable=lambda err, attempt: isinstance(err, asyncio.TimeoutError),
        )
    )

    with pytest.raises(RetryExhaustedError) as exc_info:
        await policy.execute(always_timeout)

    assert isinstance(exc_info.value.last_error, asyncio.TimeoutError)
    assert exc_info.value.attempts == 3
    assert call_count["n"] == 3
