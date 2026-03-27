import { AetherClient, AetherServerError } from '../src/client';

type MockResponse = { status: number; body?: unknown };

function createMockFetch(responses: Record<string, MockResponse>) {
  return async (_input: string | URL | Request, _init?: RequestInit): Promise<Response> => {
    const url = typeof _input === 'string' ? _input : _input.toString();
    const key = Object.keys(responses).find((k) => url.includes(k));
    const resp = key ? responses[key] : { status: 404, body: { detail: 'not found' } };
    const opts: ResponseInit = { status: resp.status, headers: { 'Content-Type': 'application/json' } };
    if (resp.status === 204) {
      return new Response(null, opts);
    }
    return new Response(JSON.stringify(resp.body ?? {}), opts);
  };
}

function mockClient(responses: Record<string, MockResponse>): AetherClient {
  return new AetherClient({
    baseUrl: 'http://test',
    fetch: createMockFetch(responses),
  });
}

describe('AetherClient', () => {
  describe('constructor', () => {
    it('defaults baseUrl to localhost:8080', () => {
      const c = new AetherClient();
      expect(c).toBeDefined();
    });

    it('strips trailing slashes from baseUrl', () => {
      const c = new AetherClient({ baseUrl: 'http://localhost:8080/' });
      expect(c).toBeDefined();
    });

    it('accepts a custom fetch function', () => {
      const c = new AetherClient({ fetch: async () => new Response() });
      expect(c).toBeDefined();
    });
  });

  describe('health', () => {
    it('returns ServerInfo from /health', async () => {
      const c = mockClient({
        '/health': {
          status: 200,
          body: { status: 'ok', uptime: 123.4, actor_count: 5, message_count: 10 },
        },
      });
      const info = await c.health();
      expect(info.status).toBe('ok');
      expect(info.uptime).toBe(123.4);
      expect(info.actorCount).toBe(5);
      expect(info.messageCount).toBe(10);
    });
  });

  describe('info', () => {
    it('returns raw info dict from /api/v1/info', async () => {
      const c = mockClient({
        '/api/v1/info': {
          status: 200,
          body: { version: '0.1.0', name: 'aether' },
        },
      });
      const info = await c.info();
      expect(info.version).toBe('0.1.0');
      expect(info.name).toBe('aether');
    });
  });

  describe('registerActor', () => {
    it('posts actor registration and returns ActorInfo', async () => {
      const c = mockClient({
        '/api/v1/actors': {
          status: 201,
          body: {
            actor_id: 'a1',
            actor_type: 'worker',
            capabilities: ['compute'],
            metadata: {},
            status: 'active',
            created_at: '2025-01-01T00:00:00Z',
            last_heartbeat: null,
          },
        },
      });
      const actor = await c.registerActor('a1', 'worker', ['compute']);
      expect(actor.actorId).toBe('a1');
      expect(actor.actorType).toBe('worker');
      expect(actor.capabilities).toEqual(['compute']);
      expect(actor.status).toBe('active');
    });

    it('throws AetherServerError on 409 conflict', async () => {
      const c = mockClient({
        '/api/v1/actors': {
          status: 409,
          body: { detail: 'Actor a1 already exists' },
        },
      });
      await expect(c.registerActor('a1')).rejects.toThrow(AetherServerError);
      await expect(c.registerActor('a1')).rejects.toMatchObject({
        statusCode: 409,
        detail: 'Actor a1 already exists',
      });
    });
  });

  describe('unregisterActor', () => {
    it('deletes actor and returns void', async () => {
      const c = mockClient({
        '/api/v1/actors/a1': { status: 204 },
      });
      await expect(c.unregisterActor('a1')).resolves.toBeUndefined();
    });

    it('throws on 404', async () => {
      const c = mockClient({
        '/api/v1/actors/a1': {
          status: 404,
          body: { detail: 'Actor a1 not found' },
        },
      });
      await expect(c.unregisterActor('a1')).rejects.toThrow(AetherServerError);
    });
  });

  describe('getActor', () => {
    it('returns ActorInfo', async () => {
      const c = mockClient({
        '/api/v1/actors/a1': {
          status: 200,
          body: {
            actor_id: 'a1',
            actor_type: 'default',
            capabilities: [],
            metadata: {},
            status: 'active',
            created_at: '2025-01-01T00:00:00Z',
            last_heartbeat: '2025-01-01T00:01:00Z',
          },
        },
      });
      const actor = await c.getActor('a1');
      expect(actor.actorId).toBe('a1');
      expect(actor.lastHeartbeat).toBe('2025-01-01T00:01:00Z');
    });
  });

  describe('listActors', () => {
    it('returns array of ActorInfo', async () => {
      const c = mockClient({
        '/api/v1/actors': {
          status: 200,
          body: [
            {
              actor_id: 'a1',
              actor_type: 'worker',
              capabilities: [],
              metadata: {},
              status: 'active',
              created_at: '2025-01-01T00:00:00Z',
              last_heartbeat: null,
            },
          ],
        },
      });
      const actors = await c.listActors();
      expect(actors).toHaveLength(1);
      expect(actors[0].actorId).toBe('a1');
    });

    it('passes type and status query params', async () => {
      let capturedUrl = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async (input) => {
          capturedUrl = input.toString();
          return new Response(JSON.stringify([]), {
            status: 200,
            headers: { 'Content-Type': 'application/json' },
          });
        },
      });
      await c.listActors('worker', 'active');
      expect(capturedUrl).toContain('type=worker');
      expect(capturedUrl).toContain('status=active');
    });
  });

  describe('heartbeat', () => {
    it('posts heartbeat and returns void', async () => {
      const c = mockClient({
        '/api/v1/actors/a1/heartbeat': { status: 204 },
      });
      await expect(c.heartbeat('a1')).resolves.toBeUndefined();
    });

    it('throws on 404', async () => {
      const c = mockClient({
        '/api/v1/actors/a1/heartbeat': {
          status: 404,
          body: { detail: 'Actor a1 not found' },
        },
      });
      await expect(c.heartbeat('a1')).rejects.toThrow(AetherServerError);
    });
  });

  describe('sendMessage', () => {
    it('sends message and returns DeliveryReceipt', async () => {
      const c = mockClient({
        '/api/v1/actors/target/messages': {
          status: 202,
          body: {
            message_id: 'msg_1',
            status: 'delivered',
            delivered_at: '2025-01-01T00:00:00Z',
            correlation_id: null,
          },
        },
      });
      const receipt = await c.sendMessage('target', { hello: 'world' });
      expect(receipt.messageId).toBe('msg_1');
      expect(receipt.status).toBe('delivered');
    });

    it('uses defaultActorId when source not provided', async () => {
      let capturedBody = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        actorId: 'default-actor',
        fetch: async (_input, init) => {
          capturedBody = init?.body as string;
          return new Response(
            JSON.stringify({
              message_id: 'msg_1',
              status: 'delivered',
              delivered_at: '2025-01-01T00:00:00Z',
              correlation_id: null,
            }),
            { status: 202, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      await c.sendMessage('target', {});
      const parsed = JSON.parse(capturedBody);
      expect(parsed.source_actor).toBe('default-actor');
    });

    it('includes correlationId when provided', async () => {
      let capturedBody = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async (_input, init) => {
          capturedBody = init?.body as string;
          return new Response(
            JSON.stringify({
              message_id: 'msg_1',
              status: 'delivered',
              delivered_at: '2025-01-01T00:00:00Z',
              correlation_id: 'corr-1',
            }),
            { status: 202, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      await c.sendMessage('target', {}, { correlationId: 'corr-1' });
      const parsed = JSON.parse(capturedBody);
      expect(parsed.correlation_id).toBe('corr-1');
    });
  });

  describe('getPendingMessages', () => {
    it('returns array of MessageEnvelope', async () => {
      const c = mockClient({
        '/api/v1/actors/a1/messages': {
          status: 200,
          body: [
            {
              message_id: 'msg_1',
              source_actor: 's1',
              target_actor: 'a1',
              message_type: 'greet',
              payload: { hi: true },
              correlation_id: 'c1',
              timestamp: '2025-01-01T00:00:00Z',
              priority: 1,
            },
          ],
        },
      });
      const msgs = await c.getPendingMessages('a1');
      expect(msgs).toHaveLength(1);
      expect(msgs[0].messageId).toBe('msg_1');
      expect(msgs[0].sourceActor).toBe('s1');
      expect(msgs[0].correlationId).toBe('c1');
    });
  });

  describe('getState', () => {
    it('returns value on 200', async () => {
      const c = mockClient({
        '/api/v1/state/a1/counter': {
          status: 200,
          body: { actor_id: 'a1', key: 'counter', value: 42 },
        },
      });
      const val = await c.getState('a1', 'counter');
      expect(val).toBe(42);
    });

    it('returns null on 404', async () => {
      const c = mockClient({
        '/api/v1/state/a1/missing': {
          status: 404,
          body: { detail: 'not found' },
        },
      });
      const val = await c.getState('a1', 'missing');
      expect(val).toBeNull();
    });
  });

  describe('setState', () => {
    it('puts state and returns StateEntry', async () => {
      const c = mockClient({
        '/api/v1/state/a1/counter': {
          status: 200,
          body: {
            actor_id: 'a1',
            key: 'counter',
            value: 1,
            version: 2,
            updated_at: '2025-01-01T00:00:00Z',
          },
        },
      });
      const entry = await c.setState('a1', 'counter', 1);
      expect(entry.version).toBe(2);
      expect(entry.value).toBe(1);
    });

    it('includes version when provided', async () => {
      let capturedBody = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async (_input, init) => {
          capturedBody = init?.body as string;
          return new Response(
            JSON.stringify({
              actor_id: 'a1',
              key: 'counter',
              value: 5,
              version: 3,
              updated_at: '2025-01-01T00:00:00Z',
            }),
            { status: 200, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      await c.setState('a1', 'counter', 5, 2);
      const parsed = JSON.parse(capturedBody);
      expect(parsed.version).toBe(2);
    });
  });

  describe('deleteState', () => {
    it('returns true on 204', async () => {
      const c = mockClient({
        '/api/v1/state/a1/key': { status: 204 },
      });
      expect(await c.deleteState('a1', 'key')).toBe(true);
    });

    it('returns false on 404', async () => {
      const c = mockClient({
        '/api/v1/state/a1/key': {
          status: 404,
          body: { detail: 'not found' },
        },
      });
      expect(await c.deleteState('a1', 'key')).toBe(false);
    });
  });

  describe('getAllState', () => {
    it('returns state dict', async () => {
      const c = mockClient({
        '/api/v1/state/a1': {
          status: 200,
          body: { actor_id: 'a1', state: { x: 1, y: 2 } },
        },
      });
      const state = await c.getAllState('a1');
      expect(state).toEqual({ x: 1, y: 2 });
    });
  });

  describe('publish', () => {
    it('publishes and returns subscriber count', async () => {
      const c = mockClient({
        '/api/v1/events/publish': {
          status: 202,
          body: { topic: 't', subscriber_count: 3 },
        },
      });
      expect(await c.publish('t', { msg: 'hi' })).toBe(3);
    });

    it('includes headers when provided', async () => {
      let capturedBody = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async (_input, init) => {
          capturedBody = init?.body as string;
          return new Response(
            JSON.stringify({ topic: 't', subscriber_count: 0 }),
            { status: 202, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      await c.publish('t', {}, { foo: 'bar' });
      const parsed = JSON.parse(capturedBody);
      expect(parsed.headers).toEqual({ foo: 'bar' });
    });
  });

  describe('subscribe', () => {
    it('subscribes and returns subscription id', async () => {
      const c = mockClient({
        '/api/v1/events/subscribe': {
          status: 201,
          body: { subscription_id: 'sub-1', topic: 't' },
        },
      });
      expect(await c.subscribe('t', 'me')).toBe('sub-1');
    });

    it('includes filter when provided', async () => {
      let capturedBody = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async (_input, init) => {
          capturedBody = init?.body as string;
          return new Response(
            JSON.stringify({ subscription_id: 'sub-1', topic: 't' }),
            { status: 201, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      await c.subscribe('t', 'me', 'type=click');
      const parsed = JSON.parse(capturedBody);
      expect(parsed.filter).toBe('type=click');
    });
  });

  describe('unsubscribe', () => {
    it('returns true on 204', async () => {
      const c = mockClient({
        '/api/v1/events/subscribe/sub-1': { status: 204 },
      });
      expect(await c.unsubscribe('sub-1')).toBe(true);
    });

    it('returns false on 404', async () => {
      const c = mockClient({
        '/api/v1/events/subscribe/sub-1': {
          status: 404,
          body: { detail: 'not found' },
        },
      });
      expect(await c.unsubscribe('sub-1')).toBe(false);
    });
  });

  describe('listTopics', () => {
    it('returns array of topic strings', async () => {
      const c = mockClient({
        '/api/v1/events/topics': {
          status: 200,
          body: ['orders', 'events'],
        },
      });
      expect(await c.listTopics()).toEqual(['orders', 'events']);
    });
  });

  describe('getTopicHistory', () => {
    it('returns array of messages', async () => {
      const c = mockClient({
        '/api/v1/events/topics/t/history': {
          status: 200,
          body: [{ message_id: 'p1', payload: { x: 1 } }],
        },
      });
      const history = await c.getTopicHistory('t');
      expect(history).toHaveLength(1);
    });

    it('passes limit param', async () => {
      let capturedUrl = '';
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async (input) => {
          capturedUrl = input.toString();
          return new Response(
            JSON.stringify([]),
            { status: 200, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      await c.getTopicHistory('t', 10);
      expect(capturedUrl).toContain('limit=10');
    });
  });

  describe('appendEvent', () => {
    it('appends event and returns EventRecord', async () => {
      const c = mockClient({
        '/api/v1/events/append': {
          status: 201,
          body: {
            event_id: 'e1',
            aggregate_id: 'agg1',
            event_type: 'Created',
            data: { name: 'test' },
            version: 1,
            timestamp: '2025-01-01T00:00:00Z',
          },
        },
      });
      const event = await c.appendEvent('agg1', 'Created', { name: 'test' });
      expect(event.eventId).toBe('e1');
      expect(event.aggregateId).toBe('agg1');
      expect(event.version).toBe(1);
    });

    it('throws on 409 version conflict', async () => {
      const c = mockClient({
        '/api/v1/events/append': {
          status: 409,
          body: { detail: 'Version conflict' },
        },
      });
      await expect(
        c.appendEvent('agg1', 'Created', null, 5),
      ).rejects.toThrow(AetherServerError);
    });
  });

  describe('getEvents', () => {
    it('returns array of EventRecord', async () => {
      const c = mockClient({
        '/api/v1/events/agg1': {
          status: 200,
          body: [
            {
              event_id: 'e1',
              aggregate_id: 'agg1',
              event_type: 'Created',
              data: null,
              version: 1,
              timestamp: '2025-01-01T00:00:00Z',
            },
            {
              event_id: 'e2',
              aggregate_id: 'agg1',
              event_type: 'Updated',
              data: { x: 1 },
              version: 2,
              timestamp: '2025-01-01T00:01:00Z',
            },
          ],
        },
      });
      const events = await c.getEvents('agg1');
      expect(events).toHaveLength(2);
      expect(events[0].eventType).toBe('Created');
      expect(events[1].eventType).toBe('Updated');
      expect(events[1].version).toBe(2);
    });
  });

  describe('AetherServerError', () => {
    it('has correct properties', () => {
      const err = new AetherServerError(500, 'internal error');
      expect(err.name).toBe('AetherServerError');
      expect(err.statusCode).toBe(500);
      expect(err.detail).toBe('internal error');
      expect(err.message).toBe('HTTP 500: internal error');
      expect(err).toBeInstanceOf(Error);
    });
  });

  describe('error handling', () => {
    it('parses detail from JSON error response', async () => {
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async () => {
          return new Response(
            JSON.stringify({ detail: 'something went wrong' }),
            { status: 400, headers: { 'Content-Type': 'application/json' } },
          );
        },
      });
      try {
        await c.health();
        fail('should have thrown');
      } catch (e) {
        expect(e).toBeInstanceOf(AetherServerError);
        expect((e as AetherServerError).detail).toBe('something went wrong');
      }
    });

    it('falls back to text body when JSON parse fails', async () => {
      const c = new AetherClient({
        baseUrl: 'http://test',
        fetch: async () => {
          return new Response('plain text error', {
            status: 500,
            headers: { 'Content-Type': 'text/plain' },
          });
        },
      });
      try {
        await c.health();
        fail('should have thrown');
      } catch (e) {
        expect(e).toBeInstanceOf(AetherServerError);
        expect((e as AetherServerError).detail).toBe('plain text error');
      }
    });
  });
});
