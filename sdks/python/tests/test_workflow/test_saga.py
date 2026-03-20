"""
Tests for Workflow Engine - Saga Module
"""

import pytest
from datetime import datetime
from unittest.mock import AsyncMock, MagicMock

from aether_sdk.workflow.saga import (
    Saga,
    SagaStep,
    SagaExecutor,
    saga,
)
from aether_sdk.workflow.types import (
    SagaStatus,
    StepStatus,
    SagaContext,
    Duration,
    RetryConfig,
    RetryPolicy,
    SagaError,
    SagaStepFailedError,
    SagaCompensationFailedError,
)


class TestSagaStep:
    """Tests for SagaStep."""
    
    def test_create_step(self):
        step = SagaStep(name="test-step")
        assert step.name == "test-step"
        assert step.action is None
        assert step.compensate is None
        assert step.status == StepStatus.PENDING
    
    def test_with_action(self):
        async def my_action(ctx):
            return "result"
        
        step = SagaStep(name="test").with_action(my_action)
        assert step.action is not None
    
    def test_with_compensation(self):
        async def my_compensate(ctx):
            pass
        
        step = SagaStep(name="test").with_compensation(my_compensate)
        assert step.compensate is not None
    
    def test_with_retry(self):
        config = RetryConfig(max_attempts=5)
        step = SagaStep(name="test").with_retry(config)
        assert step.retry_config.max_attempts == 5
    
    def test_with_timeout(self):
        step = SagaStep(name="test").with_timeout(Duration.from_seconds(30))
        assert step.timeout.total_seconds == 30
    
    def test_skip_if(self):
        step = SagaStep(name="test").skip_if(lambda ctx: True)
        assert step.skip_condition is not None


class TestSaga:
    """Tests for Saga definition."""
    
    def test_create_saga(self):
        s = Saga("order-processing")
        assert s.name == "order-processing"
        assert len(s.steps) == 0
    
    def test_add_steps(self):
        s = (
            Saga("test-saga")
            .step("step1")
            .step("step2")
            .step("step3")
        )
        
        assert len(s.steps) == 3
        assert s.steps[0].name == "step1"
        assert s.steps[1].name == "step2"
        assert s.steps[2].name == "step3"
    
    def test_get_step(self):
        s = (
            Saga("test-saga")
            .step("step1")
            .step("step2")
        )
        
        step = s.get_step("step1")
        assert step is not None
        assert step.name == "step1"
        
        assert s.get_step("nonexistent") is None
    
    def test_saga_decorator(self):
        s = saga("decorated-saga")
        assert isinstance(s, Saga)
        assert s.name == "decorated-saga"


