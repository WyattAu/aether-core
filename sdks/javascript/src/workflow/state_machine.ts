/**
 * Workflow State Machine.
 *
 * Provides state machine definitions with transitions, guards,
 * and lifecycle hooks for building long-running processes.
 *
 * @module aether/workflow/state_machine
 */

import {
  WorkflowStatus,
  WorkflowContext,
  WorkflowResult,
  TransitionResult,
  WorkflowError,
  InvalidTransitionError,
  WorkflowSuspendedError,
  StateMachineEventType,
  TransitionHandler,
} from './types';

/**
 * A state in the workflow state machine.
 */
export class StateDefinition {
  name: string;
  isInitial: boolean = false;
  isFinal: boolean = false;
  onEnter?: TransitionHandler;
  onExit?: TransitionHandler;
  metadata: Record<string, unknown> = {};

  constructor(name: string) {
    this.name = name;
  }
}

/**
 * A transition between two states.
 */
export class StateTransition {
  name: string;
  fromState: string;
  toState: string;
  guard?: (context: WorkflowContext) => boolean;
  action?: TransitionHandler;
  metadata: Record<string, unknown> = {};

  constructor(
    name: string,
    fromState: string,
    toState: string,
    guard?: (context: WorkflowContext) => boolean
  ) {
    this.name = name;
    this.fromState = fromState;
    this.toState = toState;
    this.guard = guard;
  }

  /** Set the transition action. Returns `this` for chaining. */
  withAction(action: TransitionHandler): this {
    this.action = action;
    return this;
  }
}

/**
 * History entry for a state change.
 */
export interface StateHistoryEntry {
  /** Type of event (transition, enter, exit, etc.). */
  type: StateMachineEventType;
  /** State name the event relates to. */
  state: string;
  /** Transition name (for transition events). */
  transition?: string;
  /** Previous state (for transition events). */
  fromState?: string;
  /** Target state (for transition events). */
  toState?: string;
  /** Timestamp of the event. */
  timestamp: Date;
  /** Optional error details. */
  error?: string;
}

/**
 * A workflow definition as a state machine.
 *
 * Defines states and transitions between them. Supports method
 * chaining for fluent API construction.
 *
 * @typeParam T - Type of the workflow input.
 *
 * @example
 * ```typescript
 * const wf = new StateMachineBuilder('order-workflow')
 *   .state('created', { isInitial: true })
 *   .state('shipped', { isFinal: true })
 *   .transition('ship', 'created', 'shipped')
 *   .onEnter('shipped', notifyShipped)
 *   .build();
 * ```
 */
export class StateMachineBuilder<T = unknown> {
  private readonly _states: Map<string, StateDefinition> = new Map();
  private readonly _transitions: Map<string, StateTransition[]> = new Map();
  private _initialState: string | null = null;
  private readonly _finalStates: Set<string> = new Set();
  private readonly _metadata: Record<string, unknown> = {};

  constructor(public readonly name: string) {}

  /**
   * Add a state to the state machine.
   *
   * @param name      - State name.
   * @param options   - State configuration options.
   * @returns `this` for chaining.
   * @throws {WorkflowError} If multiple initial states are defined.
   */
  state(
    name: string,
    options: {
      isInitial?: boolean;
      isFinal?: boolean;
    } = {}
  ): this {
    const def = new StateDefinition(name);
    def.isInitial = options.isInitial ?? false;
    def.isFinal = options.isFinal ?? false;
    this._states.set(name, def);

    if (def.isInitial) {
      if (this._initialState !== null) {
        throw new WorkflowError(
          `Multiple initial states: ${this._initialState} and ${name}`
        );
      }
      this._initialState = name;
    }

    if (def.isFinal) {
      this._finalStates.add(name);
    }

    return this;
  }

  /**
   * Set the on-enter handler for a state.
   *
   * @returns `this` for chaining.
   * @throws {WorkflowError} If the state name is not recognized.
   */
  onEnter(stateName: string, handler: TransitionHandler): this {
    const def = this._states.get(stateName);
    if (def === undefined) {
      throw new WorkflowError(`Unknown state: ${stateName}`);
    }
    def.onEnter = handler;
    return this;
  }

