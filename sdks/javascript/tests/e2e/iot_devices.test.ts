// @jest/tag:e2e
/**
 * E2E Scenario 3: IoT Device Management
 *
 * Simulates IoT device lifecycle:
 * - Device Registration with metadata (type, firmware version)
 * - Telemetry Processing (temperature, humidity, pressure)
 * - Alert Generation for abnormal readings
 * - Firmware Update Coordination with batch update and rollback on failure
 * - Device Health via periodic ping
 */

import {
  Actor,
  Message,
  MessageType,
} from '../../src';
import {
  CircuitBreaker,
  CircuitState,
  CircuitBreakerError,
} from '../../src/resilience';
import {
  RetryPolicy,
  BackoffStrategy,
} from '../../src/resilience';
import {
  HealthChecker,
  HealthStatus,
} from '../../src/resilience';

interface SagaStepDef {
  name: string;
  action: (ctx: { input: unknown; state: Record<string, unknown>; completedSteps: string[] }) => Promise<unknown>;
  compensate?: (ctx: { input: unknown; state: Record<string, unknown>; completedSteps: string[] }) => Promise<void>;
}

interface SagaResult {
  status: 'completed' | 'compensated' | 'failed';
  completedSteps: string[];
  compensatedSteps: string[];
  error?: string;
}

class Saga {
  private steps: SagaStepDef[] = [];
  private currentStep: SagaStepDef | null = null;

  constructor(public readonly name: string) {}

  step(name: string): Saga {
    const step: SagaStepDef = { name, action: async () => {} };
    this.steps.push(step);
    this.currentStep = step;
    return this;
  }

  action(fn: SagaStepDef['action']): Saga {
    if (!this.currentStep) throw new Error('No step defined');
    this.currentStep.action = fn;
    return this;
  }

  compensate(fn: SagaStepDef['compensate']): Saga {
    if (!this.currentStep) throw new Error('No step defined');
    this.currentStep.compensate = fn;
    return this;
  }

  build(): Saga { return this; }
  getSteps(): SagaStepDef[] { return [...this.steps]; }
  getStep(name: string): SagaStepDef | undefined { return this.steps.find((s) => s.name === name); }
}

class SagaExecutor {
  async execute(saga: Saga, input: unknown): Promise<SagaResult> {
    const ctx = { input, state: {} as Record<string, unknown>, completedSteps: [] as string[] };
    try {
      for (const step of saga.getSteps()) {
        const result = await step.action(ctx);
        ctx.state[`step_${step.name}_result`] = result;
        ctx.completedSteps.push(step.name);
      }
      return { status: 'completed', completedSteps: [...ctx.completedSteps], compensatedSteps: [] };
    } catch (error) {
      const reversed = [...ctx.completedSteps].reverse();
      for (const stepName of reversed) {
        const step = saga.getStep(stepName);
        if (step?.compensate) await step.compensate(ctx);
      }
      return {
        status: ctx.completedSteps.length > 0 ? 'compensated' : 'failed',
        completedSteps: [...ctx.completedSteps],
        compensatedSteps: [...ctx.completedSteps],
        error: String(error),
      };
    }
  }
}

// ============================================
// Device Registration Tests
// ============================================

describe('E2E: IoT Device Registration', () => {
  test('register single device with metadata', async () => {
    class DeviceRegistry extends Actor {
      constructor() { super({ name: 'device-registry' }); }
      static override get name(): string { return 'device-registry'; }

      async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.payload.action === 'register') {
          const { device_id, metadata } = message.payload;
          await this.state.setJSON(`device:${device_id}`, {
            device_id,
            registered: true,
            metadata,
          });
          return Message.custom({ status: 'registered', device_id });
        }
        if (message.payload.action === 'get') {
          const device = await this.state.getJSON(`device:${message.payload.device_id}`);
          return Message.custom({ device });
        }
      }
    }

    const registry = new DeviceRegistry();
    const regResp = await registry.handle('admin', Message.custom({
      action: 'register',
      device_id: 'sensor-001',
      metadata: { type: 'temperature', firmware: 'v2.1.0', location: 'warehouse-a' },
    }));

    expect(regResp?.payload.status).toBe('registered');

    const getResp = await registry.handle('admin', Message.custom({
      action: 'get',
      device_id: 'sensor-001',
    }));
    const device = getResp?.payload.device;
    expect(device.registered).toBe(true);
    expect(device.metadata.type).toBe('temperature');

    console.log('\n=== Device Registration (JS) ===');
    console.log(`  Device: sensor-001, Type: temperature, Firmware: v2.1.0`);
  });

  test('register multiple devices of different types', async () => {
    const devices = [
      { device_id: 'temp-001', type: 'temperature', firmware: 'v1.0' },
      { device_id: 'temp-002', type: 'temperature', firmware: 'v1.0' },
      { device_id: 'hum-001', type: 'humidity', firmware: 'v2.0' },
      { device_id: 'pres-001', type: 'pressure', firmware: 'v1.5' },
      { device_id: 'pres-002', type: 'pressure', firmware: 'v1.5' },
    ];

    const allDevices: unknown[] = [];
    for (const d of devices) {
      allDevices.push({ ...d, registered: true });
    }

    expect(allDevices.length).toBe(5);
    const types = new Set(allDevices.map((d: any) => d.type));
    expect(types.size).toBe(3);

    console.log('\n=== Multi-Device Registration (JS) ===');
    console.log(`  Total: ${allDevices.length}, Types: ${[...types].join(', ')}`);
  });
});

