/**
 * IoT Sensor Pipeline Example (TypeScript)
 *
 * Demonstrates:
 * - TumblingWindow for time-based aggregation of sensor events
 * - Watermark-based late data handling
 * - BackpressureController for burst sensor data
 * - MultiLevelBackpressure with priority queues
 * - RateBasedBackpressure for rate limiting
 * - Alert generation when thresholds are exceeded
 */

import {
  Timestamp,
  Duration,
  StreamEvent,
  Watermark,
  WindowType,
  PaneInfo,
  BackpressureStrategy,
  LateDataPolicy,
  createStreamEvent,
  createWindowSpec,
  createWindowInfo,
  createStreamConfig,
  createBackpressureConfig,
} from '../src/streaming/types';
import { TumblingWindow, WindowAssigner, WindowTrigger } from '../src/streaming/window';
import { BackpressureController, MultiLevelBackpressure, RateBasedBackpressure } from '../src/streaming/backpressure';

// ========================================
// Types
// ========================================

interface SensorReading {
  sensorId: string;
  sensorType: string;
  value: number;
  unit: string;
}

interface AggregatedWindow {
  sensorType: string;
  avg: number;
  min: number;
  max: number;
  count: number;
  windowStartMs: number;
  windowEndMs: number;
}

// ========================================
// Constants
// ========================================

const THRESHOLDS: Record<string, { min: number; max: number; unit: string }> = {
  temperature: { min: -10, max: 50, unit: 'C' },
  humidity: { min: 0, max: 100, unit: '%' },
  pressure: { min: 950, max: 1050, unit: 'hPa' },
};

const SENSOR_IDS: Record<string, string[]> = {
  temperature: ['temp-001', 'temp-002'],
  humidity: ['hum-001', 'hum-002'],
  pressure: ['press-001'],
};

const RANGES: Record<string, [number, number]> = {
  temperature: [15, 35],
  humidity: [30, 80],
  pressure: [990, 1030],
};

// ========================================
// Utilities
// ========================================

const AUDIT_LOG: string[] = [];

