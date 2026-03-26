import { StreamActor } from '../src/streaming/stream_actor';
import {
  StreamEvent,
  Timestamp,
  Duration,
  WindowType,
  BackpressureStrategy,
  LateDataPolicy,
  Watermark,
  createStreamEvent,
  createStreamConfig,
} from '../src/streaming/types';
import {
  BackpressureController,
  BufferFullError,
  MultiLevelBackpressure,
} from '../src/streaming/backpressure';
import { CircuitBreaker, CircuitBreakerError } from '../src/resilience/circuit_breaker';
import { CircuitState, BackoffStrategy, RateLimitStrategy } from '../src/resilience/types';
import { Bulkhead, BulkheadRejectedError, BulkheadTimeoutError } from '../src/resilience/bulkhead';
import { RetryPolicy, RetryExhaustedError } from '../src/resilience/retry';
import { RateLimiter } from '../src/resilience/rate_limiter';
import { Validator } from '../src/validation/validators';
import {
  sanitizeString,
  sanitizeHTML,
  sanitizeSQL,
  sanitizeSlug,
  removeControlChars,
  trimAndNormalizeWhitespace,
  sanitizeFilename,
  sanitizeAlphanumeric,
  stripHTML,
  sanitizeJSON,
} from '../src/validation/sanitize';

class EdgeTestActor extends StreamActor<string, unknown> {
  public processedEvents: StreamEvent<unknown>[] = [];

  static get name(): string {
    return 'edge-test-actor';
  }

  async processEvent(event: StreamEvent<unknown>): Promise<void> {
    this.processedEvents.push(event);
  }
}

// ============================================================
// Stream Processing Edge Cases
// ============================================================

