/**
 * Tests for Windowing Functions
 */

import {
  WindowAssigner,
  WindowTrigger,
  TumblingWindow,
  SlidingWindow,
  SessionWindow,
  WindowState,
  createWindowState,
} from '../../src/streaming/window';
import {
  Timestamp,
  Duration,
  WindowType,
  WindowSpec,
  StreamEvent,
  createWindowSpec,
  createStreamEvent,
} from '../../src/streaming/types';

describe('WindowState', () => {
  const start = new Timestamp(1000);
  const end = new Timestamp(2000);
  const key = 'test-key';

  test('createWindowState creates window with correct properties', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);

    expect(state.windowId).toBe('window-1');
    expect(state.key).toBe(key);
    expect(state.start).toBe(start);
    expect(state.end).toBe(end);
    expect(state.events).toEqual([]);
    expect(state.isClosed).toBe(false);
    expect(state.earlyFired).toBe(false);
    expect(state.onTimeFired).toBe(false);
  });

  test('isEmpty returns true for empty window', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    expect(state.isEmpty()).toBe(true);
  });

  test('isEmpty returns false after adding event', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event: StreamEvent<number> = {
      key: key,
      value: 42,
      timestamp: new Timestamp(1500),
    };
    state.events.push(event);
    expect(state.isEmpty()).toBe(false);
  });

  test('clear empties events and closes window', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event: StreamEvent<number> = {
      key: key,
      value: 42,
      timestamp: new Timestamp(1500),
    };
    state.events.push(event);
    state.clear();
    expect(state.events).toHaveLength(0);
    expect(state.isClosed).toBe(true);
  });
});

describe('WindowAssigner - Tumbling Windows', () => {
  const size = Duration.fromMillis(1000);
  const spec: WindowSpec = createWindowSpec(WindowType.Tumbling, size);
  let assigner: WindowAssigner<string, number>;

  beforeEach(() => {
    assigner = new WindowAssigner(spec);
  });

  test('assigns event to correct tumbling window', () => {
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    const windows = assigner.assign(event, 'key1');

    expect(windows).toHaveLength(1);
    expect(windows[0].start.milliseconds).toBe(1000);
    expect(windows[0].end.milliseconds).toBe(2000);
    expect(windows[0].events).toContain(event);
  });

  test('assigns events in same window to same window state', () => {
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1200),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(1500),
    };

    assigner.assign(event1, 'key1');
    const windows = assigner.assign(event2, 'key1');

    expect(windows).toHaveLength(1);
    expect(windows[0].events).toHaveLength(2);
  });

  test('assigns events to different windows based on time', () => {
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(500),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(1500),
    };

    const windows1 = assigner.assign(event1, 'key1');
    const windows2 = assigner.assign(event2, 'key1');

    expect(windows1[0].windowId).not.toBe(windows2[0].windowId);
  });

  test('separates windows by key', () => {
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    const event2: StreamEvent<number> = {
      key: 'key2',
      value: 43,
      timestamp: new Timestamp(1500),
    };

    const windows1 = assigner.assign(event1, 'key1');
    const windows2 = assigner.assign(event2, 'key2');

    expect(windows1[0].windowId).not.toBe(windows2[0].windowId);
  });
});

describe('WindowAssigner - Sliding Windows', () => {
  const size = Duration.fromMillis(1000);
  const slide = Duration.fromMillis(500);
  const spec: WindowSpec = createWindowSpec(WindowType.Sliding, size, { slide });
  let assigner: WindowAssigner<string, number>;

  beforeEach(() => {
    assigner = new WindowAssigner(spec);
  });

  test('assigns event to multiple overlapping windows', () => {
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1200),
    };
    const windows = assigner.assign(event, 'key1');

    // Event at 1200 should be in multiple windows
    expect(windows.length).toBeGreaterThan(0);
  });

  test('windows have correct size', () => {
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1200),
    };
    const windows = assigner.assign(event, 'key1');

    for (const window of windows) {
      const windowSize = window.end.milliseconds - window.start.milliseconds;
      expect(windowSize).toBe(size.milliseconds);
    }
  });
});

