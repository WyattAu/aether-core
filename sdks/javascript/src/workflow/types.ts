/**
 * Core Types for the Workflow Engine.
 *
 * Provides foundational types for the workflow engine including
 * saga patterns, state machines, and human tasks.
 *
 * @module aether/workflow/types
 */

import { AetherError } from '../errors';

// ============================================
// Enums
// ============================================

/**
 * Status of a saga step execution.
 */
export enum SagaStepStatus {
  Pending = 'pending',
  Running = 'running',
  Completed = 'completed',
  Compensating = 'compensating',
  Compensated = 'compensated',
  Failed = 'failed',
  Skipped = 'skipped',
}

/**
 * Status of a saga execution.
 */
export enum SagaStatus {
  Pending = 'pending',
  Running = 'running',
  Completed = 'completed',
  Compensating = 'compensating',
  Compensated = 'compensated',
  Failed = 'failed',
}

/**
 * Status of a workflow execution.
 */
export enum WorkflowStatus {
  Created = 'created',
  Running = 'running',
  Suspended = 'suspended',
  Completed = 'completed',
  Failed = 'failed',
  Cancelled = 'cancelled',
}

/**
 * Status of a human task.
 */
export enum HumanTaskStatus {
  Pending = 'pending',
  Assigned = 'assigned',
  InProgress = 'in-progress',
  Completed = 'completed',
  Rejected = 'rejected',
  Timeout = 'timeout',
  Escalated = 'escalated',
}

/**
 * Event types for state machine transitions.
 */
export enum StateMachineEventType {
  Enter = 'enter',
  Exit = 'exit',
  Transition = 'transition',
  GuardFailed = 'guard-failed',
  ActionFailed = 'action-failed',
}

/**
 * Retry policy for saga steps.
 */
export enum RetryPolicy {
  None = 'none',
  Fixed = 'fixed',
  Exponential = 'exponential',
  ExponentialJitter = 'exponential-jitter',
}

// ============================================
// Value Types
// ============================================

/**
 * Represents a duration of time.
 *
 * Provides a type-safe way to work with time durations internally
 * stored as milliseconds.
 *
 * @example
 * ```typescript
 * const d = Duration.seconds(30);
 * console.log(d.toSeconds()); // 30
 * ```
 */
export class Duration {
  private constructor(private readonly milliseconds: number) {}

  /** Create a duration from milliseconds. */
  static milliseconds(ms: number): Duration {
    return new Duration(Math.round(ms));
  }

  /** Create a duration from seconds. */
  static seconds(s: number): Duration {
    return new Duration(Math.round(s * 1000));
  }

  /** Create a duration from minutes. */
  static minutes(m: number): Duration {
    return new Duration(Math.round(m * 60 * 1000));
  }

  /** Create a duration from hours. */
  static hours(h: number): Duration {
    return new Duration(Math.round(h * 60 * 60 * 1000));
  }

  /** Create a duration from days. */
  static days(d: number): Duration {
    return new Duration(Math.round(d * 24 * 60 * 60 * 1000));
  }

  /** Total duration in milliseconds. */
  toMilliseconds(): number {
    return this.milliseconds;
  }

  /** Total duration in seconds. */
  toSeconds(): number {
    return this.milliseconds / 1000;
  }

  /** Total duration in minutes. */
  toMinutes(): number {
    return this.milliseconds / (1000 * 60);
  }

  /** Total duration in hours. */
  toHours(): number {
    return this.milliseconds / (1000 * 60 * 60);
  }

  /** Add two durations. */
  add(other: Duration): Duration {
    return new Duration(this.milliseconds + other.milliseconds);
  }

  /** Subtract two durations (clamped to zero). */
  subtract(other: Duration): Duration {
    return new Duration(Math.max(0, this.milliseconds - other.milliseconds));
  }
}

/**
 * Configuration for retry behavior on saga steps.
 */
export interface RetryConfig {
  /** Maximum number of retry attempts (default: 3). */
  maxAttempts: number;
  /** Retry policy type (default: Exponential). */
  policy: RetryPolicy;
  /** Initial delay before first retry (default: 1 second). */
  initialDelay: Duration;
  /** Maximum delay between retries (default: 60 seconds). */
  maxDelay: Duration;
  /** Multiplier for exponential backoff (default: 2.0). */
  multiplier: number;
  /** Jitter factor between 0.0 and 1.0 (default: 0.1). */
  jitter: number;
}

/**
 * Default retry configuration.
 */
export function defaultRetryConfig(): RetryConfig {
  return {
    maxAttempts: 3,
    policy: RetryPolicy.Exponential,
    initialDelay: Duration.seconds(1),
    maxDelay: Duration.seconds(60),
    multiplier: 2.0,
    jitter: 0.1,
  };
}

// ============================================
// Context Types
// ============================================

/**
 * Context passed through saga execution.
 *
 * Contains input data, accumulated state, and execution metadata.
 *
 * @typeParam T - Type of the saga input data.
 */
