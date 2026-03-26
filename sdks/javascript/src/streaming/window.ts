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
  createWindowSpec,
} from './types';

/**
 * State for a single active window.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 */
export interface WindowState<K, V> {
  /** Unique window identifier. */
  windowId: string;
  /** Partition key for this window. */
  key: K;
  /** Window start time (inclusive). */
  start: Timestamp;
  /** Window end time (exclusive). */
  end: Timestamp;
  /** Events accumulated in this window. */
  events: StreamEvent<V>[];
  /** Maximum event timestamp observed. */
  maxTimestamp?: Timestamp;
  /** Whether the window has been closed. */
  isClosed: boolean;
  /** Whether an early pane has been fired. */
  earlyFired: boolean;
  /** Whether the on-time pane has been fired. */
  onTimeFired: boolean;
  /**
   * Add an event to the window if it falls within the time range.
   *
   * @param event - The event to add.
   * @returns `true` if the event was accepted.
   */
  addEvent(event: StreamEvent<V>): boolean;
  /**
   * Check if the window contains no events.
   *
   * @returns `true` if the event list is empty.
   */
  isEmpty(): boolean;
  /**
   * Clear all events and mark the window as closed.
   */
  clear(): void;
}

/**
 * Create window state.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 * @param windowId - Unique window identifier.
 * @param key      - Partition key.
 * @param start    - Window start timestamp.
 * @param end      - Window end timestamp.
 * @returns A new {@link WindowState}.
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
 * Assigns events to windows based on the configured {@link WindowSpec}.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 */
export class WindowAssigner<K, V> {
  private readonly windows: Map<string, WindowState<K, V>> = new Map();
  private readonly keyWindows: Map<K, string[]> = new Map();

  /**
   * Create a new WindowAssigner.
   *
   * @param spec - The window specification defining type, size, and parameters.
   */
  constructor(public readonly spec: WindowSpec) {}

  /**
   * Assign an event to one or more windows.
   *
   * The assignment strategy depends on the window type:
   * - **Tumbling**: Assigned to exactly one window.
   * - **Sliding**: May be assigned to multiple overlapping windows.
   * - **Session**: Assigned to an existing session window or a new one.
   *
   * @param event - The stream event.
   * @param key   - The partition key.
   * @returns Array of windows the event was assigned to.
   */
  assign(event: StreamEvent<V>, key: K): WindowState<K, V>[] {
    const windows: WindowState<K, V>[] = [];

    switch (this.spec.type) {
      case WindowType.Tumbling:
        const tumblingWindow = this.assignTumbling(event, key);
        if (tumblingWindow) windows.push(tumblingWindow);
        break;
      case WindowType.Sliding:
        windows.push(...this.assignSliding(event, key));
        break;
      case WindowType.Session:
        const sessionWindow = this.assignSession(event, key);
        if (sessionWindow) windows.push(sessionWindow);
        break;
    }

    return windows;
  }

  /**
   * Assign to a tumbling window.
   * @internal
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
      const window = createWindowState<K, V>(
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
   * @internal
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
        const window = createWindowState<K, V>(
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
   * Assign to a session window (dynamic based on inactivity gap).
   * @internal
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
    }

    if (mergedWindow) {
      return mergedWindow;
    }

    const windowId = `${key}_session_${eventTs}`;
    const window = createWindowState<K, V>(
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

  /**
   * Add a window ID to the per-key index.
   * @internal
   */
  private addToKeyIndex(key: K, windowId: string): void {
    const existing = this.keyWindows.get(key) ?? [];
    existing.push(windowId);
    this.keyWindows.set(key, existing);
  }

  /**
   * Get windows that should fire based on the current watermark.
   *
   * Returns windows whose end time is past the watermark and that have
   * not yet been closed.
   *
   * @param watermark - The current watermark timestamp.
   * @returns Array of windows ready to fire.
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
   * Remove all closed windows from internal storage.
   *
   * @returns The number of windows removed.
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
 * Triggers window firing with custom aggregation logic.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 * @typeParam R - The result type of the window handler.
 */
export class WindowTrigger<K, V, R> {
  private readonly results: R[] = [];

  /**
   * Create a new WindowTrigger.
   *
   * @param assigner    - The window assigner that tracks window state.
   * @param handler     - Function called when a window fires.
   * @param earlyFiring - Optional duration after window start for early firing.
   */
  constructor(
    public readonly assigner: WindowAssigner<K, V>,
    public readonly handler: (events: StreamEvent<V>[], info: WindowInfo) => R,
    private readonly earlyFiring?: Duration | null
  ) {}

