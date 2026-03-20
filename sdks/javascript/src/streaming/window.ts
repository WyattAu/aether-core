/**
 * Windowing Functions
 *
 * Time-based windowing for stream processing:
 * - Tumbling windows: Fixed-size, non-overlapping
 * - Sliding windows: Fixed-size, overlapping
 * - Session windows: Dynamic size based on activity gaps
 *
 * @module aether/streaming/window
 */

import {
  Duration,
  Timestamp,
  WindowType,
  PaneInfo,
  WindowSpec,
  WindowInfo,
  StreamEvent,
  createWindowInfo,
} from './types';

/**
 * State for a single window.
 */
export interface WindowState<K, V> {
  /** Window identifier */
  windowId: string;
  /** Partition key */
  key: K;
  /** Window start time */
  start: Timestamp;
  /** Window end time */
  end: Timestamp;
  /** Events in window */
  events: StreamEvent<V>[];
  /** Maximum timestamp seen */
  maxTimestamp?: Timestamp;
  /** Whether window is closed */
  isClosed: boolean;
  /** Early firing flag */
  earlyFired: boolean;
  /** On-time firing flag */
  onTimeFired: boolean;

  /**
   * Add event to window.
   */
  addEvent(event: StreamEvent<V>): boolean {
    if (this.isClosed) {
      return false;
    }
    if (
      event.timestamp.isBefore(this.start) ||
      !event.timestamp.isBefore(this.end)
    ) {
      return false;
    }
    this.events.push(event);
    if (!this.maxTimestamp || event.timestamp.isAfter(this.maxTimestamp)) {
      this.maxTimestamp = event.timestamp;
    }
    return true;
  }

  /**
   * Check if window has no events.
   */
  isEmpty(): boolean {
    return this.events.length === 0;
  }

  /**
   * Clear window events.
   */
  clear(): void {
    this.events = [];
    this.isClosed = true;
  }
}

/**
 * Create window state.
 */
export function createWindowState<K, V>(
  windowId: string,
  key: K,
  start: Timestamp,
  end: Timestamp
): WindowState<K, V> {
  return {
    windowId,
    key,
    start,
    end,
    events: [],
    maxTimestamp: undefined,
    isClosed: false,
    earlyFired: false,
    onTimeFired: false,
    addEvent(event: StreamEvent<V>): boolean {
      if (this.isClosed) {
        return false;
      }
      if (
        event.timestamp.isBefore(this.start) ||
        !event.timestamp.isBefore(this.end)
      ) {
        return false;
      }
      this.events.push(event);
      if (!this.maxTimestamp || event.timestamp.isAfter(this.maxTimestamp)) {
        this.maxTimestamp = event.timestamp;
      }
      return true;
    },
    isEmpty(): boolean {
      return this.events.length === 0;
    },
    clear(): void {
      this.events = [];
      this.isClosed = true;
    },
  };
}

/**
 * Assigns events to windows.
 */
export class WindowAssigner<K, V> {
  private readonly windows: Map<string, WindowState<K, V>> = new Map();
  private readonly keyWindows: Map<K, string[]> = new Map();

  constructor(public readonly spec: WindowSpec) {}

  /**
   * Assign event to one or more windows.
   */
  assign(event: StreamEvent<V>, key: K): WindowState<K, V>[] {
    const windows: WindowState<K, V>[] = [];

    switch (this.spec.type) {
      case WindowType.Tumbling:
        const window = this.assignTumbling(event, key);
        if (window) windows.push(window);
        break;
      case WindowType.Sliding:
        windows.push(...this.assignSliding(event, key));
        break;
      case WindowType.Session:
        const window = this.assignSession(event, key);
        if (window) windows.push(window);
        break;
    }

    return windows;
  }

  /**
   * Assign to tumbling window.
   */
  private assignTumbling(
    event: StreamEvent<V>,
    key: K
  ): WindowState<K, V> | undefined {
    const sizeMs = this.spec.size.milliseconds;
    const startMs = Math.floor(event.timestamp.milliseconds / sizeMs) * sizeMs;
    const endMs = startMs + sizeMs;

    const windowId = `${key}_${startMs}`;

    if (!this.windows.has(windowId)) {
      const window = createWindowState(
        windowId,
        key,
        new Timestamp(startMs),
        new Timestamp(endMs)
      );
      this.windows.set(windowId, window);
      this.addToKeyIndex(key, windowId);
    }

    const window = this.windows.get(windowId)!;
    window.addEvent(event);
    return window;
  }

