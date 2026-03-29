/**
 * Aether gRPC Client
 *
 * gRPC client for communicating with an Aether reference server.
 * Uses @grpc/grpc-js with @grpc/proto-loader for dynamic proto loading.
 * Shares the same data model types as AetherClient (HTTP) so callers
 * can swap transports.
 *
 * Usage:
 *
 *     import { AetherGrpcClient } from '@aether/sdk';
 *
 *     const client = new AetherGrpcClient('localhost:50051');
 *     await client.connect();
 *     await client.registerActor('my-actor', 'worker');
 *     await client.setState('my-actor', 'counter', 0);
 *     const value = await client.getState('my-actor', 'counter');
 *     await client.close();
 *
 * Or with auto-connect/disconnect:
 *
 *     const client = await AetherGrpcClient.create('localhost:50051');
 *     // ... use client ...
 *     await client.close();
 */

import * as path from 'path';
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';

import type {
  ActorInfo,
  MessageEnvelope,
  DeliveryReceipt,
  StateEntry,
  EventRecord,
  ServerInfo,
} from './client';

// Re-export the shared types
export type {
  ActorInfo,
  MessageEnvelope,
  DeliveryReceipt,
  StateEntry,
  EventRecord,
  ServerInfo,
} from './client';

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

export class AetherGrpcError extends Error {
  constructor(
    public readonly code: string,
    public readonly detail: string,
  ) {
    super(`gRPC ${code}: ${detail}`);
    this.name = 'AetherGrpcError';
  }
}

// ---------------------------------------------------------------------------
// Proto loading
// ---------------------------------------------------------------------------

const PROTO_PATH = path.join(__dirname, '..', 'proto', 'aether.proto');

function loadProto() {
  const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
    keepCase: false,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
  });
  return grpc.loadPackageDefinition(packageDefinition) as any;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Convert a proto Timestamp-like object to an ISO string (or null). */
function tsToISO(ts: any): string | null {
  if (!ts || (ts.seconds === 0 && ts.nanos === 0)) return null;
  const sec = typeof ts.seconds === 'string' ? parseInt(ts.seconds, 10) : ts.seconds;
  const ms = sec * 1000 + (ts.nanos ?? 0) / 1e6;
  return new Date(ms).toISOString();
}

/** Serialize a JS value to JSON Buffer for gRPC bytes fields. */
function jsonToBuffer(value: unknown): Buffer {
  if (value === undefined || value === null) return Buffer.alloc(0);
  return Buffer.from(JSON.stringify(value));
}

/** Deserialize a gRPC bytes field (Buffer) back to a JS value. */
function bufferToJson(buf: Buffer | Uint8Array): any {
  if (!buf || buf.length === 0) return null;
  return JSON.parse(Buffer.from(buf).toString('utf8'));
}

