/**
 * Workflow Module.
 *
 * Re-exports all public types and classes for the workflow engine,
 * including saga patterns, state machines, and human tasks.
 *
 * @module aether/workflow
 */

// Types
export {
  Duration,
  SagaStepStatus,
  SagaStatus,
  WorkflowStatus,
  HumanTaskStatus,
  StateMachineEventType,
  RetryPolicy,
  defaultRetryConfig,
} from './types';

export type {
  RetryConfig,
  SagaContext,
  WorkflowContext,
  HumanTaskContext,
  TaskFormField,
  TaskForm,
  SagaStepResult,
  SagaResult,
  WorkflowResult,
  TransitionResult,
  CompensationResult,
  StateTransition,
  StateDefinition,
  ActionHandler,
  CompensationHandler,
  TransitionHandler,
  TaskFormValidator,
} from './types';

export {
  SagaError,
  SagaStepFailedError,
  SagaCompensationFailedError,
  WorkflowError,
  InvalidTransitionError,
  WorkflowSuspendedError,
  HumanTaskError,
  HumanTaskTimeoutError,
  HumanTaskNotAssignedError,
} from './types';

// Saga
export { SagaStep, SagaBuilder, SagaExecutor, saga } from './saga';

// State Machine
export {
  StateDefinition as StateDefinitionClass,
  StateTransition as StateTransitionClass,
  StateMachineBuilder,
  StateMachineExecutor,
  stateMachine,
} from './state_machine';

export type { StateHistoryEntry } from './state_machine';

// Human Task
export {
  TaskFormField as TaskFormFieldClass,
  TaskForm as TaskFormClass,
  HumanTask,
  HumanTaskManager,
  createHumanTaskManager,
} from './human_task';
