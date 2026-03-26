/**
 * Event-Sourced Order System using the Aether JS SDK.
 *
 * NOTE: The JavaScript SDK does not yet have event sourcing or saga modules.
 * This example demonstrates the same concepts (event log, state replay,
 * saga-style orchestration with compensation, and validation) using the
 * available APIs: Actor, StateHandle, Message, RetryPolicy, and Validator.
 *
 * Demonstrated concepts:
 *   - Event log for order lifecycle (OrderCreated, PaymentProcessed, Shipped, Delivered)
 *   - State reconstruction from events (replay)
 *   - Saga-style orchestration with compensation on failure
 *   - Schema/event validation via the Validator class
 *
 * Full order lifecycle:
 *     Create -> Pay -> Ship -> Deliver
 *
 * If payment fails, the saga compensates by releasing inventory.
 *
 * Usage:
 *   npx ts-node examples/order-system.ts
 */

import { Actor, Message, MessageType, Capability, StateHandle } from '../src';
import { Validator, validateRequired, validateFloat } from '../src/validation';
import { RetryPolicy, BackoffStrategy } from '../src/resilience';

// -------------------------------------------------------------------
// Types
// -------------------------------------------------------------------

interface OrderItem {
    sku: string;
    name: string;
    qty: number;
    price: number;
}

interface OrderEvent {
    type: string;
    order_id: string;
    timestamp: string;
    [key: string]: any;
}

interface OrderState {
    order_id: string;
    status: 'pending' | 'created' | 'paid' | 'shipped' | 'delivered' | 'cancelled';
    customer_id: string;
    items: OrderItem[];
    total: number;
    payment_id: string | null;
    tracking_number: string | null;
    carrier: string | null;
    delivered_at: string | null;
}

// -------------------------------------------------------------------
// Event validation
// -------------------------------------------------------------------

/**
 * Validate an OrderCreated event using the Aether Validator.
 */
function validateOrderCreated(event: Record<string, any>): string[] {
    const v = new Validator();
    v.required('order_id', event.order_id);
    v.required('customer_id', event.customer_id);
    v.array('items', event.items);
    v.float('total', event.total);
    v.minValue('total', event.total, 0);

    if (!v.isValid()) {
        return Object.entries(v.getErrors()).flatMap(
            ([field, messages]) => messages.map(msg => `${field}: ${msg}`)
        );
    }
    return [];
}

// -------------------------------------------------------------------
// Order aggregate (replay-based)
// -------------------------------------------------------------------

/**
 * Order aggregate that rebuilds its state by replaying events.
 *
 * Mirrors the Python Aggregate pattern using a simple event list.
 */
class OrderAggregate {
    state: OrderState = {
        order_id: '',
        status: 'pending',
        customer_id: '',
        items: [],
        total: 0,
        payment_id: null,
        tracking_number: null,
        carrier: null,
        delivered_at: null,
    };
    private events: OrderEvent[] = [];

    get id(): string {
        return this.state.order_id;
    }

    get version(): number {
        return this.events.length;
    }

    get uncommittedEvents(): OrderEvent[] {
        return [...this.events];
    }

    /**
     * Apply a single event to mutate state.
     */
    applyEvent(event: OrderEvent): void {
        const payload = event;

        switch (event.type) {
            case 'order_created':
                this.state.status = 'created';
                this.state.customer_id = payload.customer_id;
                this.state.items = payload.items;
                this.state.total = payload.total;
                this.state.order_id = payload.order_id;
                break;
            case 'payment_processed':
                this.state.status = 'paid';
                this.state.payment_id = payload.payment_id;
                break;
            case 'order_shipped':
                this.state.status = 'shipped';
                this.state.tracking_number = payload.tracking_number;
                this.state.carrier = payload.carrier;
                break;
            case 'order_delivered':
                this.state.status = 'delivered';
                this.state.delivered_at = payload.delivered_at;
                break;
            case 'order_cancelled':
                this.state.status = 'cancelled';
                break;
            case 'inventory_reserved':
            case 'inventory_released':
                // These affect external services, not order state directly
                break;
        }

        this.events.push(event);
    }