  /**
   * Set the on-exit handler for a state.
   *
   * @returns `this` for chaining.
   * @throws {WorkflowError} If the state name is not recognized.
   */
  onExit(stateName: string, handler: TransitionHandler): this {
    const def = this._states.get(stateName);
    if (def === undefined) {
      throw new WorkflowError(`Unknown state: ${stateName}`);
    }
    def.onExit = handler;
    return this;
  }

  /**
   * Add a transition between states.
   *
   * @param name       - Transition name.
   * @param fromState  - Source state name.
   * @param toState    - Target state name.
   * @param guard      - Optional guard predicate.
   * @returns `this` for chaining.
   * @throws {WorkflowError} If either state is not recognized.
   */
  transition(
    name: string,
    fromState: string,
    toState: string,
    guard?: (context: WorkflowContext) => boolean
  ): this {
    if (!this._states.has(fromState)) {
      throw new WorkflowError(`Unknown source state: ${fromState}`);
    }
    if (!this._states.has(toState)) {
      throw new WorkflowError(`Unknown target state: ${toState}`);
    }

    const t = new StateTransition(name, fromState, toState, guard);
    const existing = this._transitions.get(fromState);
    if (existing) {
      existing.push(t);
    } else {
      this._transitions.set(fromState, [t]);
    }

    return this;
  }

  /**
   * Set the action for a named transition.
   *
   * @returns `this` for chaining.
   * @throws {WorkflowError} If the transition name is not found.
   */
  withAction(transitionName: string, action: TransitionHandler): this {
    for (const transitions of this._transitions.values()) {
      for (const t of transitions) {
        if (t.name === transitionName) {
          t.action = action;
          return this;
        }
      }
    }
    throw new WorkflowError(`Unknown transition: ${transitionName}`);
  }

  /** Add metadata. Returns `this` for chaining. */
  withMetadata(key: string, value: unknown): this {
    this._metadata[key] = value;
    return this;
  }

  /**
   * Validate the state machine definition.
   *
   * @returns `this` for chaining.
   * @throws {WorkflowError} If no initial state is defined.
   */
  build(): this {
    if (this._initialState === null) {
      throw new WorkflowError('No initial state defined');
    }
    return this;
  }

  /** Return a copy of all state definitions. */
  get states(): Map<string, StateDefinition> {
    return new Map(this._states);
  }

  /** Return the initial state name. */
  get initialState(): string {
    if (this._initialState === null) {
      throw new WorkflowError('No initial state defined');
    }
    return this._initialState;
  }

  /** Check whether a state is a final (terminal) state. */
  isFinalState(stateName: string): boolean {
    return this._finalStates.has(stateName);
  }

  /** Return all transitions from a given state. */
  getTransitions(fromState: string): StateTransition[] {
    return this._transitions.get(fromState) ?? [];
  }

  /** Look up a specific transition by name from a state. */
  getTransition(fromState: string, name: string): StateTransition | undefined {
    return this._transitions.get(fromState)?.find(t => t.name === name);
  }

  /**
   * Validate that a transition is allowed.
   *
   * Checks that the transition exists and its guard (if any) returns `true`.
   *
   * @returns The matching {@link StateTransition}, or `undefined` if invalid.
   */
  validateTransition(
    fromState: string,
    transitionName: string,
    context: WorkflowContext
  ): StateTransition | undefined {
    const transition = this.getTransition(fromState, transitionName);
    if (transition === undefined) {
      return undefined;
    }
    if (transition.guard !== undefined && !transition.guard(context)) {
      return undefined;
    }
    return transition;
  }
}

/**
 * Executes workflow state machines.
 *
 * Manages workflow lifecycle, state transitions, and history tracking.
 *
 * @example
 * ```typescript
 * const executor = new StateMachineExecutor();
 * const result = await executor.start(workflow, { orderId: '123' });
 * const tr = await executor.send(result.workflowId, 'ship');
 * const status = await executor.getStatus(result.workflowId);
 * ```
 */
export class StateMachineExecutor {
  private readonly workflows: Map<string, WorkflowContext> = new Map();
  private readonly definitions: Map<string, StateMachineBuilder> = new Map();
  private readonly histories: Map<string, StateHistoryEntry[]> = new Map();

