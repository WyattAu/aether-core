"""
Workflow State Machine

Provides visual workflow definitions with state transitions
for building long-running processes.

Example:
    >>> from aether_sdk.workflow.state_machine import Workflow, State, Transition, WorkflowExecutor
    >>> workflow = Workflow("approval-flow")
    ...     .state("draft")
    ...     .state("approved", is_final=True)
    ...     .transition("submit", "draft", "approved")
    ...     .build()
    >>> executor = WorkflowExecutor()
    >>> result = await executor.start(workflow, {"request_id": "123"})
"""

from __future__ import annotations

import logging
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable, Dict, Generic, List, Optional, Set, TypeVar

from .types import (
    Duration,
    InvalidTransitionError,
    TransitionHandler,
    TransitionResult,
    WorkflowContext,
    WorkflowError,
    WorkflowResult,
    WorkflowStatus,
    WorkflowSuspendedError,
)

logger = logging.getLogger(__name__)

T = TypeVar("T")


@dataclass
class State:
    """A state in the workflow state machine.

    Attributes:
        name: State name.
        is_initial: Whether this is the starting state.
        is_final: Whether this is a terminal state.
        on_enter: Action executed when entering this state.
        on_exit: Action executed when leaving this state.
        timeout: Optional timeout for this state.
        timeout_transition: Transition to fire on timeout.
        metadata: Arbitrary key-value metadata.
    """

    name: str
    is_initial: bool = False
    is_final: bool = False
    on_enter: Optional[TransitionHandler] = None
    on_exit: Optional[TransitionHandler] = None
    timeout: Optional[Duration] = None
    timeout_transition: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)


@dataclass
class Transition:
    """A transition between two states.

    Attributes:
        name: Transition name.
        from_state: Source state name.
        to_state: Target state name.
        guard: Optional predicate that must return ``True`` for the
            transition to proceed.
        action: Optional action executed during the transition.
        metadata: Arbitrary key-value metadata.
    """

    name: str
    from_state: str
    to_state: str
    guard: Optional[Callable[[WorkflowContext], bool]] = None
    action: Optional[TransitionHandler] = None
    metadata: Dict[str, Any] = field(default_factory=dict)


