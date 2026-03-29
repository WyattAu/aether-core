/**
 * Tests for AetherGrpcClient
 *
 * Uses a minimal in-process gRPC server that implements the Aether proto
 * services with simple in-memory handlers. This avoids importing the real
 * server (which would cause proto symbol conflicts).
 */

import * as path from 'path';
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';

import { AetherGrpcClient, AetherGrpcError } from '../src/grpc_client';

// ---------------------------------------------------------------------------
// Minimal test server
// ---------------------------------------------------------------------------

const PROTO_PATH = path.resolve(__dirname, '..', 'proto', 'aether.proto');

function loadPackageDefinition() {
  return protoLoader.loadSync(PROTO_PATH, {
    keepCase: false,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
  });
}

function getPackage(proto: any) {
  const pd = grpc.loadPackageDefinition(proto);
  return (pd as any).aether.server.v1;
}

// In-memory stores
const actorStore = new Map<string, any>();
const stateStore = new Map<string, any>();
const subscribers = new Map<string, any>();
const events = new Map<string, any[]>();

function makeTimestamp() {
  const now = Date.now();
  return { seconds: Math.floor(now / 1000), nanos: (now % 1000) * 1e6 };
}

function makeHandlers(pkg: any) {
  return {
    actor: {
      register: (call: any, cb: any) => {
        const req = call.request;
        const actor = {
          actorId: req.actorId,
          actorType: req.actorType || 'default',
          capabilities: req.capabilities || [],
          metadata: req.metadata || {},
          status: 'active',
          createdAt: makeTimestamp(),
          lastHeartbeat: null,
        };
        actorStore.set(req.actorId, actor);
        cb(null, actor);
      },
      unregister: (_call: any, cb: any) => {
        const id = _call.request.actorId;
        actorStore.delete(id);
        cb(null, { success: true });
      },
      getActor: (call: any, cb: any) => {
        const actor = actorStore.get(call.request.actorId);
        if (!actor) cb({ code: grpc.status.NOT_FOUND, details: 'Actor not found' });
        else cb(null, actor);
      },
      listActors: (_call: any, cb: any) => {
        const all = Array.from(actorStore.values());
        cb(null, { actors: all, total: all.length });
      },
      heartbeat: (call: any, cb: any) => {
        const actor = actorStore.get(call.request.actorId);
        if (actor) {
          actor.lastHeartbeat = makeTimestamp();
          actorStore.set(call.request.actorId, actor);
        }
        cb(null, { success: true });
      },
    },
    message: {
      send: (call: any, cb: any) => {
        const req = call.request;
        const target = actorStore.get(req.targetActor);
        const status = target ? 'delivered' : 'buffered';
        cb(null, {
          messageId: `msg-${Date.now()}`,
          status,
          deliveredAt: makeTimestamp(),
          correlationId: req.correlationId || null,
        });
      },
      getPending: (_call: any, cb: any) => {
        cb(null, { messages: [] });
      },
    },
    state: {
      getState: (call: any, cb: any) => {
        const { actorId, key } = call.request;
        const fullKey = `${actorId}:${key}`;
        const entry = stateStore.get(fullKey);
        if (!entry) {
          cb(null, { found: false, key, value: Buffer.alloc(0), version: 0 });
        } else {
          cb(null, { ...entry, found: true });
        }
      },
      setState: (call: any, cb: any) => {
        const { actorId, key, value, expectedVersion } = call.request;
        const fullKey = `${actorId}:${key}`;
        const existing = stateStore.get(fullKey);
        const version = (existing?.version ?? 0) + 1;
        const entry = {
          actorId,
          key,
          value,
          version,
          updatedAt: makeTimestamp(),
        };
        stateStore.set(fullKey, entry);
        cb(null, entry);
      },
      deleteState: (call: any, cb: any) => {
        const { actorId, key } = call.request;
        const fullKey = `${actorId}:${key}`;
        const deleted = stateStore.delete(fullKey);
        cb(null, { deleted });
      },
      getAllState: (call: any, cb: any) => {
        const prefix = `${call.request.actorId}:`;
        const result: Record<string, Buffer> = {};
        for (const [k, v] of stateStore.entries()) {
          if (k.startsWith(prefix)) {
            const key = k.slice(prefix.length);
            result[key] = (v as any).value;
          }
        }
        cb(null, { state: result });
      },
    },
    event: {
      publish: (call: any, cb: any) => {
        cb(null, { subscribersNotified: 0 });
      },
      subscribe: (call: any, cb: any) => {
        const { topic, subscriberId } = call.request;
        const subId = `sub-${Date.now()}`;
        subscribers.set(subId, { topic, subscriberId });
        cb(null, { subscriptionId: subId });
      },
      unsubscribe: (call: any, cb: any) => {
        const deleted = subscribers.delete(call.request.subscriptionId);
        cb(null, { success: deleted });
      },
      listTopics: (_call: any, cb: any) => {
        const topics = new Set<string>();
        for (const s of subscribers.values()) {
          topics.add((s as any).topic);
        }
        cb(null, { topics: Array.from(topics) });
      },
      appendEvent: (call: any, cb: any) => {
        const { aggregateId, eventType, data } = call.request;
        const agg = events.get(aggregateId) || [];
        const version = agg.length + 1;
        const record = {
          eventId: `evt-${Date.now()}`,
          aggregateId,
          eventType,
          data,
          version,
          timestamp: makeTimestamp(),
        };
        agg.push(record);
        events.set(aggregateId, agg);
        cb(null, record);
      },
      getEvents: (call: any, cb: any) => {
        const agg = events.get(call.request.aggregateId) || [];
        cb(null, { events: agg });
      },
    },
    health: {
      health: (_call: any, cb: any) => {
        cb(null, {
          status: 'ok',
          uptime: 1234.5,
          actorCount: actorStore.size,
          messageCount: 0,
        });
      },
      ready: (_call: any, cb: any) => {
        cb(null, { status: 'ok', uptime: 1234.5, actorCount: 0, messageCount: 0 });
      },
      info: (_call: any, cb: any) => {
        cb(null, {
          version: '1.7.0',
          status: 'ok',
          uptime: 1234.5,
          actorCount: actorStore.size,
          messageCount: 0,
        });
      },
    },
  };
}

