"""
Tests for Workflow Engine - Human Task Module
"""

import pytest
from datetime import datetime, timedelta
from unittest.mock import AsyncMock, MagicMock, patch

from aether_sdk.workflow.human_task import (
    HumanTask,
    HumanTaskManager,
    InMemoryTaskStore,
    TaskQuery,
    TaskStore,
    create_human_task_manager,
)
from aether_sdk.workflow.types import (
    HumanTaskStatus,
    HumanTaskContext,
    HumanTaskError,
    Duration,
)


class TestHumanTaskContext:
    """Tests for HumanTaskContext."""
    
    def test_create_context(self):
        ctx = HumanTaskContext(
            task_type="approval",
            title="Approve Request",
        )
        assert ctx.task_id is not None
        assert ctx.task_type == "approval"
        assert ctx.title == "Approve Request"
        assert ctx.status == HumanTaskStatus.PENDING
        assert ctx.priority == 5
    
    def test_with_assignee(self):
        ctx = HumanTaskContext(
            task_type="review",
            title="Review Document",
        )
        ctx.assignee = "user123"
        ctx.status = HumanTaskStatus.ASSIGNED
        
        assert ctx.assignee == "user123"
        assert ctx.status == HumanTaskStatus.ASSIGNED


class TestInMemoryTaskStore:
    """Tests for InMemoryTaskStore."""
    
    @pytest.fixture
    def store(self):
        return InMemoryTaskStore()
    
    @pytest.fixture
    def sample_task(self):
        ctx = HumanTaskContext(
            task_type="approval",
            title="Test Task",
        )
        ctx.workflow_id = "wf-123"
        return ctx
    
    @pytest.mark.asyncio
    async def test_create_and_get(self, store, sample_task):
        """Test creating and retrieving a task."""
        await store.create(sample_task)
        
        retrieved = await store.get(sample_task.task_id)
        assert retrieved is not None
        assert retrieved.task_id == sample_task.task_id
    
    @pytest.mark.asyncio
    async def test_update(self, store, sample_task):
        """Test updating a task."""
        await store.create(sample_task)
        
        sample_task.status = HumanTaskStatus.IN_PROGRESS
        await store.update(sample_task)
        
        retrieved = await store.get(sample_task.task_id)
        assert retrieved.status == HumanTaskStatus.IN_PROGRESS
    
    @pytest.mark.asyncio
    async def test_delete(self, store, sample_task):
        """Test deleting a task."""
        await store.create(sample_task)
        
        await store.delete(sample_task.task_id)
        
        retrieved = await store.get(sample_task.task_id)
        assert retrieved is None
    
    @pytest.mark.asyncio
    async def test_query_by_workflow(self, store, sample_task):
        """Test querying tasks by workflow ID."""
        # Create tasks for different workflows
        task1 = HumanTaskContext(task_type="t1", title="Task 1")
        task1.workflow_id = "wf-1"
        task2 = HumanTaskContext(task_type="t2", title="Task 2")
        task2.workflow_id = "wf-2"
        task3 = HumanTaskContext(task_type="t3", title="Task 3")
        task3.workflow_id = "wf-1"
        
        await store.create(task1)
        await store.create(task2)
        await store.create(task3)
        
        results = await store.query(TaskQuery(workflow_id="wf-1"))
        assert len(results) == 2
    
    @pytest.mark.asyncio
    async def test_query_by_assignee(self, store):
        """Test querying tasks by assignee."""
        task1 = HumanTaskContext(task_type="t1", title="Task 1")
        task1.assignee = "user1"
        task1.status = HumanTaskStatus.ASSIGNED
        
        task2 = HumanTaskContext(task_type="t2", title="Task 2")
        task2.assignee = "user2"
        task2.status = HumanTaskStatus.ASSIGNED
        
        await store.create(task1)
        await store.create(task2)
        
        results = await store.query(TaskQuery(assignee="user1"))
        assert len(results) == 1
        assert results[0].assignee == "user1"
    
    @pytest.mark.asyncio
    async def test_query_by_status(self, store):
        """Test querying tasks by status."""
        task1 = HumanTaskContext(task_type="t1", title="Task 1")
        task1.status = HumanTaskStatus.PENDING
        
        task2 = HumanTaskContext(task_type="t2", title="Task 2")
        task2.status = HumanTaskStatus.COMPLETED
        
        await store.create(task1)
        await store.create(task2)
        
        results = await store.query(
            TaskQuery(status=[HumanTaskStatus.PENDING])
        )
        assert len(results) == 1
        assert results[0].status == HumanTaskStatus.PENDING


