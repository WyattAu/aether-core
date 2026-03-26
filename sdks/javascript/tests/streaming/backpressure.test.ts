/**
 * Tests for Backpressure Controller
 * @module aether/streaming/backpressure
 */

import {
  BackpressureController,
  BackpressureError,
  BufferFullError,
  MultiLevelBackpressure,
  RateBasedBackpressure,
  DEFAULT_BACKPRESSURE_CONFIG,
} from '../../src/streaming/backpressure';
import {
  Timestamp,
  Duration,
  BackpressureStrategy,
  StreamEvent,
} from '../../src/streaming/types';

// Helper to create test events
function createTestEvent(value: number, ts?: number): StreamEvent<number> {
  return {
    key: `key-${value}`,
    value,
    timestamp: new Timestamp(ts ?? Date.now()),
  };
}

describe('BackpressureController', () => {
  describe('constructor', () => {
    test('should create with default config', () => {
      const controller = new BackpressureController();
      expect(controller.size()).toBe(0);
      expect(controller.isEmpty()).toBe(true);
    });

    test('should create with custom config', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Drop,
        bufferSize: 100,
        highWatermark: 0.8,
        lowWatermark: 0.3,
      });

      const stats = controller.getStats();
      expect(stats.totalEvents).toBe(0);
    });
  });

  describe('tryPush', () => {
    test('should accept events when buffer has room', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      const event = createTestEvent(1);
      const result = controller.tryPush(event);

      expect(result).toBe(true);
      expect(controller.size()).toBe(1);
    });

    test('should track statistics', () => {
      const controller = new BackpressureController();

      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));

      const stats = controller.getStats();
      expect(stats.totalEvents).toBe(2);
      expect(stats.bufferedEvents).toBe(2);
      expect(stats.currentBufferSize).toBe(2);
    });
  });

  describe('Buffer strategy', () => {
    test('should reject when buffer is full', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 2,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));
      const result = controller.tryPush(createTestEvent(3));

      expect(result).toBe(false);
      const stats = controller.getStats();
      expect(stats.rejectedEvents).toBe(1);
    });
  });

  describe('Drop strategy', () => {
    test('should drop events when buffer is full', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Drop,
        bufferSize: 2,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));
      const result = controller.tryPush(createTestEvent(3));

      expect(result).toBe(false);
      const stats = controller.getStats();
      expect(stats.droppedEvents).toBe(1);
    });
  });

  describe('Fail strategy', () => {
    test('should throw BufferFullError when buffer is full', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Fail,
        bufferSize: 2,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));

      const event3 = createTestEvent(3);
      expect(() => controller.tryPush(event3)).toThrow(BufferFullError);
    });

    test('BufferFullError should contain buffer size and event', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Fail,
        bufferSize: 1,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.tryPush(createTestEvent(1));

      try {
        controller.tryPush(createTestEvent(2));
        fail('Should have thrown');
      } catch (error) {
        expect(error).toBeInstanceOf(BufferFullError);
        const bfe = error as BufferFullError;
        expect(bfe.bufferSize).toBe(1);
      }
    });
  });

  describe('Latest strategy', () => {
    test('should keep latest events when buffer is full', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Latest,
        bufferSize: 2,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));
      const result = controller.tryPush(createTestEvent(3));

      expect(result).toBe(true);
      expect(controller.size()).toBe(2);

      const stats = controller.getStats();
      expect(stats.droppedEvents).toBe(1);
    });
  });

  describe('pop', () => {
    test('should return undefined when empty', () => {
      const controller = new BackpressureController();
      expect(controller.pop()).toBeUndefined();
    });

    test('should return first event', () => {
      const controller = new BackpressureController();
      const event1 = createTestEvent(1);
      const event2 = createTestEvent(2);

      controller.tryPush(event1);
      controller.tryPush(event2);

      const popped = controller.pop();
      expect(popped).toBe(event1);
      expect(controller.size()).toBe(1);
    });

    test('should update statistics', () => {
      const controller = new BackpressureController();
      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));

      controller.pop();

      const stats = controller.getStats();
      expect(stats.bufferedEvents).toBe(1);
      expect(stats.currentBufferSize).toBe(1);
    });
  });

  describe('peek', () => {
    test('should return undefined when empty', () => {
      const controller = new BackpressureController();
      expect(controller.peek()).toBeUndefined();
    });

    test('should return first event without removing', () => {
      const controller = new BackpressureController();
      const event = createTestEvent(1);
      controller.tryPush(event);

      const peeked = controller.peek();
      expect(peeked).toBe(event);
      expect(controller.size()).toBe(1);
    });
  });

  describe('clear', () => {
    test('should remove all events', () => {
      const controller = new BackpressureController();
      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));

      const count = controller.clear();

      expect(count).toBe(2);
      expect(controller.isEmpty()).toBe(true);
    });

    test('should update statistics', () => {
      const controller = new BackpressureController();
      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));

      controller.clear();

      const stats = controller.getStats();
      expect(stats.droppedEvents).toBe(2);
      expect(stats.bufferedEvents).toBe(0);
      expect(stats.highWatermarkReached).toBe(false);
    });
  });

  describe('isOverloaded', () => {
    test('should return false when empty', () => {
      const controller = new BackpressureController();
      expect(controller.isOverloaded).toBe(false);
    });

    test('should return true when above high watermark', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      // Fill to 90%
      for (let i = 0; i < 9; i++) {
        controller.tryPush(createTestEvent(i));
      }

      expect(controller.isOverloaded).toBe(true);
    });

    test('should return false when below high watermark', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      for (let i = 0; i < 5; i++) {
        controller.tryPush(createTestEvent(i));
      }

      expect(controller.isOverloaded).toBe(false);
    });
  });

  describe('isRecovered', () => {
    test('should return true when empty', () => {
      const controller = new BackpressureController();
      expect(controller.isRecovered).toBe(true);
    });

    test('should return true when below low watermark', () => {
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      for (let i = 0; i < 3; i++) {
        controller.tryPush(createTestEvent(i));
      }

      expect(controller.isRecovered).toBe(true);
    });
  });

  describe('callbacks', () => {
    test('should call onOverflow callback', () => {
      let overflowCalled = false;
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
        onOverflow: () => {
          overflowCalled = true;
        },
      });

      for (let i = 0; i < 9; i++) {
        controller.tryPush(createTestEvent(i));
      }

      expect(overflowCalled).toBe(true);
    });

    test('should call onResume callback', () => {
      let resumeCalled = false;
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
        onResume: () => {
          resumeCalled = true;
        },
      });

      // Fill to high watermark
      for (let i = 0; i < 9; i++) {
        controller.tryPush(createTestEvent(i));
      }

      // Drain to low watermark
      while (controller.size() > 5) {
        controller.pop();
      }

      expect(resumeCalled).toBe(true);
    });

    test('should set overflow callback', () => {
      let called = false;
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.setOverflowCallback(() => {
        called = true;
      });

      for (let i = 0; i < 9; i++) {
        controller.tryPush(createTestEvent(i));
      }

      expect(called).toBe(true);
    });

    test('should set resume callback', () => {
      let called = false;
      const controller = new BackpressureController({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      controller.setResumeCallback(() => {
        called = true;
      });

      for (let i = 0; i < 9; i++) {
        controller.tryPush(createTestEvent(i));
      }

      while (controller.size() > 5) {
        controller.pop();
      }

      expect(called).toBe(true);
    });
  });

  describe('resetStats', () => {
    test('should reset statistics counters', () => {
      const controller = new BackpressureController();
      controller.tryPush(createTestEvent(1));
      controller.tryPush(createTestEvent(2));

      controller.resetStats();

      const stats = controller.getStats();
      expect(stats.totalEvents).toBe(0);
      expect(stats.droppedEvents).toBe(0);
      expect(stats.bufferedEvents).toBe(2); // Should not reset
    });
  });
});

