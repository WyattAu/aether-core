import {
  SagaStep,
  SagaBuilder,
  SagaExecutor,
  saga,
} from '../../src/workflow/saga';
import {
  SagaStatus,
  SagaStepStatus,
  SagaError,
  Duration,
  RetryPolicy,
  defaultRetryConfig,
} from '../../src/workflow/types';

jest.useFakeTimers();

afterEach(() => {
  jest.useRealTimers();
});

describe('SagaStep', () => {
  it('creates with a name and Pending status', () => {
    const step = new SagaStep('step1');
    expect(step.name).toBe('step1');
    expect(step.status).toBe(SagaStepStatus.Pending);
    expect(step.attempts).toBe(0);
  });

  it('chains withAction', () => {
    const step = new SagaStep('s').withAction(() => 'ok');
    expect(step.action).toBeDefined();
    const result = step.action!(null as any);
    expect(result).toBe('ok');
  });

  it('chains withCompensation', () => {
    const step = new SagaStep('s').withCompensation(() => 'undo');
    expect(step.compensate).toBeDefined();
  });

  it('chains withRetry', () => {
    const cfg = defaultRetryConfig();
    const step = new SagaStep('s').withRetry(cfg);
    expect(step.retryConfig).toBe(cfg);
  });

  it('chains withTimeout', () => {
    const step = new SagaStep('s').withTimeout(Duration.seconds(5));
    expect(step.timeout?.toSeconds()).toBe(5);
  });

  it('chains skipIf', () => {
    const step = new SagaStep('s').skipIf(() => true);
    expect(step.skipCondition!(null as any)).toBe(true);
  });

  it('toResult returns snapshot', () => {
    const step = new SagaStep('s');
    step.attempts = 2;
    step.error = 'fail';
    const r = step.toResult();
    expect(r.stepName).toBe('s');
    expect(r.status).toBe(SagaStepStatus.Pending);
    expect(r.attempts).toBe(2);
    expect(r.error).toBe('fail');
  });
});

describe('SagaBuilder', () => {
  it('creates with a name', () => {
    const b = new SagaBuilder('test');
    expect(b.name).toBe('test');
  });

  it('adds steps via step() and action()', () => {
    const b = new SagaBuilder('test')
      .step('a')
      .action(() => 'result-a')
      .step('b')
      .action(() => 'result-b');

    expect(b.steps).toHaveLength(2);
    expect(b.steps[0].name).toBe('a');
    expect(b.steps[1].name).toBe('b');
  });

  it('compensate() sets compensation on current step', () => {
    const b = new SagaBuilder('test')
      .step('s1')
      .compensate(() => 'undo');

    expect(b.steps[0].compensate).toBeDefined();
  });

  it('withRetry() sets retry on current step', () => {
    const cfg = defaultRetryConfig();
    const b = new SagaBuilder('test')
      .step('s1')
      .withRetry(cfg);

    expect(b.steps[0].retryConfig).toBe(cfg);
  });

  it('withTimeout() sets timeout on current step', () => {
    const b = new SagaBuilder('test')
      .step('s1')
      .withTimeout(Duration.seconds(10));

    expect(b.steps[0].timeout?.toSeconds()).toBe(10);
  });

  it('skipIf() sets skip condition on current step', () => {
    const b = new SagaBuilder('test')
      .step('s1')
      .skipIf(() => true);

    expect(b.steps[0].skipCondition!(null as any)).toBe(true);
  });

  it('getStep() finds a step by name', () => {
    const b = new SagaBuilder('test').step('a').action(() => null);
    expect(b.getStep('a')?.name).toBe('a');
    expect(b.getStep('missing')).toBeUndefined();
  });

  it('build() throws if a step has no action', () => {
    const b = new SagaBuilder('test').step('no-action');
    expect(() => b.build()).toThrow("Step 'no-action' has no action defined");
  });

  it('build() succeeds when all steps have actions', () => {
    const b = new SagaBuilder('test')
      .step('a')
      .action(() => null)
      .build();
    expect(b.steps).toHaveLength(1);
  });

  it('action() throws if no step is defined', () => {
    const b = new SagaBuilder('test');
    expect(() => b.action(() => null)).toThrow('No step defined. Call step() first.');
  });

  it('saga() factory creates a SagaBuilder', () => {
    const b = saga('factory-test');
    expect(b).toBeInstanceOf(SagaBuilder);
    expect(b.name).toBe('factory-test');
  });
});