describe('WindowAssigner - Session Windows', () => {
  const gap = Duration.fromMillis(500);
  const spec: WindowSpec = createWindowSpec(WindowType.Session, Duration.fromMillis(0), { gap });
  let assigner: WindowAssigner<string, number>;

  beforeEach(() => {
    assigner = new WindowAssigner(spec);
  });

  test('creates new session for first event', () => {
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const windows = assigner.assign(event, 'key1');

    expect(windows).toHaveLength(1);
    expect(windows[0].events).toContain(event);
  });

  test('separates sessions by key', () => {
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const event2: StreamEvent<number> = {
      key: 'key2',
      value: 43,
      timestamp: new Timestamp(1000),
    };

    const windows1 = assigner.assign(event1, 'key1');
    const windows2 = assigner.assign(event2, 'key2');

    expect(windows1[0].key).toBe('key1');
    expect(windows2[0].key).toBe('key2');
  });
});

describe('WindowState - addEvent', () => {
  const start = new Timestamp(1000);
  const end = new Timestamp(2000);
  const key = 'test-key';

  test('addEvent returns true for valid event', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    expect(state.addEvent(event)).toBe(true);
    expect(state.events).toHaveLength(1);
  });

  test('addEvent returns false for event before window start', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(500),
    };
    expect(state.addEvent(event)).toBe(false);
    expect(state.events).toHaveLength(0);
  });

  test('addEvent returns false for event at window end', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(2000),
    };
    expect(state.addEvent(event)).toBe(false);
    expect(state.events).toHaveLength(0);
  });

  test('addEvent returns false for event after window end', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(2500),
    };
    expect(state.addEvent(event)).toBe(false);
    expect(state.events).toHaveLength(0);
  });

  test('addEvent returns false when window is closed', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    state.clear(); // closes the window
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    expect(state.addEvent(event)).toBe(false);
  });

  test('addEvent updates maxTimestamp', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1200),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(1800),
    };
    state.addEvent(event1);
    state.addEvent(event2);
    expect(state.maxTimestamp!.milliseconds).toBe(1800);
  });

  test('addEvent does not update maxTimestamp for earlier events', () => {
    const state = createWindowState<string, number>('window-1', key, start, end);
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1800),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(1200),
    };
    state.addEvent(event1);
    state.addEvent(event2);
    expect(state.maxTimestamp!.milliseconds).toBe(1800);
  });
});

describe('WindowAssigner - Session Windows (extended)', () => {
  const gap = Duration.fromMillis(500);
  const spec: WindowSpec = createWindowSpec(WindowType.Session, Duration.fromMillis(0), { gap });

  test('merges events within gap into same session', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(1300),
    };

    const windows1 = assigner.assign(event1, 'key1');
    const windows2 = assigner.assign(event2, 'key1');

    // Second event should merge into same session
    expect(windows2).toHaveLength(1);
    expect(windows2[0].events).toHaveLength(2);
  });

  test('creates new session when gap exceeded', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(2000),
    };

    assigner.assign(event1, 'key1');
    const windows2 = assigner.assign(event2, 'key1');

    // Gap of 1000ms > 500ms gap, should create new session
    expect(windows2).toHaveLength(1);
    expect(windows2[0].events).toHaveLength(1);
  });
});

describe('WindowAssigner - getTriggeredWindows', () => {
  const size = Duration.fromMillis(1000);
  const spec: WindowSpec = createWindowSpec(WindowType.Tumbling, size);

  test('returns windows where end is after watermark', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    assigner.assign(event, 'key1');

    // Watermark at 1500, window ends at 2000 -> should trigger
    const triggered = assigner.getTriggeredWindows(new Timestamp(1500));
    expect(triggered).toHaveLength(1);
  });

  test('does not return closed windows', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    assigner.assign(event, 'key1');

    // Close the window
    const allWindows = assigner.assign(event, 'key1');
    allWindows[0].clear();

    const triggered = assigner.getTriggeredWindows(new Timestamp(1500));
    expect(triggered).toHaveLength(0);
  });

  test('does not return windows where end is before watermark', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    assigner.assign(event, 'key1');

    // Window ends at 2000, watermark at 2500 -> window end NOT after watermark
    const triggered = assigner.getTriggeredWindows(new Timestamp(2500));
    expect(triggered).toHaveLength(0);
  });
});

describe('WindowAssigner - cleanupClosed', () => {
  const size = Duration.fromMillis(1000);
  const spec: WindowSpec = createWindowSpec(WindowType.Tumbling, size);

  test('removes closed windows and returns count', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event1: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(500),
    };
    const event2: StreamEvent<number> = {
      key: 'key1',
      value: 43,
      timestamp: new Timestamp(1500),
    };

    assigner.assign(event1, 'key1');
    const windows = assigner.assign(event2, 'key1');
    windows[0].clear();

    const removed = assigner.cleanupClosed();
    expect(removed).toBe(1);
  });

  test('returns 0 when no closed windows', () => {
    const assigner = new WindowAssigner<string, number>(spec);
    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    assigner.assign(event, 'key1');

    const removed = assigner.cleanupClosed();
    expect(removed).toBe(0);
  });
});

