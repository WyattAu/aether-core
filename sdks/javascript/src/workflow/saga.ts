/**
 * Saga Pattern Implementation.
 *
 * Provides distributed transaction coordination with compensation
 * for building reliable multi-step workflows.
 *
 * @module aether/workflow/saga
 */

import {
  SagaStepStatus,
  SagaStatus,
  SagaContext,
  SagaResult,
  SagaStepResult,
  SagaError,
  SagaStepFailedError,
  SagaCompensationFailedError,
  Duration,
  RetryConfig,
  RetryPolicy,
  defaultRetryConfig,
  ActionHandler,
  CompensationHandler,
} from './types';

/**
 * A single step in a saga.
 *
 * Each step has an action (the forward operation) and an optional
 * compensation (the undo operation). Steps can be retried, timed out,
 * or conditionally skipped.
 *
 * @typeParam T - Type of the saga input.
 */
export class SagaStep<T = unknown> {
  name: string;
  action?: ActionHandler<T>;
  compensate?: CompensationHandler<T>;
  retryConfig?: RetryConfig;
  timeout?: Duration;
  skipCondition?: (context: SagaContext<T>) => boolean;
  status: SagaStepStatus = SagaStepStatus.Pending;
  attempts: number = 0;
  error?: string;
  startedAt?: Date;
  completedAt?: Date;

  constructor(name: string) {
    this.name = name;
  }

  /** Set the action handler. Returns `this` for chaining. */
  withAction(action: ActionHandler<T>): this {
    this.action = action;
    return this;
  }

  /** Set the compensation handler. Returns `this` for chaining. */
  withCompensation(compensate: CompensationHandler<T>): this {
    this.compensate = compensate;
    return this;
  }

  /** Set retry configuration. Returns `this` for chaining. */
  withRetry(config: RetryConfig): this {
    this.retryConfig = config;
    return this;
  }

  /** Set a step timeout. Returns `this` for chaining. */
  withTimeout(timeout: Duration): this {
    this.timeout = timeout;
    return this;
  }

  /** Set a skip condition. If it returns `true`, the step is skipped. Returns `this` for chaining. */
  skipIf(condition: (context: SagaContext<T>) => boolean): this {
    this.skipCondition = condition;
    return this;
  }

  /** Produce a snapshot of the current step result. */
  toResult(): SagaStepResult {
    return {
      stepName: this.name,
      status: this.status,
      attempts: this.attempts,
      startedAt: this.startedAt,
      completedAt: this.completedAt,
      error: this.error,
    };
  }
}

/**
 * Fluent builder for constructing saga definitions.
 *
 * Sagas provide distributed transaction semantics by defining undo
 * operations for each step. If a step fails, all previously completed
 * steps are compensated in reverse order.
 *
 * @typeParam T - Type of the saga input.
 *
 * @example
 * ```typescript
 * const saga = new SagaBuilder<MyInput>('order-processing')
 *   .step('reserve-inventory')
 *     .action(reserveInventory)
 *     .compensate(releaseInventory)
 *   .step('process-payment')
 *     .action(processPayment)
 *     .compensate(refundPayment)
 *   .build();
 * ```
 */
export class SagaBuilder<T = unknown> {
  private readonly _steps: SagaStep<T>[] = [];
  private _currentStep: SagaStep<T> | null = null;
  private readonly _metadata: Record<string, unknown> = {};

  constructor(public readonly name: string) {}

  /** Add a new step to the saga. Returns `this` for chaining. */
  step(name: string): this {
    const newStep = new SagaStep<T>(name);
    this._steps.push(newStep);
    this._currentStep = newStep;
    return this;
  }

  /** Set the action for the current step. Returns `this` for chaining. */
  action(handler: ActionHandler<T>): this {
    this._requireCurrentStep();
    this._currentStep!.action = handler;
    return this;
  }

  /** Set the compensation for the current step. Returns `this` for chaining. */
  compensate(handler: CompensationHandler<T>): this {
    this._requireCurrentStep();
    this._currentStep!.compensate = handler;
    return this;
  }

  /** Set retry config for the current step. Returns `this` for chaining. */
  withRetry(config: RetryConfig): this {
    this._requireCurrentStep();
    this._currentStep!.retryConfig = config;
    return this;
  }

  /** Set timeout for the current step. Returns `this` for chaining. */
  withTimeout(duration: Duration): this {
    this._requireCurrentStep();
    this._currentStep!.timeout = duration;
    return this;
  }

