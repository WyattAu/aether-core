// @jest/tag:e2e
/**
 * E2E Scenario 1: E-Commerce Order Processing
 *
 * Simulates a complete e-commerce flow with saga-based compensation:
 * - Order Actor manages order state (Created -> Paid -> Fulfilled -> Shipped)
 * - Payment Actor processes payments (succeeds or fails)
 * - Inventory Actor manages stock levels
 * - Shipping Actor schedules shipments
 * - Full saga with compensation on failure
 */

import {
  Actor,
  ActorConfig,
  Message,
  MessageType,
  StateHandle,
} from '../../src';
import {
  CircuitBreaker,
  CircuitBreakerConfig,
  CircuitState,
} from '../../src/resilience';
import {
  RetryPolicy,
  RetryConfig,
  BackoffStrategy,
} from '../../src/resilience';

// ============================================
// Saga Implementation for JS
// ============================================

type SagaStepResult = unknown;

interface SagaStepDef {
  name: string;
  action: (ctx: SagaContext) => Promise<SagaStepResult>;
  compensate?: (ctx: SagaContext) => Promise<void>;
}

interface SagaContext {
  input: unknown;
  state: Record<string, unknown>;
  completedSteps: string[];
}

interface SagaResult {
  status: 'completed' | 'compensated' | 'failed';
  completedSteps: string[];
  compensatedSteps: string[];
  error?: string;
}

class Saga {
  private steps: SagaStepDef[] = [];
  private currentStep: SagaStepDef | null = null;

  constructor(public readonly name: string) {}

  step(name: string): Saga {
    const step: SagaStepDef = {
      name,
      action: async () => {},
    };
    this.steps.push(step);
    this.currentStep = step;
    return this;
  }

  action(fn: (ctx: SagaContext) => Promise<SagaStepResult>): Saga {
    if (!this.currentStep) throw new Error('No step defined');
    this.currentStep.action = fn;
    return this;
  }

  compensate(fn: (ctx: SagaContext) => Promise<void>): Saga {
    if (!this.currentStep) throw new Error('No step defined');
    this.currentStep.compensate = fn;
    return this;
  }

  build(): Saga {
    for (const step of this.steps) {
      if (!step.action) throw new Error(`Step '${step.name}' has no action`);
    }
    return this;
  }

  getSteps(): SagaStepDef[] {
    return [...this.steps];
  }

  getStep(name: string): SagaStepDef | undefined {
    return this.steps.find((s) => s.name === name);
  }
}

class SagaExecutor {
  async execute(saga: Saga, input: unknown): Promise<SagaResult> {
    const ctx: SagaContext = {
      input,
      state: {},
      completedSteps: [],
    };

    try {
      for (const step of saga.getSteps()) {
        const result = await step.action(ctx);
        ctx.state[`step_${step.name}_result`] = result;
        ctx.completedSteps.push(step.name);
      }

      return {
        status: 'completed',
        completedSteps: [...ctx.completedSteps],
        compensatedSteps: [],
      };
    } catch (error) {
      await this.compensate(saga, ctx);
      return {
        status: ctx.completedSteps.length > 0 ? 'compensated' : 'failed',
        completedSteps: [...ctx.completedSteps],
        compensatedSteps: [...ctx.completedSteps],
        error: String(error),
      };
    }
  }

  private async compensate(saga: Saga, ctx: SagaContext): Promise<void> {
    const reversed = [...ctx.completedSteps].reverse();
    for (const stepName of reversed) {
      const step = saga.getStep(stepName);
      if (step?.compensate) {
        await step.compensate(ctx);
      }
    }
  }
}

// ============================================
// Tests
// ============================================

