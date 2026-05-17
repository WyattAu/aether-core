"""
Human Task Integration

Provides support for human-in-the-loop workflows with task assignment,
timeouts, and escalation.

Example:
    >>> from aether_sdk.workflow.human_task import HumanTask, HumanTaskManager
    >>> task = HumanTask(task_type="approval", title="Approve PO")
    >>> manager = HumanTaskManager()
    >>> await manager.create_task(task, "wf-1", "approval-step")
"""

from __future__ import annotations

import asyncio
import logging
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, TypeVar

from .types import (
    Duration,
    HumanTaskContext,
    HumanTaskError,
    HumanTaskStatus,
    HumanTaskTimeoutError,
    TaskFormValidator,
)

logger = logging.getLogger(__name__)

T = TypeVar("T")


@dataclass
class FormField:
    """A field definition in a human task form.

    Attributes:
        name: Field name (used as the key in form data).
        field_type: Input type (``"text"``, ``"number"``, ``"boolean"``,
            ``"select"``, ``"date"``, etc.).
        label: Optional display label.
        description: Optional help text.
        required: Whether the field is mandatory.
        default: Default value if not provided.
        options: Options for ``"select"`` fields.
        validation: Optional validation rules (e.g. ``{"min": 0, "max": 100}``).
    """

    name: str
    field_type: str
    label: Optional[str] = None
    description: Optional[str] = None
    required: bool = False
    default: Optional[Any] = None
    options: Optional[List[Dict[str, Any]]] = None
    validation: Optional[Dict[str, Any]] = None


@dataclass
class TaskForm:
    """A form definition for a human task.

    Contains a list of :class:`FormField` instances and provides
    validation logic.

    Example:
        >>> form = TaskForm()
        ...     .add_field("approved", "boolean", required=True)
        ...     .add_field("comments", "text")
        >>> form.validate({"approved": True})
        True
    """

    fields: List[FormField] = field(default_factory=list)

    def add_field(
        self,
        name: str,
        field_type: str,
        label: Optional[str] = None,
        required: bool = False,
        **kwargs: Any,
    ) -> TaskForm:
        """Add a field to the form.

        Args:
            name: Field name.
            field_type: Input type.
            label: Display label (defaults to *name*).
            required: Whether the field is mandatory.
            **kwargs: Additional keyword arguments forwarded to
                :class:`FormField`.

        Returns:
            Self for method chaining.
        """
        self.fields.append(
            FormField(
                name=name,
                field_type=field_type,
                label=label or name,
                required=required,
                **kwargs,
            )
        )
        return self

    def validate(self, data: Dict[str, Any]) -> bool:
        """Validate form data against the field definitions.

        Checks required fields, type constraints, and numeric min/max.

        Args:
            data: Form data as a dict.

        Returns:
            ``True`` if the data is valid.
        """
        for fld in self.fields:
            if fld.required and fld.name not in data:
                return False

            if fld.name in data:
                value = data[fld.name]

                if fld.field_type == "number" and not isinstance(value, (int, float)):
                    return False
                elif fld.field_type == "boolean" and not isinstance(value, bool):
                    return False
                elif fld.field_type == "text" and not isinstance(value, str):
                    return False

                if fld.validation:
                    min_val = fld.validation.get("min")
                    max_val = fld.validation.get("max")

                    if min_val is not None and value < min_val:
                        return False
                    if max_val is not None and value > max_val:
                        return False

        return True

    def to_dict(self) -> Dict[str, Any]:
        """Serialize the form to a dictionary.

        Returns:
            A dict with a ``"fields"`` key containing a list of
            field descriptors.
        """
        return {
            "fields": [
                {
                    "name": f.name,
                    "type": f.field_type,
                    "label": f.label,
                    "description": f.description,
                    "required": f.required,
                    "default": f.default,
                    "options": f.options,
                    "validation": f.validation,
                }
                for f in self.fields
            ]
        }


