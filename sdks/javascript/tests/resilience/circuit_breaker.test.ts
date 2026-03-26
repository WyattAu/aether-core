/**
 * Tests for Circuit Breaker Pattern
 * @module aether/resilience/circuit_breaker
 */

import {
  CircuitBreaker,
  CircuitBreakerError,
  CircuitBreakerManager,
  apiCircuitBreaker,
  databaseCircuitBreaker,
} from '../../src/resilience/circuit_breaker';
import { CircuitState } from '../../src/resilience/types';

describe('CircuitBreaker', () => {
  describe('constructor', () => {
    test('should create with default config', () => {
      const breaker = new CircuitBreaker();
      expect(breaker.getState()).toBe(CircuitState.Closed);
    });

    test('should create with custom config', () => {
      const breaker = new CircuitBreaker({
        name: 'test-breaker',
        failureThreshold: 3,
        successThreshold: 2,
        resetTimeout: 10000,
        failureWindow: 30000,
      });

      const stats = breaker.getStats();
      expect(stats.state).toBe(CircuitState.Closed);
    });
  });

  describe('Closed state', () => {
    test('should execute function successfully', async () => {
        const breaker = new CircuitBreaker({ name: 'test' });
        const result = await breaker.execute(() => Promise.resolve('success'));

        expect(result).toBe('success');
      });

    test('should track successes', async () => {
        const breaker = new CircuitBreaker({ name: 'test' });

        await breaker.execute(() => Promise.resolve(1));
        await breaker.execute(() => Promise.resolve(2));

        const stats = breaker.getStats();
        expect(stats.successCount).toBe(2);
        expect(stats.totalCalls).toBe(2);
      });

    test('should track failures', async () => {
        const breaker = new CircuitBreaker({
          name: 'test',
          failureThreshold: 5,
        });

        try {
          await breaker.execute(() => Promise.reject(new Error('test error')));
        } catch (e) {
          // Expected
        }

        const stats = breaker.getStats();
        expect(stats.failureCount).toBe(1);
      });

      test('should open after failure threshold', async () => {
        const breaker = new CircuitBreaker({
          name: 'test',
          failureThreshold: 3,
          failureWindow: 60000,
        });

        // Cause 3 failures
        for (let i = 0; i < 3; i++) {
          try {
            await breaker.execute(() => Promise.reject(new Error('fail')));
          } catch (e) {
            // Expected
          }
        }

        expect(breaker.getState()).toBe(CircuitState.Open);
      });
  });

  describe('Open state', () => {
    test('should reject all calls', async () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.forceState(CircuitState.Open);

      await expect(breaker.execute(() => Promise.resolve('success'))).rejects.toThrow(
        CircuitBreakerError
      );
    });

    test('should include state in error', async () => {
      const breaker = new CircuitBreaker({ name: 'test-breaker' });
      breaker.forceState(CircuitState.Open);

      try {
        await breaker.execute(() => Promise.resolve('success'));
        fail('Should have thrown');
      } catch (error) {
        expect(error).toBeInstanceOf(CircuitBreakerError);
        expect((error as CircuitBreakerError).state).toBe(CircuitState.Open);
        expect((error as Error).message).toContain('test-breaker');
      }
    });
  });

  describe('Half-Open state', () => {
    test('should allow limited calls', async () => {
      const breaker = new CircuitBreaker({
        name: 'test',
        successThreshold: 2,
      });
      breaker.forceState(CircuitState.HalfOpen);

      const result = await breaker.execute(() => Promise.resolve('success'));
      expect(result).toBe('success');
    });

    test('should close after success threshold', async () => {
      const breaker = new CircuitBreaker({
        name: 'test',
        successThreshold: 2,
      });
      breaker.forceState(CircuitState.HalfOpen);

      await breaker.execute(() => Promise.resolve(1));
      await breaker.execute(() => Promise.resolve(2));

      expect(breaker.getState()).toBe(CircuitState.Closed);
    });

    test('should open on any failure', async () => {
      const breaker = new CircuitBreaker({
        name: 'test',
        successThreshold: 3,
      });
      breaker.forceState(CircuitState.HalfOpen);

      try {
        await breaker.execute(() => Promise.reject(new Error('fail')));
      } catch (e) {
        // Expected
      }

      expect(breaker.getState()).toBe(CircuitState.Open);
    });
  });

  describe('executeWithFallback', () => {
    test('should return result on success', async () => {
      const breaker = new CircuitBreaker({ name: 'test' });

      const result = await breaker.executeWithFallback(
        () => Promise.resolve('primary'),
        () => Promise.resolve('fallback')
      );

      expect(result).toBe('primary');
    });

    test('should use fallback when circuit is open', async () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.forceState(CircuitState.Open);

      const result = await breaker.executeWithFallback(
        () => Promise.resolve('primary'),
        () => Promise.resolve('fallback')
      );

      expect(result).toBe('fallback');
    });

    test('should throw original error on failure', async () => {
      const breaker = new CircuitBreaker({ name: 'test' });

      await expect(
        breaker.executeWithFallback(
          () => Promise.reject(new Error('original error')),
          () => Promise.resolve('fallback')
        )
      ).rejects.toThrow('original error');
    });
  });

  describe('recordSuccess / recordFailure', () => {
    test('should record success manually', () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.recordSuccess();

      const stats = breaker.getStats();
      expect(stats.successCount).toBe(1);
    });

    test('should record failure manually', () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.recordFailure();

      const stats = breaker.getStats();
      expect(stats.failureCount).toBe(1);
    });
  });

  describe('forceState', () => {
    test('should force state to Open', () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.forceState(CircuitState.Open);

      expect(breaker.getState()).toBe(CircuitState.Open);
    });

    test('should force state to HalfOpen', () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.forceState(CircuitState.HalfOpen);

      expect(breaker.getState()).toBe(CircuitState.HalfOpen);
    });

    test('should force state to Closed', () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.forceState(CircuitState.Open);
      breaker.forceState(CircuitState.Closed);

      expect(breaker.getState()).toBe(CircuitState.Closed);
    });
  });

  describe('reset', () => {
    test('should reset to closed state', () => {
      const breaker = new CircuitBreaker({ name: 'test' });
      breaker.forceState(CircuitState.Open);
      breaker.reset();

      expect(breaker.getState()).toBe(CircuitState.Closed);
    });

    test('should clear counters', async () => {
      const breaker = new CircuitBreaker({ name: 'test' });

      await breaker.execute(() => Promise.resolve(1));
      breaker.recordFailure();

      breaker.reset();

      const stats = breaker.getStats();
      expect(stats.successCount).toBe(0);
      expect(stats.failureCount).toBe(0);
    });
  });
});

