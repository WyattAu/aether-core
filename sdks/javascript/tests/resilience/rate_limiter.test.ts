/**
 * Tests for Rate Limiter Pattern
 */

import {
  RateLimiter,
  RateLimitExhaustedError,
  RateLimiterManager,
  apiRateLimiter,
  strictRateLimiter,
  burstyRateLimiter,
} from '../../src/resilience/rate_limiter';
import { RateLimitStrategy } from '../../src/resilience/types';

describe('RateLimiter', () => {
  beforeEach(() => {
    jest.useRealTimers();
  });

  describe('TokenBucket strategy', () => {
    test('should allow requests up to bucket size', () => {
      const limiter = new RateLimiter({
        maxRequests: 100,
        refillRate: 50,
        strategy: RateLimitStrategy.TokenBucket,
      });

      for (let i = 0; i < 10; i++) {
        const result = limiter.tryAcquire();
        expect(result.allowed).toBe(true);
      }
    });

    test('should deny when tokens exhausted', () => {
      const limiter = new RateLimiter({
        maxRequests: 5,
        refillRate: 5,
        strategy: RateLimitStrategy.TokenBucket,
      });

      // Use all tokens
      for (let i = 0; i < 5; i++) {
        limiter.tryAcquire();
      }

      // Next should be denied
      const result = limiter.tryAcquire();
      expect(result.allowed).toBe(false);
    });

    test('should calculate correct wait time', () => {
      const limiter = new RateLimiter({
        maxRequests: 5,
        refillRate: 10,
        strategy: RateLimitStrategy.TokenBucket,
      });

      // Use all tokens
      for (let i = 0; i < 5; i++) {
        limiter.tryAcquire();
      }

      const result = limiter.tryAcquire();
      expect(result.allowed).toBe(false);
      expect(result.retryAfter).toBeGreaterThanOrEqual(0);
    });
  });

  describe('SlidingWindow strategy', () => {
    test('should allow requests within window', () => {
      const limiter = new RateLimiter({
        maxRequests: 5,
        windowMs: 1000,
        strategy: RateLimitStrategy.SlidingWindow,
      });

      const results = [];
      for (let i = 0; i < 5; i++) {
        results.push(limiter.tryAcquire());
      }

      expect(results.every((r) => r.allowed)).toBe(true);
    });

    test('should deny when window is full', () => {
      const limiter = new RateLimiter({
        maxRequests: 3,
        windowMs: 1000,
        strategy: RateLimitStrategy.SlidingWindow,
      });

      // Fill the window
      for (let i = 0; i < 3; i++) {
        limiter.tryAcquire();
      }

      // Next should be denied
      const result = limiter.tryAcquire();
      expect(result.allowed).toBe(false);
    });
  });

  describe('FixedWindow strategy', () => {
    test('should allow requests within window', () => {
      const limiter = new RateLimiter({
        maxRequests: 5,
        windowMs: 1000,
        strategy: RateLimitStrategy.FixedWindow,
      });

      const results = [];
      for (let i = 0; i < 5; i++) {
        results.push(limiter.tryAcquire());
      }

      expect(results.every((r) => r.allowed)).toBe(true);
    });

    test('should deny when window is full', () => {
      const limiter = new RateLimiter({
        maxRequests: 2,
        windowMs: 1000,
        strategy: RateLimitStrategy.FixedWindow,
      });

      limiter.tryAcquire();
      limiter.tryAcquire();

      const result = limiter.tryAcquire();
      expect(result.allowed).toBe(false);
    });
  });

  describe('acquire method', () => {
    test('should resolve when allowed', async () => {
      const limiter = new RateLimiter({ maxRequests: 10 });

      await expect(limiter.acquire()).resolves.toBeUndefined();
    });

    test('should wait and retry when rate limited', async () => {
      jest.useFakeTimers();
      
      const limiter = new RateLimiter({
        maxRequests: 1,
        windowMs: 1000,
      });

      // Use the single request
      await limiter.acquire();

      // Next call should wait and retry (not throw immediately)
      const acquirePromise = limiter.acquire();
      
      // Advance time past the window to allow the retry to succeed
      jest.advanceTimersByTime(1100);
      
      // Should resolve after waiting, not throw
      await expect(acquirePromise).resolves.toBeUndefined();
      
      jest.useRealTimers();
    });
  });

  describe('execute method', () => {
    test('should execute function when allowed', async () => {
      const limiter = new RateLimiter({ maxRequests: 10 });
      const result = await limiter.execute(() => Promise.resolve('success'));
      expect(result).toBe('success');
    });

    test('should wait and retry when rate limited', async () => {
      jest.useFakeTimers();
      
      const limiter = new RateLimiter({
        maxRequests: 1,
        windowMs: 1000,
      });

      // First call succeeds
      const firstResult = await limiter.execute(() => Promise.resolve('first'));
      expect(firstResult).toBe('first');

      // Second call should wait and retry (not throw immediately)
      const executePromise = limiter.execute(() => Promise.resolve('second'));
      
      // Advance time past the window to allow the retry to succeed
      jest.advanceTimersByTime(1100);
      
      // Should resolve after waiting, not throw
      await expect(executePromise).resolves.toBe('second');
      
      jest.useRealTimers();
    });
  });

});

describe('RateLimiterManager', () => {
  test('should create and retrieve limiters', () => {
    const manager = new RateLimiterManager();
    const limiter1 = manager.getLimiter('api');
    const limiter2 = manager.getLimiter('database');

    expect(limiter1).toBeDefined();
    expect(limiter2).toBeDefined();
    expect(limiter1).not.toBe(limiter2);
  });

  test('should return same limiter for same name', () => {
    const manager = new RateLimiterManager();
    const limiter1 = manager.getLimiter('api');
    const limiter2 = manager.getLimiter('api');

    expect(limiter1).toBe(limiter2);
  });

  test('should use default config', () => {
    const manager = new RateLimiterManager({
      maxRequests: 50,
      windowMs: 500,
    });
    const limiter = manager.getLimiter('custom');
    const result = limiter.tryAcquire();
    expect(result.allowed).toBe(true);
  });

  test('should clear all limiters', () => {
    const manager = new RateLimiterManager();
    manager.getLimiter('api');
    manager.getLimiter('database');
    manager.clear();

    expect(manager.getNames()).toHaveLength(0);
  });

  test('should list limiter names', () => {
    const manager = new RateLimiterManager();
    manager.getLimiter('api');
    manager.getLimiter('database');

    const names = manager.getNames();
    expect(names).toContain('api');
    expect(names).toContain('database');
  });
});

describe('Pre-configured limiters', () => {
  test('apiRateLimiter should have correct config', () => {
    const limiter = apiRateLimiter();
    const result = limiter.tryAcquire();

    expect(result.allowed).toBe(true);
  });

  test('strictRateLimiter should have correct config', () => {
    const limiter = strictRateLimiter();
    const result = limiter.tryAcquire();

    expect(result.allowed).toBe(true);
  });

  test('burstyRateLimiter should have correct config', () => {
    const limiter = burstyRateLimiter();
    const result = limiter.tryAcquire();

    expect(result.allowed).toBe(true);
  });
});
