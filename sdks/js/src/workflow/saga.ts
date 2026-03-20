/**
 * Saga Pattern Implementation
 * 
 * Provides distributed transaction coordination with compensation
 * for building reliable multi-step workflows across actors.
 */

import {
    Saga,
    SagaStep,
    SagaExecutor,
    SagaDefinition,
    SagaContext,
    SagaResult,
    SagaStatus,
    StepStatus,
    RetryPolicy,
    RetryConfig,
    Duration,
    SagaError,
    CompensationError,
} from './types';

/**
 * SagaDefinition defines a saga with ordered steps.
 */
export class SagaDefinition<T = any> {
    private steps: Map<string, SagaStep<T>> = new Map();
    private stepOrder: string[] = [];
    private currentStep: SagaStep<T> | null = null;
    readonly name: string;

    constructor(name: string) {
        this.name = name;
    }

    /**
     * Add a step to the saga.
     */
    step(name: string): SagaDefinition<T> {
        const newStep = new SagaStep<T>(name);
        this.steps.set(name, newStep);
        this.stepOrder.push(name);
        this.currentStep = newStep;
        return this;
    }

    /**
     * Set the action handler for the current step.
     */
    action(handler: (ctx: SagaContext<T>) => Promise<any | void>): SagaDefinition<T> {
        if (!this.currentStep) {
            throw new Error('No step defined. Call step() first.');
        }
        this.currentStep.setAction(handler);
        return this;
    }

    /**
     * Set the compensation handler for the current step.
     */
    compensate(handler: (ctx: SagaContext<T>) => Promise<any | void>): SagaDefinition<T> {
        if (!this.currentStep) {
            throw new Error('No step defined. Call step() first.');
        }
        this.currentStep.setCompensation(handler);
        return this;
    }

    /**
     * Set retry configuration for the current step.
     */
    retry(config: RetryConfig): SagaDefinition<T> {
        if (!this.currentStep) {
            throw new Error('No step defined. Call step() first.');
        }
        this.currentStep.setRetryConfig(config);
        return this;
    }

    /**
     * Set timeout for the current step.
     */
    timeout(duration: Duration): SagaDefinition<T> {
        if (!this.currentStep) {
            throw new Error('No step defined. Call step() first.');
        }
        this.currentStep.setTimeout(duration);
        return this;
    }

    /**
     * Set skip condition for the current step.
     */
    skipIf(condition: (ctx: SagaContext<T>) => boolean): SagaDefinition<T> {
        if (!this.currentStep) {
            throw new Error('No step defined. Call step() first.');
        }
        this.currentStep.setSkipCondition(condition);
        return this;
    }

    /**
     * Get all steps in order.
     */
    getSteps(): SagaStep<T>[] {
        return this.stepOrder.map(name => this.steps.get(name)!);
    }

    /**
     * Get a specific step by name.
     */
    getStep(name: string): SagaStep<T> | undefined {
        return this.steps.get(name);
    }

    /**
     * Build and validate the saga definition.
     */
    build(): SagaDefinition<T> {
        if (this.stepOrder.length === 0) {
            throw new Error('Saga must have at least one step');
        }
        return this;
    }
}

/**
 * SagaStep represents a single step in a saga.
 */
export class SagaStep<T = any> {
    readonly name: string;
    private actionHandler?: (ctx: SagaContext<T>) => Promise<any | void>;
    private compensationHandler?: (ctx: SagaContext<T>) => Promise<any | void>;
    private skipCondition?: (ctx: SagaContext<T>) => boolean;
    private retryConfig?: RetryConfig;
    private timeout?: Duration;

    status: StepStatus = StepStatus.PENDING;
    attempts: number = 0;
    error?: string;
    startedAt?: Date;
    completedAt?: Date;

    constructor(name: string) {
        this.name = name;
    }

    setAction(handler: (ctx: SagaContext<T>) => Promise<any | void>): this {
        this.actionHandler = handler;
    }

