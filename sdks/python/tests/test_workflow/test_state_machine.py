"""
Tests for Workflow Engine - State Machine Module
"""

import pytest
from aether_sdk.workflow.state_machine import (
    State,
    Transition,
    Workflow,
    WorkflowExecutor,
    workflow,
)
from aether_sdk.workflow.types import (
    Duration,
    InvalidTransitionError,
    WorkflowError,
    WorkflowStatus,
    WorkflowSuspendedError,
)


class TestState:
    """Tests for State definition."""

    def test_create_state(self):
        state = State(name="draft")
        assert state.name == "draft"
        assert state.is_initial is False
        assert state.is_final is False

    def test_initial_state(self):
        state = State(name="start", is_initial=True)
        assert state.is_initial is True

    def test_final_state(self):
        state = State(name="end", is_final=True)
        assert state.is_final is True

    def test_state_with_timeout(self):
        state = State(
            name="waiting",
            timeout=Duration.from_seconds(60),
            timeout_transition="timeout",
        )
        assert state.timeout.total_seconds == 60
        assert state.timeout_transition == "timeout"


class TestTransition:
    """Tests for Transition definition."""

    def test_create_transition(self):
        transition = Transition(
            name="submit",
            from_state="draft",
            to_state="pending",
        )
        assert transition.name == "submit"
        assert transition.from_state == "draft"
        assert transition.to_state == "pending"

    def test_transition_with_guard(self):
        transition = Transition(
            name="approve",
            from_state="pending",
            to_state="approved",
            guard=lambda ctx: ctx.get_variable("amount", 0) < 1000,
        )
        assert transition.guard is not None


class TestWorkflow:
    """Tests for Workflow definition."""

    def test_create_workflow(self):
        wf = Workflow("approval-flow")
        assert wf.name == "approval-flow"

    def test_add_states(self):
        wf = (
            Workflow("test")
            .state("draft", is_initial=True)
            .state("pending")
            .state("approved", is_final=True)
            .build()
        )

        assert "draft" in wf.states
        assert "pending" in wf.states
        assert "approved" in wf.states
        assert wf.initial_state == "draft"
        assert wf.is_final_state("approved")

    def test_add_transitions(self):
        wf = (
            Workflow("test")
            .state("a", is_initial=True)
            .state("b")
            .transition("go", from_state="a", to_state="b")
            .build()
        )

        transitions = wf.get_transitions("a")
        assert len(transitions) == 1
        assert transitions[0].name == "go"

    def test_workflow_decorator(self):
        wf = workflow("decorated-workflow")
        assert isinstance(wf, Workflow)
        assert wf.name == "decorated-workflow"

    def test_multiple_initial_states_error(self):
        with pytest.raises(WorkflowError):
            (Workflow("test").state("a", is_initial=True).state("b", is_initial=True))

    def test_no_initial_state_error(self):
        wf = Workflow("test").state("a")
        with pytest.raises(WorkflowError):
            wf.build()

    def test_invalid_source_state_error(self):
        wf = Workflow("test").state("a", is_initial=True)
        with pytest.raises(WorkflowError):
            wf.transition("go", from_state="nonexistent", to_state="a")

    def test_invalid_target_state_error(self):
        wf = Workflow("test").state("a", is_initial=True)
        with pytest.raises(WorkflowError):
            wf.transition("go", from_state="a", to_state="nonexistent")


