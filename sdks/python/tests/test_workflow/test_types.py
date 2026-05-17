"""
Tests for Workflow Engine - Types Module
"""

from datetime import timedelta

from aether_sdk.workflow.types import (
    Duration,
    HumanTaskContext,
    HumanTaskError,
    HumanTaskStatus,
    InvalidTransitionError,
    RetryConfig,
    RetryPolicy,
    SagaContext,
    SagaResult,
    SagaStatus,
    SagaStepFailedError,
    TransitionResult,
    WorkflowContext,
    WorkflowError,
    WorkflowResult,
    WorkflowStatus,
)


class TestDuration:
    """Tests for Duration type."""

    def test_from_seconds(self):
        d = Duration.from_seconds(5)
        assert d.milliseconds == 5000
        assert d.total_seconds == 5.0

    def test_from_minutes(self):
        d = Duration.from_minutes(2)
        assert d.milliseconds == 120000
        assert d.total_minutes == 2.0

    def test_from_hours(self):
        d = Duration.from_hours(1.5)
        assert d.milliseconds == 5400000
        assert d.total_hours == 1.5

    def test_from_days(self):
        d = Duration.from_days(1)
        assert d.milliseconds == 86400000

    def test_to_timedelta(self):
        d = Duration.from_seconds(30)
        td = d.to_timedelta()
        assert td == timedelta(seconds=30)

    def test_add(self):
        d1 = Duration.from_seconds(10)
        d2 = Duration.from_seconds(5)
        result = d1 + d2
        assert result.milliseconds == 15000

    def test_subtract(self):
        d1 = Duration.from_seconds(10)
        d2 = Duration.from_seconds(3)
        result = d1 - d2
        assert result.milliseconds == 7000

    def test_subtract_no_negative(self):
        d1 = Duration.from_seconds(5)
        d2 = Duration.from_seconds(10)
        result = d1 - d2
        assert result.milliseconds == 0  # Clamped to 0


class TestRetryConfig:
    """Tests for RetryConfig."""

    def test_default_values(self):
        config = RetryConfig()
        assert config.max_attempts == 3
        assert config.policy == RetryPolicy.EXPONENTIAL
        assert config.multiplier == 2.0
        assert config.jitter == 0.1

    def test_custom_values(self):
        config = RetryConfig(
            max_attempts=5,
            policy=RetryPolicy.FIXED,
            initial_delay=Duration.from_seconds(2),
            max_delay=Duration.from_seconds(120),
            multiplier=1.5,
            jitter=0.2,
        )
        assert config.max_attempts == 5
        assert config.policy == RetryPolicy.FIXED


class TestEnums:
    """Tests for enum types."""

    def test_saga_status_values(self):
        assert SagaStatus.PENDING.value == "pending"
        assert SagaStatus.RUNNING.value == "running"
        assert SagaStatus.COMPLETED.value == "completed"
        assert SagaStatus.COMPENSATING.value == "compensating"
        assert SagaStatus.COMPENSATED.value == "compensated"
        assert SagaStatus.FAILED.value == "failed"

    def test_workflow_status_values(self):
        assert WorkflowStatus.CREATED.value == "created"
        assert WorkflowStatus.RUNNING.value == "running"
        assert WorkflowStatus.SUSPENDED.value == "suspended"
        assert WorkflowStatus.COMPLETED.value == "completed"
        assert WorkflowStatus.FAILED.value == "failed"
        assert WorkflowStatus.CANCELLED.value == "cancelled"

    def test_human_task_status_values(self):
        assert HumanTaskStatus.PENDING.value == "pending"
        assert HumanTaskStatus.ASSIGNED.value == "assigned"
        assert HumanTaskStatus.IN_PROGRESS.value == "in_progress"
        assert HumanTaskStatus.COMPLETED.value == "completed"
        assert HumanTaskStatus.REJECTED.value == "rejected"
        assert HumanTaskStatus.TIMEOUT.value == "timeout"