/** Convert a gRPC ServiceError to an AetherGrpcError. */
function handleRpcError(err: Error): never {
  if ('code' in err && 'details' in err) {
    const svcErr = err as grpc.ServiceError;
    const codeName = grpc.status[svcErr.code] ?? String(svcErr.code);
    throw new AetherGrpcError(codeName, svcErr.details || codeName);
  }
  throw err;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export class AetherGrpcClient {
  private proto: any;
  private channel?: grpc.Client;
  private actors?: any;
  private messages?: any;
  private state?: any;
  private events?: any;
  private healthStub?: any;
  private target: string;
  private timeoutMs: number;
  private defaultActorId?: string;
  private token?: string;
  private secure: boolean;
  private extraMetadata: grpc.Metadata;

  /**
   * Create a new (disconnected) gRPC client.
   * Prefer `AetherGrpcClient.create()` for auto-connect.
   */
  constructor(
    target: string = 'localhost:50051',
    opts?: {
      timeout?: number;        // seconds (default 30)
      actorId?: string;
      token?: string;
      secure?: boolean;
      metadata?: grpc.Metadata;
    },
  ) {
    this.target = target;
    this.timeoutMs = (opts?.timeout ?? 30) * 1000;
    this.defaultActorId = opts?.actorId;
    this.token = opts?.token;
    this.secure = opts?.secure ?? false;
    this.extraMetadata = opts?.metadata ?? new grpc.Metadata();
    this.proto = loadProto();
  }

  // ---- Factory -----------------------------------------------------------

  /**
   * Create and connect to an Aether gRPC server.
   *
   * @param target  gRPC address, e.g. `'localhost:50051'`
   * @param opts    Optional configuration
   * @returns       A connected client ready to use
   */
  static async create(
    target: string,
    opts?: {
      timeout?: number;        // seconds (default 30)
      actorId?: string;
      token?: string;
      secure?: boolean;
      metadata?: grpc.Metadata;
    },
  ): Promise<AetherGrpcClient> {
    const client = new AetherGrpcClient(target, opts);
    await client.connect();
    return client;
  }

  // ---- Lifecycle ---------------------------------------------------------

  /**
   * Establish the gRPC channel and create service stubs.
   * Prefer `AetherGrpcClient.create()` for auto-connect.
   */
  async connect(): Promise<void> {
    const creds = this.secure
      ? grpc.credentials.createSsl()
      : grpc.credentials.createInsecure();

    const pkg = this.proto.aether.server.v1;

    this.channel = new grpc.Client(this.target, creds);
    this.actors = new pkg.ActorService(this.target, creds);
    this.messages = new pkg.MessageService(this.target, creds);
    this.state = new pkg.StateService(this.target, creds);
    this.events = new pkg.EventService(this.target, creds);
    this.healthStub = new pkg.HealthService(this.target, creds);

    // Wait for channel to be ready
    await new Promise<void>((resolve, reject) => {
      const deadline = Date.now() + this.timeoutMs;
      this.channel!.waitForReady(deadline, (err?: Error) => {
        if (err) reject(handleRpcError(err));
        else resolve();
      });
    });
  }

  /** Close the gRPC channel. */
  close(): void {
    if (this.channel) {
      this.channel.close();
      this.channel = undefined;
    }
  }

  // ---- Internals ---------------------------------------------------------

  private ensureConnected(): grpc.Client {
    if (!this.channel) {
      throw new Error("Client not connected. Use 'AetherGrpcClient.create()' or call connect()");
    }
    return this.channel;
  }

  /** Build per-call metadata with optional auth token. */
  private metadata(): grpc.Metadata | undefined {
    const md = this.extraMetadata.clone();
    if (this.token) {
      md.set('authorization', `Bearer ${this.token}`);
    }
    return md;
  }

  /** Promisify a unary gRPC call. */
  private unary<T>(
    stub: any,
    method: string,
    request: any,
  ): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const deadline = new Date(Date.now() + this.timeoutMs);
      stub[method](
        request,
        this.metadata(),
        { deadline },
        (err: grpc.ServiceError | null, resp: T) => {
          if (err) {
            try {
              handleRpcError(err);
            } catch (e) {
              reject(e);
            }
          } else {
            resolve(resp);
          }
        },
      );
    });
  }

  // === Health =============================================================

  async health(): Promise<ServerInfo> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.healthStub, 'health', {});
    return {
      status: resp.status ?? 'ok',
      uptime: resp.uptime ?? 0,
      actorCount: resp.actorCount ?? 0,
      messageCount: resp.messageCount ?? 0,
    };
  }

  async info(): Promise<Record<string, unknown>> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.healthStub, 'info', {});
    return {
      version: resp.version ?? '0.0.0',
      status: resp.status ?? 'ok',
      uptime: resp.uptime ?? 0,
      actorCount: resp.actorCount ?? 0,
      messageCount: resp.messageCount ?? 0,
    };
  }

  // === Actors =============================================================

  async registerActor(
    actorId: string,
    actorType = 'default',
    capabilities?: string[],
    metadata?: Record<string, string>,
  ): Promise<ActorInfo> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.actors, 'register', {
      actorId,
      actorType,
      capabilities: capabilities ?? [],
      metadata: metadata ?? {},
    });
    return this.parseActorInfo(resp);
  }

  async unregisterActor(actorId: string): Promise<void> {
    this.ensureConnected();
    await this.unary<any>(this.actors, 'unregister', { actorId });
  }

  async getActor(actorId: string): Promise<ActorInfo> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.actors, 'getActor', { actorId });
    return this.parseActorInfo(resp);
  }

  async listActors(
    actorType?: string,
    status?: string,
  ): Promise<ActorInfo[]> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.actors, 'listActors', {
      actorType: actorType ?? '',
      status: status ?? '',
    });
    return (resp.actors ?? []).map((a: any) => this.parseActorInfo(a));
  }

  async heartbeat(actorId: string): Promise<void> {
    this.ensureConnected();
    await this.unary<any>(this.actors, 'heartbeat', { actorId });
  }

  // === Messaging ==========================================================

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
    this.ensureConnected();
    const resp = await this.unary<any>(this.messages, 'send', {
      sourceActor: options?.source ?? this.defaultActorId ?? 'unknown',
      targetActor: target,
      messageType: options?.messageType ?? 'default',
      payload: jsonToBuffer(payload),
      correlationId: options?.correlationId ?? '',
      priority: options?.priority ?? 0,
    });
    return {
      messageId: resp.messageId ?? '',
      status: resp.status ?? 'delivered',
      deliveredAt: tsToISO(resp.deliveredAt) ?? new Date().toISOString(),
      correlationId: resp.correlationId || null,
    };
  }

  async getPendingMessages(actorId: string): Promise<MessageEnvelope[]> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.messages, 'getPending', {
      actorId,
    });
    return (resp.messages ?? []).map((m: any) => ({
      messageId: m.messageId ?? '',
      sourceActor: m.sourceActor ?? '',
      targetActor: m.targetActor ?? '',
      messageType: m.messageType ?? 'default',
      payload: bufferToJson(m.payload),
      correlationId: m.correlationId || null,
      timestamp: tsToISO(m.timestamp) ?? new Date().toISOString(),
      priority: m.priority ?? 0,
    }));
  }

  // === State ==============================================================

  async getState(actorId: string, key: string): Promise<unknown> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.state, 'getState', {
      actorId,
      key,
    });
    if (!resp.found) return null;
    return bufferToJson(resp.value);
  }

  async setState(
    actorId: string,
    key: string,
    value: unknown,
    version?: number,
  ): Promise<StateEntry> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.state, 'setState', {
      actorId,
      key,
      value: jsonToBuffer(value),
      expectedVersion: version ?? 0,
    });
    return {
      actorId: resp.actorId ?? actorId,
      key: resp.key ?? key,
      value: bufferToJson(resp.value),
      version: resp.version ?? 1,
      updatedAt: tsToISO(resp.updatedAt) ?? new Date().toISOString(),
    };
  }

  async deleteState(actorId: string, key: string): Promise<boolean> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.state, 'deleteState', {
      actorId,
      key,
    });
    return resp.deleted ?? false;
  }

  async getAllState(actorId: string): Promise<Record<string, unknown>> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.state, 'getAllState', {
      actorId,
    });
    const stateMap: Record<string, unknown> = {};
    if (resp.state) {
      for (const [k, v] of Object.entries(resp.state)) {
        stateMap[k] = bufferToJson(v as Buffer);
      }
    }
    return stateMap;
  }

  // === Pub/Sub ============================================================

  async publish(
    topic: string,
    payload: unknown,
    headers?: Record<string, string>,
  ): Promise<number> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.events, 'publish', {
      topic,
      payload: jsonToBuffer(payload),
      headers: headers ?? {},
    });
    return resp.subscribersNotified ?? 0;
  }

  async subscribe(
    topic: string,
    subscriberId: string,
    filter?: string,
  ): Promise<string> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.events, 'subscribe', {
      topic,
      subscriberId,
      filter: filter ?? '',
    });
    return resp.subscriptionId ?? '';
  }

  async unsubscribe(subscriptionId: string): Promise<boolean> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.events, 'unsubscribe', {
      subscriptionId,
    });
    return resp.success ?? false;
  }

  async listTopics(): Promise<string[]> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.events, 'listTopics', {});
    return resp.topics ?? [];
  }

  // === Event Sourcing =====================================================

  async appendEvent(
    aggregateId: string,
    eventType: string,
    data?: unknown,
    expectedVersion?: number,
  ): Promise<EventRecord> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.events, 'appendEvent', {
      aggregateId,
      eventType,
      data: jsonToBuffer(data),
      expectedVersion: expectedVersion ?? 0,
    });
    return {
      eventId: resp.eventId ?? '',
      aggregateId: resp.aggregateId ?? aggregateId,
      eventType: resp.eventType ?? eventType,
      data: bufferToJson(resp.data),
      version: resp.version ?? 1,
      timestamp: tsToISO(resp.timestamp) ?? new Date().toISOString(),
    };
  }

  async getEvents(aggregateId: string): Promise<EventRecord[]> {
    this.ensureConnected();
    const resp = await this.unary<any>(this.events, 'getEvents', {
      aggregateId,
    });
    return (resp.events ?? []).map((e: any) => ({
      eventId: e.eventId ?? '',
      aggregateId: e.aggregateId ?? aggregateId,
      eventType: e.eventType ?? '',
      data: bufferToJson(e.data),
      version: e.version ?? 1,
      timestamp: tsToISO(e.timestamp) ?? new Date().toISOString(),
    }));
  }

  // === Parser =============================================================

  private parseActorInfo(a: any): ActorInfo {
    return {
      actorId: a.actorId ?? '',
      actorType: a.actorType ?? 'default',
      capabilities: a.capabilities ?? [],
      metadata: a.metadata ?? {},
      status: a.status ?? 'active',
      createdAt: tsToISO(a.createdAt) ?? new Date().toISOString(),
      lastHeartbeat: tsToISO(a.lastHeartbeat),
    };
  }
}
