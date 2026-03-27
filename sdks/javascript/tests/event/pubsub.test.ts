import { PubSubClient } from '../../src/event/pubsub';
import { DeliveryGuarantee } from '../../src/event/types';
import type { EventMessage } from '../../src/event/types';

describe('PubSubClient', () => {
  let client: PubSubClient;

  beforeEach(() => {
    client = new PubSubClient();
  });

  afterEach(async () => {
    await client.close();
  });

  it('publishes and delivers to exact topic subscriber', async () => {
    const received: EventMessage[] = [];
    await client.subscribe('orders.created', (e) => { received.push(e); });
    await client.publish('orders.created', { orderId: '123' });
    expect(received).toHaveLength(1);
    expect(received[0].topic).toBe('orders.created');
    expect(received[0].payload).toEqual({ orderId: '123' });
  });

  it('publishes with key and headers', async () => {
    let received: EventMessage | undefined;
    await client.subscribe('t', (e) => { received = e; });
    const headers = new Map([['trace-id', 'abc']]);
    await client.publish('t', 'data', 'my-key', headers);
    expect(received!.key).toBe('my-key');
    expect(received!.partitionKey).toBe('my-key');
    expect(received!.headers.get('trace-id')).toBe('abc');
  });

  it('delivers to wildcard subscribers', async () => {
    const received: EventMessage[] = [];
    await client.subscribe('orders.*', (e) => { received.push(e); });
    await client.publish('orders.created', {});
    await client.publish('orders.shipped', {});
    expect(received).toHaveLength(2);
  });

  it('wildcard does not match deeper segments', async () => {
    const received: EventMessage[] = [];
    await client.subscribe('orders.*', (e) => { received.push(e); });
    await client.publish('orders.created.shipped', {});
    expect(received).toHaveLength(0);
  });

  it('wildcard does not match shorter segments', async () => {
    const received: EventMessage[] = [];
    await client.subscribe('orders.created.*', (e) => { received.push(e); });
    await client.publish('orders.created', {});
    expect(received).toHaveLength(0);
  });

  it('does not deliver to non-matching subscribers', async () => {
    const received: EventMessage[] = [];
    await client.subscribe('users.created', (e) => { received.push(e); });
    await client.publish('orders.created', {});
    expect(received).toHaveLength(0);
  });

  it('applies filter predicate', async () => {
    const received: EventMessage[] = [];
    await client.subscribe('orders.created', (e) => { received.push(e); }, {
      filter: (e) => (e.payload as any)?.total > 100,
    });
    await client.publish('orders.created', { total: 50 });
    await client.publish('orders.created', { total: 200 });
    expect(received).toHaveLength(1);
    expect(received[0].payload).toEqual({ total: 200 });
  });

  it('delivers to multiple subscribers on same topic', async () => {
    const r1: EventMessage[] = [];
    const r2: EventMessage[] = [];
    await client.subscribe('t', (e) => { r1.push(e); });
    await client.subscribe('t', (e) => { r2.push(e); });
    await client.publish('t', {});
    expect(r1).toHaveLength(1);
    expect(r2).toHaveLength(1);
  });

  it('supports async handlers', async () => {
    let called = false;
    await client.subscribe('t', async () => { called = true; });
    await client.publish('t', {});
    expect(called).toBe(true);
  });

  it('unsubscribes and stops delivery', async () => {
    const received: EventMessage[] = [];
    const sub = await client.subscribe('t', (e) => { received.push(e); });
    await client.unsubscribe(sub.id);
    await client.publish('t', {});
    expect(received).toHaveLength(0);
  });

  it('unsubscribe is a no-op for unknown ID', async () => {
    await expect(client.unsubscribe('nonexistent')).resolves.not.toThrow();
  });

  it('getSubscribers returns matching subscriptions', async () => {
    await client.subscribe('orders.*', () => {});
    await client.subscribe('orders.created', () => {});
    await client.subscribe('users.*', () => {});

    const subs = client.getSubscribers('orders.created');
    expect(subs).toHaveLength(2);
  });

  it('getTopics returns registered topic patterns', async () => {
    await client.subscribe('a.b', () => {});
    await client.subscribe('a.*', () => {});

    const topics = client.getTopics();
    expect(topics).toContain('a.b');
    expect(topics).toContain('a.*');
  });

  it('close removes all subscriptions', async () => {
    await client.subscribe('t', () => {});
    await client.close();
    expect(client.getTopics()).toHaveLength(0);
  });

  it('returns event with id and timestamp', async () => {
    const msg = await client.publish('t', {});
    expect(msg.id).toBeTruthy();
    expect(msg.timestamp).toBeInstanceOf(Date);
  });

  it('delivers with AT_LEAST_ONCE guarantee (default)', async () => {
    const clientWithRetry = new PubSubClient({
      deliveryGuarantee: DeliveryGuarantee.AT_LEAST_ONCE,
    });
    let attempts = 0;
    await clientWithRetry.subscribe('t', () => {
      attempts++;
      if (attempts === 1) throw new Error('transient');
    }, { maxRetries: 3 });
    await clientWithRetry.publish('t', {});
    expect(attempts).toBe(2);
    await clientWithRetry.close();
  });

  it('delivers with AT_MOST_ONCE guarantee (no retry)', async () => {
    const clientNoRetry = new PubSubClient({
      deliveryGuarantee: DeliveryGuarantee.AT_MOST_ONCE,
    });
    let attempts = 0;
    await clientNoRetry.subscribe('t', () => {
      attempts++;
      throw new Error('fail');
    }, { maxRetries: 3 });
    await clientNoRetry.publish('t', {});
    expect(attempts).toBe(1);
    await clientNoRetry.close();
  });

  it('publishes to dead letter topic on delivery failure', async () => {
    const client = new PubSubClient();
    const dlqReceived: EventMessage[] = [];
    await client.subscribe('dlq', (e) => { dlqReceived.push(e); });
    await client.subscribe('t', () => { throw new Error('always fail'); }, {
      maxRetries: 0,
      deadLetterTopic: 'dlq',
    });
    await client.publish('t', { original: 'data' });
    expect(dlqReceived).toHaveLength(1);
    expect((dlqReceived[0].payload as any).subscriptionId).toBeTruthy();
    await client.close();
  });
});
