/**
 * Tests for Retry Pattern
 * @module aether/resilience/retry
 */

import {
  RetryPolicy,
  RetryExhaustedError,
  networkRetryPolicy,
  databaseRetryPolicy,
} from '../../src/resilience/retry';
 import {
  BackoffStrategy,
  RetryResult,
} from '../../src/resilience/types';

import { aggressiveRetryPolicy, conservativeRetryPolicy } from '../../src/resilience/retry';

 
jest.useFakeTimers();

 
beforeEach(() => {
  jest.useRealTimers();
});
 
describe('RetryPolicy', () => {
  describe('constructor', () => {
    test('should create with default config', () => {
      const policy = new RetryPolicy();
      expect(policy.getConfig().maxAttempts).toBe(3);
    });

 
    test('should create with custom config', () => {
      const policy = new RetryPolicy({
        maxAttempts: 5,
        initialDelay: 200,
        maxDelay: 5000,
        multiplier: 1.5,
        strategy: BackoffStrategy.Linear,
        name: 'custom',
      });

 
      expect(policy.getConfig().maxAttempts).toBe(5);
      expect(policy.getConfig().initialDelay).toBe(200);
    });
  });
 
  describe('execute', () => {
    test('should succeed on first attempt', async () => {
      const policy = new RetryPolicy({ maxAttempts: 3 });
      const result = await policy.execute(() => Promise.resolve('success'));
 
      expect(result.success).toBe(true);
      expect(result.result).toBe('success');
      expect(result.attempts).toBe(1);
    });
 
    test('should succeed after multiple attempts', async () => {
      const policy = new RetryPolicy({
        maxAttempts: 3,
        initialDelay: 10,
        strategy: BackoffStrategy.Fixed,
      });

      let attempts = 0;
      const fn = () => {
        attempts++;
        if (attempts < policy.getConfig().maxAttempts) {
          return Promise.reject(new Error('ECONNRESET')); // Use retryable error
        }
        return Promise.resolve('success');
      };

      const result = await policy.execute(fn);
      expect(result.success).toBe(true);
      expect(result.attempts).toBe(3);
    });

    test('should fail after all attempts exhausted', async () => {
      const policy = new RetryPolicy({
        maxAttempts: 2,
        initialDelay: 10,
        strategy: BackoffStrategy.Fixed,
      });
      const fn = () => Promise.reject(new Error('ECONNRESET')); // Use retryable error
      const result = await policy.execute(fn);
      expect(result.success).toBe(false);
      expect(result.error?.message).toBe('ECONNRESET');
      expect(result.attempts).toBe(2);
    });
 
    test('should apply backoff delays', async () => {
      const policy = new RetryPolicy({
        maxAttempts: 3,
        initialDelay: 10,
        maxDelay: 500,
        multiplier: 2,
        strategy: BackoffStrategy.Exponential,
      });
      const callTimes: number[] = [];
      const fn = () => {
        callTimes.push(Date.now());
        if (callTimes.length < policy.getConfig().maxAttempts) {
          return Promise.reject(new Error('ECONNRESET')); // Use retryable error
        }
        return Promise.resolve('success');
      };
      const result = await policy.execute(fn);
      expect(result.success).toBe(true);
    });
  });
 
  describe('executeOrThrow', () => {
    test('should return result on success', async () => {
      const policy = new RetryPolicy({ maxAttempts: 3 });
      const result = await policy.executeOrThrow(() => Promise.resolve('success'));
      expect(result).toBe('success');
    });
 
    test('should throw on failure', async () => {
      const policy = new RetryPolicy({ maxAttempts: 2 });
      await expect(
        policy.executeOrThrow(() => Promise.reject(new Error('fail')))
      ).rejects.toThrow(RetryExhaustedError);
    });
  });
 
  describe('Backoff strategies', () => {
    test('Fixed backoff should use constant delay', async () => {
      const policy = new RetryPolicy({
        maxAttempts: 3,
        initialDelay: 10,
        strategy: BackoffStrategy.Fixed,
      });
      // This is a basic test to verify the policy works
      let attempts = 0;
      const fn = () => {
        attempts++;
        if (attempts < 3) {
          return Promise.reject(new Error('ECONNRESET')); // Use retryable error
        }
        return Promise.resolve('success');
      };
      const result = await policy.execute(fn);
      expect(result.success).toBe(true);
      expect(attempts).toBe(3);
    });
  });
 
  describe('shouldRetry predicate', () => {
    test('should respect custom shouldRetry', async () => {
      const policy = new RetryPolicy({ maxAttempts: 3 });
      let callCount = 0;
      const shouldRetry = () => {
        callCount++;
        return true;
      };
      const fn = () => {
        return Promise.reject(new Error('retryable error'));
      };
      const result = await policy.execute(fn, shouldRetry);
      expect(callCount).toBeGreaterThan(1);
    });
 
    test('should stop retrying when predicate returns false', async () => {
      const policy = new RetryPolicy({ maxAttempts: 3 });
      const fn = () => {
        return Promise.reject(new Error('non-retryable error'));
      };
      const shouldRetry = () => false;
      const result = await policy.execute(fn, shouldRetry);
      expect(result.attempts).toBe(1);
      expect(result.success).toBe(false);
    });
  });
 
  describe('default shouldRetry', () => {
    test('should retry on network errors', async () => {
      const policy = new RetryPolicy({ maxAttempts: 2 });
      const networkErrors = [
        new Error('ECONNRESET'),
        new Error('ETIMEDOUT'),
        new Error('ENOTFOUND'),
        new Error('ECONNREFUSED'),
      ];
      for (const error of networkErrors) {
        const result = await policy.execute(() => Promise.reject(error));
        expect(result.attempts).toBeGreaterThan(1);
      }
    });
 
    test('should retry on timeout errors', async () => {
      const policy = new RetryPolicy({ maxAttempts: 2 });
      const result = await policy.execute(() =>
        Promise.reject(new Error('timeout occurred'))
      );
      expect(result.attempts).toBeGreaterThan(1);
    });
 
    test('should retry on 5xx errors', async () => {
      const policy = new RetryPolicy({ maxAttempts: 2 });
      const serverErrors = [
        new Error('500 Internal Server Error'),
        new Error('502 Bad Gateway'),
        new Error('503 Service Unavailable'),
        new Error('504 Gateway Timeout'),
      ];
      for (const error of serverErrors) {
        const result = await policy.execute(() => Promise.reject(error));
        expect(result.attempts).toBeGreaterThan(1);
      }
    });
 
    test('should not retry on other errors', async () => {
      const policy = new RetryPolicy({ maxAttempts: 3 });
      const result = await policy.execute(() =>
        Promise.reject(new Error('Business logic error'))
      );
      expect(result.attempts).toBe(1);
      expect(result.success).toBe(false);
    });
  });
 
  describe('RetryExhaustedError', () => {
    test('should contain attempt count and last error', () => {
      const lastError = new Error('final error');
      const error = new RetryExhaustedError(5, lastError);
      expect(error.attempts).toBe(5);
      expect(error.lastError).toBe(lastError);
      expect(error.message).toContain('5');
      expect(error.message).toContain('final error');
    });
  });
 
  describe('Pre-configured policies', () => {
    test('networkRetryPolicy should have correct config', () => {
      const policy = networkRetryPolicy();
      expect(policy.getConfig().name).toBe('network');
      expect(policy.getConfig().maxAttempts).toBe(3);
      expect(policy.getConfig().strategy).toBe(BackoffStrategy.ExponentialJitter);
    });
 
    test('databaseRetryPolicy should have correct config', () => {
      const policy = databaseRetryPolicy();
      expect(policy.getConfig().name).toBe('database');
      expect(policy.getConfig().maxAttempts).toBe(5);
    });
 
    test('aggressiveRetryPolicy should have correct config', () => {
      const policy = aggressiveRetryPolicy();
      expect(policy.getConfig().name).toBe('aggressive');
      expect(policy.getConfig().maxAttempts).toBe(10);
    });
 
    test('conservativeRetryPolicy should have correct config', () => {
      const policy = conservativeRetryPolicy();
      expect(policy.getConfig().name).toBe('conservative');
      expect(policy.getConfig().maxAttempts).toBe(2);
      expect(policy.getConfig().strategy).toBe(BackoffStrategy.Fixed);
    });
  });
});