  /**
   * Start a new workflow execution.
   *
   * @typeParam T - Type of the workflow input.
   * @param workflow   - The state machine definition.
   * @param input      - Input data.
   * @param workflowId - Optional explicit ID.
   * @returns A {@link WorkflowResult} with the initial status.
   */
  async start<T>(
    workflow: StateMachineBuilder<T>,
    input: T,
    workflowId?: string
  ): Promise<WorkflowResult> {
    const wfId = workflowId ?? crypto.randomUUID();

    const context = new WorkflowContext<T>(wfId);
    context.workflowType = workflow.name;
    context.currentState = workflow.initialState;
    context.input = input;
    context.startedAt = new Date();
    context.updatedAt = new Date();

    this.workflows.set(wfId, context);
    this.definitions.set(wfId, workflow);
    this.histories.set(wfId, []);

    const initialStateDef = workflow.states.get(workflow.initialState);
    if (initialStateDef?.onEnter) {
      try {
        await Promise.resolve(initialStateDef.onEnter(context));
      } catch (e) {
        // Log but don't fail the start
      }
    }

    this._recordHistory(wfId, {
      type: StateMachineEventType.Enter,
      state: workflow.initialState,
    });

    context.addHistoryEvent('workflow_started', {
      initialState: workflow.initialState,
    });

    return {
      workflowId: wfId,
      status: WorkflowStatus.Running,
      currentState: context.currentState,
      history: [...context.history],
      startedAt: context.startedAt,
    };
  }