@dataclass
class HumanTask:
    """A human task that pauses workflow execution until completed.

    Human tasks support assignment, delegation, timeouts, and
    escalation. They are typically used for approvals, reviews,
    or other manual decisions.

    Attributes:
        task_type: Logical type of the task (e.g. ``"approval"``).
        title: Human-readable title.
        description: Detailed description of the task.
        assignee: User assigned to the task.
        candidate_users: Users eligible to claim the task.
        candidate_groups: Groups eligible to claim the task.
        priority: Priority level (1 = highest, 10 = lowest).
        due_date: When the task is expected to be completed.
        timeout: Duration before automatic timeout action.
        timeout_action: Action on timeout (``"escalate"``,
            ``"complete"``, ``"fail"``).
        form: Optional :class:`TaskForm` for structured input.
        form_validator: Optional custom validation callable.
        metadata: Arbitrary key-value metadata.
        task_id: Unique task identifier (auto-generated).
        workflow_id: Parent workflow instance ID.
        step_name: Workflow step that created this task.
        status: Current task status.
        created_at: When the task was created.
        updated_at: When the task was last modified.
        completed_at: When the task was completed.
        completed_by: User who completed the task.
        result: Form data submitted on completion.

    Example:
        >>> task = HumanTask(task_type="approval", title="Approve PO")
        ...     .with_assignee("manager@co.com")
        ...     .with_priority(3)
    """

    task_type: str
    title: str
    description: str = ""
    assignee: Optional[str] = None
    candidate_users: List[str] = field(default_factory=list)
    candidate_groups: List[str] = field(default_factory=list)
    priority: int = 5
    due_date: Optional[datetime] = None
    timeout: Optional[Duration] = None
    timeout_action: str = "escalate"
    form: Optional[TaskForm] = None
    form_validator: Optional[TaskFormValidator] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    task_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    workflow_id: str = ""
    step_name: str = ""
    status: HumanTaskStatus = HumanTaskStatus.PENDING
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    completed_by: Optional[str] = None
    result: Optional[Dict[str, Any]] = None

    def with_assignee(self, assignee: str) -> HumanTask:
        """Set the assignee.

        Args:
            assignee: User identifier.

        Returns:
            Self for chaining.
        """
        self.assignee = assignee
        return self

    def with_candidates(
        self,
        users: Optional[List[str]] = None,
        groups: Optional[List[str]] = None,
    ) -> HumanTask:
        """Set candidate users and/or groups.

        Args:
            users: List of user identifiers.
            groups: List of group identifiers.

        Returns:
            Self for chaining.
        """
        if users:
            self.candidate_users = users
        if groups:
            self.candidate_groups = groups
        return self

    def with_priority(self, priority: int) -> HumanTask:
        """Set priority (clamped to 1–10).

        Args:
            priority: Priority level.

        Returns:
            Self for chaining.
        """
        self.priority = max(1, min(10, priority))
        return self

    def with_due_date(self, due_date: datetime) -> HumanTask:
        """Set the due date.

        Args:
            due_date: Due date.

        Returns:
            Self for chaining.
        """
        self.due_date = due_date
        return self

    def with_timeout(
        self,
        timeout: Duration,
        action: str = "escalate",
    ) -> HumanTask:
        """Set timeout and action.

        Args:
            timeout: Timeout duration.
            action: Action on timeout (``"escalate"``, ``"complete"``,
                ``"fail"``).

        Returns:
            Self for chaining.
        """
        self.timeout = timeout
        self.timeout_action = action
        return self

    def with_form(self, form: TaskForm) -> HumanTask:
        """Set the task form.

        Args:
            form: A :class:`TaskForm` instance.

        Returns:
            Self for chaining.
        """
        self.form = form
        return self

    def is_overdue(self) -> bool:
        """Check whether the task is past its due date.

        Returns:
            ``True`` if the due date has passed (or was never set).
        """
        if self.due_date is None:
            return False
        return datetime.now(timezone.utc) > self.due_date

    def is_expired(self) -> bool:
        """Check whether the task has timed out.

        Returns:
            ``True`` if the timeout duration has elapsed since creation.
        """
        if self.timeout is None:
            return False
        expires_at = self.created_at + self.timeout.to_timedelta()
        return datetime.now(timezone.utc) > expires_at

    def to_context(self) -> HumanTaskContext:
        """Convert to a :class:`HumanTaskContext` for storage.

        Returns:
            A :class:`HumanTaskContext` with all task fields.
        """
        return HumanTaskContext(
            task_id=self.task_id,
            task_type=self.task_type,
            workflow_id=self.workflow_id,
            step_name=self.step_name,
            title=self.title,
            description=self.description,
            assignee=self.assignee,
            candidate_users=self.candidate_users,
            candidate_groups=self.candidate_groups,
            priority=self.priority,
            due_date=self.due_date,
            form_data=self.form.to_dict() if self.form else {},
            result=self.result,
            status=self.status,
            created_at=self.created_at,
            updated_at=self.updated_at,
            completed_at=self.completed_at,
            completed_by=self.completed_by,
            metadata=self.metadata,
        )

    @classmethod
    def from_context(cls, context: HumanTaskContext) -> HumanTask:
        """Reconstruct a :class:`HumanTask` from a :class:`HumanTaskContext`.

        Args:
            context: The context to reconstruct from.

        Returns:
            A :class:`HumanTask` instance.
        """
        task = cls(
            task_type=context.task_type,
            title=context.title,
            description=context.description,
            assignee=context.assignee,
            candidate_users=context.candidate_users,
            candidate_groups=context.candidate_groups,
            priority=context.priority,
            due_date=context.due_date,
            metadata=context.metadata,
        )
        task.task_id = context.task_id
        task.workflow_id = context.workflow_id
        task.step_name = context.step_name
        task.status = context.status
        task.created_at = context.created_at
        task.updated_at = context.updated_at
        task.completed_at = context.completed_at
        task.completed_by = context.completed_by
        task.result = context.result

        if context.form_data:
            task.form = TaskForm()

        return task


