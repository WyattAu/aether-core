import {
  TaskFormField,
  TaskForm,
  HumanTask,
  HumanTaskManager,
  createHumanTaskManager,
} from '../../src/workflow/human_task';
import {
  HumanTaskStatus,
  HumanTaskError,
  HumanTaskTimeoutError,
  Duration,
} from '../../src/workflow/types';

describe('TaskFormField', () => {
  it('creates with name and type', () => {
    const f = new TaskFormField('name', 'text');
    expect(f.name).toBe('name');
    expect(f.fieldType).toBe('text');
  });

  it('defaults label to name', () => {
    const f = new TaskFormField('email', 'text');
    expect(f.label).toBe('email');
  });

  it('accepts custom label', () => {
    const f = new TaskFormField('email', 'text', { label: 'Email Address' });
    expect(f.label).toBe('Email Address');
  });

  it('defaults required to false', () => {
    const f = new TaskFormField('opt', 'text');
    expect(f.required).toBe(false);
  });

  it('accepts required=true', () => {
    const f = new TaskFormField('name', 'text', { required: true });
    expect(f.required).toBe(true);
  });

  it('accepts default value', () => {
    const f = new TaskFormField('role', 'text', { default: 'viewer' });
    expect(f.default).toBe('viewer');
  });

  it('accepts validation rules', () => {
    const f = new TaskFormField('age', 'number', { validation: { min: 0, max: 150 } });
    expect(f.validation?.min).toBe(0);
    expect(f.validation?.max).toBe(150);
  });
});

describe('TaskForm', () => {
  it('starts with no fields', () => {
    const form = new TaskForm();
    expect(form.fields).toHaveLength(0);
  });

  it('addField chains and adds field', () => {
    const form = new TaskForm()
      .addField('name', 'text', { required: true })
      .addField('age', 'number');

    expect(form.fields).toHaveLength(2);
    expect(form.fields[0].name).toBe('name');
    expect(form.fields[1].name).toBe('age');
  });

  it('validates required fields present', () => {
    const form = new TaskForm().addField('name', 'text', { required: true });
    expect(form.validate({ name: 'Alice' })).toBe(true);
    expect(form.validate({})).toBe(false);
  });

  it('validates field types', () => {
    const form = new TaskForm()
      .addField('count', 'number')
      .addField('active', 'boolean')
      .addField('label', 'text');

    expect(form.validate({ count: 1, active: true, label: 'ok' })).toBe(true);
    expect(form.validate({ count: 'not-a-number' })).toBe(false);
    expect(form.validate({ active: 'not-bool' })).toBe(false);
    expect(form.validate({ label: 123 })).toBe(false);
  });

  it('validates min/max constraints', () => {
    const form = new TaskForm()
      .addField('score', 'number', { validation: { min: 0, max: 100 } });

    expect(form.validate({ score: 50 })).toBe(true);
    expect(form.validate({ score: -1 })).toBe(false);
    expect(form.validate({ score: 101 })).toBe(false);
  });

  it('skips validation for missing optional fields', () => {
    const form = new TaskForm().addField('optional', 'text');
    expect(form.validate({})).toBe(true);
  });

  it('toDict serializes fields', () => {
    const form = new TaskForm().addField('name', 'text', { required: true, label: 'Name' });
    const dict = form.toDict();
    const fields = dict.fields as Array<Record<string, unknown>>;
    expect(fields).toHaveLength(1);
    expect(fields[0].name).toBe('name');
    expect(fields[0].type).toBe('text');
    expect(fields[0].required).toBe(true);
  });
});

