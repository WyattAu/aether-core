/**
 * Health Check Pattern Implementation.
 * Kubernetes-compatible health probes for monitoring service health.
 * @module aether/resilience/health_check
 */

import {
  HealthStatus,
  HealthCheckConfig,
  HealthCheckResult,
  HealthReport,
  HealthCheckFn,
} from './types';
import { withTracing } from './tracing';

/**
 * Default health check configuration.
 */
const DEFAULT_CONFIG: HealthCheckConfig = {
  name: 'default',
  interval: 30000,
  timeout: 5000,
  failureThreshold: 3,
  successThreshold: 1,
  initialDelay: 0,
};

/**
 * Internal health check state.
 */
interface HealthCheckState {
  config: HealthCheckConfig;
  check: HealthCheckFn;
  consecutiveFailures: number;
  consecutiveSuccesses: number;
  lastResult: HealthCheckResult | null;
  status: HealthStatus;
  running: boolean;
  intervalId: NodeJS.Timeout | null;
}

/**
 * Health Checker implementation.
 *
 * Supports three types of probes (Kubernetes-compatible):
 * - Liveness: Is the service running?
 * - Readiness: Is the service ready to accept traffic?
 * - Startup: Has the service finished starting?
 *
 * @example
 * ```typescript
 * const health = new HealthChecker();
 *
 * health.addCheck('database', async () => {
 *   const connected = await db.ping();
 *   return {
 *     name: 'database',
 *     status: connected ? HealthStatus.Healthy : HealthStatus.Unhealthy,
 *     timestamp: Date.now(),
 *     duration: 0,
 *   };
 * });
 *
 * // Get health report
 * const report = await health.check();
 * ```
 */
export class HealthChecker {
  private checks: Map<string, HealthCheckState> = new Map();
  private started = false;

  /**
   * Add a health check.
   *
   * @param name - Check name
   * @param check - Health check function
   * @param config - Optional configuration
   */
  addCheck(
    name: string,
    check: HealthCheckFn,
    config: Partial<HealthCheckConfig> = {}
  ): void {
    const fullConfig: HealthCheckConfig = {
      ...DEFAULT_CONFIG,
      ...config,
      name,
    };

    this.checks.set(name, {
      config: fullConfig,
      check,
      consecutiveFailures: 0,
      consecutiveSuccesses: 0,
      lastResult: null,
      status: HealthStatus.Starting,
      running: false,
      intervalId: null,
    });
  }

  /**
   * Remove a health check.
   */
  removeCheck(name: string): boolean {
    const state = this.checks.get(name);
    if (state?.intervalId) {
      clearInterval(state.intervalId);
    }
    return this.checks.delete(name);
  }

  /**
   * Start periodic health checks.
   */
  start(): void {
    if (this.started) {
      return;
    }

    this.started = true;

    for (const [name, state] of this.checks) {
      // Apply initial delay
      setTimeout(() => {
        this.runCheck(name);
        // Start periodic checks
        state.intervalId = setInterval(
          () => this.runCheck(name),
          state.config.interval
        );
      }, state.config.initialDelay);
    }
  }

  /**
   * Stop all health checks.
   */
  stop(): void {
    this.started = false;

    for (const state of this.checks.values()) {
      if (state.intervalId) {
        clearInterval(state.intervalId);
        state.intervalId = null;
      }
    }
  }

  /**
   * Run a single health check.
   */
  private async runCheck(name: string): Promise<void> {
    const state = this.checks.get(name);
    if (!state) {
      return;
    }

    const startTime = Date.now();
    let result: HealthCheckResult;

    try {
      // Run with timeout
      result = await this.runWithTimeout(
        state.check,
        state.config.timeout
      );
      result.duration = Date.now() - startTime;
    } catch (error) {
      result = {
        name,
        status: HealthStatus.Unhealthy,
        message: error instanceof Error ? error.message : 'Health check failed',
        timestamp: Date.now(),
        duration: Date.now() - startTime,
      };
    }

    // Update state based on result
    this.updateState(state, result);
    state.lastResult = result;
  }

