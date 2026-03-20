/**
 * Human Task Integration
 * 
 * Provides human task management for workflow approvals and manual steps.
 */

import {
    HumanTaskContext,
    HumanTaskStatus,
    HumanTaskError,
    TaskFormValidator,
    Duration,
} from './types';

/**
 * TaskStore interface for task persistence.
 */
export interface TaskStore {
    create(task: HumanTaskContext): Promise<void>;
    update(task: HumanTaskContext): Promise<void>;
    get(taskId: string): Promise<HumanTaskContext | undefined>;
    query(query: TaskQuery): Promise<HumanTaskContext[]>;
    delete(taskId: string): Promise<void>;
}

/**
 * TaskQuery parameters for searching tasks.
 */
export interface TaskQuery {
    workflowId?: string;
    taskType?: string;
    assignee?: string;
    candidateUser?: string;
    candidateGroup?: string;
    status?: HumanTaskStatus[];
    priority?: number;
    dueBefore?: Date;
    createdAfter?: Date;
    limit?: number;
    offset?: number;
}

/**
 * TaskAssignmentHandler handles task assignment logic.
 */
export interface TaskAssignmentHandler {
    (task: HumanTaskContext, candidates: string[]): Promise<string | undefined>;
}

/**
 * TaskCompletionHandler handles task completion logic.
 */
export interface TaskCompletionHandler {
    (task: HumanTaskContext, result: Record<string, any>): Promise<void>;
}

/**
 * TaskTimeoutHandler handles task timeout logic.
 */
export interface TaskTimeoutHandler {
    (task: HumanTaskContext): Promise<void>;
}

/**
 * HumanTaskManager manages human tasks in workflows.
 */
export class HumanTaskManager {
    private store: TaskStore;
    private assignmentHandlers: Map<string, TaskAssignmentHandler> = new Map();
    private completionHandlers: Map<string, TaskCompletionHandler> = new Map();
    private timeoutHandlers: Map<string, TaskTimeoutHandler> = new Map();
    private validators: Map<string, TaskFormValidator> = new Map();
    private timeoutChecker?: ReturnType<typeof setInterval>;

    constructor(store: TaskStore) {
        this.store = store;
    }

    /**
     * Create a new human task.
     */
    async createTask(
        taskType: string,
        options: {
            workflowId?: string;
            stepName?: string;
            title: string;
            description?: string;
            assignee?: string;
            candidateUsers?: string[];
            candidateGroups?: string[];
            priority?: number;
            dueDate?: Date;
            formData?: Record<string, any>;
            metadata?: Record<string, any>;
        }
    ): Promise<HumanTaskContext> {
        const task: HumanTaskContext = {
            taskId: generateUUID(),
            taskType,
            workflowId: options.workflowId ?? '',
            stepName: options.stepName ?? '',
            title: options.title,
            description: options.description ?? '',
            assignee: options.assignee,
            candidateUsers: options.candidateUsers ?? [],
            candidateGroups: options.candidateGroups ?? [],
            priority: options.priority ?? 5,
            dueDate: options.dueDate,
            formData: options.formData ?? {},
            result: undefined,
            status: options.assignee 
                ? HumanTaskStatus.ASSIGNED 
                : HumanTaskStatus.PENDING,
            createdAt: new Date(),
            updatedAt: undefined,
            completedAt: undefined,
            completedBy: undefined,
            metadata: options.metadata ?? {},
        };

        await this.store.create(task);

        // Auto-assign if handler exists
        if (!task.assignee && task.candidateUsers.length > 0) {
            const handler = this.assignmentHandlers.get(taskType);
            if (handler) {
                const assignee = await handler(task, task.candidateUsers);
                if (assignee) {
                    task.assignee = assignee;
                    task.status = HumanTaskStatus.ASSIGNED;
                    task.updatedAt = new Date();
                    await this.store.update(task);
                }
            }
        }

        return task;
    }

    /**
     * Claim a task (assign to self).
     */
    async claimTask(taskId: string, userId: string): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        if (task.status !== HumanTaskStatus.PENDING && task.status !== HumanTaskStatus.ASSIGNED) {
            throw new HumanTaskError(`Task cannot be claimed in status: ${task.status}`);
        }

        // Check if user is eligible
        const isEligible = 
            task.candidateUsers.includes(userId) ||
            task.candidateGroups.length === 0 || // Open to anyone
            task.assignee === userId;

        if (!isEligible && task.assignee && task.assignee !== userId) {
            throw new HumanTaskError(`Task is already assigned to: ${task.assignee}`);
        }

