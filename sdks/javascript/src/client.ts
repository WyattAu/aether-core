/**
 * Aether Server Client
 *
 * HTTP client for communicating with an Aether reference server.
 */

export interface ActorInfo {
  actorId: string;
  actorType: string;
  capabilities: string[];
  metadata: Record<string, unknown>;
  status: string;
  createdAt: string;
  lastHeartbeat: string | null;
}

export interface MessageEnvelope {
  messageId: string;
  sourceActor: string;
  targetActor: string;
  messageType: string;
  payload: unknown;
  correlationId: string | null;
  timestamp: string;
  priority: number;
}

export interface DeliveryReceipt {
  messageId: string;
  status: string;
  deliveredAt: string;
  correlationId: string | null;
}

export interface StateEntry {
  actorId: string;
  key: string;
  value: unknown;
  version: number;
  updatedAt: string;
}

export interface EventRecord {
  eventId: string;
  aggregateId: string;
  eventType: string;
  data: unknown;
  version: number;
  timestamp: string;
}

export interface ServerInfo {
  status: string;
  uptime: number;
  actorCount: number;
  messageCount: number;
}

export class AetherServerError extends Error {
  constructor(
    public readonly statusCode: number,
    public readonly detail: string,
  ) {
    super(`HTTP ${statusCode}: ${detail}`);
    this.name = 'AetherServerError';
  }
}

function toSnakeCase(str: string): string {
  return str.replace(/[A-Z]/g, (c) => '_' + c.toLowerCase());
}

function keysToSnake(obj: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(obj)) {
    out[toSnakeCase(k)] = v;
  }
  return out;
}

function keysToCamel(obj: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(obj)) {
    const camel = k.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
    out[camel] = v;
  }
  return out;
}

function parseActor(data: Record<string, unknown>): ActorInfo {
  const d = keysToCamel(data);
  return {
    actorId: d.actorId as string,
    actorType: (d.actorType as string) ?? 'default',
    capabilities: (d.capabilities as string[]) ?? [],
    metadata: (d.metadata as Record<string, unknown>) ?? {},
    status: (d.status as string) ?? 'active',
    createdAt: d.createdAt as string,
    lastHeartbeat: (d.lastHeartbeat as string) ?? null,
  };
}

function parseMessage(data: Record<string, unknown>): MessageEnvelope {
  const d = keysToCamel(data);
  return {
    messageId: d.messageId as string,
    sourceActor: d.sourceActor as string,
    targetActor: d.targetActor as string,
    messageType: (d.messageType as string) ?? 'default',
    payload: d.payload,
    correlationId: (d.correlationId as string) ?? null,
    timestamp: d.timestamp as string,
    priority: (d.priority as number) ?? 0,
  };
}

function parseReceipt(data: Record<string, unknown>): DeliveryReceipt {
  const d = keysToCamel(data);
  return {
    messageId: d.messageId as string,
    status: (d.status as string) ?? 'delivered',
    deliveredAt: d.deliveredAt as string,
    correlationId: (d.correlationId as string) ?? null,
  };
}

function parseStateEntry(data: Record<string, unknown>): StateEntry {
  const d = keysToCamel(data);
  return {
    actorId: (d.actorId as string) ?? '',
    key: (d.key as string) ?? '',
    value: d.value,
    version: (d.version as number) ?? 1,
    updatedAt: d.updatedAt as string,
  };
}

function parseEvent(data: Record<string, unknown>): EventRecord {
  const d = keysToCamel(data);
  return {
    eventId: d.eventId as string,
    aggregateId: d.aggregateId as string,
    eventType: d.eventType as string,
    data: d.data,
    version: (d.version as number) ?? 1,
    timestamp: d.timestamp as string,
  };
}

export class AetherClient {
  private baseUrl: string;
  private defaultActorId?: string;
  private fetchFn: typeof fetch;

  constructor(config?: {
    baseUrl?: string;
    timeout?: number;
    actorId?: string;
    fetch?: typeof fetch;
  }) {
    this.baseUrl = (config?.baseUrl ?? 'http://localhost:8080').replace(/\/+$/, '');
    this.defaultActorId = config?.actorId;
    this.fetchFn = config?.fetch ?? globalThis.fetch;
  }

