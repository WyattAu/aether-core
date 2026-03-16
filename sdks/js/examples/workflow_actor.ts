/**
 * Workflow Actor Example
 * 
 * Demonstrates a workflow orchestration pattern with step execution,
 * state management, and error handling.
 */
import { Actor, Message, MessageType, State } from '@aether/sdk';

// ============================================
// Types
// ============================================

interface WorkflowStep {
    id: string;
    name: string;
    status: 'pending' | 'running' | 'completed' | 'failed' | 'skipped';
    startedAt?: string;
    completedAt?: string;
    error?: string;
}

interface Workflow {
    id: string;
    name: string;
    status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
    steps: WorkflowStep[];
    currentStep: number;
    createdAt: string;
    updatedAt: string;
}

interface WorkflowDefinition {
    name: string;
    steps: string[];
}

// ============================================
// Workflow Actor
// ============================================

class WorkflowActor extends Actor {
    private workflows: Map<string, Workflow> = new Map();
    private definitions: Map<string, WorkflowDefinition> = new Map();
    private state: State;
    private stateKey: string;

    constructor() {
        super('workflow-actor');
        this.state = new State();
        this.stateKey = 'workflow_state';
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG', 'TIME');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Workflow Actor starting...`);
        
        // Load persisted state
        await this.loadState();
        
        // Register default workflow definitions
        this.registerDefaultWorkflows();
        
        console.log(`[${this.name}] Loaded ${this.workflows.size} workflows, ${this.definitions.size} definitions`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Workflow Actor stopping, saving state...`);
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
            case 'create':
                return this.handleCreate(payload);
            case 'start':
                return this.handleStart(payload);
            case 'cancel':
                return this.handleCancel(payload);
            case 'status':
                return this.handleStatus(payload);
            case 'list':
                return this.handleList();
            case 'step_complete':
                return this.handleStepComplete(payload);
            case 'step_fail':
                return this.handleStepFail(payload);
            case 'register_definition':
                return this.handleRegisterDefinition(payload);
            case 'list_definitions':
                return this.handleListDefinitions();
            default:
                return Message.response({ error: `unknown action: ${action}` });
        }
    }

    private registerDefaultWorkflows(): void {
        // Simple deployment workflow
        this.definitions.set('deploy', {
            name: 'Deployment Workflow',
            steps: ['build', 'test', 'security-scan', 'deploy', 'verify']
        });

        // Data processing workflow
        this.definitions.set('data-pipeline', {
            name: 'Data Pipeline',
            steps: ['extract', 'transform', 'validate', 'load', 'notify']
        });

        // CI/CD workflow
        this.definitions.set('cicd', {
            name: 'CI/CD Pipeline',
            steps: ['checkout', 'install', 'lint', 'test', 'build', 'deploy-staging', 'test-staging', 'deploy-production']
        });
    }

    private handleCreate(payload: Record<string, any>): Message {
        const definitionId = payload.definition_id || 'deploy';
        const workflowId = payload.workflow_id || `wf-${Date.now()}`;

        const definition = this.definitions.get(definitionId);
        if (!definition) {
            return Message.response({ error: `definition not found: ${definitionId}` });
        }

        const steps: WorkflowStep[] = definition.steps.map(stepId => ({
            id: stepId,
            name: this.formatStepName(stepId),
            status: 'pending'
        }));

        const workflow: Workflow = {
            id: workflowId,
            name: payload.name || definition.name,
            status: 'pending',
            steps: steps,
            currentStep: -1,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString()
        };

        this.workflows.set(workflowId, workflow);
        this.saveState();

        console.log(`[${this.name}] Created workflow '${workflowId}' with ${steps.length} steps`);

        return Message.response({
            action: 'created',
            workflow_id: workflowId,
            name: workflow.name,
            step_count: steps.length,
            status: workflow.status
        });
    }

    private handleStart(payload: Record<string, any>): Message {
        const workflowId = payload.workflow_id;
        if (!workflowId) {
            return Message.response({ error: 'workflow_id required' });
        }

        const workflow = this.workflows.get(workflowId);
        if (!workflow) {
            return Message.response({ error: `workflow not found: ${workflowId}` });
        }

        if (workflow.status !== 'pending') {
            return Message.response({ error: `workflow already ${workflow.status}` });
        }

        workflow.status = 'running';
        workflow.currentStep = 0;
        workflow.updatedAt = new Date().toISOString();

        if (workflow.steps.length > 0) {
            workflow.steps[0].status = 'running';
            workflow.steps[0].startedAt = new Date().toISOString();
        }

        this.saveState();

        console.log(`[${this.name}] Started workflow '${workflowId}'`);

        return Message.response({
            action: 'started',
            workflow_id: workflowId,
            current_step: workflow.steps[0]?.id,
            status: workflow.status
        });
    }

    private handleCancel(payload: Record<string, any>): Message {
        const workflowId = payload.workflow_id;
        if (!workflowId) {
            return Message.response({ error: 'workflow_id required' });
        }

        const workflow = this.workflows.get(workflowId);
        if (!workflow) {
            return Message.response({ error: `workflow not found: ${workflowId}` });
        }

        workflow.status = 'cancelled';
        workflow.updatedAt = new Date().toISOString();

        // Mark all pending/running steps as skipped
        for (const step of workflow.steps) {
            if (step.status === 'pending' || step.status === 'running') {
                step.status = 'skipped';
            }
        }

        this.saveState();

        console.log(`[${this.name}] Cancelled workflow '${workflowId}'`);

        return Message.response({
            action: 'cancelled',
            workflow_id: workflowId,
            status: workflow.status
        });
    }

    private handleStatus(payload: Record<string, any>): Message {
        const workflowId = payload.workflow_id;
        if (!workflowId) {
            return Message.response({ error: 'workflow_id required' });
        }

        const workflow = this.workflows.get(workflowId);
        if (!workflow) {
            return Message.response({ error: `workflow not found: ${workflowId}` });
        }

        return Message.response({
            action: 'status',
            workflow_id: workflowId,
            name: workflow.name,
            status: workflow.status,
            current_step: workflow.currentStep,
            steps: workflow.steps.map(s => ({
                id: s.id,
                name: s.name,
                status: s.status
            })),
            created_at: workflow.createdAt,
            updated_at: workflow.updatedAt
        });
    }

    private handleList(): Message {
        const workflows = Array.from(this.workflows.values()).map(w => ({
            id: w.id,
            name: w.name,
            status: w.status,
            step_count: w.steps.length,
            created_at: w.createdAt
        }));

        return Message.response({
            action: 'list',
            workflows: workflows,
            count: workflows.length
        });
    }

    private handleStepComplete(payload: Record<string, any>): Message {
        const workflowId = payload.workflow_id;
        const stepId = payload.step_id;

        if (!workflowId || !stepId) {
            return Message.response({ error: 'workflow_id and step_id required' });
        }

        const workflow = this.workflows.get(workflowId);
        if (!workflow) {
            return Message.response({ error: `workflow not found: ${workflowId}` });
        }

        const step = workflow.steps.find(s => s.id === stepId);
        if (!step) {
            return Message.response({ error: `step not found: ${stepId}` });
        }

        step.status = 'completed';
        step.completedAt = new Date().toISOString();

        // Move to next step
        workflow.currentStep++;
        workflow.updatedAt = new Date().toISOString();

        if (workflow.currentStep >= workflow.steps.length) {
            // All steps complete
            workflow.status = 'completed';
            console.log(`[${this.name}] Workflow '${workflowId}' completed`);
        } else {
            // Start next step
            const nextStep = workflow.steps[workflow.currentStep];
            nextStep.status = 'running';
            nextStep.startedAt = new Date().toISOString();
            console.log(`[${this.name}] Workflow '${workflowId}' moved to step '${nextStep.id}'`);
        }

        this.saveState();

        return Message.response({
            action: 'step_complete',
            workflow_id: workflowId,
            step_id: stepId,
            next_step: workflow.currentStep < workflow.steps.length 
                ? workflow.steps[workflow.currentStep].id 
                : null,
            workflow_status: workflow.status
        });
    }

    private handleStepFail(payload: Record<string, any>): Message {
        const workflowId = payload.workflow_id;
        const stepId = payload.step_id;
        const error = payload.error || 'Unknown error';

        if (!workflowId || !stepId) {
            return Message.response({ error: 'workflow_id and step_id required' });
        }

        const workflow = this.workflows.get(workflowId);
        if (!workflow) {
            return Message.response({ error: `workflow not found: ${workflowId}` });
        }

        const step = workflow.steps.find(s => s.id === stepId);
        if (!step) {
            return Message.response({ error: `step not found: ${stepId}` });
        }

        step.status = 'failed';
        step.error = error;
        step.completedAt = new Date().toISOString();

        // Fail the workflow
        workflow.status = 'failed';
        workflow.updatedAt = new Date().toISOString();

        // Mark remaining steps as skipped
        for (let i = workflow.currentStep + 1; i < workflow.steps.length; i++) {
            workflow.steps[i].status = 'skipped';
        }

        this.saveState();

        console.log(`[${this.name}] Workflow '${workflowId}' failed at step '${stepId}': ${error}`);

        return Message.response({
            action: 'step_failed',
            workflow_id: workflowId,
            step_id: stepId,
            error: error,
            workflow_status: workflow.status
        });
    }

    private handleRegisterDefinition(payload: Record<string, any>): Message {
        const definitionId = payload.definition_id;
        const name = payload.name;
        const steps = payload.steps;

        if (!definitionId || !name || !steps || !Array.isArray(steps)) {
            return Message.response({ error: 'definition_id, name, and steps[] required' });
        }

        this.definitions.set(definitionId, { name, steps });
        this.saveState();

        console.log(`[${this.name}] Registered workflow definition '${definitionId}'`);

        return Message.response({
            action: 'definition_registered',
            definition_id: definitionId,
            step_count: steps.length
        });
    }

    private handleListDefinitions(): Message {
        const definitions = Array.from(this.definitions.entries()).map(([id, def]) => ({
            id,
            name: def.name,
            step_count: def.steps.length,
            steps: def.steps
        }));

        return Message.response({
            action: 'list_definitions',
            definitions: definitions,
            count: definitions.length
        });
    }

    private formatStepName(stepId: string): string {
        return stepId
            .split('-')
            .map(word => word.charAt(0).toUpperCase() + word.slice(1))
            .join(' ');
    }

    private async loadState(): Promise<void> {
        try {
            const data = await this.state.read(this.stateKey);
            if (data) {
                const state = JSON.parse(data);
                if (state.workflows) {
                    for (const [id, wf] of Object.entries(state.workflows)) {
                        this.workflows.set(id, wf as Workflow);
                    }
                }
                if (state.definitions) {
                    for (const [id, def] of Object.entries(state.definitions)) {
                        this.definitions.set(id, def as WorkflowDefinition);
                    }
                }
            }
        } catch (error) {
            console.error(`[${this.name}] Failed to load state:`, error);
        }
    }

    private async saveState(): Promise<void> {
        const state = {
            workflows: Object.fromEntries(this.workflows),
            definitions: Object.fromEntries(this.definitions)
        };
        await this.state.write(this.stateKey, JSON.stringify(state));
    }
}

// ============================================
// Main Entry Point
// ============================================

async function main(): Promise<void> {
    const actor = new WorkflowActor();

    process.on('SIGINT', async () => {
        console.log('\nShutting down workflow actor...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    console.log('Available workflow definitions: deploy, data-pipeline, cicd');
    console.log('Actions: create, start, cancel, status, list, step_complete, step_fail');

    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