    /**
     * Rebuild state from a list of events (replay).
     */
    loadFromHistory(events: OrderEvent[]): void {
        this.state = {
            order_id: '',
            status: 'pending',
            customer_id: '',
            items: [],
            total: 0,
            payment_id: null,
            tracking_number: null,
            carrier: null,
            delivered_at: null,
        };
        this.events = [];
        for (const event of events) {
            this.applyEvent(event);
        }
    }

    toString(): string {
        return `Order(id='${this.state.order_id}', status='${this.state.status}', total=${this.state.total}, items=${this.state.items.length})`;
    }
}

// -------------------------------------------------------------------
// In-memory event store
// -------------------------------------------------------------------

class InMemoryEventStore {
    private streams: Map<string, OrderEvent[]> = new Map();

    async append(aggregateId: string, events: OrderEvent[]): Promise<number> {
        if (!this.streams.has(aggregateId)) {
            this.streams.set(aggregateId, []);
        }
        const stream = this.streams.get(aggregateId)!;
        stream.push(...events);
        return stream.length;
    }

    async getEvents(aggregateId: string): Promise<OrderEvent[]> {
        return this.streams.get(aggregateId) ?? [];
    }
}

// -------------------------------------------------------------------
// Saga executor (simplified)
// -------------------------------------------------------------------

interface SagaStep {
    name: string;
    action: (ctx: SagaContext) => Promise<any>;
    compensate: (ctx: SagaContext) => Promise<void>;
}

interface SagaContext {
    input: Record<string, any>;
    state: Record<string, any>;
    completedSteps: string[];
    compensatedSteps: string[];
}

type SagaStatus = 'completed' | 'compensated' | 'failed';

interface SagaResult {
    status: SagaStatus;
    completedSteps: string[];
    compensatedSteps: string[];
    error?: string;
}

/**
 * Simple saga executor: runs steps in order, compensates in reverse on failure.
 */
async function executeSaga(
    steps: SagaStep[],
    input: Record<string, any>
): Promise<SagaResult> {
    const ctx: SagaContext = {
        input,
        state: {},
        completedSteps: [],
        compensatedSteps: [],
    };

    for (const step of steps) {
        try {
            await step.action(ctx);
            ctx.completedSteps.push(step.name);
        } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            console.log(`  [Saga] Step '${step.name}' FAILED: ${errorMsg}`);

            // Compensate completed steps in reverse
            for (let i = ctx.completedSteps.length - 1; i >= 0; i--) {
                const completedName = ctx.completedSteps[i];
                const completedStep = steps.find(s => s.name === completedName);
                if (completedStep?.compensate) {
                    await completedStep.compensate(ctx);
                    ctx.compensatedSteps.push(completedName);
                }
            }

            return {
                status: ctx.compensatedSteps.length > 0 ? 'compensated' : 'failed',
                completedSteps: [...ctx.completedSteps],
                compensatedSteps: [...ctx.compensatedSteps],
                error: errorMsg,
            };
        }
    }

    return {
        status: 'completed',
        completedSteps: [...ctx.completedSteps],
        compensatedSteps: [],
    };
}

// -------------------------------------------------------------------
// Saga step handlers (simulate external services)
// -------------------------------------------------------------------

async function reserveInventory(ctx: SagaContext): Promise<void> {
    const orderId = ctx.input.order_id;
    console.log(`  [Saga] Reserving inventory for order ${orderId}...`);
    ctx.state.inventoryReserved = true;
}

async function releaseInventory(ctx: SagaContext): Promise<void> {
    const orderId = ctx.input.order_id;
    console.log(`  [Saga] COMPENSATE: Releasing inventory for order ${orderId}...`);
    ctx.state.inventoryReserved = false;
}

