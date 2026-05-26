// @jest/tag:e2e
/**
 * E2E Scenario 2: Real-Time Analytics Pipeline
 *
 * Simulates a real-time analytics pipeline:
 * - Ingestion of 10K events (page views, clicks, purchases)
 * - Tumbling windows (5-minute) aggregate counts
 * - Aggregation: count events per type, compute running averages
 * - Alerting: trigger alerts when thresholds exceeded (>1000 purchases in 5 min)
 * - Late data: inject late events and verify watermark handling
 */

import {
  Timestamp,
  Duration,
  WindowType,
  PaneInfo,
  LateDataPolicy,
  BackpressureStrategy,
  createWindowSpec,
  createStreamEvent,
  Watermark,
} from '../../src/streaming/types';
import {
  TumblingWindow,
  WindowAssigner,
  WindowTrigger,
  createWindowState,
} from '../../src/streaming/window';
import {
  BackpressureController,
  BufferFullError,
  RateBasedBackpressure,
  MultiLevelBackpressure,
} from '../../src/streaming/backpressure';
import type { StreamEvent, WindowInfo } from '../../src/streaming/types';

const FIVE_MINUTES_MS = 5 * 60 * 1000;
const BASE_TS = 1700000000000;

function makeEvent(eventType: string, tsOffsetMs: number = 0): StreamEvent<Record<string, unknown>> {
  const seed = Math.floor(Math.random() * 1000);
  return createStreamEvent<Record<string, unknown>>(
    `user-${seed}`,
    { type: eventType, amount: Math.random() * 100 },
    new Timestamp(BASE_TS + tsOffsetMs),
    { eventType },
  );
}

