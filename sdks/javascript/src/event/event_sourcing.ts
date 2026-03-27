/**
 * Event Sourcing for State Persistence
 *
 * Provides event sourcing capabilities for building event-sourced aggregates
 * and persisting state as a sequence of events.
 *
 * @example
 * ```typescript
 * import { EventStore, AggregateRoot } from 'aether-sdk/event';
 *
 * class Order extends AggregateRoot {
 *   public status = 'pending';
 *   public items: string[] = [];
 *
 *   applyOrderCreated(data: Record<string, unknown>): void {
 *     this.status = 'created';
 *     this.items = (data.items as string[]) ?? [];
 *   }
 *
 *   applyOrderShipped(_data: Record<string, unknown>): void {
 *     this.status = 'shipped';
 *   }
 * }
 *
 * const store = new EventStore();
 * const newVersion = await store.appendEvents('order-123', [
 *   { eventType: 'OrderCreated', data: { items: ['widget'] } },
 *   { eventType: 'OrderShipped', data: {} },
 * ]);
 * ```
 *
 * @module aether/event/event_sourcing
 */

import {
  EventStoreRecord,
  EventEnvelope,
  Snapshot,
} from './types';

/** @internal */
function generateId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

/**
 * Thrown when optimistic concurrency check fails during event append.
 */
export class ConcurrencyError extends Error {
  /**
   * @param message          - Description of the conflict.
   * @param expectedVersion  - The version the caller expected.
   * @param actualVersion    - The current version in the store.
   */
  constructor(
    message: string,
    public readonly expectedVersion: number,
    public readonly actualVersion: number,
  ) {
    super(message);
    this.name = 'ConcurrencyError';
  }
}

/**
 * Event upcaster that transforms events from an older schema to a newer one.
 *
 * Upcasters are registered by event type and version range to support
 * gradual schema evolution without breaking existing event consumers.
 *
 * @example
 * ```typescript
 * const upcaster: EventUpcaster = {
 *   eventType: 'UserCreated',
 *   fromVersion: 1,
 *   toVersion: 2,
 *   upcast: (data) => ({
 *     ...data,
 *     fullName: `${data.firstName} ${data.lastName}`,
 *   }),
 * };
 * ```
 */
export interface EventUpcaster {
  /** Event type this upcaster handles. */
  eventType: string;
  /** Source schema version. */
  fromVersion: number;
  /** Target schema version. */
  toVersion: number;
  /** Transformation function applied to the event data. */
  upcast: (data: Record<string, unknown>) => Record<string, unknown>;
}

/**
 * In-memory event store with optimistic concurrency control.
 *
 * Stores events per aggregate stream and supports snapshots for
 * optimized replay. Concurrency conflicts are detected via version
 * checking.
 *
 * @example
 * ```typescript
 * const store = new EventStore();
 *
 * // Append events with optimistic concurrency
 * await store.appendEvents('order-123', [
 *   { eventType: 'OrderCreated', data: { total: 99.99 } },
 * ], 0);
 *
 * // Read back events
 * const events = await store.getEvents('order-123');
 *
 * // Query by event type
 * const created = await store.getEventsByType('OrderCreated');
 *
 * // Create a snapshot
 * await store.createSnapshot('order-123');
 * const snapshot = await store.getSnapshot('order-123');
 * ```
 */
export class EventStore {
  private readonly streams = new Map<string, EventStoreRecord[]>();
  private readonly snapshots = new Map<string, Snapshot>();
  private readonly allEvents: EventStoreRecord[] = [];
  private readonly upcasters: EventUpcaster[] = [];

  /**
   * Register an event upcaster for schema evolution.
   *
   * @param upcaster - The upcaster to register.
   */
  registerUpcaster(upcaster: EventUpcaster): void {
    this.upcasters.push(upcaster);
  }

  /**
   * Append events to an aggregate's event stream.
   *
   * Each event is assigned a monotonically increasing version number.
   * If `expectedVersion` is provided, the store verifies the current
   * stream version matches before appending.
   *
   * @param aggregateId    - Aggregate identifier.
   * @param events         - Events to append (eventType + data pairs).
   * @param expectedVersion - Optional version for optimistic concurrency.
   * @returns The new stream version after append.
   * @throws {ConcurrencyError} If the expected version does not match.
   */
  async appendEvents(
    aggregateId: string,
    events: Array<{
      eventType: string;
      data: Record<string, unknown>;
      metadata?: Record<string, unknown>;
    }>,
    expectedVersion?: number,
  ): Promise<number> {
    let stream = this.streams.get(aggregateId);
    if (!stream) {
      stream = [];
      this.streams.set(aggregateId, stream);
    }

    const currentVersion = stream.length;

    if (expectedVersion !== undefined && currentVersion !== expectedVersion) {
      throw new ConcurrencyError(
        `Expected version ${expectedVersion}, but current version is ${currentVersion} for aggregate ${aggregateId}`,
        expectedVersion,
        currentVersion,
      );
    }

    for (let i = 0; i < events.length; i++) {
      const { eventType, data, metadata } = events[i];
      const record: EventStoreRecord = {
        eventId: generateId(),
        aggregateId,
        eventType,
        data,
        metadata: metadata ?? {},
        version: currentVersion + i + 1,
        timestamp: new Date(),
      };

      stream.push(record);
      this.allEvents.push(record);
    }

    return stream.length;
  }

