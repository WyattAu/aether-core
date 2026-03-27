/**
 * Human Task Integration.
 *
 * Provides support for human-in-the-loop workflows with task assignment,
 * timeouts, and escalation.
 *
 * @module aether/workflow/human_task
 */

import {
  HumanTaskStatus,
  HumanTaskContext,
  HumanTaskError,
  HumanTaskTimeoutError,
  HumanTaskNotAssignedError,
  Duration,
  TaskForm as TaskFormInterface,
  TaskFormField as TaskFormFieldInterface,
  TaskFormValidator,
} from './types';

/**
 * A single field definition in a human task form.
 */
export class TaskFormField implements TaskFormFieldInterface {
  name: string;
  fieldType: string;
  label?: string;
  description?: string;
  required: boolean;
  default?: unknown;
  options?: Array<Record<string, unknown>>;
  validation?: Record<string, unknown>;

  constructor(
    name: string,
    fieldType: string,
    options: {
      label?: string;
      description?: string;
      required?: boolean;
      default?: unknown;
      options?: Array<Record<string, unknown>>;
      validation?: Record<string, unknown>;
    } = {}
  ) {
    this.name = name;
    this.fieldType = fieldType;
    this.label = options.label ?? name;
    this.description = options.description;
    this.required = options.required ?? false;
    this.default = options.default;
    this.options = options.options;
    this.validation = options.validation;
  }
}

/**
 * A form definition for a human task.
 *
 * Contains a list of {@link TaskFormField} instances and provides
 * validation logic.
 *
 * @example
 * ```typescript
 * const form = new TaskForm()
 *   .addField('approved', 'boolean', { required: true })
 *   .addField('comments', 'text');
 * form.validate({ approved: true }); // true
 * ```
 */
export class TaskForm implements TaskFormInterface {
  fields: TaskFormField[] = [];

  /**
   * Add a field to the form.
   *
   * @returns `this` for chaining.
   */
  addField(
    name: string,
    fieldType: string,
    options: {
      label?: string;
      description?: string;
      required?: boolean;
      default?: unknown;
      options?: Array<Record<string, unknown>>;
      validation?: Record<string, unknown>;
    } = {}
  ): this {
    this.fields.push(new TaskFormField(name, fieldType, options));
    return this;
  }

  /**
   * Validate form data against the field definitions.
   *
   * Checks required fields, type constraints, and numeric min/max.
   *
   * @returns `true` if the data is valid.
   */
  validate(data: Record<string, unknown>): boolean {
    for (const field of this.fields) {
      if (field.required && !(field.name in data)) {
        return false;
      }

      if (field.name in data) {
        const value = data[field.name];

        if (field.fieldType === 'number' && typeof value !== 'number') {
          return false;
        } else if (field.fieldType === 'boolean' && typeof value !== 'boolean') {
          return false;
        } else if (field.fieldType === 'text' && typeof value !== 'string') {
          return false;
        }

        if (field.validation) {
          const min = field.validation['min'] as number | undefined;
          const max = field.validation['max'] as number | undefined;

          if (min !== undefined && typeof value === 'number' && value < min) {
            return false;
          }
          if (max !== undefined && typeof value === 'number' && value > max) {
            return false;
          }
        }
      }
    }
    return true;
  }

  /** Serialize the form to a plain object. */
  toDict(): Record<string, unknown> {
    return {
      fields: this.fields.map(f => ({
        name: f.name,
        type: f.fieldType,
        label: f.label,
        description: f.description,
        required: f.required,
        default: f.default,
        options: f.options,
        validation: f.validation,
      })),
    };
  }
}

/**
 * A human task that pauses workflow execution until completed.
 *
 * Human tasks support assignment, delegation, timeouts, and escalation.
 * They are typically used for approvals, reviews, or other manual decisions.
 *
 * @example
 * ```typescript
 * const task = new HumanTask('approval', 'Approve Purchase Order')
 *   .withAssignee('manager@company.com')
 *   .withPriority(3);
 * ```
 */
export class HumanTask {
  taskType: string;
  title: string;
  description: string = '';
  assignee?: string;
  candidateUsers: string[] = [];
  candidateGroups: string[] = [];
  priority: number = 5;
  dueDate?: Date;
  timeout?: Duration;
  timeoutAction: string = 'escalate';
  form?: TaskForm;
  formValidator?: TaskFormValidator;
  readonly metadata: Record<string, unknown> = {};
  readonly taskId: string;
  workflowId: string = '';
  stepName: string = '';
  status: HumanTaskStatus = HumanTaskStatus.Pending;
  createdAt: Date;
  updatedAt?: Date;
  completedAt?: Date;
  completedBy?: string;
  result?: Record<string, unknown>;

