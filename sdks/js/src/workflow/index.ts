/**
 * Workflow Engine Module
 * 
 * Provides workflow engine features including:
 * - Saga pattern for distributed transactions
 * - Workflow state machine with visual definitions
 * - Human task integration for approvals
 */

// Re-export all types
export * from './types';

// Re-export saga implementation
export * from './saga';

// Re-export state machine implementation
export * from './state_machine';

// Re-export human task implementation
export * from './human_task';

// Convenience re-exports
export {
    // Saga
    SagaDefinition,
    SagaStep,
    SagaExecutor,
    saga,
    
    // State Machine
    WorkflowBuilder,
    WorkflowInstance,
    WorkflowExecutorInstance,
    workflow,
    
    // Human Task
    HumanTaskManager,
    InMemoryTaskStore,
    createHumanTaskManager,
    
    // Types
    Duration,
    SagaStatus,
    StepStatus,
    WorkflowStatus,
    TransitionStatus,
    HumanTaskStatus,
    RetryPolicy,
    RetryConfig,
    SagaContext,
    WorkflowContext,
    HumanTaskContext,
    SagaResult,
    WorkflowResult,
    TransitionResult,
    State,
    Transition,
    SagaError,
    CompensationError,
    WorkflowError,
    InvalidTransitionError,
    HumanTaskError,
} from './types';
