import { CircuitBreaker } from '../../src/resilience/circuit_breaker';
import { RateLimiter } from '../../src/resilience/rate_limiter';
import { BackpressureController } from '../../src/streaming/backpressure';
import { Timestamp, BackpressureStrategy } from '../../src/streaming/types';

const DURATION_MS = parseInt(process.env.STABILITY_TEST_DURATION_MS || '3000', 10);

describe('Stability Tests', () => {
  let startTime: number;

  beforeEach(() => {
    startTime = Date.now();
  });

  function timeRemaining(): number {
    return Math.max(0, DURATION_MS - (Date.now() - startTime));
  }

  it('circuit breaker handles continuous operations without leaks', async () => {
    const breaker = new CircuitBreaker({
      failureThreshold: 5,
      resetTimeout: 1000,
      successThreshold: 3,
    });

    const errors: Error[] = [];
    let operations = 0;
    const interval = setInterval(async () => {
      if (timeRemaining() <= 0) {
        clearInterval(interval);
        return;
      }
      try {
        await breaker.execute(async () => Math.random());
        operations++;
      } catch (e) {
        errors.push(e as Error);
      }
    }, 10);

    await new Promise<void>((resolve) => {
      const check = setInterval(() => {
        if (timeRemaining() <= 0) {
          clearInterval(check);
          clearInterval(interval);
          resolve();
        }
      }, 100);
    });

    expect(errors.length).toBe(0);
    expect(operations).toBeGreaterThan(0);
  }, 10000);

  it('rate limiter maintains steady throughput over duration', async () => {
    const limiter = new RateLimiter({
      maxRequests: 100,
      windowMs: 1000,
    });

    const successes: number[] = [];
    const interval = setInterval(async () => {
      if (timeRemaining() <= 0) {
        clearInterval(interval);
        return;
      }
      const result = limiter.tryAcquire();
      if (result.allowed) {
        successes.push(Date.now());
      }
    }, 5);

    await new Promise<void>((resolve) => {
      const check = setInterval(() => {
        if (timeRemaining() <= 0) {
          clearInterval(check);
          clearInterval(interval);
          resolve();
        }
      }, 100);
    });

    expect(successes.length).toBeGreaterThan(0);
  }, 10000);

  it('backpressure controller handles burst and drain patterns', async () => {
    const controller = new BackpressureController({
      strategy: BackpressureStrategy.Drop,
      highWatermark: 0.9,
      lowWatermark: 0.1,
      bufferSize: 1500,
    });

    const errors: Error[] = [];
    let processed = 0;
    const BURST_DURATION_MS = 2000;
    const burstStart = Date.now();

    const interval = setInterval(() => {
      if (Date.now() - burstStart >= BURST_DURATION_MS) {
        clearInterval(interval);
        return;
      }
      try {
        const pressure = Math.random() > 0.5
          ? Math.floor(Math.random() * 1500)
          : Math.floor(Math.random() * 50);
        const event = { key: 'k', value: pressure, timestamp: new Timestamp(Date.now()) };
        controller.tryPush(event);
        processed++;
      } catch (e) {
        errors.push(e as Error);
      }
    }, 20);

    await new Promise<void>((resolve) => {
      const check = setInterval(() => {
        if (Date.now() - burstStart >= BURST_DURATION_MS) {
          clearInterval(check);
          clearInterval(interval);
          resolve();
        }
      }, 100);
    });

    expect(errors.length).toBe(0);
    expect(processed).toBeGreaterThan(0);
  }, 10000);

  afterAll(() => {
    const elapsed = Date.now() - startTime;
    console.log(`Stability test duration: ${elapsed}ms (target: ${DURATION_MS}ms)`);
  });
});