async function processPayment(ctx: SagaContext, fail = false): Promise<void> {
    const orderId = ctx.input.order_id;
    const amount = ctx.input.total;
    console.log(`  [Saga] Processing payment of $${amount.toFixed(2)} for order ${orderId}...`);

    if (fail) {
        console.log(`  [Saga] PAYMENT FAILED for order ${orderId}!`);
        throw new Error(`Payment declined for order ${orderId}`);
    }

    const paymentId = `pay-${orderId}`;
    console.log(`  [Saga] Payment successful (${paymentId}).`);
    ctx.state.paymentId = paymentId;
}

async function refundPayment(ctx: SagaContext): Promise<void> {
    const orderId = ctx.input.order_id;
    console.log(`  [Saga] COMPENSATE: Refunding payment for order ${orderId}...`);
    ctx.state.paymentId = null;
}

async function shipOrder(ctx: SagaContext): Promise<void> {
    const orderId = ctx.input.order_id;
    const carrier = ctx.input.carrier ?? 'ACME Express';
    const tracking = `TRK-${orderId.toUpperCase()}`;
    console.log(`  [Saga] Shipping order ${orderId} via ${carrier} (${tracking})...`);
    ctx.state.trackingNumber = tracking;
    ctx.state.carrier = carrier;
}

async function cancelShipment(ctx: SagaContext): Promise<void> {
    const orderId = ctx.input.order_id;
    console.log(`  [Saga] COMPENSATE: Cancelling shipment for order ${orderId}...`);
    ctx.state.trackingNumber = null;
}

// -------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------

function makeEvent(type: string, orderId: string, extra: Record<string, any> = {}): OrderEvent {
    return {
        type,
        order_id: orderId,
        timestamp: new Date().toISOString(),
        ...extra,
    };
}

function printSeparator(title: string): void {
    console.log();
    console.log(`--- ${title} ---`);
}

// -------------------------------------------------------------------
// Main demo
// -------------------------------------------------------------------