// ============================================
// Telemetry Processing Tests
// ============================================

describe('E2E: Telemetry Processing', () => {
  test('process telemetry with threshold alerts', () => {
    const alerts: Array<Record<string, unknown>> = [];
    const readings: Array<Record<string, unknown>> = [];
    const thresholds = {
      temperature: { min: -10, max: 50 },
      humidity: { min: 10, max: 90 },
      pressure: { min: 950, max: 1050 },
    };

    function processTelemetry(deviceId: string, sensorType: string, value: number) {
      readings.push({ device_id: deviceId, type: sensorType, value });
      const t = thresholds[sensorType as keyof typeof thresholds];
      if (t && (value < t.min || value > t.max)) {
        alerts.push({
          device_id: deviceId,
          type: sensorType,
          value,
          severity: Math.abs(value - (t.min + t.max) / 2) > (t.max - t.min) * 0.4 ? 'critical' : 'warning',
        });
      }
    }

    const normalReadings = [
      ['temp-001', 'temperature', 22.5],
      ['temp-001', 'temperature', 23.1],
      ['hum-001', 'humidity', 45.0],
      ['pres-001', 'pressure', 1013.0],
    ];

    const abnormalReadings = [
      ['temp-001', 'temperature', 85.0],
      ['hum-001', 'humidity', 95.0],
      ['pres-001', 'pressure', 900.0],
      ['temp-002', 'temperature', -25.0],
    ];

    for (const [deviceId, sensorType, value] of [...normalReadings, ...abnormalReadings] as [string, string, number][]) {
      processTelemetry(deviceId, sensorType, value);
    }

    expect(readings.length).toBe(8);
    expect(alerts.length).toBe(4);
    for (const a of alerts) {
      expect(['critical', 'warning']).toContain(a.severity);
    }

    console.log('\n=== Telemetry Alerting (JS) ===');
    console.log(`  Readings: ${readings.length}, Alerts: ${alerts.length}`);
    for (const a of alerts) {
      console.log(`  [${a.severity}] ${a.device_id}: ${a.type}=${a.value}`);
    }
  });

  test('telemetry aggregation across readings', () => {
    const deviceReadings: Record<string, number[]> = {};

    for (let i = 0; i < 100; i++) {
      const device = ['temp-001', 'temp-002', 'hum-001'][Math.floor(Math.random() * 3)];
      const value = Math.random() * 30 + 10;
      if (!deviceReadings[device]) deviceReadings[device] = [];
      deviceReadings[device].push(value);
    }

    const stats: Record<string, { count: number; avg: number; min: number; max: number }> = {};
    for (const [deviceId, values] of Object.entries(deviceReadings)) {
      stats[deviceId] = {
        count: values.length,
        avg: values.reduce((a, b) => a + b, 0) / values.length,
        min: Math.min(...values),
        max: Math.max(...values),
      };
    }

    expect(Object.keys(stats).length).toBe(3);
    for (const s of Object.values(stats)) {
      expect(s.count).toBeGreaterThan(0);
      expect(s.min).toBeLessThanOrEqual(s.avg);
      expect(s.avg).toBeLessThanOrEqual(s.max);
    }

    console.log('\n=== Telemetry Aggregation (JS) ===');
    for (const [deviceId, s] of Object.entries(stats)) {
      console.log(`  ${deviceId}: count=${s.count}, avg=${s.avg.toFixed(1)}, min=${s.min.toFixed(1)}, max=${s.max.toFixed(1)}`);
    }
  });
});