  constructor(taskType: string, title: string) {
    this.taskType = taskType;
    this.title = title;
    this.taskId = crypto.randomUUID();
    this.createdAt = new Date();
  }

  /** Set the assignee. Returns `this` for chaining. */
  withAssignee(assignee: string): this {
    this.assignee = assignee;
    return this;
  }

  /** Set candidate users and/or groups. Returns `this` for chaining. */
  withCandidates(users?: string[], groups?: string[]): this {
    if (users) this.candidateUsers = users;
    if (groups) this.candidateGroups = groups;
    return this;
  }

  /** Set priority (clamped to 1–10). Returns `this` for chaining. */
  withPriority(priority: number): this {
    this.priority = Math.max(1, Math.min(10, priority));
    return this;
  }

  /** Set the due date. Returns `this` for chaining. */
  withDueDate(dueDate: Date): this {
    this.dueDate = dueDate;
    return this;
  }

  /** Set timeout and action. Returns `this` for chaining. */
  withTimeout(timeout: Duration, action: string = 'escalate'): this {
    this.timeout = timeout;
    this.timeoutAction = action;
    return this;
  }

  /** Set the task form. Returns `this` for chaining. */
  withForm(form: TaskForm): this {
    this.form = form;
    return this;
  }

  /** Set a custom form validator. Returns `this` for chaining. */
  withFormValidator(validator: TaskFormValidator): this {
    this.formValidator = validator;
    return this;
  }

  /** Check whether the task is past its due date. */
  isOverdue(): boolean {
    if (this.dueDate === undefined) return false;
    return new Date() > this.dueDate;
  }

  /** Check whether the task has timed out. */
  isExpired(): boolean {
    if (this.timeout === undefined) return false;
    const expiresAt = new Date(this.createdAt.getTime() + this.timeout.toMilliseconds());
    return new Date() > expiresAt;
  }

  /** Convert to a {@link HumanTaskContext} for storage. */
  toContext(): HumanTaskContext {
    return {
      taskId: this.taskId,
      taskType: this.taskType,
      workflowId: this.workflowId,
      stepName: this.stepName,
      title: this.title,
      description: this.description,
      assignee: this.assignee,
      candidateUsers: [...this.candidateUsers],
      candidateGroups: [...this.candidateGroups],
      priority: this.priority,
      dueDate: this.dueDate,
      formData: this.form?.toDict() ?? {},
      result: this.result,
      status: this.status,
      createdAt: this.createdAt,
      updatedAt: this.updatedAt,
      completedAt: this.completedAt,
      completedBy: this.completedBy,
      metadata: { ...this.metadata },
    };
  }
}

/**
 * Manages the lifecycle of human tasks in workflows.
 *
 * Handles task creation, assignment, claiming, completion,
 * rejection, delegation, and escalation with in-memory storage.
 *
 * @example
 * ```typescript
 * const manager = new HumanTaskManager();
 * const task = await manager.createTask(ht, 'wf-1', 'step-1');
 * await manager.claimTask(task.taskId, 'user@company.com');
 * await manager.completeTask(task.taskId, { approved: true });
 * ```
 */
export class HumanTaskManager {
  private readonly tasks: Map<string, HumanTask> = new Map();
  private readonly pendingTimeouts: Map<string, NodeJS.Timeout> = new Map();

  /**
   * Create and register a new human task.
   *
   * @param task       - The task definition.
   * @param workflowId - Parent workflow instance ID.
   * @param stepName   - Workflow step that created the task.
   * @returns The created task (with IDs and timestamps set).
   */
  async createTask(
    task: HumanTask,
    workflowId: string,
    stepName: string
  ): Promise<HumanTask> {
    task.workflowId = workflowId;
    task.stepName = stepName;
    task.status = HumanTaskStatus.Pending;
    task.createdAt = new Date();

    this.tasks.set(task.taskId, task);

    if (task.timeout) {
      this._scheduleTimeout(task);
    }

    return task;
  }