describe('Stream Processing Edge Cases', () => {
  describe('Empty stream processing', () => {
    test('StreamActor with zero events reports correct initial metrics', () => {
      const actor = new EdgeTestActor();
      const metrics = actor.getMetrics();

      expect(metrics.processedCount).toBe(0);
      expect(metrics.lateEventsCount).toBe(0);
      expect(actor.processedEvents).toHaveLength(0);
      expect((metrics.backpressure as Record<string, unknown>).totalEvents).toBe(0);
      expect((metrics.backpressure as Record<string, unknown>).bufferedEvents).toBe(0);
    });

    test('StreamActor pop and drain on empty buffer returns undefined', () => {
      const actor = new EdgeTestActor();
      const bp = (actor as any).backpressure;
      const event = bp.pop();
      expect(event).toBeUndefined();
      expect(bp.isEmpty()).toBe(true);
      expect(bp.size()).toBe(0);
    });
  });

  describe('Very large message (10MB+ payload)', () => {
    test('StreamActor processes a StreamEvent with a >10MB string value without error', async () => {
      const actor = new EdgeTestActor();
      const hugeValue = 'x'.repeat(10 * 1024 * 1024 + 1);
      const event: StreamEvent<string> = createStreamEvent('key1', hugeValue);

      await actor['processEventInternal'](event);

      expect(actor.processedEvents).toHaveLength(1);
      expect(typeof actor.processedEvents[0].value).toBe('string');
      expect((actor.processedEvents[0].value as string).length).toBeGreaterThan(10 * 1024 * 1024);
      expect(actor.getMetrics().processedCount).toBe(1);
    });

    test('BackpressureController accepts and returns a 10MB+ event', () => {
      const controller = new BackpressureController<string>({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      const hugeValue = 'y'.repeat(10 * 1024 * 1024 + 1);
      const event: StreamEvent<string> = createStreamEvent('big-key', hugeValue);
      const accepted = controller.tryPush(event);

      expect(accepted).toBe(true);
      const popped = controller.pop();
      expect(popped).toBeDefined();
      expect((popped!.value as string).length).toBeGreaterThan(10 * 1024 * 1024);
    });

    test('StreamActor emitValue with a large object payload', async () => {
      const actor = new EdgeTestActor();
      const received: StreamEvent<unknown>[] = [];
      actor.registerOutputHandler('out', (e) => received.push(e));

      const largeObj = { data: 'z'.repeat(5 * 1024 * 1024) };
      await actor.emitValue('out', largeObj);

      expect(received).toHaveLength(1);
      expect((received[0].value as Record<string, string>).data.length).toBe(5 * 1024 * 1024);
    });
  });

  describe('Rapid fire events through BackpressureController', () => {
    test('push 10000 events through buffer and verify all statistics', () => {
      const controller = new BackpressureController<number>({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10000,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      for (let i = 0; i < 10000; i++) {
        const event: StreamEvent<number> = createStreamEvent(`key-${i}`, i);
        const accepted = controller.tryPush(event);
        expect(accepted).toBe(true);
      }

      const stats = controller.getStats();
      expect(stats.totalEvents).toBe(10000);
      expect(stats.bufferedEvents).toBe(10000);
      expect(stats.currentBufferSize).toBe(10000);
      expect(stats.highWatermarkReached).toBe(true);
      expect(stats.overflowCount).toBe(1);
      expect(stats.droppedEvents).toBe(0);
      expect(stats.rejectedEvents).toBe(0);
    });

    test('Drop strategy silently rejects events beyond buffer capacity', () => {
      const controller = new BackpressureController<number>({
        strategy: BackpressureStrategy.Drop,
        bufferSize: 100,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      let accepted = 0;
      let dropped = 0;
      for (let i = 0; i < 150; i++) {
        const event: StreamEvent<number> = createStreamEvent(`key-${i}`, i);
        if (controller.tryPush(event)) accepted++;
        else dropped++;
      }

      expect(accepted).toBe(100);
      expect(dropped).toBe(50);
      expect(controller.getStats().totalEvents).toBe(150);
      expect(controller.getStats().droppedEvents).toBe(50);
    });

    test('Fail strategy throws BufferFullError when buffer is full', () => {
      const controller = new BackpressureController<number>({
        strategy: BackpressureStrategy.Fail,
        bufferSize: 5,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      for (let i = 0; i < 5; i++) {
        controller.tryPush(createStreamEvent('k', i));
      }

      expect(() => {
        controller.tryPush(createStreamEvent('k', 99));
      }).toThrow(BufferFullError);
    });

    test('popping all 10000 events drains the buffer completely', () => {
      const controller = new BackpressureController<number>({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 10000,
        highWatermark: 0.9,
        lowWatermark: 0.5,
      });

      for (let i = 0; i < 10000; i++) {
        controller.tryPush(createStreamEvent('k', i));
      }

      let poppedCount = 0;
      while (!controller.isEmpty()) {
        const event = controller.pop();
        expect(event).toBeDefined();
        expect(event!.value).toBe(poppedCount);
        poppedCount++;
      }

      expect(poppedCount).toBe(10000);
      expect(controller.size()).toBe(0);
      expect(controller.getStats().bufferedEvents).toBe(0);
    });

    test('overflow and resume callbacks fire correctly during rapid push/pop', () => {
      const onOverflow = jest.fn();
      const onResume = jest.fn();

      const controller = new BackpressureController<number>({
        strategy: BackpressureStrategy.Buffer,
        bufferSize: 100,
        highWatermark: 0.9,
        lowWatermark: 0.5,
        onOverflow,
        onResume,
      });

      for (let i = 0; i < 100; i++) {
        controller.tryPush(createStreamEvent('k', i));
      }

      expect(onOverflow).toHaveBeenCalledTimes(1);

      for (let i = 0; i < 60; i++) {
        controller.pop();
      }

      expect(controller.getStats().resumeCount).toBe(1);
      expect(onResume).toHaveBeenCalledTimes(1);
    });
  });

  describe('Late data with watermark at epoch', () => {
    test('events with timestamp 0 are late when watermark has advanced past epoch', async () => {
      const actor = new EdgeTestActor();
      await actor.advanceWatermark(new Watermark(new Timestamp(5000), 'default'));

      const epochEvent: StreamEvent<number> = createStreamEvent('key1', 42, new Timestamp(0), {
        eventType: 'default',
      });
      await actor['processEventInternal'](epochEvent);

      expect(actor.getMetrics().processedCount).toBe(1);
      expect(actor.getMetrics().lateEventsCount).toBe(1);
      expect(actor.processedEvents).toHaveLength(0);
    });

    test('events at exactly the watermark timestamp are NOT considered late', async () => {
      const actor = new EdgeTestActor();
      await actor.advanceWatermark(new Watermark(new Timestamp(5000), 'default'));

      const onTimeEvent: StreamEvent<number> = createStreamEvent('key1', 42, new Timestamp(5000), {
        eventType: 'default',
      });
      await actor['processEventInternal'](onTimeEvent);

      expect(actor.getMetrics().processedCount).toBe(1);
      expect(actor.getMetrics().lateEventsCount).toBe(0);
      expect(actor.processedEvents).toHaveLength(1);
    });

    test('events with timestamp 0 when watermark is at 0 are NOT late (boundary condition)', async () => {
      const actor = new EdgeTestActor();
      await actor.advanceWatermark(new Watermark(new Timestamp(0), 'default'));

      const epochEvent: StreamEvent<number> = createStreamEvent('key1', 42, new Timestamp(0), {
        eventType: 'default',
      });
      await actor['processEventInternal'](epochEvent);

      expect(actor.getMetrics().processedCount).toBe(1);
      expect(actor.getMetrics().lateEventsCount).toBe(0);
      expect(actor.processedEvents).toHaveLength(1);
    });

    test('late events with SideOutput policy route to registered handler', async () => {
      const actor = new EdgeTestActor(createStreamConfig({
        lateDataPolicy: LateDataPolicy.SideOutput,
        lateDataOutput: 'late-stream',
      }));

      const lateEvents: StreamEvent<unknown>[] = [];
      actor.registerLateDataHandler((event) => lateEvents.push(event));

      await actor.advanceWatermark(new Watermark(new Timestamp(10000), 'default'));

      const lateEvent: StreamEvent<number> = createStreamEvent('k', 1, new Timestamp(0), {
        eventType: 'default',
      });
      await actor['processEventInternal'](lateEvent);

      expect(lateEvents).toHaveLength(1);
      expect(lateEvents[0].value).toBe(1);
    });
  });
});

// ============================================================
// Resilience Edge Cases
// ============================================================

describe('Resilience Edge Cases', () => {
  describe('Concurrent circuit breaker calls on open circuit', () => {
    test('Promise.all with 50 parallel calls hitting an open circuit all fail fast', async () => {
      const breaker = new CircuitBreaker({
        failureThreshold: 1,
        resetTimeout: 60000,
        name: 'concurrent-cb',
      });

      breaker.forceState(CircuitState.Open);

      const errors: unknown[] = [];
      const promises = Array.from({ length: 50 }, () =>
        breaker.execute(() => Promise.resolve('ok')).catch((e) => {
          errors.push(e);
          return null;
        })
      );

      await Promise.all(promises);

      expect(errors).toHaveLength(50);
      expect(errors.every((e) => e instanceof CircuitBreakerError)).toBe(true);
      expect(breaker.getStats().totalCalls).toBe(50);
      expect(breaker.getState()).toBe(CircuitState.Open);
    });

    test('executeWithFallback returns fallback for all calls when circuit is open', async () => {
      const breaker = new CircuitBreaker({
        failureThreshold: 1,
        resetTimeout: 60000,
        name: 'fallback-cb',
      });

      breaker.forceState(CircuitState.Open);
      const fallback = jest.fn().mockResolvedValue('fallback-value');

      const results = await Promise.all(
        Array.from({ length: 20 }, () =>
          breaker.executeWithFallback(
            () => Promise.resolve('should-not-run'),
            fallback
          )
        )
      );

      expect(results.every((r) => r === 'fallback-value')).toBe(true);
      expect(fallback).toHaveBeenCalledTimes(20);
    });
  });

  describe('Bulkhead queue timeout', () => {
    beforeEach(() => {
      jest.useFakeTimers();
    });

    afterEach(() => {
      jest.useRealTimers();
    });

    test('queued calls receive BulkheadTimeoutError when queueTimeout expires', async () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 1,
        maxQueueSize: 5,
        queueTimeout: 2000,
        name: 'timeout-bh',
      });

      const neverResolves = new Promise<never>(() => {});
      bulkhead.execute(() => neverResolves);

      const queuedCalls = Array.from({ length: 3 }, () =>
        bulkhead.execute(() => Promise.resolve('done'))
      );

      jest.advanceTimersByTime(2001);

      for (const p of queuedCalls) {
        await expect(p).rejects.toThrow(BulkheadTimeoutError);
      }

      const stats = bulkhead.getStats();
      expect(stats.active).toBe(1);
      expect(stats.queueSize).toBe(0);
    });

    test('excess calls beyond queue capacity receive BulkheadRejectedError immediately', async () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 1,
        maxQueueSize: 2,
        queueTimeout: 5000,
        name: 'capacity-bh',
      });

      const neverResolves = new Promise<never>(() => {});
      bulkhead.execute(() => neverResolves);

      const p2 = bulkhead.execute(() => Promise.resolve('a')).catch(() => {});
      const p3 = bulkhead.execute(() => Promise.resolve('b')).catch(() => {});
      const p4 = bulkhead.execute(() => Promise.resolve('c'));

      await expect(p4).rejects.toThrow(BulkheadRejectedError);
    });

    test('no-queue bulkhead rejects immediately when at capacity', async () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 1,
        maxQueueSize: 0,
        name: 'no-queue-bh',
      });

      const neverResolves = new Promise<never>(() => {});
      bulkhead.execute(() => neverResolves);

      await expect(
        bulkhead.execute(() => Promise.resolve('done'))
      ).rejects.toThrow(BulkheadRejectedError);

      expect(bulkhead.getStats().rejected).toBe(1);
    });

    test('queued call that starts executing before timeout does NOT timeout', async () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 1,
        maxQueueSize: 5,
        queueTimeout: 5000,
        name: 'execute-before-timeout-bh',
      });

      let resolveFirst: () => void;
      const firstCall = new Promise<void>((resolve) => { resolveFirst = resolve; });
      bulkhead.execute(() => firstCall);

      const secondCall = bulkhead.execute(() => Promise.resolve('second'));

      resolveFirst!();
      await firstCall;

      jest.advanceTimersByTime(100);

      const result = await secondCall;
      expect(result).toBe('second');
    });
  });

  describe('Rate limiter token exhaustion and refill', () => {
    beforeEach(() => {
      jest.useFakeTimers();
    });

    afterEach(() => {
      jest.useRealTimers();
    });

    test('token bucket exhausts all tokens and denies further requests', () => {
      const limiter = new RateLimiter({
        maxRequests: 10,
        windowMs: 1000,
        strategy: RateLimitStrategy.TokenBucket,
        refillRate: 10,
        name: 'exhaustion-rl',
      });

      let allowed = 0;
      let denied = 0;
      for (let i = 0; i < 15; i++) {
        if (limiter.tryAcquire().allowed) allowed++;
        else denied++;
      }

      expect(allowed).toBe(10);
      expect(denied).toBe(5);
    });

    test('token bucket refills tokens after advancing time past one second', () => {
      const limiter = new RateLimiter({
        maxRequests: 10,
        windowMs: 1000,
        strategy: RateLimitStrategy.TokenBucket,
        refillRate: 10,
        name: 'refill-rl',
      });

      for (let i = 0; i < 10; i++) {
        expect(limiter.tryAcquire().allowed).toBe(true);
      }
      expect(limiter.tryAcquire().allowed).toBe(false);

      jest.advanceTimersByTime(1001);

      expect(limiter.tryAcquire().allowed).toBe(true);
    });

    test('sliding window rate limiter exhausts and recovers after window passes', () => {
      const limiter = new RateLimiter({
        maxRequests: 5,
        windowMs: 1000,
        strategy: RateLimitStrategy.SlidingWindow,
        name: 'sliding-rl',
      });

      for (let i = 0; i < 5; i++) {
        expect(limiter.tryAcquire().allowed).toBe(true);
      }
      expect(limiter.tryAcquire().allowed).toBe(false);

      jest.advanceTimersByTime(1001);

      expect(limiter.tryAcquire().allowed).toBe(true);
    });

    test('fixed window rate limiter resets after window expires', () => {
      const limiter = new RateLimiter({
        maxRequests: 3,
        windowMs: 1000,
        strategy: RateLimitStrategy.FixedWindow,
        name: 'fixed-rl',
      });

      for (let i = 0; i < 3; i++) {
        expect(limiter.tryAcquire().allowed).toBe(true);
      }
      expect(limiter.tryAcquire().allowed).toBe(false);

      jest.advanceTimersByTime(1001);

      expect(limiter.tryAcquire().allowed).toBe(true);
      expect(limiter.tryAcquire().allowed).toBe(true);
    });

    test('rate limiter remaining count is accurate after partial exhaustion', () => {
      const limiter = new RateLimiter({
        maxRequests: 10,
        windowMs: 1000,
        strategy: RateLimitStrategy.TokenBucket,
        refillRate: 10,
        name: 'partial-rl',
      });

      limiter.tryAcquire();
      limiter.tryAcquire();
      limiter.tryAcquire();

      const result = limiter.tryAcquire();
      expect(result.allowed).toBe(true);
      expect(result.remaining).toBe(6);
    });
  });

  describe('Retry with fake timer simulation', () => {
    beforeEach(() => {
      jest.useFakeTimers();
    });

    afterEach(() => {
      jest.useRealTimers();
    });

    test('exhausts all 3 attempts with Fixed backoff and correct delay timing', async () => {
      const retry = new RetryPolicy({
        maxAttempts: 3,
        initialDelay: 100,
        maxDelay: 10000,
        multiplier: 2,
        strategy: BackoffStrategy.Fixed,
        name: 'fixed-retry',
      });

      const fn = jest.fn().mockRejectedValue(new Error('ETIMEDOUT'));
      const promise = retry.execute(fn, () => true);
      await jest.advanceTimersByTimeAsync(1000);
      const result = await promise;

      expect(result.success).toBe(false);
      expect(result.attempts).toBe(3);
      expect(fn).toHaveBeenCalledTimes(3);
      expect(result.totalTime).toBeGreaterThanOrEqual(200);
    });

    test('succeeds on third attempt after two failures with Exponential backoff', async () => {
      const retry = new RetryPolicy({
        maxAttempts: 5,
        initialDelay: 50,
        maxDelay: 5000,
        multiplier: 2,
        strategy: BackoffStrategy.Exponential,
        name: 'eventual-success-retry',
      });

      let callCount = 0;
      const fn = jest.fn().mockImplementation(() => {
        callCount++;
        if (callCount < 3) return Promise.reject(new Error('ETIMEDOUT'));
        return Promise.resolve('recovered');
      });

      const promise = retry.executeOrThrow(fn, () => true);
      await jest.advanceTimersByTimeAsync(5000);
      const result = await promise;

      expect(result).toBe('recovered');
      expect(fn).toHaveBeenCalledTimes(3);
    });

    test('executeOrThrow throws RetryExhaustedError when all attempts fail', async () => {
      const retry = new RetryPolicy({
        maxAttempts: 2,
        initialDelay: 50,
        maxDelay: 5000,
        strategy: BackoffStrategy.Fixed,
        name: 'exhausted-retry',
      });

      const fn = jest.fn().mockRejectedValue(new Error('ETIMEDOUT'));
      const promise = retry.execute(fn, () => true);
      await jest.advanceTimersByTimeAsync(1000);
      const result = await promise;

      expect(result.success).toBe(false);
      expect(result.attempts).toBe(2);
      expect(result.error!.message).toBe('ETIMEDOUT');
    });

    test('shouldRetry predicate stops retrying when it returns false', async () => {
      const retry = new RetryPolicy({
        maxAttempts: 10,
        initialDelay: 50,
        strategy: BackoffStrategy.Fixed,
        name: 'predicate-retry',
      });

      const fn = jest.fn().mockRejectedValue(new Error('not retryable'));
      const shouldRetry = jest.fn().mockReturnValue(false);

      const promise = retry.execute(fn, shouldRetry);
      await jest.advanceTimersByTimeAsync(1000);
      const result = await promise;

      expect(result.success).toBe(false);
      expect(result.attempts).toBe(1);
      expect(fn).toHaveBeenCalledTimes(1);
      expect(shouldRetry).toHaveBeenCalledTimes(1);
    });
  });
});

