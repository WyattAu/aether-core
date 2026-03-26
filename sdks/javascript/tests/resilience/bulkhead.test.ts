/**
 * Tests for Bulkhead Pattern
 * @module aether/resilience/bulkhead
 */

import {
  Bulkhead,
  BulkheadRejectedError,
  BulkheadTimeoutError,
  BulkheadManager,
  apiBulkhead,
  databaseBulkhead,
  strictBulkhead,
} from '../../src/resilience/bulkhead';

import { BulkheadConfig } from '../../src/resilience/types';

jest.useFakeTimers();

beforeEach(() => {
  jest.useRealTimers();
});

describe('Bulkhead', () => {
  describe('constructor', () => {
    test('should create with default config', () => {
      const bulkhead = new Bulkhead();
      const stats = bulkhead.getStats();

      expect(stats.active).toBe(0);
      expect(stats.available).toBe(10);
    });

    test('should create with custom config', () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 5,
        maxQueueSize: 10,
        queueTimeout: 1000,
        name: 'test',
      });

      const stats = bulkhead.getStats();
      expect(stats.available).toBe(5);
    });
  });

  describe('execute', () => {
    test('should execute function when capacity available', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 2 });
      const result = await bulkhead.execute(() => Promise.resolve('success'));
      expect(result).toBe('success');
    });

    test('should track active calls', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 2 });
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      
      const stats = bulkhead.getStats();
      expect(stats.active).toBe(2);
      expect(stats.accepted).toBe(2);
    });

    test('should reject when at capacity without queue', async () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 1,
        maxQueueSize: 0,
        name: 'test',
      });

      // First call succeeds and stays active
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      
      // Second call should be rejected immediately
      await expect(bulkhead.execute(() => Promise.resolve(2))).rejects.toThrow(BulkheadRejectedError);
    });

    test('should queue when capacity available and queue enabled', async () => {
      const bulkhead = new Bulkhead({
        maxConcurrent: 1,
        maxQueueSize: 5,
        queueTimeout: 500,
        name: 'test',
      });
      
      const executionOrder: number[] = [];
      
      // First call occupies the slot and takes 50ms
      const fn1 = () => new Promise<string>(resolve => {
        setTimeout(() => {
          executionOrder.push(1);
          resolve('first');
        }, 50);
      });
      
      // Second and third calls should be queued and execute after first completes
      const fn2 = () => new Promise<string>(resolve => {
        setTimeout(() => {
          executionOrder.push(2);
          resolve('second');
        }, 50);
      });

      const fn3 = () => new Promise<string>(resolve => {
        setTimeout(() => {
          executionOrder.push(3);
          resolve('third');
        }, 50);
      });
      
      const results = await Promise.all([
        bulkhead.execute(fn1),
        bulkhead.execute(fn2),
        bulkhead.execute(fn3),
      ]);
      
      // All should complete
      expect(results).toEqual(['first', 'second', 'third']);
      expect(executionOrder).toEqual([1, 2, 3]);
    });
  });

  describe('hasCapacity', () => {
    test('should return true when capacity available', () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 2 });
      expect(bulkhead.hasCapacity()).toBe(true);
    });

    test('should return false when at capacity', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 1 });
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      expect(bulkhead.hasCapacity()).toBe(false);
    });
  });

  describe('availablePermits', () => {
    test('should return available permits', () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 5 });
      expect(bulkhead.availablePermits()).toBe(5);
    });

    test('should decrease available permits', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 5 });
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      expect(bulkhead.availablePermits()).toBe(3);
    });
  });

  describe('reset', () => {
    test('should reset bulkhead state', async () => {
      const bulkhead = new Bulkhead({ maxConcurrent: 1 });
      bulkhead.execute(() => new Promise<void>(resolve => setTimeout(resolve, 100)));
      bulkhead.reset();
      expect(bulkhead.hasCapacity()).toBe(true);
    });
  });
});

describe('BulkheadManager', () => {
  test('should create, retrieve bulkheads', () => {
    const manager = new BulkheadManager();
    const bulkhead1 = manager.getBulkhead('api');
    const bulkhead2 = manager.getBulkhead('database');

    expect(bulkhead1).toBeDefined();
    expect(bulkhead2).toBeDefined();
    expect(manager.getNames()).toEqual(expect.arrayContaining(['api', 'database']));
  });

  test('should clear all bulkheads', () => {
    const manager = new BulkheadManager();
    manager.getBulkhead('api');
    manager.getBulkhead('database');
    manager.clear();

    expect(manager.getNames()).toHaveLength(0);
  });
});

describe('Pre-configured bulkheads', () => {
  test('apiBulkhead should have correct config', () => {
    const bulkhead = apiBulkhead();
    const stats = bulkhead.getStats();
    expect(stats.available).toBe(25);
  });

  test('databaseBulkhead should have correct config', () => {
    const bulkhead = databaseBulkhead();
    const stats = bulkhead.getStats();
    expect(stats.available).toBe(10);
  });

  test('strictBulkhead should have correct config', () => {
    const bulkhead = strictBulkhead();
    const stats = bulkhead.getStats();
    expect(stats.available).toBe(5);
  });
});

describe('Error classes', () => {
  test('BulkheadRejectedError should have correct properties', () => {
    const error = new BulkheadRejectedError('test', 5, 10);
    expect(error.name).toBe('BulkheadRejectedError');
    expect(error.active).toBe(5);
    expect(error.maxConcurrent).toBe(10);
    expect(error.message).toContain('5/10');
  });

  test('BulkheadTimeoutError should have correct properties', () => {
    const error = new BulkheadTimeoutError('test', 1000);
    expect(error.name).toBe('BulkheadTimeoutError');
    expect(error.queueTime).toBe(1000);
    expect(error.message).toContain('1000ms');
  });
});
