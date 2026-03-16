/**
 * Scheduler Actor Example
 * 
 * Demonstrates scheduled task execution with cron-like patterns,
 * task management, and execution tracking.
 */
import { Actor, Message, MessageType, State } from '@aether/sdk';

// ============================================
// Types
// ============================================

interface ScheduledTask {
    id: string;
    name: string;
    cron: string;           // Cron expression (simplified: minute hour day month weekday)
    action: string;         // Action to execute
    payload: any;           // Payload for the action
    enabled: boolean;
    lastRun?: string;
    nextRun?: string;
    runCount: number;
    errorCount: number;
    createdAt: string;
}

interface TaskExecution {
    taskId: string;
    executedAt: string;
    success: boolean;
    error?: string;
    durationMs: number;
}

// ============================================
// Simple Cron Parser
// ============================================

class CronParser {
    static getNextRun(cron: string, from?: Date): Date {
        const parts = cron.split(' ');
        if (parts.length !== 5) {
            throw new Error('Invalid cron expression');
        }

        const [minute, hour, day, month, weekday] = parts;
        const now = from || new Date();
        const next = new Date(now);

        // Simple implementation: run at the next matching time
        // In production, use a proper cron library
        
        // For simplicity, we'll handle basic patterns:
        // "* * * * *" - every minute
        // "*/5 * * * *" - every 5 minutes
        // "0 * * * *" - every hour
        // "0 0 * * *" - every day at midnight

        if (minute === '*') {
            next.setMinutes(next.getMinutes() + 1, 0, 0);
        } else if (minute.startsWith('*/')) {
            const interval = parseInt(minute.substring(2));
            const currentMinute = next.getMinutes();
            const nextMinute = Math.ceil((currentMinute + 1) / interval) * interval;
            if (nextMinute >= 60) {
                next.setHours(next.getHours() + 1, 0, 0, 0);
            } else {
                next.setMinutes(nextMinute, 0, 0);
            }
        } else if (hour === '*') {
            next.setHours(next.getHours() + 1, 0, 0, 0);
        } else {
            // Default: next occurrence of specified time
            const targetMinute = parseInt(minute);
            const targetHour = parseInt(hour);
            next.setHours(targetHour, targetMinute, 0, 0);
            if (next <= now) {
                next.setDate(next.getDate() + 1);
            }
        }

        return next;
    }

    static shouldRun(task: ScheduledTask): boolean {
        if (!task.enabled || !task.nextRun) {
            return false;
        }
        return new Date(task.nextRun) <= new Date();
    }
}

// ============================================
// Scheduler Actor
// ============================================

class SchedulerActor extends Actor {
    private tasks: Map<string, ScheduledTask> = new Map();
    private executions: TaskExecution[] = [];
    private state: State;
    private stateKey: string;
    private schedulerInterval?: ReturnType<typeof setInterval>;
    private maxExecutionsHistory: number = 100;

