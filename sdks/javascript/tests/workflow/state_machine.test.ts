import {
  StateMachineBuilder,
  StateMachineExecutor,
  stateMachine,
  StateDefinition,
  StateTransition,
} from '../../src/workflow/state_machine';
import {
  WorkflowStatus,
  WorkflowError,
  InvalidTransitionError,
  WorkflowSuspendedError,
  StateMachineEventType,
  WorkflowContext,
} from '../../src/workflow/types';

function createSimpleWorkflow() {
  return new StateMachineBuilder('test-wf')
    .state('idle', { isInitial: true })
    .state('processing')
    .state('done', { isFinal: true })
    .transition('start', 'idle', 'processing')
    .transition('finish', 'processing', 'done')
    .build();
}

describe('StateMachineBuilder', () => {
  it('creates with a name', () => {
    const sm = new StateMachineBuilder('sm');
    expect(sm.name).toBe('sm');
  });

  it('adds states', () => {
    const sm = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .state('b', { isFinal: true });
    expect(sm.states.size).toBe(2);
    expect(sm.states.get('a')?.isInitial).toBe(true);
    expect(sm.states.get('b')?.isFinal).toBe(true);
  });

  it('throws on multiple initial states', () => {
    const sm = new StateMachineBuilder('test');
    sm.state('a', { isInitial: true });
    expect(() => sm.state('b', { isInitial: true })).toThrow(WorkflowError);
    expect(() => sm.state('b', { isInitial: true })).toThrow('Multiple initial states');
  });

  it('adds transitions', () => {
    const sm = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .state('b')
      .transition('go', 'a', 'b');
    const transitions = sm.getTransitions('a');
    expect(transitions).toHaveLength(1);
    expect(transitions[0].name).toBe('go');
    expect(transitions[0].toState).toBe('b');
  });

  it('throws on transition with unknown source state', () => {
    const sm = new StateMachineBuilder('test').state('a');
    expect(() => sm.transition('go', 'missing', 'a')).toThrow('Unknown source state');
  });

  it('throws on transition with unknown target state', () => {
    const sm = new StateMachineBuilder('test').state('a');
    expect(() => sm.transition('go', 'a', 'missing')).toThrow('Unknown target state');
  });

  it('onEnter throws for unknown state', () => {
    const sm = new StateMachineBuilder('test');
    expect(() => sm.onEnter('missing', () => {})).toThrow('Unknown state');
  });

  it('onExit throws for unknown state', () => {
    const sm = new StateMachineBuilder('test');
    expect(() => sm.onExit('missing', () => {})).toThrow('Unknown state');
  });

  it('build() throws when no initial state', () => {
    const sm = new StateMachineBuilder('test').state('a');
    expect(() => sm.build()).toThrow('No initial state defined');
  });

  it('build() succeeds with initial state', () => {
    const sm = new StateMachineBuilder('test').state('a', { isInitial: true }).build();
    expect(sm.initialState).toBe('a');
  });

  it('initialState throws when no initial state defined', () => {
    const sm = new StateMachineBuilder('test').state('a');
    expect(() => sm.initialState).toThrow('No initial state defined');
  });

  it('isFinalState checks correctly', () => {
    const sm = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .state('b', { isFinal: true });
    expect(sm.isFinalState('a')).toBe(false);
    expect(sm.isFinalState('b')).toBe(true);
    expect(sm.isFinalState('missing')).toBe(false);
  });

  it('getTransitions returns empty array for unknown state', () => {
    const sm = new StateMachineBuilder('test').state('a', { isInitial: true }).build();
    expect(sm.getTransitions('missing')).toEqual([]);
  });

  it('getTransition finds specific transition', () => {
    const sm = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .state('b')
      .transition('go', 'a', 'b')
      .build();
    expect(sm.getTransition('a', 'go')?.name).toBe('go');
    expect(sm.getTransition('a', 'missing')).toBeUndefined();
  });

  it('validateTransition checks guard', () => {
    const sm = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .state('b')
      .transition('go', 'a', 'b', () => false)
      .build();

    const ctx = new WorkflowContext();
    expect(sm.validateTransition('a', 'go', ctx)).toBeUndefined();
  });

  it('validateTransition returns transition when guard passes', () => {
    const sm = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .state('b')
      .transition('go', 'a', 'b', () => true)
      .build();

    const ctx = new WorkflowContext();
    const t = sm.validateTransition('a', 'go', ctx);
    expect(t?.name).toBe('go');
  });

  it('withAction throws for unknown transition', () => {
    const sm = new StateMachineBuilder('test').state('a', { isInitial: true }).build();
    expect(() => sm.withAction('missing', () => {})).toThrow('Unknown transition');
  });

  it('stateMachine() factory creates builder', () => {
    const sm = stateMachine('factory');
    expect(sm).toBeInstanceOf(StateMachineBuilder);
    expect(sm.name).toBe('factory');
  });
});

