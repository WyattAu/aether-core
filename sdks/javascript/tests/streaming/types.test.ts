/**
 * Tests for Streaming Types
 * @module aether/streaming/types
 */

import {
  Timestamp,
  Duration,
  WindowType,
  LateDataPolicy,
  WatermarkStrategy,
  BackpressureStrategy,
  DeliverySemantics,
  PaneInfo,
  Watermark,
  createStreamEvent,
  createWindowSpec,
  createWindowInfo,
  createStreamConfig,
  createBackpressureConfig,
  createDeliveryConfig,
} from '../../src/streaming/types';

describe('Timestamp', () => {
  test('should create from milliseconds', () => {
    const ts = new Timestamp(1000);
    expect(ts.milliseconds).toBe(1000);
  });

  test('should create from now', () => {
    const before = Date.now();
    const ts = Timestamp.now();
    const after = Date.now();
    expect(ts.milliseconds).toBeGreaterThanOrEqual(before);
    expect(ts.milliseconds).toBeLessThanOrEqual(after);
  });

  test('should create from Date', () => {
    const date = new Date('2024-01-01T00:00:00Z');
    const ts = Timestamp.fromDate(date);
    expect(ts.milliseconds).toBe(date.getTime());
  });

  test('should create from seconds', () => {
    const ts = Timestamp.fromSeconds(1000);
    expect(ts.milliseconds).toBe(1000000);
  });

  test('should convert to Date', () => {
    const ts = new Timestamp(1000);
    const date = ts.toDate();
    expect(date.getTime()).toBe(1000);
  });

  test('should convert to seconds', () => {
    const ts = new Timestamp(5000);
    expect(ts.toSeconds()).toBe(5);
  });

  test('should add duration', () => {
    const ts = new Timestamp(1000);
    const duration = new Duration(500);
    const result = ts.add(duration);
    expect(result.milliseconds).toBe(1500);
  });

  test('should subtract timestamp to get duration', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(500);
    const result = ts1.subtract(ts2);
    expect(result.milliseconds).toBe(500);
  });

  test('should subtract duration from timestamp', () => {
    const ts = new Timestamp(1000);
    const duration = new Duration(300);
    const result = ts.subtractDuration(duration);
    expect(result.milliseconds).toBe(700);
  });

  test('should compare timestamps', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(2000);

    expect(ts1.compareTo(ts2)).toBeLessThan(0);
    expect(ts2.compareTo(ts1)).toBeGreaterThan(0);
    expect(ts1.compareTo(ts1)).toBe(0);
  });

  test('should check equality', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(1000);
    const ts3 = new Timestamp(2000);

    expect(ts1.equals(ts2)).toBe(true);
    expect(ts1.equals(ts3)).toBe(false);
  });

  test('should check isBefore', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(2000);

    expect(ts1.isBefore(ts2)).toBe(true);
    expect(ts2.isBefore(ts1)).toBe(false);
    expect(ts1.isBefore(ts1)).toBe(false);
  });

  test('should check isAfter', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(2000);

    expect(ts2.isAfter(ts1)).toBe(true);
    expect(ts1.isAfter(ts2)).toBe(false);
    expect(ts1.isAfter(ts1)).toBe(false);
  });

  test('should check isBeforeOrEqual', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(2000);

    expect(ts1.isBeforeOrEqual(ts2)).toBe(true);
    expect(ts1.isBeforeOrEqual(ts1)).toBe(true);
    expect(ts2.isBeforeOrEqual(ts1)).toBe(false);
  });

  test('should check isAfterOrEqual', () => {
    const ts1 = new Timestamp(1000);
    const ts2 = new Timestamp(2000);

    expect(ts2.isAfterOrEqual(ts1)).toBe(true);
    expect(ts1.isAfterOrEqual(ts1)).toBe(true);
    expect(ts1.isAfterOrEqual(ts2)).toBe(false);
  });

  test('should serialize to JSON', () => {
    const ts = new Timestamp(1000);
    expect(ts.toJSON()).toBe(1000);
  });

  test('should deserialize from JSON', () => {
    const ts = Timestamp.fromJSON(1000);
    expect(ts.milliseconds).toBe(1000);
  });
});

describe('Duration', () => {
  test('should create from milliseconds', () => {
    const d = Duration.fromMillis(1000);
    expect(d.milliseconds).toBe(1000);
  });

  test('should create from seconds', () => {
    const d = Duration.fromSeconds(5);
    expect(d.milliseconds).toBe(5000);
  });

  test('should create from minutes', () => {
    const d = Duration.fromMinutes(2);
    expect(d.milliseconds).toBe(120000);
  });

  test('should create from hours', () => {
    const d = Duration.fromHours(1);
    expect(d.milliseconds).toBe(3600000);
  });

  test('should convert to seconds', () => {
    const d = Duration.fromMillis(5000);
    expect(d.toSeconds()).toBe(5);
  });

  test('should convert to milliseconds', () => {
    const d = new Duration(1000);
    expect(d.toMillis()).toBe(1000);
  });

  test('should add durations', () => {
    const d1 = Duration.fromMillis(1000);
    const d2 = Duration.fromMillis(500);
    const result = d1.add(d2);
    expect(result.milliseconds).toBe(1500);
  });

  test('should subtract durations', () => {
    const d1 = Duration.fromMillis(1000);
    const d2 = Duration.fromMillis(300);
    const result = d1.subtract(d2);
    expect(result.milliseconds).toBe(700);
  });

  test('should multiply duration', () => {
    const d = Duration.fromMillis(1000);
    const result = d.multiply(3);
    expect(result.milliseconds).toBe(3000);
  });
});