  /**
   * Assign to overlapping sliding windows.
   */
  private assignSliding(event: StreamEvent<V>, key: K): WindowState<K, V>[] {
    const windows: WindowState<K, V>[] = [];
    const sizeMs = this.spec.size.milliseconds;
    const slideMs = this.spec.slide?.milliseconds ?? sizeMs;

    const eventTs = event.timestamp.milliseconds;

    let windowStart = Math.floor(eventTs / slideMs) * slideMs;
    while (windowStart + sizeMs > eventTs && windowStart >= 0) {
      windowStart -= slideMs;
    }
    windowStart += slideMs;

    let currentStart = windowStart;
    while (currentStart <= eventTs) {
      const windowId = `${key}_${currentStart}`;

      if (!this.windows.has(windowId)) {
        const window = createWindowState(
          windowId,
          key,
          new Timestamp(currentStart),
          new Timestamp(currentStart + sizeMs)
        );
        this.windows.set(windowId, window);
        this.addToKeyIndex(key, windowId);
      }

      const window = this.windows.get(windowId)!;
      if (window.addEvent(event)) {
        windows.push(window);
      }

      currentStart += slideMs;
    }

    return windows;
  }

  /**
   * Assign to session window (dynamic based on gap).
   */
  private assignSession(
    event: StreamEvent<V>,
    key: K
  ): WindowState<K, V> | undefined {
    const gapMs = this.spec.gap?.milliseconds ?? 0;
    const eventTs = event.timestamp.milliseconds;

    const keyWindowIds = this.keyWindows.get(key) ?? [];
    let mergedWindow: WindowState<K, V> | undefined;

    for (const windowId of [...keyWindowIds]) {
      const window = this.windows.get(windowId);
      if (!window || window.isClosed) {
        continue;
      }

      if (window.maxTimestamp) {
        const timeDiff = Math.abs(eventTs - window.maxTimestamp.milliseconds);
        if (timeDiff <= gapMs) {
          if (!mergedWindow) {
            window.addEvent(event);
            mergedWindow = window;
          } else {
            for (const evt of window.events) {
              mergedWindow.addEvent(evt);
            }
            window.isClosed = true;
          }
        }
    }

    if (mergedWindow) {
      return mergedWindow;
    }

    const windowId = `${key}_session_${eventTs}`;
    const window = createWindowState(
      windowId,
      key,
      new Timestamp(eventTs),
      new Timestamp(eventTs + gapMs + 1)
    );
    window.addEvent(event);
    this.windows.set(windowId, window);
    this.addToKeyIndex(key, windowId);

    return window;
  }

  private addToKeyIndex(key: K, windowId: string): void {
    const existing = this.keyWindows.get(key) ?? [];
    existing.push(windowId);
    this.keyWindows.set(key, existing);
  }

  /**
   * Get windows ready to fire based on watermark.
   */
  getTriggeredWindows(watermark: Timestamp): WindowState<K, V>[] {
    const triggered: WindowState<K, V>[] = [];

    for (const window of this.windows.values()) {
      if (window.isClosed) {
        continue;
      }

      if (!window.end.isAfter(watermark)) {
        continue;
      }

      window.onTimeFired = true;
      triggered.push(window);
    }

    return triggered;
  }

  /**
   * Remove closed windows.
   */
  cleanupClosed(): number {
    const toRemove: string[] = [];

    for (const [wid, window] of this.windows.entries()) {
      if (window.isClosed) {
        toRemove.push(wid);
      }
    }

    for (const wid of toRemove) {
      this.windows.delete(wid);
      for (const [key, windowIds] of this.keyWindows.entries()) {
        this.keyWindows.set(
          key,
          windowIds.filter((id) => id !== wid)
        );
      }
    }

    return toRemove.length;
  }
}

/**
 * Triggers window firing with custom logic.
 */
export class WindowTrigger<K, V, R> {
  private readonly results: R[] = [];

