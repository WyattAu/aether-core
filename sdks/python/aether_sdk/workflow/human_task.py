"""
Human Task Integration

Provides support for human-in-the-loop workflows with task assignment,
timeouts, and escalation.

Example:
    from aether_sdk.workflow import HumanTask, HumanTaskManager
    
    # Create a human task
    task = HumanTask(
        task_type="approval",
        title="Approve Purchase Order",
        description="Review and approve the purchase order",
        assignee="manager@company.com",
        priority=3,
        due_date=datetime.now() + timedelta(days=3),
        form_data={
            "fields": [
                {"name": "approved", "type": "boolean"},
                {"name": "comments", "type": "text"},
            ]
        }
    )
    
    # Wait for completion
    result = await task_manager.wait_for_completion(task.task_id)
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Optional,
    Generic,
    TypeVar,
)
from abc import ABC, abstractmethod
import asyncio
import logging
import uuid

from .types import (
    HumanTaskStatus,
    HumanTaskContext,
    HumanTaskError,
    HumanTaskTimeoutError,
    HumanTaskNotAssignedError,
    Duration,
    TaskFormValidator,
)

logger = logging.getLogger(__name__)

T = TypeVar('T')


@dataclass
class FormField:
    """A field in a human task form."""
    name: str
    field_type: str  # text, number, boolean, select, date, etc.
    label: Optional[str] = None
    description: Optional[str] = None
    required: bool = False
    default: Optional[Any] = None
    options: Optional[List[Dict[str, Any]]] = None  # For select fields
    validation: Optional[Dict[str, Any]] = None  # Validation rules


@dataclass
class TaskForm:
    """A form definition for a human task."""
    fields: List[FormField] = field(default_factory=list)
    
    def add_field(
        self,
        name: str,
        field_type: str,
        label: Optional[str] = None,
        required: bool = False,
        **kwargs: Any,
    ) -> TaskForm:
        """Add a field to the form."""
        self.fields.append(FormField(
            name=name,
            field_type=field_type,
            label=label or name,
            required=required,
            **kwargs,
        ))
        return self
    
    def validate(self, data: Dict[str, Any]) -> bool:
        """Validate form data."""
        for field in self.fields:
            if field.required and field.name not in data:
                return False
            
            if field.name in data:
                value = data[field.name]
                
                # Type validation
                if field.field_type == "number" and not isinstance(value, (int, float)):
                    return False
                elif field.field_type == "boolean" and not isinstance(value, bool):
                    return False
                elif field.field_type == "text" and not isinstance(value, str):
                    return False
                
                # Custom validation
                if field.validation:
                    min_val = field.validation.get("min")
                    max_val = field.validation.get("max")
                    
                    if min_val is not None and value < min_val:
                        return False
                    if max_val is not None and value > max_val:
                        return False
        
        return True
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary."""
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
    """
    A human task definition.
    
    Human tasks pause workflow execution until completed by a human.
    They support assignment, delegation, and escalation.
    """
    task_type: str
    title: str
    description: str = ""
    assignee: Optional[str] = None
    candidate_users: List[str] = field(default_factory=list)
    candidate_groups: List[str] = field(default_factory=list)
    priority: int = 5  # 1 (highest) to 10 (lowest)
    due_date: Optional[datetime] = None
    timeout: Optional[Duration] = None
    timeout_action: str = "escalate"  # escalate, complete, fail
    form: Optional[TaskForm] = None
    form_validator: Optional[TaskFormValidator] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    # Runtime state
    task_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    workflow_id: str = ""
    step_name: str = ""
    status: HumanTaskStatus = HumanTaskStatus.PENDING
    created_at: datetime = field(default_factory=datetime.utcnow)
    updated_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    completed_by: Optional[str] = None
    result: Optional[Dict[str, Any]] = None
    
    def with_assignee(self, assignee: str) -> HumanTask:
        """Set the assignee."""
        self.assignee = assignee
        return self
    
    def with_candidates(
        self,
        users: Optional[List[str]] = None,
        groups: Optional[List[str]] = None,
    ) -> HumanTask:
        """Set candidate users and groups."""
        if users:
            self.candidate_users = users
        if groups:
            self.candidate_groups = groups
        return self
    
    def with_priority(self, priority: int) -> HumanTask:
        """Set priority (1-10)."""
        self.priority = max(1, min(10, priority))
        return self
    
    def with_due_date(self, due_date: datetime) -> HumanTask:
        """Set due date."""
        self.due_date = due_date
        return self
    
    def with_timeout(
        self,
        timeout: Duration,
        action: str = "escalate",
    ) -> HumanTask:
        """Set timeout and action."""
        self.timeout = timeout
        self.timeout_action = action
        return self
    
    def with_form(self, form: TaskForm) -> HumanTask:
        """Set the task form."""
        self.form = form
        return self
    
    def is_overdue(self) -> bool:
        """Check if task is overdue."""
        if self.due_date is None:
            return False
        return datetime.utcnow() > self.due_date
    
    def is_expired(self) -> bool:
        """Check if task has timed out."""
        if self.timeout is None:
            return False
        expires_at = self.created_at + self.timeout.to_timedelta()
        return datetime.utcnow() > expires_at
    
    def to_context(self) -> HumanTaskContext:
        """Convert to context for storage."""
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
        """Create from context."""
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
            task.form = TaskForm()  # Would need to reconstruct form
        
        return task