describe('E2E: E-Commerce Order Processing', () => {
  function buildOrderSaga(
    inventory: Record<string, number>,
    auditLog: string[],
    options: { forcePaymentFail?: boolean; forceShippingFail?: boolean } = {},
  ): Saga {
    return new Saga('order-processing')
      .step('validate-order')
      .action(async (ctx) => {
        const order = ctx.input as { order_id: string; items: { product: string; quantity: number }[] };
        auditLog.push(`VALIDATE: order ${order.order_id}`);
        if (!order.items || order.items.length === 0) throw new Error('Empty order');
        return { order_id: order.order_id };
      })
      .step('reserve-inventory')
      .action(async (ctx) => {
        const order = ctx.input as { order_id: string; items: { product: string; quantity: number }[] };
        for (const item of order.items) {
          if ((inventory[item.product] ?? 0) < item.quantity) {
            throw new Error(`Insufficient stock: ${item.product}`);
          }
          inventory[item.product] -= item.quantity;
        }
        auditLog.push(`INVENTORY: Reserved for ${order.order_id}`);
        return { reserved: true };
      })
      .compensate(async (ctx) => {
        const order = ctx.input as { order_id: string; items: { product: string; quantity: number }[] };
        for (const item of order.items) {
          inventory[item.product] = (inventory[item.product] ?? 0) + item.quantity;
        }
        auditLog.push('INVENTORY: RELEASED');
      })
      .step('process-payment')
      .action(async (ctx) => {
        if (options.forcePaymentFail) {
          auditLog.push('PAYMENT: FAILED');
          throw new Error('Payment declined');
        }
        auditLog.push('PAYMENT: SUCCESS');
        return { transaction_id: 'txn-123' };
      })
      .compensate(async () => {
        auditLog.push('PAYMENT: REFUNDED');
      })
      .step('schedule-shipping')
      .action(async (ctx) => {
        if (options.forceShippingFail) {
          auditLog.push('SHIPPING: FAILED');
          throw new Error('Carrier unavailable');
        }
        auditLog.push('SHIPPING: SCHEDULED');
        return { tracking: 'TRACK-001' };
      })
      .compensate(async () => {
        auditLog.push('SHIPPING: CANCELLED');
      })
      .build();
  }

  test('complete order flow: order -> paid -> shipped', async () => {
    const inventory: Record<string, number> = { widget: 100, gadget: 50 };
    const auditLog: string[] = [];
    const saga = buildOrderSaga(inventory, auditLog);
    const executor = new SagaExecutor();

    const result = await executor.execute(saga, {
      order_id: 'ORD-JS-001',
      items: [{ product: 'widget', quantity: 5 }],
    });

    expect(result.status).toBe('completed');
    expect(result.completedSteps).toEqual([
      'validate-order',
      'reserve-inventory',
      'process-payment',
      'schedule-shipping',
    ]);
    expect(inventory.widget).toBe(95);
    expect(auditLog).toContain('PAYMENT: SUCCESS');
    expect(auditLog).toContain('SHIPPING: SCHEDULED');

    console.log('\n=== Complete Order Flow (JS) ===');
    console.log(`  Order: ORD-JS-001`);
    console.log(`  Status: ${result.status}`);
    console.log(`  Inventory: widget=${inventory.widget}, gadget=${inventory.gadget}`);
  });

  test('payment failure triggers compensation', async () => {
    const inventory: Record<string, number> = { widget: 100 };
    const auditLog: string[] = [];
    const saga = buildOrderSaga(inventory, auditLog, { forcePaymentFail: true });
    const executor = new SagaExecutor();

    const result = await executor.execute(saga, {
      order_id: 'ORD-JS-002',
      items: [{ product: 'widget', quantity: 3 }],
    });

    expect(result.status).toBe('compensated');
    expect(result.completedSteps).toEqual(['validate-order', 'reserve-inventory']);
    expect(auditLog).toContain('INVENTORY: RELEASED');
    expect(auditLog).not.toContain('SHIPPING: SCHEDULED');
    expect(inventory.widget).toBe(100);

    console.log('\n=== Payment Failure (JS) ===');
    console.log(`  Status: ${result.status}`);
    console.log(`  Compensation: inventory released`);
  });

  test('shipping failure triggers full rollback', async () => {
    const inventory: Record<string, number> = { gadget: 50 };
    const auditLog: string[] = [];
    const saga = buildOrderSaga(inventory, auditLog, { forceShippingFail: true });
    const executor = new SagaExecutor();

    const result = await executor.execute(saga, {
      order_id: 'ORD-JS-003',
      items: [{ product: 'gadget', quantity: 4 }],
    });

    expect(result.status).toBe('compensated');
    expect(result.completedSteps).toContain('process-payment');
    expect(auditLog).toContain('PAYMENT: REFUNDED');
    expect(auditLog).toContain('INVENTORY: RELEASED');
    expect(auditLog).toContain('SHIPPING: FAILED');
    expect(inventory.gadget).toBe(50);

    console.log('\n=== Shipping Failure Rollback (JS) ===');
    console.log(`  Status: ${result.status}`);
    console.log(`  Full compensation: payment refunded, inventory restored, shipping cancelled`);
  });

  test('multiple items in single order', async () => {
    const inventory: Record<string, number> = { widget: 100, gadget: 50, doohickey: 25 };
    const auditLog: string[] = [];
    const saga = buildOrderSaga(inventory, auditLog);
    const executor = new SagaExecutor();

    const result = await executor.execute(saga, {
      order_id: 'ORD-JS-004',
      items: [
        { product: 'widget', quantity: 10 },
        { product: 'gadget', quantity: 5 },
        { product: 'doohickey', quantity: 2 },
      ],
    });

    expect(result.status).toBe('completed');
    expect(inventory.widget).toBe(90);
    expect(inventory.gadget).toBe(45);
    expect(inventory.doohickey).toBe(23);

    console.log('\n=== Multi-Item Order (JS) ===');
    console.log(`  Status: ${result.status}`);
    console.log(`  Inventory: ${JSON.stringify(inventory)}`);
  });

  test('actor-based order processing with StateHandle', async () => {
    class OrderActor extends Actor {
      private processedOrders: string[] = [];

      constructor() {
        super({ name: 'order-actor' });
      }

      static override get name(): string {
        return 'order-actor';
      }

      async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.payload.action === 'create') {
          const order = message.payload.order;
          await this.state.setJSON(`order:${order.id}`, {
            id: order.id,
            status: 'created',
            items: order.items,
          });
          this.processedOrders.push(order.id);
          return Message.custom({ status: 'created' });
        }
      }

      getProcessedOrders(): string[] {
        return this.processedOrders;
      }

      getState() {
        return this.state;
      }
    }

    class PaymentActor extends Actor {
      constructor() {
        super({ name: 'payment-actor' });
      }

      static override get name(): string {
        return 'payment-actor';
      }

      async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.payload.action === 'charge') {
          const { order_id, amount } = message.payload;
          await this.state.setJSON(`payment:txn-${order_id}`, {
            order_id,
            amount,
            status: 'completed',
          });
          return Message.custom({ txn_id: `txn-${order_id}`, status: 'success' });
        }
      }
    }

    const orderActor = new OrderActor();
    const paymentActor = new PaymentActor();

    const orderResp = await orderActor.handle('customer', Message.custom({
      action: 'create',
      order: { id: 'ORD-ACTOR-001', items: ['widget'] },
    }));
    expect(orderResp?.payload.status).toBe('created');
    expect(orderActor.getProcessedOrders()).toContain('ORD-ACTOR-001');

    const savedOrder = await orderActor.getState().getJSON<{ status: string }>(`order:ORD-ACTOR-001`);
    expect(savedOrder?.status).toBe('created');

    const paymentResp = await paymentActor.handle('order-actor', Message.custom({
      action: 'charge',
      order_id: 'ORD-ACTOR-001',
      amount: 49.99,
    }));
    expect(paymentResp?.payload.status).toBe('success');

    console.log('\n=== Actor Message-Based Order (JS) ===');
    console.log(`  Order created and persisted`);
    console.log(`  Payment completed`);
  });

  test('circuit breaker for payment service', async () => {
    const cb = new CircuitBreaker({
      failureThreshold: 3,
      resetTimeout: 100,
    });

    let callCount = 0;
    async function flakyPayment(): Promise<string> {
      callCount++;
      if (callCount <= 3) throw new Error('Payment service down');
      return 'ok';
    }

    for (let i = 0; i < 3; i++) {
      try {
        await cb.execute(flakyPayment);
      } catch {
        // expected
      }
    }

    expect(cb.getState()).toBe(CircuitState.Open);

    try {
      await cb.execute(flakyPayment);
    } catch {
      // expected - circuit open
    }

    console.log('\n=== Circuit Breaker Payment (JS) ===');
    console.log(`  State after 3 failures: ${cb.getState()}`);
    console.log(`  Stats: ${JSON.stringify(cb.getStats())}`);
  });

  test('retry policy for resilient order submission', async () => {
    let attempts = 0;

    async function submitOrder(): Promise<string> {
      attempts++;
      if (attempts < 3) throw new Error('ETIMEDOUT: Service unavailable');
      return 'order-submitted';
    }

    const retry = new RetryPolicy({
      maxAttempts: 5,
      initialDelay: 10,
      maxDelay: 100,
      strategy: BackoffStrategy.ExponentialJitter,
    });

    const result = await retry.execute(submitOrder);

    expect(result.success).toBe(true);
    expect(result.attempts).toBe(3);

    console.log('\n=== Retry Order Submission (JS) ===');
    console.log(`  Attempts: ${result.attempts}`);
    console.log(`  Total time: ${result.totalTime}ms`);
  });
});