class TestHumanTaskManager:
    """Tests for HumanTaskManager."""
    
    @pytest.fixture
    def manager(self):
        return create_human_task_manager()
    
    @pytest.mark.asyncio
    async def test_create_task(self, manager):
        """Test creating a human task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve Request",
                "description": "Please review and approve",
                "priority": 1,
            }
        )
        
        assert task.task_id is not None
        assert task.task_type == "approval"
        assert task.title == "Approve Request"
        assert task.status == HumanTaskStatus.PENDING
        assert task.priority == 1
    
    @pytest.mark.asyncio
    async def test_create_task_with_assignee(self, manager):
        """Test creating a task with an assignee."""
        task = await manager.create_task(
            task_type="review",
            options={
                "title": "Review",
                "assignee": "user123",
            }
        )
        
        assert task.assignee == "user123"
        assert task.status == HumanTaskStatus.ASSIGNED
    
    @pytest.mark.asyncio
    async def test_claim_task(self, manager):
        """Test claiming a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "candidate_users": ["user1", "user2"],
            }
        )
        
        claimed = await manager.claim_task(task.task_id, "user1")
        
        assert claimed.assignee == "user1"
        assert claimed.status == HumanTaskStatus.ASSIGNED
    
    @pytest.mark.asyncio
    async def test_claim_already_assigned(self, manager):
        """Test claiming a task already assigned to someone else."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "assignee": "user1",
            }
        )
        
        with pytest.raises(HumanTaskError):
            await manager.claim_task(task.task_id, "user2")
    
    @pytest.mark.asyncio
    async def test_release_task(self, manager):
        """Test releasing a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "assignee": "user1",
            }
        )
        
        released = await manager.release_task(task.task_id)
        
        assert released.assignee is None
        assert released.status == HumanTaskStatus.PENDING
    
    @pytest.mark.asyncio
    async def test_start_task(self, manager):
        """Test starting a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "assignee": "user1",
            }
        )
        
        started = await manager.start_task(task.task_id)
        
        assert started.status == HumanTaskStatus.IN_PROGRESS
    
    @pytest.mark.asyncio
    async def test_complete_task(self, manager):
        """Test completing a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "assignee": "user1",
            }
        )
        
        result = {"approved": True, "comments": "Looks good"}
        completed = await manager.complete_task(
            task.task_id,
            result,
            "user1"
        )
        
        assert completed.status == HumanTaskStatus.COMPLETED
        assert completed.result == result
        assert completed.completed_by == "user1"
        assert completed.completed_at is not None
    
    @pytest.mark.asyncio
    async def test_reject_task(self, manager):
        """Test rejecting a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "assignee": "user1",
            }
        )
        
        rejected = await manager.reject_task(
            task.task_id,
            "Does not meet requirements",
            "user1"
        )
        
        assert rejected.status == HumanTaskStatus.REJECTED
        assert rejected.result["rejected"] is True
        assert "requirements" in rejected.result["reason"]
    
    @pytest.mark.asyncio
    async def test_escalate_task(self, manager):
        """Test escalating a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
            }
        )
        
        escalated = await manager.escalate_task(
            task.task_id,
            "No response for 24 hours"
        )
        
        assert escalated.status == HumanTaskStatus.ESCALATED
        assert "escalationReason" in escalated.metadata
    
    @pytest.mark.asyncio
    async def test_delegate_task(self, manager):
        """Test delegating a task."""
        task = await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "assignee": "user1",
            }
        )
        
        delegated = await manager.delegate_task(task.task_id, "user2")
        
        assert delegated.assignee == "user2"
    
    @pytest.mark.asyncio
    async def test_get_user_tasks(self, manager):
        """Test getting tasks assigned to a user."""
        # Create multiple tasks
        await manager.create_task(
            task_type="t1",
            options={"title": "Task 1", "assignee": "user1"}
        )
        await manager.create_task(
            task_type="t2",
            options={"title": "Task 2", "assignee": "user1"}
        )
        await manager.create_task(
            task_type="t3",
            options={"title": "Task 3", "assignee": "user2"}
        )
        
        tasks = await manager.get_user_tasks("user1")
        
        assert len(tasks) == 2
    
    @pytest.mark.asyncio
    async def test_get_available_tasks(self, manager):
        """Test getting available tasks for a user."""
        # Create task with candidates
        await manager.create_task(
            task_type="approval",
            options={
                "title": "Approve",
                "candidate_users": ["user1", "user2"],
            }
        )
        
        # Create task with candidate groups
        await manager.create_task(
            task_type="review",
            options={
                "title": "Review",
                "candidate_groups": ["managers"],
            }
        )
        
        available = await manager.get_available_tasks("user1", ["managers"])
        
        assert len(available) == 2


class TestHumanTaskValidators:
    """Tests for task form validators."""
    
    @pytest.fixture
    def manager_with_validator(self):
        manager = create_human_task_manager()
        
        # Register validator for "expense-approval" tasks
        def expense_validator(result):
            return (
                "amount" in result and
                isinstance(result["amount"], (int, float)) and
                result["amount"] > 0
            )
        
        manager.set_validator("expense-approval", expense_validator)
        return manager
    
    @pytest.mark.asyncio
    async def test_valid_result_passes(self, manager_with_validator):
        """Test that valid results pass validation."""
        task = await manager_with_validator.create_task(
            task_type="expense-approval",
            options={
                "title": "Approve Expense",
                "assignee": "user1",
            }
        )
        
        result = {"amount": 100, "approved": True}
        completed = await manager_with_validator.complete_task(
            task.task_id,
            result,
            "user1"
        )
        
        assert completed.status == HumanTaskStatus.COMPLETED
    
    @pytest.mark.asyncio
    async def test_invalid_result_fails(self, manager_with_validator):
        """Test that invalid results fail validation."""
        task = await manager_with_validator.create_task(
            task_type="expense-approval",
            options={
                "title": "Approve Expense",
                "assignee": "user1",
            }
        )
        
        result = {"approved": True}  # Missing amount
        
        with pytest.raises(HumanTaskError):
            await manager_with_validator.complete_task(
                task.task_id,
                result,
                "user1"
            )


class TestTaskCompletionHandlers:
    """Tests for task completion handlers."""
    
    @pytest.mark.asyncio
    async def test_completion_handler_called(self):
        """Test that completion handler is called."""
        manager = create_human_task_manager()
        handler_called = False
        
        async def on_complete(task, result):
            nonlocal handler_called
            handler_called = True
        
        manager.on_completion("test-type", on_complete)
        
        task = await manager.create_task(
            task_type="test-type",
            options={"title": "Test", "assignee": "user1"}
        )
        
        await manager.complete_task(task.task_id, {"done": True}, "user1")
        
        assert handler_called