class Workflow(Generic[T]):
    """A workflow definition as a state machine.

    Defines states and transitions between them. Supports method
    chaining for fluent API construction.

    Example:
        >>> wf = Workflow("order-workflow")
        ...     .state("created", is_initial=True)
        ...     .state("shipped", is_final=True)
        ...     .transition("ship", "created", "shipped")
        ...     .on_enter("shipped", notify_shipped)
        ...     .build()

    Type Parameters:
        T: Type of the workflow input.
    """

    def __init__(self, name: str):
        """Initialize the workflow.

        Args:
            name: Workflow name.
        """
        self.name = name
        self._states: Dict[str, State] = {}
        self._transitions: Dict[str, List[Transition]] = {}
        self._initial_state: Optional[str] = None
        self._final_states: Set[str] = set()
        self._current_state: Optional[State] = None
        self._metadata: Dict[str, Any] = {}

    def state(
        self,
        name: str,
        is_initial: bool = False,
        is_final: bool = False,
        timeout: Optional[Duration] = None,
        timeout_transition: Optional[str] = None,
    ) -> Workflow[T]:
        """Add a state to the workflow.

        Args:
            name: State name.
            is_initial: Whether this is the initial state.
            is_final: Whether this is a terminal state.
            timeout: Optional timeout for this state.
            timeout_transition: Transition to take on timeout.

        Returns:
            Self for method chaining.

        Raises:
            WorkflowError: If multiple initial states are defined.
        """
        state = State(
            name=name,
            is_initial=is_initial,
            is_final=is_final,
            timeout=timeout,
            timeout_transition=timeout_transition,
        )
        self._states[name] = state

        if is_initial:
            if self._initial_state is not None:
                raise WorkflowError(
                    f"Multiple initial states: {self._initial_state} and {name}"
                )
            self._initial_state = name

        if is_final:
            self._final_states.add(name)

        self._current_state = state
        return self

    def on_enter(self, state_name: str, handler: TransitionHandler) -> Workflow[T]:
        """Set the on-enter handler for a state.

        Args:
            state_name: Name of an existing state.
            handler: Async or sync callable.

        Returns:
            Self for method chaining.

        Raises:
            WorkflowError: If the state name is not recognized.
        """
        if state_name not in self._states:
            raise WorkflowError(f"Unknown state: {state_name}")
        self._states[state_name].on_enter = handler
        return self

    def on_exit(self, state_name: str, handler: TransitionHandler) -> Workflow[T]:
        """Set the on-exit handler for a state.

        Args:
            state_name: Name of an existing state.
            handler: Async or sync callable.

        Returns:
            Self for method chaining.

        Raises:
            WorkflowError: If the state name is not recognized.
        """
        if state_name not in self._states:
            raise WorkflowError(f"Unknown state: {state_name}")
        self._states[state_name].on_exit = handler
        return self

    def transition(
        self,
        name: str,
        from_state: str,
        to_state: str,
        guard: Optional[Callable[[WorkflowContext], bool]] = None,
    ) -> Workflow[T]:
        """Add a transition between states.

        Args:
            name: Transition name.
            from_state: Source state name.
            to_state: Target state name.
            guard: Optional guard predicate.

        Returns:
            Self for method chaining.

        Raises:
            WorkflowError: If either state is not recognized.
        """
        if from_state not in self._states:
            raise WorkflowError(f"Unknown source state: {from_state}")
        if to_state not in self._states:
            raise WorkflowError(f"Unknown target state: {to_state}")

        transition = Transition(
            name=name,
            from_state=from_state,
            to_state=to_state,
            guard=guard,
        )

        if from_state not in self._transitions:
            self._transitions[from_state] = []
        self._transitions[from_state].append(transition)

        return self

    def with_action(
        self,
        transition_name: str,
        action: TransitionHandler,
    ) -> Workflow[T]:
        """Set the action for a named transition.

        Args:
            transition_name: Name of an existing transition.
            action: Async or sync callable.

        Returns:
            Self for method chaining.

        Raises:
            WorkflowError: If the transition name is not found.
        """
        for transitions in self._transitions.values():
            for t in transitions:
                if t.name == transition_name:
                    t.action = action
                    return self
        raise WorkflowError(f"Unknown transition: {transition_name}")

    def with_metadata(self, key: str, value: Any) -> Workflow[T]:
        """Add metadata to the workflow.

        Args:
            key: Metadata key.
            value: Metadata value.

        Returns:
            Self for method chaining.
        """
        self._metadata[key] = value
        return self

    def build(self) -> Workflow[T]:
        """Validate and return the workflow.

        Returns:
            The validated workflow instance.

        Raises:
            WorkflowError: If no initial state is defined.
        """
        if self._initial_state is None:
            raise WorkflowError("No initial state defined")

        return self

    @property
    def states(self) -> Dict[str, State]:
        """Return a copy of all states.

        Returns:
            A dict mapping state names to :class:`State` objects.
        """
        return dict(self._states)

    @property
    def initial_state(self) -> str:
        """Return the initial state name.

        Returns:
            The name of the initial state.

        Raises:
            WorkflowError: If no initial state is defined.
        """
        if self._initial_state is None:
            raise WorkflowError("No initial state defined")
        return self._initial_state

    def is_final_state(self, state_name: str) -> bool:
        """Check whether a state is a final (terminal) state.

        Args:
            state_name: State name.

        Returns:
            ``True`` if the state is final.
        """
        return state_name in self._final_states

    def get_transitions(self, from_state: str) -> List[Transition]:
        """Return all transitions from a given state.

        Args:
            from_state: Source state name.

        Returns:
            A list of :class:`Transition` objects.
        """
        return self._transitions.get(from_state, [])

    def get_transition(self, from_state: str, name: str) -> Optional[Transition]:
        """Look up a specific transition by name from a state.

        Args:
            from_state: Source state name.
            name: Transition name.

        Returns:
            The matching :class:`Transition`, or ``None``.
        """
        for t in self._transitions.get(from_state, []):
            if t.name == name:
                return t
        return None

    def validate_transition(
        self,
        from_state: str,
        transition_name: str,
        context: WorkflowContext,
    ) -> Optional[Transition]:
        """Validate that a transition is allowed.

        Checks that the transition exists and its guard (if any)
        returns ``True``.

        Args:
            from_state: Current state name.
            transition_name: Name of the transition to validate.
            context: The workflow context (passed to guard predicates).

        Returns:
            The :class:`Transition` if valid, ``None`` otherwise.
        """
        transition = self.get_transition(from_state, transition_name)
        if transition is None:
            return None

        if transition.guard and not transition.guard(context):
            return None

        return transition