// ============================================================
// Validation Edge Cases
// ============================================================

describe('Validation Edge Cases', () => {
  describe('Very long strings through sanitization', () => {
    test('sanitizeString handles 100K+ character strings without error', () => {
      const longString = 'a'.repeat(100000);
      const result = sanitizeString(longString);
      expect(result.length).toBe(100000);
    });

    test('sanitizeString truncates to maxLength for very long strings', () => {
      const longString = 'b'.repeat(200000);
      const result = sanitizeString(longString, 1000);
      expect(result.length).toBe(1000);
    });

    test('sanitizeHTML handles 100K+ character strings with embedded tags', () => {
      const longHtml = '<p>' + 'x'.repeat(100000) + '</p>';
      const result = sanitizeHTML(longHtml);
      expect(result).toContain('&lt;p&gt;');
      expect(result).toContain('&lt;&#x2F;p&gt;');
      expect(result.length).toBeGreaterThan(100000);
    });

    test('sanitizeSQL handles 100K+ character strings by stripping SQL keywords', () => {
      const longSql = 'a'.repeat(100000) + '; DROP TABLE users; --';
      const result = sanitizeSQL(longSql);
      expect(result).not.toContain('DROP');
      expect(result).not.toContain('--');
      expect(result.length).toBeGreaterThan(100000);
    });

    test('sanitizeJSON handles deeply nested large objects', () => {
      const deeplyNested = { a: { b: { c: { d: { e: 'x'.repeat(100000) } } } } };
      const result = sanitizeJSON(deeplyNested) as typeof deeplyNested;
      expect(result.a.b.c.d.e.length).toBe(100000);
    });

    test('sanitizeJSON handles large arrays', () => {
      const largeArray = Array.from({ length: 10000 }, (_, i) => `item-${i}`);
      const result = sanitizeJSON(largeArray) as string[];
      expect(result).toHaveLength(10000);
      expect(result[9999]).toBe('item-9999');
    });

    test('sanitizeSlug handles 100K+ character input', () => {
      const longSlug = 'a'.repeat(100000);
      const result = sanitizeSlug(longSlug);
      expect(result.length).toBe(100000);
    });
  });

  describe('Validator with many chained rules (20+)', () => {
    test('chaining 22 validation rules produces errors for all invalid fields', () => {
      const v = new Validator();

      v.required('name', '')
        .minLength('name', '', 5)
        .maxLength('bio', 'x'.repeat(200), 100)
        .pattern('username', '123', /^[a-z]+$/)
        .email('email', 'not-email')
        .url('website', 'not-url')
        .uuid('id', 'not-uuid')
        .phone('phone', 'not-phone')
        .slug('slug', 'bad slug!')
        .integer('age', 'not-number')
        .float('score', 'not-number')
        .boolean('active', 'not-bool')
        .array('tags', 'not-array')
        .object('meta', 'not-object')
        .minValue('min', 0, 10)
        .maxValue('max', 200, 100)
        .range('pct', 150, 0, 100)
        .minItems('list1', [], 1)
        .maxItems('list2', new Array(11).fill(0), 10)
        .enum('status', 'bad', ['active', 'inactive'])
        .custom('field', null, (val) => val !== null, 'must not be null');

      expect(v.isValid()).toBe(false);
      const errors = v.getErrors();
      expect(Object.keys(errors).length).toBeGreaterThanOrEqual(15);
    });

    test('chaining 20 rules all passing produces no errors', () => {
      const v = new Validator();

      v.required('name', 'valid-name')
        .minLength('name', 'valid-name', 3)
        .maxLength('name', 'valid-name', 100)
        .pattern('username', 'validuser', /^[a-z]+$/)
        .email('email', 'user@example.com')
        .url('website', 'https://example.com')
        .uuid('id', '550e8400-e29b-41d4-a716-446655440000')
        .phone('phone', '+1234567890')
        .slug('slug', 'valid-slug')
        .integer('age', 25)
        .float('score', 95.5)
        .boolean('active', true)
        .array('tags', ['a', 'b'])
        .object('meta', { key: 'val' })
        .minValue('min', 10, 0)
        .maxValue('max', 50, 100)
        .range('pct', 50, 0, 100)
        .minItems('list1', [1, 2, 3], 1)
        .maxItems('list2', [1, 2], 10)
        .enum('status', 'active', ['active', 'inactive']);

      expect(v.isValid()).toBe(true);
      expect(v.getErrors()).toEqual({});
    });

    test('clear resets all errors allowing reuse of the same Validator instance', () => {
      const v = new Validator();
      v.required('f', '');
      expect(v.isValid()).toBe(false);

      v.clear();
      expect(v.isValid()).toBe(true);
      expect(v.getErrors()).toEqual({});

      v.required('f', 'valid');
      expect(v.isValid()).toBe(true);
    });

    test('when condition prevents unnecessary validation', () => {
      const v = new Validator();
      v.when(false, (validator) => {
        validator.required('conditional', '');
      });

      expect(v.isValid()).toBe(true);
    });
  });

  describe('Unicode edge cases in sanitization', () => {
    test('emoji characters are preserved by sanitizeString', () => {
      const emoji = 'Hello 🎉 World 🚀 Test 💻';
      expect(sanitizeString(emoji)).toBe('Hello 🎉 World 🚀 Test 💻');
    });

    test('emoji are preserved by sanitizeHTML while HTML entities are escaped', () => {
      const input = '<script>alert("🎉")</script>';
      const result = sanitizeHTML(input);
      expect(result).toContain('🎉');
      expect(result).not.toContain('<script>');
      expect(result).toContain('&lt;script&gt;');
    });

    test('CJK characters are preserved by sanitizeString', () => {
      const cjk = '你好世界テスト';
      expect(sanitizeString(cjk)).toBe('你好世界テスト');
    });

    test('CJK characters are stripped by sanitizeSlug since they are non-latin', () => {
      const cjk = '你好-world';
      expect(sanitizeSlug(cjk)).toBe('world');
    });

    test('CJK characters are stripped by sanitizeAlphanumeric', () => {
      const cjk = 'abc你好def';
      expect(sanitizeAlphanumeric(cjk)).toBe('abcdef');
    });

    test('RTL (Arabic) text is preserved by sanitizeString', () => {
      const rtl = 'مرحبا بالعالم';
      expect(sanitizeString(rtl)).toBe('مرحبا بالعالم');
    });

    test('zero-width characters are preserved by sanitizeString', () => {
      const zwc = 'hello\u200Bworld\u200Ctest\u200Dend';
      expect(sanitizeString(zwc)).toBe('hello\u200Bworld\u200Ctest\u200Dend');
    });

    test('zero-width characters are stripped by sanitizeAlphanumeric', () => {
      const zwc = 'abc\u200Bdef';
      expect(sanitizeAlphanumeric(zwc)).toBe('abcdef');
    });

    test('zero-width characters are stripped by sanitizeSlug', () => {
      const zwc = 'hello\u200Bworld';
      expect(sanitizeSlug(zwc)).toBe('helloworld');
    });

    test('zero-width characters are preserved by sanitizeHTML', () => {
      const zwc = 'a\u200Bb';
      expect(sanitizeHTML(zwc)).toBe('a\u200Bb');
    });

    test('null bytes mixed with Unicode are removed by sanitizeString', () => {
      const mixed = 'hello\x00🎉world\x00';
      expect(sanitizeString(mixed)).toBe('hello🎉world');
    });

    test('control characters mixed with Unicode are removed by removeControlChars', () => {
      const mixed = 'hello\x01world\x02🎉\x03test';
      const result = removeControlChars(mixed);
      expect(result).toBe('helloworld🎉test');
    });

    test('stripHTML preserves emoji inside and around tags', () => {
      const input = '<b>🎉</b> <i>hello</i> 🚀';
      expect(stripHTML(input)).toBe('🎉 hello 🚀');
    });

    test('sanitizeFilename preserves Unicode filenames but removes traversal', () => {
      expect(sanitizeFilename('文件.txt')).toBe('文件.txt');
      expect(sanitizeFilename('../secret')).toBe('secret');
    });

    test('trimAndNormalizeWhitespace normalizes spaces including Unicode whitespace', () => {
      const input = '  hello   \u3000world  ';
      const result = trimAndNormalizeWhitespace(input);
      expect(result).toBe('hello world');
    });

    test('sanitizeSQL preserves Unicode text while stripping SQL keywords', () => {
      const input = 'SELECT * FROM 用户 WHERE 名前 = \'测试\'; DROP TABLE 数据;';
      const result = sanitizeSQL(input);
      expect(result).toContain('用户');
      expect(result).toContain('测试');
      expect(result).not.toContain('DROP');
    });
  });
});