        task.assignee = userId;
        task.status = HumanTaskStatus.ASSIGNED;
        task.updatedAt = new Date();

        await this.store.update(task);
        return task;
    }

    /**
     * Release a task (unassign).
     */
    async releaseTask(taskId: string): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        if (task.status !== HumanTaskStatus.ASSIGNED && task.status !== HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError(`Task cannot be released in status: ${task.status}`);
        }

        task.assignee = undefined;
        task.status = HumanTaskStatus.PENDING;
        task.updatedAt = new Date();

        await this.store.update(task);
        return task;
    }

    /**
     * Start working on a task.
     */
    async startTask(taskId: string): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        if (task.status !== HumanTaskStatus.ASSIGNED) {
            throw new HumanTaskError(`Task must be assigned before starting`);
        }

        task.status = HumanTaskStatus.IN_PROGRESS;
        task.updatedAt = new Date();

        await this.store.update(task);
        return task;
    }

    /**
     * Complete a task with a result.
     */
    async completeTask(
        taskId: string,
        result: Record<string, any>,
        completedBy: string
    ): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        if (task.status !== HumanTaskStatus.ASSIGNED && task.status !== HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError(`Task cannot be completed in status: ${task.status}`);
        }

        // Validate result if validator exists
        const validator = this.validators.get(task.taskType);
        if (validator && !validator(result)) {
            throw new HumanTaskError(`Task result validation failed`);
        }

        task.result = result;
        task.status = HumanTaskStatus.COMPLETED;
        task.completedAt = new Date();
        task.completedBy = completedBy;
        task.updatedAt = new Date();

        await this.store.update(task);

        // Call completion handler
        const handler = this.completionHandlers.get(task.taskType);
        if (handler) {
            await handler(task, result);
        }

        return task;
    }

    /**
     * Reject a task with a reason.
     */
    async rejectTask(
        taskId: string,
        reason: string,
        rejectedBy: string
    ): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        if (task.status !== HumanTaskStatus.ASSIGNED && task.status !== HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError(`Task cannot be rejected in status: ${task.status}`);
        }

        task.result = { rejected: true, reason };
        task.status = HumanTaskStatus.REJECTED;
        task.completedAt = new Date();
        task.completedBy = rejectedBy;
        task.updatedAt = new Date();

        await this.store.update(task);
        return task;
    }

    /**
     * Escalate a task.
     */
    async escalateTask(taskId: string, reason: string): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        task.status = HumanTaskStatus.ESCALATED;
        task.metadata = { ...task.metadata, escalationReason: reason };
        task.updatedAt = new Date();

        await this.store.update(task);
        return task;
    }

    /**
     * Delegate a task to another user.
     */
    async delegateTask(taskId: string, toUser: string): Promise<HumanTaskContext> {
        const task = await this.store.get(taskId);
        if (!task) {
            throw new HumanTaskError(`Task not found: ${taskId}`);
        }

        if (task.status !== HumanTaskStatus.ASSIGNED && task.status !== HumanTaskStatus.IN_PROGRESS) {
            throw new HumanTaskError(`Task cannot be delegated in status: ${task.status}`);
        }

        task.assignee = toUser;
        task.status = HumanTaskStatus.ASSIGNED;
        task.updatedAt = new Date();

        await this.store.update(task);
        return task;
    }

    /**
     * Query tasks based on criteria.
     */
    async queryTasks(query: TaskQuery): Promise<HumanTaskContext[]> {
        return this.store.query(query);
    }

    /**
     * Get a task by ID.
     */
    async getTask(taskId: string): Promise<HumanTaskContext | undefined> {
        return this.store.get(taskId);
    }

    /**
     * Get tasks for a workflow.
     */
    async getWorkflowTasks(workflowId: string): Promise<HumanTaskContext[]> {
        return this.store.query({ workflowId });
    }

    /**
     * Get tasks assigned to a user.
     */
    async getUserTasks(userId: string): Promise<HumanTaskContext[]> {
        return this.store.query({
            assignee: userId,
            status: [
                HumanTaskStatus.ASSIGNED,
                HumanTaskStatus.IN_PROGRESS,
            ],
        });
    }

    /**
     * Get tasks available for a user (candidate).
     */
    async getAvailableTasks(userId: string, groups?: string[]): Promise<HumanTaskContext[]> {
        // This would need a more sophisticated query in a real implementation
        const pendingTasks = await this.store.query({
            status: [HumanTaskStatus.PENDING],
        });

        return pendingTasks.filter(task => 
            task.candidateUsers.includes(userId) ||
            (groups && groups.some(g => task.candidateGroups.includes(g))) ||
            (task.candidateUsers.length === 0 && task.candidateGroups.length === 0)
        );
    }

    /**
     * Register an assignment handler for a task type.
     */
    onAssignment(taskType: string, handler: TaskAssignmentHandler): void {
        this.assignmentHandlers.set(taskType, handler);
    }

    /**
     * Register a completion handler for a task type.
     */
    onCompletion(taskType: string, handler: TaskCompletionHandler): void {
        this.completionHandlers.set(taskType, handler);
    }

    /**
     * Register a timeout handler for a task type.
     */
    onTimeout(taskType: string, handler: TaskTimeoutHandler): void {
        this.timeoutHandlers.set(taskType, handler);
    }

    /**
     * Register a form validator for a task type.
     */
    setValidator(taskType: string, validator: TaskFormValidator): void {
        this.validators.set(taskType, validator);
    }

    /**
     * Start the timeout checker.
     */
    startTimeoutChecker(intervalMs: number = 60000): void {
        if (this.timeoutChecker) {
            return;
        }

        this.timeoutChecker = setInterval(async () => {
            try {
                await this.checkTimeouts();
            } catch (error) {
                console.error('Timeout checker error:', error);
            }
        }, intervalMs);
    }

    /**
     * Stop the timeout checker.
     */
    stopTimeoutChecker(): void {
        if (this.timeoutChecker) {
            clearInterval(this.timeoutChecker);
            this.timeoutChecker = undefined;
        }
    }

    private async checkTimeouts(): Promise<void> {
        const now = new Date();
        
        // Get tasks with due dates
        const tasks = await this.store.query({
            status: [
                HumanTaskStatus.PENDING,
                HumanTaskStatus.ASSIGNED,
                HumanTaskStatus.IN_PROGRESS,
            ],
        });

        for (const task of tasks) {
            if (task.dueDate && task.dueDate < now) {
                // Mark as timed out
                task.status = HumanTaskStatus.TIMEOUT;
                task.updatedAt = now;
                task.metadata = { ...task.metadata, timedOutAt: now.toISOString() };
                
                await this.store.update(task);

                // Call timeout handler
                const handler = this.timeoutHandlers.get(task.taskType);
                if (handler) {
                    try {
                        await handler(task);
                    } catch (error) {
                        console.error(`Timeout handler error for task ${task.taskId}:`, error);
                    }
                }
            }
        }
    }
}