  constructor(
    public readonly assigner: WindowAssigner<K, V>,
    public readonly handler: (events: StreamEvent<V>[], info: WindowInfo) => R,
    private readonly earlyFiring?: Duration
 null  {}

  /**
   * Process event and return any triggered results.
   */
  process(event: StreamEvent<V>, key: K): R[] {
    const results: R[] = [];
    const windows = this.assigner.assign(event, key);

    if (this.earlyFiring) {
      for (const window of windows) {
        if (!window.earlyFired && !window.isEmpty()) {
          if (window.maxTimestamp) {
            const elapsed =
              event.timestamp.milliseconds - window.start.milliseconds;
            if (elapsed >= this.earlyFiring.milliseconds) {
              const result = this.fireWindow(window, PaneInfo.Early);
              if (result !== undefined) {
                results.push(result);
              }
              window.earlyFired = true;
            }
          }
        }
      }
    }

    return results;
  }

  /**
   * Advance watermark and fire completed windows.
   */
  advanceWatermark(watermark: Timestamp): R[] {
    const results: R[] = [];
    const triggered = this.assigner.getTriggeredWindows(watermark);

    for (const window of triggered) {
      if (!window.isEmpty()) {
        const pane = window.onTimeFired ? PaneInfo.Late : PaneInfo.OnTime;
        const result = this.fireWindow(window, pane);
        if (result !== undefined) {
          results.push(result);
        }
      }
    }

    return results;
  }

  /**
   * Fire window and return result.
   */
  private fireWindow(
    window: WindowState<K, V>,
    pane: PaneInfo
  ): R | undefined {
    if (window.isEmpty()) {
      return undefined;
    }

    const info = createWindowInfo(
      window.start,
      window.end,
      window.maxTimestamp ?? window.start,
      pane,
      window.windowId
    );

    return this.handler([...window.events], info);
  }
}

/**
 * Convenience class for tumbling windows.
 *
 * @example
 * ```typescript
 * const window = new TumblingWindow<string, Event>(
 *   Duration.fromMinutes(5),
 *   (events, info) => {
 *     // Process events in 5-minute tumbling window
 *     return aggregate(events);
 *   }
 * );
 *
 * const result = window.process(event, key);
 * ```
 */
export class TumblingWindow<K, V, R = {
  private readonly trigger: WindowTrigger<K, V, R>;

  constructor(
    size: Duration,
    handler: (events: StreamEvent<V>[], info: WindowInfo) => R,
    lateTolerance?: Duration
  ) {
    const spec = createWindowSpec(WindowType.Tumbling, size, {
      lateTolerance: lateTolerance ?? Duration.fromMillis(0),
    });
    this.trigger = new WindowTrigger(
      new WindowAssigner(spec),
      handler
    );
  }

  process(event: StreamEvent<V>, key: K): R[] {
    return this.trigger.process(event, key);
  }

  advanceWatermark(watermark: Timestamp): R[] {
    return this.trigger.advanceWatermark(watermark);
  }
}

/**
 * Convenience class for sliding windows.
 *
 * @example
 * ```typescript
 * const window = new SlidingWindow<string, Event>(
 *   Duration.fromMinutes(10),  // size
 *   Duration.fromMinutes(1),  // slide every minute
 *   (events, info) => {
 *     return aggregate(events);
 *   }
 * );
 *
 * const result = window.process(event, key);
 * ```
 */
export class SlidingWindow<K, V, R> {
  private readonly trigger: WindowTrigger<K, V, R>;

  constructor(
    size: Duration,
    slide: Duration,
    handler: (events: StreamEvent<V>[], info: WindowInfo) => R,
    lateTolerance?: Duration
  ) {
    const spec = createWindowSpec(WindowType.Sliding, size, {
      slide,
      lateTolerance: lateTolerance ?? Duration.fromMillis(0),
    });
    this.trigger = new WindowTrigger(
      new WindowAssigner(spec),
      handler
    );
  }
  process(event: StreamEvent<V>, key: K): R[] {
    return this.trigger.process(event, key);
  }

  advanceWatermark(watermark: Timestamp): R[] {
    return this.trigger.advanceWatermark(watermark);
  }
}

/**
 * Convenience class for session windows.
 *
 * @example
 * ```typescript
 * const window = new SessionWindow<string, Event>(
 *   Duration.fromMinutes(5),  // 5 minute gap
 *   (events, info) => {
 *     return aggregate(events);
 *   }
 * );
 *
 * const result = window.process(event, key);
 * ```
 */
export class SessionWindow<K, V, R> {
  private readonly trigger: WindowTrigger<K, V, R>;

  constructor(
    gap: Duration,
    handler: (events: StreamEvent<V>[], info: WindowInfo) => R,
    lateTolerance?: Duration
  ) {
    const spec = createWindowSpec(WindowType.Session, Duration.fromMillis(0), {
      gap,
      lateTolerance: lateTolerance ?? Duration.fromMillis(0),
    });
    this.trigger = new WindowTrigger(
      new WindowAssigner(spec),
      handler
    );
  }
  process(event: StreamEvent<V>, key: K): R[] {
    return this.trigger.process(event, key);
  }

  advanceWatermark(watermark: Timestamp): R[] {
    return this.trigger.advanceWatermark(watermark);
  }
}
