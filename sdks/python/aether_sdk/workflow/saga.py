"""
Saga Pattern Implementation

Provides distributed transaction coordination with compensation
for building reliable multi-step workflows across actors.

Example:
    from aether_sdk.workflow import Saga, SagaStep, SagaExecutor
    
    # Define saga steps
    order_saga = Saga("order-processing")
        .step("reserve-inventory")
            .action(async (ctx) => inventory.reserve(ctx.input.order_id))
            .compensate(async (ctx) => inventory.release(ctx.input.order_id))
        .step("process-payment")
            .action(async (ctx) => payment.charge(ctx.input.order_id))
            .compensate(async (ctx) => payment.refund(ctx.input.order_id))
        .step("ship-order")
            .action(async (ctx) => shipping.create_shipment(ctx.input.order_id))
            .compensate(async (ctx) => shipping.cancel_shipment(ctx.input.order_id))
        .build()
    
    # Execute saga
    result = await executor.execute(order_saga, {"order_id": "123"})
"""

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime
from typing import (
    Any,
    Callable,
    Dict,
    Generic,
    List,
    Optional,
    TypeVar,
    Awaitable,
)
from abc import ABC, abstractmethod
import asyncio
import logging
import random
import uuid

from .types import (
    SagaStatus,
    StepStatus,
    SagaContext,
    SagaResult,
    SagaError,
    SagaStepFailedError,
    SagaCompensationFailedError,
    Duration,
    RetryConfig,
    RetryPolicy,
    ActionHandler,
    CompensationHandler,
)

logger = logging.getLogger(__name__)

T = TypeVar('T')
R = TypeVar('R')


@dataclass
class SagaStep(Generic[T]):
    """
    A single step in a saga.
    
    Each step has:
    - An action that executes the step's logic
    - An optional compensation that undoes the action if later steps fail
    - Retry configuration for handling transient failures
    
    Type Parameters:
        T: Type of the saga input
    """
    name: str
    action: Optional[ActionHandler[T]] = None
    compensate: Optional[CompensationHandler[T]] = None
    retry_config: Optional[RetryConfig] = None
    timeout: Optional[Duration] = None
    skip_condition: Optional[Callable[[SagaContext[T]], bool]] = None
    
    # Execution state (populated during execution)
    status: StepStatus = StepStatus.PENDING
    attempts: int = 0
    error: Optional[str] = None
    started_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None
    
    def with_action(self, action: ActionHandler[T]) -> SagaStep[T]:
        """Set the action handler."""
        self.action = action
        return self
    
    def with_compensation(self, compensate: CompensationHandler[T]) -> SagaStep[T]:
        """Set the compensation handler."""
        self.compensate = compensate
        return self
    
    def with_retry(self, config: RetryConfig) -> SagaStep[T]:
        """Set retry configuration."""
        self.retry_config = config
        return self
    
    def with_timeout(self, timeout: Duration) -> SagaStep[T]:
        """Set step timeout."""
        self.timeout = timeout
        return self
    
    def skip_if(self, condition: Callable[[SagaContext[T]], bool]) -> SagaStep[T]:
        """Set skip condition."""
        self.skip_condition = condition
        return self


class Saga(Generic[T]):
    """
    A saga definition containing ordered steps.
    
    Sagas provide distributed transaction semantics by defining
    compensating actions for each step.
    
    Example:
        saga = Saga("order-processing")
            .step("reserve-inventory")
                .with_action(reserve_inventory)
                .with_compensation(release_inventory)
            .step("process-payment")
                .with_action(process_payment)
                .with_compensation(refund_payment)
            .build()
    
    Type Parameters:
        T: Type of the saga input
    """
    
    def __init__(self, name: str):
        self.name = name
        self._steps: List[SagaStep[T]] = []
        self._current_step: Optional[SagaStep[T]] = None
        self._metadata: Dict[str, Any] = {}
    
    def step(self, name: str) -> Saga[T]:
        """
        Add a new step to the saga.
        
        Returns self for method chaining.
        """
        new_step = SagaStep[T](name=name)
        self._steps.append(new_step)
        self._current_step = new_step
        return self
    
    def action(self, handler: ActionHandler[T]) -> Saga[T]:
        """Set the action for the current step."""
        if self._current_step is None:
            raise ValueError("No step defined. Call step() first.")
        self._current_step.action = handler
        return self
    
    def compensate(self, handler: CompensationHandler[T]) -> Saga[T]:
        """Set the compensation for the current step."""
        if self._current_step is None:
            raise ValueError("No step defined. Call step() first.")
        self._current_step.compensate = handler
        return self
    
    def retry(self, config: RetryConfig) -> Saga[T]:
        """Set retry config for the current step."""
        if self._current_step is None:
            raise ValueError("No step defined. Call step() first.")
        self._current_step.retry_config = config
        return self
    
    def timeout(self, duration: Duration) -> Saga[T]:
        """Set timeout for the current step."""
        if self._current_step is None:
            raise ValueError("No step defined. Call step() first.")
        self._current_step.timeout = duration
        return self
    
    def skip_if(self, condition: Callable[[SagaContext[T]], bool]) -> Saga[T]:
        """Set skip condition for the current step."""
        if self._current_step is None:
            raise ValueError("No step defined. Call step() first.")
        self._current_step.skip_condition = condition
        return self
    
    def with_metadata(self, key: str, value: Any) -> Saga[T]:
        """Add metadata to the saga."""
        self._metadata[key] = value
        return self
    
    @property
    def steps(self) -> List[SagaStep[T]]:
        """Get all steps."""
        return list(self._steps)
    
    def get_step(self, name: str) -> Optional[SagaStep[T]]:
        """Get a step by name."""
        for step in self._steps:
            if step.name == name:
                return step
        return None