class HumanTaskManager:
    """Manages the lifecycle of human tasks in workflows.

    Handles task creation, assignment, claiming, completion,
    rejection, delegation, and escalation.

    Example:
        >>> manager = HumanTaskManager()
        >>> task = await manager.create_task(ht, "wf-1", "step-1")
        >>> await manager.claim_task(task.task_id, "user@co.com")
        >>> await manager.complete_task(task.task_id, {"approved": True})
    """

    def __init__(self):
        """Initialize with empty task and event registries."""
        self._tasks: Dict[str, HumanTask] = {}
        self._pending_timeouts: Dict[str, asyncio.Task] = {}
        self._completion_events: Dict[str, asyncio.Event] = {}

    async def create_task(
        self,
        task: HumanTask,
        workflow_id: str,
        step_name: str,
    ) -> HumanTask:
        """Create and register a new human task.

        Args:
            task: The task definition.
            workflow_id: Parent workflow instance ID.
            step_name: Workflow step that created the task.

        Returns:
            The created task (with IDs and timestamps set).
        """
        task.workflow_id = workflow_id
        task.step_name = step_name
        task.status = HumanTaskStatus.PENDING
        task.created_at = datetime.now(timezone.utc)

        self._tasks[task.task_id] = task

        if task.timeout:
            self._schedule_timeout(task)

        logger.info(f"Created human task {task.task_id} for workflow {workflow_id}")

        return task

    async def claim_task(
        self,
        task_id: str,
        user: str,
    ) -> HumanTask:
        """Claim a task for a specific user.

        The user must be the assigned user or in the candidate
        users/groups.

        Args:
            task_id: The task ID.
            user: The user claiming the task.

        Returns:
            The updated task.

        Raises:
            HumanTaskError: If the task is not found, not claimable,
                or the user is not authorized.
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")

        if task.status not in (HumanTaskStatus.PENDING, HumanTaskStatus.ASSIGNED):
            raise HumanTaskError(f"Task cannot be claimed: status is {task.status}")

        can_claim = (
            task.assignee == user
            or user in task.candidate_users
            or any(
                group in task.candidate_groups for group in self._get_user_groups(user)
            )
        )

        if not can_claim:
            raise HumanTaskError(f"User {user} cannot claim task {task_id}")

        task.assignee = user
        task.status = HumanTaskStatus.IN_PROGRESS
        task.updated_at = datetime.now(timezone.utc)

        logger.info(f"Task {task_id} claimed by {user}")

        return task

    async def complete_task(
        self,
        task_id: str,
        result: Dict[str, Any],
        user: Optional[str] = None,
    ) -> HumanTask:
        """Complete a task with a result.

        Validates form data if a form is defined.

        Args:
            task_id: The task ID.
            result: The task result (form data).
            user: Optional user completing the task.

        Returns:
            The completed task.

        Raises:
            HumanTaskError: If the task is not found, already
                completed, timed out, or validation fails.
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")

        if task.status == HumanTaskStatus.COMPLETED:
            raise HumanTaskError(f"Task already completed: {task_id}")

        if task.status == HumanTaskStatus.TIMEOUT:
            raise HumanTaskError(f"Task has timed out: {task_id}")

        if task.form and not task.form.validate(result):
            raise HumanTaskError(f"Invalid form data for task {task_id}")

        if task.form_validator and not task.form_validator(result):
            raise HumanTaskError(f"Custom validation failed for task {task_id}")

        task.result = result
        task.status = HumanTaskStatus.COMPLETED
        task.completed_at = datetime.now(timezone.utc)
        task.completed_by = user or task.assignee
        task.updated_at = datetime.now(timezone.utc)

        if task_id in self._pending_timeouts:
            self._pending_timeouts[task_id].cancel()
            del self._pending_timeouts[task_id]

        if task_id in self._completion_events:
            self._completion_events[task_id].set()

        logger.info(f"Task {task_id} completed by {task.completed_by}")

        return task

    async def reject_task(
        self,
        task_id: str,
        reason: str,
        user: Optional[str] = None,
    ) -> HumanTask:
        """Reject a task.

        Args:
            task_id: The task ID.
            reason: Reason for rejection.
            user: Optional user rejecting the task.

        Returns:
            The rejected task.

        Raises:
            HumanTaskError: If the task is not found.
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")

        task.status = HumanTaskStatus.REJECTED
        task.result = {"rejected": True, "reason": reason}
        task.completed_at = datetime.now(timezone.utc)
        task.completed_by = user or task.assignee
        task.updated_at = datetime.now(timezone.utc)

        if task_id in self._pending_timeouts:
            self._pending_timeouts[task_id].cancel()
            del self._pending_timeouts[task_id]

        if task_id in self._completion_events:
            self._completion_events[task_id].set()

        logger.info(f"Task {task_id} rejected by {task.completed_by}: {reason}")

        return task

    async def escalate_task(
        self,
        task_id: str,
        escalate_to: Optional[str] = None,
    ) -> HumanTask:
        """Escalate a task to another user or group.

        Args:
            task_id: The task ID.
            escalate_to: User email or group name to escalate to.

        Returns:
            The escalated task.

        Raises:
            HumanTaskError: If the task is not found.
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")

        task.status = HumanTaskStatus.ESCALATED
        task.updated_at = datetime.now(timezone.utc)

        if escalate_to:
            if "@" in escalate_to:
                task.assignee = escalate_to
            else:
                task.candidate_groups.append(escalate_to)

        logger.warning(f"Task {task_id} escalated to {escalate_to or 'manager'}")

        return task

    async def delegate_task(
        self,
        task_id: str,
        delegate_to: str,
    ) -> HumanTask:
        """Delegate a task to another user.

        Args:
            task_id: The task ID.
            delegate_to: User to delegate to.

        Returns:
            The delegated task.

        Raises:
            HumanTaskError: If the task is not found.
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")

        old_assignee = task.assignee
        task.assignee = delegate_to
        task.updated_at = datetime.now(timezone.utc)

        logger.info(f"Task {task_id} delegated from {old_assignee} to {delegate_to}")

        return task

    async def get_task(self, task_id: str) -> Optional[HumanTask]:
        """Get a task by ID.

        Args:
            task_id: The task ID.

        Returns:
            The :class:`HumanTask`, or ``None`` if not found.
        """
        return self._tasks.get(task_id)

    async def get_tasks_for_user(
        self,
        user: str,
        include_completed: bool = False,
    ) -> List[HumanTask]:
        """Get all tasks assigned to or claimable by a user.

        Args:
            user: User identifier.
            include_completed: Whether to include completed, rejected,
                and timed-out tasks.

        Returns:
            A list of :class:`HumanTask` instances.
        """
        tasks = []
        for task in self._tasks.values():
            if task.assignee == user or user in task.candidate_users:
                if include_completed or task.status not in (
                    HumanTaskStatus.COMPLETED,
                    HumanTaskStatus.REJECTED,
                    HumanTaskStatus.TIMEOUT,
                ):
                    tasks.append(task)
        return tasks

    async def wait_for_completion(
        self,
        task_id: str,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Wait for a task to complete.

        Args:
            task_id: The task ID.
            timeout: Maximum wait time in seconds.

        Returns:
            The task result dict.

        Raises:
            HumanTaskError: If the task is not found or does not
                complete successfully.
            HumanTaskTimeoutError: If the timeout expires.
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")

        if task.status == HumanTaskStatus.COMPLETED:
            return task.result or {}

        if task_id not in self._completion_events:
            self._completion_events[task_id] = asyncio.Event()

        event = self._completion_events[task_id]

        try:
            await asyncio.wait_for(event.wait(), timeout=timeout)
        except asyncio.TimeoutError:
            raise HumanTaskTimeoutError(task_id)

        task = self._tasks.get(task_id)
        if task and task.status == HumanTaskStatus.COMPLETED:
            return task.result or {}
        else:
            raise HumanTaskError(f"Task {task_id} did not complete successfully")

    def _schedule_timeout(self, task: HumanTask) -> None:
        """Schedule an automatic timeout for a task.

        Args:
            task: The task to schedule a timeout for.
        """
        if task.timeout is None:
            return

        async def timeout_handler():
            try:
                await asyncio.sleep(task.timeout.total_seconds())
                current_task = self._tasks.get(task_id)
                if current_task and current_task.status not in (
                    HumanTaskStatus.COMPLETED,
                    HumanTaskStatus.REJECTED,
                ):
                    await self._handle_timeout(current_task)
            except asyncio.CancelledError:
                pass

        task_id = task.task_id
        self._pending_timeouts[task_id] = asyncio.create_task(timeout_handler())

    async def _handle_timeout(self, task: HumanTask) -> None:
        """Handle a task that has timed out.

        Args:
            task: The timed-out task.
        """
        logger.warning(f"Task {task.task_id} timed out")

        task.status = HumanTaskStatus.TIMEOUT
        task.updated_at = datetime.now(timezone.utc)

        if task.timeout_action == "escalate":
            await self.escalate_task(task.task_id)
        elif task.timeout_action == "fail":
            task.result = {"failed": True, "reason": "timeout"}
            if task.task_id in self._completion_events:
                self._completion_events[task.task_id].set()

    def _get_user_groups(self, user: str) -> List[str]:
        """Get groups for a user (placeholder for identity integration).

        Args:
            user: User identifier.

        Returns:
            An empty list (override or integrate with an IdP).
        """
        return []


@dataclass
class TaskQuery:
    """Query parameters for searching tasks.

    Attributes:
        workflow_id: Filter by workflow ID.
        assignee: Filter by assignee.
        candidate_user: Filter by candidate user.
        candidate_group: Filter by candidate group.
        status: Filter by status (list of statuses).
        task_type: Filter by task type.
        priority_min: Minimum priority (inclusive).
        priority_max: Maximum priority (inclusive).
        due_before: Include tasks due before this date.
        due_after: Include tasks due after this date.
        limit: Maximum number of results.
        offset: Offset for pagination.
    """

    workflow_id: Optional[str] = None
    assignee: Optional[str] = None
    candidate_user: Optional[str] = None
    candidate_group: Optional[str] = None
    status: Optional[List[HumanTaskStatus]] = None
    task_type: Optional[str] = None
    priority_min: Optional[int] = None
    priority_max: Optional[int] = None
    due_before: Optional[datetime] = None
    due_after: Optional[datetime] = None
    limit: Optional[int] = None
    offset: Optional[int] = None


class TaskStore(ABC):
    """Abstract base class for task persistence backends.

    Implementations can use in-memory storage, databases, or
    external services.
    """

    @abstractmethod
    async def create(self, task: HumanTaskContext) -> None:
        """Persist a new task.

        Args:
            task: The task context to store.
        """
        pass

    @abstractmethod
    async def get(self, task_id: str) -> Optional[HumanTaskContext]:
        """Retrieve a task by ID.

        Args:
            task_id: The task ID.

        Returns:
            The task context, or ``None`` if not found.
        """
        pass

    @abstractmethod
    async def update(self, task: HumanTaskContext) -> None:
        """Update an existing task.

        Args:
            task: The task context with updated fields.
        """
        pass

    @abstractmethod
    async def delete(self, task_id: str) -> None:
        """Delete a task.

        Args:
            task_id: The task ID.
        """
        pass

    @abstractmethod
    async def query(self, query: TaskQuery) -> List[HumanTaskContext]:
        """Query tasks based on criteria.

        Args:
            query: A :class:`TaskQuery` with filter parameters.

        Returns:
            A list of matching task contexts.
        """
        pass


class InMemoryTaskStore(TaskStore):
    """In-memory implementation of :class:`TaskStore`.

    Useful for testing and simple use cases. Data is not persisted
    across restarts.
    """

    def __init__(self):
        """Initialize with an empty task store."""
        self._tasks: Dict[str, HumanTaskContext] = {}
        self._lock = asyncio.Lock()

    async def create(self, task: HumanTaskContext) -> None:
        """Store a new task.

        Args:
            task: The task context to store.
        """
        async with self._lock:
            self._tasks[task.task_id] = task

    async def get(self, task_id: str) -> Optional[HumanTaskContext]:
        """Retrieve a task by ID.

        Args:
            task_id: The task ID.

        Returns:
            The task context, or ``None``.
        """
        return self._tasks.get(task_id)

    async def update(self, task: HumanTaskContext) -> None:
        """Update an existing task (no-op if not found).

        Args:
            task: The task context with updated fields.
        """
        async with self._lock:
            if task.task_id in self._tasks:
                self._tasks[task.task_id] = task

    async def delete(self, task_id: str) -> None:
        """Delete a task (no-op if not found).

        Args:
            task_id: The task ID.
        """
        async with self._lock:
            self._tasks.pop(task_id, None)

    async def query(self, query: TaskQuery) -> List[HumanTaskContext]:
        """Query tasks based on criteria with pagination.

        Args:
            query: A :class:`TaskQuery`.

        Returns:
            A filtered, paginated list of task contexts.
        """
        results = []

        for task in self._tasks.values():
            if query.workflow_id and task.workflow_id != query.workflow_id:
                continue

            if query.assignee and task.assignee != query.assignee:
                continue

            if (
                query.candidate_user
                and query.candidate_user not in task.candidate_users
            ):
                continue

            if (
                query.candidate_group
                and query.candidate_group not in task.candidate_groups
            ):
                continue

            if query.status and task.status not in query.status:
                continue

            if query.task_type and task.task_type != query.task_type:
                continue

            if query.priority_min is not None and task.priority < query.priority_min:
                continue
            if query.priority_max is not None and task.priority > query.priority_max:
                continue

            if query.due_before and task.due_date and task.due_date >= query.due_before:
                continue
            if query.due_after and task.due_date and task.due_date <= query.due_after:
                continue

            results.append(task)

        if query.offset:
            results = results[query.offset :]
        if query.limit:
            results = results[: query.limit]

        return results


def create_human_task_manager(
    store: Optional[TaskStore] = None,
) -> HumanTaskManager:
    """Factory function to create a :class:`HumanTaskManager`.

    Args:
        store: Reserved for future use (currently ignored).

    Returns:
        A new :class:`HumanTaskManager` instance.

    Example:
        >>> manager = create_human_task_manager()
        >>> task = await manager.create_task(
        ...     HumanTask(task_type="approval", title="Approve"),
        ...     "wf-123", "step-1",
        ... )
    """
    return HumanTaskManager()


__all__ = [
    "FormField",
    "TaskForm",
    "HumanTask",
    "HumanTaskManager",
    "TaskQuery",
    "TaskStore",
    "InMemoryTaskStore",
    "create_human_task_manager",
]