describe('CircuitBreakerManager', () => {
  test('should create and retrieve breakers', () => {
    const manager = new CircuitBreakerManager();

    const breaker1 = manager.getBreaker('api');
    const breaker2 = manager.getBreaker('api');

    expect(breaker1).toBe(breaker2);
  });

  test('should list all breaker names', () => {
    const manager = new CircuitBreakerManager();

    manager.getBreaker('api');
    manager.getBreaker('database');

    expect(manager.getNames()).toContain('api');
    expect(manager.getNames()).toContain('database');
  });

  test('should remove breaker', () => {
    const manager = new CircuitBreakerManager();

    manager.getBreaker('api');
    const removed = manager.remove('api');

    expect(removed).toBe(true);
    expect(manager.getNames()).not.toContain('api');
  });

  test('should clear all breakers', () => {
    const manager = new CircuitBreakerManager();

    manager.getBreaker('api');
    manager.getBreaker('database');
    manager.clear();

    expect(manager.getNames()).toHaveLength(0);
  });

  test('should use default config for new breakers', () => {
    const manager = new CircuitBreakerManager({
      failureThreshold: 10,
    });

    const breaker = manager.getBreaker('test');
    const stats = breaker.getStats();

    expect(stats.state).toBe(CircuitState.Closed);
  });
});

describe('Pre-configured breakers', () => {
  test('apiCircuitBreaker should have correct config', () => {
    const breaker = apiCircuitBreaker();
    expect(breaker.getState()).toBe(CircuitState.Closed);
  });

  test('databaseCircuitBreaker should have correct config', () => {
    const breaker = databaseCircuitBreaker();
    expect(breaker.getState()).toBe(CircuitState.Closed);
  });
});