describe('HumanTask', () => {
  it('creates with taskType and title', () => {
    const task = new HumanTask('approval', 'Approve Order');
    expect(task.taskType).toBe('approval');
    expect(task.title).toBe('Approve Order');
    expect(task.taskId).toBeTruthy();
    expect(task.status).toBe(HumanTaskStatus.Pending);
    expect(task.createdAt).toBeInstanceOf(Date);
  });

  it('withAssignee chains', () => {
    const task = new HumanTask('t', 'T').withAssignee('user@co.com');
    expect(task.assignee).toBe('user@co.com');
  });

  it('withCandidates chains users and groups', () => {
    const task = new HumanTask('t', 'T').withCandidates(['u1'], ['g1']);
    expect(task.candidateUsers).toEqual(['u1']);
    expect(task.candidateGroups).toEqual(['g1']);
  });

  it('withPriority clamps to 1-10', () => {
    const task = new HumanTask('t', 'T');
    task.withPriority(0);
    expect(task.priority).toBe(1);
    task.withPriority(15);
    expect(task.priority).toBe(10);
    task.withPriority(5);
    expect(task.priority).toBe(5);
  });

  it('withDueDate chains', () => {
    const date = new Date('2030-01-01');
    const task = new HumanTask('t', 'T').withDueDate(date);
    expect(task.dueDate).toBe(date);
  });

  it('withTimeout chains', () => {
    const task = new HumanTask('t', 'T').withTimeout(Duration.seconds(30), 'fail');
    expect(task.timeout?.toSeconds()).toBe(30);
    expect(task.timeoutAction).toBe('fail');
  });

  it('withForm chains', () => {
    const form = new TaskForm();
    const task = new HumanTask('t', 'T').withForm(form);
    expect(task.form).toBe(form);
  });

  it('withFormValidator chains', () => {
    const validator = () => true;
    const task = new HumanTask('t', 'T').withFormValidator(validator);
    expect(task.formValidator).toBe(validator);
  });

  it('isOverdue returns false when no dueDate', () => {
    const task = new HumanTask('t', 'T');
    expect(task.isOverdue()).toBe(false);
  });

  it('isOverdue returns true when past dueDate', () => {
    const task = new HumanTask('t', 'T').withDueDate(new Date('2000-01-01'));
    expect(task.isOverdue()).toBe(true);
  });

  it('isExpired returns false when no timeout', () => {
    const task = new HumanTask('t', 'T');
    expect(task.isExpired()).toBe(false);
  });

  it('isExpired returns false when within timeout', () => {
    const task = new HumanTask('t', 'T').withTimeout(Duration.hours(24));
    expect(task.isExpired()).toBe(false);
  });

  it('toContext serializes task state', () => {
    const task = new HumanTask('approval', 'Approve')
      .withAssignee('user@co.com')
      .withPriority(3);

    const ctx = task.toContext();
    expect(ctx.taskType).toBe('approval');
    expect(ctx.title).toBe('Approve');
    expect(ctx.assignee).toBe('user@co.com');
    expect(ctx.priority).toBe(3);
    expect(ctx.candidateUsers).toEqual([]);
  });
});

