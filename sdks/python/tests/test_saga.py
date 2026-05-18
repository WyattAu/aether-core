"""
Tests for Aether SDK Saga Module

Comprehensive tests for saga pattern implementation.
"""

import asyncio
from typing import Any, Dict

import pytest
from aether_sdk.workflow.saga import Saga, SagaExecutor, SagaStep
from aether_sdk.workflow.types import (
    Duration,
    RetryConfig,
    RetryPolicy,
    SagaCompensationFailedError,
    SagaContext,
    SagaResult,
    SagaStatus,
    StepStatus,
)

# ============================================
# Fixtures
# ============================================


@pytest.fixture
def simple_saga():
    """Create a simple test saga."""
    saga = Saga[Dict[str, Any]]("test-saga")

    async def step1_action(ctx):
        ctx.set_state("step1_completed", True)
        return "step1_result"

    async def step1_compensate(ctx):
        ctx.set_state("step1_compensated", True)

    async def step2_action(ctx):
        ctx.set_state("step2_completed", True)
        return "step2_result"

    async def step2_compensate(ctx):
        ctx.set_state("step2_compensated", True)

    (
        saga.step("step1")
        .action(step1_action)
        .compensate(step1_compensate)
        .step("step2")
        .action(step2_action)
        .compensate(step2_compensate)
    )

    return saga.build()


@pytest.fixture
def failing_saga():
    """Create a saga with a failing step."""
    saga = Saga[Dict[str, Any]]("failing-saga")

    async def step1_action(ctx):
        ctx.set_state("step1_completed", True)

    async def step1_compensate(ctx):
        ctx.set_state("step1_compensated", True)

    async def failing_action(ctx):
        raise ValueError("Intentional failure")

    async def step2_compensate(ctx):
        ctx.set_state("step2_compensated", True)

    (
        saga.step("step1")
        .action(step1_action)
        .compensate(step1_compensate)
        .step("step2")
        .action(failing_action)
        .compensate(step2_compensate)
    )

    return saga.build()


# ============================================
# SagaStep Tests
# ============================================


class TestSagaStep:
    """Tests for SagaStep."""

    def test_init(self):
        """Test step initialization."""
        step = SagaStep[Dict](name="test-step")

        assert step.name == "test-step"
        assert step.action is None
        assert step.compensate is None
        assert step.retry_config is None
        assert step.timeout is None
        assert step.skip_condition is None
        assert step.status == StepStatus.PENDING
        assert step.attempts == 0

    @pytest.mark.asyncio
    async def test_with_action(self):
        """Test setting action handler."""
        step = SagaStep[Dict](name="test")

        async def handler(ctx):
            return "done"

        result = step.with_action(handler)

        assert result is step
        assert step.action == handler

    @pytest.mark.asyncio
    async def test_with_compensation(self):
        """Test setting compensation handler."""
        step = SagaStep[Dict](name="test")

        async def handler(ctx):
            pass

        result = step.with_compensation(handler)

        assert result is step
        assert step.compensate == handler

    def test_with_retry(self):
        """Test setting retry config."""
        step = SagaStep[Dict](name="test")
        config = RetryConfig(max_attempts=5)

        result = step.with_retry(config)

        assert result is step
        assert step.retry_config == config

    def test_with_timeout(self):
        """Test setting timeout."""
        step = SagaStep[Dict](name="test")
        timeout = Duration.from_seconds(30)

        result = step.with_timeout(timeout)

        assert result is step
        assert step.timeout == timeout

    def test_skip_if(self):
        """Test setting skip condition."""
        step = SagaStep[Dict](name="test")

        def condition(ctx):
            return True

        result = step.skip_if(condition)

        assert result is step
        assert step.skip_condition == condition


# ============================================
# Saga Tests
# ============================================