async function main(): Promise<void> {
    console.log('='.repeat(60));
    console.log('  Aether SDK - Event-Sourced Order System Example');
    console.log('  (JS adaptation — no native event sourcing / saga modules)');
    console.log('='.repeat(60));

    const eventStore = new InMemoryEventStore();

    // ----------------------------------------------------------------
    // 1. Event validation
    // ----------------------------------------------------------------
    printSeparator('Event Validation');

    const validEvent = {
        order_id: 'ord-001',
        customer_id: 'cust-1',
        items: [],
        total: 29.99,
    };
    const validErrors = validateOrderCreated(validEvent);
    console.log(`  Validation (valid event): ${validErrors.length === 0 ? 'PASS' : `FAIL: ${validErrors}`}`);

    const invalidEvent = { order_id: 'ord-002' };
    const invalidErrors = validateOrderCreated(invalidEvent);
    console.log(`  Validation (invalid event): FAIL -> ${invalidErrors.join('; ')}`);

    // ----------------------------------------------------------------
    // 2. Event sourcing: build order from events
    // ----------------------------------------------------------------
    printSeparator('Event Sourcing - Build Order from Events');

    const orderId = 'ord-100';
    await eventStore.append(orderId, [
        makeEvent('inventory_reserved', orderId),
        makeEvent('order_created', orderId, {
            customer_id: 'cust-42',
            items: [
                { sku: 'WIDGET-1', name: 'Widget', qty: 2, price: 9.99 },
                { sku: 'GEAR-7', name: 'Gear', qty: 1, price: 10.01 },
            ],
            total: 29.99,
        }),
        makeEvent('payment_processed', orderId, {
            amount: 29.99,
            payment_id: 'pay-ord-100',
        }),
        makeEvent('order_shipped', orderId, {
            tracking_number: 'TRK-ORD100',
            carrier: 'ACME Express',
        }),
        makeEvent('order_delivered', orderId, {
            delivered_at: new Date().toISOString(),
        }),
    ]);

    // Reconstruct the order by replaying events
    const order = new OrderAggregate();
    const events = await eventStore.getEvents(orderId);
    order.loadFromHistory(events);
    console.log(`  [Reconstructed] ${order}`);

    // ----------------------------------------------------------------
    // 3. Saga: successful order processing
    // ----------------------------------------------------------------
    printSeparator('Saga - Successful Order (create -> pay -> ship)');

    const orderInput = {
        order_id: 'ord-200',
        customer_id: 'cust-77',
        items: [{ sku: 'GIZMO-3', name: 'Gizmo', qty: 1, price: 49.99 }],
        total: 49.99,
        carrier: 'FastShip',
    };

    const successSteps: SagaStep[] = [
        { name: 'reserve-inventory', action: reserveInventory, compensate: releaseInventory },
        { name: 'process-payment', action: (ctx) => processPayment(ctx, false), compensate: refundPayment },
        { name: 'ship-order', action: shipOrder, compensate: cancelShipment },
    ];

    const successResult = await executeSaga(successSteps, orderInput);
    console.log(`  [Saga Result] status=${successResult.status}, steps=${successResult.completedSteps}`);

    // ----------------------------------------------------------------
    // 4. Saga: payment failure triggers compensation
    // ----------------------------------------------------------------
    printSeparator('Saga - Payment Failure (compensation flow)');

    const failedOrderInput = {
        order_id: 'ord-300',
        customer_id: 'cust-88',
        items: [{ sku: 'THING-9', name: 'Thing', qty: 3, price: 15.00 }],
        total: 45.00,
        carrier: 'SlowMail',
    };

    const failedSteps: SagaStep[] = [
        { name: 'reserve-inventory', action: reserveInventory, compensate: releaseInventory },
        { name: 'process-payment', action: (ctx) => processPayment(ctx, true), compensate: refundPayment },
        { name: 'ship-order', action: shipOrder, compensate: cancelShipment },
    ];

    const failedResult = await executeSaga(failedSteps, failedOrderInput);
    console.log(`  [Saga Result] status=${failedResult.status}, error=${failedResult.error}`);
    console.log(`  [Saga Result] compensated_steps=${failedResult.compensatedSteps}`);

    // ----------------------------------------------------------------
    // 5. State reconstruction after saga events
    // ----------------------------------------------------------------
    printSeparator('State Reconstruction after Saga Events');

    const sagaOrderId = orderInput.order_id;
    await eventStore.append(sagaOrderId, [
        makeEvent('inventory_reserved', sagaOrderId),
        makeEvent('order_created', sagaOrderId, orderInput),
        makeEvent('payment_processed', sagaOrderId, {
            amount: orderInput.total,
            payment_id: 'pay-ord-200',
        }),
        makeEvent('order_shipped', sagaOrderId, {
            tracking_number: 'TRK-ORD200',
            carrier: 'FastShip',
        }),
    ]);

    const sagaOrder = new OrderAggregate();
    const sagaEvents = await eventStore.getEvents(sagaOrderId);
    sagaOrder.loadFromHistory(sagaEvents);
    console.log(`  [Reconstructed from saga events] ${sagaOrder}`);

    // ----------------------------------------------------------------
    // 6. Retry policy (demonstrates the resilience module)
    // ----------------------------------------------------------------
    printSeparator('Retry Policy (Resilience Module)');

    const retry = new RetryPolicy({
        maxAttempts: 3,
        initialDelay: 10,
        maxDelay: 100,
        strategy: BackoffStrategy.Fixed,
    });

    let attemptCount = 0;
    const retryResult = await retry.execute(async () => {
        attemptCount++;
        if (attemptCount < 3) {
            throw new Error('Transient failure');
        }
        return 'success';
    });

    console.log(`  Retry result: success=${retryResult.success}, attempts=${retryResult.attempts}`);

    console.log();
    console.log('='.repeat(60));
    console.log('  Order system demo complete!');
    console.log('='.repeat(60));
}

// Run the demo
main().catch(console.error);
