/**
 * Health Check Implementation
 * 
 * Provides standardized health check functionality for actors
 * compatible with Kubernetes liveness/readiness probes.
 * 
 * @module @aether/sdk/resilience
 */

// ============================================
// Types
// ============================================

export type HealthStatus = 'healthy' | 'degraded' | 'unhealthy';

export interface HealthCheckResult {
    /** Overall status */
    status: HealthStatus;
    /** Component identifier */
    componentId: string;
    /** Component type (actor, database, cache, etc.) */
    componentType: string;
    /** Observed value (e.g., latency, connections) */
    observedValue?: any;
    /** Unit of the observed value */
    observedUnit?: string;
    /** Additional output message */
    output?: string;
    /** Time of check */
    time: string;
    /** Additional details */
    details?: Record<string, any>;
}

export interface HealthReport {
    /** Overall status */
    status: HealthStatus;
    /** Version of the service */
    version: string;
    /** Service identifier */
    serviceId: string;
    /** Time of report */
    time: string;
    /** Individual check results */
    checks: Record<string, HealthCheckResult>;
    /** Uptime in seconds */
    uptime: number;
}

export type HealthCheckFn = () => Promise<HealthCheckResult> | HealthCheckResult;

export interface HealthCheckOptions {
    /** Timeout for the check in ms */
    timeout?: number;
    /** Critical check (failure = unhealthy) */
    critical?: boolean;
    /** Interval to run check (0 = on demand only) */
    interval?: number;
    /** Cache duration for result in ms */
    cacheDuration?: number;
}

// ============================================
// Health Check Implementation
// ============================================

export class HealthChecker {
    private checks: Map<string, {
        fn: HealthCheckFn;
        options: Required<HealthCheckOptions>;
        lastResult?: HealthCheckResult;
        lastRun?: number;
        intervalId?: ReturnType<typeof setInterval>;
    }> = new Map();

    private readonly serviceId: string;
    private readonly version: string;
    private readonly startTime: number = Date.now();

    constructor(serviceId: string = 'aether-actor', version: string = '1.0.0') {
        this.serviceId = serviceId;
        this.version = version;
    }

    /**
     * Register a health check
     */
    registerCheck(
        name: string,
        fn: HealthCheckFn,
        options: HealthCheckOptions = {}
    ): this {
        const resolvedOptions: Required<HealthCheckOptions> = {
            timeout: options.timeout ?? 5000,
            critical: options.critical ?? false,
            interval: options.interval ?? 0,
            cacheDuration: options.cacheDuration ?? 0,
        };

        const entry = {
            fn,
            options: resolvedOptions,
        };

        this.checks.set(name, entry);

        // Set up interval if specified
        if (resolvedOptions.interval > 0) {
            entry.intervalId = setInterval(async () => {
                try {
                    entry.lastResult = await this.runCheck(name);
                    entry.lastRun = Date.now();
                } catch (error) {
                    entry.lastResult = {
                        status: 'unhealthy',
                        componentId: name,
                        componentType: 'check',
                        output: error instanceof Error ? error.message : 'Check failed',
                        time: new Date().toISOString(),
                    };
                    entry.lastRun = Date.now();
                }
            }, resolvedOptions.interval);
        }

        return this;
    }

    /**
     * Unregister a health check
     */
    unregisterCheck(name: string): this {
        const entry = this.checks.get(name);
        if (entry?.intervalId) {
            clearInterval(entry.intervalId);
        }
        this.checks.delete(name);
        return this;
    }

    /**
     * Run a single health check
     */
    async runCheck(name: string): Promise<HealthCheckResult> {
        const entry = this.checks.get(name);
        if (!entry) {
            return {
                status: 'unhealthy',
                componentId: name,
                componentType: 'check',
                output: 'Check not found',
                time: new Date().toISOString(),
            };
        }

        // Return cached result if still valid
        if (entry.options.cacheDuration > 0 && entry.lastResult && entry.lastRun) {
            if (Date.now() - entry.lastRun < entry.options.cacheDuration) {
                return entry.lastResult;
            }
        }

        // Run check with timeout
        try {
            const result = await this.withTimeout(
                entry.fn(),
                entry.options.timeout
            );
            entry.lastResult = result;
            entry.lastRun = Date.now();
            return result;
        } catch (error) {
            const result: HealthCheckResult = {
                status: 'unhealthy',
                componentId: name,
                componentType: 'check',
                output: error instanceof Error ? error.message : 'Check failed',
                time: new Date().toISOString(),
            };
            entry.lastResult = result;
            entry.lastRun = Date.now();
            return result;
        }
    }

