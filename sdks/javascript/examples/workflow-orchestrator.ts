/**
 * Distributed Workflow Orchestrator Example (TypeScript)
 *
 * Demonstrates:
 * - Custom state machine with transitions map
 * - Actor-based saga steps with compensation
 * - Human task integration (simulated review)
 * - StateHandle for persistence / audit trail
 *
 * Since the JS SDK does not have dedicated workflow modules, this example
 * builds a simplified workflow orchestrator using the core Actor, StateHandle,
 * and Messaging primitives.
 */

import { Actor } from '../src/actor';
import { Message, MessageType } from '../src/messaging';
import { StateHandle } from '../src/state';

// ========================================
// Types
// ========================================

interface WorkflowState {
  currentState: string;
  input: Record<string, any>;
  history: AuditEntry[];
  variables: Record<string, any>;
  startedAt: string;
  updatedAt: string;
}

interface AuditEntry {
  type: string;
  timestamp: string;
  details: Record<string, any>;
}

interface SagaStepDefinition {
  name: string;
  action: (ctx: SagaContext) => Promise<any>;
  compensate?: (ctx: SagaContext) => Promise<void>;
}

interface SagaContext {
  input: Record<string, any>;
  state: Record<string, any>;
  completedSteps: string[];
  failedStep?: string;
  error?: string;
  set(key: string, value: any): void;
  get(key: string): any;
  markCompleted(step: string): void;
}

interface SagaResult {
  status: 'completed' | 'compensated' | 'failed';
  completedSteps: string[];
  compensatedSteps: string[];
  error?: string;
  durationMs: number;
}

interface HumanTaskDef {
  taskId: string;
  taskType: string;
  title: string;
  assignee: string;
  status: 'pending' | 'in_progress' | 'completed' | 'rejected';
  result?: Record<string, any>;
  completedBy?: string;
}

// ========================================
// Utilities
// ========================================

function uuid(): string {
  return Math.random().toString(36).substring(2, 10);
}

const AUDIT_LOG: string[] = [];