  /**
   * Claim a task for a specific user.
   *
   * The user must be the assigned user or in the candidate users/groups.
   *
   * @throws {HumanTaskError} If the task is not found, not claimable, or the user is not authorized.
   */
  async claimTask(taskId: string, user: string): Promise<HumanTask> {
    const task = this.tasks.get(taskId);
    if (task === undefined) {
      throw new HumanTaskError(`Task not found: ${taskId}`);
    }

    if (
      task.status !== HumanTaskStatus.Pending &&
      task.status !== HumanTaskStatus.Assigned
    ) {
      throw new HumanTaskError(
        `Task cannot be claimed: status is ${task.status}`
      );
    }

    const canClaim =
      task.assignee === user ||
      task.candidateUsers.includes(user) ||
      this._getGroupsForUser(user).some(g =>
        task.candidateGroups.includes(g)
      );

    if (!canClaim) {
      throw new HumanTaskError(
        `User ${user} cannot claim task ${taskId}`
      );
    }

    task.assignee = user;
    task.status = HumanTaskStatus.InProgress;
    task.updatedAt = new Date();

    return task;
  }

  /**
   * Complete a task with a result.
   *
   * Validates form data if a form is defined.
   *
   * @throws {HumanTaskError} If the task is not found, already completed, timed out, or validation fails.
   */
  async completeTask(
    taskId: string,
    result: Record<string, unknown>,
    user?: string
  ): Promise<HumanTask> {
    const task = this.tasks.get(taskId);
    if (task === undefined) {
      throw new HumanTaskError(`Task not found: ${taskId}`);
    }

    if (task.status === HumanTaskStatus.Completed) {
      throw new HumanTaskError(`Task already completed: ${taskId}`);
    }

    if (task.status === HumanTaskStatus.Timeout) {
      throw new HumanTaskError(`Task has timed out: ${taskId}`);
    }

    if (task.form && !task.form.validate(result)) {
      throw new HumanTaskError(`Invalid form data for task ${taskId}`);
    }

    if (task.formValidator && !task.formValidator(result)) {
      throw new HumanTaskError(
        `Custom validation failed for task ${taskId}`
      );
    }

    task.result = result;
    task.status = HumanTaskStatus.Completed;
    task.completedAt = new Date();
    task.completedBy = user ?? task.assignee;
    task.updatedAt = new Date();

    this._cancelTimeout(taskId);

    return task;
  }

  /**
   * Reject a task.
   *
   * @throws {HumanTaskError} If the task is not found.
   */
  async rejectTask(
    taskId: string,
    reason: string,
    user?: string
  ): Promise<HumanTask> {
    const task = this.tasks.get(taskId);
    if (task === undefined) {
      throw new HumanTaskError(`Task not found: ${taskId}`);
    }

    task.status = HumanTaskStatus.Rejected;
    task.result = { rejected: true, reason };
    task.completedAt = new Date();
    task.completedBy = user ?? task.assignee;
    task.updatedAt = new Date();

    this._cancelTimeout(taskId);

    return task;
  }

  /**
   * Escalate a task to another user or group.
   *
   * @param taskId     - The task ID.
   * @param escalateTo - User email or group name to escalate to.
   * @throws {HumanTaskError} If the task is not found.
   */
  async escalateTask(
    taskId: string,
    escalateTo?: string
  ): Promise<HumanTask> {
    const task = this.tasks.get(taskId);
    if (task === undefined) {
      throw new HumanTaskError(`Task not found: ${taskId}`);
    }

    task.status = HumanTaskStatus.Escalated;
    task.updatedAt = new Date();

    if (escalateTo) {
      if (escalateTo.includes('@')) {
        task.assignee = escalateTo;
      } else {
        task.candidateGroups.push(escalateTo);
      }
    }

    return task;
  }

  /**
   * Delegate a task to another user.
   *
   * @throws {HumanTaskError} If the task is not found.
   */
  async delegateTask(
    taskId: string,
    delegateTo: string
  ): Promise<HumanTask> {
    const task = this.tasks.get(taskId);
    if (task === undefined) {
      throw new HumanTaskError(`Task not found: ${taskId}`);
    }

    const oldAssignee = task.assignee;
    task.assignee = delegateTo;
    task.updatedAt = new Date();

    return task;
  }

  /** Get a task by ID. Returns `undefined` if not found. */
  async getTask(taskId: string): Promise<HumanTask | undefined> {
    return this.tasks.get(taskId);
  }

