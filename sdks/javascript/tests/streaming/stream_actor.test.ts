/**
 * Tests for Stream Actor Base Class
 */

import { StreamActor, StreamingStateHandle } from '../../src/streaming/stream_actor';
import { Actor } from '../../src/actor';
import { Message, MessageType } from '../../src/messaging';
import { StateHandle } from '../../src/state';
import {
  StreamEvent,
  Watermark,
  Timestamp,
  Duration,
  WindowType,
  LateDataPolicy,
  WatermarkStrategy,
  createStreamEvent,
  createWindowSpec,
  createStreamConfig,
} from '../../src/streaming/types';

// Create a concrete test implementation
class TestStreamActor extends StreamActor<string, number> {
  public processedEvents: StreamEvent<number>[] = [];
  public processEventCalled = false;

  static get name(): string {
    return 'test-stream-actor';
  }

  async processEvent(event: StreamEvent<number>): Promise<void> {
    this.processEventCalled = true;
    this.processedEvents.push(event);
  }
}

// Mock StateHandle for testing - extends StateHandle to get all methods
class MockStateHandle extends StateHandle {
  async keys(): Promise<string[]> {
    return Array.from(this['store'].keys());
  }
}

describe('StreamingStateHandle', () => {
  let stateHandle: MockStateHandle;
  let streamingHandle: StreamingStateHandle;

  beforeEach(() => {
    stateHandle = new MockStateHandle();
    streamingHandle = new StreamingStateHandle(stateHandle);
  });

  describe('getValue', () => {
    test('returns value when exists', async () => {
      await stateHandle.setString('key', JSON.stringify({ name: 'test' }));

      
      const result = await streamingHandle.getValue<{ name: string }>('key');
      
      expect(result).toEqual({ name: 'test' });
    });

 
    test('returns undefined when not exists', async () => {
      const result = await streamingHandle.getValue('nonexistent');
      
      expect(result).toBeUndefined();
    });
 
    test('returns default value when not exists', async () => {
      const result = await streamingHandle.getValue('nonexistent', 'default');
      
      expect(result).toBe('default');
    });
  });
 
  describe('setValue', () => {
    test('stores value as JSON', async () => {
      await streamingHandle.setValue('key', { count: 42 });
      
      const stored = await stateHandle.getString('key');
      expect(stored).toBe(JSON.stringify({ count: 42 }));
    });
  });
 
  describe('getList', () => {
    test('returns empty array when not exists', async () => {
      const result = await streamingHandle.getList<number>('key');
      
      expect(result).toEqual([]);
    });
 
    test('returns array from stored JSON', async () => {
      await stateHandle.setString('key', JSON.stringify([1, 2, 3]));
      
      const result = await streamingHandle.getList<number>('key');
      
      expect(result).toEqual([1, 2, 3]);
    });
 
    test('wraps single value in array', async () => {
      await stateHandle.setString('key', JSON.stringify(42));
      
      const result = await streamingHandle.getList<number>('key');
      
      expect(result).toEqual([42]);
    });
  });
 
  describe('appendToList', () => {
    test('appends item to existing list', async () => {
      await stateHandle.setString('key', JSON.stringify([1, 2]));
      
      await streamingHandle.appendToList('key', 3);
      
      const result = await streamingHandle.getList<number>('key');
      expect(result).toEqual([1, 2, 3]);
    });
 
    test('creates new list when not exists', async () => {
      await streamingHandle.appendToList('key', 1);
      
      const result = await streamingHandle.getList<number>('key');
      expect(result).toEqual([1]);
    });
  });
 
  describe('clearList', () => {
    test('clears list to empty array', async () => {
      await stateHandle.setString('key', JSON.stringify([1, 2, 3]));
      
      await streamingHandle.clearList('key');
      
      const result = await streamingHandle.getList<number>('key');
      expect(result).toEqual([]);
    });
  });
 
  describe('getMap', () => {
    test('returns empty object when not exists', async () => {
      const result = await streamingHandle.getMap<'a' | 'b', number>('key');
      
      expect(result).toEqual({});
    });
 
    test('returns stored map', async () => {
      await stateHandle.setString('key', JSON.stringify({ a: 1, b: 2 }));
      
      const result = await streamingHandle.getMap<'a' | 'b', number>('key');
      
      expect(result).toEqual({ a: 1, b: 2 });
    });
  });
 
  describe('putInMap', () => {
    test('adds entry to map', async () => {
      await streamingHandle.putInMap('key', 'a', 1);
      
      const result = await streamingHandle.getMap<'a', number>('key');
      expect(result).toEqual({ a: 1 });
    });
 
    test('updates existing entry', async () => {
      await streamingHandle.putInMap('key', 'a', 1);
      await streamingHandle.putInMap('key', 'a', 2);
      
      const result = await streamingHandle.getMap<'a', number>('key');
      expect(result).toEqual({ a: 2 });
    });
  });
 
  describe('removeFromMap', () => {
    test('removes entry and returns value', async () => {
      await streamingHandle.putInMap('key', 'a', 1);
      
      const removed = await streamingHandle.removeFromMap<'a', number>('key', 'a');
      
      expect(removed).toBe(1);
      const result = await streamingHandle.getMap<'a', number>('key');
      expect(result).toEqual({});
    });
 
    test('returns undefined for non-existent key', async () => {
      const removed = await streamingHandle.removeFromMap('key', 'nonexistent');
      
      expect(removed).toBeUndefined();
    });
  });
 
  describe('clearMap', () => {
    test('clears map to empty object', async () => {
      await streamingHandle.putInMap('key', 'a', 1);
      await streamingHandle.putInMap('key', 'b', 2);
      
      await streamingHandle.clearMap('key');
      
      const result = await streamingHandle.getMap('key');
      expect(result).toEqual({});
    });
  });
});
 
