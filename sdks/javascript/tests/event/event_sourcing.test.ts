import {
  EventStore,
  AggregateRoot,
  ConcurrencyError,
} from '../../src/event/event_sourcing';
import type { EventUpcaster } from '../../src/event/event_sourcing';
import type { EventEnvelope, Snapshot } from '../../src/event/types';

class BankAccount extends AggregateRoot {
  public balance = 0;
  public owner = '';

  applyAccountOpened(data: Record<string, unknown>): void {
    this.owner = data.owner as string;
    this.balance = (data.initialBalance as number) ?? 0;
  }

  applyMoneyDeposited(data: Record<string, unknown>): void {
    this.balance += (data.amount as number) ?? 0;
  }

  applyMoneyWithdrawn(data: Record<string, unknown>): void {
    this.balance -= (data.amount as number) ?? 0;
  }
}

describe('EventStore', () => {
  let store: EventStore;

  beforeEach(() => {
    store = new EventStore();
  });

  it('appends events to a new stream', async () => {
    const version = await store.appendEvents('acc-1', [
      { eventType: 'AccountOpened', data: { owner: 'Alice' } },
    ]);
    expect(version).toBe(1);
  });

  it('appends multiple events with incrementing versions', async () => {
    const v1 = await store.appendEvents('acc-1', [
      { eventType: 'AccountOpened', data: { owner: 'Alice' } },
    ]);
    const v2 = await store.appendEvents('acc-1', [
      { eventType: 'MoneyDeposited', data: { amount: 100 } },
    ]);
    expect(v1).toBe(1);
    expect(v2).toBe(2);
  });

  it('appends multiple events in one call', async () => {
    const version = await store.appendEvents('acc-1', [
      { eventType: 'AccountOpened', data: {} },
      { eventType: 'MoneyDeposited', data: { amount: 50 } },
      { eventType: 'MoneyDeposited', data: { amount: 25 } },
    ]);
    expect(version).toBe(3);
  });

  it('throws ConcurrencyError on version mismatch', async () => {
    await store.appendEvents('acc-1', [
      { eventType: 'AccountOpened', data: {} },
    ]);
    await expect(store.appendEvents('acc-1', [{ eventType: 'X', data: {} }], 0))
      .rejects.toThrow(ConcurrencyError);
  });

  it('ConcurrencyError has expectedVersion and actualVersion', async () => {
    await store.appendEvents('acc-1', [{ eventType: 'X', data: {} }]);
    try {
      await store.appendEvents('acc-1', [{ eventType: 'Y', data: {} }], 0);
    } catch (e) {
      expect(e).toBeInstanceOf(ConcurrencyError);
      expect((e as ConcurrencyError).expectedVersion).toBe(0);
      expect((e as ConcurrencyError).actualVersion).toBe(1);
    }
  });

  it('succeeds with correct expectedVersion', async () => {
    await store.appendEvents('acc-1', [{ eventType: 'X', data: {} }], 0);
    const v = await store.appendEvents('acc-1', [{ eventType: 'Y', data: {} }], 1);
    expect(v).toBe(2);
  });

  it('gets events for an aggregate', async () => {
    await store.appendEvents('acc-1', [
      { eventType: 'AccountOpened', data: { owner: 'Bob' } },
      { eventType: 'MoneyDeposited', data: { amount: 200 } },
    ]);

    const events = await store.getEvents('acc-1');
    expect(events).toHaveLength(2);
    expect(events[0].eventType).toBe('AccountOpened');
    expect(events[0].version).toBe(1);
    expect(events[1].eventType).toBe('MoneyDeposited');
    expect(events[1].version).toBe(2);
  });

  it('returns empty array for unknown aggregate', async () => {
    const events = await store.getEvents('missing');
    expect(events).toHaveLength(0);
  });

  it('filters events by type across all aggregates', async () => {
    await store.appendEvents('acc-1', [
      { eventType: 'AccountOpened', data: {} },
      { eventType: 'MoneyDeposited', data: {} },
    ]);
    await store.appendEvents('acc-2', [
      { eventType: 'AccountOpened', data: {} },
    ]);

    const events = await store.getEventsByType('AccountOpened');
    expect(events).toHaveLength(2);
  });

  it('returns empty array when filtering by unknown event type', async () => {
    const events = await store.getEventsByType('Nonexistent');
    expect(events).toHaveLength(0);
  });

  it('gets events between versions', async () => {
    await store.appendEvents('acc-1', [
      { eventType: 'E1', data: {} },
      { eventType: 'E2', data: {} },
      { eventType: 'E3', data: {} },
      { eventType: 'E4', data: {} },
    ]);

    const events = await store.getEventsBetweenVersions('acc-1', 2, 3);
    expect(events).toHaveLength(2);
    expect(events[0].version).toBe(2);
    expect(events[1].version).toBe(3);
  });

  it('gets events between versions for unknown aggregate', async () => {
    const events = await store.getEventsBetweenVersions('missing', 1, 5);
    expect(events).toHaveLength(0);
  });

  it('creates and retrieves a snapshot', async () => {
    await store.appendEvents('acc-1', [
      { eventType: 'E1', data: {} },
      { eventType: 'E2', data: {} },
    ]);

    const snapshot = await store.createSnapshot('acc-1', 'Account', { balance: 100 });
    expect(snapshot.version).toBe(2);
    expect(snapshot.state.balance).toBe(100);

    const retrieved = await store.getSnapshot('acc-1');
    expect(retrieved?.aggregateId).toBe('acc-1');
  });

  it('returns undefined for missing snapshot', async () => {
    const snapshot = await store.getSnapshot('missing');
    expect(snapshot).toBeUndefined();
  });

  it('getAllEvents returns all events', async () => {
    await store.appendEvents('a1', [{ eventType: 'E1', data: {}, metadata: { aggregateType: 'Order' } }]);
    await store.appendEvents('a2', [{ eventType: 'E2', data: {}, metadata: { aggregateType: 'User' } }]);

    const all = await store.getAllEvents();
    expect(all).toHaveLength(2);
  });

  it('getAllEvents filters by aggregateType', async () => {
    await store.appendEvents('a1', [{ eventType: 'E1', data: {}, metadata: { aggregateType: 'Order' } }]);
    await store.appendEvents('a2', [{ eventType: 'E2', data: {}, metadata: { aggregateType: 'User' } }]);

    const orders = await store.getAllEvents('Order');
    expect(orders).toHaveLength(1);
    expect(orders[0].aggregateId).toBe('a1');
  });

  it('applies upcasters to events', async () => {
    const upcaster: EventUpcaster = {
      eventType: 'UserCreated',
      fromVersion: 1,
      toVersion: 2,
      upcast: (data) => ({ ...data, fullName: `${data.firstName} ${data.lastName}` }),
    };
    store.registerUpcaster(upcaster);

    await store.appendEvents('user-1', [
      { eventType: 'UserCreated', data: { firstName: 'Alice', lastName: 'Smith' } },
    ]);

    const events = await store.getEvents('user-1');
    expect(events[0].data.fullName).toBe('Alice Smith');
    expect(events[0].data.firstName).toBe('Alice');
  });

  it('applies upcasters in getEventsByType', async () => {
    const upcaster: EventUpcaster = {
      eventType: 'E',
      fromVersion: 1,
      toVersion: 2,
      upcast: (data) => ({ ...data, versioned: true }),
    };
    store.registerUpcaster(upcaster);

    await store.appendEvents('a1', [{ eventType: 'E', data: {} }]);
    const events = await store.getEventsByType('E');
    expect(events[0].data.versioned).toBe(true);
  });

  it('no-op when no upcasters registered', async () => {
    await store.appendEvents('a1', [{ eventType: 'E', data: { x: 1 } }]);
    const events = await store.getEvents('a1');
    expect(events[0].data).toEqual({ x: 1 });
  });
});

