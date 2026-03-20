/**
 * Workflow State Machine
 * 
 * Provides visual workflow definitions with state transitions
 * for building long-running processes.
 */

import {
    Workflow,
    State,
    Transition,
    WorkflowExecutor,
    WorkflowContext,
    WorkflowResult,
    TransitionResult,
    WorkflowStatus,
    Duration,
    WorkflowError,
    InvalidTransitionError,
    HistoryEvent,
} from './types';

/**
 * WorkflowBuilder creates workflow definitions using a fluent API.
 */
export class WorkflowBuilder<T = any> {
    private states: Map<string, State> = new Map();
    private transitions: Map<string, Transition[]> = new Map();
    private initialState: string | null = null;
    private finalStates: Set<string> = new Set();
    private currentState: State | null = null;
    private metadata: Record<string, any> = {};

    constructor(public readonly name: string) {}

    /**
     * Add a state to the workflow.
     */
    state(
        name: string,
        options?: {
            isInitial?: boolean;
            isFinal?: boolean;
            timeout?: Duration;
            timeoutTransition?: string;
        }
    ): WorkflowBuilder<T> {
        const state: State = {
            name,
            isInitial: options?.isInitial ?? false,
            isFinal: options?.isFinal ?? false,
            timeout: options?.timeout,
            timeoutTransition: options?.timeoutTransition,
            metadata: {},
            onEnter: undefined,
            onExit: undefined,
        };

        this.states.set(name, state);

        if (options?.isInitial) {
            if (this.initialState !== null) {
                throw new WorkflowError(`Multiple initial states: ${this.initialState} and ${name}`);
            }
            this.initialState = name;
        }

        if (options?.isFinal) {
            this.finalStates.add(name);
        }

        this.currentState = state;
        return this;
    }

    /**
     * Set the on-enter handler for a state.
     */
    onEnter(stateName: string, handler: (ctx: WorkflowContext<T>) => Promise<any | void>): WorkflowBuilder<T> {
        const state = this.states.get(stateName);
        if (!state) {
            throw new WorkflowError(`Unknown state: ${stateName}`);
        }
        state.onEnter = handler;
        return this;
    }

    /**
     * Set the on-exit handler for a state.
     */
    onExit(stateName: string, handler: (ctx: WorkflowContext<T>) => Promise<any | void>): WorkflowBuilder<T> {
        const state = this.states.get(stateName);
        if (!state) {
            throw new WorkflowError(`Unknown state: ${stateName}`);
        }
        state.onExit = handler;
        return this;
    }

    /**
     * Add a transition between states.
     */
    transition(
        name: string,
        fromState: string,
        toState: string,
        options?: {
            guard?: (ctx: WorkflowContext<T>) => boolean;
        }
    ): WorkflowBuilder<T> {
        if (!this.states.has(fromState)) {
            throw new WorkflowError(`Unknown source state: ${fromState}`);
        }
        if (!this.states.has(toState)) {
            throw new WorkflowError(`Unknown target state: ${toState}`);
        }

        const transition: Transition = {
            name,
            fromState,
            toState,
            guard: options?.guard,
            action: undefined,
            metadata: {},
        };

        if (!this.transitions.has(fromState)) {
            this.transitions.set(fromState, []);
        }
        this.transitions.get(fromState)!.push(transition);

        return this;
    }

    /**
     * Set the action for a transition.
     */
    withAction(transitionName: string, action: (ctx: WorkflowContext<T>) => Promise<any | void>): WorkflowBuilder<T> {
        for (const ts of this.transitions.values()) {
            for (const t of ts) {
                if (t.name === transitionName) {
                    t.action = action;
                    return this;
                }
            }
        }
        throw new WorkflowError(`Unknown transition: ${transitionName}`);
    }

    /**
     * Add metadata to the workflow.
     */
    withMetadata(key: string, value: any): WorkflowBuilder<T> {
        this.metadata[key] = value;
        return this;
    }