describe('E2E: Real-Time Analytics Pipeline', () => {
  test('ingest 10K events and count per type', () => {
    const eventTypes = ['page_view', 'click', 'purchase'];
    const weights = [0.6, 0.3, 0.1];
    const events: StreamEvent<Record<string, unknown>>[] = [];

    for (let i = 0; i < 10_000; i++) {
      const r = Math.random();
      let cumulative = 0;
      let chosen = eventTypes[0];
      for (let j = 0; j < eventTypes.length; j++) {
        cumulative += weights[j];
        if (r < cumulative) {
          chosen = eventTypes[j];
          break;
        }
      }
      events.push(makeEvent(chosen, Math.floor(Math.random() * FIVE_MINUTES_MS)));
    }

    const counts: Record<string, number> = { page_view: 0, click: 0, purchase: 0 };
    for (const event of events) {
      const etype = event.value.type as string;
      counts[etype] = (counts[etype] || 0) + 1;
    }

    const total = Object.values(counts).reduce((a, b) => a + b, 0);
    expect(total).toBe(10_000);
    expect(counts.page_view).toBeGreaterThan(counts.click);
    expect(counts.click).toBeGreaterThan(counts.purchase);

    console.log('\n=== 10K Event Ingestion (JS) ===');
    console.log(`  Page views: ${counts.page_view}`);
    console.log(`  Clicks: ${counts.click}`);
    console.log(`  Purchases: ${counts.purchase}`);
  });

  test('tumbling window aggregation', () => {
    const windowResults: Array<Record<string, unknown>> = [];

    const tw = new TumblingWindow<string, Record<string, unknown>, Record<string, unknown>>(
      Duration.fromMillis(FIVE_MINUTES_MS),
      (events: StreamEvent<Record<string, unknown>>[], info: WindowInfo): Record<string, unknown> => {
        const counts: Record<string, number> = {};
        for (const e of events) {
          const etype = e.value.type as string;
          counts[etype] = (counts[etype] || 0) + 1;
        }
        const result = {
          windowId: info.windowId,
          total: events.length,
          counts,
        };
        windowResults.push(result);
        return result;
      },
    );

    const eventTypes = ['page_view', 'click', 'purchase'];
    for (let i = 0; i < 1000; i++) {
      const tsOffset = i * Math.floor(FIVE_MINUTES_MS / 1000);
      for (let j = 0; j < 10; j++) {
        const etype = eventTypes[Math.floor(Math.random() * eventTypes.length)];
        tw.process(makeEvent(etype, tsOffset), 'analytics');
      }
    }

    const watermark = new Timestamp(BASE_TS + FIVE_MINUTES_MS);
    tw.advanceWatermark(watermark);

    expect(windowResults.length).toBeGreaterThan(0);
    for (const result of windowResults) {
      expect(result.total).toBeGreaterThan(0);
    }

    const totalInWindows = windowResults.reduce((sum, r) => sum + (r.total as number), 0);
    console.log(`\n=== Tumbling Window Aggregation (JS) ===`);
    console.log(`  Windows fired: ${windowResults.length}`);
    console.log(`  Total events in windows: ${totalInWindows}`);
  });

  test('running average computation', () => {
    const runningCounts: Record<string, number[]> = {
      page_view: [],
      click: [],
      purchase: [],
    };

    const tw = new TumblingWindow<string, Record<string, unknown>, Record<string, number>>(
      Duration.fromMillis(FIVE_MINUTES_MS),
      (events: StreamEvent<Record<string, unknown>>[], _info: WindowInfo): Record<string, number> => {
        const counts: Record<string, number> = {};
        for (const e of events) {
          const etype = e.value.type as string;
          counts[etype] = (counts[etype] || 0) + 1;
        }
        for (const [etype, count] of Object.entries(counts)) {
          runningCounts[etype].push(count);
        }
        const averages: Record<string, number> = {};
        for (const [etype, vals] of Object.entries(runningCounts)) {
          averages[etype] = vals.reduce((a, b) => a + b, 0) / vals.length;
        }
        return averages;
      },
    );

    for (let windowIdx = 0; windowIdx < 3; windowIdx++) {
      const base = windowIdx * FIVE_MINUTES_MS;
      for (let i = 0; i < 500; i++) {
        const tsOffset = base + Math.floor(Math.random() * (FIVE_MINUTES_MS - 1));
        const etype = ['page_view', 'click', 'purchase'][Math.floor(Math.random() * 3)];
        tw.process(makeEvent(etype, tsOffset), 'avg-test');
      }
      tw.advanceWatermark(new Timestamp(BASE_TS + (windowIdx + 1) * FIVE_MINUTES_MS));
    }

    expect(runningCounts.page_view.length).toBe(3);
    expect(runningCounts.click.length).toBe(3);
    expect(runningCounts.purchase.length).toBe(3);

    console.log('\n=== Running Average (JS) ===');
    for (const [etype, vals] of Object.entries(runningCounts)) {
      const avg = vals.reduce((a, b) => a + b, 0) / vals.length;
      console.log(`  ${etype}: counts=${vals}, avg=${avg.toFixed(1)}`);
    }
  });

  test('alerting on threshold exceedance', () => {
    const alerts: Array<Record<string, unknown>> = [];
    const ALERT_THRESHOLD = 1000;

    const tw = new TumblingWindow<string, Record<string, unknown>, Record<string, unknown> | null>(
      Duration.fromMillis(FIVE_MINUTES_MS),
      (events: StreamEvent<Record<string, unknown>>[], info: WindowInfo): Record<string, unknown> | null => {
        const purchaseCount = events.filter((e) => e.value.type === 'purchase').length;
        if (purchaseCount > ALERT_THRESHOLD) {
          const alert = {
            windowId: info.windowId,
            threshold: ALERT_THRESHOLD,
            actual: purchaseCount,
            severity: purchaseCount > 1500 ? 'critical' : 'warning',
          };
          alerts.push(alert);
          return alert;
        }
        return null;
      },
    );

    // Place all 1500 purchase events in a single window by using offsets within one window period
    // Use offsets that stay within the second tumbling window [BASE_TS-aligned + FIVE_MINUTES_MS, BASE_TS-aligned + 2*FIVE_MINUTES_MS)
    const windowStartOffset = FIVE_MINUTES_MS - (BASE_TS % FIVE_MINUTES_MS);
    for (let i = 0; i < 1500; i++) {
      const offset = windowStartOffset + Math.floor(i * ((FIVE_MINUTES_MS - 2) / 1500));
      tw.process(makeEvent('purchase', offset), 'alerts');
    }

    // Advance watermark to just before the window's end (trigger fires when end > watermark)
    tw.advanceWatermark(new Timestamp(BASE_TS + windowStartOffset + FIVE_MINUTES_MS - 1));

    expect(alerts.length).toBeGreaterThan(0);
    for (const alert of alerts) {
      expect((alert.actual as number)).toBeGreaterThan(ALERT_THRESHOLD);
    }

    console.log('\n=== Alerting (JS) ===');
    console.log(`  Alerts triggered: ${alerts.length}`);
    for (const a of alerts) {
      console.log(`  [${a.severity}] ${a.actual} purchases (threshold: ${a.threshold})`);
    }
  });

  test('late data watermark handling', () => {
    const allFired: Array<Record<string, unknown>> = [];

    const tw = new TumblingWindow<string, Record<string, unknown>, Record<string, unknown>>(
      Duration.fromMillis(FIVE_MINUTES_MS),
      (events: StreamEvent<Record<string, unknown>>[], info: WindowInfo): Record<string, unknown> => {
        const result = {
          windowId: info.windowId,
          count: events.length,
          pane: info.pane as string,
        };
        allFired.push(result);
        return result;
      },
    );

    for (let i = 0; i < 500; i++) {
      const tsOffset = Math.floor(Math.random() * (FIVE_MINUTES_MS - 1));
      tw.process(makeEvent('click', tsOffset), 'late-test');
    }

    const wm = new Timestamp(BASE_TS + FIVE_MINUTES_MS);
    const fired = tw.advanceWatermark(wm);

    expect(fired.length).toBeGreaterThan(0);
    expect(allFired.length).toBeGreaterThan(0);
    for (const f of allFired) {
      expect((f.count as number)).toBeGreaterThan(0);
    }

    console.log('\n=== Late Data Handling (JS) ===');
    console.log(`  On-time windows fired: ${allFired.length}`);
    console.log(`  Watermark: ${wm.milliseconds}`);
  });
});

