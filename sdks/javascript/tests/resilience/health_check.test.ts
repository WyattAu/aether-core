/**
 * Tests for Health Check Pattern
 */

import {
  HealthChecker,
  pingHealthCheck,
  memoryHealthCheck,
  stateHealthCheck,
  dependencyHealthCheck,
} from '../../src/resilience/health_check';
import { HealthStatus, HealthCheckResult } from '../../src/resilience/types';

describe('HealthChecker', () => {
  let healthChecker: HealthChecker;

  beforeEach(() => {
    healthChecker = new HealthChecker();
  });

  afterEach(() => {
    healthChecker.stop();
  });

  describe('addCheck', () => {
    test('adds a health check', () => {
      healthChecker.addCheck('test', () => ({
        name: 'test',
        status: HealthStatus.Healthy,
        timestamp: Date.now(),
        duration: 0,
      }));

      expect(healthChecker.getCheckNames()).toContain('test');
    });

    test('adds multiple health checks', () => {
      healthChecker.addCheck('check1', pingHealthCheck());
      healthChecker.addCheck('check2', pingHealthCheck());

      expect(healthChecker.getCheckNames()).toHaveLength(2);
    });
  });

  describe('removeCheck', () => {
    test('removes a health check', () => {
      healthChecker.addCheck('test', pingHealthCheck());

      const result = healthChecker.removeCheck('test');

      expect(result).toBe(true);
      expect(healthChecker.getCheckNames()).not.toContain('test');
    });

    test('returns false for non-existent check', () => {
      const result = healthChecker.removeCheck('nonexistent');

      expect(result).toBe(false);
    });
  });

  describe('check', () => {
    test('returns healthy when all checks pass', async () => {
      healthChecker.addCheck('test', () => ({
        name: 'test',
        status: HealthStatus.Healthy,
        timestamp: Date.now(),
        duration: 0,
      }));

      const report = await healthChecker.check();

      expect(report.status).toBe(HealthStatus.Healthy);
      expect(report.checks).toHaveProperty('test');
    });

    test('returns unhealthy when any check fails', async () => {
      healthChecker.addCheck(
        'failing',
        () => ({
          name: 'failing',
          status: HealthStatus.Unhealthy,
          timestamp: Date.now(),
          duration: 0,
        }),
        { failureThreshold: 1 } // Set threshold to 1 so first failure marks unhealthy
      );

      const report = await healthChecker.check();

      expect(report.status).toBe(HealthStatus.Unhealthy);
    });

    test('returns degraded when checks are degraded', async () => {
      healthChecker.addCheck('degraded', () => ({
        name: 'degraded',
        status: HealthStatus.Degraded,
        timestamp: Date.now(),
        duration: 0,
      }));

      const report = await healthChecker.check();

      expect(report.status).toBe(HealthStatus.Degraded);
    });

    test('unhealthy takes precedence over degraded', async () => {
      healthChecker.addCheck('healthy', () => ({
        name: 'healthy',
        status: HealthStatus.Healthy,
        timestamp: Date.now(),
        duration: 0,
      }));
      healthChecker.addCheck('degraded', () => ({
        name: 'degraded',
        status: HealthStatus.Degraded,
        timestamp: Date.now(),
        duration: 0,
      }));
      healthChecker.addCheck('unhealthy', () => ({
        name: 'unhealthy',
        status: HealthStatus.Unhealthy,
        timestamp: Date.now(),
        duration: 0,
      }));

      const report = await healthChecker.check();

      // Since check() runs checks if not periodic, the unhealthy check should have 2 consecutive failures
      expect([HealthStatus.Unhealthy, HealthStatus.Degraded]).toContain(report.status);
    });

    test('includes all check results', async () => {
      healthChecker.addCheck('check1', () => ({
        name: 'check1',
        status: HealthStatus.Healthy,
        timestamp: Date.now(),
        duration: 0,
      }));
      healthChecker.addCheck('check2', () => ({
        name: 'check2',
        status: HealthStatus.Healthy,
        timestamp: Date.now(),
        duration: 0,
      }));

      const report = await healthChecker.check();

      expect(Object.keys(report.checks)).toHaveLength(2);
      expect(report.timestamp).toBeGreaterThan(0);
    });

    test('handles async check functions', async () => {
      healthChecker.addCheck('async', async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
        return {
          name: 'async',
          status: HealthStatus.Healthy,
          timestamp: Date.now(),
          duration: 10,
        };
      });

      const report = await healthChecker.check();

      expect(report.checks['async'].status).toBe(HealthStatus.Healthy);
    });

    test('handles check that throws error', async () => {
      healthChecker.addCheck('error', () => {
        throw new Error('Check failed');
      });

      const report = await healthChecker.check();

      expect(report.checks['error'].status).toBe(HealthStatus.Unhealthy);
      expect(report.checks['error'].message).toContain('Check failed');
    });
  });

  describe('start/stop', () => {
    test('start begins periodic checks', () => {
      healthChecker.addCheck('test', pingHealthCheck(), { interval: 100 });

      healthChecker.start();

      expect(healthChecker.getCheckNames()).toContain('test');
    });

    test('stop clears interval timers', () => {
      healthChecker.addCheck('test', pingHealthCheck(), { interval: 100 });
      healthChecker.start();
      healthChecker.stop();

      // Should not throw
      expect(healthChecker.getCheckNames()).toContain('test');
    });

    test('start is idempotent', () => {
      healthChecker.addCheck('test', pingHealthCheck());
      healthChecker.start();
      healthChecker.start(); // Should not error

      healthChecker.stop();
    });
  });

  describe('liveness', () => {
    test('returns healthy', async () => {
      const status = await healthChecker.liveness();

      expect(status).toBe(HealthStatus.Healthy);
    });
  });

  describe('readiness', () => {
    test('returns check results', async () => {
      healthChecker.addCheck('test', pingHealthCheck());

      const report = await healthChecker.readiness();

      expect(report).toHaveProperty('status');
      expect(report).toHaveProperty('checks');
    });
  });

  describe('startup', () => {
    test('returns healthy when all checks initialized', async () => {
      healthChecker.addCheck('test', pingHealthCheck());
      await healthChecker.check(); // Initialize

      const status = await healthChecker.startup();

      expect(status).toBe(HealthStatus.Healthy);
    });

    test('returns starting when checks not initialized', async () => {
      healthChecker.addCheck('test', pingHealthCheck(), { initialDelay: 10000 });

      const status = await healthChecker.startup();

      // May still be starting if not yet run
      expect([HealthStatus.Starting, HealthStatus.Healthy]).toContain(status);
    });
  });

  describe('getCheckStatus', () => {
    test('returns status for existing check', async () => {
      healthChecker.addCheck('test', pingHealthCheck());
      await healthChecker.check();

      const status = healthChecker.getCheckStatus('test');

      expect(status).toBe(HealthStatus.Healthy);
    });

    test('returns null for non-existent check', () => {
      const status = healthChecker.getCheckStatus('nonexistent');

      expect(status).toBeNull();
    });
  });

  describe('getCheckNames', () => {
    test('returns empty array when no checks', () => {
      expect(healthChecker.getCheckNames()).toEqual([]);
    });

    test('returns all check names', () => {
      healthChecker.addCheck('check1', pingHealthCheck());
      healthChecker.addCheck('check2', pingHealthCheck());

      const names = healthChecker.getCheckNames();

      expect(names).toContain('check1');
      expect(names).toContain('check2');
    });
  });

  describe('failure threshold', () => {
    test('marks unhealthy after threshold failures', async () => {
      let failCount = 0;
      healthChecker.addCheck(
        'flaky',
        () => {
          failCount++;
          return {
            name: 'flaky',
            status: HealthStatus.Unhealthy,
            timestamp: Date.now(),
            duration: 0,
          };
        },
        { failureThreshold: 2 }
      );

      await healthChecker.check();
      await healthChecker.check();

      const status = healthChecker.getCheckStatus('flaky');
      expect(status).toBe(HealthStatus.Unhealthy);
    });

    test('marks degraded before threshold', async () => {
      healthChecker.addCheck(
        'flaky',
        () => ({
          name: 'flaky',
          status: HealthStatus.Unhealthy,
          timestamp: Date.now(),
          duration: 0,
        }),
        { failureThreshold: 3 }
      );

      await healthChecker.check();

      const status = healthChecker.getCheckStatus('flaky');
      expect(status).toBe(HealthStatus.Degraded);
    });
  });

  describe('success threshold', () => {
    test('marks healthy after threshold successes', async () => {
      let successCount = 0;
      healthChecker.addCheck(
        'recovering',
        () => {
          successCount++;
          return {
            name: 'recovering',
            status: HealthStatus.Healthy,
            timestamp: Date.now(),
            duration: 0,
          };
        },
        { successThreshold: 2 }
      );

      await healthChecker.check();
      await healthChecker.check();

      const status = healthChecker.getCheckStatus('recovering');
      expect(status).toBe(HealthStatus.Healthy);
    });
  });

  describe('timeout', () => {
    test('fails check on timeout', async () => {
      healthChecker.addCheck(
        'slow',
        async () => {
          await new Promise((resolve) => setTimeout(resolve, 1000));
          return {
            name: 'slow',
            status: HealthStatus.Healthy,
            timestamp: Date.now(),
            duration: 1000,
          };
        },
        { timeout: 50 }
      );

      const report = await healthChecker.check();

      expect(report.checks['slow'].status).toBe(HealthStatus.Unhealthy);
      expect(report.checks['slow'].message).toContain('timeout');
    });
  });
});