    /**
     * Build and validate the workflow definition.
     */
    build(): WorkflowInstance<T> {
        if (this.initialState === null) {
            throw new WorkflowError('No initial state defined');
        }

        return new WorkflowInstance<T>(
            this.name,
            this.states,
            this.transitions,
            this.initialState,
            this.finalStates,
            this.metadata
        );
    }
}

/**
 * WorkflowInstance is a built workflow definition.
 */
export class WorkflowInstance<T = any> {
    constructor(
        public readonly name: string,
        private readonly states: Map<string, State>,
        private readonly transitions: Map<string, Transition[]>,
        public readonly initialState: string,
        private readonly finalStates: Set<string>,
        private readonly metadata: Record<string, any>
    ) {}

    /**
     * Get a state by name.
     */
    getState(name: string): State | undefined {
        return this.states.get(name);
    }

    /**
     * Check if a state is final.
     */
    isFinalState(stateName: string): boolean {
        return this.finalStates.has(stateName);
    }

    /**
     * Get all transitions from a state.
     */
    getTransitions(fromState: string): Transition[] {
        return this.transitions.get(fromState) ?? [];
    }

    /**
     * Get a specific transition.
     */
    getTransition(fromState: string, name: string): Transition | undefined {
        for (const t of this.getTransitions(fromState)) {
            if (t.name === name) {
                return t;
            }
        }
        return undefined;
    }

    /**
     * Validate a transition is allowed.
     */
    validateTransition(
        fromState: string,
        transitionName: string,
        context: WorkflowContext<T>
    ): Transition | undefined {
        const transition = this.getTransition(fromState, transitionName);
        if (!transition) {
            return undefined;
        }

        if (transition.guard && !transition.guard(context)) {
            return undefined;
        }

        return transition;
    }
}

/**
 * WorkflowExecutorInstance executes workflow state machines.
 */
export class WorkflowExecutorInstance {
    private workflows: Map<string, WorkflowContext> = new Map();
    private definitions: Map<string, WorkflowInstance> = new Map();

    /**
     * Start a new workflow execution.
     */
    async start<T>(
        workflow: WorkflowInstance<T>,
        input?: T,
        workflowId?: string
    ): Promise<WorkflowResult> {
        const wfId = workflowId ?? generateUUID();

        const context: WorkflowContext<T> = {
            workflowId: wfId,
            workflowType: workflow.name,
            currentState: workflow.initialState,
            input,
            variables: {},
            history: [],
            startedAt: new Date(),
            updatedAt: new Date(),
            metadata: {},
        };

        this.workflows.set(wfId, context);
        this.definitions.set(wfId, workflow);

        // Execute on-enter for initial state
        const initialState = workflow.getState(workflow.initialState);
        if (initialState?.onEnter) {
            try {
                await initialState.onEnter(context);
            } catch (error) {
                console.error(`Failed to execute on-enter for initial state: ${error}`);
            }
        }

        this.addHistoryEvent(context, 'workflow_started', { initialState: workflow.initialState });

        return {
            workflowId: wfId,
            status: WorkflowStatus.RUNNING,
            currentState: context.currentState,
            startedAt: context.startedAt,
            history: [],
        };
    }

    /**
     * Execute a state transition.
     */
    async transition(
        workflowId: string,
        transitionName: string,
        payload?: Record<string, any>
    ): Promise<TransitionResult> {
        const context = this.workflows.get(workflowId);
        if (!context) {
            throw new WorkflowError(`Unknown workflow: ${workflowId}`);
        }

        const workflow = this.definitions.get(workflowId);
        if (!workflow) {
            throw new WorkflowError(`Unknown workflow definition: ${workflowId}`);
        }

        const fromState = context.currentState;

        // Validate transition
        const transition = workflow.validateTransition(fromState, transitionName, context);
        if (!transition) {
            throw new InvalidTransitionError(fromState, transitionName, workflowId);
        }

        const toState = transition.toState;

        try {
            // Execute on-exit for current state
            const currentStateDef = workflow.getState(fromState);
            if (currentStateDef?.onExit) {
                await currentStateDef.onExit(context);
            }

            // Execute transition action
            if (transition.action) {
                await transition.action(context);
            }

            // Update state
            context.currentState = toState;
            context.updatedAt = new Date();

            // Execute on-enter for new state
            const newStateDef = workflow.getState(toState);
            if (newStateDef?.onEnter) {
                await newStateDef.onEnter(context);
            }

            this.addHistoryEvent(context, 'transition', {
                transition: transitionName,
                fromState,
                toState,
            });

            return {
                success: true,
                fromState,
                toState,
                timestamp: new Date(),
            };
        } catch (error) {
            this.addHistoryEvent(context, 'transition_failed', {
                transition: transitionName,
                fromState,
                error: error.message,
            });

            return {
                success: false,
                fromState,
                toState,
                error: error.message,
                timestamp: new Date(),
            };
        }
    }