describe('MultiLevelBackpressure', () => {
  test('should push events with priority', () => {
    const bp = new MultiLevelBackpressure(100);

    bp.push(createTestEvent(1), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(2), MultiLevelBackpressure.NORMAL);
    bp.push(createTestEvent(3), MultiLevelBackpressure.LOW);

    expect(bp.size()).toBe(3);
  });

  test('should pop highest priority first', () => {
    const bp = new MultiLevelBackpressure(100);

    bp.push(createTestEvent(1), MultiLevelBackpressure.LOW);
    bp.push(createTestEvent(2), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(3), MultiLevelBackpressure.NORMAL);

    const first = bp.pop();
    expect(first?.value).toBe(2); // HIGH

    const second = bp.pop();
    expect(second?.value).toBe(3); // NORMAL

    const third = bp.pop();
    expect(third?.value).toBe(1); // LOW
  });

  test('should drop low priority when full', () => {
    const bp = new MultiLevelBackpressure(3);

    bp.push(createTestEvent(1), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(2), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(3), MultiLevelBackpressure.LOW);

    // This should drop the LOW priority event
    bp.push(createTestEvent(4), MultiLevelBackpressure.NORMAL);

    const stats = bp.getStats();
    expect(stats.droppedEvents).toBe(1);
  });

  test('should reject low priority when only high priority exists', () => {
    const bp = new MultiLevelBackpressure(2);

    bp.push(createTestEvent(1), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(2), MultiLevelBackpressure.HIGH);

    const result = bp.push(createTestEvent(3), MultiLevelBackpressure.LOW);
    expect(result).toBe(false);
  });

  test('should always accept high priority', () => {
    const bp = new MultiLevelBackpressure(2);

    bp.push(createTestEvent(1), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(2), MultiLevelBackpressure.HIGH);

    const result = bp.push(createTestEvent(3), MultiLevelBackpressure.HIGH);
    expect(result).toBe(true);
  });

  test('should return statistics', () => {
    const bp = new MultiLevelBackpressure(100);

    bp.push(createTestEvent(1), MultiLevelBackpressure.HIGH);
    bp.push(createTestEvent(2), MultiLevelBackpressure.NORMAL);

    const stats = bp.getStats();
    expect(stats.totalEvents).toBe(2);
    expect(stats.currentBufferSize).toBe(2);
  });
});

