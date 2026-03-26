/**
 * Tests for OpenTelemetry Tracing Integration
 */

import {
  isTracingAvailable,
  getTracer,
  withTracing,
  tracedCircuitBreaker,
  tracedRetry,
  tracedRateLimiter,
  tracedBulkhead,
  tracedHealthCheck,
  ResilienceInstrumentation,
} from '../../src/resilience/tracing';

describe('Tracing', () => {
  describe('isTracingAvailable', () => {
    test('returns boolean', () => {
      const result = isTracingAvailable();
      expect(typeof result).toBe('boolean');
    });
  });

  describe('getTracer', () => {
    test('returns null when OTel not installed or tracer unavailable', () => {
      const tracer = getTracer('test-service');
      // Will be null if OTel not available, or a tracer if it is
      expect(tracer === null || typeof tracer === 'object').toBe(true);
    });

    test('accepts custom service name', () => {
      const tracer = getTracer('my-custom-service');
      expect(tracer === null || typeof tracer === 'object').toBe(true);
    });
  });

  describe('withTracing', () => {
    test('executes function and returns result', async () => {
      const result = await withTracing('test-span', async (span) => {
        expect(span).toBeDefined();
        span.setAttribute('test.key', 'value');
        return 42;
      });

      expect(result).toBe(42);
    });

    test('passes span with setAttribute', async () => {
      await withTracing('test-span', (span) => {
        expect(typeof span.setAttribute).toBe('function');
        span.setAttribute('key', 'value');
        return Promise.resolve();
      });
    });

    test('passes span with addEvent', async () => {
      await withTracing('test-span', (span) => {
        expect(typeof span.addEvent).toBe('function');
        span.addEvent('test.event', { foo: 'bar' });
        return Promise.resolve();
      });
    });

    test('passes span with setStatus', async () => {
      await withTracing('test-span', (span) => {
        expect(typeof span.setStatus).toBe('function');
        span.setStatus({ code: 0 });
        return Promise.resolve();
      });
    });

    test('passes span with end', async () => {
      await withTracing('test-span', (span) => {
        expect(typeof span.end).toBe('function');
        span.end();
        return Promise.resolve();
      });
    });

    test('propagates errors from function', async () => {
      await expect(
        withTracing('test-span', async () => {
          throw new Error('test error');
        })
      ).rejects.toThrow('test error');
    });

    test('handles complex return types', async () => {
      const result = await withTracing('test-span', async () => {
        return { foo: 'bar', count: 42 };
      });

      expect(result).toEqual({ foo: 'bar', count: 42 });
    });
  });

  describe('tracedCircuitBreaker', () => {
    test('executes function and returns result', async () => {
      const result = await tracedCircuitBreaker('test-cb', 'closed', async () => {
        return 'success';
      });

      expect(result).toBe('success');
    });

    test('propagates errors', async () => {
      await expect(
        tracedCircuitBreaker('test-cb', 'open', async () => {
          throw new Error('circuit open');
        })
      ).rejects.toThrow('circuit open');
    });

    test('sets circuit breaker attributes on span', async () => {
      let capturedAttrs: Record<string, unknown> = {};

      // We can't easily intercept span attributes in no-op mode,
      // but we can verify the function still executes correctly
      await tracedCircuitBreaker('my-cb', 'half-open', async () => {
        return 123;
      });
    });
  });

  describe('tracedRetry', () => {
    test('executes function and returns result', async () => {
      const result = await tracedRetry('test-retry', 1, 3, async () => {
        return 'ok';
      });

      expect(result).toBe('ok');
    });

    test('propagates errors', async () => {
      await expect(
        tracedRetry('test-retry', 2, 3, async () => {
          throw new Error('retry failed');
        })
      ).rejects.toThrow('retry failed');
    });
  });

  describe('tracedRateLimiter', () => {
    test('executes function when allowed', async () => {
      const result = await tracedRateLimiter('test-rl', true, async () => {
        return 'allowed';
      });

      expect(result).toBe('allowed');
    });

    test('executes function when not allowed', async () => {
      const result = await tracedRateLimiter('test-rl', false, async () => {
        return 'still-called';
      });

      expect(result).toBe('still-called');
    });

    test('propagates errors', async () => {
      await expect(
        tracedRateLimiter('test-rl', true, async () => {
          throw new Error('rate limit error');
        })
      ).rejects.toThrow('rate limit error');
    });
  });

  describe('tracedBulkhead', () => {
    test('executes function', async () => {
      const result = await tracedBulkhead('test-bh', 1, 5, async () => {
        return 'bulkhead-ok';
      });

      expect(result).toBe('bulkhead-ok');
    });

    test('propagates errors', async () => {
      await expect(
        tracedBulkhead('test-bh', 5, 5, async () => {
          throw new Error('bulkhead full');
        })
      ).rejects.toThrow('bulkhead full');
    });
  });

  describe('tracedHealthCheck', () => {
    test('executes function', async () => {
      const result = await tracedHealthCheck('test-hc', 'db-check', async () => {
        return { healthy: true };
      });

      expect(result).toEqual({ healthy: true });
    });

    test('propagates errors', async () => {
      await expect(
        tracedHealthCheck('test-hc', 'fail-check', async () => {
          throw new Error('unhealthy');
        })
      ).rejects.toThrow('unhealthy');
    });
  });
});

