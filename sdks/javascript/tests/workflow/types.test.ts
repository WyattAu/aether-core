import {
  Duration,
  SagaStepStatus,
  SagaStatus,
  WorkflowStatus,
  HumanTaskStatus,
  StateMachineEventType,
  RetryPolicy,
  defaultRetryConfig,
  SagaContext,
  WorkflowContext,
  SagaError,
  SagaStepFailedError,
  SagaCompensationFailedError,
  WorkflowError,
  InvalidTransitionError,
  WorkflowSuspendedError,
  HumanTaskError,
  HumanTaskTimeoutError,
  HumanTaskNotAssignedError,
} from '../../src/workflow/types';

describe('Duration', () => {
  it('creates from milliseconds', () => {
    const d = Duration.milliseconds(1500);
    expect(d.toMilliseconds()).toBe(1500);
  });

  it('creates from seconds', () => {
    const d = Duration.seconds(30);
    expect(d.toMilliseconds()).toBe(30000);
    expect(d.toSeconds()).toBe(30);
  });

  it('creates from minutes', () => {
    const d = Duration.minutes(5);
    expect(d.toMilliseconds()).toBe(300000);
    expect(d.toMinutes()).toBe(5);
  });

  it('creates from hours', () => {
    const d = Duration.hours(2);
    expect(d.toMilliseconds()).toBe(7200000);
    expect(d.toHours()).toBe(2);
  });

  it('creates from days', () => {
    const d = Duration.days(1);
    expect(d.toMilliseconds()).toBe(86400000);
  });

  it('rounds fractional inputs', () => {
    const d = Duration.milliseconds(1.7);
    expect(d.toMilliseconds()).toBe(2);
  });

  it('adds two durations', () => {
    const a = Duration.seconds(10);
    const b = Duration.seconds(20);
    expect(a.add(b).toSeconds()).toBe(30);
  });

  it('subtracts durations', () => {
    const a = Duration.seconds(30);
    const b = Duration.seconds(10);
    expect(a.subtract(b).toSeconds()).toBe(20);
  });

  it('clamps subtraction to zero', () => {
    const a = Duration.seconds(5);
    const b = Duration.seconds(10);
    expect(a.subtract(b).toMilliseconds()).toBe(0);
  });

  it('converts between units correctly', () => {
    const d = Duration.hours(1);
    expect(d.toMinutes()).toBe(60);
    expect(d.toSeconds()).toBe(3600);
  });

  it('handles zero duration', () => {
    const d = Duration.milliseconds(0);
    expect(d.toMilliseconds()).toBe(0);
  });

  it('handles negative input by rounding toward zero', () => {
    const d = Duration.seconds(-1);
    expect(d.toMilliseconds()).toBe(-1000);
  });
});

describe('Enums', () => {
  it('SagaStepStatus has expected values', () => {
    expect(SagaStepStatus.Pending).toBe('pending');
    expect(SagaStepStatus.Running).toBe('running');
    expect(SagaStepStatus.Completed).toBe('completed');
    expect(SagaStepStatus.Compensating).toBe('compensating');
    expect(SagaStepStatus.Compensated).toBe('compensated');
    expect(SagaStepStatus.Failed).toBe('failed');
    expect(SagaStepStatus.Skipped).toBe('skipped');
  });

  it('SagaStatus has expected values', () => {
    expect(SagaStatus.Pending).toBe('pending');
    expect(SagaStatus.Running).toBe('running');
    expect(SagaStatus.Completed).toBe('completed');
    expect(SagaStatus.Compensating).toBe('compensating');
    expect(SagaStatus.Compensated).toBe('compensated');
    expect(SagaStatus.Failed).toBe('failed');
  });

  it('WorkflowStatus has expected values', () => {
    expect(WorkflowStatus.Created).toBe('created');
    expect(WorkflowStatus.Running).toBe('running');
    expect(WorkflowStatus.Suspended).toBe('suspended');
    expect(WorkflowStatus.Completed).toBe('completed');
    expect(WorkflowStatus.Failed).toBe('failed');
    expect(WorkflowStatus.Cancelled).toBe('cancelled');
  });

  it('HumanTaskStatus has expected values', () => {
    expect(HumanTaskStatus.Pending).toBe('pending');
    expect(HumanTaskStatus.Assigned).toBe('assigned');
    expect(HumanTaskStatus.InProgress).toBe('in-progress');
    expect(HumanTaskStatus.Completed).toBe('completed');
    expect(HumanTaskStatus.Rejected).toBe('rejected');
    expect(HumanTaskStatus.Timeout).toBe('timeout');
    expect(HumanTaskStatus.Escalated).toBe('escalated');
  });

  it('StateMachineEventType has expected values', () => {
    expect(StateMachineEventType.Enter).toBe('enter');
    expect(StateMachineEventType.Exit).toBe('exit');
    expect(StateMachineEventType.Transition).toBe('transition');
    expect(StateMachineEventType.GuardFailed).toBe('guard-failed');
    expect(StateMachineEventType.ActionFailed).toBe('action-failed');
  });

  it('RetryPolicy enum has expected values', () => {
    expect(RetryPolicy.None).toBe('none');
    expect(RetryPolicy.Fixed).toBe('fixed');
    expect(RetryPolicy.Exponential).toBe('exponential');
    expect(RetryPolicy.ExponentialJitter).toBe('exponential-jitter');
  });
});

describe('defaultRetryConfig', () => {
  it('returns sensible defaults', () => {
    const cfg = defaultRetryConfig();
    expect(cfg.maxAttempts).toBe(3);
    expect(cfg.policy).toBe(RetryPolicy.Exponential);
    expect(cfg.initialDelay.toSeconds()).toBe(1);
    expect(cfg.maxDelay.toSeconds()).toBe(60);
    expect(cfg.multiplier).toBe(2.0);
    expect(cfg.jitter).toBe(0.1);
  });
});