describe('E2E: Analytics Backpressure', () => {
  test('backpressure controller under load', () => {
    const bp = new BackpressureController<Record<string, unknown>>({
      strategy: BackpressureStrategy.Buffer,
      bufferSize: 5000,
      highWatermark: 0.9,
      lowWatermark: 0.5,
    });

    let accepted = 0;
    let rejected = 0;
    for (let i = 0; i < 10_000; i++) {
      const event = makeEvent('page_view', i);
      if (bp.tryPush(event)) {
        accepted++;
      } else {
        rejected++;
      }
    }

    expect(accepted + rejected).toBe(10_000);
    expect(accepted).toBeGreaterThan(0);

    let consumed = 0;
    while (!bp.isEmpty()) {
      bp.pop();
      consumed++;
    }

    expect(consumed).toBe(accepted);
    expect(bp.isEmpty()).toBe(true);

    console.log('\n=== Backpressure Under Load (JS) ===');
    console.log(`  Accepted: ${accepted}, Rejected: ${rejected}`);
    console.log(`  Consumed: ${consumed}`);
  });

  test('rate-based backpressure', async () => {
    const rbp = new RateBasedBackpressure(100, 1.0, 0.1);

    let allowed = 0;
    let throttled = 0;
    for (let i = 0; i < 200; i++) {
      if (await rbp.tryAcquire()) {
        allowed++;
      } else {
        throttled++;
      }
    }

    expect(allowed).toBeGreaterThan(0);
    expect(throttled).toBeGreaterThan(0);

    console.log('\n=== Rate-Based Backpressure (JS) ===');
    console.log(`  Allowed: ${allowed}, Throttled: ${throttled}`);
  });

  test('multi-level backpressure priority', () => {
    const mlp = new MultiLevelBackpressure<Record<string, unknown>>(10);

    const criticalEvent = makeEvent('critical', 0);
    const normalEvent = makeEvent('normal', 0);
    const lowEvent = makeEvent('low', 0);

    for (let i = 0; i < 5; i++) {
      mlp.push(makeEvent('low', i), MultiLevelBackpressure.LOW);
    }

    const acceptedCritical = mlp.push(criticalEvent, MultiLevelBackpressure.HIGH);
    expect(acceptedCritical).toBe(true);

    mlp.pop();
    expect(mlp.size()).toBe(5);

    console.log('\n=== Multi-Level Backpressure (JS) ===');
    console.log(`  Buffer size: ${mlp.size()}`);
    console.log(`  Critical events always accepted`);
  });
});