    /**
     * Run all health checks and generate report
     */
    async runAll(): Promise<HealthReport> {
        const checkResults: Record<string, HealthCheckResult> = {};

        for (const [name] of this.checks) {
            checkResults[name] = await this.runCheck(name);
        }

        const status = this.calculateOverallStatus(checkResults);

        return {
            status,
            version: this.version,
            serviceId: this.serviceId,
            time: new Date().toISOString(),
            checks: checkResults,
            uptime: Math.floor((Date.now() - this.startTime) / 1000),
        };
    }

    /**
     * Get liveness status (is the service alive?)
     */
    async getLiveness(): Promise<{ alive: boolean; time: string }> {
        return {
            alive: true,
            time: new Date().toISOString(),
        };
    }

    /**
     * Get readiness status (is the service ready to accept traffic?)
     */
    async getReadiness(): Promise<{ ready: boolean; time: string; checks?: Record<string, boolean> }> {
        const report = await this.runAll();
        
        const checks: Record<string, boolean> = {};
        for (const [name, result] of Object.entries(report.checks)) {
            const entry = this.checks.get(name);
            // Non-critical checks don't affect readiness
            if (entry?.options.critical) {
                checks[name] = result.status !== 'unhealthy';
            }
        }

        const ready = report.status !== 'unhealthy';

        return {
            ready,
            time: report.time,
            checks: Object.keys(checks).length > 0 ? checks : undefined,
        };
    }

    /**
     * Get startup status (has the service started?)
     */
    async getStartup(): Promise<{ started: boolean; time: string }> {
        return {
            started: true,
            time: new Date().toISOString(),
        };
    }

    /**
     * Clean up all interval-based checks
     */
    shutdown(): void {
        for (const entry of this.checks.values()) {
            if (entry.intervalId) {
                clearInterval(entry.intervalId);
            }
        }
        this.checks.clear();
    }

    // ============================================
    // Private Methods
    // ============================================

    private calculateOverallStatus(checks: Record<string, HealthCheckResult>): HealthStatus {
        let hasDegraded = false;
        let hasUnhealthy = false;

        for (const [name, result] of Object.entries(checks)) {
            const entry = this.checks.get(name);
            
            if (result.status === 'unhealthy') {
                if (entry?.options.critical) {
                    return 'unhealthy';
                }
                hasUnhealthy = true;
            } else if (result.status === 'degraded') {
                hasDegraded = true;
            }
        }

        if (hasUnhealthy || hasDegraded) {
            return 'degraded';
        }

        return 'healthy';
    }

    private async withTimeout<T>(promise: Promise<T> | T, timeoutMs: number): Promise<T> {
        if (!(promise instanceof Promise)) {
            return promise;
        }

        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                reject(new Error(`Health check timed out after ${timeoutMs}ms`));
            }, timeoutMs);

            promise
                .then(result => {
                    clearTimeout(timer);
                    resolve(result);
                })
                .catch(error => {
                    clearTimeout(timer);
                    reject(error);
                });
        });
    }
}

// ============================================
// Predefined Health Checks
// ============================================

/**
 * Create a simple ping health check
 */
export function pingHealthCheck(): HealthCheckFn {
    return () => ({
        status: 'healthy',
        componentId: 'ping',
        componentType: 'self',
        observedValue: 1,
        observedUnit: 'ms',
        time: new Date().toISOString(),
    });
}

/**
 * Create a memory health check
 */