/**
 * InMemoryTaskStore is a simple in-memory task store.
 */
export class InMemoryTaskStore implements TaskStore {
    private tasks: Map<string, HumanTaskContext> = new Map();

    async create(task: HumanTaskContext): Promise<void> {
        this.tasks.set(task.taskId, { ...task });
    }

    async update(task: HumanTaskContext): Promise<void> {
        this.tasks.set(task.taskId, { ...task });
    }

    async get(taskId: string): Promise<HumanTaskContext | undefined> {
        const task = this.tasks.get(taskId);
        return task ? { ...task } : undefined;
    }

    async query(query: TaskQuery): Promise<HumanTaskContext[]> {
        let results = Array.from(this.tasks.values());

        if (query.workflowId) {
            results = results.filter(t => t.workflowId === query.workflowId);
        }
        if (query.taskType) {
            results = results.filter(t => t.taskType === query.taskType);
        }
        if (query.assignee) {
            results = results.filter(t => t.assignee === query.assignee);
        }
        if (query.status && query.status.length > 0) {
            results = results.filter(t => query.status!.includes(t.status));
        }
        if (query.priority !== undefined) {
            results = results.filter(t => t.priority === query.priority);
        }
        if (query.dueBefore) {
            results = results.filter(t => t.dueDate && t.dueDate < query.dueBefore!);
        }
        if (query.createdAfter) {
            results = results.filter(t => t.createdAt > query.createdAfter!);
        }

        // Sort by priority then creation date
        results.sort((a, b) => {
            if (a.priority !== b.priority) {
                return a.priority - b.priority;
            }
            return a.createdAt.getTime() - b.createdAt.getTime();
        });

        if (query.offset) {
            results = results.slice(query.offset);
        }
        if (query.limit) {
            results = results.slice(0, query.limit);
        }

        return results;
    }

    async delete(taskId: string): Promise<void> {
        this.tasks.delete(taskId);
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
 * Helper function to create a human task manager with in-memory store.
 */
export function createHumanTaskManager(): HumanTaskManager {
    return new HumanTaskManager(new InMemoryTaskStore());
}