class WorkflowExecutor:
    """Executes workflow state machines.

    Manages workflow lifecycle, state transitions, and persistence.

    Example:
        >>> executor = WorkflowExecutor()
        >>> result = await executor.start(workflow, {"order_id": "123"})
        >>> result = await executor.transition(result.workflow_id, "ship")
        >>> status = await executor.get_status(result.workflow_id)
    """

    def __init__(self):
        """Initialize with empty workflow registries."""
        self._workflows: Dict[str, WorkflowContext] = {}
        self._definitions: Dict[str, Workflow] = {}

    async def start(
        self,
        workflow: Workflow[T],
        input: T,
        workflow_id: Optional[str] = None,
    ) -> WorkflowResult:
        """Start a new workflow execution.

        Args:
            workflow: The workflow definition.
            input: Input data for the workflow.
            workflow_id: Optional explicit ID.

        Returns:
            A :class:`WorkflowResult` with the initial status.
        """
        wf_id = workflow_id or str(uuid.uuid4())

        context = WorkflowContext[T](
            workflow_id=wf_id,
            workflow_type=workflow.name,
            current_state=workflow.initial_state,
            input=input,
            started_at=datetime.now(timezone.utc),
            updated_at=datetime.now(timezone.utc),
        )

        self._workflows[wf_id] = context
        self._definitions[wf_id] = workflow

        initial_state = workflow._states.get(workflow.initial_state)
        if initial_state and initial_state.on_enter:
            try:
                await initial_state.on_enter(context)
            except Exception as e:
                logger.error(f"Failed to execute on-enter for initial state: {e}")

        context.add_history_event(
            "workflow_started",
            initial_state=workflow.initial_state,
        )

        return WorkflowResult(
            workflow_id=wf_id,
            status=WorkflowStatus.RUNNING,
            current_state=context.current_state,
            started_at=context.started_at,
        )

    async def transition(
        self,
        workflow_id: str,
        transition_name: str,
        payload: Optional[Dict[str, Any]] = None,
    ) -> TransitionResult:
        """Execute a state transition on a running workflow.

        Args:
            workflow_id: The workflow instance ID.
            transition_name: The transition to execute.
            payload: Optional payload for the transition.

        Returns:
            A :class:`TransitionResult` with the outcome.

        Raises:
            WorkflowError: If the workflow or definition is not found.
            WorkflowSuspendedError: If the workflow is suspended.
            InvalidTransitionError: If the transition is not valid.
        """
        context = self._workflows.get(workflow_id)
        if context is None:
            raise WorkflowError(f"Unknown workflow: {workflow_id}")

        workflow = self._definitions.get(workflow_id)
        if workflow is None:
            raise WorkflowError(f"Unknown workflow definition: {workflow_id}")

        if context.status == WorkflowStatus.SUSPENDED:
            raise WorkflowSuspendedError(workflow_id)

        from_state = context.current_state

        transition = workflow.validate_transition(from_state, transition_name, context)
        if transition is None:
            raise InvalidTransitionError(from_state, transition_name, workflow_id)

        to_state = transition.to_state

        try:
            current_state_def = workflow._states.get(from_state)
            if current_state_def and current_state_def.on_exit:
                await current_state_def.on_exit(context)

            if transition.action:
                await transition.action(context)

            context.current_state = to_state
            context.updated_at = datetime.now(timezone.utc)

            new_state_def = workflow._states.get(to_state)
            if new_state_def and new_state_def.on_enter:
                await new_state_def.on_enter(context)

            context.add_history_event(
                "transition",
                transition=transition_name,
                from_state=from_state,
                to_state=to_state,
            )

            return TransitionResult(
                success=True,
                from_state=from_state,
                to_state=to_state,
            )

        except Exception as e:
            logger.error(f"Transition failed: {e}", exc_info=True)
            context.add_history_event(
                "transition_failed",
                transition=transition_name,
                from_state=from_state,
                error=str(e),
            )

            return TransitionResult(
                success=False,
                from_state=from_state,
                to_state=to_state,
                error=str(e),
            )

    async def suspend(self, workflow_id: str, reason: str = "") -> None:
        """Suspend a running workflow.

        Args:
            workflow_id: The workflow instance ID.
            reason: Optional reason for suspension.

        Raises:
            WorkflowError: If the workflow is not found.
        """
        context = self._workflows.get(workflow_id)
        if context is None:
            raise WorkflowError(f"Unknown workflow: {workflow_id}")

        context.status = WorkflowStatus.SUSPENDED
        context.updated_at = datetime.now(timezone.utc)
        context.add_history_event("suspended", reason=reason)

    async def resume(self, workflow_id: str) -> None:
        """Resume a suspended workflow.

        Args:
            workflow_id: The workflow instance ID.

        Raises:
            WorkflowError: If the workflow is not found or not suspended.
        """
        context = self._workflows.get(workflow_id)
        if context is None:
            raise WorkflowError(f"Unknown workflow: {workflow_id}")

        if context.status != WorkflowStatus.SUSPENDED:
            raise WorkflowError(f"Workflow {workflow_id} is not suspended")

        context.status = WorkflowStatus.RUNNING
        context.updated_at = datetime.now(timezone.utc)
        context.add_history_event("resumed")

    async def cancel(self, workflow_id: str, reason: str = "") -> None:
        """Cancel a running workflow.

        Args:
            workflow_id: The workflow instance ID.
            reason: Optional reason for cancellation.

        Raises:
            WorkflowError: If the workflow is not found.
        """
        context = self._workflows.get(workflow_id)
        if context is None:
            raise WorkflowError(f"Unknown workflow: {workflow_id}")

        context.status = WorkflowStatus.CANCELLED
        context.updated_at = datetime.now(timezone.utc)
        context.add_history_event("cancelled", reason=reason)

    async def get_status(self, workflow_id: str) -> Optional[WorkflowResult]:
        """Get the current status of a workflow.

        Args:
            workflow_id: The workflow instance ID.

        Returns:
            A :class:`WorkflowResult`, or ``None`` if not found.
        """
        context = self._workflows.get(workflow_id)
        if context is None:
            return None

        workflow = self._definitions.get(workflow_id)
        is_final = workflow.is_final_state(context.current_state) if workflow else False

        status = context.status
        if status == WorkflowStatus.RUNNING and is_final:
            status = WorkflowStatus.COMPLETED

        return WorkflowResult(
            workflow_id=workflow_id,
            status=status,
            current_state=context.current_state,
            output=context.variables,
            history=context.history,
            started_at=context.started_at,
            updated_at=context.updated_at,
        )

    def get_available_transitions(self, workflow_id: str) -> List[str]:
        """Return the names of transitions available from the current state.

        Transitions whose guards return ``False`` are excluded.

        Args:
            workflow_id: The workflow instance ID.

        Returns:
            A list of transition names.
        """
        context = self._workflows.get(workflow_id)
        if context is None:
            return []

        workflow = self._definitions.get(workflow_id)
        if workflow is None:
            return []

        transitions = workflow.get_transitions(context.current_state)
        return [t.name for t in transitions if t.guard is None or t.guard(context)]


def workflow(name: str) -> Workflow:
    """Factory function to create a new workflow.

    Args:
        name: Workflow name.

    Returns:
        A new :class:`Workflow` instance.

    Example:
        >>> wf = workflow("approval-flow")
        ...     .state("draft", is_initial=True)
        ...     .state("approved", is_final=True)
        ...     .transition("submit", "draft", "approved")
        ...     .build()
    """
    return Workflow(name)


__all__ = [
    "State",
    "Transition",
    "Workflow",
    "WorkflowExecutor",
    "workflow",
]