export function memoryHealthCheck(
    maxHeapMB: number = 1024,
    warnThreshold: number = 0.8
): HealthCheckFn {
    return () => {
        const mem = process.memoryUsage();
        const heapUsedMB = mem.heapUsed / (1024 * 1024);
        const heapTotalMB = mem.heapTotal / (1024 * 1024);
        const usage = heapUsedMB / heapTotalMB;

        let status: HealthStatus;
        if (heapUsedMB > maxHeapMB || usage > 0.95) {
            status = 'unhealthy';
        } else if (usage > warnThreshold) {
            status = 'degraded';
        } else {
            status = 'healthy';
        }

        return {
            status,
            componentId: 'memory',
            componentType: 'system',
            observedValue: Math.round(heapUsedMB),
            observedUnit: 'MB',
            output: `Heap usage: ${Math.round(heapUsedMB)}MB / ${Math.round(heapTotalMB)}MB (${Math.round(usage * 100)}%)`,
            time: new Date().toISOString(),
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
 * Create a CPU health check
 */
export function cpuHealthCheck(warnThreshold: number = 0.8): HealthCheckFn {
    let lastCpuUsage = process.cpuUsage();
    let lastTime = Date.now();

    return () => {
        const currentTime = Date.now();
        const currentCpuUsage = process.cpuUsage();
        
        const elapsedMs = currentTime - lastTime;
        const userDiff = currentCpuUsage.user - lastCpuUsage.user;
        const systemDiff = currentCpuUsage.system - lastCpuUsage.system;
        
        const totalCpuMicros = userDiff + systemDiff;
        const cpuUsage = totalCpuMicros / (elapsedMs * 1000); // Convert to percentage

        lastCpuUsage = currentCpuUsage;
        lastTime = currentTime;

        let status: HealthStatus;
        if (cpuUsage > 0.95) {
            status = 'unhealthy';
        } else if (cpuUsage > warnThreshold) {
            status = 'degraded';
        } else {
            status = 'healthy';
        }

        return {
            status,
            componentId: 'cpu',
            componentType: 'system',
            observedValue: Math.round(cpuUsage * 100),
            observedUnit: '%',
            output: `CPU usage: ${Math.round(cpuUsage * 100)}%`,
            time: new Date().toISOString(),
            details: {
                user: currentCpuUsage.user,
                system: currentCpuUsage.system,
            },
        };
    };
}

/**
 * Create a state storage health check
 */
export function stateHealthCheck(
    stateKey: string,
    readFn: (key: string) => Promise<boolean>
): HealthCheckFn {
    return async () => {
        const start = Date.now();
        try {
            const exists = await readFn(stateKey);
            const latency = Date.now() - start;

            let status: HealthStatus;
            if (latency > 1000) {
                status = 'degraded';
            } else {
                status = 'healthy';
            }

            return {
                status,
                componentId: 'state-storage',
                componentType: 'storage',
                observedValue: latency,
                observedUnit: 'ms',
                output: `State storage ${exists ? 'accessible' : 'empty'}`,
                time: new Date().toISOString(),
            };
        } catch (error) {
            return {
                status: 'unhealthy',
                componentId: 'state-storage',
                componentType: 'storage',
                output: error instanceof Error ? error.message : 'State check failed',
                time: new Date().toISOString(),
            };
        }
    };
}

/**
 * Create an async dependency health check
 */
export function dependencyHealthCheck(
    name: string,
    checkFn: () => Promise<boolean>,
    timeoutMs: number = 5000
): HealthCheckFn {
    return async () => {
        const start = Date.now();
        try {
            const result = await Promise.race([
                checkFn(),
                new Promise<boolean>((_, reject) =>
                    setTimeout(() => reject(new Error('Timeout')), timeoutMs)
                ),
            ]);

            const latency = Date.now() - start;
            const status: HealthStatus = result ? 'healthy' : 'unhealthy';

            return {
                status,
                componentId: name,
                componentType: 'dependency',
                observedValue: latency,
                observedUnit: 'ms',
                time: new Date().toISOString(),
            };
        } catch (error) {
            return {
                status: 'unhealthy',
                componentId: name,
                componentType: 'dependency',
                output: error instanceof Error ? error.message : 'Dependency check failed',
                time: new Date().toISOString(),
            };
        }
    };
}