function log(msg: string): void {
  const ts = new Date().toISOString().substring(11, 23);
  const entry = `  [${ts}] ${msg}`;
  AUDIT_LOG.push(entry);
  console.log(entry);
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// ========================================
// State Machine
// ========================================

interface TransitionDef {
  name: string;
  from: string;
  to: string;
}

class StateMachine {
  private transitions: Map<string, TransitionDef[]> = new Map();

  addTransition(transition: TransitionDef): this {
    const fromMap = this.transitions.get(transition.from) ?? [];
    fromMap.push(transition);
    this.transitions.set(transition.from, fromMap);
    return this;
  }

  getAvailableTransitions(currentState: string): string[] {
    return (this.transitions.get(currentState) ?? []).map(t => t.name);
  }

  execute(transitionName: string, currentState: string): string | null {
    const candidates = this.transitions.get(currentState) ?? [];
    const transition = candidates.find(t => t.name === transitionName);
    return transition?.to ?? null;
  }
}

// ========================================
// Saga Executor
// ========================================

class SagaExecutor {
  async execute(
    steps: SagaStepDefinition[],
    input: Record<string, any>,
  ): Promise<SagaResult> {
    const startTime = Date.now();
    const ctx: SagaContext = {
      input,
      state: {},
      completedSteps: [],
      set(key, value) { this.state[key] = value; },
      get(key) { return this.state[key]; },
      markCompleted(step) {
        if (!this.completedSteps.includes(step)) this.completedSteps.push(step);
      },
    };

    try {
      for (const step of steps) {
        log(`  [SAGA] Executing step: ${step.name}`);
        await step.action(ctx);
        ctx.markCompleted(step.name);
        log(`  [SAGA] Step completed: ${step.name}`);
      }

      return {
        status: 'completed',
        completedSteps: ctx.completedSteps,
        compensatedSteps: [],
        durationMs: Date.now() - startTime,
      };
    } catch (err: any) {
      ctx.failedStep = steps.find(s => !ctx.completedSteps.includes(s.name))?.name;
      ctx.error = err?.message ?? String(err);
      log(`  [SAGA] Step failed: ${ctx.failedStep}: ${ctx.error}`);

      const completedReversed = [...ctx.completedSteps].reverse();
      const compensatedSteps: string[] = [];

      for (const stepName of completedReversed) {
        const step = steps.find(s => s.name === stepName);
        if (step?.compensate) {
          log(`  [COMPENSATE] Compensating: ${stepName}`);
          try {
            await step.compensate(ctx);
            compensatedSteps.push(stepName);
            log(`  [COMPENSATE] Compensated: ${stepName}`);
          } catch (compErr: any) {
            log(`  [COMPENSATE] Compensation failed for ${stepName}: ${compErr.message}`);
          }
        }
      }

      return {
        status: compensatedSteps.length > 0 ? 'compensated' : 'failed',
        completedSteps: ctx.completedSteps,
        compensatedSteps,
        error: ctx.error,
        durationMs: Date.now() - startTime,
      };
    }
  }
}

// ========================================
// Workflow Executor
// ========================================

class WorkflowExecutor {
  private stateMachine = new StateMachine();
  private workflows = new Map<string, WorkflowState>();
  private humanTasks = new Map<string, HumanTaskDef>();

  addTransition(def: TransitionDef): this {
    this.stateMachine.addTransition(def);
    return this;
  }

  async start(
    input: Record<string, any>,
    initialState: string,
  ): Promise<string> {
    const wfId = uuid();
    this.workflows.set(wfId, {
      currentState: initialState,
      input,
      history: [],
      variables: {},
      startedAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    });
    this.addAudit(wfId, 'workflow_started', { state: initialState });
    log(`Workflow started: ${wfId}, state=${initialState}`);
    return wfId;
  }

  async transition(wfId: string, transitionName: string): Promise<string | null> {
    const wf = this.workflows.get(wfId);
    if (!wf) throw new Error(`Unknown workflow: ${wfId}`);

    const newState = this.stateMachine.execute(transitionName, wf.currentState);
    if (newState === null) {
      log(`Invalid transition '${transitionName}' from '${wf.currentState}'`);
      return null;
    }

    const oldState = wf.currentState;
    wf.currentState = newState;
    wf.updatedAt = new Date().toISOString();
    this.addAudit(wfId, 'transition', { transition: transitionName, from: oldState, to: newState });
    log(`Transition: ${oldState} -> ${newState} (via ${transitionName})`);
    return newState;
  }

  getStatus(wfId: string): WorkflowState | undefined {
    return this.workflows.get(wfId);
  }

  createHumanTask(
    taskType: string,
    title: string,
    assignee: string,
    workflowId: string,
  ): HumanTaskDef {
    const task: HumanTaskDef = {
      taskId: uuid(),
      taskType,
      title,
      assignee,
      status: 'pending',
    };
    this.humanTasks.set(task.taskId, task);
    log(`Human task created: ${task.taskId} (${taskType}) assigned to ${assignee}`);
    return task;
  }

  async completeHumanTask(taskId: string, result: Record<string, any>): Promise<HumanTaskDef> {
    const task = this.humanTasks.get(taskId);
    if (!task) throw new Error(`Task not found: ${taskId}`);
    task.status = 'completed';
    task.result = result;
    task.completedBy = task.assignee;
    log(`Human task completed: ${taskId} by ${task.completedBy} => ${JSON.stringify(result)}`);
    return task;
  }

  private addAudit(wfId: string, type: string, details: Record<string, any>): void {
    const wf = this.workflows.get(wfId);
    if (wf) {
      wf.history.push({
        type,
        timestamp: new Date().toISOString(),
        details,
      });
    }
  }
}

// ========================================
// Main
// ========================================

async function runWorkflowOrchestrator(): Promise<void> {
  console.log('='.repeat(70));
  console.log('  AETHER WORKFLOW ORCHESTRATOR - Document Approval Example (TS)');
  console.log('='.repeat(70));
  console.log();

  const executor = new WorkflowExecutor();
  const sagaExec = new SagaExecutor();

  console.log('--- Step 1: Define the approval state machine ---');
  executor
    .addTransition({ name: 'submit', from: 'draft', to: 'pending_review' })
    .addTransition({ name: 'approve', from: 'pending_review', to: 'approved' })
    .addTransition({ name: 'reject', from: 'pending_review', to: 'rejected' })
    .addTransition({ name: 'saga_success', from: 'approved', to: 'completed' })
    .addTransition({ name: 'saga_failed', from: 'approved', to: 'failed' });
  console.log('  States: draft, pending_review, approved, rejected, completed, failed');
  console.log('  Transitions defined successfully');
  console.log();

  console.log('--- Step 2: Define the post-approval saga ---');

  const sagaSteps: SagaStepDefinition[] = [
    {
      name: 'notify-team',
      action: async (ctx) => {
        log('  [SAGA] Notifying team about approved document...');
        await sleep(50);
        ctx.set('notification_sent', true);
        log('  [SAGA] Team notification sent');
        return 'ok';
      },
      compensate: async (ctx) => {
        log('  [COMPENSATE] Withdrawing team notification...');
        ctx.set('notification_sent', false);
        log('  [COMPENSATE] Notification withdrawn');
      },
    },
    {
      name: 'archive-document',
      action: async (ctx) => {
        log('  [SAGA] Archiving document to storage...');
        await sleep(50);
        if (ctx.input.simulateFailure) {
          throw new Error('Storage service unavailable during archival');
        }
        ctx.set('archived', true);
        log('  [SAGA] Document archived');
        return 'ok';
      },
      compensate: async (ctx) => {
        log('  [COMPENSATE] Un-archiving document...');
        ctx.set('archived', false);
        log('  [COMPENSATE] Document un-archived');
      },
    },
    {
      name: 'send-confirmation',
      action: async (ctx) => {
        log('  [SAGA] Sending confirmation email...');
        await sleep(50);
        ctx.set('confirmation_sent', true);
        log('  [SAGA] Confirmation sent');
        return 'ok';
      },
      compensate: async (ctx) => {
        log('  [COMPENSATE] Withdrawing confirmation...');
        ctx.set('confirmation_sent', false);
        log('  [COMPENSATE] Confirmation withdrawn');
      },
    },
  ];
  console.log(`  Saga: ${sagaSteps.length} steps with compensation`);
  sagaSteps.forEach(s => {
    console.log(`    - ${s.name} (compensation: ${s.compensate ? 'yes' : 'no'})`);
  });
  console.log();

  for (const [runNum, shouldFail] of [[1, false], [2, true]] as [number, boolean][]) {
    const docTitle = `Q4 Report ${runNum}${shouldFail ? ' (failure scenario)' : ''}`;
    console.log('='.repeat(70));
    console.log(`  RUN ${runNum}: Document '${docTitle}'`);
    console.log('='.repeat(70));

    console.log('\n  >> Starting workflow...');
    const wfId = await executor.start({ title: docTitle, simulateFailure: shouldFail }, 'draft');

    console.log('\n  >> Transition: draft -> pending_review');
    await executor.transition(wfId, 'submit');

    console.log('\n  >> Creating human review task...');
    const task = executor.createHumanTask(
      'document_review',
      `Review: ${docTitle}`,
      'reviewer@company.com',
      wfId,
    );

    await sleep(30);

    console.log('\n  >> Human reviewer approves the document...');
    await executor.completeHumanTask(task.taskId, {
      approved: true,
      comments: 'Looks good, approved.',
    });

    console.log('\n  >> Transition: pending_review -> approved');
    await executor.transition(wfId, 'approve');

    console.log('\n  >> Executing post-approval saga...');
    const sagaResult = await sagaExec.execute(sagaSteps, executor.getStatus(wfId)!.input);
    log(`Saga result: status=${sagaResult.status}`);
    log(`  Completed steps: [${sagaResult.completedSteps.join(', ')}]`);
    if (sagaResult.compensatedSteps.length > 0) {
      log(`  Compensated steps: [${sagaResult.compensatedSteps.join(', ')}]`);
    }
    if (sagaResult.error) {
      log(`  Error: ${sagaResult.error}`);
    }
    log(`  Duration: ${sagaResult.durationMs}ms`);

    if (sagaResult.status === 'completed') {
      console.log('\n  >> Transition: approved -> completed');
      await executor.transition(wfId, 'saga_success');
    } else {
      console.log('\n  >> Transition: approved -> failed');
      await executor.transition(wfId, 'saga_failed');
    }

    const finalState = executor.getStatus(wfId)!;
    log(`Final state: ${finalState.currentState}`);

    console.log('\n  >> Audit trail:');
    for (const entry of finalState.history) {
      const detailStr = Object.entries(entry.details)
        .map(([k, v]) => `${k}=${v}`)
        .join(', ');
      log(`  AUDIT: ${entry.type} (${detailStr})`);
    }
    console.log();
  }

  console.log('='.repeat(70));
  console.log('  FULL AUDIT LOG');
  console.log('='.repeat(70));
  for (const entry of AUDIT_LOG) {
    console.log(entry);
  }
  console.log(`\nTotal audit entries: ${AUDIT_LOG.length}`);
}

(async () => {
  await runWorkflowOrchestrator();
})();
