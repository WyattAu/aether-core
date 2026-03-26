// @jest/tag:chaos

import {
  CircuitBreaker,
  CircuitBreakerError,
} from '../../src/resilience/circuit_breaker';
import {
  CircuitState,
  BackoffStrategy,
  RateLimitStrategy,
} from '../../src/resilience/types';
import { Bulkhead, BulkheadRejectedError } from '../../src/resilience/bulkhead';
import { RateLimiter } from '../../src/resilience/rate_limiter';
import { RetryPolicy } from '../../src/resilience/retry';
import {
  BackpressureController,
} from '../../src/streaming/backpressure';
import {
  BackpressureStrategy,
  WindowType,
  Timestamp,
  Duration,
  createWindowSpec,
  createStreamEvent,
} from '../../src/streaming/types';
import type { StreamEvent } from '../../src/streaming/types';
import { WindowAssigner } from '../../src/streaming/window';

describe('Chaos Tests', () => {
  beforeAll(() => {
    jest.setTimeout(60_000);
  });

  // ------------------------------------------------------------------ //
  //  1. Circuit Breaker Flapping                                        //
  // ------------------------------------------------------------------ //

  test('Circuit Breaker Flapping - rapid state transitions', async () => {
    const breaker = new CircuitBreaker({
      failureThreshold: 5,
      successThreshold: 3,
      resetTimeout: 100,
      failureWindow: 120_000,
    });

    let opens = 0;
    let closes = 0;
    let halfOpenEntered = 0;
    const totalCalls = 1000;
    const cycleLength = 8;
    const cycles = totalCalls / cycleLength;

    const start = Date.now();
    let callCount = 0;

    for (let cycle = 0; cycle < cycles; cycle++) {
      for (let i = 0; i < 5; i++) {
        try {
          await breaker.execute(() => Promise.reject(new Error('fail')));
        } catch {
          // expected
        }
        callCount++;
      }

      if (breaker.getState() === CircuitState.Open) {
        opens++;
      }

      await new Promise((r) => setTimeout(r, 110));

      const stateBeforeSuccess = breaker.getState();
      if (stateBeforeSuccess === CircuitState.HalfOpen) {
        halfOpenEntered++;
      }

      for (let i = 0; i < 3; i++) {
        await breaker.execute(() => Promise.resolve('ok'));
        callCount++;
      }

      if (breaker.getState() === CircuitState.Closed) {
        closes++;
      }
    }

    const elapsed = Date.now() - start;
    console.log(`[chaos] Circuit Breaker Flapping: ${callCount} calls in ${elapsed}ms`);
    console.log(`  Opens: ${opens}, Closes: ${closes}, Half-opens: ${halfOpenEntered}`);

    expect(callCount).toBe(totalCalls);
    expect(opens).toBe(cycles);
    expect(closes).toBe(cycles);
    expect(halfOpenEntered).toBe(cycles);
    expect(breaker.getState()).toBe(CircuitState.Closed);

    const stats = breaker.getStats();
    expect(stats.state).toBe(CircuitState.Closed);
  }, 60_000);

  // ------------------------------------------------------------------ //
  //  2. Bulkhead Exhaustion + Recovery                                  //
  // ------------------------------------------------------------------ //

  test('Bulkhead Exhaustion + Recovery', async () => {
    const bulkhead = new Bulkhead({
      maxConcurrent: 10,
      maxQueueSize: 5,
      queueTimeout: 0,
      name: 'chaos-bulkhead',
    });

    const operationDuration = 100;
    let successCount = 0;
    let rejectCount = 0;

    const operations = Array.from({ length: 20 }, (_, i) =>
      bulkhead
        .execute(
          () =>
            new Promise<string>((resolve) =>
              setTimeout(() => resolve(`op-${i}`), operationDuration)
            )
        )
        .then(() => {
          successCount++;
        })
        .catch((e) => {
          if (e instanceof BulkheadRejectedError) {
            rejectCount++;
          }
        })
    );

    await Promise.all(operations);

    const elapsed = performance.now();
    console.log(`[chaos] Bulkhead Exhaustion: 20 ops, ${successCount} succeeded, ${rejectCount} rejected`);
    console.log(`  ${rejectCount} rejections with BulkheadRejectedError`);

    expect(rejectCount).toBe(5);
    expect(successCount).toBe(15);

    const stats = bulkhead.getStats();
    console.log(`  Final stats: active=${stats.active}, queueSize=${stats.queueSize}, rejected=${stats.rejected}, accepted=${stats.accepted}`);

    expect(stats.active).toBe(0);
    expect(stats.queueSize).toBe(0);
    expect(stats.rejected).toBe(5);
    expect(stats.accepted).toBe(15);
    expect(stats.available).toBe(10);
  });

  // ------------------------------------------------------------------ //
  //  3. Rate Limiter Burst                                              //
  // ------------------------------------------------------------------ //

  test('Rate Limiter Burst - token bucket depletion', () => {
    const limiter = new RateLimiter({
      maxRequests: 10,
      refillRate: 10,
      strategy: RateLimitStrategy.TokenBucket,
      name: 'chaos-limiter',
    });

    const start = Date.now();
    let allowed = 0;
    let denied = 0;

    for (let i = 0; i < 100; i++) {
      const result = limiter.tryAcquire();
      if (result.allowed) {
        allowed++;
      } else {
        denied++;
      }
    }

    const elapsed = Date.now() - start;
    console.log(`[chaos] Rate Limiter Burst: ${allowed} allowed, ${denied} denied in ${elapsed}ms`);

    expect(allowed).toBe(10);
    expect(denied).toBe(90);

    const lastResult = limiter.tryAcquire();
    expect(lastResult.allowed).toBe(false);
    expect(lastResult.remaining).toBe(0);
  });

  // ------------------------------------------------------------------ //
  //  4. Backpressure Overflow Recovery                                  //
  // ------------------------------------------------------------------ //

  test('Backpressure Overflow Recovery - small buffer, drop strategy', () => {
    const controller = new BackpressureController<number>({
      strategy: BackpressureStrategy.Drop,
      bufferSize: 50,
      highWatermark: 0.9,
      lowWatermark: 0.5,
    });

    let maxObservedSize = 0;

    const start = Date.now();
    for (let i = 0; i < 200; i++) {
      const event = createStreamEvent('key', i, Timestamp.now());
      controller.tryPush(event);
      const size = controller.size();
      if (size > maxObservedSize) {
        maxObservedSize = size;
      }
    }

    const elapsed = Date.now() - start;
    const stats = controller.getStats();

    console.log(`[chaos] Backpressure Overflow: pushed 200 events in ${elapsed}ms`);
    console.log(`  Buffer size: ${controller.size()}, max observed: ${maxObservedSize}`);
    console.log(`  totalEvents=${stats.totalEvents}, dropped=${stats.droppedEvents}, buffered=${stats.bufferedEvents}`);

    expect(maxObservedSize).toBeLessThanOrEqual(50);
    expect(controller.size()).toBe(50);
    expect(stats.totalEvents).toBe(200);
    expect(stats.droppedEvents).toBe(150);

    let poppedCount = 0;
    while (!controller.isEmpty()) {
      const event = controller.pop();
      if (event) poppedCount++;
    }

    const afterStats = controller.getStats();
    console.log(`  Popped: ${poppedCount}, remaining: ${controller.size()}`);
    console.log(`  After drain: totalEvents=${afterStats.totalEvents}, dropped=${afterStats.droppedEvents}, buffered=${afterStats.bufferedEvents}`);

    expect(poppedCount).toBe(50);
    expect(controller.isEmpty()).toBe(true);
    expect(afterStats.totalEvents).toBe(200);
    expect(afterStats.droppedEvents).toBe(150);
    expect(afterStats.bufferedEvents).toBe(0);
  });

  // ------------------------------------------------------------------ //
  //  5. Window Storm                                                   //
  // ------------------------------------------------------------------ //

  test('Window Storm - 50K events through tumbling windows', () => {
    const eventCount = 50_000;
    const uniqueTimestamps = 500;
    const eventsPerTimestamp = eventCount / uniqueTimestamps;

    const events: StreamEvent<number>[] = [];
    for (let i = 0; i < eventCount; i++) {
      const ts = Math.floor(i / eventsPerTimestamp);
      events.push(createStreamEvent('sensor-1', i, new Timestamp(ts)));
    }

    const spec = createWindowSpec(WindowType.Tumbling, Duration.fromMillis(100));
    const assigner = new WindowAssigner<string, number>(spec);

    const memBefore = process.memoryUsage().heapUsed;
    const start = Date.now();

    const allWindowIds = new Set<string>();
    let totalAssigned = 0;

    for (const event of events) {
      const windows = assigner.assign(event, 'sensor-1');
      for (const w of windows) {
        allWindowIds.add(w.windowId);
        totalAssigned++;
      }
    }

    const elapsed = Date.now() - start;
    const memAfter = process.memoryUsage().heapUsed;
    const memDelta = memAfter - memBefore;

    const expectedWindows = Math.ceil(uniqueTimestamps / 100);

    console.log(`[chaos] Window Storm: ${eventCount} events, ${allWindowIds.size} windows in ${elapsed}ms`);
    console.log(`  Unique timestamps: ${uniqueTimestamps}, events per ts: ${eventsPerTimestamp}`);
    console.log(`  Total assigned: ${totalAssigned}, memory delta: ${(memDelta / 1024 / 1024).toFixed(2)}MB`);

    expect(totalAssigned).toBe(eventCount);
    expect(allWindowIds.size).toBe(expectedWindows);
    expect(elapsed).toBeLessThan(10_000);
  });

  // ------------------------------------------------------------------ //
  //  6. Retry Timeout Storm                                            //
  // ------------------------------------------------------------------ //

  test('Retry Timeout Storm - 100 failing operations with fake timers', async () => {
    jest.useFakeTimers();

    const retry = new RetryPolicy({
      maxAttempts: 3,
      strategy: BackoffStrategy.Fixed,
      initialDelay: 50,
      maxDelay: 5000,
      name: 'chaos-retry',
    });

    const failingFn = (): Promise<string> =>
      new Promise((_, reject) =>
        setTimeout(() => reject(new Error('timeout')), 10)
      );

    const promises = Array.from({ length: 100 }, () => retry.execute(failingFn));

    async function advance(ms: number) {
      jest.advanceTimersByTime(ms);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    }

    const startFakeTime = Date.now();

    await advance(10);
    await advance(50);
    await advance(10);
    await advance(50);
    await advance(10);

    const results = await Promise.all(promises);

    const fakeElapsed = Date.now() - startFakeTime;

    jest.useRealTimers();

    let totalAttempts = 0;
    let allFailed = true;

    for (const r of results) {
      totalAttempts += r.attempts;
      if (r.success) allFailed = false;
      expect(r.success).toBe(false);
      expect(r.attempts).toBe(3);
      expect(r.error).toBeDefined();
    }

    console.log(`[chaos] Retry Timeout Storm: 100 ops, ${totalAttempts} total attempts, fake time: ${fakeElapsed}ms`);
    console.log(`  All failed: ${allFailed}, avg attempts: ${(totalAttempts / 100).toFixed(1)}`);

    expect(totalAttempts).toBe(300);
    expect(allFailed).toBe(true);
    expect(fakeElapsed).toBeGreaterThanOrEqual(120);
    expect(fakeElapsed).toBeLessThanOrEqual(200);
  });
});