function log(msg: string): void {
  const ts = new Date().toISOString().substring(11, 23);
  const entry = `  [${ts}] ${msg}`;
  AUDIT_LOG.push(entry);
  console.log(entry);
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function randomRange(min: number, max: number): number {
  return Math.round((Math.random() * (max - min) + min) * 10) / 10;
}

function randomChoice<T>(arr: T[]): T {
  return arr[Math.floor(Math.random() * arr.length)];
}

function randomChoiceWeighted<T>(items: T[], weights: number[]): T {
  const total = weights.reduce((a, b) => a + b, 0);
  let r = Math.random() * total;
  for (let i = 0; i < items.length; i++) {
    r -= weights[i];
    if (r <= 0) return items[i];
  }
  return items[items.length - 1];
}

// ========================================
// Sensor helpers
// ========================================

function generateReading(sensorType: string): SensorReading {
  const [lo, hi] = RANGES[sensorType];
  return {
    sensorId: randomChoice(SENSOR_IDS[sensorType]),
    sensorType,
    value: randomRange(lo, hi),
    unit: THRESHOLDS[sensorType].unit,
  };
}

function checkThreshold(reading: SensorReading): string | null {
  const t = THRESHOLDS[reading.sensorType];
  if (reading.value > t.max) {
    return `HIGH: ${reading.sensorType}=${reading.value}${t.unit} exceeds ${t.max}${t.unit}`;
  }
  if (reading.value < t.min) {
    return `LOW: ${reading.sensorType}=${reading.value}${t.unit} below ${t.min}${t.unit}`;
  }
  return null;
}

function aggregateEvents(
  events: StreamEvent<SensorReading>[],
  info: { start: Timestamp; end: Timestamp; maxTimestamp: Timestamp; pane: PaneInfo; windowId?: string }
): AggregatedWindow {
  if (events.length === 0) {
    return { sensorType: '', avg: 0, min: 0, max: 0, count: 0, windowStartMs: 0, windowEndMs: 0 };
  }

  const values = events.map(e => e.value.value);
  const sensorType = events[0].value.sensorType;

  return {
    sensorType,
    avg: Math.round((values.reduce((a, b) => a + b, 0) / values.length) * 10) / 10,
    min: Math.min(...values),
    max: Math.max(...values),
    count: values.length,
    windowStartMs: info.start.milliseconds,
    windowEndMs: info.end.milliseconds,
  };
}

// ========================================
// Main
// ========================================

async function runSensorPipeline(): Promise<void> {
  console.log('='.repeat(70));
  console.log('  AETHER IOT SENSOR PIPELINE (TypeScript)');
  console.log('='.repeat(70));
  console.log();

  const windowSize = Duration.fromSeconds(60);
  const allAggregations: AggregatedWindow[] = [];
  const allAlerts: string[] = [];

  console.log('--- Step 1: Set up tumbling windows per sensor type ---');
  const tempWindow = new TumblingWindow<string, SensorReading, AggregatedWindow>(
    windowSize, aggregateEvents
  );
  const humWindow = new TumblingWindow<string, SensorReading, AggregatedWindow>(
    windowSize, aggregateEvents
  );
  const pressWindow = new TumblingWindow<string, SensorReading, AggregatedWindow>(
    windowSize, aggregateEvents
  );
  log(`Created tumbling windows (size=${windowSize.toSeconds()}s)`);
  console.log();

  console.log('--- Step 2: Set up backpressure controller ---');
  const bpConfig = createBackpressureConfig({
    strategy: BackpressureStrategy.Buffer,
    bufferSize: 50,
    highWatermark: 0.8,
    lowWatermark: 0.3,
  });
  const backpressure = new BackpressureController<SensorReading>(bpConfig);
  let overflowCount = 0;
  let resumeCount = 0;

  backpressure.setOverflowCallback(() => {
    overflowCount++;
    log(`BACKPRESSURE: High watermark reached (overflow #${overflowCount})`);
  });
  backpressure.setResumeCallback(() => {
    resumeCount++;
    log(`BACKPRESSURE: Buffer recovered (resume #${resumeCount})`);
  });

  log(`Backpressure configured: strategy=${bpConfig.strategy}, buffer=${bpConfig.bufferSize}`);
  console.log();

  console.log('--- Step 3: Set up watermark tracking ---');
  const watermarks: Map<string, Timestamp> = new Map();
  const lateEvents: StreamEvent<SensorReading>[] = [];
  log('Watermark tracking initialized');
  console.log();

  console.log('--- Step 4: Generate and process sensor events ---');
  console.log();

  const baseTimeMs = Math.floor(Date.now() / 60000) * 60000;
  const windowMap: Record<string, TumblingWindow<string, SensorReading, AggregatedWindow>> = {
    temperature: tempWindow,
    humidity: humWindow,
    pressure: pressWindow,
  };

  let eventsAccepted = 0;
  let eventsDropped = 0;
  let eventsLate = 0;

  for (let secondOffset = 0; secondOffset < 125; secondOffset += 2) {
    const eventTimeMs = baseTimeMs + secondOffset * 1000;
    const eventTs = new Timestamp(eventTimeMs);
    const numReadings = randomChoiceWeighted([1, 2, 3], [5, 3, 1]);

    for (let r = 0; r < numReadings; r++) {
      const sensorType = randomChoice(['temperature', 'humidity', 'pressure']);

      const isLate = Math.random() < 0.05;
      if (isLate && secondOffset > 30) {
        const reading = generateReading(sensorType);
        const lateTs = new Timestamp(eventTimeMs - Math.floor(Math.random() * 30000 + 90000));
        const lateEvent = createStreamEvent<SensorReading>(
          reading.sensorId,
          reading,
          lateTs,
          { eventType: sensorType },
        );

        const currentWm = watermarks.get(sensorType) ?? new Timestamp(0);
        if (lateEvent.timestamp.isBefore(currentWm)) {
          eventsLate++;
          lateEvents.push(lateEvent);
          const lateTime = lateEvent.timestamp.toDate().toISOString().substring(11, 19);
          const wmTime = currentWm.toDate().toISOString().substring(11, 19);
          log(`LATE DATA: ${sensorType} from ${reading.sensorId} at ${lateTime} (watermark: ${wmTime})`);
          continue;
        }
      }

      const reading = generateReading(sensorType);
      const event = createStreamEvent<SensorReading>(
        reading.sensorId,
        reading,
        eventTs,
        { eventType: sensorType },
      );

      const alert = checkThreshold(reading);
      if (alert) {
        allAlerts.push(alert);
        log(`ALERT: ${alert}`);
      }

      if (!backpressure.tryPush(event)) {
        eventsDropped++;
        log(`DROPPED: Event from ${reading.sensorId} (buffer full)`);
        continue;
      }

      eventsAccepted++;

      const bufferedEvent = backpressure.pop();
      if (bufferedEvent) {
        const window = windowMap[(bufferedEvent.value as unknown as SensorReading).sensorType];
        const results = window.process(bufferedEvent, bufferedEvent.eventType ?? 'default');
        allAggregations.push(...results);
      }
    }

    const watermarkTs = new Timestamp(eventTimeMs + 5000);
    for (const stype of ['temperature', 'humidity', 'pressure']) {
      watermarks.set(stype, watermarkTs);
      const window = windowMap[stype];
      const results = window.advanceWatermark(watermarkTs);
      allAggregations.push(...results);
    }

    if (secondOffset % 20 === 0) {
      const stats = backpressure.getStats();
      log(`PROGRESS: t=${secondOffset}s | accepted=${eventsAccepted} ` +
        `dropped=${eventsDropped} late=${eventsLate} ` +
        `buffer=${stats.currentBufferSize}/${bpConfig.bufferSize} ` +
        `windows_fired=${allAggregations.length}`);
    }
  }

  console.log();

  console.log('--- Step 5: Fire remaining windows ---');
  const finalWatermark = new Timestamp(baseTimeMs + 180000);
  for (const [stype, window] of Object.entries(windowMap)) {
    const results = window.advanceWatermark(finalWatermark);
    allAggregations.push(...results);
  }
  log(`Fired remaining windows, total aggregations: ${allAggregations.length}`);
  console.log();

  console.log('--- Step 6: Multi-level backpressure demo ---');
  const mlbp = new MultiLevelBackpressure<SensorReading>(10);
  for (let i = 0; i < 15; i++) {
    const priority = i < 8 ? MultiLevelBackpressure.LOW : MultiLevelBackpressure.HIGH;
    const event = createStreamEvent(`sensor-${i}`, { sensorId: `${i}`, sensorType: 'temp', value: i, unit: 'C' });
    const accepted = mlbp.push(event, priority);
    if (!accepted) {
      log(`MLBP: Event ${i} dropped (priority=${priority})`);
    }
  }
  const mlbpStats = mlbp.getStats();
  log(`Multi-level BP: total=${mlbpStats.totalEvents}, dropped=${mlbpStats.droppedEvents}, buffered=${mlbpStats.bufferedEvents}`);
  while (!mlbp.isEmpty()) {
    mlbp.pop();
  }
  log('Multi-level BP: drained');
  console.log();

  console.log('--- Step 7: Rate-based backpressure demo ---');
  const rateBP = new RateBasedBackpressure(5, 1.0, 0.2);
  let allowed = 0;
  let rejected = 0;
  for (let i = 0; i < 20; i++) {
    if (await rateBP.tryAcquire()) {
      allowed++;
    } else {
      rejected++;
    }
  }
  log(`Rate-based BP: allowed=${allowed}, rejected=${rejected}, active=${rateBP.isBackpressureActive}`);
  console.log();

  console.log('='.repeat(70));
  console.log('  PIPELINE SUMMARY');
  console.log('='.repeat(70));
  log(`Total events accepted: ${eventsAccepted}`);
  log(`Total events dropped:  ${eventsDropped}`);
  log(`Total late events:     ${eventsLate}`);
  log(`Total alerts:          ${allAlerts.length}`);
  log(`Total window results:  ${allAggregations.length}`);
  log(`Backpressure overflows: ${overflowCount}`);
  log(`Backpressure resumes:   ${resumeCount}`);
  console.log();

  if (allAggregations.length > 0) {
    console.log('  Window Aggregations:');
    for (const agg of allAggregations.slice(0, 12)) {
      const durationS = (agg.windowEndMs - agg.windowStartMs) / 1000;
      log(`  [${agg.sensorType.padEnd(11)}] avg=${String(agg.avg).padStart(6)} ` +
        `min=${String(agg.min).padStart(6)} max=${String(agg.max).padStart(6)} ` +
        `count=${String(agg.count).padStart(3)} window=${durationS}s`);
    }
    if (allAggregations.length > 12) {
      log(`  ... and ${allAggregations.length - 12} more`);
    }
  }
  console.log();

  if (allAlerts.length > 0) {
    console.log('  Alerts Generated:');
    for (const alert of allAlerts.slice(0, 8)) {
      log(`  ! ${alert}`);
    }
    if (allAlerts.length > 8) {
      log(`  ... and ${allAlerts.length - 8} more`);
    }
  } else {
    log('No threshold alerts generated (all readings within range)');
  }
  console.log();

  log(`Total audit entries: ${AUDIT_LOG.length}`);
}

(async () => {
  await runSensorPipeline();
})();