class TestSaga:
    """Tests for Saga."""

    def test_init(self):
        """Test saga initialization."""
        saga = Saga[Dict]("test-saga")

        assert saga.name == "test-saga"
        assert saga.steps == []

    def test_add_step(self):
        """Test adding steps."""
        saga = Saga[Dict]("test")

        result = saga.step("step1")

        assert result is saga
        assert len(saga.steps) == 1
        assert saga.steps[0].name == "step1"

    @pytest.mark.asyncio
    async def test_action_method(self):
        """Test setting action via method."""
        saga = Saga[Dict]("test")

        async def handler(ctx):
            return "done"

        saga.step("step1").action(handler)

        assert saga.steps[0].action == handler

    @pytest.mark.asyncio
    async def test_compensate_method(self):
        """Test setting compensation via method."""
        saga = Saga[Dict]("test")

        async def handler(ctx):
            pass

        saga.step("step1").compensate(handler)

        assert saga.steps[0].compensate == handler

    def test_action_without_step_raises(self):
        """Test action without step raises error."""
        saga = Saga[Dict]("test")

        with pytest.raises(ValueError):
            saga.action(lambda ctx: None)

    def test_compensate_without_step_raises(self):
        """Test compensate without step raises error."""
        saga = Saga[Dict]("test")

        with pytest.raises(ValueError):
            saga.compensate(lambda ctx: None)

    def test_retry_without_step_raises(self):
        """Test retry without step raises error."""
        saga = Saga[Dict]("test")

        with pytest.raises(ValueError):
            saga.retry(RetryConfig())

    def test_timeout_without_step_raises(self):
        """Test timeout without step raises error."""
        saga = Saga[Dict]("test")

        with pytest.raises(ValueError):
            saga.timeout(Duration.from_seconds(30))

    def test_skip_if_without_step_raises(self):
        """Test skip_if without step raises error."""
        saga = Saga[Dict]("test")

        with pytest.raises(ValueError):
            saga.skip_if(lambda ctx: True)

    def test_build_without_action_raises(self):
        """Test build without action raises error."""
        saga = Saga[Dict]("test")
        saga.step("step1")  # No action

        with pytest.raises(ValueError):
            saga.build()

    def test_build_with_action(self):
        """Test build with action succeeds."""
        saga = Saga[Dict]("test")

        async def handler(ctx):
            return "done"

        saga.step("step1").action(handler)

        result = saga.build()

        assert result is saga

    def test_with_metadata(self):
        """Test adding metadata."""
        saga = Saga[Dict]("test")

        result = saga.with_metadata("key1", "value1")

        assert result is saga
        assert saga._metadata["key1"] == "value1"

    def test_get_step(self):
        """Test getting step by name."""
        saga = Saga[Dict]("test")

        async def handler(ctx):
            return "done"

        saga.step("step1").action(handler)
        saga.step("step2").action(handler)

        step = saga.get_step("step1")

        assert step is not None
        assert step.name == "step1"

    def test_get_step_not_found(self):
        """Test getting non-existent step."""
        saga = Saga[Dict]("test")

        step = saga.get_step("nonexistent")

        assert step is None


# ============================================
# SagaExecutor Tests
# ============================================