class TestSagaExecutor:
    """Tests for SagaExecutor."""
    
    @pytest.fixture
    def executor(self):
        return SagaExecutor()
    
    @pytest.fixture
    def simple_saga(self):
        """Create a simple saga with mock steps."""
        async def action1(ctx):
            ctx.set_state("step1_completed", True)
        
        async def compensate1(ctx):
            ctx.set_state("step1_compensated", True)
        
        async def action2(ctx):
            ctx.set_state("step2_completed", True)
        
        async def compensate2(ctx):
            ctx.set_state("step2_compensated", True)
        
        return (
            Saga("test-saga")
            .step("step1")
            .action(action1)
            .compensate(compensate1)
            .step("step2")
            .action(action2)
            .compensate(compensate2)
            .build()
        )
    
    @pytest.mark.asyncio
    async def test_execute_success(self, executor, simple_saga):
        """Test successful saga execution."""
        result = await executor.execute(simple_saga, {"order_id": "123"})
        
        assert result.status == SagaStatus.COMPLETED
        assert "step1" in result.completed_steps
        assert "step2" in result.completed_steps
        assert result.error is None
        assert result.started_at is not None
        assert result.completed_at is not None
    
    @pytest.mark.asyncio
    async def test_execute_with_context_id(self, executor, simple_saga):
        """Test saga execution with custom context ID."""
        result = await executor.execute(simple_saga, None, context_id="custom-id")
        
        assert result.saga_id == "custom-id"
    
    @pytest.mark.asyncio
    async def test_execute_step_failure_triggers_compensation(self, executor):
        """Test that step failure triggers compensation of completed steps."""
        executed_steps = []
        
        async def action1(ctx):
            executed_steps.append("action1")
        
        async def compensate1(ctx):
            executed_steps.append("compensate1")
        
        async def failing_action(ctx):
            executed_steps.append("failing_action")
            raise ValueError("Step failed!")
        
        failing_saga = (
            Saga("failing-saga")
            .step("step1")
            .action(action1)
            .compensate(compensate1)
            .step("step2")
            .action(failing_action)
            .build()
        )
        
        result = await executor.execute(failing_saga, None)
        
        assert result.status == SagaStatus.COMPENSATED
        assert "step1" in result.completed_steps
        assert "action1" in executed_steps
        assert "compensate1" in executed_steps
    
    @pytest.mark.asyncio
    async def test_skip_step(self, executor):
        """Test skipping steps based on condition."""
        executed_steps = []
        
        async def action1(ctx):
            executed_steps.append("action1")
        
        async def action2(ctx):
            executed_steps.append("action2")
        
        skip_saga = (
            Saga("skip-saga")
            .step("step1")
            .action(action1)
            .step("step2")
            .action(action2)
            .skip_if(lambda ctx: True)  # Always skip
            .build()
        )
        
        result = await executor.execute(skip_saga, None)
        
        assert result.status == SagaStatus.COMPLETED
        assert "action1" in executed_steps
        assert "action2" not in executed_steps  # Should be skipped
    
    @pytest.mark.asyncio
    async def test_retry_on_failure(self, executor):
        """Test retry behavior on transient failures."""
        attempt_count = 0
        
        async def transient_failure(ctx):
            nonlocal attempt_count
            attempt_count += 1
            if attempt_count < 3:
                raise ValueError("Transient error")
            # Success on 3rd attempt
        
        retry_saga = (
            Saga("retry-saga")
            .step("flaky-step")
            .action(transient_failure)
            .retry(RetryConfig(max_attempts=5, policy=RetryPolicy.FIXED))
            .build()
        )
        
        result = await executor.execute(retry_saga, None)
        
        assert result.status == SagaStatus.COMPLETED
        assert attempt_count == 3
    
    @pytest.mark.asyncio
    async def test_max_retries_exceeded(self, executor):
        """Test that saga fails after max retries exceeded."""
        async def always_fail(ctx):
            raise ValueError("Always fails")
        
        fail_saga = (
            Saga("fail-saga")
            .step("failing-step")
            .action(always_fail)
            .retry(RetryConfig(max_attempts=2))
            .build()
        )
        
        result = await executor.execute(fail_saga, None)
        
        assert result.status == SagaStatus.FAILED
        assert result.error is not None
    
    @pytest.mark.asyncio
    async def test_no_compensation_for_no_completed_steps(self, executor):
        """Test that no compensation occurs if no steps completed."""
        async def fail_immediately(ctx):
            raise ValueError("Fail immediately")
        
        no_comp_saga = (
            Saga("no-comp-saga")
            .step("fail-first")
            .action(fail_immediately)
            .build()
        )
        
        result = await executor.execute(no_comp_saga, None)
        
        assert result.status == SagaStatus.FAILED
        assert len(result.completed_steps) == 0


class TestSagaContext:
    """Tests for SagaContext during execution."""
    
    @pytest.mark.asyncio
    async def test_context_state_persistence(self):
        """Test that context state persists across steps."""
        states = {}
        
        async def step1(ctx):
            ctx.set_state("key1", "value1")
        
        async def step2(ctx):
            states["key1"] = ctx.get_state("key1")
            ctx.set_state("key2", "value2")
        
        async def step3(ctx):
            states["key2"] = ctx.get_state("key2")
        
        test_saga = (
            Saga("state-saga")
            .step("step1").action(step1)
            .step("step2").action(step2)
            .step("step3").action(step3)
            .build()
        )
        
        executor = SagaExecutor()
        result = await executor.execute(test_saga, None)
        
        assert result.status == SagaStatus.COMPLETED
        assert states["key1"] == "value1"
        assert states["key2"] == "value2"
    
    @pytest.mark.asyncio
    async def test_step_completion_tracking(self):
        """Test that completed steps are tracked correctly."""
        test_saga = (
            Saga("tracking-saga")
            .step("a").action(lambda ctx: None)
            .step("b").action(lambda ctx: None)
            .step("c").action(lambda ctx: None)
            .build()
        )
        
        executor = SagaExecutor()
        result = await executor.execute(test_saga, None)
        
        assert result.completed_steps == ["a", "b", "c"]


class TestSagaErrors:
    """Tests for saga error handling."""
    
    def test_saga_step_failed_error(self):
        cause = ValueError("Underlying error")
        error = SagaStepFailedError("step1", cause)
        
        assert error.step_name == "step1"
        assert error.cause == cause
        assert "step1" in str(error)
    
    def test_saga_compensation_failed_error(self):
        cause = RuntimeError("Compensation failed")
        error = SagaCompensationFailedError("step1", cause)
        
        assert error.step_name == "step1"
        assert error.cause == cause