  /**
   * Run a check with timeout.
   */
  private async runWithTimeout(
    check: HealthCheckFn,
    timeout: number
  ): Promise<HealthCheckResult> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error('Health check timeout'));
      }, timeout);

      Promise.resolve(check())
        .then((result) => {
          clearTimeout(timer);
          resolve(result);
        })
        .catch((error) => {
          clearTimeout(timer);
          reject(error);
        });
    });
  }

  /**
   * Update health check state based on result.
   */
  private updateState(
    state: HealthCheckState,
    result: HealthCheckResult
  ): void {
    if (result.status === HealthStatus.Healthy) {
      state.consecutiveSuccesses++;
      state.consecutiveFailures = 0;

      if (state.consecutiveSuccesses >= state.config.successThreshold) {
        state.status = HealthStatus.Healthy;
      }
    } else {
      state.consecutiveFailures++;
      state.consecutiveSuccesses = 0;

      if (state.consecutiveFailures >= state.config.failureThreshold) {
        state.status = HealthStatus.Unhealthy;
      } else {
        state.status = HealthStatus.Degraded;
      }
    }
  }

  /**
   * Run all health checks and return a report.
   */
  async check(): Promise<HealthReport> {
    return withTracing('health_check.all', async (span) => {
      const checks: Record<string, HealthCheckResult> = {};
      let overallStatus = HealthStatus.Healthy;

      for (const [name, state] of this.checks) {
        // Run check if not running periodically
        if (!state.intervalId) {
          await this.runCheck(name);
        }

        if (state.lastResult) {
          checks[name] = state.lastResult;
        }

        // Determine overall status
        if (state.status === HealthStatus.Unhealthy) {
          overallStatus = HealthStatus.Unhealthy;
        } else if (
          state.status === HealthStatus.Degraded &&
          overallStatus !== HealthStatus.Unhealthy
        ) {
          overallStatus = HealthStatus.Degraded;
        }
      }

      span.setAttribute('health_check.overall_status', overallStatus);
      span.setAttribute('health_check.total_checks', Object.keys(checks).length);

      return {
        status: overallStatus,
        checks,
        timestamp: Date.now(),
      };
    });
  }

  /**
   * Get liveness status (is the service running?).
   */
  async liveness(): Promise<HealthStatus> {
    // Liveness is simple: if we can respond, we're alive
    return HealthStatus.Healthy;
  }

  /**
   * Get readiness status (is the service ready for traffic?).
   */
  async readiness(): Promise<HealthReport> {
    return this.check();
  }

  /**
   * Get startup status (has the service finished starting?).
   */
  async startup(): Promise<HealthStatus> {
    // Check if all checks have been initialized
    for (const state of this.checks.values()) {
      if (state.status === HealthStatus.Starting) {
        return HealthStatus.Starting;
      }
    }
    return HealthStatus.Healthy;
  }

  /**
   * Get current status of a specific check.
   */
  getCheckStatus(name: string): HealthStatus | null {
    return this.checks.get(name)?.status ?? null;
  }

  /**
   * Get all check names.
   */
  getCheckNames(): string[] {
    return Array.from(this.checks.keys());
  }
}

// ============================================
// Pre-built Health Checks
// ============================================

/**
 * Simple ping health check.
 */
export function pingHealthCheck(): HealthCheckFn {
  return () => ({
    name: 'ping',
    status: HealthStatus.Healthy,
    timestamp: Date.now(),
    duration: 0,
  });
}

/**
 * Memory health check.
 */
export function memoryHealthCheck(
  maxHeapUsedPercent = 90
): HealthCheckFn {
  return () => {
    const mem = process.memoryUsage();
    const heapUsedPercent = (mem.heapUsed / mem.heapTotal) * 100;

    return {
      name: 'memory',
      status:
        heapUsedPercent > maxHeapUsedPercent
          ? HealthStatus.Unhealthy
          : HealthStatus.Healthy,
      message: `Heap usage: ${heapUsedPercent.toFixed(1)}%`,
      timestamp: Date.now(),
      duration: 0,
      details: {
        heapUsed: mem.heapUsed,
        heapTotal: mem.heapTotal,
        rss: mem.rss,
        external: mem.external,
      },
    };
  };
}

/**
 * State store health check (placeholder).
 */
export function stateHealthCheck(
  getState: () => Promise<boolean>
): HealthCheckFn {
  return async () => {
    const startTime = Date.now();
    try {
      const healthy = await getState();
      return {
        name: 'state',
        status: healthy ? HealthStatus.Healthy : HealthStatus.Unhealthy,
        timestamp: Date.now(),
        duration: Date.now() - startTime,
      };
    } catch (error) {
      return {
        name: 'state',
        status: HealthStatus.Unhealthy,
        message: error instanceof Error ? error.message : 'State check failed',
        timestamp: Date.now(),
        duration: Date.now() - startTime,
      };
    }
  };
}

/**
 * Dependency health check.
 */
export function dependencyHealthCheck(
  name: string,
  check: () => Promise<boolean>
): HealthCheckFn {
  return async () => {
    const startTime = Date.now();
    try {
      const healthy = await check();
      return {
        name,
        status: healthy ? HealthStatus.Healthy : HealthStatus.Unhealthy,
        timestamp: Date.now(),
        duration: Date.now() - startTime,
      };
    } catch (error) {
      return {
        name,
        status: HealthStatus.Unhealthy,
        message: error instanceof Error ? error.message : 'Dependency check failed',
        timestamp: Date.now(),
        duration: Date.now() - startTime,
      };
    }
  };
}