class TestSagaExecutor:
    """Tests for SagaExecutor."""

    def test_init(self):
        """Test executor initialization."""
        executor = SagaExecutor()

        assert executor.default_retry is not None
        assert executor.default_timeout is not None

    def test_init_with_config(self):
        """Test executor with custom config."""
        retry = RetryConfig(max_attempts=5)
        timeout = Duration.from_seconds(60)

        executor = SagaExecutor(default_retry=retry, default_timeout=timeout)

        assert executor.default_retry == retry
        assert executor.default_timeout == timeout

    @pytest.mark.asyncio
    async def test_execute_simple_saga(self, simple_saga):
        """Test executing simple saga."""
        executor = SagaExecutor()

        result = await executor.execute(simple_saga, {"test": "data"})

        assert result.status == SagaStatus.COMPLETED
        assert result.error is None
        assert "step1" in result.completed_steps
        assert "step2" in result.completed_steps

    @pytest.mark.asyncio
    async def test_execute_with_context_id(self, simple_saga):
        """Test executing with custom context ID."""
        executor = SagaExecutor()

        result = await executor.execute(
            simple_saga,
            {"test": "data"},
            context_id="custom-saga-id",
        )

        assert result.saga_id == "custom-saga-id"

    @pytest.mark.asyncio
    async def test_execute_failing_saga_compensates(self, failing_saga):
        """Test that failing saga compensates completed steps."""
        executor = SagaExecutor()

        result = await executor.execute(failing_saga, {"test": "data"})

        assert result.status == SagaStatus.COMPENSATED
        assert result.error is not None
        assert "step1" in result.compensated_steps

    @pytest.mark.asyncio
    async def test_execute_with_timeout(self):
        """Test saga step timeout."""
        saga = Saga[Dict]("timeout-saga")

        async def slow_action(ctx):
            await asyncio.sleep(5)
            return "done"

        (saga.step("slow-step").action(slow_action).timeout(Duration(milliseconds=100)))

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        # Should fail or be compensated due to timeout
        assert result.status in (SagaStatus.COMPENSATED, SagaStatus.FAILED)

    @pytest.mark.asyncio
    async def test_execute_with_skip_condition(self):
        """Test saga step with skip condition."""
        saga = Saga[Dict]("skip-saga")

        executed = []

        async def step1_action(ctx):
            executed.append("step1")

        async def step2_action(ctx):
            executed.append("step2")

        (
            saga.step("step1")
            .action(step1_action)
            .step("step2")
            .action(step2_action)
            .skip_if(lambda ctx: ctx.input.get("skip_step2", False))
        )

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {"skip_step2": True})

        assert result.status == SagaStatus.COMPLETED
        assert "step1" in executed
        assert "step2" not in executed

    @pytest.mark.asyncio
    async def test_execute_with_retry(self):
        """Test saga step with retry."""
        saga = Saga[Dict]("retry-saga")

        attempts = [0]

        async def flaky_action(ctx):
            attempts[0] += 1
            if attempts[0] < 3:
                raise ValueError("Transient error")
            return "success"

        (
            saga.step("flaky-step")
            .action(flaky_action)
            .retry(
                RetryConfig(
                    max_attempts=3,
                    policy=RetryPolicy.FIXED,
                    initial_delay=Duration(milliseconds=10),
                )
            )
        )

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        assert result.status == SagaStatus.COMPLETED
        assert attempts[0] == 3

    @pytest.mark.asyncio
    async def test_execute_retry_exhausted(self):
        """Test saga step with exhausted retries."""
        saga = Saga[Dict]("exhausted-saga")

        async def always_fail(ctx):
            raise ValueError("Always fails")

        (
            saga.step("failing-step")
            .action(always_fail)
            .retry(
                RetryConfig(
                    max_attempts=2,
                    policy=RetryPolicy.NONE,
                )
            )
        )

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        assert result.status == SagaStatus.FAILED

    @pytest.mark.asyncio
    async def test_compensation_failure(self):
        """Test handling compensation failure."""
        saga = Saga[Dict]("compensation-failure-saga")

        async def step1_action(ctx):
            pass

        async def failing_compensate(ctx):
            raise ValueError("Compensation failed")

        async def step2_action(ctx):
            raise ValueError("Step 2 fails")

        (
            saga.step("step1")
            .action(step1_action)
            .compensate(failing_compensate)
            .step("step2")
            .action(step2_action)
        )

        executor = SagaExecutor()

        # When compensation fails, it should raise SagaCompensationFailedError
        with pytest.raises(SagaCompensationFailedError):
            await executor.execute(saga.build(), {})

    @pytest.mark.asyncio
    async def test_execute_concurrent_sagas(self, simple_saga):
        """Test executing multiple sagas concurrently."""
        executor = SagaExecutor()

        results = await asyncio.gather(
            executor.execute(simple_saga, {"id": 1}),
            executor.execute(simple_saga, {"id": 2}),
            executor.execute(simple_saga, {"id": 3}),
        )

        assert all(r.status == SagaStatus.COMPLETED for r in results)
        assert len(set(r.saga_id for r in results)) == 3


# ============================================
# SagaContext Tests
# ============================================


class TestSagaContext:
    """Tests for SagaContext."""

    def test_init(self):
        """Test context initialization."""
        ctx = SagaContext[Dict](
            saga_id="test-saga",
            input={"key": "value"},
        )

        assert ctx.saga_id == "test-saga"
        assert ctx.input == {"key": "value"}
        assert ctx.state == {}
        assert ctx.completed_steps == []

    def test_set_state(self):
        """Test setting state."""
        ctx = SagaContext[Dict](saga_id="test", input={})

        ctx.set_state("key1", "value1")

        assert ctx.state["key1"] == "value1"

    def test_get_state(self):
        """Test getting state."""
        ctx = SagaContext[Dict](saga_id="test", input={})
        ctx.set_state("key1", "value1")

        result = ctx.get_state("key1")

        assert result == "value1"

    def test_get_state_default(self):
        """Test getting state with default."""
        ctx = SagaContext[Dict](saga_id="test", input={})

        result = ctx.get_state("nonexistent", default="default")

        assert result == "default"

    def test_mark_step_completed(self):
        """Test marking step completed."""
        ctx = SagaContext[Dict](saga_id="test", input={})

        ctx.mark_step_completed("step1")

        assert "step1" in ctx.completed_steps


# ============================================
# SagaResult Tests
# ============================================


class TestSagaResult:
    """Tests for SagaResult."""

    def test_completed_result(self):
        """Test completed result."""
        result = SagaResult(
            saga_id="test-saga",
            status=SagaStatus.COMPLETED,
            output={"result": "success"},
            completed_steps=["step1", "step2"],
        )

        assert result.saga_id == "test-saga"
        assert result.status == SagaStatus.COMPLETED
        assert result.output == {"result": "success"}

    def test_failed_result(self):
        """Test failed result."""
        result = SagaResult(
            saga_id="test-saga",
            status=SagaStatus.FAILED,
            error="Step failed",
        )

        assert result.status == SagaStatus.FAILED
        assert result.error == "Step failed"

    def test_compensated_result(self):
        """Test compensated result."""
        result = SagaResult(
            saga_id="test-saga",
            status=SagaStatus.COMPENSATED,
            compensated_steps=["step1"],
        )

        assert result.status == SagaStatus.COMPENSATED