describe('StateMachineExecutor', () => {
  it('starts a workflow in the initial state', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const result = await executor.start(wf, {});

    expect(result.status).toBe(WorkflowStatus.Running);
    expect(result.currentState).toBe('idle');
    expect(result.workflowId).toBeTruthy();
  });

  it('runs onEnter handler on start', async () => {
    let entered = false;
    const wf = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .onEnter('a', () => { entered = true; })
      .build();

    const executor = new StateMachineExecutor();
    await executor.start(wf, {});
    expect(entered).toBe(true);
  });

  it('executes a valid transition', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});
    const tr = await executor.send(startResult.workflowId, 'start');

    expect(tr.success).toBe(true);
    expect(tr.fromState).toBe('idle');
    expect(tr.toState).toBe('processing');
  });

  it('rejects invalid transition', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});

    await expect(executor.send(startResult.workflowId, 'finish'))
      .rejects.toThrow(InvalidTransitionError);
  });

  it('rejects transition on suspended workflow', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});
    await executor.suspend(startResult.workflowId);

    await expect(executor.send(startResult.workflowId, 'start'))
      .rejects.toThrow(WorkflowSuspendedError);
  });

  it('throws on send for unknown workflow', async () => {
    const executor = new StateMachineExecutor();
    await expect(executor.send('missing', 'start')).rejects.toThrow(WorkflowError);
  });

  it('canTransition checks availability', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});

    expect(executor.canTransition(startResult.workflowId, 'start')).toBe(true);
    expect(executor.canTransition(startResult.workflowId, 'finish')).toBe(false);
    expect(executor.canTransition('missing', 'start')).toBe(false);
  });

  it('getAvailableEvents returns valid transitions', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});

    const events = executor.getAvailableEvents(startResult.workflowId);
    expect(events).toContain('start');
    expect(events).not.toContain('finish');
  });

  it('suspend and resume lifecycle', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});

    await executor.suspend(startResult.workflowId, 'maintenance');
    let status = await executor.getStatus(startResult.workflowId);
    expect(status?.status).toBe(WorkflowStatus.Suspended);

    await executor.resume(startResult.workflowId);
    status = await executor.getStatus(startResult.workflowId);
    expect(status?.status).toBe(WorkflowStatus.Running);
  });

  it('resume throws if not suspended', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});
    await expect(executor.resume(startResult.workflowId)).rejects.toThrow(WorkflowError);
  });

  it('cancel sets status to Cancelled', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const startResult = await executor.start(wf, {});

    await executor.cancel(startResult.workflowId, 'user request');
    const status = await executor.getStatus(startResult.workflowId);
    expect(status?.status).toBe(WorkflowStatus.Cancelled);
  });

  it('getStatus returns null for unknown workflow', async () => {
    const executor = new StateMachineExecutor();
    const status = await executor.getStatus('missing');
    expect(status).toBeNull();
  });

  it('getStatus reports Completed when in final state', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const start = await executor.start(wf, {});
    await executor.send(start.workflowId, 'start');
    await executor.send(start.workflowId, 'finish');

    const status = await executor.getStatus(start.workflowId);
    expect(status?.status).toBe(WorkflowStatus.Completed);
  });

  it('getHistory returns recorded events', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const start = await executor.start(wf, {});
    await executor.send(start.workflowId, 'start');

    const history = executor.getHistory(start.workflowId);
    expect(history).not.toBeNull();
    expect(history!.length).toBeGreaterThanOrEqual(2);
    expect(history![0].type).toBe(StateMachineEventType.Enter);
  });

  it('getHistory returns null for unknown workflow', () => {
    const executor = new StateMachineExecutor();
    expect(executor.getHistory('missing')).toBeNull();
  });

  it('records transition with payload in context variables', async () => {
    const wf = createSimpleWorkflow();
    const executor = new StateMachineExecutor();
    const start = await executor.start(wf, {});
    await executor.send(start.workflowId, 'start', { key: 'val' });

    const status = await executor.getStatus(start.workflowId);
    expect((status?.output as Record<string, unknown>)?.key).toBe('val');
  });

  it('records onExit handler during transition', async () => {
    let exited = false;
    const wf = new StateMachineBuilder('test')
      .state('a', { isInitial: true })
      .onExit('a', () => { exited = true; })
      .state('b')
      .transition('go', 'a', 'b')
      .build();

    const executor = new StateMachineExecutor();
    const start = await executor.start(wf, {});
    await executor.send(start.workflowId, 'go');
    expect(exited).toBe(true);
  });
});