describe('Enums', () => {
  test('WindowType should have expected values', () => {
    expect(WindowType.Tumbling).toBe('tumbling');
    expect(WindowType.Sliding).toBe('sliding');
    expect(WindowType.Session).toBe('session');
  });

  test('LateDataPolicy should have expected values', () => {
    expect(LateDataPolicy.Drop).toBe('drop');
    expect(LateDataPolicy.SideOutput).toBe('side-output');
    expect(LateDataPolicy.Reprocess).toBe('reprocess');
  });

  test('WatermarkStrategy should have expected values', () => {
    expect(WatermarkStrategy.EventTime).toBe('event-time');
    expect(WatermarkStrategy.ProcessingTime).toBe('processing-time');
    expect(WatermarkStrategy.BoundedOutOfOrder).toBe('bounded-out-of-order');
  });

  test('BackpressureStrategy should have expected values', () => {
    expect(BackpressureStrategy.Buffer).toBe('buffer');
    expect(BackpressureStrategy.Drop).toBe('drop');
    expect(BackpressureStrategy.Fail).toBe('fail');
    expect(BackpressureStrategy.Latest).toBe('latest');
  });

  test('DeliverySemantics should have expected values', () => {
    expect(DeliverySemantics.AtMostOnce).toBe('at-most-once');
    expect(DeliverySemantics.AtLeastOnce).toBe('at-least-once');
    expect(DeliverySemantics.ExactlyOnce).toBe('exactly-once');
  });

  test('PaneInfo should have expected values', () => {
    expect(PaneInfo.Early).toBe('early');
    expect(PaneInfo.OnTime).toBe('on-time');
    expect(PaneInfo.Late).toBe('late');
  });
});

describe('Watermark', () => {
  test('should create watermark', () => {
    const ts = new Timestamp(1000);
    const wm = new Watermark(ts, 'stream-1');
    expect(wm.timestamp).toBe(ts);
    expect(wm.streamId).toBe('stream-1');
    expect(wm.partition).toBeUndefined();
  });

  test('should create watermark with partition', () => {
    const ts = new Timestamp(1000);
    const wm = new Watermark(ts, 'stream-1', 2);
    expect(wm.partition).toBe(2);
  });

  test('should detect late events', () => {
    const ts = new Timestamp(1000);
    const wm = new Watermark(ts, 'stream-1');

    const early = new Timestamp(500);
    const late = new Timestamp(1500);

    expect(wm.isLate(early)).toBe(true);
    expect(wm.isLate(late)).toBe(false);
  });

  test('should serialize to JSON', () => {
    const ts = new Timestamp(1000);
    const wm = new Watermark(ts, 'stream-1', 2);
    const json = wm.toJSON();

    expect(json).toEqual({
      timestamp: 1000,
      streamId: 'stream-1',
      partition: 2,
    });
  });

  test('should deserialize from object', () => {
    const wm = Watermark.fromObject({
      timestamp: 1000,
      streamId: 'stream-1',
      partition: 2,
    });

    expect(wm.timestamp.milliseconds).toBe(1000);
    expect(wm.streamId).toBe('stream-1');
    expect(wm.partition).toBe(2);
  });
});

describe('createStreamEvent', () => {
  test('should create event with defaults', () => {
    const event = createStreamEvent('key1', { data: 'test' });

    expect(event.key).toBe('key1');
    expect(event.value).toEqual({ data: 'test' });
    expect(event.timestamp).toBeDefined();
    expect(event.headers).toBeUndefined();
  });

  test('should create event with custom timestamp', () => {
    const ts = new Timestamp(1000);
    const event = createStreamEvent('key1', 'value', ts);

    expect(event.timestamp).toBe(ts);
  });

  test('should create event with options', () => {
    const event = createStreamEvent('key1', 'value', undefined, {
      headers: { 'content-type': 'application/json' },
      partition: 1,
      offset: 100,
      eventType: 'test-event',
    });

    expect(event.headers).toEqual({ 'content-type': 'application/json' });
    expect(event.partition).toBe(1);
    expect(event.offset).toBe(100);
    expect(event.eventType).toBe('test-event');
  });
});