export class SagaContext<T = unknown> {
  readonly sagaId: string;
  input?: T;
  readonly state: Map<string, unknown> = new Map();
  readonly completedSteps: string[] = [];
  failedStep?: string;
  error?: string;
  startedAt?: Date;
  completedAt?: Date;
  readonly metadata: Record<string, unknown> = {};

  constructor(sagaId?: string) {
    this.sagaId = sagaId ?? crypto.randomUUID();
  }

  /** Set a state value. */
  setState(key: string, value: unknown): void {
    this.state.set(key, value);
  }

  /** Get a state value, or `defaultValue` if not found. */
  getState(key: string, defaultValue?: unknown): unknown {
    return this.state.has(key) ? this.state.get(key) : defaultValue;
  }

  /** Mark a step as completed. */
  markStepCompleted(stepName: string): void {
    if (!this.completedSteps.includes(stepName)) {
      this.completedSteps.push(stepName);
    }
  }

  /** Check if a step has been completed. */
  isStepCompleted(stepName: string): boolean {
    return this.completedSteps.includes(stepName);
  }
}

/**
 * Context passed through workflow execution.
 *
 * Contains workflow state, variables, and execution history.
 *
 * @typeParam T - Type of the workflow input.
 */
export class WorkflowContext<T = unknown> {
  readonly workflowId: string;
  workflowType: string = '';
  currentState: string = '';
  status: WorkflowStatus = WorkflowStatus.Running;
  input?: T;
  readonly variables: Map<string, unknown> = new Map();
  readonly history: Array<Record<string, unknown>> = [];
  startedAt?: Date;
  updatedAt?: Date;
  readonly metadata: Record<string, unknown> = {};

  constructor(workflowId?: string) {
    this.workflowId = workflowId ?? crypto.randomUUID();
  }

  /** Set a workflow variable. */
  setVariable(key: string, value: unknown): void {
    this.variables.set(key, value);
  }

  /** Get a workflow variable, or `defaultValue` if not found. */
  getVariable(key: string, defaultValue?: unknown): unknown {
    return this.variables.has(key) ? this.variables.get(key) : defaultValue;
  }

  /** Add an event to the execution history. */
  addHistoryEvent(eventType: string, details?: Record<string, unknown>): void {
    this.history.push({
      type: eventType,
      timestamp: new Date().toISOString(),
      ...details,
    });
  }
}

/**
 * Context for a human task.
 */
export interface HumanTaskContext {
  taskId: string;
  taskType: string;
  workflowId: string;
  stepName: string;
  title: string;
  description: string;
  assignee?: string;
  candidateUsers: string[];
  candidateGroups: string[];
  priority: number;
  dueDate?: Date;
  formData: Record<string, unknown>;
  result?: Record<string, unknown>;
  status: HumanTaskStatus;
  createdAt: Date;
  updatedAt?: Date;
  completedAt?: Date;
  completedBy?: string;
  metadata: Record<string, unknown>;
}

// ============================================
// Form Types
// ============================================

/**
 * A single field definition in a human task form.
 */
export interface TaskFormField {
  /** Field name (used as the key in form data). */
  name: string;
  /** Input type (`"text"`, `"number"`, `"boolean"`, `"select"`, `"date"`, etc.). */
  fieldType: string;
  /** Optional display label. */
  label?: string;
  /** Optional help text. */
  description?: string;
  /** Whether the field is mandatory. */
  required?: boolean;
  /** Default value if not provided. */
  default?: unknown;
  /** Options for `"select"` fields. */
  options?: Array<Record<string, unknown>>;
  /** Validation rules (e.g. `{ min: 0, max: 100 }`). */
  validation?: Record<string, unknown>;
}

/**
 * A form definition for a human task.
 */
export interface TaskForm {
  /** Ordered list of field definitions. */
  fields: TaskFormField[];
}

// ============================================
// Result Types
// ============================================

/**
 * Result of a single saga step execution.
 *
 * @typeParam T - Type of the step result data.
 */
export interface SagaStepResult<T = unknown> {
  /** Step name. */
  stepName: string;
  /** Execution status. */
  status: SagaStepStatus;
  /** Step result value. */
  result?: T;
  /** Error message if the step failed. */
  error?: string;
  /** Number of attempts made. */
  attempts: number;
  /** When the step started executing. */
  startedAt?: Date;
  /** When the step finished (or failed). */
  completedAt?: Date;
}

/**
 * Result of a saga execution.
 *
 * @typeParam T - Type of the output data.
 */
export interface SagaResult<T = unknown> {
  /** Unique saga execution identifier. */
  sagaId: string;
  /** Final saga status. */
  status: SagaStatus;
  /** Output data if the saga completed successfully. */
  output?: T;
  /** Error message if the saga failed. */
  error?: string;
  /** Names of steps that completed before failure. */
  completedSteps: string[];
  /** Names of steps that were compensated. */
  compensatedSteps: string[];
  /** When the saga started. */
  startedAt?: Date;
  /** When the saga finished. */
  completedAt?: Date;
  /** Total execution time in milliseconds. */
  durationMs?: number;
}

/**
 * Result of a workflow execution.
 *
 * @typeParam T - Type of the output data.
 */