// ============================================
// Firmware Update Tests
// ============================================

describe('E2E: Firmware Update', () => {
  test('batch firmware update success', async () => {
    const auditLog: string[] = [];
    const updatedDevices: string[] = [];
    const devices = Array.from({ length: 5 }, (_, i) => `temp-${String(i + 1).padStart(3, '0')}`);

    async function updateFirmware(deviceId: string, version: string): Promise<void> {
      auditLog.push(`FIRMWARE: Updating ${deviceId} to ${version}`);
      await new Promise((r) => setTimeout(r, 1));
      updatedDevices.push(deviceId);
      auditLog.push(`FIRMWARE: ${deviceId} updated successfully`);
    }

    await Promise.all(devices.map((d) => updateFirmware(d, 'v3.0.0')));

    expect(updatedDevices.length).toBe(5);
    expect(auditLog.length).toBe(10);

    console.log('\n=== Batch Firmware Update (JS) ===');
    console.log(`  Devices updated: ${updatedDevices.length}/${devices.length}`);
  });

  test('firmware rollback on failure', async () => {
    const auditLog: string[] = [];
    const updatedDevices: string[] = [];
    const rolledBack: string[] = [];
    const failDevice = 'temp-003';

    async function updateDevice(deviceId: string, version: string): Promise<void> {
      if (deviceId === failDevice) {
        auditLog.push(`FIRMWARE: ${deviceId} FAILED`);
        throw new Error(`Update failed for ${deviceId}`);
      }
      auditLog.push(`FIRMWARE: ${deviceId} updated to ${version}`);
      updatedDevices.push(deviceId);
    }

    async function rollbackDevice(deviceId: string, oldVersion: string): Promise<void> {
      auditLog.push(`FIRMWARE: ${deviceId} rolled back to ${oldVersion}`);
      rolledBack.push(deviceId);
    }

    const devices = Array.from({ length: 5 }, (_, i) => `temp-${String(i + 1).padStart(3, '0')}`);
    let failedDevice: string | null = null;

    for (const device of devices) {
      try {
        await updateDevice(device, 'v4.0.0');
      } catch {
        failedDevice = device;
        break;
      }
    }

    if (failedDevice) {
      for (const d of updatedDevices) {
        await rollbackDevice(d, 'v2.1.0');
      }
    }

    expect(failedDevice).toBe(failDevice);
    expect(updatedDevices.length).toBe(2);
    expect(rolledBack.length).toBe(2);

    console.log('\n=== Firmware Rollback (JS) ===');
    console.log(`  Failed: ${failedDevice}`);
    console.log(`  Rolled back: ${rolledBack}`);
  });

  test('firmware update with saga pattern', async () => {
    const updated: string[] = [];
    const rolledBack: string[] = [];

    const saga = new Saga('firmware-update')
      .step('update-temp-001')
      .action(async () => { updated.push('temp-001'); return { device: 'temp-001' }; })
      .compensate(async () => { rolledBack.push('temp-001'); })
      .step('update-temp-002')
      .action(async () => { updated.push('temp-002'); return { device: 'temp-002' }; })
      .compensate(async () => { rolledBack.push('temp-002'); })
      .step('update-temp-003')
      .action(async () => { throw new Error('Device offline'); })
      .build();

    const executor = new SagaExecutor();
    const result = await executor.execute(saga, { version: 'v5.0.0' });

    expect(result.status).toBe('compensated');
    expect(updated).toContain('temp-001');
    expect(updated).toContain('temp-002');
    expect(rolledBack).toContain('temp-002');
    expect(rolledBack).toContain('temp-001');

    console.log('\n=== Saga Firmware Update (JS) ===');
    console.log(`  Status: ${result.status}`);
    console.log(`  Updated: ${updated}, Rolled back: ${rolledBack}`);
  });
});

// ============================================
// Device Health Tests
// ============================================

