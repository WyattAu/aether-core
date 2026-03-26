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
 * @internal
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
 * Manages named health checks and supports three Kubernetes-compatible probe types:
 *
 * - **Liveness** — Is the service running? (always returns Healthy if reachable).
 * - **Readiness** — Is the service ready to accept traffic? (runs all checks).
 * - **Startup** — Has the service finished starting? (checks for Starting state).
 *
 * Individual checks can have their own intervals, timeouts, and failure/success
 * thresholds. The overall status is the worst status among all checks.
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
 * console.log(report.status); // 'healthy' | 'unhealthy' | 'degraded'
 * ```
 */
export class HealthChecker {
  private checks: Map<string, HealthCheckState> = new Map();
  private started = false;

  /**
   * Add a named health check.
   *
   * @param name   - Unique check name.
   * @param check  - Async or sync function returning a {@link HealthCheckResult}.
   * @param config - Optional per-check configuration overrides.
   *
   * @example
   * ```typescript
   * health.addCheck('redis', async () => ({
   *   name: 'redis',
   *   status: (await redis.ping()) ? HealthStatus.Healthy : HealthStatus.Unhealthy,
   *   timestamp: Date.now(),
   *   duration: 0,
   * }), { timeout: 2000, interval: 10000 });
   * ```
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
   * Remove a health check by name.
   *
   * Stops the periodic timer if the check is running.
   *
   * @param name - The check name to remove.
   * @returns `true` if the check was found and removed.
   */
  removeCheck(name: string): boolean {
    const state = this.checks.get(name);
    if (state?.intervalId) {
      clearInterval(state.intervalId);
    }
    return this.checks.delete(name);
  }

  /**
   * Start periodic health checks for all registered checks.
   *
   * Each check runs on its own interval after an optional initial delay.
   * Calling this method when already started is a no-op.
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
   * Stop all periodic health checks.
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
   *
   * @param name - The check name to execute.
   * @internal
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
   * Run a check with a timeout.
   *
   * @param check   - The health check function.
   * @param timeout - Maximum execution time in ms.
   * @returns The health check result.
   * @throws Error If the check exceeds the timeout.
   * @internal
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
   * Update health check state based on the result.
   *
   * @param state  - The internal check state.
   * @param result - The latest check result.
   * @internal
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
   * Run all health checks and return a comprehensive report.
   *
   * Checks that are not running periodically will be executed on-demand.
   * The overall status is the worst status among all checks.
   *
   * @returns A {@link HealthReport} with overall status and per-check results.
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
   *
   * If this method returns, the service is considered alive.
   *
   * @returns Always returns {@link HealthStatus.Healthy}.
   */
  async liveness(): Promise<HealthStatus> {
    // Liveness is simple: if we can respond, we're alive
    return HealthStatus.Healthy;
  }

  /**
   * Get readiness status (is the service ready for traffic?).
   *
   * Delegates to {@link check} and returns the full health report.
   *
   * @returns A {@link HealthReport} with overall status and per-check details.
   */
  async readiness(): Promise<HealthReport> {
    return this.check();
  }

  /**
   * Get startup status (has the service finished starting?).
   *
   * Returns `Starting` if any check has not yet completed its first
   * successful run; otherwise returns `Healthy`.
   *
   * @returns The current startup status.
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
   * Get the current status of a specific check.
   *
   * @param name - The check name.
   * @returns The check's {@link HealthStatus}, or `null` if not found.
   */
  getCheckStatus(name: string): HealthStatus | null {
    return this.checks.get(name)?.status ?? null;
  }

  /**
   * Get all registered check names.
   *
   * @returns An array of check names.
   */
  getCheckNames(): string[] {
    return Array.from(this.checks.keys());
  }
}

// ============================================
// Pre-built Health Checks
// ============================================

/**
 * Create a simple ping health check that always reports healthy.
 *
 * @returns A {@link HealthCheckFn} suitable for basic liveness probes.
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
 * Create a memory health check that monitors heap usage.
 *
 * Reports `Unhealthy` when the heap used percentage exceeds the threshold.
 *
 * @param maxHeapUsedPercent - Heap usage percentage threshold (default: 90).
 * @returns A {@link HealthCheckFn} that reports memory health.
 *
 * @example
 * ```typescript
 * health.addCheck('memory', memoryHealthCheck(85));
 * ```
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
 * Create a state store health check.
 *
 * Calls the provided `getState` function and reports Healthy if it returns `true`.
 *
 * @param getState - Async function returning `true` if the state store is healthy.
 * @returns A {@link HealthCheckFn} that reports state store health.
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
 * Create a generic dependency health check.
 *
 * @param name  - The dependency name (used in the result).
 * @param check - Async function returning `true` if the dependency is healthy.
 * @returns A {@link HealthCheckFn} that reports dependency health.
 *
 * @example
 * ```typescript
 * health.addCheck('redis', dependencyHealthCheck('redis', async () => {
 *   await redis.ping();
 *   return true;
 * }));
 * ```
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