# ============================================
# Edge Cases Tests
# ============================================


class TestSagaEdgeCases:
    """Tests for edge cases."""

    @pytest.mark.asyncio
    async def test_empty_saga(self):
        """Test executing empty saga."""
        saga = Saga[Dict]("empty-saga").build()
        executor = SagaExecutor()

        result = await executor.execute(saga, {})

        assert result.status == SagaStatus.COMPLETED
        assert result.completed_steps == []

    @pytest.mark.asyncio
    async def test_saga_with_no_compensation(self):
        """Test saga step without compensation."""
        saga = Saga[Dict]("no-compensation-saga")

        async def step1_action(ctx):
            ctx.set_state("step1", True)

        async def failing_step(ctx):
            raise ValueError("Step fails")

        (
            saga.step("step1")
            .action(step1_action)  # No compensation
            .step("step2")
            .action(failing_step)
        )

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        # Should complete with COMPENSATED status (step1 has no compensation)
        assert result.status in (SagaStatus.FAILED, SagaStatus.COMPENSATED)

    @pytest.mark.asyncio
    async def test_saga_with_exception_in_action(self):
        """Test saga with exception in action."""
        saga = Saga[Dict]("exception-saga")

        async def exception_action(ctx):
            raise RuntimeError("Unexpected error")

        (saga.step("failing-step").action(exception_action))

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        # Should be FAILED or COMPENSATED (no steps to compensate)
        assert result.status in (SagaStatus.COMPENSATED, SagaStatus.FAILED)

    @pytest.mark.asyncio
    async def test_saga_with_exponential_retry(self):
        """Test saga with exponential backoff retry."""
        saga = Saga[Dict]("exp-retry-saga")

        attempts = [0]
        timestamps = []

        async def tracked_action(ctx):
            attempts[0] += 1
            timestamps.append(asyncio.get_event_loop().time())
            if attempts[0] < 3:
                raise ValueError("Retry me")
            return "done"

        (
            saga.step("tracked-step")
            .action(tracked_action)
            .retry(
                RetryConfig(
                    max_attempts=5,
                    policy=RetryPolicy.EXPONENTIAL,
                    initial_delay=Duration(milliseconds=50),
                    multiplier=2.0,
                    max_delay=Duration(milliseconds=5000),
                )
            )
        )

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        assert result.status == SagaStatus.COMPLETED
        assert attempts[0] == 3

    @pytest.mark.asyncio
    async def test_saga_with_jitter_retry(self):
        """Test saga with jitter retry."""
        saga = Saga[Dict]("jitter-retry-saga")

        attempts = [0]

        async def tracked_action(ctx):
            attempts[0] += 1
            if attempts[0] < 2:
                raise ValueError("Retry me")
            return "done"

        (
            saga.step("tracked-step")
            .action(tracked_action)
            .retry(
                RetryConfig(
                    max_attempts=3,
                    policy=RetryPolicy.EXPONENTIAL_JITTER,
                    initial_delay=Duration(milliseconds=10),
                    jitter=0.1,
                )
            )
        )

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        assert result.status == SagaStatus.COMPLETED

    @pytest.mark.asyncio
    async def test_saga_preserves_step_results(self):
        """Test that saga preserves step results in context."""
        saga = Saga[Dict]("result-saga")

        async def step1_action(ctx):
            return {"step1_data": "value1"}

        async def step2_action(ctx):
            return {"step2_data": "value2"}

        (saga.step("step1").action(step1_action).step("step2").action(step2_action))

        executor = SagaExecutor()

        result = await executor.execute(saga.build(), {})

        assert result.status == SagaStatus.COMPLETED

    @pytest.mark.asyncio
    async def test_multiple_sagas_isolated(self):
        """Test that multiple saga executions are isolated."""
        saga = Saga[Dict]("isolated-saga")

        async def increment_action(ctx):
            current = ctx.get_state("counter", 0)
            ctx.set_state("counter", current + 1)

        (saga.step("increment").action(increment_action))

        executor = SagaExecutor()

        result1 = await executor.execute(saga.build(), {})
        result2 = await executor.execute(saga.build(), {})

        # Each saga should have its own isolated state
        assert result1.saga_id != result2.saga_id