describe('createWindowSpec', () => {
  test('should create tumbling window spec', () => {
    const size = Duration.fromMinutes(5);
    const spec = createWindowSpec(WindowType.Tumbling, size);

    expect(spec.type).toBe(WindowType.Tumbling);
    expect(spec.size).toBe(size);
    expect(spec.lateTolerance.milliseconds).toBe(0);
  });

  test('should create sliding window spec', () => {
    const size = Duration.fromMinutes(10);
    const slide = Duration.fromMinutes(1);
    const spec = createWindowSpec(WindowType.Sliding, size, { slide });

    expect(spec.type).toBe(WindowType.Sliding);
    expect(spec.slide).toBe(slide);
  });

  test('should throw for sliding window without slide', () => {
    const size = Duration.fromMinutes(10);
    expect(() => createWindowSpec(WindowType.Sliding, size)).toThrow(
      "Sliding window requires 'slide' parameter"
    );
  });

  test('should create session window spec', () => {
    const gap = Duration.fromMinutes(5);
    const spec = createWindowSpec(WindowType.Session, Duration.fromMillis(0), { gap });

    expect(spec.type).toBe(WindowType.Session);
    expect(spec.gap).toBe(gap);
  });

  test('should throw for session window without gap', () => {
    expect(() =>
      createWindowSpec(WindowType.Session, Duration.fromMillis(0))
    ).toThrow("Session window requires 'gap' parameter");
  });

  test('should create window spec with all options', () => {
    const size = Duration.fromMinutes(5);
    const spec = createWindowSpec(WindowType.Tumbling, size, {
      lateTolerance: Duration.fromSeconds(30),
      allowedLateness: Duration.fromMinutes(1),
    });

    expect(spec.lateTolerance.milliseconds).toBe(30000);
    expect(spec.allowedLateness.milliseconds).toBe(60000);
  });
});

describe('createWindowInfo', () => {
  test('should create window info', () => {
    const start = new Timestamp(0);
    const end = new Timestamp(1000);
    const maxTs = new Timestamp(500);

    const info = createWindowInfo(start, end, maxTs, PaneInfo.OnTime, 'window-1');

    expect(info.start).toBe(start);
    expect(info.end).toBe(end);
    expect(info.maxTimestamp).toBe(maxTs);
    expect(info.pane).toBe(PaneInfo.OnTime);
    expect(info.windowId).toBe('window-1');
  });
});

describe('createStreamConfig', () => {
  test('should create default config', () => {
    const config = createStreamConfig();

    expect(config.inputStreams).toEqual([]);
    expect(config.outputStreams).toEqual([]);
    expect(config.parallelism).toBe(1);
    expect(config.partitionStrategy).toBe('key');
  });

  test('should create config with options', () => {
    const config = createStreamConfig({
      inputStreams: ['input-1'],
      outputStreams: ['output-1'],
      parallelism: 4,
      partitionStrategy: 'hash',
      watermarkStrategy: WatermarkStrategy.EventTime,
      checkpointingEnabled: true,
    });

    expect(config.inputStreams).toEqual(['input-1']);
    expect(config.outputStreams).toEqual(['output-1']);
    expect(config.parallelism).toBe(4);
    expect(config.partitionStrategy).toBe('hash');
    expect(config.watermarkStrategy).toBe(WatermarkStrategy.EventTime);
    expect(config.checkpointingEnabled).toBe(true);
  });
});

describe('createBackpressureConfig', () => {
  test('should create default config', () => {
    const config = createBackpressureConfig();

    expect(config.strategy).toBe(BackpressureStrategy.Buffer);
    expect(config.bufferSize).toBe(10000);
    expect(config.highWatermark).toBe(0.9);
    expect(config.lowWatermark).toBe(0.5);
  });

  test('should create config with options', () => {
    const config = createBackpressureConfig({
      strategy: BackpressureStrategy.Drop,
      bufferSize: 5000,
      highWatermark: 0.8,
      lowWatermark: 0.3,
    });

    expect(config.strategy).toBe(BackpressureStrategy.Drop);
    expect(config.bufferSize).toBe(5000);
    expect(config.highWatermark).toBe(0.8);
    expect(config.lowWatermark).toBe(0.3);
  });
});

describe('createDeliveryConfig', () => {
  test('should create default config', () => {
    const config = createDeliveryConfig();

    expect(config.semantics).toBe(DeliverySemantics.AtLeastOnce);
    expect(config.maxRetries).toBe(3);
    expect(config.enableIdempotence).toBe(false);
  });

  test('should create config with options', () => {
    const config = createDeliveryConfig({
      semantics: DeliverySemantics.ExactlyOnce,
      maxRetries: 5,
      deadLetterTopic: 'dlq-topic',
      enableIdempotence: true,
    });

    expect(config.semantics).toBe(DeliverySemantics.ExactlyOnce);
    expect(config.maxRetries).toBe(5);
    expect(config.deadLetterTopic).toBe('dlq-topic');
    expect(config.enableIdempotence).toBe(true);
  });
});