// ---------------------------------------------------------------------------
// Test utilities
// ---------------------------------------------------------------------------

let server: grpc.Server;
const TEST_PORT = 'localhost:0';

function setupServer(): Promise<number> {
  return new Promise((resolve, reject) => {
    server = new grpc.Server();
    const proto = loadPackageDefinition();
    const pkg = getPackage(proto);
    const handlers = makeHandlers(pkg);

    server.addService(pkg.ActorService.service, handlers.actor);
    server.addService(pkg.MessageService.service, handlers.message);
    server.addService(pkg.StateService.service, handlers.state);
    server.addService(pkg.EventService.service, handlers.event);
    server.addService(pkg.HealthService.service, handlers.health);

    server.bindAsync(
      TEST_PORT,
      grpc.ServerCredentials.createInsecure(),
      (err, port) => {
        if (err) reject(err);
        else resolve(port);
      },
    );
  });
}

function teardownServer(): Promise<void> {
  return new Promise((resolve, reject) => {
    if (server) {
      server.tryShutdown(() => {
        server = undefined as any;
        resolve();
      });
    } else {
      resolve();
    }
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AetherGrpcClient', () => {
  let port: number;
  let client: AetherGrpcClient;

  beforeAll(async () => {
    port = await setupServer();
  });

  afterAll(async () => {
    if (client) client.close();
    await teardownServer();
  });

  beforeEach(() => {
    actorStore.clear();
    stateStore.clear();
    subscribers.clear();
    events.clear();
  });

  afterEach(() => {
    if (client) client.close();
  });

  // --- Construction & Connection ------------------------------------------

  describe('construction', () => {
    test('create() returns a connected client', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      expect(client).toBeInstanceOf(AetherGrpcClient);
    });

    test('create() with options', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`, {
        timeout: 5,
        actorId: 'test-actor',
        token: 'secret-token',
      });
      expect(client).toBeInstanceOf(AetherGrpcClient);
    });

    test('calling methods before connect throws', async () => {
      const raw = new AetherGrpcClient(`localhost:${port}`, { timeout: 30 });
      await expect(raw.health()).rejects.toThrow("Client not connected");
    });
  });

  // --- Health -------------------------------------------------------------

  describe('health', () => {
    test('health() returns server info', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const info = await client.health();
      expect(info.status).toBe('ok');
      expect(info.uptime).toBeGreaterThan(0);
      expect(typeof info.actorCount).toBe('number');
    });

    test('info() returns version and status', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const info = await client.info();
      expect(info.version).toBe('1.7.0');
      expect(info.status).toBe('ok');
    });
  });

  // --- Actors -------------------------------------------------------------

  describe('actors', () => {
    test('registerActor() returns ActorInfo', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const actor = await client.registerActor('actor-1', 'worker', ['compute']);
      expect(actor.actorId).toBe('actor-1');
      expect(actor.actorType).toBe('worker');
      expect(actor.capabilities).toEqual(['compute']);
      expect(actor.status).toBe('active');
      expect(actor.createdAt).toBeTruthy();
    });

    test('registerActor() with metadata', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const actor = await client.registerActor(
        'actor-meta',
        'default',
        [],
        { region: 'us-east' },
      );
      expect(actor.metadata).toEqual({ region: 'us-east' });
    });

    test('getActor() returns registered actor', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.registerActor('actor-2');
      const actor = await client.getActor('actor-2');
      expect(actor.actorId).toBe('actor-2');
      expect(actor.status).toBe('active');
    });

    test('getActor() throws for unknown actor', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await expect(client.getActor('nonexistent')).rejects.toThrow('NOT_FOUND');
    });

    test('listActors() returns all actors', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.registerActor('actor-3a');
      await client.registerActor('actor-3b');
      const list = await client.listActors();
      expect(list.length).toBe(2);
      expect(list.map((a) => a.actorId).sort()).toEqual(['actor-3a', 'actor-3b']);
    });

    test('unregisterActor() removes actor', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.registerActor('actor-4');
      await client.unregisterActor('actor-4');
      await expect(client.getActor('actor-4')).rejects.toThrow('NOT_FOUND');
    });

    test('heartbeat() succeeds for registered actor', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.registerActor('actor-5');
      // Should not throw
      await client.heartbeat('actor-5');
      const actor = await client.getActor('actor-5');
      expect(actor.lastHeartbeat).toBeTruthy();
    });
  });

  // --- Messaging ----------------------------------------------------------

  describe('messaging', () => {
    test('sendMessage() to registered actor returns delivered', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.registerActor('msg-target');
      const receipt = await client.sendMessage('msg-target', { hello: 'world' });
      expect(receipt.messageId).toBeTruthy();
      expect(receipt.status).toBe('delivered');
      expect(receipt.deliveredAt).toBeTruthy();
    });

    test('sendMessage() to unknown actor returns buffered', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const receipt = await client.sendMessage('ghost', { data: true });
      expect(receipt.status).toBe('buffered');
    });

    test('sendMessage() with options', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`, {
        actorId: 'sender',
      });
      const receipt = await client.sendMessage('target', 'payload', {
        messageType: 'custom',
        correlationId: 'corr-123',
        priority: 5,
      });
      expect(receipt.messageId).toBeTruthy();
    });

    test('getPendingMessages() returns empty array', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const msgs = await client.getPendingMessages('actor-1');
      expect(msgs).toEqual([]);
    });
  });

  // --- State --------------------------------------------------------------

  describe('state', () => {
    test('getState() returns null for missing key', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const val = await client.getState('actor-1', 'missing');
      expect(val).toBeNull();
    });

    test('setState() and getState() roundtrip', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const entry = await client.setState('actor-1', 'counter', 42);
      expect(entry.key).toBe('counter');
      expect(entry.version).toBe(1);
      expect(entry.value).toBe(42);

      const val = await client.getState('actor-1', 'counter');
      expect(val).toBe(42);
    });

    test('setState() with object value', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const obj = { name: 'test', nested: { a: 1 } };
      await client.setState('actor-1', 'config', obj);
      const val = await client.getState('actor-1', 'config');
      expect(val).toEqual(obj);
    });

    test('setState() increments version on update', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const e1 = await client.setState('actor-1', 'key', 'v1');
      expect(e1.version).toBe(1);
      const e2 = await client.setState('actor-1', 'key', 'v2');
      expect(e2.version).toBe(2);
    });

    test('deleteState() returns true for existing key', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.setState('actor-1', 'temp', 'val');
      const deleted = await client.deleteState('actor-1', 'temp');
      expect(deleted).toBe(true);
      const val = await client.getState('actor-1', 'temp');
      expect(val).toBeNull();
    });

    test('deleteState() returns false for missing key', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const deleted = await client.deleteState('actor-1', 'nonexistent');
      expect(deleted).toBe(false);
    });

    test('getAllState() returns all keys for actor', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.setState('actor-1', 'a', 1);
      await client.setState('actor-1', 'b', 2);
      const state = await client.getAllState('actor-1');
      expect(state).toEqual({ a: 1, b: 2 });
    });

    test('getAllState() returns empty object for unknown actor', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const state = await client.getAllState('ghost');
      expect(state).toEqual({});
    });
  });

  // --- Pub/Sub ------------------------------------------------------------

  describe('pubsub', () => {
    test('subscribe() returns subscription ID', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const subId = await client.subscribe('topic-1', 'sub-1');
      expect(subId).toBeTruthy();
    });

    test('unsubscribe() returns true for active subscription', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const subId = await client.subscribe('topic-2', 'sub-2');
      const result = await client.unsubscribe(subId);
      expect(result).toBe(true);
    });

    test('unsubscribe() returns false for unknown subscription', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const result = await client.unsubscribe('ghost-sub');
      expect(result).toBe(false);
    });

    test('publish() returns subscriber count', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const count = await client.publish('topic-3', { msg: 'hello' });
      expect(typeof count).toBe('number');
    });

    test('listTopics() returns subscribed topics', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.subscribe('topic-a', 'sub-a');
      await client.subscribe('topic-b', 'sub-b');
      const topics = await client.listTopics();
      expect(topics).toContain('topic-a');
      expect(topics).toContain('topic-b');
    });
  });

  // --- Event Sourcing -----------------------------------------------------

  describe('event sourcing', () => {
    test('appendEvent() returns EventRecord', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const record = await client.appendEvent('agg-1', 'UserCreated', { name: 'Alice' });
      expect(record.eventId).toBeTruthy();
      expect(record.aggregateId).toBe('agg-1');
      expect(record.eventType).toBe('UserCreated');
      expect(record.data).toEqual({ name: 'Alice' });
      expect(record.version).toBe(1);
    });

    test('appendEvent() increments version', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.appendEvent('agg-2', 'Event1');
      const e2 = await client.appendEvent('agg-2', 'Event2');
      expect(e2.version).toBe(2);
    });

    test('getEvents() returns all events for aggregate', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      await client.appendEvent('agg-3', 'Created', { x: 1 });
      await client.appendEvent('agg-3', 'Updated', { x: 2 });
      const evts = await client.getEvents('agg-3');
      expect(evts.length).toBe(2);
      expect(evts[0].eventType).toBe('Created');
      expect(evts[1].eventType).toBe('Updated');
    });

    test('getEvents() returns empty for unknown aggregate', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);
      const evts = await client.getEvents('ghost-agg');
      expect(evts).toEqual([]);
    });
  });

  // --- Error Handling -----------------------------------------------------

  describe('error handling', () => {
    test('AetherGrpcError has code and detail', () => {
      const err = new AetherGrpcError('NOT_FOUND', 'Actor not found');
      expect(err.code).toBe('NOT_FOUND');
      expect(err.detail).toBe('Actor not found');
      expect(err.message).toBe('gRPC NOT_FOUND: Actor not found');
      expect(err.name).toBe('AetherGrpcError');
    });
  });

  // --- Integration --------------------------------------------------------

  describe('integration', () => {
    test('full workflow: register, state, message, events', async () => {
      client = await AetherGrpcClient.create(`localhost:${port}`);

      // Register
      const actor = await client.registerActor('wf-actor', 'workflow');
      expect(actor.actorId).toBe('wf-actor');

      // State
      await client.setState('wf-actor', 'step', 'init');
      const step = await client.getState('wf-actor', 'step');
      expect(step).toBe('init');

      // Message
      const receipt = await client.sendMessage('wf-actor', { action: 'start' });
      expect(receipt.status).toBe('delivered');

      // Events
      const evt = await client.appendEvent('wf-actor', 'StepCompleted', { step: 'init' });
      expect(evt.eventType).toBe('StepCompleted');

      const evts = await client.getEvents('wf-actor');
      expect(evts.length).toBe(1);

      // Cleanup
      await client.unregisterActor('wf-actor');
    });
  });
});
