/**
 * Pub/Sub Messaging for Event-Driven Architecture
 *
 * Provides topic-based publish/subscribe messaging with wildcard topic
 * matching, message filtering, and configurable delivery guarantees.
 *
 * @example
 * ```typescript
 * import { PubSubClient } from 'aether-sdk/event';
 *
 * const client = new PubSubClient();
 * const sub = await client.subscribe('orders.*', async (event) => {
 *   console.log(`Order event: ${event.payload}`);
 * });
 *
 * await client.publish('orders.created', { orderId: '123', total: 99.99 });
 * await client.unsubscribe(sub.id);
 * ```
 *
 * @module aether/event/pubsub
 */

import {
  EventMessage,
  Subscription,
  SubscriptionOptions,
  EventHandler,
  EventFilter,
  DeliveryGuarantee,
} from './types';

/** @internal */
function generateId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

const DEFAULT_OPTIONS: SubscriptionOptions = {
  ackTimeout: 30000,
  maxRetries: 3,
};

/**
 * In-memory pub/sub client with topic-based routing.
 *
 * Supports wildcard topic patterns (e.g., `orders.*`), message filtering,
 * and configurable delivery guarantees. Messages are routed synchronously
 * to matching subscribers within the same process.
 *
 * @example
 * ```typescript
 * const client = new PubSubClient();
 *
 * // Subscribe to all order events
 * await client.subscribe('orders.*', (event) => {
 *   console.log(event.topic, event.payload);
 * });
 *
 * // Subscribe with a filter
 * await client.subscribe('orders.created', (event) => {
 *   console.log('High-value order:', event.payload);
 * }, {
 *   filter: (e) => (e.payload as any).total > 100,
 *   ackTimeout: 60000,
 * });
 *
 * // Publish an event
 * const msgId = await client.publish('orders.created', {
 *   orderId: '123',
 *   total: 199.99,
 * }, 'order-123');
 * ```
 */
export class PubSubClient {
  private readonly subscriptions = new Map<string, Subscription>();
  private readonly topicSubscriptions = new Map<string, Set<string>>();
  private readonly deliveryGuarantee: DeliveryGuarantee;

  /**
   * Create a new PubSubClient.
   *
   * @param options.deliveryGuarantee - Delivery guarantee (default: AT_LEAST_ONCE).
   */
  constructor(
    options?: {
      deliveryGuarantee?: DeliveryGuarantee;
    }
  ) {
    this.deliveryGuarantee = options?.deliveryGuarantee ?? DeliveryGuarantee.AT_LEAST_ONCE;
  }

  /**
   * Publish an event to a topic.
   *
   * Routes the event to all subscribers whose topic pattern matches.
   * Async handlers are awaited; sync handlers are called directly.
   *
   * @param topic   - Topic name to publish to.
   * @param payload - Event payload.
   * @param key     - Optional partitioning key.
   * @param headers - Optional message headers.
   * @returns The published event message.
   */
  async publish(
    topic: string,
    payload: unknown,
    key?: string,
    headers?: Map<string, string>,
  ): Promise<EventMessage> {
    const event: EventMessage = {
      id: generateId(),
      topic,
      payload,
      timestamp: new Date(),
      headers: headers ?? new Map(),
      key,
      partitionKey: key,
    };

    const targets = this.getMatchingSubscriptions(event.topic);

    for (const subscription of targets) {
      if (subscription.filter && !subscription.filter(event)) {
        continue;
      }

      try {
        if (this.deliveryGuarantee === DeliveryGuarantee.AT_LEAST_ONCE) {
          await this.deliverWithRetry(subscription, event);
        } else {
          await this.deliverOnce(subscription, event);
        }
      } catch {
        if (subscription.options.deadLetterTopic) {
          await this.publish(
            subscription.options.deadLetterTopic,
            {
              originalEvent: event,
              subscriptionId: subscription.id,
              error: 'Delivery failed after retries',
            },
            key,
            headers,
          );
        }
      }
    }

    return event;
  }

