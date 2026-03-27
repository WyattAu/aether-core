/**
 * Aether SDK Event Module
 *
 * Provides event-driven architecture primitives:
 * - Pub/Sub: Topic-based publish/subscribe messaging
 * - Event Sourcing: State persistence via event streams
 * - Schema Registry: Event validation and schema evolution
 *
 * @example
 * ```typescript
 * import {
 *   PubSubClient,
 *   EventStore,
 *   AggregateRoot,
 *   SchemaRegistry,
 *   EventMessage,
 *   DeliveryGuarantee,
 *   SchemaCompatibilityMode,
 * } from 'aether-sdk/event';
 * ```
 *
 * @module aether/event
 */

export {
  DeliveryGuarantee,
  SchemaCompatibilityMode,
} from './types';

export type {
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
} from './types';

export { PubSubClient } from './pubsub';

export {
  EventStore,
  AggregateRoot,
  ConcurrencyError,
} from './event_sourcing';

export type { EventUpcaster } from './event_sourcing';

export {
  SchemaRegistry,
  SchemaValidationError,
} from './schema';

export type {
  FieldValidationError,
  CompatibilityResult,
} from './schema';