  private async request<T>(path: string, options: RequestInit = {}): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const resp = await this.fetchFn(url, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
    });

    if (resp.status === 204) return undefined as T;

    if (resp.status >= 400) {
      const clone = resp.clone();
      let detail: string;
      try {
        const body = (await clone.json()) as Record<string, unknown>;
        detail = (body.detail as string) ?? JSON.stringify(body);
      } catch {
        detail = await resp.text();
      }
      throw new AetherServerError(resp.status, detail);
    }

    return resp.json() as Promise<T>;
  }

  // === Health ===

  async health(): Promise<ServerInfo> {
    const data = await this.request<Record<string, unknown>>('/health');
    const d = keysToCamel(data);
    return {
      status: (d.status as string) ?? 'ok',
      uptime: (d.uptime as number) ?? 0,
      actorCount: (d.actorCount as number) ?? 0,
      messageCount: (d.messageCount as number) ?? 0,
    };
  }

  async info(): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>('/api/v1/info');
  }

  // === Actors ===

  async registerActor(
    actorId: string,
    actorType = 'default',
    capabilities?: string[],
    metadata?: Record<string, unknown>,
  ): Promise<ActorInfo> {
    const body = keysToSnake({
      actorId,
      actorType,
      capabilities: capabilities ?? [],
      metadata: metadata ?? {},
    });
    const data = await this.request<Record<string, unknown>>('/api/v1/actors', {
      method: 'POST',
      body: JSON.stringify(body),
    });
    return parseActor(data);
  }

  async unregisterActor(actorId: string): Promise<void> {
    await this.request<void>(`/api/v1/actors/${actorId}`, {
      method: 'DELETE',
    });
  }

  async getActor(actorId: string): Promise<ActorInfo> {
    const data = await this.request<Record<string, unknown>>(
      `/api/v1/actors/${actorId}`,
    );
    return parseActor(data);
  }

  async listActors(
    actorType?: string,
    status?: string,
  ): Promise<ActorInfo[]> {
    const params = new URLSearchParams();
    if (actorType !== undefined) params.set('type', actorType);
    if (status !== undefined) params.set('status', status);
    const qs = params.toString();
    const path = `/api/v1/actors${qs ? `?${qs}` : ''}`;
    const data = await this.request<Record<string, unknown>[]>(path);
    return data.map(parseActor);
  }

  async heartbeat(actorId: string): Promise<void> {
    await this.request<void>(`/api/v1/actors/${actorId}/heartbeat`, {
      method: 'POST',
    });
  }

  // === Messaging ===

  async sendMessage(
    target: string,
    payload: unknown,
    options?: {
      source?: string;
      messageType?: string;
      correlationId?: string;
      priority?: number;
    },
  ): Promise<DeliveryReceipt> {
    const body = keysToSnake({
      sourceActor: options?.source ?? this.defaultActorId ?? 'unknown',
      targetActor: target,
      messageType: options?.messageType ?? 'default',
      payload,
      priority: options?.priority ?? 0,
      ...(options?.correlationId != null
        ? { correlation_id: options.correlationId }
        : {}),
    });
    const data = await this.request<Record<string, unknown>>(
      `/api/v1/actors/${target}/messages`,
      {
        method: 'POST',
        body: JSON.stringify(body),
      },
    );
    return parseReceipt(data);
  }

  async getPendingMessages(actorId: string): Promise<MessageEnvelope[]> {
    const data = await this.request<Record<string, unknown>[]>(
      `/api/v1/actors/${actorId}/messages`,
    );
    return data.map(parseMessage);
  }

  // === State ===

  async getState(actorId: string, key: string): Promise<unknown | null> {
    try {
      const data = await this.request<{ value: unknown }>(
        `/api/v1/state/${actorId}/${key}`,
      );
      return data.value;
    } catch (e) {
      if (e instanceof AetherServerError && e.statusCode === 404) return null;
      throw e;
    }
  }

  async setState(
    actorId: string,
    key: string,
    value: unknown,
    version?: number,
  ): Promise<StateEntry> {
    const body: Record<string, unknown> = { value };
    if (version !== undefined) body.version = version;
    const data = await this.request<Record<string, unknown>>(
      `/api/v1/state/${actorId}/${key}`,
      {
        method: 'PUT',
        body: JSON.stringify(body),
      },
    );
    return parseStateEntry(data);
  }

  async deleteState(actorId: string, key: string): Promise<boolean> {
    try {
      await this.request<void>(`/api/v1/state/${actorId}/${key}`, {
        method: 'DELETE',
      });
      return true;
    } catch (e) {
      if (e instanceof AetherServerError && e.statusCode === 404) return false;
      throw e;
    }
  }

  async getAllState(actorId: string): Promise<Record<string, unknown>> {
    const data = await this.request<{ state: Record<string, unknown> }>(
      `/api/v1/state/${actorId}`,
    );
    return data.state ?? {};
  }

  // === Pub/Sub ===

  async publish(
    topic: string,
    payload: unknown,
    headers?: Record<string, string>,
  ): Promise<number> {
    const body = { topic, payload, headers: headers ?? {} };
    const data = await this.request<{ subscriber_count: number }>(
      '/api/v1/events/publish',
      {
        method: 'POST',
        body: JSON.stringify(body),
      },
    );
    return data.subscriber_count ?? 0;
  }

  async subscribe(
    topic: string,
    subscriberId: string,
    filter?: string,
  ): Promise<string> {
    const body: Record<string, unknown> = { topic, subscriber_id: subscriberId };
    if (filter !== undefined) body.filter = filter;
    const data = await this.request<{ subscription_id: string }>(
      '/api/v1/events/subscribe',
      {
        method: 'POST',
        body: JSON.stringify(body),
      },
    );
    return data.subscription_id;
  }

  async unsubscribe(subscriptionId: string): Promise<boolean> {
    try {
      await this.request<void>(
        `/api/v1/events/subscribe/${subscriptionId}`,
        { method: 'DELETE' },
      );
      return true;
    } catch (e) {
      if (e instanceof AetherServerError && e.statusCode === 404) return false;
      throw e;
    }
  }

  async listTopics(): Promise<string[]> {
    return this.request<string[]>('/api/v1/events/topics');
  }

  async getTopicHistory(topic: string, limit = 50): Promise<unknown[]> {
    const params = new URLSearchParams({ limit: String(limit) });
    return this.request<unknown[]>(
      `/api/v1/events/topics/${topic}/history?${params}`,
    );
  }

  // === Event Sourcing ===

  async appendEvent(
    aggregateId: string,
    eventType: string,
    data?: unknown,
    expectedVersion?: number,
  ): Promise<EventRecord> {
    const body: Record<string, unknown> = {
      aggregate_id: aggregateId,
      event_type: eventType,
      data,
    };
    if (expectedVersion !== undefined) body.expected_version = expectedVersion;
    const resp = await this.request<Record<string, unknown>>(
      '/api/v1/events/append',
      {
        method: 'POST',
        body: JSON.stringify(body),
      },
    );
    return parseEvent(resp);
  }

  async getEvents(aggregateId: string): Promise<EventRecord[]> {
    const data = await this.request<Record<string, unknown>[]>(
      `/api/v1/events/${aggregateId}`,
    );
    return data.map(parseEvent);
  }
}
