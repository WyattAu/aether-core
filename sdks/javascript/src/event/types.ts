/**
 * Event Module Type Definitions
 *
 * Core types for the event-driven architecture:
 * - EventMessage: Structured event with metadata
 * - Subscription: Topic subscription with delivery options
 * - EventStoreRecord: Persisted event sourcing record
 * - SchemaDefinition: Event schema for validation
 * - DeliveryGuarantee: Message delivery semantics
 * - SchemaCompatibilityMode: Schema evolution modes
 *
 * @module aether/event/types
 */

/**
 * Message delivery guarantee levels.
 *
 * Controls the trade-off between performance and reliability when
 * delivering events to subscribers.
 */
export enum DeliveryGuarantee {
  /** Guaranteed delivery but possible duplicates. */
  AT_LEAST_ONCE = 'at-least-once',
  /** Fire and forget; no delivery confirmation. */
  AT_MOST_ONCE = 'at-most-once',
  /** Exactly-once delivery; no duplicates, no loss. */
  EXACTLY_ONCE = 'exactly-once',
}

/**
 * Schema compatibility modes for version evolution.
 *
 * Determines how strict the registry is when evolving a schema to a new version.
 */
export enum SchemaCompatibilityMode {
  /** No compatibility enforcement; breaking changes allowed. */
  NONE = 'none',
  /** New schema can read data written by the old schema. */
  BACKWARD = 'backward',
  /** Old schema can read data written by the new schema. */
  FORWARD = 'forward',
  /** Both backward and forward compatible. */
  FULL = 'full',
}

/**
 * A structured event message in the pub/sub system.
 *
 * Carries a payload along with routing metadata, headers, and
 * optional partitioning keys.
 *
 * @example
 * ```typescript
 * const event: EventMessage = {
 *   id: crypto.randomUUID(),
 *   topic: 'orders.created',
 *   payload: { orderId: '123', total: 99.99 },
 *   timestamp: new Date(),
 *   headers: new Map([['trace-id', 'abc']]),
 *   key: 'order-123',
 * };
 * ```
 */
export interface EventMessage {
  /** Unique event identifier. */
  id: string;
  /** Topic the event was published to. */
  topic: string;
  /** Event payload data. */
  payload: unknown;
  /** When the event was created. */
  timestamp: Date;
  /** String-keyed metadata headers. */
  headers: Map<string, string>;
  /** Optional partitioning key for ordered delivery. */
  key?: string;
  /** Optional explicit partition key. */
  partitionKey?: string;
}

/**
 * Subscription options controlling delivery behavior.
 */
export interface SubscriptionOptions {
  /** Timeout in milliseconds before automatic acknowledgement (default: 30000). */
  ackTimeout: number;
  /** Maximum number of delivery retry attempts (default: 3). */
  maxRetries: number;
  /** Topic to route failed messages after retries are exhausted. */
  deadLetterTopic?: string;
}

/**
 * A subscription to a topic pattern in the pub/sub system.
 *
 * @example
 * ```typescript
 * const sub: Subscription = {
 *   id: 'sub-1',
 *   topic: 'orders.*',
 *   handler: async (event) => console.log(event),
 *   options: { ackTimeout: 10000, maxRetries: 5 },
 * };
 * ```
 */
export interface Subscription {
  /** Unique subscription identifier. */
  id: string;
  /** Topic pattern (supports `*` wildcards). */
  topic: string;
  /** Handler invoked when a matching event arrives. */
  handler: (event: EventMessage) => void | Promise<void>;
  /** Optional message filter predicate. */
  filter?: (event: EventMessage) => boolean;
  /** Delivery behavior configuration. */
  options: SubscriptionOptions;
}

/**
 * A persisted event record in the event store.
 *
 * Each record represents a state-changing event appended to an
 * aggregate's event stream.
 *
 * @example
 * ```typescript
 * const record: EventStoreRecord = {
 *   eventId: 'evt-1',
 *   aggregateId: 'order-123',
 *   eventType: 'OrderCreated',
 *   data: { orderId: '123', items: ['widget'] },
 *   metadata: { userId: 'u-1' },
 *   version: 1,
 *   timestamp: new Date(),
 * };
 * ```
 */