class TestWorkflowExecutor:
    """Tests for WorkflowExecutor."""

    @pytest.fixture
    def simple_workflow(self):
        """Create a simple approval workflow."""
        return (
            Workflow("approval")
            .state("draft", is_initial=True)
            .state("pending")
            .state("approved", is_final=True)
            .state("rejected", is_final=True)
            .transition("submit", from_state="draft", to_state="pending")
            .transition("approve", from_state="pending", to_state="approved")
            .transition("reject", from_state="pending", to_state="rejected")
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_start_workflow(self, executor, simple_workflow):
        """Test starting a new workflow."""
        result = await executor.start(simple_workflow, {"request_id": "123"})

        assert result.status == WorkflowStatus.RUNNING
        assert result.current_state == "draft"
        assert result.started_at is not None

    @pytest.mark.asyncio
    async def test_start_with_custom_id(self, executor, simple_workflow):
        """Test starting workflow with custom ID."""
        result = await executor.start(
            simple_workflow,
            {"request_id": "123"},
            workflow_id="custom-wf-id",
        )

        assert result.workflow_id == "custom-wf-id"

    @pytest.mark.asyncio
    async def test_valid_transition(self, executor, simple_workflow):
        """Test executing a valid transition."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        transition_result = await executor.transition(wf_id, "submit")

        assert transition_result.success is True
        assert transition_result.from_state == "draft"
        assert transition_result.to_state == "pending"

    @pytest.mark.asyncio
    async def test_invalid_transition(self, executor, simple_workflow):
        """Test executing an invalid transition."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        # Try to approve from draft (invalid)
        with pytest.raises(InvalidTransitionError):
            await executor.transition(wf_id, "approve")

    @pytest.mark.asyncio
    async def test_transition_to_final_state(self, executor, simple_workflow):
        """Test transitioning to a final state."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        await executor.transition(wf_id, "submit")
        await executor.transition(wf_id, "approve")

        status = await executor.get_status(wf_id)
        assert status.status == WorkflowStatus.COMPLETED
        assert status.current_state == "approved"

    @pytest.mark.asyncio
    async def test_suspend_and_resume(self, executor, simple_workflow):
        """Test suspending and resuming workflow."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        await executor.suspend(wf_id, "Waiting for external input")

        status = await executor.get_status(wf_id)
        assert status.status == WorkflowStatus.SUSPENDED

        # Cannot transition while suspended
        with pytest.raises(WorkflowSuspendedError):
            await executor.transition(wf_id, "submit")

        await executor.resume(wf_id)

        status = await executor.get_status(wf_id)
        assert status.status == WorkflowStatus.RUNNING

    @pytest.mark.asyncio
    async def test_cancel_workflow(self, executor, simple_workflow):
        """Test cancelling a workflow."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        await executor.cancel(wf_id, "User requested cancellation")

        status = await executor.get_status(wf_id)
        assert status.status == WorkflowStatus.CANCELLED

    @pytest.mark.asyncio
    async def test_get_available_transitions(self, executor, simple_workflow):
        """Test getting available transitions."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        # From draft, only submit should be available
        available = executor.get_available_transitions(wf_id)
        assert available == ["submit"]

        # After submit, approve and reject should be available
        await executor.transition(wf_id, "submit")
        available = executor.get_available_transitions(wf_id)
        assert set(available) == {"approve", "reject"}

    @pytest.mark.asyncio
    async def test_history_tracking(self, executor, simple_workflow):
        """Test that history is tracked correctly."""
        result = await executor.start(simple_workflow, {"request_id": "123"})
        wf_id = result.workflow_id

        await executor.transition(wf_id, "submit")
        await executor.transition(wf_id, "approve")

        status = await executor.get_status(wf_id)

        # Should have workflow_started + 2 transitions
        event_types = [h["type"] for h in status.history]
        assert "workflow_started" in event_types
        assert event_types.count("transition") == 2


class TestWorkflowWithGuards:
    """Tests for workflows with guard conditions."""

    @pytest.fixture
    def workflow_with_guard(self):
        """Create a workflow with guard conditions."""
        return (
            Workflow("expense-approval")
            .state("submitted", is_initial=True)
            .state("approved", is_final=True)
            .transition(
                "auto_approve",
                from_state="submitted",
                to_state="approved",
                guard=lambda ctx: ctx.get_variable("amount", 0) < 100,
            )
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_guard_allows_transition(self, executor, workflow_with_guard):
        """Test that guard allows transition when condition is met."""
        result = await executor.start(workflow_with_guard, {})
        wf_id = result.workflow_id

        # Set amount below threshold
        ctx = executor._workflows[wf_id]
        ctx.set_variable("amount", 50)

        available = executor.get_available_transitions(wf_id)
        assert "auto_approve" in available

    @pytest.mark.asyncio
    async def test_guard_blocks_transition(self, executor, workflow_with_guard):
        """Test that guard blocks transition when condition is not met."""
        result = await executor.start(workflow_with_guard, {})
        wf_id = result.workflow_id

        # Set amount above threshold
        ctx = executor._workflows[wf_id]
        ctx.set_variable("amount", 500)

        available = executor.get_available_transitions(wf_id)
        assert "auto_approve" not in available


class TestWorkflowWithActions:
    """Tests for workflows with transition actions."""

    @pytest.fixture
    def workflow_with_actions(self):
        """Create a workflow with transition actions."""
        self.action_executed = False
        self.enter_executed = False
        self.exit_executed = False

        async def transition_action(ctx):
            self.action_executed = True
            ctx.set_variable("action_ran", True)

        async def on_enter_handler(ctx):
            self.enter_executed = True

        async def on_exit_handler(ctx):
            self.exit_executed = True

        return (
            Workflow("action-workflow")
            .state("start", is_initial=True)
            .on_exit("start", on_exit_handler)
            .state("end", is_final=True)
            .on_enter("end", on_enter_handler)
            .transition("go", from_state="start", to_state="end")
            .with_action("go", transition_action)
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_actions_executed(self, executor, workflow_with_actions):
        """Test that transition and state actions are executed."""
        result = await executor.start(workflow_with_actions, {})
        wf_id = result.workflow_id

        await executor.transition(wf_id, "go")

        assert self.action_executed
        assert self.enter_executed
        assert self.exit_executed

        status = await executor.get_status(wf_id)
        assert status.output.get("action_ran") is True


class TestWorkflowErrors:
    """Tests for workflow error handling."""

    def test_multiple_initial_states_error(self):
        """Test that multiple initial states raise an error."""
        wf = Workflow("test")
        wf.state("a", is_initial=True)

        with pytest.raises(WorkflowError, match="Multiple initial states"):
            wf.state("b", is_initial=True)

    def test_on_enter_unknown_state_error(self):
        """Test that on_enter for unknown state raises an error."""
        wf = Workflow("test")
        wf.state("a", is_initial=True)

        with pytest.raises(WorkflowError, match="Unknown state"):
            wf.on_enter("unknown", lambda ctx: None)

    def test_on_exit_unknown_state_error(self):
        """Test that on_exit for unknown state raises an error."""
        wf = Workflow("test")
        wf.state("a", is_initial=True)

        with pytest.raises(WorkflowError, match="Unknown state"):
            wf.on_exit("unknown", lambda ctx: None)

    def test_transition_unknown_source_state_error(self):
        """Test that transition from unknown state raises an error."""
        wf = Workflow("test")
        wf.state("a", is_initial=True)
        wf.state("b")

        with pytest.raises(WorkflowError, match="Unknown source state"):
            wf.transition("go", from_state="unknown", to_state="b")

    def test_transition_unknown_target_state_error(self):
        """Test that transition to unknown state raises an error."""
        wf = Workflow("test")
        wf.state("a", is_initial=True)
        wf.state("b")

        with pytest.raises(WorkflowError, match="Unknown target state"):
            wf.transition("go", from_state="a", to_state="unknown")

    def test_with_action_unknown_transition_error(self):
        """Test that with_action for unknown transition raises an error."""
        wf = (
            Workflow("test")
            .state("a", is_initial=True)
            .state("b")
            .transition("go", from_state="a", to_state="b")
        )

        with pytest.raises(WorkflowError, match="Unknown transition"):
            wf.with_action("unknown", lambda ctx: None)

    def test_build_without_initial_state_error(self):
        """Test that build without initial state raises an error."""
        wf = Workflow("test")
        wf.state("a")  # Not initial

        with pytest.raises(WorkflowError, match="No initial state"):
            wf.build()

    def test_initial_state_property_without_initial_state(self):
        """Test accessing initial_state property without initial state."""
        wf = Workflow("test")

        with pytest.raises(WorkflowError, match="No initial state"):
            _ = wf.initial_state


class TestWorkflowMetadata:
    """Tests for workflow metadata."""

    def test_with_metadata(self):
        """Test adding metadata to workflow."""
        wf = (
            Workflow("test")
            .state("a", is_initial=True)
            .with_metadata("version", "1.0")
            .with_metadata("author", "test")
            .build()
        )

        assert wf._metadata["version"] == "1.0"
        assert wf._metadata["author"] == "test"


class TestWorkflowGuardValidation:
    """Tests for guard validation in transitions."""

    @pytest.fixture
    def workflow_with_guard(self):
        return (
            Workflow("guarded")
            .state("start", is_initial=True)
            .state("end", is_final=True)
            .transition(
                "proceed",
                from_state="start",
                to_state="end",
                guard=lambda ctx: ctx.get_variable("allowed", False),
            )
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_validate_transition_guard_passes(
        self, executor, workflow_with_guard
    ):
        """Test validate_transition when guard passes."""
        result = await executor.start(workflow_with_guard, {})
        wf_id = result.workflow_id

        ctx = executor._workflows[wf_id]
        ctx.set_variable("allowed", True)

        transition = workflow_with_guard.validate_transition("start", "proceed", ctx)
        assert transition is not None
        assert transition.name == "proceed"

    @pytest.mark.asyncio
    async def test_validate_transition_guard_fails(self, executor, workflow_with_guard):
        """Test validate_transition when guard fails."""
        result = await executor.start(workflow_with_guard, {})
        wf_id = result.workflow_id

        ctx = executor._workflows[wf_id]
        ctx.set_variable("allowed", False)

        transition = workflow_with_guard.validate_transition("start", "proceed", ctx)
        assert transition is None

    @pytest.mark.asyncio
    async def test_validate_transition_not_found(self, executor, workflow_with_guard):
        """Test validate_transition when transition doesn't exist."""
        result = await executor.start(workflow_with_guard, {})
        wf_id = result.workflow_id

        ctx = executor._workflows[wf_id]

        transition = workflow_with_guard.validate_transition(
            "start", "nonexistent", ctx
        )
        assert transition is None


class TestWorkflowExecutorErrors:
    """Tests for executor error handling."""

    @pytest.fixture
    def simple_workflow(self):
        return (
            Workflow("simple")
            .state("start", is_initial=True)
            .state("end", is_final=True)
            .transition("go", from_state="start", to_state="end")
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_transition_unknown_workflow(self, executor):
        """Test transition on unknown workflow raises error."""
        with pytest.raises(WorkflowError, match="Unknown workflow"):
            await executor.transition("unknown-id", "go")

    @pytest.mark.asyncio
    async def test_get_status_unknown_workflow(self, executor):
        """Test get_status on unknown workflow returns None."""
        result = await executor.get_status("unknown-id")
        assert result is None

    @pytest.mark.asyncio
    async def test_suspend_unknown_workflow(self, executor):
        """Test suspend on unknown workflow raises error."""
        with pytest.raises(WorkflowError, match="Unknown workflow"):
            await executor.suspend("unknown-id")

    @pytest.mark.asyncio
    async def test_resume_unknown_workflow(self, executor):
        """Test resume on unknown workflow raises error."""
        with pytest.raises(WorkflowError, match="Unknown workflow"):
            await executor.resume("unknown-id")

    @pytest.mark.asyncio
    async def test_cancel_unknown_workflow(self, executor):
        """Test cancel on unknown workflow raises error."""
        with pytest.raises(WorkflowError, match="Unknown workflow"):
            await executor.cancel("unknown-id")

    @pytest.mark.asyncio
    async def test_resume_non_suspended_workflow(self, executor, simple_workflow):
        """Test resume on non-suspended workflow raises error."""
        result = await executor.start(simple_workflow, {})

        with pytest.raises(WorkflowError, match="is not suspended"):
            await executor.resume(result.workflow_id)

    @pytest.mark.asyncio
    async def test_transition_suspended_workflow(self, executor, simple_workflow):
        """Test transition on suspended workflow raises error."""
        result = await executor.start(simple_workflow, {})
        await executor.suspend(result.workflow_id)

        with pytest.raises(WorkflowSuspendedError):
            await executor.transition(result.workflow_id, "go")


class TestTransitionFailure:
    """Tests for transition failure handling."""

    @pytest.fixture
    def workflow_with_failing_action(self):
        async def failing_action(ctx):
            raise RuntimeError("Action failed!")

        return (
            Workflow("failing")
            .state("start", is_initial=True)
            .state("end", is_final=True)
            .transition("go", from_state="start", to_state="end")
            .with_action("go", failing_action)
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_transition_failure_returns_error(
        self, executor, workflow_with_failing_action
    ):
        """Test that transition failure is captured in result."""
        result = await executor.start(workflow_with_failing_action, {})
        wf_id = result.workflow_id

        transition_result = await executor.transition(wf_id, "go")

        assert transition_result.success is False
        assert "Action failed" in transition_result.error
        assert transition_result.from_state == "start"
        assert transition_result.to_state == "end"

        # Verify history event was added (events use "type" key, not "event")
        ctx = executor._workflows[wf_id]
        assert any(e.get("type") == "transition_failed" for e in ctx.history)


class TestWorkflowFactory:
    """Tests for workflow factory function."""

    def test_workflow_factory(self):
        """Test the workflow() factory function."""
        wf = workflow("factory-test")
        assert wf.name == "factory-test"

        wf = (
            wf.state("start", is_initial=True)
            .state("end", is_final=True)
            .transition("go", from_state="start", to_state="end")
            .build()
        )

        assert "start" in wf.states
        assert "end" in wf.states


class TestGetAvailableTransitions:
    """Tests for get_available_transitions method."""

    @pytest.fixture
    def workflow_with_multiple_transitions(self):
        return (
            Workflow("multi")
            .state("start", is_initial=True)
            .state("a")
            .state("b")
            .transition("to_a", from_state="start", to_state="a")
            .transition("to_b", from_state="start", to_state="b")
            .transition(
                "guarded",
                from_state="start",
                to_state="b",
                guard=lambda ctx: ctx.get_variable("allow", False),
            )
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_get_available_transitions_all(
        self, executor, workflow_with_multiple_transitions
    ):
        """Test getting all available transitions."""
        result = await executor.start(workflow_with_multiple_transitions, {})
        wf_id = result.workflow_id

        available = executor.get_available_transitions(wf_id)

        # Should include to_a, to_b, but not guarded (guard returns False)
        assert "to_a" in available
        assert "to_b" in available
        assert "guarded" not in available

    @pytest.mark.asyncio
    async def test_get_available_transitions_with_guard_pass(
        self, executor, workflow_with_multiple_transitions
    ):
        """Test getting available transitions with passing guard."""
        result = await executor.start(workflow_with_multiple_transitions, {})
        wf_id = result.workflow_id

        ctx = executor._workflows[wf_id]
        ctx.set_variable("allow", True)

        available = executor.get_available_transitions(wf_id)

        assert "guarded" in available

    @pytest.mark.asyncio
    async def test_get_available_transitions_unknown_workflow(self, executor):
        """Test getting transitions for unknown workflow."""
        available = executor.get_available_transitions("unknown-id")
        assert available == []


class TestWorkflowCompletion:
    """Tests for workflow completion detection."""

    @pytest.fixture
    def workflow_with_final_state(self):
        return (
            Workflow("completion")
            .state("start", is_initial=True)
            .state("processing")
            .state("done", is_final=True)
            .transition("process", from_state="start", to_state="processing")
            .transition("finish", from_state="processing", to_state="done")
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_workflow_not_completed_in_intermediate_state(
        self, executor, workflow_with_final_state
    ):
        """Test that workflow is not completed in intermediate state."""
        result = await executor.start(workflow_with_final_state, {})
        wf_id = result.workflow_id

        await executor.transition(wf_id, "process")

        status = await executor.get_status(wf_id)
        assert status.status == WorkflowStatus.RUNNING

    @pytest.mark.asyncio
    async def test_workflow_completed_in_final_state(
        self, executor, workflow_with_final_state
    ):
        """Test that workflow is completed when reaching final state."""
        result = await executor.start(workflow_with_final_state, {})
        wf_id = result.workflow_id

        await executor.transition(wf_id, "process")
        await executor.transition(wf_id, "finish")

        status = await executor.get_status(wf_id)
        assert status.status == WorkflowStatus.COMPLETED


class TestWorkflowWithCustomID:
    """Tests for workflow with custom ID."""

    @pytest.fixture
    def simple_workflow(self):
        return (
            Workflow("simple")
            .state("start", is_initial=True)
            .state("end", is_final=True)
            .transition("go", from_state="start", to_state="end")
            .build()
        )

    @pytest.fixture
    def executor(self):
        return WorkflowExecutor()

    @pytest.mark.asyncio
    async def test_start_with_custom_id(self, executor, simple_workflow):
        """Test starting workflow with custom ID."""
        custom_id = "my-custom-workflow-id"
        result = await executor.start(simple_workflow, {}, workflow_id=custom_id)

        assert result.workflow_id == custom_id

        # Verify we can access it with the custom ID
        status = await executor.get_status(custom_id)
        assert status is not None
        assert status.workflow_id == custom_id