  /**
   * Get all events for an aggregate.
   *
   * @param aggregateId - Aggregate identifier.
   * @returns Array of event records (empty if not found).
   */
  async getEvents(aggregateId: string): Promise<EventStoreRecord[]> {
    const stream = this.streams.get(aggregateId);
    if (!stream) {
      return [];
    }
    return this.upcastEvents([...stream]);
  }

  /**
   * Get all events across all aggregates, filtered by event type.
   *
   * @param eventType - The event type to filter by.
   * @returns Array of matching event records.
   */
  async getEventsByType(eventType: string): Promise<EventStoreRecord[]> {
    const matched = this.allEvents.filter((e) => e.eventType === eventType);
    return this.upcastEvents(matched);
  }

  /**
   * Get the latest snapshot for an aggregate.
   *
   * @param aggregateId - Aggregate identifier.
   * @returns The snapshot, or `undefined` if none exists.
   */
  async getSnapshot(aggregateId: string): Promise<Snapshot | undefined> {
    return this.snapshots.get(aggregateId);
  }

  /**
   * Create a snapshot of an aggregate's current state.
   *
   * The snapshot captures all events up to the current stream version.
   * Subsequent `loadFromHistory` calls on an aggregate can start from
   * the snapshot instead of replaying from the beginning.
   *
   * @param aggregateId    - Aggregate identifier.
   * @param aggregateType  - Aggregate type name (for metadata).
   * @param state          - Current aggregate state to snapshot.
   * @param metadata       - Optional snapshot metadata.
   * @returns The created snapshot.
   */
  async createSnapshot(
    aggregateId: string,
    aggregateType: string,
    state: Record<string, unknown>,
    metadata?: Record<string, unknown>,
  ): Promise<Snapshot> {
    const stream = this.streams.get(aggregateId);
    const version = stream ? stream.length : 0;

    const snapshot: Snapshot = {
      aggregateId,
      aggregateType,
      version,
      state,
      timestamp: new Date(),
      metadata: metadata ?? {},
    };

    this.snapshots.set(aggregateId, snapshot);
    return snapshot;
  }

  /**
   * Get events within a version range for an aggregate.
   *
   * @param aggregateId - Aggregate identifier.
   * @param fromVersion - Start version (inclusive).
   * @param toVersion   - End version (inclusive).
   * @returns Array of event records in the range.
   */
  async getEventsBetweenVersions(
    aggregateId: string,
    fromVersion: number,
    toVersion: number,
  ): Promise<EventStoreRecord[]> {
    const stream = this.streams.get(aggregateId);
    if (!stream) {
      return [];
    }
    const filtered = stream.filter(
      (e) => e.version >= fromVersion && e.version <= toVersion,
    );
    return this.upcastEvents(filtered);
  }

  /**
   * Get all events across all aggregates, optionally filtered.
   *
   * @param aggregateType - Optional aggregate type to filter by.
   * @param fromTimestamp - Optional timestamp to filter events after.
   * @returns Array of event records.
   */
  async getAllEvents(
    aggregateType?: string,
    fromTimestamp?: Date,
  ): Promise<EventStoreRecord[]> {
    let events = [...this.allEvents];

    if (aggregateType) {
      events = events.filter(
        (e) => e.metadata.aggregateType === aggregateType,
      );
    }

    if (fromTimestamp) {
      events = events.filter((e) => e.timestamp >= fromTimestamp);
    }

    return this.upcastEvents(events);
  }

  /**
   * Apply registered upcasters to a list of event records.
   *
   * @param events - Events to upcast.
   * @returns Events with upcasting applied.
   */
  private upcastEvents(events: EventStoreRecord[]): EventStoreRecord[] {
    if (this.upcasters.length === 0) {
      return events;
    }

    return events.map((event) => {
      let data = { ...event.data };

      for (const upcaster of this.upcasters) {
        if (upcaster.eventType === event.eventType) {
          data = upcaster.upcast(data);
        }
      }

      return { ...event, data };
    });
  }
}