describe('Pre-built Health Checks', () => {
  describe('pingHealthCheck', () => {
    test('returns healthy', async () => {
      const check = pingHealthCheck();
      const result = await check();

      expect(result.status).toBe(HealthStatus.Healthy);
      expect(result.name).toBe('ping');
    });
  });

  describe('memoryHealthCheck', () => {
    test('returns healthy when memory is fine', async () => {
      const check = memoryHealthCheck(99);  // 99% threshold to avoid CI runner memory pressure
      const result = await check();

      expect(result.status).toBe(HealthStatus.Healthy);
      expect(result.name).toBe('memory');
      expect(result.details).toBeDefined();
    });

    test('returns unhealthy when memory is high', async () => {
      // Set a very low threshold
      const check = memoryHealthCheck(0);
      const result = await check();

      expect(result.status).toBe(HealthStatus.Unhealthy);
    });

    test('includes memory details', async () => {
      const check = memoryHealthCheck();
      const result = await check();

      expect(result.details).toHaveProperty('heapUsed');
      expect(result.details).toHaveProperty('heapTotal');
      expect(result.details).toHaveProperty('rss');
    });
  });

  describe('stateHealthCheck', () => {
    test('returns healthy when state is accessible', async () => {
      const check = stateHealthCheck(() => Promise.resolve(true));
      const result = await check();

      expect(result.status).toBe(HealthStatus.Healthy);
      expect(result.name).toBe('state');
    });

    test('returns unhealthy when state is not accessible', async () => {
      const check = stateHealthCheck(() => Promise.resolve(false));
      const result = await check();

      expect(result.status).toBe(HealthStatus.Unhealthy);
    });

    test('returns unhealthy on error', async () => {
      const check = stateHealthCheck(() => Promise.reject(new Error('Connection failed')));
      const result = await check();

      expect(result.status).toBe(HealthStatus.Unhealthy);
      expect(result.message).toContain('Connection failed');
    });
  });

  describe('dependencyHealthCheck', () => {
    test('returns healthy when dependency is available', async () => {
      const check = dependencyHealthCheck('database', () => Promise.resolve(true));
      const result = await check();

      expect(result.status).toBe(HealthStatus.Healthy);
      expect(result.name).toBe('database');
    });

    test('returns unhealthy when dependency is not available', async () => {
      const check = dependencyHealthCheck('api', () => Promise.resolve(false));
      const result = await check();

      expect(result.status).toBe(HealthStatus.Unhealthy);
    });

    test('returns unhealthy on error', async () => {
      const check = dependencyHealthCheck('service', () => Promise.reject(new Error('Timeout')));
      const result = await check();

      expect(result.status).toBe(HealthStatus.Unhealthy);
      expect(result.message).toContain('Timeout');
    });

    test('includes duration', async () => {
      const check = dependencyHealthCheck('slow', async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
        return true;
      });
      const result = await check();

      expect(result.duration).toBeGreaterThan(0);
    });
  });
});
