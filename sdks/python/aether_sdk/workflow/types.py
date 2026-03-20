"""
Core Types for Workflow Engine

Provides foundational types for the workflow engine including
saga patterns, state machines, and human tasks.

Example:
    from aether_sdk.workflow import (
        SagaStep,
        SagaContext,
        WorkflowState,
        HumanTask,
    )
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum
from typing import (
    Any,
    Callable,
    Dict,
    Generic,
    List,
    Optional,
    TypeVar,
    Union,
)
from abc import ABC, abstractmethod
import uuid

from ..exceptions import AetherError


# ============================================
# Enums
# ============================================

class SagaStatus(Enum):
    """Status of a saga execution."""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    COMPENSATING = "compensating"
    COMPENSATED = "compensated"
    FAILED = "failed"


class StepStatus(Enum):
    """Status of an individual saga step."""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    COMPENSATING = "compensating"
    COMPENSATED = "compensated"
    FAILED = "failed"
    SKIPPED = "skipped"


class WorkflowStatus(Enum):
    """Status of a workflow execution."""
    CREATED = "created"
    RUNNING = "running"
    SUSPENDED = "suspended"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class TransitionStatus(Enum):
    """Status of a state transition."""
    PENDING = "pending"
    SUCCESS = "success"
    FAILED = "failed"
    ROLLED_BACK = "rolled_back"


class HumanTaskStatus(Enum):
    """Status of a human task."""
    PENDING = "pending"
    ASSIGNED = "assigned"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    REJECTED = "rejected"
    TIMEOUT = "timeout"
    ESCALATED = "escalated"


class RetryPolicy(Enum):
    """Retry policy for saga steps."""
    NONE = "none"
    FIXED = "fixed"
    EXPONENTIAL = "exponential"
    EXPONENTIAL_JITTER = "exponential_jitter"


# ============================================
# Value Types
# ============================================

@dataclass(frozen=True)
class Duration:
    """
    Represents a duration of time.
    
    Provides a type-safe way to work with time durations.
    """
    milliseconds: int = 0
    
    @classmethod
    def from_seconds(cls, seconds: float) -> Duration:
        """Create duration from seconds."""
        return cls(milliseconds=int(seconds * 1000))
    
    @classmethod
    def from_minutes(cls, minutes: float) -> Duration:
        """Create duration from minutes."""
        return cls(milliseconds=int(minutes * 60 * 1000))
    
    @classmethod
    def from_hours(cls, hours: float) -> Duration:
        """Create duration from hours."""
        return cls(milliseconds=int(hours * 60 * 60 * 1000))
    
    @classmethod
    def from_days(cls, days: float) -> Duration:
        """Create duration from days."""
        return cls(milliseconds=int(days * 24 * 60 * 60 * 1000))
    
    @property
    def total_seconds(self) -> float:
        """Total duration in seconds."""
        return self.milliseconds / 1000
    
    @property
    def total_minutes(self) -> float:
        """Total duration in minutes."""
        return self.milliseconds / (1000 * 60)
    
    @property
    def total_hours(self) -> float:
        """Total duration in hours."""
        return self.milliseconds / (1000 * 60 * 60)
    
    def to_timedelta(self) -> timedelta:
        """Convert to Python timedelta."""
        return timedelta(milliseconds=self.milliseconds)
    
    def __add__(self, other: Duration) -> Duration:
        return Duration(milliseconds=self.milliseconds + other.milliseconds)
    
    def __sub__(self, other: Duration) -> Duration:
        return Duration(milliseconds=max(0, self.milliseconds - other.milliseconds))


@dataclass
class RetryConfig:
    """
    Configuration for retry behavior.
    
    Attributes:
        max_attempts: Maximum number of retry attempts
        policy: Retry policy type
        initial_delay: Initial delay before first retry
        max_delay: Maximum delay between retries
        multiplier: Multiplier for exponential backoff
        jitter: Jitter factor (0.0 to 1.0)
    """
    max_attempts: int = 3
    policy: RetryPolicy = RetryPolicy.EXPONENTIAL
    initial_delay: Duration = field(default_factory=lambda: Duration.from_seconds(1))
    max_delay: Duration = field(default_factory=lambda: Duration.from_seconds(60))
    multiplier: float = 2.0
    jitter: float = 0.1


# ============================================
# Context Types
# ============================================

T = TypeVar('T')


@dataclass
class SagaContext(Generic[T]):
    """
    Context passed through saga execution.
    
    Contains input data, accumulated state, and execution metadata.
    
    Type Parameters:
        T: Type of the input data
    """
    saga_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    input: Optional[T] = None
    state: Dict[str, Any] = field(default_factory=dict)
    completed_steps: List[str] = field(default_factory=list)
    failed_step: Optional[str] = None
    error: Optional[str] = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def set_state(self, key: str, value: Any) -> None:
        """Set a state value."""
        self.state[key] = value
    
    def get_state(self, key: str, default: Any = None) -> Any:
        """Get a state value."""
        return self.state.get(key, default)
    
    def mark_step_completed(self, step_name: str) -> None:
        """Mark a step as completed."""
        if step_name not in self.completed_steps:
            self.completed_steps.append(step_name)
    
    def is_step_completed(self, step_name: str) -> bool:
        """Check if a step has been completed."""
        return step_name in self.completed_steps


@dataclass
class WorkflowContext(Generic[T]):
    """
    Context passed through workflow execution.
    
    Contains workflow state, variables, and execution history.
    
    Type Parameters:
        T: Type of the workflow input
    """
    workflow_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    workflow_type: str = ""
    current_state: str = ""
    input: Optional[T] = None
    variables: Dict[str, Any] = field(default_factory=dict)
    history: List[Dict[str, Any]] = field(default_factory=list)
    started_at: Optional[datetime] = None
    updated_at: Optional[datetime] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def set_variable(self, key: str, value: Any) -> None:
        """Set a workflow variable."""
        self.variables[key] = value
    
    def get_variable(self, key: str, default: Any = None) -> Any:
        """Get a workflow variable."""
        return self.variables.get(key, default)
    
    def add_history_event(self, event_type: str, **details: Any) -> None:
        """Add an event to the history."""
        self.history.append({
            "type": event_type,
            "timestamp": datetime.utcnow().isoformat(),
            "details": details,
        })


@dataclass
class HumanTaskContext:
    """
    Context for a human task.
    
    Contains task information, assignment, and completion status.
    """
    task_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    task_type: str = ""
    workflow_id: str = ""
    step_name: str = ""
    title: str = ""
    description: str = ""
    assignee: Optional[str] = None
    candidate_users: List[str] = field(default_factory=list)
    candidate_groups: List[str] = field(default_factory=list)
    priority: int = 5  # 1 (highest) to 10 (lowest)
    due_date: Optional[datetime] = None
    form_data: Dict[str, Any] = field(default_factory=dict)
    result: Optional[Dict[str, Any]] = None
    status: HumanTaskStatus = HumanTaskStatus.PENDING
    created_at: datetime = field(default_factory=datetime.utcnow)
    updated_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    completed_by: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)


# ============================================
# Result Types
# ============================================

@dataclass
class SagaResult(Generic[T]):
    """
    Result of a saga execution.
    
    Type Parameters:
        T: Type of the output data
    """
    saga_id: str
    status: SagaStatus
    output: Optional[T] = None
    error: Optional[str] = None
    completed_steps: List[str] = field(default_factory=list)
    compensated_steps: List[str] = field(default_factory=list)
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    duration_ms: Optional[int] = None


@dataclass
class WorkflowResult(Generic[T]):
    """
    Result of a workflow execution.
    
    Type Parameters:
        T: Type of the output data
    """
    workflow_id: str
    status: WorkflowStatus
    output: Optional[T] = None
    error: Optional[str] = None
    current_state: str = ""
    history: List[Dict[str, Any]] = field(default_factory=list)
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    duration_ms: Optional[int] = None


@dataclass
class TransitionResult:
    """Result of a state transition."""
    success: bool
    from_state: str
    to_state: str
    error: Optional[str] = None
    timestamp: datetime = field(default_factory=datetime.utcnow)


# ============================================
# Exceptions
# ============================================

class SagaError(AetherError):
    """Base exception for saga errors."""
    pass


class SagaStepFailedError(SagaError):
    """Raised when a saga step fails."""
    def __init__(self, step_name: str, cause: Optional[Exception] = None):
        self.step_name = step_name
        self.cause = cause
        super().__init__(f"Saga step '{step_name}' failed: {cause or 'unknown error'}")


class SagaCompensationFailedError(SagaError):
    """Raised when saga compensation fails."""
    def __init__(self, step_name: str, cause: Optional[Exception] = None):
        self.step_name = step_name
        self.cause = cause
        super().__init__(f"Saga compensation for '{step_name}' failed: {cause or 'unknown error'}")


class WorkflowError(AetherError):
    """Base exception for workflow errors."""
    pass


class InvalidTransitionError(WorkflowError):
    """Raised when an invalid state transition is attempted."""
    def __init__(self, from_state: str, to_state: str, workflow_id: str = ""):
        self.from_state = from_state
        self.to_state = to_state
        self.workflow_id = workflow_id
        super().__init__(f"Invalid transition from '{from_state}' to '{to_state}' in workflow {workflow_id}")


class WorkflowSuspendedError(WorkflowError):
    """Raised when attempting to execute a suspended workflow."""
    def __init__(self, workflow_id: str, reason: str = ""):
        self.workflow_id = workflow_id
        super().__init__(f"Workflow {workflow_id} is suspended: {reason}")


class HumanTaskError(AetherError):
    """Base exception for human task errors."""
    pass


class HumanTaskTimeoutError(HumanTaskError):
    """Raised when a human task times out."""
    def __init__(self, task_id: str):
        self.task_id = task_id
        super().__init__(f"Human task {task_id} timed out")


class HumanTaskNotAssignedError(HumanTaskError):
    """Raised when attempting to complete an unassigned task."""
    def __init__(self, task_id: str):
        self.task_id = task_id
        super().__init__(f"Human task {task_id} is not assigned")


# ============================================
# Handler Types
# ============================================

ActionHandler = Callable[[SagaContext[T]], Any]
CompensationHandler = Callable[[SagaContext[T]], Any]
TransitionHandler = Callable[[WorkflowContext[T]], Any]
TaskFormValidator = Callable[[Dict[str, Any]], bool]


__all__ = [
    # Enums
    "SagaStatus",
    "StepStatus",
    "WorkflowStatus",
    "TransitionStatus",
    "HumanTaskStatus",
    "RetryPolicy",
    
    # Value types
    "Duration",
    "RetryConfig",
    
    # Context types
    "SagaContext",
    "WorkflowContext",
    "HumanTaskContext",
    
    # Result types
    "SagaResult",
    "WorkflowResult",
    "TransitionResult",
    
    # Exceptions
    "SagaError",
    "SagaStepFailedError",
    "SagaCompensationFailedError",
    "WorkflowError",
    "InvalidTransitionError",
    "WorkflowSuspendedError",
    "HumanTaskError",
    "HumanTaskTimeoutError",
    "HumanTaskNotAssignedError",
    
    # Handler types
    "ActionHandler",
    "CompensationHandler",
    "TransitionHandler",
    "TaskFormValidator",
]