    setCompensation(handler: (ctx: SagaContext<T>) => Promise<any | void>): this {
        this.compensationHandler = handler;
    }

    setRetryConfig(config: RetryConfig): void {
        this.retryConfig = config;
    }

    setTimeout(duration: Duration): void {
        this.timeout = duration;
    }

    setSkipCondition(condition: (ctx: SagaContext<T>) => boolean): void {
        this.skipCondition = condition;
    }

    getAction(): ((ctx: SagaContext<T>) => Promise<any | void>) | undefined {
        return this.actionHandler;
    }

    getCompensation(): ((ctx: SagaContext<T>) => Promise<any | void>) | undefined {
        return this.compensationHandler;
    }

    getSkipCondition(): ((ctx: SagaContext<T>) => boolean) | undefined {
        return this.skipCondition;
    }

    getRetryConfig(): RetryConfig | undefined {
        return this.retryConfig;
    }

    getTimeout(): Duration | undefined {
        return this.timeout;
    }
}

/**
 * SagaExecutor executes sagas with compensation support.
 */
export class SagaExecutor {
    private defaultRetry: RetryConfig;
    private defaultTimeout: Duration;
    private runningSagas: Map<string, SagaContext> = new Map();

    constructor(
        defaultRetry?: RetryConfig,
        defaultTimeout?: Duration
    ) {
        this.defaultRetry = defaultRetry ?? {
            maxAttempts: 3,
            policy: RetryPolicy.EXPONENTIAL,
            initialDelay: Duration.fromSeconds(1),
            maxDelay: Duration.fromSeconds(60),
            multiplier: 2.0,
            jitter: 0.1,
        };
        this.defaultTimeout = defaultTimeout ?? Duration.fromSeconds(30);
    }

    /**
     * Execute a saga with the given input.
     */
    async execute<T>(
        saga: SagaDefinition<T>,
        input?: T,
        contextId?: string
    ): Promise<SagaResult> {
        const context: SagaContext<T> = {
            sagaId: contextId ?? generateUUID(),
            input,
            state: {},
            completedSteps: [],
            failedStep: undefined,
            error: undefined,
            startedAt: new Date(),
            completedAt: undefined,
            metadata: {},
        };

        this.runningSagas.set(context.sagaId, context);

        try {
            // Execute all steps
            for (const step of saga.getSteps()) {
                await this.executeStep(step, context);
            }

            // All steps completed
            context.completedAt = new Date();

            return {
                sagaId: context.sagaId,
                status: SagaStatus.COMPLETED,
                output: context.state['output'],
                completedSteps: [...context.completedSteps],
                startedAt: context.startedAt,
                completedAt: context.completedAt,
                durationMs: context.completedAt && context.startedAt
                    ? context.completedAt.getTime() - context.startedAt.getTime()
                    : undefined,
            };
        } catch (error) {
                if (error instanceof SagaError) {
                context.failedStep = error.stepName;
                context.error = error.cause?.message ?? error.message;

                // Compensate completed steps
                await this.compensate(saga, context);

                context.completedAt = new Date();

                return {
                    sagaId: context.sagaId,
                    status: context.completedSteps.length > 0 
                        ? SagaStatus.COMPENSATED 
                        : SagaStatus.FAILED,
                    error: context.error,
                    completedSteps: [...context.completedSteps],
                    compensatedSteps: [...context.completedSteps],
                    startedAt: context.startedAt,
                    completedAt: context.completedAt,
                    durationMs: context.completedAt && context.startedAt
                        ? context.completedAt.getTime() - context.startedAt.getTime()
                        : undefined,
                };
            }

            context.error = error.message;
            context.completedAt = new Date();

            return {
                sagaId: context.sagaId,
                status: SagaStatus.FAILED,
                error: context.error,
                startedAt: context.startedAt,
                completedAt: context.completedAt,
            };
        } finally {
            this.runningSagas.delete(context.sagaId);
        }
    }

