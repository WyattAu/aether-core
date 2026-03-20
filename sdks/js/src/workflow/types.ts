/**
 * Core Types for Workflow Engine
 * 
 * Provides foundational types for the workflow engine including
 * saga patterns, state machines, and human tasks.
 */

import { Duration, SagaStatus, StepStatus, WorkflowStatus, HumanTaskStatus } from './enums';
import { RetryPolicy, RetryConfig } from './config';
import {
    SagaContext,
    WorkflowContext
    HumanTaskContext
} from './context';
import {
    SagaResult
    WorkflowResult
    TransitionResult
} from './result';
import {
    SagaStep
    State
    Transition
} from './definitions';
import {
    SagaError
    CompensationError
    WorkflowError
    InvalidTransitionError
    HumanTaskError
} from './errors';

// ============================================
// Duration
// ============================================

export class Duration {
    readonly milliseconds: number = 0;

    static fromSeconds(seconds: number): Duration {
        return new Duration({ milliseconds: seconds * 1000 });
    }

    static fromMinutes(minutes: number): Duration {
        return new Duration({ milliseconds: minutes * 60 * 1000 });
    }

    static fromHours(hours: number): Duration {
        return new Duration({ milliseconds: hours * 3600 * 1000 });
    }

    static fromDays(days: number): Duration {
        return new Duration({ milliseconds: days * 24 * 3600 * 1000 });
    }

    get totalSeconds(): number {
        return this.milliseconds / 1000;
    }

    get totalMinutes(): number {
        return this.milliseconds / 60000;
    }

    get totalHours(): number {
        return this.milliseconds / 3600000;
    }

    add(other: Duration): Duration {
        return new Duration({ milliseconds: this.milliseconds + other.milliseconds });
    }

    subtract(other: Duration): Duration {
        return new Duration({ milliseconds: Math.max(0, this.milliseconds - other.milliseconds) });
    }
}

// ============================================
// Enums
// ============================================

export enum SagaStatus {
    PENDING = 'pending',
    RUNNING = 'running',
    COMPLETED = 'completed',
    COMPENSATING = 'compensating',
    COMPENSATED = 'compensated',
    FAILED = 'failed',
}

export enum StepStatus {
    PENDING = 'pending',
    RUNNING = 'running',
    COMPLETED = 'completed',
    COMPENSATING = 'compensating',
    COMPENSATED = 'compensated',
    FAILED = 'failed',
    SKIPPED = 'skipped',
}

export enum WorkflowStatus {
    CREATED = 'created',
    RUNNING = 'running',
    SUSPENDED = 'suspended',
    COMPLETED = 'completed',
    FAILED = 'failed',
    CANCELLED = 'cancelled',
}

export enum TransitionStatus {
    PENDING = 'pending',
    SUCCESS = 'success',
    FAILED = 'failed',
    ROLLED_BACK = 'rolled_back',
}

export enum HumanTaskStatus {
    PENDING = 'pending',
    ASSIGNED = 'assigned',
    IN_PROGRESS = 'in_progress',
    COMPLETED = 'completed',
    REJECTED = 'rejected',
    TIMEOUT = 'timeout',
    ESCALATED = 'escalated',
}

export enum RetryPolicy {
    NONE = 'none',
    FIXED = 'fixed',
    EXPONENTIAL = 'exponential',
    EXPONENTIAL_JITTER = 'exponential_jitter',
}

// ============================================
// Retry Configuration
// ============================================

export interface RetryConfig {
    maxAttempts: number;
    policy: RetryPolicy;
    initialDelay: Duration;
    maxDelay: Duration;
    multiplier: number;
    jitter: number;
}

export function createDefaultRetryConfig(): RetryConfig {
    return {
        maxAttempts: 3,
        policy: RetryPolicy.EXPONENTIAL,
        initialDelay: Duration.fromSeconds(1),
        maxDelay: Duration.fromSeconds(60),
        multiplier: 2.0,
        jitter: 0.1,
    };
}

// ============================================
// Context Types
// ============================================

export interface SagaContext<T = any> {
    sagaId: string;
    input?: T;
    state: Record<string, any>;
    completedSteps: string[];
    failedStep?: string;
    error?: string;
    startedAt?: Date;
    completedAt?: Date;
    metadata: Record<string, any>;
}