  /**
   * Get all tasks assigned to or claimable by a user.
   *
   * @param includeCompleted - Whether to include completed, rejected, and timed-out tasks.
   */
  async getTasksForUser(
    user: string,
    includeCompleted: boolean = false
  ): Promise<HumanTask[]> {
    const terminalStatuses = new Set([
      HumanTaskStatus.Completed,
      HumanTaskStatus.Rejected,
      HumanTaskStatus.Timeout,
    ]);

    const result: HumanTask[] = [];
    for (const task of this.tasks.values()) {
      if (task.assignee === user || task.candidateUsers.includes(user)) {
        if (includeCompleted || !terminalStatuses.has(task.status)) {
          result.push(task);
        }
      }
    }
    return result;
  }

  /**
   * Wait for a task to complete.
   *
   * Uses polling since Node.js does not have a native `asyncio.Event` equivalent.
   *
   * @param taskId  - The task ID.
   * @param timeout - Maximum wait time in milliseconds.
   * @returns The task result dict.
   * @throws {HumanTaskError}       If the task is not found or does not complete successfully.
   * @throws {HumanTaskTimeoutError} If the timeout expires.
   */
  async waitForCompletion(
    taskId: string,
    timeout?: number
  ): Promise<Record<string, unknown>> {
    const task = this.tasks.get(taskId);
    if (task === undefined) {
      throw new HumanTaskError(`Task not found: ${taskId}`);
    }

    if (task.status === HumanTaskStatus.Completed) {
      return task.result ?? {};
    }

    if (timeout !== undefined) {
      return new Promise<Record<string, unknown>>((resolve, reject) => {
        const interval = setInterval(() => {
          const current = this.tasks.get(taskId);
          if (current === undefined) {
            clearInterval(interval);
            reject(new HumanTaskError(`Task not found: ${taskId}`));
            return;
          }
          if (
            current.status === HumanTaskStatus.Completed ||
            current.status === HumanTaskStatus.Rejected ||
            current.status === HumanTaskStatus.Timeout
          ) {
            clearInterval(interval);
            if (current.status === HumanTaskStatus.Completed) {
              resolve(current.result ?? {});
            } else {
              reject(
                new HumanTaskError(
                  `Task ${taskId} did not complete successfully (status: ${current.status})`
                )
              );
            }
          }
        }, 100);

        setTimeout(() => {
          clearInterval(interval);
          reject(new HumanTaskTimeoutError(taskId));
        }, timeout);
      });
    }

    // No timeout: poll until completion
    while (true) {
      const current = this.tasks.get(taskId);
      if (current === undefined) {
        throw new HumanTaskError(`Task not found: ${taskId}`);
      }
      if (current.status === HumanTaskStatus.Completed) {
        return current.result ?? {};
      }
      if (
        current.status === HumanTaskStatus.Rejected ||
        current.status === HumanTaskStatus.Timeout
      ) {
        throw new HumanTaskError(
          `Task ${taskId} did not complete successfully (status: ${current.status})`
        );
      }
      await new Promise(r => setTimeout(r, 100));
    }
  }

  private _scheduleTimeout(task: HumanTask): void {
    if (task.timeout === undefined) return;

    const taskId = task.taskId;
    const timeoutMs = task.timeout.toMilliseconds();

    const timer = setTimeout(() => {
      const current = this.tasks.get(taskId);
      if (
        current !== undefined &&
        current.status !== HumanTaskStatus.Completed &&
        current.status !== HumanTaskStatus.Rejected
      ) {
        this._handleTimeout(current);
      }
    }, timeoutMs);

    this.pendingTimeouts.set(taskId, timer);
  }

  private async _handleTimeout(task: HumanTask): Promise<void> {
    task.status = HumanTaskStatus.Timeout;
    task.updatedAt = new Date();

    if (task.timeoutAction === 'escalate') {
      await this.escalateTask(task.taskId);
    } else if (task.timeoutAction === 'fail') {
      task.result = { failed: true, reason: 'timeout' };
    }
  }

  private _cancelTimeout(taskId: string): void {
    const timer = this.pendingTimeouts.get(taskId);
    if (timer !== undefined) {
      clearTimeout(timer);
      this.pendingTimeouts.delete(taskId);
    }
  }

  /**
   * Get groups for a user.
   *
   * Placeholder for identity integration — override or integrate with an IdP.
   */
  protected _getGroupsForUser(_user: string): string[] {
    return [];
  }
}

/**
 * Factory function to create a new {@link HumanTaskManager}.
 */
export function createHumanTaskManager(): HumanTaskManager {
  return new HumanTaskManager();
}