  /**
   * Execute a state transition on a running workflow.
   *
   * @param workflowId      - The workflow instance ID.
   * @param transitionName  - The transition to execute.
   * @param payload         - Optional payload for the transition.
   * @returns A {@link TransitionResult} with the outcome.
   * @throws {WorkflowError}           If the workflow is not found.
   * @throws {WorkflowSuspendedError}  If the workflow is suspended.
   * @throws {InvalidTransitionError}  If the transition is not valid.
   */
  async send(
    workflowId: string,
    transitionName: string,
    payload?: Record<string, unknown>
  ): Promise<TransitionResult> {
    const context = this.workflows.get(workflowId);
    if (context === undefined) {
      throw new WorkflowError(`Unknown workflow: ${workflowId}`);
    }

    const workflow = this.definitions.get(workflowId);
    if (workflow === undefined) {
      throw new WorkflowError(`Unknown workflow definition: ${workflowId}`);
    }

    if (context.status === WorkflowStatus.Suspended) {
      throw new WorkflowSuspendedError(workflowId);
    }

    const fromState = context.currentState;

    const transition = workflow.validateTransition(
      fromState,
      transitionName,
      context
    );
    if (transition === undefined) {
      this._recordHistory(workflowId, {
        type: StateMachineEventType.GuardFailed,
        state: fromState,
        transition: transitionName,
        toState: workflow.getTransition(fromState, transitionName)?.toState,
      });
      throw new InvalidTransitionError(fromState, transitionName, workflowId);
    }

    const toState = transition.toState;

    try {
      const fromDef = workflow.states.get(fromState);
      if (fromDef?.onExit) {
        await Promise.resolve(fromDef.onExit(context));
      }

      this._recordHistory(workflowId, {
        type: StateMachineEventType.Exit,
        state: fromState,
      });

      if (transition.action) {
        await Promise.resolve(transition.action(context));
      }

      if (payload) {
        for (const [k, v] of Object.entries(payload)) {
          context.setVariable(k, v);
        }
      }

      context.currentState = toState;
      context.updatedAt = new Date();

      this._recordHistory(workflowId, {
        type: StateMachineEventType.Transition,
        state: toState,
        fromState,
        toState,
        transition: transitionName,
      });

      const toDef = workflow.states.get(toState);
      if (toDef?.onEnter) {
        await Promise.resolve(toDef.onEnter(context));
      }

      this._recordHistory(workflowId, {
        type: StateMachineEventType.Enter,
        state: toState,
      });

      context.addHistoryEvent('transition', {
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
    } catch (e) {
      const errMsg = e instanceof Error ? e.message : String(e);

      this._recordHistory(workflowId, {
        type: StateMachineEventType.ActionFailed,
        state: fromState,
        transition: transitionName,
        toState,
        error: errMsg,
      });

      context.addHistoryEvent('transition_failed', {
        transition: transitionName,
        fromState,
        error: errMsg,
      });

      return {
        success: false,
        fromState,
        toState,
        error: errMsg,
        timestamp: new Date(),
      };
    }
  }

  /**
   * Check whether a transition is allowed from the current state.
   */
  canTransition(workflowId: string, eventName: string): boolean {
    const context = this.workflows.get(workflowId);
    if (context === undefined) return false;

    const workflow = this.definitions.get(workflowId);
    if (workflow === undefined) return false;

    return workflow.validateTransition(context.currentState, eventName, context) !== undefined;
  }

  /**
   * Return the names of transitions available from the current state.
   *
   * Transitions whose guards return `false` are excluded.
   */
  getAvailableEvents(workflowId: string): string[] {
    const context = this.workflows.get(workflowId);
    if (context === undefined) return [];

    const workflow = this.definitions.get(workflowId);
    if (workflow === undefined) return [];

    const transitions = workflow.getTransitions(context.currentState);
    return transitions
      .filter(t => t.guard === undefined || t.guard(context))
      .map(t => t.name);
  }

  /**
   * Suspend a running workflow.
   *
   * @throws {WorkflowError} If the workflow is not found.
   */
  async suspend(workflowId: string, reason: string = ''): Promise<void> {
    const context = this.workflows.get(workflowId);
    if (context === undefined) {
      throw new WorkflowError(`Unknown workflow: ${workflowId}`);
    }
    context.status = WorkflowStatus.Suspended;
    context.updatedAt = new Date();
    context.addHistoryEvent('suspended', { reason });
  }

  /**
   * Resume a suspended workflow.
   *
   * @throws {WorkflowError} If the workflow is not found or not suspended.
   */
  async resume(workflowId: string): Promise<void> {
    const context = this.workflows.get(workflowId);
    if (context === undefined) {
      throw new WorkflowError(`Unknown workflow: ${workflowId}`);
    }
    if (context.status !== WorkflowStatus.Suspended) {
      throw new WorkflowError(`Workflow ${workflowId} is not suspended`);
    }
    context.status = WorkflowStatus.Running;
    context.updatedAt = new Date();
    context.addHistoryEvent('resumed');
  }

  /**
   * Cancel a running workflow.
   *
   * @throws {WorkflowError} If the workflow is not found.
   */
  async cancel(workflowId: string, reason: string = ''): Promise<void> {
    const context = this.workflows.get(workflowId);
    if (context === undefined) {
      throw new WorkflowError(`Unknown workflow: ${workflowId}`);
    }
    context.status = WorkflowStatus.Cancelled;
    context.updatedAt = new Date();
    context.addHistoryEvent('cancelled', { reason });
  }

  /**
   * Get the current status of a workflow.
   *
   * @returns A {@link WorkflowResult}, or `null` if not found.
   */
  async getStatus(workflowId: string): Promise<WorkflowResult | null> {
    const context = this.workflows.get(workflowId);
    if (context === undefined) return null;

    const workflow = this.definitions.get(workflowId);
    const isFinal = workflow ? workflow.isFinalState(context.currentState) : false;

    let status = context.status;
    if (status === WorkflowStatus.Running && isFinal) {
      status = WorkflowStatus.Completed;
    }

    return {
      workflowId,
      status,
      currentState: context.currentState,
      output: Object.fromEntries(context.variables) as Record<string, unknown>,
      history: [...context.history],
      startedAt: context.startedAt,
      updatedAt: context.updatedAt,
    };
  }

  /**
   * Get the state change history for a workflow.
   *
   * @returns An array of history entries, or `null` if not found.
   */
  getHistory(workflowId: string): StateHistoryEntry[] | null {
    return this.histories.get(workflowId) ?? null;
  }

  private _recordHistory(workflowId: string, entry: Omit<StateHistoryEntry, 'timestamp'>): void {
    const history = this.histories.get(workflowId);
    if (history) {
      history.push({ ...entry, timestamp: new Date() });
    }
  }
}

/**
 * Factory function to create a new state machine definition.
 *
 * @param name - State machine name.
 * @returns A new {@link StateMachineBuilder} instance.
 */
export function stateMachine<T = unknown>(name: string): StateMachineBuilder<T> {
  return new StateMachineBuilder<T>(name);
}