    constructor() {
        super('scheduler-actor');
        this.state = new State();
        this.stateKey = 'scheduler_state';
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG', 'TIME');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Scheduler Actor starting...`);
        
        await this.loadState();
        
        // Start scheduler loop
        this.startScheduler();
        
        console.log(`[${this.name}] Loaded ${this.tasks.size} scheduled tasks`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Scheduler Actor stopping...`);
        
        // Stop scheduler
        if (this.schedulerInterval) {
            clearInterval(this.schedulerInterval);
        }
        
        await this.saveState();
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type !== MessageType.REQUEST && message.type !== MessageType.RPC_REQUEST) {
            return null;
        }

        const payload = message.payload as Record<string, any> | null;
        if (!payload || typeof payload !== 'object') {
            return Message.response({ error: 'invalid payload' });
        }

        const action = payload.action || '';

        switch (action) {
            case 'schedule':
                return this.handleSchedule(payload);
            case 'cancel':
                return this.handleCancel(payload);
            case 'enable':
                return this.handleEnable(payload);
            case 'disable':
                return this.handleDisable(payload);
            case 'status':
                return this.handleStatus(payload);
            case 'list':
                return this.handleList();
            case 'history':
                return this.handleHistory(payload);
            case 'run_now':
                return this.handleRunNow(payload);
            case 'clear_history':
                return this.handleClearHistory();
            default:
                return Message.response({ error: `unknown action: ${action}` });
        }
    }

    private startScheduler(): void {
        // Check every second for tasks to run
        this.schedulerInterval = setInterval(() => {
            this.checkAndRunTasks();
        }, 1000);
    }

    private checkAndRunTasks(): void {
        for (const [taskId, task] of this.tasks) {
            if (CronParser.shouldRun(task)) {
                this.executeTask(task).catch(error => {
                    console.error(`[${this.name}] Task ${taskId} failed:`, error);
                });
            }
        }
    }

    private async executeTask(task: ScheduledTask): Promise<void> {
        const startTime = Date.now();
        console.log(`[${this.name}] Executing task '${task.name}' (${task.id})`);

        let success = true;
        let error: string | undefined;

        try {
            // Simulate task execution
            // In a real implementation, this would invoke another actor or perform an action
            await this.performTaskAction(task);
        } catch (e) {
            success = false;
            error = e instanceof Error ? e.message : String(e);
        }

        const durationMs = Date.now() - startTime;

        // Record execution
        const execution: TaskExecution = {
            taskId: task.id,
            executedAt: new Date().toISOString(),
            success,
            error,
            durationMs
        };

        this.executions.unshift(execution);
        if (this.executions.length > this.maxExecutionsHistory) {
            this.executions.pop();
        }

        // Update task
        task.lastRun = execution.executedAt;
        task.runCount++;
        if (!success) {
            task.errorCount++;
        }

        // Calculate next run
        try {
            const nextRun = CronParser.getNextRun(task.cron);
            task.nextRun = nextRun.toISOString();
        } catch (e) {
            console.error(`[${this.name}] Invalid cron for task ${task.id}:`, e);
            task.enabled = false;
        }

        await this.saveState();

        console.log(`[${this.name}] Task '${task.name}' ${success ? 'completed' : 'failed'} in ${durationMs}ms`);
    }

    private async performTaskAction(task: ScheduledTask): Promise<void> {
        // Simulate different task actions
        switch (task.action) {
            case 'log':
                console.log(`[Scheduled] ${task.name}: ${JSON.stringify(task.payload)}`);
                break;
            case 'cleanup':
                console.log(`[Scheduled] Running cleanup: ${JSON.stringify(task.payload)}`);
                break;
            case 'report':
                console.log(`[Scheduled] Generating report: ${JSON.stringify(task.payload)}`);
                break;
            case 'sync':
                console.log(`[Scheduled] Syncing data: ${JSON.stringify(task.payload)}`);
                break;
            default:
                console.log(`[Scheduled] Unknown action: ${task.action}`);
        }

        // Simulate work
        await new Promise(resolve => setTimeout(resolve, Math.random() * 100));
    }

    private handleSchedule(payload: Record<string, any>): Message {
        const name = payload.name;
        const cron = payload.cron || '* * * * *';
        const action = payload.action || 'log';
        const taskPayload = payload.payload;
        const enabled = payload.enabled !== false;

        if (!name) {
            return Message.response({ error: 'name is required' });
        }

        const taskId = `task-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

        let nextRun: Date;
        try {
            nextRun = CronParser.getNextRun(cron);
        } catch (e) {
            return Message.response({ error: `invalid cron expression: ${e}` });
        }

        const task: ScheduledTask = {
            id: taskId,
            name,
            cron,
            action,
            payload: taskPayload,
            enabled,
            runCount: 0,
            errorCount: 0,
            nextRun: nextRun.toISOString(),
            createdAt: new Date().toISOString()
        };

        this.tasks.set(taskId, task);
        this.saveState();

        console.log(`[${this.name}] Scheduled task '${name}' (${taskId}) with cron '${cron}'`);

        return Message.response({
            action: 'scheduled',
            task_id: taskId,
            name,
            cron,
            next_run: task.nextRun,
            enabled
        });
    }

    private handleCancel(payload: Record<string, any>): Message {
        const taskId = payload.task_id;
        if (!taskId) {
            return Message.response({ error: 'task_id is required' });
        }

        const task = this.tasks.get(taskId);
        if (!task) {
            return Message.response({ error: `task not found: ${taskId}` });
        }

        this.tasks.delete(taskId);
        this.saveState();

        console.log(`[${this.name}] Cancelled task '${task.name}' (${taskId})`);

        return Message.response({
            action: 'cancelled',
            task_id: taskId,
            name: task.name
        });
    }

    private handleEnable(payload: Record<string, any>): Message {
        const taskId = payload.task_id;
        if (!taskId) {
            return Message.response({ error: 'task_id is required' });
        }

        const task = this.tasks.get(taskId);
        if (!task) {
            return Message.response({ error: `task not found: ${taskId}` });
        }

        task.enabled = true;
        this.saveState();

        return Message.response({
            action: 'enabled',
            task_id: taskId,
            name: task.name
        });
    }

    private handleDisable(payload: Record<string, any>): Message {
        const taskId = payload.task_id;
        if (!taskId) {
            return Message.response({ error: 'task_id is required' });
        }

        const task = this.tasks.get(taskId);
        if (!task) {
            return Message.response({ error: `task not found: ${taskId}` });
        }

        task.enabled = false;
        this.saveState();

        return Message.response({
            action: 'disabled',
            task_id: taskId,
            name: task.name
        });
    }

    private handleStatus(payload: Record<string, any>): Message {
        const taskId = payload.task_id;
        if (!taskId) {
            return Message.response({ error: 'task_id is required' });
        }

        const task = this.tasks.get(taskId);
        if (!task) {
            return Message.response({ error: `task not found: ${taskId}` });
        }

        return Message.response({
            action: 'status',
            task_id: taskId,
            name: task.name,
            cron: task.cron,
            action_type: task.action,
            enabled: task.enabled,
            last_run: task.lastRun,
            next_run: task.nextRun,
            run_count: task.runCount,
            error_count: task.errorCount,
            created_at: task.createdAt
        });
    }

    private handleList(): Message {
        const tasks = Array.from(this.tasks.values()).map(t => ({
            id: t.id,
            name: t.name,
            cron: t.cron,
            enabled: t.enabled,
            next_run: t.nextRun,
            run_count: t.runCount
        }));

        return Message.response({
            action: 'list',
            tasks,
            count: tasks.length
        });
    }

    private handleHistory(payload: Record<string, any>): Message {
        const limit = payload.limit || 20;
        const taskId = payload.task_id;

        let executions = this.executions;
        if (taskId) {
            executions = executions.filter(e => e.taskId === taskId);
        }

        executions = executions.slice(0, limit);

        return Message.response({
            action: 'history',
            executions,
            count: executions.length
        });
    }

    private async handleRunNow(payload: Record<string, any>): Promise<Message> {
        const taskId = payload.task_id;
        if (!taskId) {
            return Message.response({ error: 'task_id is required' });
        }

        const task = this.tasks.get(taskId);
        if (!task) {
            return Message.response({ error: `task not found: ${taskId}` });
        }

        await this.executeTask(task);

        return Message.response({
            action: 'run_now',
            task_id: taskId,
            name: task.name,
            last_run: task.lastRun
        });
    }

    private handleClearHistory(): Message {
        const count = this.executions.length;
        this.executions = [];
        this.saveState();

        return Message.response({
            action: 'history_cleared',
            cleared_count: count
        });
    }

    private async loadState(): Promise<void> {
        try {
            const data = await this.state.read(this.stateKey);
            if (data) {
                const state = JSON.parse(data);
                if (state.tasks) {
                    for (const [id, task] of Object.entries(state.tasks)) {
                        this.tasks.set(id, task as ScheduledTask);
                    }
                }
                if (state.executions) {
                    this.executions = state.executions;
                }
            }
        } catch (error) {
            console.error(`[${this.name}] Failed to load state:`, error);
        }
    }

    private async saveState(): Promise<void> {
        const state = {
            tasks: Object.fromEntries(this.tasks),
            executions: this.executions
        };
        await this.state.write(this.stateKey, JSON.stringify(state));
    }
}

// ============================================
// Main Entry Point
// ============================================

async function main(): Promise<void> {
    const actor = new SchedulerActor();

    process.on('SIGINT', async () => {
        console.log('\nShutting down scheduler actor...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    console.log('Cron patterns: minute hour day month weekday');
    console.log('Examples: "* * * * *" (every minute), "*/5 * * * *" (every 5 min)');
    console.log('Actions: schedule, cancel, enable, disable, status, list, history, run_now');

    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