describe('SagaContext', () => {
  it('generates a sagaId if none provided', () => {
    const ctx = new SagaContext();
    expect(ctx.sagaId).toBeTruthy();
    expect(typeof ctx.sagaId).toBe('string');
  });

  it('uses provided sagaId', () => {
    const ctx = new SagaContext('my-id');
    expect(ctx.sagaId).toBe('my-id');
  });

  it('sets and gets state', () => {
    const ctx = new SagaContext();
    ctx.setState('key', 'value');
    expect(ctx.getState('key')).toBe('value');
  });

  it('returns defaultValue for missing state key', () => {
    const ctx = new SagaContext();
    expect(ctx.getState('missing', 'fallback')).toBe('fallback');
    expect(ctx.getState('missing')).toBeUndefined();
  });

  it('tracks completed steps', () => {
    const ctx = new SagaContext();
    ctx.markStepCompleted('step1');
    ctx.markStepCompleted('step2');
    expect(ctx.completedSteps).toEqual(['step1', 'step2']);
  });

  it('does not duplicate completed steps', () => {
    const ctx = new SagaContext();
    ctx.markStepCompleted('step1');
    ctx.markStepCompleted('step1');
    expect(ctx.completedSteps).toEqual(['step1']);
  });

  it('checks step completion', () => {
    const ctx = new SagaContext();
    ctx.markStepCompleted('done');
    expect(ctx.isStepCompleted('done')).toBe(true);
    expect(ctx.isStepCompleted('other')).toBe(false);
  });

  it('stores typed input', () => {
    const ctx = new SagaContext<{ orderId: string }>();
    ctx.input = { orderId: '123' };
    expect(ctx.input?.orderId).toBe('123');
  });
});

describe('WorkflowContext', () => {
  it('generates a workflowId if none provided', () => {
    const ctx = new WorkflowContext();
    expect(ctx.workflowId).toBeTruthy();
  });

  it('uses provided workflowId', () => {
    const ctx = new WorkflowContext('wf-1');
    expect(ctx.workflowId).toBe('wf-1');
  });

  it('defaults to Running status', () => {
    const ctx = new WorkflowContext();
    expect(ctx.status).toBe(WorkflowStatus.Running);
  });

  it('sets and gets variables', () => {
    const ctx = new WorkflowContext();
    ctx.setVariable('x', 42);
    expect(ctx.getVariable('x')).toBe(42);
    expect(ctx.getVariable('y', 0)).toBe(0);
  });

  it('records history events', () => {
    const ctx = new WorkflowContext();
    ctx.addHistoryEvent('test-event', { detail: true });
    expect(ctx.history).toHaveLength(1);
    expect(ctx.history[0].type).toBe('test-event');
    expect(ctx.history[0].detail).toBe(true);
  });

  it('adds timestamp to history events', () => {
    const ctx = new WorkflowContext();
    ctx.addHistoryEvent('ev');
    expect(ctx.history[0].timestamp).toBeTruthy();
  });
});

describe('Errors', () => {
  it('SagaError extends Error with correct name', () => {
    const err = new SagaError('saga broke');
    expect(err).toBeInstanceOf(Error);
    expect(err).toBeInstanceOf(SagaError);
    expect(err.name).toBe('SagaError');
    expect(err.message).toBe('saga broke');
  });

  it('SagaStepFailedError carries step name and cause', () => {
    const cause = new Error('boom');
    const err = new SagaStepFailedError('step1', cause);
    expect(err.stepName).toBe('step1');
    expect(err.cause).toBe(cause);
    expect(err.message).toContain('step1');
    expect(err.message).toContain('boom');
  });

  it('SagaStepFailedError handles unknown cause', () => {
    const err = new SagaStepFailedError('step2');
    expect(err.message).toContain('step2');
    expect(err.message).toContain('unknown error');
  });

  it('SagaCompensationFailedError carries step name and cause', () => {
    const err = new SagaCompensationFailedError('step1', new Error('fail'));
    expect(err.stepName).toBe('step1');
    expect(err.name).toBe('SagaCompensationFailedError');
  });

  it('WorkflowError extends AetherError', () => {
    const err = new WorkflowError('wf err');
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe('WorkflowError');
  });

  it('InvalidTransitionError carries from/to states', () => {
    const err = new InvalidTransitionError('A', 'B', 'wf-1');
    expect(err.fromState).toBe('A');
    expect(err.toState).toBe('B');
    expect(err.workflowId).toBe('wf-1');
    expect(err.message).toContain('A');
    expect(err.message).toContain('B');
  });

  it('WorkflowSuspendedError carries workflowId', () => {
    const err = new WorkflowSuspendedError('wf-1', 'maintenance');
    expect(err.workflowId).toBe('wf-1');
    expect(err.message).toContain('maintenance');
  });

  it('HumanTaskError extends AetherError', () => {
    const err = new HumanTaskError('task err');
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe('HumanTaskError');
  });

  it('HumanTaskTimeoutError carries taskId', () => {
    const err = new HumanTaskTimeoutError('task-1');
    expect(err.taskId).toBe('task-1');
    expect(err.name).toBe('HumanTaskTimeoutError');
  });

  it('HumanTaskNotAssignedError carries taskId', () => {
    const err = new HumanTaskNotAssignedError('task-2');
    expect(err.taskId).toBe('task-2');
    expect(err.name).toBe('HumanTaskNotAssignedError');
  });
});