class SagaExecutor:
    """
    Executes sagas with compensation support.
    
    The executor runs saga steps in order and automatically
    compensates completed steps if a later step fails.
    
    Example:
        executor = SagaExecutor()
        result = await executor.execute(order_saga, {"order_id": "123"})
        
        if result.status == SagaStatus.COMPLETED:
            print("Order processed successfully")
        else:
            print(f"Order failed: {result.error}")
    """
    
    def __init__(
        self,
        default_retry: Optional[RetryConfig] = None,
        default_timeout: Optional[Duration] = None,
    ):
        self.default_retry = default_retry or RetryConfig()
        self.default_timeout = default_timeout or Duration.from_seconds(30)
        self._running_sagas: Dict[str, SagaContext] = {}
    
    async def execute(
        self,
        saga: Saga[T],
        input: T,
        context_id: Optional[str] = None,
    ) -> SagaResult:
        """
        Execute a saga with the given input.
        
        Args:
            saga: The saga definition
            input: Input data for the saga
            context_id: Optional ID for the execution context
        
        Returns:
            SagaResult with execution status and output
        """
        # Initialize context
        context = SagaContext[T](
            saga_id=context_id or str(uuid.uuid4()),
            input=input,
            started_at=datetime.utcnow(),
        )
        
        self._running_sagas[context.saga_id] = context
        
        try:
            # Execute all steps
            for step in saga.steps:
                await self._execute_step(step, context, saga)
            
            # All steps completed
            context.completed_at = datetime.utcnow()
            
            return SagaResult(
                saga_id=context.saga_id,
                status=SagaStatus.COMPLETED,
                output=context.state.get("output"),
                completed_steps=context.completed_steps,
                started_at=context.started_at,
                completed_at=context.completed_at,
                duration_ms=int(
                    (context.completed_at - context.started_at).total_seconds() * 1000
                ) if context.started_at and context.completed_at else None,
            )
            
        except SagaStepFailedError as e:
            logger.error(f"Saga step failed: {e.step_name}", exc_info=True)
            context.failed_step = e.step_name
            context.error = str(e.cause) if e.cause else str(e)
            
            # Compensate completed steps
            await self._compensate(saga, context)
            
            context.completed_at = datetime.utcnow()
            
            return SagaResult(
                saga_id=context.saga_id,
                status=SagaStatus.COMPENSATED if context.completed_steps else SagaStatus.FAILED,
                error=context.error,
                completed_steps=context.completed_steps,
                compensated_steps=[s for s in context.completed_steps],
                started_at=context.started_at,
                completed_at=context.completed_at,
                duration_ms=int(
                    (context.completed_at - context.started_at).total_seconds() * 1000
                ) if context.started_at and context.completed_at else None,
            )
            
        except Exception as e:
            logger.error(f"Saga execution failed: {e}", exc_info=True)
            context.error = str(e)
            context.completed_at = datetime.utcnow()
            
            return SagaResult(
                saga_id=context.saga_id,
                status=SagaStatus.FAILED,
                error=context.error,
                started_at=context.started_at,
                completed_at=context.completed_at,
            )
            
        finally:
            if context.saga_id in self._running_sagas:
                del self._running_sagas[context.saga_id]
    
    async def _execute_step(
        self,
        step: SagaStep[T],
        context: SagaContext[T],
        saga: Saga[T],
    ) -> None:
        """Execute a single step with retry logic."""
        # Check skip condition
        if step.skip_condition and step.skip_condition(context):
            logger.info(f"Skipping step '{step.name}' due to skip condition")
            step.status = StepStatus.SKIPPED
            return
        
        if step.action is None:
            logger.warning(f"Step '{step.name}' has no action, skipping")
            step.status = StepStatus.SKIPPED
            return
        
        retry_config = step.retry_config or self.default_retry
        timeout = step.timeout or self.default_timeout
        
        step.status = StepStatus.RUNNING
        step.started_at = datetime.utcnow()
        
        for attempt in range(1, retry_config.max_attempts + 1):
            try:
                step.attempts = attempt
                
                # Execute with timeout
                result = await asyncio.wait_for(
                    step.action(context),
                    timeout=timeout.total_seconds,
                )
                
                # Store result in context if any
                if result is not None:
                    context.set_state(f"step_{step.name}_result", result)
                
                step.status = StepStatus.COMPLETED
                step.completed_at = datetime.utcnow()
                context.mark_step_completed(step.name)
                
                logger.info(f"Step '{step.name}' completed successfully")
                return
                
            except asyncio.TimeoutError:
                error_msg = f"Step '{step.name}' timed out after {timeout.total_seconds}s"
                logger.warning(f"{error_msg} (attempt {attempt}/{retry_config.max_attempts})")
                step.error = error_msg
                
                if attempt < retry_config.max_attempts:
                    await self._wait_for_retry(retry_config, attempt)
                else:
                    step.status = StepStatus.FAILED
                    raise SagaStepFailedError(step.name, TimeoutError(error_msg))
                    
            except Exception as e:
                logger.warning(
                    f"Step '{step.name}' failed: {e} (attempt {attempt}/{retry_config.max_attempts})"
                )
                step.error = str(e)
                
                if attempt < retry_config.max_attempts:
                    await self._wait_for_retry(retry_config, attempt)
                else:
                    step.status = StepStatus.FAILED
                    raise SagaStepFailedError(step.name, e)
    
    async def _compensate(
        self,
        saga: Saga[T],
        context: SagaContext[T],
    ) -> None:
        """Compensate all completed steps in reverse order."""
        # Get completed steps in reverse order
        completed_steps = list(reversed(context.completed_steps))
        
        for step_name in completed_steps:
            step = saga.get_step(step_name)
            if step is None or step.compensate is None:
                logger.warning(f"No compensation for step '{step_name}'")
                continue
            
            try:
                step.status = StepStatus.COMPENSATING
                await step.compensate(context)
                step.status = StepStatus.COMPENSATED
                logger.info(f"Compensated step '{step_name}'")
                
            except Exception as e:
                step.status = StepStatus.FAILED
                logger.error(f"Compensation failed for step '{step_name}': {e}")
                raise SagaCompensationFailedError(step_name, e)
    
    async def _wait_for_retry(
        self,
        config: RetryConfig,
        attempt: int,
    ) -> None:
        """Wait before retrying based on retry policy."""
        if config.policy == RetryPolicy.NONE:
            return
        
        delay_ms = config.initial_delay.milliseconds
        
        if config.policy in (RetryPolicy.EXPONENTIAL, RetryPolicy.EXPONENTIAL_JITTER):
            delay_ms = int(delay_ms * (config.multiplier ** (attempt - 1)))
            delay_ms = min(delay_ms, config.max_delay.milliseconds)
            
            if config.policy == RetryPolicy.EXPONENTIAL_JITTER:
                jitter = delay_ms * config.jitter
                delay_ms = int(delay_ms + random.uniform(-jitter, jitter))
        
        await asyncio.sleep(delay_ms / 1000)
    
    async def get_status(self, saga_id: str) -> Optional[SagaResult]:
        """Get the status of a running saga."""
        context = self._running_sagas.get(saga_id)
        if context is None:
            return None
        
        return SagaResult(
            saga_id=context.saga_id,
            status=SagaStatus.RUNNING,
            completed_steps=context.completed_steps,
            started_at=context.started_at,
        )
    
    async def compensate(self, saga_id: str) -> None:
        """
        Manually trigger compensation for a saga.
        
        This is useful for manual intervention or external triggers.
        """
        context = self._running_sagas.get(saga_id)
        if context is None:
            raise SagaError(f"No running saga with ID {saga_id}")
        
        # This would need the original saga definition to work
        # For now, just mark as compensating
        logger.warning(f"Manual compensation requested for saga {saga_id}")


def saga(name: str) -> Saga[T]:
    """
    Decorator/function to create a saga definition.
    
    Example:
        @saga("order-processing")
        def order_saga():
            return (
                Saga("order-processing")
                .step("reserve")
                .action(reserve_inventory)
                .compensate(release_inventory)
            )
    """
    return Saga(name)


__all__ = [
    "SagaStep",
    "Saga",
    "SagaExecutor",
    "saga",
]
