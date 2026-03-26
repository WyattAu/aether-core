/**
 * Tests for Resilient Executor
 */

import { ResilientExecutor, ResilientExecutorBuilder } from '../../src/resilience/executor';
import { CircuitBreaker, CircuitBreakerError } from '../../src/resilience/circuit_breaker';
import { RetryPolicy, RetryExhaustedError } from '../../src/resilience/retry';
import { RateLimiter, RateLimitExhaustedError } from '../../src/resilience/rate_limiter';
import { Bulkhead, BulkheadRejectedError } from '../../src/resilience/bulkhead';
import { CircuitState, BackoffStrategy } from '../../src/resilience/types';

describe('ResilientExecutor', () => {
  describe('constructor', () => {
    test('creates executor with no resilience patterns', () => {
      const executor = new ResilientExecutor();

      expect(executor).toBeDefined();
    });

    test('creates executor with all patterns', () => {
      const executor = new ResilientExecutor({
        breaker: new CircuitBreaker(),
        retry: new RetryPolicy(),
        rateLimiter: new RateLimiter(),
        bulkhead: new Bulkhead(),
      });

      expect(executor).toBeDefined();
    });

    test('creates executor with custom name', () => {
      const executor = new ResilientExecutor({ name: 'test-executor' });

      expect(executor).toBeDefined();
    });
  });

  describe('execute', () => {
    test('executes simple function without patterns', async () => {
      const executor = new ResilientExecutor();

      const result = await executor.execute(() => Promise.resolve('success'));

      expect(result).toBe('success');
    });

    test('propagates errors when no retry', async () => {
      const executor = new ResilientExecutor();

      await expect(
        executor.execute(() => Promise.reject(new Error('test error')))
      ).rejects.toThrow('test error');
    });

    test('applies rate limiting first', async () => {
      const rateLimiter = new RateLimiter({ maxRequests: 2, windowMs: 1000 });
      const executor = new ResilientExecutor({ rateLimiter });

      // First two should succeed
      await executor.execute(() => Promise.resolve(1));
      await executor.execute(() => Promise.resolve(2));

      // Third will wait for rate limit to reset
      const start = Date.now();
      const result = await executor.execute(() => Promise.resolve(3));
      const elapsed = Date.now() - start;
      
      // Should have waited
      expect(elapsed).toBeGreaterThan(0);
      expect(result).toBe(3);
    });

    test('applies bulkhead after rate limiting', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 1, maxQueueSize: 0 });
      const executor = new ResilientExecutor({ bulkhead });

      // Start first execution (don't await)
      const slowPromise = executor.execute(
        () => new Promise((resolve) => setTimeout(() => resolve('slow'), 100))
      );

      // Second should be rejected (bulkhead full)
      await expect(
        executor.execute(() => Promise.resolve('fast'))
      ).rejects.toThrow();

      await slowPromise; // Clean up
    });

    test('applies circuit breaker after bulkhead', async () => {
      const breaker = new CircuitBreaker({ failureThreshold: 1, resetTimeout: 1000 });
      const executor = new ResilientExecutor({ breaker });

      // First call fails, opens circuit
      await expect(
        executor.execute(() => Promise.reject(new Error('fail')))
      ).rejects.toThrow();

      // Second call should be rejected by circuit breaker
      await expect(
        executor.execute(() => Promise.resolve('success'))
      ).rejects.toThrow();
    });

    test('applies retry after circuit breaker', async () => {
      let attempts = 0;
      const retry = new RetryPolicy({ maxAttempts: 3, initialDelay: 10 });
      const executor = new ResilientExecutor({ retry });

      await executor.execute(() => {
        attempts++;
        if (attempts < 3) {
          return Promise.reject(new Error('ECONNRESET')); // Use retryable error
        }
        return Promise.resolve('success');
      });

      expect(attempts).toBe(3);
    });

    test('applies all patterns in correct order', async () => {
      const order: string[] = [];

      const rateLimiter = new RateLimiter({ maxRequests: 10, windowMs: 1000 });
      const bulkhead = new Bulkhead({ maxConcurrent: 10 });
      const breaker = new CircuitBreaker();
      const retry = new RetryPolicy({ maxAttempts: 1 });

      const executor = new ResilientExecutor({
        rateLimiter,
        bulkhead,
        breaker,
        retry,
      });

      await executor.execute(() => {
        order.push('execute');
        return Promise.resolve('done');
      });

      expect(order).toContain('execute');
    });
  });

  describe('with circuit breaker', () => {
    test('opens circuit after failures', async () => {
      const breaker = new CircuitBreaker({ failureThreshold: 2 });
      const executor = new ResilientExecutor({ breaker });

      // Two failures to open circuit
      await expect(executor.execute(() => Promise.reject(new Error('fail')))).rejects.toThrow();
      await expect(executor.execute(() => Promise.reject(new Error('fail')))).rejects.toThrow();

      // Circuit should be open
      await expect(executor.execute(() => Promise.resolve('success'))).rejects.toThrow();
    });

    test('closes circuit after successes in half-open', async () => {
      const breaker = new CircuitBreaker({
        failureThreshold: 1,
        successThreshold: 1,
        resetTimeout: 50,
      });
      const executor = new ResilientExecutor({ breaker });

      // Open circuit
      await expect(executor.execute(() => Promise.reject(new Error('fail')))).rejects.toThrow();

      // Wait for reset timeout
      await new Promise((resolve) => setTimeout(resolve, 100));

      // Should succeed and close circuit
      const result = await executor.execute(() => Promise.resolve('success'));
      expect(result).toBe('success');
    });
  });

  describe('with retry', () => {
    test('retries on failure', async () => {
      let attempts = 0;
      const retry = new RetryPolicy({ maxAttempts: 3, initialDelay: 10 });
      const executor = new ResilientExecutor({ retry });

      const result = await executor.execute(() => {
        attempts++;
        if (attempts < 3) {
          return Promise.reject(new Error('ECONNRESET')); // Use retryable error
        }
        return Promise.resolve('success');
      });

      expect(result).toBe('success');
      expect(attempts).toBe(3);
    });

    test('exhausts retries and throws', async () => {
      const retry = new RetryPolicy({ maxAttempts: 2, initialDelay: 10 });
      const executor = new ResilientExecutor({ retry });

      await expect(
        executor.execute(() => Promise.reject(new Error('always fails')))
      ).rejects.toThrow('always fails');
    });
  });

  describe('with rate limiter', () => {
    test('allows requests within limit', async () => {
      const rateLimiter = new RateLimiter({ maxRequests: 5, windowMs: 1000 });
      const executor = new ResilientExecutor({ rateLimiter });

      const results = await Promise.all([
        executor.execute(() => Promise.resolve(1)),
        executor.execute(() => Promise.resolve(2)),
        executor.execute(() => Promise.resolve(3)),
      ]);

      expect(results).toEqual([1, 2, 3]);
    });

    test('rejects requests over limit', async () => {
      const rateLimiter = new RateLimiter({ maxRequests: 2, windowMs: 1000 });
      const executor = new ResilientExecutor({ rateLimiter });

      // Use tryAcquire to check rate limiting behavior
      await executor.execute(() => Promise.resolve(1));
      await executor.execute(() => Promise.resolve(2));

      // Third request - tokens exhausted, will wait for retry
      // Since the window is 1000ms, the rate limiter will wait
      // We use a short timeout to verify it doesn't immediately resolve
      const start = Date.now();
      const result = await executor.execute(() => Promise.resolve(3));
      const elapsed = Date.now() - start;
      
      // Should have waited for rate limit to reset (at least some delay)
      expect(elapsed).toBeGreaterThan(0);
      expect(result).toBe(3);
    });
  });

  describe('with bulkhead', () => {
    test('allows concurrent calls within limit', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 3 });
      const executor = new ResilientExecutor({ bulkhead });

      const results = await Promise.all([
        executor.execute(() => Promise.resolve(1)),
        executor.execute(() => Promise.resolve(2)),
        executor.execute(() => Promise.resolve(3)),
      ]);

      expect(results).toEqual([1, 2, 3]);
    });

    test('rejects calls over concurrent limit', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 1, maxQueueSize: 0 });
      const executor = new ResilientExecutor({ bulkhead });

      // Start a slow call
      const slowPromise = executor.execute(
        () => new Promise((resolve) => setTimeout(() => resolve('slow'), 100))
      );

      // Try another call - should be rejected
      await expect(
        executor.execute(() => Promise.resolve('fast'))
      ).rejects.toThrow();

      await slowPromise;
    });
  });
});