describe('StreamActor', () => {
  let actor: TestStreamActor;
 
  beforeEach(() => {
    actor = new TestStreamActor();
  });
 
  describe('constructor', () => {
    test('creates actor with default config', () => {
      expect(actor).toBeInstanceOf(Actor);
      expect(actor['streamConfig']).toBeDefined();
      expect(actor['streamState']).toBeDefined();
      expect(actor['backpressure']).toBeDefined();
    });
 
    test('creates actor with custom config', () => {
      const customActor = new TestStreamActor(
        createStreamConfig({
          inputStreams: ['input1'],
          outputStreams: ['output1'],
          parallelism: 4,
          partitionStrategy: 'hash',
        })
      );
      
      expect(customActor['streamConfig'].parallelism).toBe(4);
      expect(customActor['streamConfig'].partitionStrategy).toBe('hash');
    });
  });
 
  describe('processEvent', () => {
    test('abstract method must be implemented', async () => {
      const event: StreamEvent<number> = createStreamEvent('key1', 42, Timestamp.now());
      
      // Access internal method to test
      await actor['processEventInternal'](event);
      
      expect(actor.processEventCalled).toBe(true);
      expect(actor.processedEvents).toContain(event);
    });
  });
 
  describe('handle', () => {
    test('processes stream_event message', async () => {
      const event: StreamEvent<number> = {
        key: 'key1',
        value: 42,
        timestamp: Timestamp.now(),
        headers: {},
      };

      const message = Message.custom({ type: 'stream_event', ...event });
      
      await actor.handle('sender', message);
      
      expect(actor.processedEvents).toHaveLength(1);
    });

    test('processes custom message with event data', async () => {
      const eventData = {
        key: 'key1',
        value: 42,
        timestamp: Date.now(),
        headers: {},
      };

      const message = Message.custom(eventData);

      await actor.handle('sender', message);
      
      expect(actor.processedEvents).toHaveLength(1);
    });

    test('processes watermark message', async () => {
      const watermarkData = {
        timestamp: Date.now(),
        streamId: 'stream1',
      };

      const message = Message.custom({ type: 'watermark', ...watermarkData });

      await actor.handle('sender', message);
      
      const watermark = actor.getWatermark('stream1');
      expect(watermark).toBeDefined();
      expect(watermark!.milliseconds).toBe(watermarkData.timestamp);
    });

    test('ignores invalid message types', async () => {
      const message = Message.custom({ type: 'unknown_type', data: {} });

      await actor.handle('sender', message);
      
      expect(actor.processedEvents).toHaveLength(0);
    })
  });
 
  describe('watermark management', () => {
    test('advanceWatermark updates watermark', async () => {
      const watermark = new Watermark(new Timestamp(1000), 'stream1');
      
      await actor.advanceWatermark(watermark);
      
      const current = actor.getWatermark('stream1');
      expect(current).toBeDefined();
      expect(current!.milliseconds).toBe(1000);
    });
 
    test('advanceWatermark only advances forward', async () => {
      await actor.advanceWatermark(new Watermark(new Timestamp(1000), 'stream1'));
      
      await actor.advanceWatermark(new Watermark(new Timestamp(500), 'stream1')); // Earlier timestamp
      
      const current = actor.getWatermark('stream1');
      expect(current!.milliseconds).toBe(1000);
    });
 
    test('getWatermark returns undefined for unknown stream', () => {
      const watermark = actor.getWatermark('unknown');
      expect(watermark).toBeUndefined();
    });
  });
 
  describe('emit', () => {
    test('emitValue sends event to output handler', async () => {
      const receivedEvents: StreamEvent<unknown>[] = [];
      actor.registerOutputHandler('output', (event) => {
        receivedEvents.push(event);
      });
      
      await actor.emitValue('output', { data: 'test' });
      
      expect(receivedEvents).toHaveLength(1);
      expect(receivedEvents[0].value).toEqual({ data: 'test' });
    });
 
    test('emitValueWithTimestamp uses provided timestamp', async () => {
      const receivedEvents: StreamEvent<unknown>[] = [];
      actor.registerOutputHandler('output', (event) => {
        receivedEvents.push(event);
      });
      
      const ts = new Timestamp(12345);
      await actor.emitValueWithTimestamp('output', { data: 'test' }, ts);
      
      expect(receivedEvents[0].timestamp.milliseconds).toBe(12345);
    });
 
    test('emitEvent sends pre-constructed event', async () => {
      const receivedEvents: StreamEvent<unknown>[] = [];
      actor.registerOutputHandler('output', (event) => {
        receivedEvents.push(event);
      });
      
      const event: StreamEvent<unknown> = {
        key: 'test-key',
        value: { data: 'test' },
        timestamp: new Timestamp(1000),
        headers: { source: 'test' },
      }
      
      await actor.emitEvent('output', event);
      
      expect(receivedEvents[0]).toEqual(event);
    });
  });
 
  describe('late event handling', () => {
    test('late events are tracked', async () => {
      // Set up watermark
      await actor.advanceWatermark(new Watermark(new Timestamp(2000), 'default'));
      
      // Send late event (before watermark)
      const lateEvent: StreamEvent<number> = {
        key: 'key1',
        value: 42,
        timestamp: new Timestamp(1000), // Before watermark
        eventType: 'default',
      }
      
      await actor['processEventInternal'](lateEvent);
      
      const metrics = actor.getMetrics();
      expect(metrics.lateEventsCount).toBe(1);
    });
  });
 
  describe('getMetrics', () => {
    test('returns stream metrics', async () => {
      const event: StreamEvent<number> = createStreamEvent('key1', 42, Timestamp.now());
      
      await actor['processEventInternal'](event);
      
      const metrics = actor.getMetrics();
      
      expect(metrics.processedCount).toBe(1);
      expect(metrics.lateEventsCount).toBe(0);
      expect(metrics.backpressure).toBeDefined();
    });
  });
 
  describe('registerOutputHandler', () => {
    test('registers handler for output stream', async () => {
      const handler = jest.fn();
      actor.registerOutputHandler('output', handler);
      
      await actor.emitValue('output', { test: 'data' });
      
      expect(handler).toHaveBeenCalled();
    });
  });
 
  describe('registerLateDataHandler', () => {
    test('registers handler for late data', () => {
      const handler = jest.fn();
      actor.registerLateDataHandler(handler);
      
      expect(actor['lateDataHandler']).toBe(handler);
    });
  });
 
  describe('windowing', () => {
    test('configureWindow sets up windowing', () => {
      const spec = createWindowSpec(WindowType.Tumbling, Duration.fromSeconds(5));
      
      actor.configureWindow(spec, (events, info) => events.length);
      
      expect(actor['windowAssigner']).toBeDefined();
      expect(actor['windowTrigger']).toBeDefined();
    });
  });
 
  describe('state management', () => {
    test('getState returns value from streaming state', async () => {
      await actor.setState('counter', 42);
      
      const value = await actor.getState<number>('counter');
      
      expect(value).toBe(42);
    });
 
    test('setState stores value', async () => {
      await actor.setState('name', 'test');
      
      const value = await actor.getState<string>('name');
      
      expect(value).toBe('test');
    });
 
    test('getListState returns list', async () => {
      await actor.updateListState('items', 'item1');
      await actor.updateListState('items', 'item2');
      
      const list = await actor.getListState<string>('items');
      
      expect(list).toContain('item1');
      expect(list).toContain('item2');
    });
 
    test('getMapState returns map', async () => {
      await actor.updateMapState('map', 'key1', 'value1');
      await actor.updateMapState('map', 'key2', 'value2');
      
      const map = await actor.getMapState<'key1' | 'key2', string>('map');
      
      expect(map.key1).toBe('value1');
      expect(map.key2).toBe('value2');
    });
  });
 
  describe('extractKey', () => {
    test('extracts key from event', () => {
      const event: StreamEvent<number> = {
        key: 'partition-key',
        value: 42,
        timestamp: Timestamp.now(),
      }
      
      const key = actor['extractKey'](event);
      
      expect(key).toBe('partition-key');
    });
  });
});