  /**
   * Subscribe to a topic pattern.
   *
   * The pattern supports `*` as a wildcard within dot-separated segments.
   * For example, `orders.*` matches `orders.created` and `orders.shipped`.
   *
   * @param topic   - Topic pattern to subscribe to.
   * @param handler - Callback for each matching event.
   * @param options - Subscription options (filter, ackTimeout, etc.).
   * @returns The created subscription.
   */
  async subscribe(
    topic: string,
    handler: EventHandler,
    options?: {
      filter?: EventFilter;
      ackTimeout?: number;
      maxRetries?: number;
      deadLetterTopic?: string;
    },
  ): Promise<Subscription> {
    const id = generateId();
    const subOptions: SubscriptionOptions = {
      ackTimeout: options?.ackTimeout ?? DEFAULT_OPTIONS.ackTimeout,
      maxRetries: options?.maxRetries ?? DEFAULT_OPTIONS.maxRetries,
      deadLetterTopic: options?.deadLetterTopic,
    };

    const subscription: Subscription = {
      id,
      topic,
      handler,
      filter: options?.filter,
      options: subOptions,
    };

    this.subscriptions.set(id, subscription);

    let subs = this.topicSubscriptions.get(topic);
    if (!subs) {
      subs = new Set();
      this.topicSubscriptions.set(topic, subs);
    }
    subs.add(id);

    return subscription;
  }

  /**
   * Unsubscribe by subscription ID.
   *
   * @param subscriptionId - The subscription to remove.
   */
  async unsubscribe(subscriptionId: string): Promise<void> {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      return;
    }

    this.subscriptions.delete(subscriptionId);

    const subs = this.topicSubscriptions.get(subscription.topic);
    if (subs) {
      subs.delete(subscriptionId);
      if (subs.size === 0) {
        this.topicSubscriptions.delete(subscription.topic);
      }
    }
  }

  /**
   * Get all active subscribers for a specific topic.
   *
   * Returns subscriptions whose pattern matches the given topic exactly.
   *
   * @param topic - Topic name to look up.
   * @returns Array of matching subscriptions.
   */
  getSubscribers(topic: string): Subscription[] {
    const results: Subscription[] = [];

    for (const subscription of this.subscriptions.values()) {
      if (this.matchesPattern(topic, subscription.topic)) {
        results.push(subscription);
      }
    }

    return results;
  }

  /**
   * Get all registered topic patterns.
   *
   * @returns Array of unique topic patterns with active subscriptions.
   */
  getTopics(): string[] {
    return Array.from(this.topicSubscriptions.keys());
  }

  /**
   * Remove all subscriptions and reset the client.
   */
  async close(): Promise<void> {
    this.subscriptions.clear();
    this.topicSubscriptions.clear();
  }

  /**
   * Get all subscriptions matching a topic name.
   *
   * @param topic - The topic name to match against.
   * @returns Array of matching subscriptions.
   */
  private getMatchingSubscriptions(topic: string): Subscription[] {
    const results: Subscription[] = [];

    for (const subscription of this.subscriptions.values()) {
      if (this.matchesPattern(topic, subscription.topic)) {
        results.push(subscription);
      }
    }

    return results;
  }

  /**
   * Check whether a concrete topic matches a subscription pattern.
   *
   * Supports `*` as a wildcard within dot-separated segments.
   * For example, `orders.*` matches `orders.created` but not `orders.created.shipped`.
   *
   * @param topic   - Concrete topic name.
   * @param pattern - Subscription pattern (may contain `*`).
   * @returns `true` if the topic matches the pattern.
   */
  private matchesPattern(topic: string, pattern: string): boolean {
    if (!pattern.includes('*')) {
      return topic === pattern;
    }

    const topicParts = topic.split('.');
    const patternParts = pattern.split('.');

    if (topicParts.length !== patternParts.length) {
      return false;
    }

    for (let i = 0; i < patternParts.length; i++) {
      if (patternParts[i] !== '*' && patternParts[i] !== topicParts[i]) {
        return false;
      }
    }

    return true;
  }

  /**
   * Deliver an event to a subscriber with retry logic.
   *
   * @param subscription - Target subscription.
   * @param event        - Event to deliver.
   */
  private async deliverWithRetry(
    subscription: Subscription,
    event: EventMessage,
  ): Promise<void> {
    const maxAttempts = subscription.options.maxRetries + 1;
    let lastError: Error | undefined;

    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        const result = subscription.handler(event);
        if (result instanceof Promise) {
          await result;
        }
        return;
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));
      }
    }

    throw lastError ?? new Error('Delivery failed');
  }

  /**
   * Deliver an event to a subscriber once (no retries).
   *
   * @param subscription - Target subscription.
   * @param event        - Event to deliver.
   */
  private async deliverOnce(
    subscription: Subscription,
    event: EventMessage,
  ): Promise<void> {
    const result = subscription.handler(event);
    if (result instanceof Promise) {
      await result;
    }
  }
}