    private async executeStep<T>(
        step: SagaStep<T>,
        context: SagaContext<T>
    ): Promise<void> {
        // Check skip condition
        const skipCondition = step.getSkipCondition();
        if (skipCondition && skipCondition(context)) {
            step.status = StepStatus.SKIPPED;
            return;
        }

        const action = step.getAction();
        if (!action) {
            step.status = StepStatus.SKIPPED;
            return;
        }

        const retryConfig = step.getRetryConfig() ?? this.defaultRetry;
        const timeout = step.getTimeout() ?? this.defaultTimeout;

        step.status = StepStatus.RUNNING;
        step.startedAt = new Date();

        for (let attempt = 1; attempt <= retryConfig.maxAttempts; attempt++) {
            try {
                step.attempts = attempt;

                // Execute with timeout
                const result = await this.executeWithTimeout(
                    action(context),
                    timeout.totalSeconds() * 1000
                );

                // Store result in context if any
                if (result !== undefined) {
                    context.state[`step_${step.name}_result`] = result;
                }

                step.status = StepStatus.COMPLETED;
                step.completedAt = new Date();
                context.completedSteps.push(step.name);

                return;
            } catch (error) {
                step.error = error.message;

                if (attempt < retryConfig.maxAttempts) {
                    await this.waitForRetry(retryConfig, attempt);
                } else {
                    step.status = StepStatus.FAILED;
                    throw new SagaError(step.name, error);
                }
            }
        }
    }

    private async executeWithTimeout<T>(
        promise: Promise<T>,
        timeoutMs: number
    ): Promise<T> {
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                reject(new Error(`Operation timed out after ${timeoutMs}ms`));
            }, timeoutMs);

            promise
                .then((result) => {
                    clearTimeout(timer);
                    resolve(result);
                })
                .catch((error) => {
                    clearTimeout(timer);
                    reject(error);
                });
        });
    }

    private async compensate<T>(
        saga: SagaDefinition<T>,
        context: SagaContext<T>
    ): Promise<void> {
        // Get completed steps in reverse order
        const completedSteps = [...context.completedSteps].reverse();

        for (const stepName of completedSteps) {
            const step = saga.getStep(stepName);
            if (!step) {
                continue;
            }

            const compensation = step.getCompensation();
            if (!compensation) {
                continue;
            }

            try {
                step.status = StepStatus.COMPENSATING;
                await compensation(context);
                step.status = StepStatus.COMPENSATED;
            } catch (error) {
                step.status = StepStatus.FAILED;
                throw new CompensationError(stepName, error);
            }
        }
    }

    private async waitForRetry(config: RetryConfig, attempt: number): Promise<void> {
        if (config.policy === RetryPolicy.NONE) {
            return;
        }

        let delayMs = config.initialDelay.milliseconds;

        if (config.policy === RetryPolicy.EXPONENTIAL || config.policy === RetryPolicy.EXPONENTIAL_JITTER) {
            delayMs = Math.floor(delayMs * Math.pow(config.multiplier, attempt - 1));
            delayMs = Math.min(delayMs, config.maxDelay.milliseconds);

            if (config.policy === RetryPolicy.EXPONENTIAL_JITTER) {
                const jitter = delayMs * config.jitter;
                delayMs = Math.floor(delayMs + (Math.random() * 2 - 1) * jitter);
            }
        }

        await new Promise(resolve => setTimeout(resolve, delayMs));
    }

    /**
     * Get the status of a running saga.
     */
    async getStatus(sagaId: string): Promise<SagaResult | undefined> {
        const context = this.runningSagas.get(sagaId);
        if (!context) {
            return undefined;
        }

        return {
            sagaId: context.sagaId,
            status: SagaStatus.RUNNING,
            completedSteps: [...context.completedSteps],
            startedAt: context.startedAt,
        };
    }
}

// Utility function
function generateUUID(): string {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
        const r = Math.random() * 16 | 0;
        return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
    }).join('');
}

/**
 * Helper function to create a saga definition.
 */
export function saga<T = any>(name: string): SagaDefinition<T> {
    return new SagaDefinition<T>(name);
}
