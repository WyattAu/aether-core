import {
  DeliveryGuarantee,
  SchemaCompatibilityMode,
} from '../../src/event/types';
import type {
  EventMessage,
  Subscription,
  SubscriptionOptions,
  EventStoreRecord,
  SchemaDefinition,
  SchemaField,
  EventEnvelope,
  Snapshot,
  EventHandler,
  EventFilter,
} from '../../src/event/types';

describe('DeliveryGuarantee enum', () => {
  it('has AT_LEAST_ONCE', () => {
    expect(DeliveryGuarantee.AT_LEAST_ONCE).toBe('at-least-once');
  });

  it('has AT_MOST_ONCE', () => {
    expect(DeliveryGuarantee.AT_MOST_ONCE).toBe('at-most-once');
  });

  it('has EXACTLY_ONCE', () => {
    expect(DeliveryGuarantee.EXACTLY_ONCE).toBe('exactly-once');
  });
});

describe('SchemaCompatibilityMode enum', () => {
  it('has NONE', () => {
    expect(SchemaCompatibilityMode.NONE).toBe('none');
  });

  it('has BACKWARD', () => {
    expect(SchemaCompatibilityMode.BACKWARD).toBe('backward');
  });

  it('has FORWARD', () => {
    expect(SchemaCompatibilityMode.FORWARD).toBe('forward');
  });

  it('has FULL', () => {
    expect(SchemaCompatibilityMode.FULL).toBe('full');
  });
});

describe('EventMessage interface', () => {
  it('can be constructed with all fields', () => {
    const msg: EventMessage = {
      id: 'evt-1',
      topic: 'orders.created',
      payload: { orderId: '123' },
      timestamp: new Date(),
      headers: new Map([['trace-id', 'abc']]),
      key: 'order-123',
      partitionKey: 'order-123',
    };
    expect(msg.id).toBe('evt-1');
    expect(msg.topic).toBe('orders.created');
    expect(msg.payload).toEqual({ orderId: '123' });
    expect(msg.headers.get('trace-id')).toBe('abc');
    expect(msg.key).toBe('order-123');
    expect(msg.partitionKey).toBe('order-123');
  });

  it('allows optional key and partitionKey to be omitted', () => {
    const msg: EventMessage = {
      id: 'evt-2',
      topic: 'test',
      payload: null,
      timestamp: new Date(),
      headers: new Map(),
    };
    expect(msg.key).toBeUndefined();
    expect(msg.partitionKey).toBeUndefined();
  });

  it('supports various payload types', () => {
    const msgs: EventMessage[] = [
      { id: '1', topic: 't', payload: 'string payload', timestamp: new Date(), headers: new Map() },
      { id: '2', topic: 't', payload: 42, timestamp: new Date(), headers: new Map() },
      { id: '3', topic: 't', payload: true, timestamp: new Date(), headers: new Map() },
      { id: '4', topic: 't', payload: [1, 2, 3], timestamp: new Date(), headers: new Map() },
      { id: '5', topic: 't', payload: null, timestamp: new Date(), headers: new Map() },
    ];
    expect(msgs).toHaveLength(5);
  });
});

describe('Subscription interface', () => {
  it('can be constructed with required fields', () => {
    const handler: EventHandler = () => {};
    const sub: Subscription = {
      id: 'sub-1',
      topic: 'orders.*',
      handler,
      options: { ackTimeout: 30000, maxRetries: 3 },
    };
    expect(sub.id).toBe('sub-1');
    expect(sub.topic).toBe('orders.*');
    expect(sub.options.ackTimeout).toBe(30000);
    expect(sub.options.maxRetries).toBe(3);
  });

  it('supports optional filter', () => {
    const filter: EventFilter = (e) => (e.payload as any)?.total > 100;
    const sub: Subscription = {
      id: 'sub-2',
      topic: 'orders.created',
      handler: () => {},
      filter,
      options: { ackTimeout: 10000, maxRetries: 5 },
    };
    expect(sub.filter).toBeDefined();
    expect(sub.filter!({ payload: { total: 200 }, topic: 'orders.created', id: '', timestamp: new Date(), headers: new Map() })).toBe(true);
    expect(sub.filter!({ payload: { total: 50 }, topic: 'orders.created', id: '', timestamp: new Date(), headers: new Map() })).toBe(false);
  });

  it('supports deadLetterTopic in options', () => {
    const opts: SubscriptionOptions = {
      ackTimeout: 5000,
      maxRetries: 1,
      deadLetterTopic: 'dlq.orders',
    };
    expect(opts.deadLetterTopic).toBe('dlq.orders');
  });

  it('handler can be async', async () => {
    let called = false;
    const handler: EventHandler = async () => { called = true; };
    const sub: Subscription = {
      id: 'sub-3',
      topic: 't',
      handler,
      options: { ackTimeout: 30000, maxRetries: 3 },
    };
    await sub.handler({ id: '', topic: 't', payload: null, timestamp: new Date(), headers: new Map() });
    expect(called).toBe(true);
  });
});

describe('EventStoreRecord interface', () => {
  it('can be constructed', () => {
    const record: EventStoreRecord = {
      eventId: 'evt-1',
      aggregateId: 'order-123',
      eventType: 'OrderCreated',
      data: { orderId: '123', total: 99.99 },
      metadata: { userId: 'u-1' },
      version: 1,
      timestamp: new Date(),
    };
    expect(record.aggregateId).toBe('order-123');
    expect(record.eventType).toBe('OrderCreated');
    expect(record.version).toBe(1);
    expect(record.metadata).toEqual({ userId: 'u-1' });
  });
});

describe('SchemaDefinition interface', () => {
  it('can be constructed with fields and schema', () => {
    const def: SchemaDefinition = {
      name: 'UserCreated',
      version: '1.0.0',
      type: 'json',
      fields: [
        { name: 'userId', type: 'string', required: true },
        { name: 'email', type: 'string', required: true },
        { name: 'role', type: 'string', required: false, defaultValue: 'viewer' },
      ],
      schema: {
        type: 'object',
        properties: {
          userId: { type: 'string' },
          email: { type: 'string' },
        },
        required: ['userId', 'email'],
      },
    };
    expect(def.name).toBe('UserCreated');
    expect(def.fields).toHaveLength(3);
    expect(def.fields[2].defaultValue).toBe('viewer');
  });
});

describe('SchemaField interface', () => {
  it('can be constructed', () => {
    const field: SchemaField = {
      name: 'age',
      type: 'integer',
      required: true,
      defaultValue: 0,
    };
    expect(field.name).toBe('age');
    expect(field.type).toBe('integer');
  });
});

describe('EventEnvelope interface', () => {
  it('can be constructed', () => {
    const envelope: EventEnvelope = {
      eventId: 'evt-abc',
      aggregateId: 'order-123',
      aggregateType: 'Order',
      eventType: 'OrderCreated',
      version: 1,
      timestamp: new Date(),
      payload: { orderId: '123' },
      metadata: { causationId: null, correlationId: 'corr-1' },
    };
    expect(envelope.aggregateType).toBe('Order');
    expect(envelope.version).toBe(1);
    expect(envelope.metadata.causationId).toBeNull();
  });
});

describe('Snapshot interface', () => {
  it('can be constructed', () => {
    const snapshot: Snapshot = {
      aggregateId: 'order-123',
      aggregateType: 'Order',
      version: 5,
      state: { status: 'shipped', total: 99.99 },
      timestamp: new Date(),
      metadata: { reason: 'periodic' },
    };
    expect(snapshot.version).toBe(5);
    expect(snapshot.state.status).toBe('shipped');
  });
});