describe('AggregateRoot', () => {
  it('starts with empty id and version 0', () => {
    const agg = new AggregateRoot();
    expect(agg.id).toBe('');
    expect(agg.version).toBe(0);
  });

  it('emitEvent increments version and applies event', () => {
    const agg = new BankAccount();
    agg.id = 'acc-1';
    agg.emitEvent('AccountOpened', { owner: 'Alice', initialBalance: 100 });
    expect(agg.version).toBe(1);
    expect(agg.owner).toBe('Alice');
    expect(agg.balance).toBe(100);
  });

  it('emitEvent tracks uncommitted events', () => {
    const agg = new BankAccount();
    agg.id = 'acc-1';
    agg.emitEvent('AccountOpened', { owner: 'Alice' });
    agg.emitEvent('MoneyDeposited', { amount: 50 });

    expect(agg.uncommittedEvents).toHaveLength(2);
    expect(agg.uncommittedEvents[0].eventType).toBe('AccountOpened');
    expect(agg.uncommittedEvents[1].eventType).toBe('MoneyDeposited');
  });

  it('markEventsCommitted clears uncommitted events', () => {
    const agg = new BankAccount();
    agg.id = 'acc-1';
    agg.emitEvent('AccountOpened', { owner: 'Alice' });
    agg.markEventsCommitted();
    expect(agg.uncommittedEvents).toHaveLength(0);
  });

  it('applyEvent invokes correct handler', () => {
    const agg = new BankAccount();
    const envelope: EventEnvelope = {
      eventId: 'e1',
      aggregateId: 'acc-1',
      aggregateType: 'BankAccount',
      eventType: 'MoneyDeposited',
      version: 1,
      timestamp: new Date(),
      payload: { amount: 250 },
      metadata: {},
    };
    agg.applyEvent(envelope);
    expect(agg.balance).toBe(250);
    expect(agg.version).toBe(1);
  });

  it('applyEvent sets aggregate id if empty', () => {
    const agg = new BankAccount();
    expect(agg.id).toBe('');
    const envelope: EventEnvelope = {
      eventId: 'e1',
      aggregateId: 'acc-1',
      aggregateType: 'BankAccount',
      eventType: 'AccountOpened',
      version: 1,
      timestamp: new Date(),
      payload: { owner: 'Alice' },
      metadata: {},
    };
    agg.applyEvent(envelope);
    expect(agg.id).toBe('acc-1');
  });

  it('applyEvent skips unknown event types silently', () => {
    const agg = new BankAccount();
    const envelope: EventEnvelope = {
      eventId: 'e1',
      aggregateId: 'acc-1',
      aggregateType: 'BankAccount',
      eventType: 'UnknownEvent',
      version: 1,
      timestamp: new Date(),
      payload: {},
      metadata: {},
    };
    expect(() => agg.applyEvent(envelope)).not.toThrow();
  });

  it('loadFromHistory replays events', () => {
    const agg = new BankAccount();
    const events: EventEnvelope[] = [
      {
        eventId: 'e1', aggregateId: 'acc-1', aggregateType: 'BankAccount',
        eventType: 'AccountOpened', version: 1, timestamp: new Date(),
        payload: { owner: 'Bob', initialBalance: 50 }, metadata: {},
      },
      {
        eventId: 'e2', aggregateId: 'acc-1', aggregateType: 'BankAccount',
        eventType: 'MoneyDeposited', version: 2, timestamp: new Date(),
        payload: { amount: 100 }, metadata: {},
      },
      {
        eventId: 'e3', aggregateId: 'acc-1', aggregateType: 'BankAccount',
        eventType: 'MoneyWithdrawn', version: 3, timestamp: new Date(),
        payload: { amount: 30 }, metadata: {},
      },
    ];
    agg.loadFromHistory(events);
    expect(agg.id).toBe('acc-1');
    expect(agg.version).toBe(3);
    expect(agg.owner).toBe('Bob');
    expect(agg.balance).toBe(120);
  });

  it('loadFromHistory with snapshot skips already-applied events', () => {
    const agg = new BankAccount();
    const snapshot: Snapshot = {
      aggregateId: 'acc-1',
      aggregateType: 'BankAccount',
      version: 2,
      state: { balance: 150, owner: 'Bob' },
      timestamp: new Date(),
      metadata: {},
    };
    const events: EventEnvelope[] = [
      {
        eventId: 'e1', aggregateId: 'acc-1', aggregateType: 'BankAccount',
        eventType: 'AccountOpened', version: 1, timestamp: new Date(),
        payload: {}, metadata: {},
      },
      {
        eventId: 'e2', aggregateId: 'acc-1', aggregateType: 'BankAccount',
        eventType: 'AccountOpened', version: 2, timestamp: new Date(),
        payload: {}, metadata: {},
      },
      {
        eventId: 'e3', aggregateId: 'acc-1', aggregateType: 'BankAccount',
        eventType: 'MoneyDeposited', version: 3, timestamp: new Date(),
        payload: { amount: 50 }, metadata: {},
      },
    ];
    agg.loadFromHistory(events, snapshot);
    expect(agg.balance).toBe(200);
    expect(agg.version).toBe(3);
  });

  it('createSnapshot captures current state', () => {
    const agg = new BankAccount();
    agg.id = 'acc-1';
    agg.balance = 500;
    agg.owner = 'Charlie';

    const snapshot = agg.createSnapshot();
    expect(snapshot.aggregateId).toBe('acc-1');
    expect(snapshot.aggregateType).toBe('BankAccount');
    expect(snapshot.version).toBe(0);
    expect(snapshot.state.balance).toBe(500);
    expect(snapshot.state.owner).toBe('Charlie');
  });

  it('createSnapshot excludes private properties', () => {
    const agg = new BankAccount();
    agg.id = 'acc-1';
    agg.emitEvent('AccountOpened', { owner: 'Alice' });

    const snapshot = agg.createSnapshot();
    expect(snapshot.state._id).toBeUndefined();
    expect(snapshot.state._version).toBeUndefined();
  });

  it('set id works', () => {
    const agg = new AggregateRoot();
    agg.id = 'my-id';
    expect(agg.id).toBe('my-id');
  });
});