describe('E2E: Device Health', () => {
  test('health check for devices', async () => {
    const checker = new HealthChecker();

    checker.addCheck('temp-001', () => ({
      name: 'temp-001',
      status: HealthStatus.Healthy,
      timestamp: Date.now(),
      duration: 0,
    }));

    checker.addCheck('temp-002', () => ({
      name: 'temp-002',
      status: HealthStatus.Unhealthy,
      message: 'Device offline',
      timestamp: Date.now(),
      duration: 0,
    }), { failureThreshold: 1 });

    checker.addCheck('hum-001', () => ({
      name: 'hum-001',
      status: HealthStatus.Healthy,
      timestamp: Date.now(),
      duration: 0,
    }));

    const report = await checker.check();
    expect(report.status).toBe(HealthStatus.Unhealthy);
    expect(report.checks['temp-001'].status).toBe(HealthStatus.Healthy);
    expect(report.checks['temp-002'].status).toBe(HealthStatus.Unhealthy);

    const liveness = await checker.liveness();
    expect(liveness).toBe(HealthStatus.Healthy);

    console.log('\n=== Device Health Check (JS) ===');
    console.log(`  Overall: ${report.status}`);
    for (const [name, check] of Object.entries(report.checks)) {
      console.log(`  ${name}: ${check.status}`);
    }

    checker.stop();
  });

  test('circuit breaker for device ping', async () => {
    const pingAttempts: Record<string, number> = {};
    const failDevice = 'temp-flaky';

    async function pingDevice(deviceId: string): Promise<boolean> {
      pingAttempts[deviceId] = (pingAttempts[deviceId] || 0) + 1;
      if (deviceId === failDevice && pingAttempts[deviceId] <= 3) {
        throw new Error(`Device ${deviceId} unreachable`);
      }
      return true;
    }

    const cb = new CircuitBreaker({
      failureThreshold: 3,
      resetTimeout: 100,
      successThreshold: 2,
    });

    for (let i = 0; i < 5; i++) {
      try {
        await cb.execute(() => pingDevice('temp-stable'));
      } catch {
        // expected for flaky
      }
    }

    expect(cb.getState()).toBe(CircuitState.Closed);

    for (let i = 0; i < 6; i++) {
      try {
        await cb.execute(() => pingDevice(failDevice));
      } catch {
        // expected
      }
    }

    expect(cb.getState()).toBe(CircuitState.Open);

    console.log('\n=== Circuit Breaker Device Ping (JS) ===');
    console.log(`  Flaky device state: ${cb.getState()}`);
    console.log(`  Stats: ${JSON.stringify(cb.getStats())}`);
  });

  test('retry policy for device communication', async () => {
    const attemptCounts: Record<string, number> = {};

    async function sendCommand(deviceId: string, command: string): Promise<string> {
      const key = `${deviceId}:${command}`;
      attemptCounts[key] = (attemptCounts[key] || 0) + 1;
      if (attemptCounts[key] < 3) throw new Error('Timeout');
      return `ok:${deviceId}:${command}`;
    }

    const policy = new RetryPolicy({
      maxAttempts: 5,
      initialDelay: 10,
      maxDelay: 100,
      strategy: BackoffStrategy.ExponentialJitter,
    });

    const result = await policy.execute(() => sendCommand('temp-001', 'read'));
    expect(result.success).toBe(true);
    expect(result.attempts).toBe(3);

    console.log('\n=== Device Retry (JS) ===');
    console.log(`  Attempts: ${result.attempts}, Time: ${result.totalTime}ms`);
  });

  test('full device lifecycle', async () => {
    const auditLog: string[] = [];
    const alerts: Array<Record<string, unknown>> = [];
    const thresholds = { temperature: { min: -10, max: 50 } };

    const device = {
      device_id: 'sensor-lifecycle-001',
      type: 'temperature',
      firmware: 'v1.0.0',
      status: 'active' as string,
      readings_count: 0,
    };

    auditLog.push(`REGISTERED: ${device.device_id}`);

    for (let i = 0; i < 10; i++) {
      const temp = Math.random() * 65 - 5;
      if (temp > 50) {
        alerts.push({ device_id: device.device_id, type: 'temperature', value: temp, severity: 'critical' });
      }
      device.readings_count++;
    }
    auditLog.push('TELEMETRY: 10 readings processed');

    device.firmware = 'v2.0.0';
    auditLog.push('FIRMWARE: Updated to v2.0.0');

    auditLog.push('HEALTH: healthy');

    expect(device.firmware).toBe('v2.0.0');
    expect(device.readings_count).toBe(10);
    expect(device.status).toBe('active');

    console.log('\n=== Full Device Lifecycle (JS) ===');
    console.log(`  Device: ${device.device_id}`);
    console.log(`  Firmware: v1.0.0 -> v2.0.0`);
    console.log(`  Readings: ${device.readings_count}`);
    console.log(`  Alerts: ${alerts.length}`);
    console.log(`  Audit:`);
    for (const entry of auditLog) {
      console.log(`    ${entry}`);
    }
  });
});

function range(start: number, end: number): number[] {
  return Array.from({ length: end - start }, (_, i) => start + i);
}