  /** Set skip condition for the current step. Returns `this` for chaining. */
  skipIf(condition: (context: SagaContext<T>) => boolean): this {
    this._requireCurrentStep();
    this._currentStep!.skipCondition = condition;
    return this;
  }

  /** Add metadata to the saga. Returns `this` for chaining. */
  withMetadata(key: string, value: unknown): this {
    this._metadata[key] = value;
    return this;
  }

  /**
   * Validate the saga definition and return it.
   *
   * @throws {Error} If any step is missing an action.
   */
  build(): this {
    for (const step of this._steps) {
      if (step.action === undefined) {
        throw new Error(`Step '${step.name}' has no action defined`);
      }
    }
    return this;
  }

  /** Return a copy of all steps. */
  get steps(): SagaStep<T>[] {
    return [...this._steps];
  }

  /** Look up a step by name, or `undefined` if not found. */
  getStep(name: string): SagaStep<T> | undefined {
    return this._steps.find(s => s.name === name);
  }

  private _requireCurrentStep(): void {
    if (this._currentStep === null) {
      throw new Error('No step defined. Call step() first.');
    }
  }
}

/**
 * Executes sagas with automatic compensation on failure.
 *
 * Runs saga steps in order. If any step fails, all previously
 * completed steps are compensated in reverse order.
 *
 * @example
 * ```typescript
 * const executor = new SagaExecutor();
 * const result = await executor.execute(saga, { orderId: '123' });
 * if (result.status === SagaStatus.Completed) {
 *   console.log('Success!');
 * }
 * ```
 */
export class SagaExecutor {
  private defaultRetry: RetryConfig;
  private defaultTimeout: Duration;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private readonly runningSagas: Map<string, SagaContext<any>> = new Map();

  constructor(options?: { defaultRetry?: RetryConfig; defaultTimeout?: Duration }) {
    this.defaultRetry = options?.defaultRetry ?? defaultRetryConfig();
    this.defaultTimeout = options?.defaultTimeout ?? Duration.seconds(30);
  }

  /**
   * Execute a saga with the given input.
   *
   * @typeParam T - Type of the saga input.
   * @param saga      - The saga definition (must be built).
   * @param input     - Input data for the saga.
   * @param contextId - Optional explicit context ID.
   * @returns A {@link SagaResult} with execution status, output, and step information.
   */
  async execute<T>(
    saga: SagaBuilder<T>,
    input: T,
    contextId?: string
  ): Promise<SagaResult> {
    const context = new SagaContext<T>(contextId);
    context.input = input;
    context.startedAt = new Date();

    this.runningSagas.set(context.sagaId, context);

    try {
      for (const step of saga.steps) {
        await this._executeStep(step, context, saga);
      }

      context.completedAt = new Date();

      return {
        sagaId: context.sagaId,
        status: SagaStatus.Completed,
        output: context.getState('output') as T | undefined,
        completedSteps: [...context.completedSteps],
        compensatedSteps: [],
        startedAt: context.startedAt,
        completedAt: context.completedAt,
        durationMs: this._durationMs(context),
      };
    } catch (e) {
      if (e instanceof SagaStepFailedError) {
        context.failedStep = e.stepName;
        context.error = e.cause?.message ?? e.message;

        await this._compensate(saga, context);

        context.completedAt = new Date();

        return {
          sagaId: context.sagaId,
          status:
            context.completedSteps.length > 0
              ? SagaStatus.Compensated
              : SagaStatus.Failed,
          error: context.error,
          completedSteps: [...context.completedSteps],
          compensatedSteps: [...context.completedSteps],
          startedAt: context.startedAt,
          completedAt: context.completedAt,
          durationMs: this._durationMs(context),
        };
      }

      context.error = e instanceof Error ? e.message : String(e);
      context.completedAt = new Date();

      return {
        sagaId: context.sagaId,
        status: SagaStatus.Failed,
        error: context.error,
        completedSteps: [...context.completedSteps],
        compensatedSteps: [],
        startedAt: context.startedAt,
        completedAt: context.completedAt,
        durationMs: this._durationMs(context),
      };
    } finally {
      this.runningSagas.delete(context.sagaId);
    }
  }

  /**
   * Get the status of a running saga.
   *
   * @returns A {@link SagaResult} with RUNNING status, or `null` if not found.
   */
  async getStatus(sagaId: string): Promise<SagaResult | null> {
    const context = this.runningSagas.get(sagaId);
    if (context === undefined) {
      return null;
    }
    return {
      sagaId: context.sagaId,
      status: SagaStatus.Running,
      completedSteps: [...context.completedSteps],
      compensatedSteps: [],
      startedAt: context.startedAt,
    };
  }