describe('SagaExecutor', () => {
  it('executes a successful saga', async () => {
    const b = new SagaBuilder('test')
      .step('s1').action(() => 'a')
      .step('s2').action(() => 'b')
      .build();

    const executor = new SagaExecutor();
    const result = await executor.execute(b, {});

    expect(result.status).toBe(SagaStatus.Completed);
    expect(result.completedSteps).toEqual(['s1', 's2']);
    expect(result.compensatedSteps).toEqual([]);
    expect(result.durationMs).toBeDefined();
  });

  it('stores step results in context', async () => {
    const b = new SagaBuilder('test')
      .step('s1').action(() => ({ val: 42 }))
      .build();

    const executor = new SagaExecutor();
    const result = await executor.execute(b, {});
    expect(result.status).toBe(SagaStatus.Completed);
  });

  it('compensates on failure in reverse order', async () => {
    const order: string[] = [];
    const b = new SagaBuilder('test')
      .step('s1').action(() => 'ok').compensate(() => { order.push('undo-s1'); })
      .step('s2').action(() => { throw new Error('fail'); }).compensate(() => { order.push('undo-s2'); })
      .build();

    const executor = new SagaExecutor({
      defaultRetry: { ...defaultRetryConfig(), maxAttempts: 1 },
    });
    const result = await executor.execute(b, {});

    expect(result.status).toBe(SagaStatus.Compensated);
    expect(result.completedSteps).toEqual(['s1']);
    expect(order).toEqual(['undo-s1']);
  });

  it('returns Failed when first step fails with no completed steps', async () => {
    const b = new SagaBuilder('test')
      .step('s1').action(() => { throw new Error('instant fail'); })
      .build();

    const executor = new SagaExecutor({
      defaultRetry: { ...defaultRetryConfig(), maxAttempts: 1 },
    });
    const result = await executor.execute(b, {});

    expect(result.status).toBe(SagaStatus.Failed);
    expect(result.compensatedSteps).toEqual([]);
  });

  it('skips steps with skipCondition returning true', async () => {
    const b = new SagaBuilder('test')
      .step('s1').action(() => 'ok').skipIf(() => true)
      .step('s2').action(() => 'ok')
      .build();

    const executor = new SagaExecutor();
    const result = await executor.execute(b, {});

    expect(result.status).toBe(SagaStatus.Completed);
    expect(result.completedSteps).toEqual(['s2']);
  });

  it('retries on failure per retry config', async () => {
    let attempts = 0;
    const b = new SagaBuilder('test')
      .step('flaky').action(() => {
        attempts++;
        if (attempts < 3) throw new Error('not yet');
        return 'done';
      })
      .build();

    const executor = new SagaExecutor({
      defaultRetry: { ...defaultRetryConfig(), maxAttempts: 3, policy: RetryPolicy.None },
    });
    jest.spyOn(Date, 'now').mockReturnValue(0);

    const result = await executor.execute(b, {});
    expect(result.status).toBe(SagaStatus.Completed);
    expect(attempts).toBe(3);
  });

  it('handles step timeout', async () => {
    const b = new SagaBuilder('test')
      .step('slow')
      .action(() => new Promise(resolve => setTimeout(resolve, 60000)))
      .withTimeout(Duration.milliseconds(1))
      .build();

    const executor = new SagaExecutor({
      defaultRetry: { ...defaultRetryConfig(), maxAttempts: 1 },
    });

    jest.advanceTimersByTime(2);
    const result = await executor.execute(b, {});
    expect(result.status).toBe(SagaStatus.Failed);
  });

  it('propagates SagaCompensationFailedError when compensation throws', async () => {
    const b = new SagaBuilder('test')
      .step('s1').action(() => 'ok').compensate(() => { throw new Error('undo fail'); })
      .step('s2').action(() => { throw new Error('step fail'); })
      .build();

    const executor = new SagaExecutor({
      defaultRetry: { ...defaultRetryConfig(), maxAttempts: 1 },
    });
    await expect(executor.execute(b, {})).rejects.toThrow('undo fail');
  });

  it('getStatus returns null for unknown saga', async () => {
    const executor = new SagaExecutor();
    const status = await executor.getStatus('nonexistent');
    expect(status).toBeNull();
  });

  it('compensate() throws for unknown saga ID', async () => {
    const executor = new SagaExecutor();
    await expect(executor.compensate('missing', new SagaBuilder('test'))).rejects.toThrow(SagaError);
  });

  it('executes with empty steps', async () => {
    const b = new SagaBuilder('empty');
    b.build();
    const executor = new SagaExecutor();
    const result = await executor.execute(b, {});
    expect(result.status).toBe(SagaStatus.Completed);
    expect(result.completedSteps).toEqual([]);
  });
});