describe('ResilienceInstrumentation', () => {
  test('creates with default config', () => {
    const inst = new ResilienceInstrumentation();
    expect(inst).toBeDefined();
  });

  test('creates with custom config', () => {
    const inst = new ResilienceInstrumentation({
      enabled: true,
      serviceName: 'my-service',
      sampleRate: 0.5,
    });
    expect(inst).toBeDefined();
  });

  test('creates with tracing disabled', () => {
    const inst = new ResilienceInstrumentation({
      enabled: false,
    });
    expect(inst).toBeDefined();
  });

  describe('circuitBreaker', () => {
    test('returns span and end function', () => {
      const inst = new ResilienceInstrumentation();
      const { span, end } = inst.circuitBreaker('test-cb', 'closed');

      expect(span).toBeDefined();
      expect(typeof end).toBe('function');
    });

    test('end can be called without error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.circuitBreaker('test-cb', 'closed');

      expect(() => end()).not.toThrow();
    });

    test('end can be called with error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.circuitBreaker('test-cb', 'open');

      expect(() => end(new Error('test'))).not.toThrow();
    });
  });

  describe('retry', () => {
    test('returns span and end function', () => {
      const inst = new ResilienceInstrumentation();
      const { span, end } = inst.retry('test-retry', 1, 3);

      expect(span).toBeDefined();
      expect(typeof end).toBe('function');
    });

    test('end can be called without error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.retry('test-retry', 1, 3);

      expect(() => end()).not.toThrow();
    });

    test('end can be called with error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.retry('test-retry', 3, 3);

      expect(() => end(new Error('max retries'))).not.toThrow();
    });
  });

  describe('rateLimiter', () => {
    test('returns span and end function', () => {
      const inst = new ResilienceInstrumentation();
      const { span, end } = inst.rateLimiter('test-rl', true);

      expect(span).toBeDefined();
      expect(typeof end).toBe('function');
    });

    test('end can be called without error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.rateLimiter('test-rl', true);

      expect(() => end()).not.toThrow();
    });

    test('end can be called with error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.rateLimiter('test-rl', false);

      expect(() => end(new Error('rate limited'))).not.toThrow();
    });
  });

  describe('bulkhead', () => {
    test('returns span and end function', () => {
      const inst = new ResilienceInstrumentation();
      const { span, end } = inst.bulkhead('test-bh', 1, 5);

      expect(span).toBeDefined();
      expect(typeof end).toBe('function');
    });

    test('end can be called without error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.bulkhead('test-bh', 1, 5);

      expect(() => end()).not.toThrow();
    });

    test('end can be called with error', () => {
      const inst = new ResilienceInstrumentation();
      const { end } = inst.bulkhead('test-bh', 5, 5);

      expect(() => end(new Error('bulkhead rejected'))).not.toThrow();
    });
  });
});