export interface EventStoreRecord {
  /** Unique event identifier. */
  eventId: string;
  /** Aggregate this event belongs to. */
  aggregateId: string;
  /** Type of event (e.g., 'OrderCreated'). */
  eventType: string;
  /** Event payload data. */
  data: Record<string, unknown>;
  /** Additional metadata (e.g., causation ID, correlation ID). */
  metadata: Record<string, unknown>;
  /** Sequence number within the aggregate stream. */
  version: number;
  /** When the event was recorded. */
  timestamp: Date;
}

/**
 * Schema field definition for event validation.
 */
export interface SchemaField {
  /** Field name. */
  name: string;
  /** Expected type (e.g., 'string', 'number', 'boolean'). */
  type: string;
  /** Whether the field is required. */
  required: boolean;
  /** Default value if the field is omitted. */
  defaultValue?: unknown;
}

/**
 * Schema definition for an event type.
 *
 * @example
 * ```typescript
 * const schema: SchemaDefinition = {
 *   name: 'UserCreated',
 *   version: '1.0.0',
 *   type: 'json',
 *   fields: [
 *     { name: 'userId', type: 'string', required: true },
 *     { name: 'email', type: 'string', required: true },
 *     { name: 'role', type: 'string', required: false, defaultValue: 'viewer' },
 *   ],
 *   schema: {
 *     type: 'object',
 *     properties: {
 *       userId: { type: 'string' },
 *       email: { type: 'string' },
 *     },
 *     required: ['userId', 'email'],
 *   },
 * };
 * ```
 */
export interface SchemaDefinition {
  /** Schema name (event type name). */
  name: string;
  /** Semantic version (e.g., '1.0.0'). */
  version: string;
  /** Schema format type (e.g., 'json', 'avro', 'protobuf'). */
  type: string;
  /** List of field definitions. */
  fields: SchemaField[];
  /** Raw schema definition (e.g., JSON Schema object). */
  schema: Record<string, unknown>;
}

/**
 * Wire-format envelope for transmitting events across boundaries.
 *
 * Wraps the event payload with routing and metadata needed for
 * persistence, replay, and auditing.
 *
 * @example
 * ```typescript
 * const envelope: EventEnvelope = {
 *   eventId: 'evt-abc',
 *   aggregateId: 'order-123',
 *   aggregateType: 'Order',
 *   eventType: 'OrderCreated',
 *   version: 1,
 *   timestamp: new Date(),
 *   payload: { orderId: '123' },
 *   metadata: { causationId: null, correlationId: 'corr-1' },
 * };
 * ```
 */
export interface EventEnvelope {
  /** Unique event identifier. */
  eventId: string;
  /** Aggregate this event belongs to. */
  aggregateId: string;
  /** Type name of the aggregate (e.g., 'Order'). */
  aggregateType: string;
  /** Event type name (e.g., 'OrderCreated'). */
  eventType: string;
  /** Sequence number within the aggregate stream. */
  version: number;
  /** When the event occurred. */
  timestamp: Date;
  /** Event payload. */
  payload: Record<string, unknown>;
  /** Additional metadata. */
  metadata: Record<string, unknown>;
}

/**
 * Point-in-time snapshot of an aggregate's state.
 *
 * Used to optimize event replay by providing a starting point
 * instead of replaying all events from the beginning.
 */
export interface Snapshot {
  /** Aggregate identifier. */
  aggregateId: string;
  /** Aggregate type name. */
  aggregateType: string;
  /** Stream version at which the snapshot was taken. */
  version: number;
  /** Serialized aggregate state. */
  state: Record<string, unknown>;
  /** When the snapshot was created. */
  timestamp: Date;
  /** Optional metadata. */
  metadata: Record<string, unknown>;
}

/**
 * Handler function type for processing events.
 *
 * @param event - The event message to process.
 */
export type EventHandler = (event: EventMessage) => void | Promise<void>;

/**
 * Predicate for filtering event messages.
 *
 * @param event - The event message to evaluate.
 * @returns `true` if the event should be delivered.
 */
export type EventFilter = (event: EventMessage) => boolean;