  /**
   * Manually trigger compensation for a running saga.
   *
   * @throws {SagaError} If no running saga with the given ID exists.
   */
  async compensate<T>(sagaId: string, saga: SagaBuilder<T>): Promise<void> {
    const context = this.runningSagas.get(sagaId);
    if (context === undefined) {
      throw new SagaError(`No running saga with ID ${sagaId}`);
    }
    await this._compensate(saga, context);
  }

  // -- private helpers ------------------------------------------------

  private async _executeStep<T>(
    step: SagaStep<T>,
    context: SagaContext<T>,
    saga: SagaBuilder<T>
  ): Promise<void> {
    if (step.skipCondition?.(context)) {
      step.status = SagaStepStatus.Skipped;
      return;
    }

    if (step.action === undefined) {
      step.status = SagaStepStatus.Skipped;
      return;
    }

    const retryConfig = step.retryConfig ?? this.defaultRetry;
    const timeout = step.timeout ?? this.defaultTimeout;

    step.status = SagaStepStatus.Running;
    step.startedAt = new Date();

    for (let attempt = 1; attempt <= retryConfig.maxAttempts; attempt++) {
      try {
        step.attempts = attempt;

        const result = await this._withTimeout(
          Promise.resolve(step.action(context)),
          timeout
        );

        if (result !== undefined) {
          context.setState(`step_${step.name}_result`, result);
        }

        step.status = SagaStepStatus.Completed;
        step.completedAt = new Date();
        context.markStepCompleted(step.name);
        return;
      } catch (e) {
        const errMsg =
          e instanceof Error ? e.message : String(e);
        step.error = errMsg;

        if (attempt < retryConfig.maxAttempts) {
          await this._waitForRetry(retryConfig, attempt);
        } else {
          step.status = SagaStepStatus.Failed;
          throw new SagaStepFailedError(
            step.name,
            e instanceof Error ? e : new Error(errMsg)
          );
        }
      }
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async _compensate(saga: SagaBuilder<any>, context: SagaContext<any>): Promise<void> {
    const completedSteps = [...context.completedSteps].reverse();

    for (const stepName of completedSteps) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const step = saga.getStep(stepName) as SagaStep<any> | undefined;
      if (step === undefined || step.compensate === undefined) {
        continue;
      }

      try {
        step.status = SagaStepStatus.Compensating;
        await Promise.resolve(step.compensate(context));
        step.status = SagaStepStatus.Compensated;
      } catch (e) {
        step.status = SagaStepStatus.Failed;
        throw new SagaCompensationFailedError(
          step.name,
          e instanceof Error ? e : new Error(String(e))
        );
      }
    }
  }

  private _waitForRetry(config: RetryConfig, attempt: number): Promise<void> {
    if (config.policy === RetryPolicy.None) {
      return Promise.resolve();
    }

    let delayMs = config.initialDelay.toMilliseconds();

    if (
      config.policy === RetryPolicy.Exponential ||
      config.policy === RetryPolicy.ExponentialJitter
    ) {
      delayMs = Math.round(delayMs * config.multiplier ** (attempt - 1));
      delayMs = Math.min(delayMs, config.maxDelay.toMilliseconds());

      if (config.policy === RetryPolicy.ExponentialJitter) {
        const jitterRange = delayMs * config.jitter;
        delayMs = Math.round(
          delayMs + (Math.random() * 2 - 1) * jitterRange
        );
      }
    }

    return new Promise(resolve => setTimeout(resolve, delayMs));
  }

  private _withTimeout<T>(promise: Promise<T>, timeout: Duration): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`Timed out after ${timeout.toSeconds()}s`)),
        timeout.toMilliseconds()
      );
      promise
        .then(value => {
          clearTimeout(timer);
          resolve(value);
        })
        .catch(err => {
          clearTimeout(timer);
          reject(err);
        });
    });
  }

  private _durationMs(context: SagaContext): number | undefined {
    if (context.startedAt && context.completedAt) {
      return context.completedAt.getTime() - context.startedAt.getTime();
    }
    return undefined;
  }
}

/**
 * Factory function to create a new saga definition.
 *
 * @param name - Saga name.
 * @returns A new {@link SagaBuilder} instance.
 */
export function saga<T = unknown>(name: string): SagaBuilder<T> {
  return new SagaBuilder<T>(name);
}