describe('HumanTaskManager', () => {
  let manager: HumanTaskManager;

  beforeEach(() => {
    manager = new HumanTaskManager();
  });

  afterEach(() => {
    manager['pendingTimeouts'].forEach(t => clearTimeout(t));
  });

  it('creates a task', async () => {
    const task = new HumanTask('review', 'Review PR');
    const created = await manager.createTask(task, 'wf-1', 'step-1');
    expect(created.status).toBe(HumanTaskStatus.Pending);
    expect(created.workflowId).toBe('wf-1');
    expect(created.stepName).toBe('step-1');
  });

  it('claims a task by assignee', async () => {
    const task = new HumanTask('t', 'T').withAssignee('user@co.com');
    await manager.createTask(task, 'wf-1', 'step-1');

    const claimed = await manager.claimTask(task.taskId, 'user@co.com');
    expect(claimed.status).toBe(HumanTaskStatus.InProgress);
    expect(claimed.assignee).toBe('user@co.com');
  });

  it('claims a task by candidate user', async () => {
    const task = new HumanTask('t', 'T').withCandidates(['user@co.com']);
    await manager.createTask(task, 'wf-1', 'step-1');

    const claimed = await manager.claimTask(task.taskId, 'user@co.com');
    expect(claimed.status).toBe(HumanTaskStatus.InProgress);
  });

  it('rejects claim for unauthorized user', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');

    await expect(manager.claimTask(task.taskId, 'stranger@co.com'))
      .rejects.toThrow(HumanTaskError);
  });

  it('rejects claim for completed task', async () => {
    const task = new HumanTask('t', 'T').withAssignee('user@co.com');
    await manager.createTask(task, 'wf-1', 'step-1');
    await manager.claimTask(task.taskId, 'user@co.com');
    await manager.completeTask(task.taskId, {});

    await expect(manager.claimTask(task.taskId, 'user@co.com'))
      .rejects.toThrow(HumanTaskError);
  });

  it('rejects claim for missing task', async () => {
    await expect(manager.claimTask('missing', 'user'))
      .rejects.toThrow(HumanTaskError);
  });

  it('completes a task', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');

    const completed = await manager.completeTask(task.taskId, { approved: true }, 'user');
    expect(completed.status).toBe(HumanTaskStatus.Completed);
    expect(completed.result).toEqual({ approved: true });
    expect(completed.completedBy).toBe('user');
    expect(completed.completedAt).toBeInstanceOf(Date);
  });

  it('rejects double complete', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');
    await manager.completeTask(task.taskId, {});

    await expect(manager.completeTask(task.taskId, {}))
      .rejects.toThrow(HumanTaskError);
  });

  it('rejects complete for missing task', async () => {
    await expect(manager.completeTask('missing', {}))
      .rejects.toThrow(HumanTaskError);
  });

  it('rejects complete for timed-out task', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');
    task.status = HumanTaskStatus.Timeout;

    await expect(manager.completeTask(task.taskId, {}))
      .rejects.toThrow(HumanTaskError);
  });

  it('validates form data on complete', async () => {
    const form = new TaskForm().addField('approved', 'boolean', { required: true });
    const task = new HumanTask('t', 'T').withForm(form);
    await manager.createTask(task, 'wf-1', 'step-1');

    await expect(manager.completeTask(task.taskId, {}))
      .rejects.toThrow(HumanTaskError);
  });

  it('rejects a task', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');

    const rejected = await manager.rejectTask(task.taskId, 'not needed', 'user');
    expect(rejected.status).toBe(HumanTaskStatus.Rejected);
    expect(rejected.result).toEqual({ rejected: true, reason: 'not needed' });
  });

  it('rejects missing task', async () => {
    await expect(manager.rejectTask('missing', 'reason'))
      .rejects.toThrow(HumanTaskError);
  });

  it('escalates a task', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');

    const escalated = await manager.escalateTask(task.taskId, 'manager@co.com');
    expect(escalated.status).toBe(HumanTaskStatus.Escalated);
    expect(escalated.assignee).toBe('manager@co.com');
  });

  it('escalates to a group', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');

    const escalated = await manager.escalateTask(task.taskId, 'managers');
    expect(escalated.candidateGroups).toContain('managers');
  });

  it('delegates a task', async () => {
    const task = new HumanTask('t', 'T').withAssignee('user1@co.com');
    await manager.createTask(task, 'wf-1', 'step-1');

    const delegated = await manager.delegateTask(task.taskId, 'user2@co.com');
    expect(delegated.assignee).toBe('user2@co.com');
  });

  it('getTask returns undefined for missing task', async () => {
    const result = await manager.getTask('missing');
    expect(result).toBeUndefined();
  });

  it('getTasksForUser filters by assignee', async () => {
    const task = new HumanTask('t', 'T').withAssignee('user@co.com');
    await manager.createTask(task, 'wf-1', 'step-1');

    const tasks = await manager.getTasksForUser('user@co.com');
    expect(tasks).toHaveLength(1);
  });

  it('getTasksForUser excludes completed by default', async () => {
    const task = new HumanTask('t', 'T').withAssignee('user@co.com');
    await manager.createTask(task, 'wf-1', 'step-1');
    await manager.completeTask(task.taskId, {});

    const tasks = await manager.getTasksForUser('user@co.com');
    expect(tasks).toHaveLength(0);

    const all = await manager.getTasksForUser('user@co.com', true);
    expect(all).toHaveLength(1);
  });

  it('waitForCompletion returns result for completed task', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');
    await manager.completeTask(task.taskId, { result: 'ok' });

    const result = await manager.waitForCompletion(task.taskId);
    expect(result).toEqual({ result: 'ok' });
  });

  it('waitForCompletion throws for missing task', async () => {
    await expect(manager.waitForCompletion('missing'))
      .rejects.toThrow(HumanTaskError);
  });

  it('waitForCompletion throws HumanTaskTimeoutError on timeout', async () => {
    const task = new HumanTask('t', 'T');
    await manager.createTask(task, 'wf-1', 'step-1');

    jest.useFakeTimers();
    const promise = manager.waitForCompletion(task.taskId, 200);
    jest.advanceTimersByTime(300);
    await expect(promise).rejects.toThrow(HumanTaskTimeoutError);
  });

  it('createHumanTaskManager factory', () => {
    const mgr = createHumanTaskManager();
    expect(mgr).toBeInstanceOf(HumanTaskManager);
  });
});