class HumanTaskManager:
    """
    Manages human tasks in workflows.
    
    The task manager handles task lifecycle, assignment,
    completion, and escalation.
    
    Example:
        manager = HumanTaskManager()
        
        # Create and assign task
        task = await manager.create_task(human_task, workflow_id, step_name)
        
        # Claim task (for candidate users/groups)
        await manager.claim_task(task.task_id, "user@company.com")
        
        # Complete task
        await manager.complete_task(task.task_id, {"approved": True})
    """
    
    def __init__(self):
        self._tasks: Dict[str, HumanTask] = {}
        self._pending_timeouts: Dict[str, asyncio.Task] = {}
        self._completion_events: Dict[str, asyncio.Event] = {}
    
    async def create_task(
        self,
        task: HumanTask,
        workflow_id: str,
        step_name: str,
    ) -> HumanTask:
        """
        Create a new human task.
        
        Args:
            task: The task definition
            workflow_id: The workflow instance ID
            step_name: The workflow step name
        
        Returns:
            The created task with ID
        """
        task.workflow_id = workflow_id
        task.step_name = step_name
        task.status = HumanTaskStatus.PENDING
        task.created_at = datetime.utcnow()
        
        self._tasks[task.task_id] = task
        
        # Set up timeout if configured
        if task.timeout:
            self._schedule_timeout(task)
        
        logger.info(f"Created human task {task.task_id} for workflow {workflow_id}")
        
        return task
    
    async def claim_task(
        self,
        task_id: str,
        user: str,
    ) -> HumanTask:
        """
        Claim a task for a user.
        
        Args:
            task_id: The task ID
            user: The user claiming the task
        
        Returns:
            The updated task
        
        Raises:
            HumanTaskError: If task cannot be claimed
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")
        
        if task.status not in (HumanTaskStatus.PENDING, HumanTaskStatus.ASSIGNED):
            raise HumanTaskError(f"Task cannot be claimed: status is {task.status}")
        
        # Check if user can claim
        can_claim = (
            task.assignee == user or
            user in task.candidate_users or
            any(group in task.candidate_groups for group in self._get_user_groups(user))
        )
        
        if not can_claim:
            raise HumanTaskError(f"User {user} cannot claim task {task_id}")
        
        task.assignee = user
        task.status = HumanTaskStatus.IN_PROGRESS
        task.updated_at = datetime.utcnow()
        
        logger.info(f"Task {task_id} claimed by {user}")
        
        return task
    
    async def complete_task(
        self,
        task_id: str,
        result: Dict[str, Any],
        user: Optional[str] = None,
    ) -> HumanTask:
        """
        Complete a task with a result.
        
        Args:
            task_id: The task ID
            result: The task result (form data)
            user: Optional user completing the task
        
        Returns:
            The completed task
        
        Raises:
            HumanTaskError: If task cannot be completed
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")
        
        if task.status == HumanTaskStatus.COMPLETED:
            raise HumanTaskError(f"Task already completed: {task_id}")
        
        if task.status == HumanTaskStatus.TIMEOUT:
            raise HumanTaskError(f"Task has timed out: {task_id}")
        
        # Validate form data if form exists
        if task.form and not task.form.validate(result):
            raise HumanTaskError(f"Invalid form data for task {task_id}")
        
        # Custom validation
        if task.form_validator and not task.form_validator(result):
            raise HumanTaskError(f"Custom validation failed for task {task_id}")
        
        task.result = result
        task.status = HumanTaskStatus.COMPLETED
        task.completed_at = datetime.utcnow()
        task.completed_by = user or task.assignee
        task.updated_at = datetime.utcnow()
        
        # Cancel timeout if scheduled
        if task_id in self._pending_timeouts:
            self._pending_timeouts[task_id].cancel()
            del self._pending_timeouts[task_id]
        
        # Signal completion
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
        """
        Reject a task.
        
        Args:
            task_id: The task ID
            reason: Reason for rejection
            user: Optional user rejecting the task
        
        Returns:
            The rejected task
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")
        
        task.status = HumanTaskStatus.REJECTED
        task.result = {"rejected": True, "reason": reason}
        task.completed_at = datetime.utcnow()
        task.completed_by = user or task.assignee
        task.updated_at = datetime.utcnow()
        
        # Cancel timeout
        if task_id in self._pending_timeouts:
            self._pending_timeouts[task_id].cancel()
            del self._pending_timeouts[task_id]
        
        # Signal completion
        if task_id in self._completion_events:
            self._completion_events[task_id].set()
        
        logger.info(f"Task {task_id} rejected by {task.completed_by}: {reason}")
        
        return task
    
    async def escalate_task(
        self,
        task_id: str,
        escalate_to: Optional[str] = None,
    ) -> HumanTask:
        """
        Escalate a task.
        
        Args:
            task_id: The task ID
            escalate_to: Optional user or group to escalate to
        
        Returns:
            The escalated task
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")
        
        task.status = HumanTaskStatus.ESCALATED
        task.updated_at = datetime.utcnow()
        
        if escalate_to:
            # Could be a user or group
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
        """
        Delegate a task to another user.
        
        Args:
            task_id: The task ID
            delegate_to: User to delegate to
        
        Returns:
            The delegated task
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")
        
        old_assignee = task.assignee
        task.assignee = delegate_to
        task.updated_at = datetime.utcnow()
        
        logger.info(f"Task {task_id} delegated from {old_assignee} to {delegate_to}")
        
        return task
    
    async def get_task(self, task_id: str) -> Optional[HumanTask]:
        """Get a task by ID."""
        return self._tasks.get(task_id)
    
    async def get_tasks_for_user(
        self,
        user: str,
        include_completed: bool = False,
    ) -> List[HumanTask]:
        """Get all tasks for a user."""
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
        """
        Wait for a task to complete.
        
        Args:
            task_id: The task ID
            timeout: Optional timeout in seconds
        
        Returns:
            The task result
        
        Raises:
            HumanTaskTimeoutError: If timeout expires
        """
        task = self._tasks.get(task_id)
        if task is None:
            raise HumanTaskError(f"Task not found: {task_id}")
        
        if task.status == HumanTaskStatus.COMPLETED:
            return task.result or {}
        
        # Create completion event
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
        """Schedule a timeout for a task."""
        async def timeout_handler():
            try:
                await asyncio.sleep(task.timeout.total_seconds)
                task = self._tasks.get(task_id)
                if task and task.status not in (
                    HumanTaskStatus.COMPLETED,
                    HumanTaskStatus.REJECTED,
                ):
                    await self._handle_timeout(task)
            except asyncio.CancelledError:
                pass
        
        task_id = task.task_id
        self._pending_timeouts[task_id] = asyncio.create_task(timeout_handler())
    
    async def _handle_timeout(self, task: HumanTask) -> None:
        """Handle task timeout."""
        logger.warning(f"Task {task.task_id} timed out")
        
        task.status = HumanTaskStatus.TIMEOUT
        task.updated_at = datetime.utcnow()
        
        if task.timeout_action == "escalate":
            await self.escalate_task(task.task_id)
        elif task.timeout_action == "fail":
            task.result = {"failed": True, "reason": "timeout"}
            if task.task_id in self._completion_events:
                self._completion_events[task.task_id].set()
        # "complete" action would require default values
    
    def _get_user_groups(self, user: str) -> List[str]:
        """Get groups for a user (placeholder for integration)."""
        # Would integrate with identity provider
        return []


__all__ = [
    "FormField",
    "TaskForm",
    "HumanTask",
    "HumanTaskManager",
]