/**
 * Base class for event-sourced aggregates.
 *
 * Aggregates maintain their state by applying events. Subclasses should
 * define `apply<EventTypeName>` methods for each event type they handle.
 * The method name is derived from the event type in PascalCase.
 *
 * @example
 * ```typescript
 * class BankAccount extends AggregateRoot {
 *   public balance = 0;
 *   public owner = '';
 *
 *   applyAccountOpened(data: Record<string, unknown>): void {
 *     this.owner = data.owner as string;
 *     this.balance = (data.initialBalance as number) ?? 0;
 *   }
 *
 *   applyMoneyDeposited(data: Record<string, unknown>): void {
 *     this.balance += (data.amount as number) ?? 0;
 *   }
 *
 *   applyMoneyWithdrawn(data: Record<string, unknown>): void {
 *     this.balance -= (data.amount as number) ?? 0;
 *   }
 * }
 * ```
 */
export class AggregateRoot {
  private _id = '';
  private _version = 0;
  private _uncommittedEvents: EventEnvelope[] = [];
  private _snapshotVersion = 0;

  /** Aggregate identifier. */
  get id(): string {
    return this._id;
  }

  /** Aggregate identifier (writable). */
  set id(value: string) {
    this._id = value;
  }

  /** Current version (number of applied events). */
  get version(): number {
    return this._version;
  }

  /** Events not yet persisted. */
  get uncommittedEvents(): EventEnvelope[] {
    return [...this._uncommittedEvents];
  }

  /**
   * Apply an event envelope to update aggregate state.
   *
   * Looks for an `apply<EventTypeName>` method on this instance based on
   * the event type. If no handler is found, the event is silently skipped.
   *
   * @param envelope - The event envelope to apply.
   */
  applyEvent(envelope: EventEnvelope): void {
    const pascalCase = envelope.eventType
      .charAt(0).toUpperCase()
      + envelope.eventType.slice(1);
    const methodName = `apply${pascalCase}`;

    const method = (this as Record<string, unknown>)[methodName];
    if (typeof method === 'function') {
      method.call(this, envelope.payload);
    }

    this._version = envelope.version;

    if (!this._id) {
      this._id = envelope.aggregateId;
    }
  }

  /**
   * Emit a new event, apply it to state, and track it as uncommitted.
   *
   * @param eventType  - Event type name (e.g., 'OrderCreated').
   * @param payload    - Event payload.
   * @param metadata   - Optional event metadata.
   * @returns The created event envelope.
   */
  emitEvent(
    eventType: string,
    payload: Record<string, unknown>,
    metadata?: Record<string, unknown>,
  ): EventEnvelope {
    this._version += 1;

    const envelope: EventEnvelope = {
      eventId: generateId(),
      aggregateId: this._id,
      aggregateType: this.constructor.name,
      eventType,
      version: this._version,
      timestamp: new Date(),
      payload,
      metadata: metadata ?? {},
    };

    this._uncommittedEvents.push(envelope);
    this.applyEvent(envelope);

    return envelope;
  }

  /**
   * Clear uncommitted events after successful persistence.
   */
  markEventsCommitted(): void {
    this._uncommittedEvents = [];
  }

  /**
   * Rebuild aggregate state from event history.
   *
   * Optionally starts from a snapshot to avoid replaying the entire
   * event stream. Only events with a version greater than the current
   * version are applied.
   *
   * @param events   - Event envelopes to replay.
   * @param snapshot - Optional starting snapshot.
   */
  loadFromHistory(events: EventEnvelope[], snapshot?: Snapshot): void {
    if (snapshot) {
      this._id = snapshot.aggregateId;
      this._version = snapshot.version;
      this._snapshotVersion = snapshot.version;

      for (const [key, value] of Object.entries(snapshot.state)) {
        (this as Record<string, unknown>)[key] = value;
      }
    }

    for (const event of events) {
      if (event.version > this._version) {
        this.applyEvent(event);
      }
    }
  }

  /**
   * Create a snapshot of the current aggregate state.
   *
   * Captures all own enumerable properties (excluding those starting
   * with `_`) as the snapshot state.
   *
   * @returns A snapshot of the current state.
   */
  createSnapshot(): Snapshot {
    const state: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(this)) {
      if (!key.startsWith('_')) {
        state[key] = value;
      }
    }

    return {
      aggregateId: this._id,
      aggregateType: this.constructor.name,
      version: this._version,
      state,
      timestamp: new Date(),
      metadata: {},
    };
  }
}