class TestSagaContext:
    """Tests for SagaContext."""

    def test_create_default(self):
        ctx = SagaContext(input={"order_id": "123"})
        assert ctx.saga_id is not None
        assert ctx.input == {"order_id": "123"}
        assert ctx.state == {}
        assert ctx.completed_steps == []
        assert ctx.failed_step is None

    def test_state_management(self):
        ctx = SagaContext(input=None)
        ctx.set_state("key1", "value1")
        ctx.set_state("key2", 42)

        assert ctx.get_state("key1") == "value1"
        assert ctx.get_state("key2") == 42
        assert ctx.get_state("missing") is None
        assert ctx.get_state("missing", "default") == "default"

    def test_step_tracking(self):
        ctx = SagaContext(input=None)

        assert not ctx.is_step_completed("step1")

        ctx.mark_step_completed("step1")
        assert ctx.is_step_completed("step1")
        assert "step1" in ctx.completed_steps

        # Duplicate marking should not add again
        ctx.mark_step_completed("step1")
        assert ctx.completed_steps.count("step1") == 1


class TestWorkflowContext:
    """Tests for WorkflowContext."""

    def test_create_default(self):
        ctx = WorkflowContext(input={"data": "test"})
        assert ctx.workflow_id is not None
        assert ctx.current_state == ""
        assert ctx.variables == {}
        assert ctx.history == []

    def test_variable_management(self):
        ctx = WorkflowContext(input=None)
        ctx.set_variable("var1", "value1")
        ctx.set_variable("var2", 100)

        assert ctx.get_variable("var1") == "value1"
        assert ctx.get_variable("var2") == 100
        assert ctx.get_variable("missing") is None
        assert ctx.get_variable("missing", "default") == "default"

    def test_history_events(self):
        ctx = WorkflowContext(input=None)
        ctx.add_history_event("transition", from_state="A", to_state="B")

        assert len(ctx.history) == 1
        event = ctx.history[0]
        assert event["type"] == "transition"
        assert event["details"]["from_state"] == "A"
        assert event["details"]["to_state"] == "B"


class TestHumanTaskContext:
    """Tests for HumanTaskContext."""

    def test_create_default(self):
        ctx = HumanTaskContext(
            task_type="approval",
            title="Approve Request",
        )
        assert ctx.task_id is not None
        assert ctx.task_type == "approval"
        assert ctx.title == "Approve Request"
        assert ctx.status == HumanTaskStatus.PENDING
        assert ctx.priority == 5
        assert ctx.candidate_users == []
        assert ctx.candidate_groups == []

    def test_with_candidates(self):
        ctx = HumanTaskContext(
            task_type="review",
            title="Review Document",
        )
        ctx.candidate_users = ["user1", "user2"]
        ctx.candidate_groups = ["managers"]
        ctx.priority = 1

        assert "user1" in ctx.candidate_users
        assert "managers" in ctx.candidate_groups
        assert ctx.priority == 1


class TestResults:
    """Tests for result types."""

    def test_saga_result(self):
        result = SagaResult(
            saga_id="saga-123",
            status=SagaStatus.COMPLETED,
            output={"result": "success"},
            completed_steps=["step1", "step2"],
        )
        assert result.saga_id == "saga-123"
        assert result.status == SagaStatus.COMPLETED
        assert result.output == {"result": "success"}

    def test_workflow_result(self):
        result = WorkflowResult(
            workflow_id="wf-456",
            status=WorkflowStatus.COMPLETED,
            current_state="final",
        )
        assert result.workflow_id == "wf-456"
        assert result.status == WorkflowStatus.COMPLETED
        assert result.current_state == "final"

    def test_transition_result(self):
        result = TransitionResult(
            success=True,
            from_state="state1",
            to_state="state2",
        )
        assert result.success is True
        assert result.from_state == "state1"
        assert result.to_state == "state2"
        assert result.error is None


class TestExceptions:
    """Tests for custom exceptions."""

    def test_saga_error(self):
        error = SagaStepFailedError("step1", ValueError("Something went wrong"))
        assert "step1" in str(error)
        assert error.step_name == "step1"
        assert isinstance(error.cause, ValueError)

    def test_workflow_error(self):
        error = WorkflowError("Workflow failed")
        assert "Workflow failed" in str(error)

    def test_invalid_transition_error(self):
        error = InvalidTransitionError("state1", "state2", "wf-123")
        assert "state1" in str(error)
        assert "state2" in str(error)
        assert "wf-123" in str(error)
        assert error.from_state == "state1"
        assert error.to_state == "state2"

    def test_human_task_error(self):
        error = HumanTaskError("Task not found")
        assert "Task not found" in str(error)