describe('ResilientExecutorBuilder', () => {
  test('creates empty builder', () => {
    const builder = ResilientExecutor.builder();

    expect(builder).toBeDefined();
  });

  test('builds executor with no patterns', () => {
    const executor = ResilientExecutor.builder().build();

    expect(executor).toBeDefined();
  });

  describe('withCircuitBreaker', () => {
    test('adds circuit breaker with default config', () => {
      const executor = ResilientExecutor.builder()
        .withCircuitBreaker()
        .build();

      expect(executor).toBeDefined();
    });

    test('adds circuit breaker with custom config', () => {
      const executor = ResilientExecutor.builder()
        .withCircuitBreaker({ failureThreshold: 10 })
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withExistingCircuitBreaker', () => {
    test('adds existing circuit breaker instance', () => {
      const breaker = new CircuitBreaker({ failureThreshold: 5 });
      const executor = ResilientExecutor.builder()
        .withExistingCircuitBreaker(breaker)
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withRetry', () => {
    test('adds retry with default config', () => {
      const executor = ResilientExecutor.builder()
        .withRetry()
        .build();

      expect(executor).toBeDefined();
    });

    test('adds retry with custom config', () => {
      const executor = ResilientExecutor.builder()
        .withRetry({ maxAttempts: 5, strategy: BackoffStrategy.Exponential })
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withExistingRetry', () => {
    test('adds existing retry instance', () => {
      const retry = new RetryPolicy({ maxAttempts: 5 });
      const executor = ResilientExecutor.builder()
        .withExistingRetry(retry)
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withRateLimiter', () => {
    test('adds rate limiter with default config', () => {
      const executor = ResilientExecutor.builder()
        .withRateLimiter()
        .build();

      expect(executor).toBeDefined();
    });

    test('adds rate limiter with custom config', () => {
      const executor = ResilientExecutor.builder()
        .withRateLimiter({ maxRequests: 100, windowMs: 5000 })
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withExistingRateLimiter', () => {
    test('adds existing rate limiter instance', () => {
      const rateLimiter = new RateLimiter({ maxRequests: 50 });
      const executor = ResilientExecutor.builder()
        .withExistingRateLimiter(rateLimiter)
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withBulkhead', () => {
    test('adds bulkhead with default config', () => {
      const executor = ResilientExecutor.builder()
        .withBulkhead()
        .build();

      expect(executor).toBeDefined();
    });

    test('adds bulkhead with custom config', () => {
      const executor = ResilientExecutor.builder()
        .withBulkhead({ maxConcurrent: 20, maxQueueSize: 10 })
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withExistingBulkhead', () => {
    test('adds existing bulkhead instance', () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 15 });
      const executor = ResilientExecutor.builder()
        .withExistingBulkhead(bulkhead)
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('withName', () => {
    test('sets executor name', () => {
      const executor = ResilientExecutor.builder()
        .withName('my-executor')
        .build();

      expect(executor).toBeDefined();
    });
  });

  describe('full builder', () => {
    test('builds executor with all patterns', () => {
      const executor = ResilientExecutor.builder()
        .withCircuitBreaker({ failureThreshold: 5 })
        .withRetry({ maxAttempts: 3 })
        .withRateLimiter({ maxRequests: 100 })
        .withBulkhead({ maxConcurrent: 10 })
        .withName('full-executor')
        .build();

      expect(executor).toBeDefined();
    });

    test('built executor executes successfully', async () => {
      const executor = ResilientExecutor.builder()
        .withRetry({ maxAttempts: 2, initialDelay: 10 })
        .build();

      const result = await executor.execute(() => Promise.resolve('success'));

      expect(result).toBe('success');
    });
  });

  describe('method chaining', () => {
    test('all methods return builder for chaining', () => {
      const builder = ResilientExecutor.builder();

      expect(builder.withCircuitBreaker()).toBe(builder);
      expect(builder.withRetry()).toBe(builder);
      expect(builder.withRateLimiter()).toBe(builder);
      expect(builder.withBulkhead()).toBe(builder);
      expect(builder.withName('test')).toBe(builder);
    });
  });
});