describe('RateBasedBackpressure', () => {
  test('should allow requests below rate limit', async () => {
    const bp = new RateBasedBackpressure(100, 1.0, 0.1);

    for (let i = 0; i < 5; i++) {
      const result = await bp.tryAcquire();
      expect(result).toBe(true);
    }
  });

  test('should block when rate exceeded', async () => {
    const bp = new RateBasedBackpressure(2, 0.1, 0.5);

    // Exhaust the rate limit
    await bp.tryAcquire();
    await bp.tryAcquire();

    // Should be blocked
    const result = await bp.tryAcquire();
    expect(result).toBe(false);
  });

  test('should report current rate', async () => {
    const bp = new RateBasedBackpressure(100, 1.0, 0.1);

    await bp.tryAcquire();
    await bp.tryAcquire();

    const rate = bp.currentRate;
    expect(rate).toBeGreaterThan(0);
  });

  test('should reset properly', async () => {
    const bp = new RateBasedBackpressure(2, 0.1, 0.5);

    await bp.tryAcquire();
    await bp.tryAcquire();
    await bp.tryAcquire(); // blocked

    bp.reset();

    const result = await bp.tryAcquire();
    expect(result).toBe(true);
  });

  test('should report backpressure state', async () => {
    const bp = new RateBasedBackpressure(1, 0.1, 0.5);

    expect(bp.isBackpressureActive).toBe(false);

    await bp.tryAcquire();
    await bp.tryAcquire(); // Exceeds limit

    expect(bp.isBackpressureActive).toBe(true);
  });
});

describe('Error classes', () => {
  test('BackpressureError should have correct name', () => {
    const error = new BackpressureError('test error');
    expect(error.name).toBe('BackpressureError');
    expect(error.message).toBe('test error');
  });

  test('BufferFullError should have correct properties', () => {
    const event = createTestEvent(1);
    const error = new BufferFullError(100, event);

    expect(error.name).toBe('BufferFullError');
    expect(error.bufferSize).toBe(100);
    expect(error.event).toBe(event);
    expect(error.message).toContain('100');
  });
});