export function createSagaContext<T>(input?: T): SagaContext<T> {
    return {
        sagaId: generateUUID(),
        input,
        state: {},
        completedSteps: [],
        failedStep: undefined,
        error: undefined,
        startedAt: undefined,
        completedAt: undefined,
        metadata: {},
    };
}

export interface WorkflowContext<T = any> {
    workflowId: string;
    workflowType: string;
    currentState: string;
    input?: T;
    variables: Record<string, any>;
    history: HistoryEvent[];
    startedAt?: Date;
    updatedAt?: Date;
    metadata: Record<string, any>;
}

export function createWorkflowContext<T>(input?: T, workflowType: string): WorkflowContext<T> {
    return {
        workflowId: generateUUID(),
        workflowType,
        currentState: '',
        input,
        variables: {},
        history: [],
        startedAt: undefined,
        updatedAt: undefined,
        metadata: {},
    };
}

export interface HistoryEvent {
    type: string;
    timestamp: Date;
    details: Record<string, any>;
}

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
    formData: Record<string, any>;
    result?: Record<string, any>;
    status: HumanTaskStatus;
    createdAt: Date;
    updatedAt?: Date;
    completedAt?: Date;
    completedBy?: string;
    metadata: Record<string, any>;
}

export function createHumanTaskContext(
    taskType: string,
    title: string,
    description: string,
): HumanTaskContext {
    return {
        taskId: generateUUID(),
        taskType,
        workflowId: '',
        stepName: '',
        title,
        description,
        assignee: undefined,
        candidateUsers: [],
        candidateGroups: [],
        priority: 5,
        dueDate: undefined,
        formData: {},
        result: undefined,
        status: HumanTaskStatus.PENDING,
        createdAt: new Date(),
        updatedAt: undefined,
        completedAt: undefined,
        completedBy: undefined,
        metadata: {},
    };
}

// ============================================
// Result Types
// ============================================

export interface SagaResult<T = any> {
    sagaId: string;
    status: SagaStatus;
    output?: T;
    error?: string;
    completedSteps: string[];
    compensatedSteps: string[];
    startedAt?: Date;
    completedAt?: Date;
    durationMs?: number;
}

export interface WorkflowResult<T = any> {
    workflowId: string;
    status: WorkflowStatus;
    output?: T;
    error?: string;
    currentState: string;
    history: HistoryEvent[];
    startedAt?: Date;
    completedAt?: Date;
    durationMs?: number;
}

export interface TransitionResult {
    success: boolean;
    fromState: string;
    toState: string;
    error?: string;
    timestamp: Date;
}

// ============================================
// Step and State Definitions
// ============================================

export interface SagaStep<T = any> {
    name: string;
    status: StepStatus;
    attempts: number;
    error?: string;
    startedAt?: Date;
    completedAt?: Date;
    timeout?: Duration;
    retryConfig?: RetryConfig;
}

export interface State {
    name: string;
    isInitial: boolean;
    isFinal: boolean;
    timeout?: Duration;
    timeoutTransition?: string;
    metadata: Record<string, any>;
}

export interface Transition {
    name: string;
    fromState: string;
    toState: string;
    metadata: Record<string, any>;
}

// ============================================
// Errors
// ============================================

export class SagaError extends Error {
    constructor(public stepName: string, public cause?: Error) {
        super(`Saga step '${stepName}' failed: ${cause?.message ?? 'unknown error'}`);
        this.name = 'SagaError';
    }
}

export class CompensationError extends Error {
    constructor(public stepName: string, public cause?: Error) {
        super(`Compensation for '${stepName}' failed: ${cause?.message ?? 'unknown error'}`);
        this.name = 'CompensationError';
    }
}

export class WorkflowError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'WorkflowError';
    }
}

export class InvalidTransitionError extends WorkflowError {
    constructor(
        public fromState: string,
        public toState: string,
        public workflowId: string = ''
    ) {
        super(`Invalid transition from '${fromState}' to '${toState}' in workflow ${workflowId}`);
    }
}

export class HumanTaskError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'HumanTaskError';
    }
}

// ============================================
// Utility Functions
// ============================================

function generateUUID(): string {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, () => {
        const r = Math.random() * 16;
        return r < 4 ? r.toString(16) : (r - 4).toString(16);
    }).join('');
}