    /**
     * Suspend a running workflow.
     */
    async suspend(workflowId: string, reason?: string): Promise<void> {
        const context = this.workflows.get(workflowId);
        if (!context) {
            throw new WorkflowError(`Unknown workflow: ${workflowId}`);
        }

        context.status = WorkflowStatus.SUSPENDED;
        context.updatedAt = new Date();
        this.addHistoryEvent(context, 'suspended', { reason });
    }

    /**
     * Resume a suspended workflow.
     */
    async resume(workflowId: string): Promise<void> {
        const context = this.workflows.get(workflowId);
        if (!context) {
            throw new WorkflowError(`Unknown workflow: ${workflowId}`);
        }

        if (context.status !== WorkflowStatus.SUSPENDED) {
            throw new WorkflowError(`Workflow ${workflowId} is not suspended`);
        }

        context.status = WorkflowStatus.RUNNING;
        context.updatedAt = new Date();
        this.addHistoryEvent(context, 'resumed', {});
    }

    /**
     * Cancel a workflow.
     */
    async cancel(workflowId: string, reason?: string): Promise<void> {
        const context = this.workflows.get(workflowId);
        if (!context) {
            throw new WorkflowError(`Unknown workflow: ${workflowId}`);
        }

        context.status = WorkflowStatus.CANCELLED;
        context.updatedAt = new Date();
        this.addHistoryEvent(context, 'cancelled', { reason });
    }

    /**
     * Get the current status of a workflow.
     */
    async getStatus(workflowId: string): Promise<WorkflowResult | undefined> {
        const context = this.workflows.get(workflowId);
        if (!context) {
            return undefined;
        }

        const workflow = this.definitions.get(workflowId);
        const isFinal = workflow ? workflow.isFinalState(context.currentState) : false;

        let status = context.status ?? WorkflowStatus.RUNNING;
        if (status === WorkflowStatus.RUNNING && isFinal) {
            status = WorkflowStatus.COMPLETED;
        }

        return {
            workflowId,
            status,
            currentState: context.currentState,
            history: context.history,
            startedAt: context.startedAt,
            updatedAt: context.updatedAt,
        };
    }

    /**
     * Get available transitions for a workflow's current state.
     */
    getAvailableTransitions(workflowId: string): string[] {
        const context = this.workflows.get(workflowId);
        if (!context) {
            return [];
        }

        const workflow = this.definitions.get(workflowId);
        if (!workflow) {
            return [];
        }

        const transitions = workflow.getTransitions(context.currentState);
        return transitions
            .filter(t => !t.guard || t.guard(context))
            .map(t => t.name);
    }

    private addHistoryEvent(
        context: WorkflowContext,
        type: string,
        details: Record<string, any>
    ): void {
        context.history.push({
            type,
            timestamp: new Date(),
            details,
        });
    }
}

// Utility function
function generateUUID(): string {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
        const r = Math.random() * 16 | 0;
        return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
    }).join('');
}

// Re-export for convenience
export { WorkflowBuilder as Workflow };

/**
 * Helper function to create a workflow builder.
 */
export function workflow<T = any>(name: string): WorkflowBuilder<T> {
    return new WorkflowBuilder<T>(name);
}