export interface WorkflowResult<T = unknown> {
  workflowId: string;
  status: WorkflowStatus;
  output?: T;
  error?: string;
  currentState: string;
  history: Array<Record<string, unknown>>;
  startedAt?: Date;
  updatedAt?: Date;
  completedAt?: Date;
  durationMs?: number;
}

/**
 * Result of a state transition.
 */
export interface TransitionResult {
  /** Whether the transition succeeded. */
  success: boolean;
  /** Source state name. */
  fromState: string;
  /** Target state name. */
  toState: string;
  /** Error message if the transition failed. */
  error?: string;
  /** Timestamp of the transition attempt. */
  timestamp: Date;
}

/**
 * Result of a compensation action.
 */
export interface CompensationResult {
  /** Step name that was compensated. */
  stepName: string;
  /** Whether compensation succeeded. */
  success: boolean;
  /** Error message if compensation failed. */
  error?: string;
}

// ============================================
// State Machine Types
// ============================================

/**
 * A transition definition between two states.
 */
export interface StateTransition {
  /** Transition name / event trigger. */
  event: string;
  /** Source state name. */
  from: string;
  /** Target state name. */
  to: string;
  /** Optional guard predicate — transition is blocked if it returns `false`. */
  guard?: (context: WorkflowContext) => boolean;
  /** Optional action executed during the transition. */
  action?: (context: WorkflowContext) => void | Promise<void>;
}

/**
 * A state definition in a state machine.
 */
export interface StateDefinition {
  /** State name. */
  name: string;
  /** Whether this is the initial (starting) state. */
  isInitial?: boolean;
  /** Whether this is a terminal (final) state. */
  isFinal?: boolean;
  /** Optional action executed when entering this state. */
  onEnter?: (context: WorkflowContext) => void | Promise<void>;
  /** Optional action executed when leaving this state. */
  onExit?: (context: WorkflowContext) => void | Promise<void>;
}

// ============================================
// Handler Types
// ============================================

/**
 * Action handler for a saga step.
 *
 * @typeParam T - Type of the saga input.
 */
export type ActionHandler<T = unknown> = (context: SagaContext<T>) => unknown;

/**
 * Compensation handler for a saga step.
 *
 * @typeParam T - Type of the saga input.
 */
export type CompensationHandler<T = unknown> = (context: SagaContext<T>) => unknown;

/**
 * Transition handler for state machine transitions.
 */
export type TransitionHandler = (context: WorkflowContext) => void | Promise<void>;

/**
 * Form data validator callable.
 */
export type TaskFormValidator = (data: Record<string, unknown>) => boolean;

// ============================================
// Exceptions
// ============================================

/**
 * Base exception for saga errors.
 */
export class SagaError extends AetherError {
  constructor(message: string) {
    super(message);
    this.name = 'SagaError';
  }
}

/**
 * Raised when a saga step fails after all retries.
 */
export class SagaStepFailedError extends SagaError {
  constructor(
    public readonly stepName: string,
    public readonly cause?: Error
  ) {
    super(`Saga step '${stepName}' failed: ${cause?.message ?? 'unknown error'}`);
    this.name = 'SagaStepFailedError';
  }
}

/**
 * Raised when saga compensation fails.
 */
export class SagaCompensationFailedError extends SagaError {
  constructor(
    public readonly stepName: string,
    public readonly cause?: Error
  ) {
    super(`Saga compensation for '${stepName}' failed: ${cause?.message ?? 'unknown error'}`);
    this.name = 'SagaCompensationFailedError';
  }
}

/**
 * Base exception for workflow errors.
 */
export class WorkflowError extends AetherError {
  constructor(message: string) {
    super(message);
    this.name = 'WorkflowError';
  }
}

/**
 * Raised when an invalid state transition is attempted.
 */
export class InvalidTransitionError extends WorkflowError {
  constructor(
    public readonly fromState: string,
    public readonly toState: string,
    public readonly workflowId: string = ''
  ) {
    super(
      `Invalid transition from '${fromState}' to '${toState}' in workflow ${workflowId}`
    );
    this.name = 'InvalidTransitionError';
  }
}

/**
 * Raised when attempting to execute a suspended workflow.
 */
export class WorkflowSuspendedError extends WorkflowError {
  constructor(
    public readonly workflowId: string,
    reason: string = ''
  ) {
    super(`Workflow ${workflowId} is suspended: ${reason}`);
    this.name = 'WorkflowSuspendedError';
  }
}

/**
 * Base exception for human task errors.
 */
export class HumanTaskError extends AetherError {
  constructor(message: string) {
    super(message);
    this.name = 'HumanTaskError';
  }
}

/**
 * Raised when a human task times out.
 */
export class HumanTaskTimeoutError extends HumanTaskError {
  constructor(public readonly taskId: string) {
    super(`Human task ${taskId} timed out`);
    this.name = 'HumanTaskTimeoutError';
  }
}

/**
 * Raised when attempting to complete an unassigned task.
 */
export class HumanTaskNotAssignedError extends HumanTaskError {
  constructor(public readonly taskId: string) {
    super(`Human task ${taskId} is not assigned`);
    this.name = 'HumanTaskNotAssignedError';
  }
}
