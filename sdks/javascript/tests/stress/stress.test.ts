// @jest/tag:stress

import {
  Timestamp,
  Duration,
  WindowType,
  BackpressureStrategy,
  createWindowSpec,
} from '../../src/streaming/types';
import type { StreamEvent } from '../../src/streaming/types';
import { WindowAssigner } from '../../src/streaming/window';
import {
  BackpressureController,
  DEFAULT_BACKPRESSURE_CONFIG,
} from '../../src/streaming/backpressure';
import {
  CircuitBreaker,
  CircuitBreakerError,
} from '../../src/resilience/circuit_breaker';
import { RetryPolicy } from '../../src/resilience/retry';
import { BackoffStrategy } from '../../src/resilience/types';

describe('Stress Tests', () => {
  const skipStress = process.env.SKIP_STRESS === '1';

  beforeAll(() => {
    jest.setTimeout(30_000);
  });

  // ------------------------------------------------------------------ //
  //  1. 1M Stream Events Through Windowing                            #
  // ------------------------------------------------------------------ #

  test('1M Stream Events Through Windowing', () => {
    if (skipStress) return;

    const heapBefore = process.memoryUsage().heapUsed;

    const spec = createWindowSpec(WindowType.Tumbling, Duration.fromSeconds(1));
    const assigner = new WindowAssigner(spec);

    const N = 100_000;
    const start = Date.now();
    for (let i = 0; i < N; i++) {
      const event: StreamEvent<number> = {
        key: `k${i % 100}`,
        value: i,
        timestamp: new Timestamp(i % 10_000),
      };
      assigner.assign(event, `k${i % 100}`);
    }
    const elapsed = Date.now() - start;

    const heapAfter = process.memoryUsage().heapUsed;
    const growthMB = (heapAfter - heapBefore) / (1024 * 1024);
    const eps = N / (elapsed / 1000);

    console.log(`\n=== ${N/1000}K Stream Events Windowing ===`);
    console.log(`  Time:       ${elapsed}ms`);
    console.log(`  Events/sec: ${Math.round(eps).toLocaleString()}`);
    console.log(`  Mem growth: ${growthMB.toFixed(1)} MB`);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    console.log(`  Windows:    ${(assigner as any).windows.size}`);

    expect(elapsed).toBeLessThan(60_000);
  });

  // ------------------------------------------------------------------ #
  //  2. 100K Concurrent Circuit Breaker Operations                     #
  // ------------------------------------------------------------------ //

  test('100K Concurrent Circuit Breaker Operations', async () => {
    if (skipStress) return;

    const breaker = new CircuitBreaker({ failureThreshold: 5 });
    let successCount = 0;
    let failCount = 0;
    let rejectCount = 0;

    const ok = async () => 'ok';
    const fail = async () => {
      throw new Error('ECONNRESET');
    };

    const calls = Array.from({ length: 100_000 }, (_, i) =>
      breaker.execute(i % 2 === 0 ? ok : fail).then(
        () => {
          successCount++;
        },
        (err: unknown) => {
          if (err instanceof CircuitBreakerError) rejectCount++;
          else failCount++;
        },
      ),
    );

    const start = Date.now();
    await Promise.all(calls);
    const elapsed = Date.now() - start;

    console.log('\n=== 100K Concurrent Circuit Breaker ===');
    console.log(`  Time:      ${elapsed}ms`);
    console.log(`  Successes: ${successCount.toLocaleString()}`);
    console.log(`  Failures:  ${failCount.toLocaleString()}`);
    console.log(`  Rejected:  ${rejectCount.toLocaleString()}`);
    console.log(`  State:     ${breaker.getState()}`);
    console.log(
      `  Ops/sec:   ${Math.round(100_000 / (elapsed / 1000)).toLocaleString()}`,
    );

    expect(successCount + failCount + rejectCount).toBe(100_000);
  });

  // ------------------------------------------------------------------ #
  //  3. 1M Backpressure Push/Pop Cycles                                #
  // ------------------------------------------------------------------ #

  test('100K Backpressure Push/Pop Cycles', () => {
    if (skipStress) return;

    const N = 100_000;
    const ctrl = new BackpressureController({
      ...DEFAULT_BACKPRESSURE_CONFIG,
      bufferSize: 200_000,
    });

    const start = Date.now();
    let pushed = 0;
    for (let i = 0; i < N; i++) {
      const event: StreamEvent<number> = {
        key: 'k',
        value: i,
        timestamp: new Timestamp(i),
      };
      if (ctrl.tryPush(event)) pushed++;
    }

    let popped = 0;
    while (!ctrl.isEmpty()) {
      ctrl.pop();
      popped++;
    }
    const elapsed = Date.now() - start;

    console.log(`\n=== ${N/1000}K Backpressure Push/Pop ===`);
    console.log(`  Time:   ${elapsed}ms`);
    console.log(`  Pushed: ${pushed.toLocaleString()}`);
    console.log(`  Popped: ${popped.toLocaleString()}`);
    console.log(
      `  Ops/s:  ${Math.round(N * 2 / (elapsed / 1000)).toLocaleString()}`,
    );

    expect(pushed).toBe(N);
    expect(popped).toBe(N);
    expect(elapsed).toBeLessThan(30_000);
  });

  // ------------------------------------------------------------------ #
  //  4. Memory Stability                                               #
  // ------------------------------------------------------------------ //

  test('Memory Stability', () => {
    if (skipStress) return;

    const samples: { x: number; y: number }[] = [];
    const iterations = 10_000;

    for (let i = 0; i < iterations; i++) {
      const w = new WindowAssigner(
        createWindowSpec(WindowType.Tumbling, Duration.fromSeconds(1)),
      );
      for (let j = 0; j < 10; j++) {
        w.assign(
          { key: 'k', value: j, timestamp: new Timestamp(j) },
          'k',
        );
      }

      const bp = new BackpressureController({
        ...DEFAULT_BACKPRESSURE_CONFIG,
        bufferSize: 1000,
      });
      for (let j = 0; j < 100; j++) {
        bp.tryPush({ key: 'k', value: j, timestamp: new Timestamp(j) });
      }
      for (let j = 0; j < 100; j++) {
        bp.pop();
      }

      if (i % 100 === 0) {
        if (global.gc) global.gc();
        const mem = process.memoryUsage().heapUsed;
        samples.push({ x: i, y: mem });
      }
    }

    // Linear regression
    const n = samples.length;
    const sx = samples.reduce((a, s) => a + s.x, 0);
    const sy = samples.reduce((a, s) => a + s.y, 0);
    const sxy = samples.reduce((a, s) => a + s.x * s.y, 0);
    const sx2 = samples.reduce((a, s) => a + s.x * s.x, 0);
    const d = n * sx2 - sx * sx;
    const slope = d !== 0 ? (n * sxy - sx * sy) / d : 0;
    const slopePerIter = slope * 100;

    const ys = samples.map((s) => s.y);

    console.log('\n=== Memory Stability ===');
    console.log(`  Iterations:      ${iterations.toLocaleString()}`);
    console.log(`  Samples:         ${n}`);
    console.log(`  Slope:           ${slopePerIter.toFixed(1)} bytes/iteration`);
    console.log(`  First mem:       ${(samples[0].y / 1024).toFixed(1)} KB`);
    console.log(`  Last mem:        ${(samples[n - 1].y / 1024).toFixed(1)} KB`);
    console.log(
      `  Mem range:       ${((Math.max(...ys) - Math.min(...ys)) / 1024).toFixed(1)} KB`,
    );

    expect(slopePerIter).toBeLessThan(1024);
  });

  // ------------------------------------------------------------------ #
  //  5. Retry Storm                                                    #
  // ------------------------------------------------------------------ //

  test('Retry Storm (10K operations)', async () => {
    if (skipStress) return;

    const callCounts = new Map<number, number>();

    async function flaky(id: number): Promise<string> {
      const count = (callCounts.get(id) ?? 0) + 1;
      callCounts.set(id, count);
      if (count < 4) throw new Error('ECONNRESET');
      return `ok-${id}`;
    }

    const policy = new RetryPolicy({
      maxAttempts: 4,
      initialDelay: 1,
      maxDelay: 50,
      strategy: BackoffStrategy.Exponential,
      multiplier: 2,
    });

    const tasks = Array.from({ length: 10_000 }, (_, i) =>
      policy.execute(() => flaky(i)),
    );

    const start = Date.now();
    const results = await Promise.all(tasks);
    const elapsed = Date.now() - start;

    const successes = results.filter((r) => r.success).length;
    const failures = results.filter((r) => !r.success).length;

    console.log('\n=== Retry Storm (10K operations) ===');
    console.log(`  Time:      ${elapsed}ms`);
    console.log(`  Successes: ${successes.toLocaleString()}`);
    console.log(`  Failures:  ${failures.toLocaleString()}`);
    console.log(
      `  Ops/sec:   ${Math.round(10_000 / (elapsed / 1000)).toLocaleString()}`,
    );

    expect(successes).toBe(10_000);
  });
});