describe('TumblingWindow', () => {
  test('creates and processes tumbling window', () => {
    const handler = jest.fn((events: StreamEvent<number>[], info) => ({
      count: events.length,
    }));

    const window = new TumblingWindow<string, number, { count: number }>(
      Duration.fromSeconds(5),
      handler
    );

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const results = window.process(event, 'key1');

    expect(Array.isArray(results)).toBe(true);
  });

  test('advanceWatermark returns array', () => {
    const handler = jest.fn((events: StreamEvent<number>[], info) => ({
      count: events.length,
    }));

    const window = new TumblingWindow<string, number, { count: number }>(
      Duration.fromSeconds(5),
      handler
    );

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    window.process(event, 'key1');

    const results = window.advanceWatermark(new Timestamp(5000));
    expect(Array.isArray(results)).toBe(true);
  });
});

describe('SlidingWindow', () => {
  test('creates and processes sliding window', () => {
    const handler = jest.fn((events: StreamEvent<number>[], info) => ({
      count: events.length,
    }));

    const window = new SlidingWindow<string, number, { count: number }>(
      Duration.fromSeconds(10), // size
      Duration.fromSeconds(1),  // slide
      handler
    );

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    const results = window.process(event, 'key1');

    expect(Array.isArray(results)).toBe(true);
  });

  test('advanceWatermark returns array', () => {
    const handler = jest.fn((events: StreamEvent<number>[], info) => ({
      count: events.length,
    }));

    const window = new SlidingWindow<string, number, { count: number }>(
      Duration.fromSeconds(10),
      Duration.fromSeconds(1),
      handler
    );

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1500),
    };
    window.process(event, 'key1');

    const results = window.advanceWatermark(new Timestamp(15000));
    expect(Array.isArray(results)).toBe(true);
  });
});

describe('SessionWindow', () => {
  test('creates and processes session window', () => {
    const handler = jest.fn((events: StreamEvent<number>[], info) => ({
      count: events.length,
    }));

    const window = new SessionWindow<string, number, { count: number }>(
      Duration.fromSeconds(5), // gap
      handler
    );

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const results = window.process(event, 'key1');

    expect(Array.isArray(results)).toBe(true);
  });

  test('advanceWatermark returns array', () => {
    const handler = jest.fn((events: StreamEvent<number>[], info) => ({
      count: events.length,
    }));

    const window = new SessionWindow<string, number, { count: number }>(
      Duration.fromSeconds(5),
      handler
    );

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    window.process(event, 'key1');

    const results = window.advanceWatermark(new Timestamp(10000));
    expect(Array.isArray(results)).toBe(true);
  });
});

describe('WindowTrigger', () => {
  test('process with early firing triggers results', () => {
    const spec = createWindowSpec(WindowType.Tumbling, Duration.fromMillis(1000));
    const assigner = new WindowAssigner<string, number>(spec);
    const handler = jest.fn((events, info) => events.length);
    const trigger = new WindowTrigger(assigner, handler, Duration.fromMillis(500));

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    const results = trigger.process(event, 'key1');

    expect(Array.isArray(results)).toBe(true);
  });

  test('advanceWatermark fires on-time windows', () => {
    const spec = createWindowSpec(WindowType.Tumbling, Duration.fromMillis(1000));
    const assigner = new WindowAssigner<string, number>(spec);
    const handler = jest.fn((events, info) => events.length);
    const trigger = new WindowTrigger(assigner, handler);

    const event: StreamEvent<number> = {
      key: 'key1',
      value: 42,
      timestamp: new Timestamp(1000),
    };
    trigger.process(event, 'key1');

    const results = trigger.advanceWatermark(new Timestamp(1500));
    expect(Array.isArray(results)).toBe(true);
  });

  test('advanceWatermark skips empty windows', () => {
    const spec = createWindowSpec(WindowType.Tumbling, Duration.fromMillis(1000));
    const assigner = new WindowAssigner<string, number>(spec);
    const handler = jest.fn((events, info) => events.length);
    const trigger = new WindowTrigger(assigner, handler);

    // No events processed
    const results = trigger.advanceWatermark(new Timestamp(1500));
    expect(results).toHaveLength(0);
    expect(handler).not.toHaveBeenCalled();
  });
});