  /**
   * Process an event and return any triggered results.
   *
   * Assigns the event to windows and optionally fires early panes
   * if early firing is configured.
   *
   * @param event - The stream event.
   * @param key   - The partition key.
   * @returns Array of results from any fired panes.
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
   * Advance the watermark and fire any completed windows.
   *
   * @param watermark - The new watermark timestamp.
   * @returns Array of results from fired windows.
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
   * Fire a window and invoke the handler.
   *
   * @param window - The window to fire.
   * @param pane   - The pane classification.
   * @returns The handler result, or `undefined` if the window is empty.
   * @internal
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
 * Convenience class for tumbling (non-overlapping) windows.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 * @typeParam R - The result type of the window handler.
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
export class TumblingWindow<K, V, R> {
  private readonly trigger: WindowTrigger<K, V, R>;

  /**
   * Create a tumbling window.
   *
   * @param size           - The fixed window size.
   * @param handler        - Aggregation function called when the window fires.
   * @param lateTolerance  - Optional tolerance for late-arriving data.
   */
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

  /**
   * Process an event and return any triggered results.
   *
   * @param event - The stream event.
   * @param key   - The partition key.
   * @returns Array of results from any fired panes.
   */
  process(event: StreamEvent<V>, key: K): R[] {
    return this.trigger.process(event, key);
  }

  /**
   * Advance the watermark and fire completed windows.
   *
   * @param watermark - The new watermark timestamp.
   * @returns Array of results from fired windows.
   */
  advanceWatermark(watermark: Timestamp): R[] {
    return this.trigger.advanceWatermark(watermark);
  }
}

/**
 * Convenience class for sliding (overlapping) windows.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 * @typeParam R - The result type of the window handler.
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

  /**
   * Create a sliding window.
   *
   * @param size          - The window size.
   * @param slide         - The slide interval (must be less than size).
   * @param handler       - Aggregation function called when the window fires.
   * @param lateTolerance - Optional tolerance for late-arriving data.
   */
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

  /**
   * Process an event and return any triggered results.
   *
   * @param event - The stream event.
   * @param key   - The partition key.
   * @returns Array of results from any fired panes.
   */
  process(event: StreamEvent<V>, key: K): R[] {
    return this.trigger.process(event, key);
  }

  /**
   * Advance the watermark and fire completed windows.
   *
   * @param watermark - The new watermark timestamp.
   * @returns Array of results from fired windows.
   */
  advanceWatermark(watermark: Timestamp): R[] {
    return this.trigger.advanceWatermark(watermark);
  }
}

/**
 * Create a tumbling window with a default pass-through handler.
 *
 * @param size - The fixed window size.
 * @returns A {@link TumblingWindow} that collects events into arrays.
 */
export function window<K, V>(
  size: Duration
): TumblingWindow<K, V, StreamEvent<V>[]> {
  return new TumblingWindow<K, V, StreamEvent<V>[]>(
    size,
    (events) => events
  );
}

/**
 * Create a tumbling window.
 *
 * @param size    - The fixed window size.
 * @param handler - Aggregation function called when the window fires.
 * @returns A {@link TumblingWindow}.
 */
export function tumbling<K, V, R>(
  size: Duration,
  handler: (events: StreamEvent<V>[], info: WindowInfo) => R
): TumblingWindow<K, V, R> {
  return new TumblingWindow<K, V, R>(size, handler);
}

/**
 * Create a sliding window.
 *
 * @param size    - The window size.
 * @param slide   - The slide interval.
 * @param handler - Aggregation function called when the window fires.
 * @returns A {@link SlidingWindow}.
 */
export function sliding<K, V, R>(
  size: Duration,
  slide: Duration,
  handler: (events: StreamEvent<V>[], info: WindowInfo) => R
): SlidingWindow<K, V, R> {
  return new SlidingWindow<K, V, R>(size, slide, handler);
}

/**
 * Create a session window.
 *
 * @param gap     - The inactivity gap that closes a session.
 * @param handler - Aggregation function called when the window fires.
 * @returns A {@link SessionWindow}.
 */
export function session<K, V, R>(
  gap: Duration,
  handler: (events: StreamEvent<V>[], info: WindowInfo) => R
): SessionWindow<K, V, R> {
  return new SessionWindow<K, V, R>(gap, handler);
}

/**
 * Convenience class for session (dynamic) windows.
 *
 * @typeParam K - The partition key type.
 * @typeParam V - The event payload type.
 * @typeParam R - The result type of the window handler.
 *
 * @example
 * ```typescript
 * const window = new SessionWindow<string, Event>(
 *   Duration.fromMinutes(5),  // 5 minute inactivity gap
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

  /**
   * Create a session window.
   *
   * @param gap           - The inactivity gap that closes a session.
   * @param handler       - Aggregation function called when the window fires.
   * @param lateTolerance - Optional tolerance for late-arriving data.
   */
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

  /**
   * Process an event and return any triggered results.
   *
   * @param event - The stream event.
   * @param key   - The partition key.
   * @returns Array of results from any fired panes.
   */
  process(event: StreamEvent<V>, key: K): R[] {
    return this.trigger.process(event, key);
  }

  /**
   * Advance the watermark and fire completed windows.
   *
   * @param watermark - The new watermark timestamp.
   * @returns Array of results from fired windows.
   */
  advanceWatermark(watermark: Timestamp): R[] {
    return this.trigger.advanceWatermark(watermark);
  }
}
