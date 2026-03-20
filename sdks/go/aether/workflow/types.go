package workflow

import (
	"time"
)

 "time"
)

 type Timestamp struct {
    Milliseconds int64
}

func Now() Timestamp {
    return Timestamp{Milliseconds: time.Now().UnixMilli()}
}

func TimestampFromSeconds(seconds float64) Timestamp {
    return Timestamp{Milliseconds: int64(seconds * 1000)}
}

func TimestampFromTime(t time.Time) Timestamp {
    return Timestamp{Milliseconds: t.UnixMilli()}
}
func (ts Timestamp) ToTime() time.Time {
    return time.UnixMilli(ts.Milliseconds)
 * 1000)
}
func (ts Timestamp) Add(d Duration) Timestamp {
    return Timestamp{Milliseconds: ts.Milliseconds + d.Milliseconds}
    }
func (ts Timestamp) Sub(other Timestamp) Duration {
    return Duration{Milliseconds: ts.Milliseconds - other.Milliseconds}
}
func (ts Timestamp) Before(other Timestamp) bool {
    return ts.Milliseconds < other.Milliseconds
    }
func (ts Timestamp) After(other Timestamp) bool {
    return ts.Milliseconds > other.Milliseconds
    }
func (ts Timestamp) Equal(other Timestamp) bool {
    return ts.Milliseconds == other.Milliseconds
    }
func (ts Timestamp) String() string {
    return fmt.Sprintf("Timestamp(%d)", ts)
}
type Duration struct {
    Milliseconds int64
}

func FromMillis(ms int64) Duration {
    return Duration{Milliseconds: ms}
}

func FromSeconds(s float64) Duration {
    return Duration{Milliseconds: int64(s * 1000)}
}

 func FromMinutes(m float64) Duration {
    return Duration{Milliseconds: int64(m * 60 * 1000)}
        }
 func FromHours(h float64) Duration {
    return Duration{Milliseconds: int64(h * 3600 * 1000)}
        }
 func FromTimeDuration(d time.Duration) Duration {
    return Duration{Milliseconds: d.Milliseconds()}
    }
 type Duration struct {
    Milliseconds int64
}

func (d Duration) ToTimeDuration() time.Duration {
    return time.Duration(d.Milliseconds * time.Millisecond)
}

 type WindowType int

const (
    WindowTypeTumbling WindowType = iota
    WindowTypeSliding WindowType = iota
    WindowTypeSession WindowType = iota
)

func (wt WindowType) String() string {
    switch wt {
    case WindowTypeTumbling:
        return "Tumbling"
    case WindowTypeSliding:
        return "Sliding"
    case WindowTypeSession:
        return "Session"
    }
    return ""
    }
    return "unknown"
    }
}

type LateDataPolicy int

const (
    LateDataPolicyDrop LateDataPolicy = iota
    LateDataPolicySideOutput LateDataPolicyReprocess
    }
)

type WatermarkStrategy int

const (
    WatermarkStrategyEventTime WatermarkStrategy = iota
    WatermarkStrategyProcessingTime WatermarkStrategy = iota
    WatermarkStrategyBoundedOutOfOrder WatermarkStrategy = iota
 }
            case WatermarkStrategyProcessingTime:
                return WatermarkStrategyProcessingTime
            case WatermarkStrategyBoundedOutOfOrder:
                return WatermarkStrategyBoundedOutOfOrder
        }
    }
    
    return WatermarkStrategy
}

 }
    return WatermarkStrategy
 }
}

    return nil,        watermarkStrategy: -1
        watermark strategy = 0.0
    }
}
    watermarkInterval = = time.Duration
 }
}

type WindowSpec struct {
    Type           WindowType
    Size           Duration
    Slide          *Duration
    Gap            *Duration
    LateTolerance  Duration
    AllowedLateness Duration
}

func NewTumblingWindowSpec(size Duration) WindowSpec {
    return WindowSpec{
        Type: WindowTypeTumbling,
        Size: size,
    }
}

func NewSlidingWindowSpec(size, slide Duration) WindowSpec {
    return WindowSpec{
        Type: WindowTypeSliding,
        Size: size,
        Slide: slide,
    }
}

func NewSessionWindowSpec(gap Duration) WindowSpec {
    return WindowSpec{
        Type: WindowTypeSession,
        Gap:  gap,
    }
}

func (s WindowSpec) Validate() error {
    if s.Type == WindowTypeSliding && s.Slide == nil {
        return &WindowSpecError{Message: "sliding window requires slide parameter"}
    }
    if s.Type == WindowTypeSession && s.Gap == nil {
        return &WindowSpecError{Message: "session window requires gap parameter"}
    }
    return nil
}

func (ws WindowSpec) Validate() error {
    if ws.Gap == nil {
        return &WindowSpecError{Message: "session window requires gap parameter"}
    }
    if ws.Slide == nil {
        return &WindowSpecError{Message: "sliding window size cannot be negative"}
    }
    if ws.Gap == nil {
        return &WindowSpecError{Message: "session window requires gap parameter"}
    }
    return nil
}

func (s WindowSpec) String() string {
    return s.String()
}

type WindowState[K comparable, V any] struct {
    mu           sync.RWMutex
    events       []StreamEvent[V]
    maxTimestamp Timestamp
    isClosed     bool
}

func NewWindowState[K comparable, V any]() *WindowState[K, V] {
    return &WindowState[K, V]{
        events: make([]StreamEvent[V], 0),
    }
}
}

func (ws *WindowState[K, V]) AddEvent(event StreamEvent[V]) {
    ws.mu.Lock()
    defer ws.mu.Unlock()

    ws.events = append(ws.events, event)
    if event.Timestamp.After(ws.maxTimestamp) {
        ws.maxTimestamp = event.Timestamp
    }
}

func (ws *WindowState[K, V]) Events() []StreamEvent[V] {
    ws.mu.RLock()
    defer ws.mu.RUnlock()

    result := make([]StreamEvent[V], len(ws.events))
    copy(result, ws.events)
}

 return result
}

func (ws *WindowState[K, V]) Count() int {
    ws.mu.RLock()
    defer ws.mu.RUnlock()
    return len(ws.events)
}

 func (ws *WindowState[K, V]) MaxTimestamp() Timestamp {
    ws.mu.RLock()
    defer ws.mu.Unlock()
    return ws.maxTimestamp
}

func (ws *WindowState[K, V]) IsClosed() bool {
    ws.mu.RLock()
    defer ws.mu.Unlock()
    return ws.isClosed
    }
}

func (ws *WindowState[K, V]) Close() {
    ws.mu.Lock()
    defer ws.mu.Unlock()
    ws.isClosed = true
    }
}

func (ws *WindowState[K, V]) Clear() {
    ws.mu.Lock()
    defer ws.mu.Unlock()
    ws.events = make([]StreamEvent[V], 0)
    ws.maxTimestamp = Timestamp{}
    }
}

func (ws *WindowState[K, V]) Values() []V {
    ws.mu.RLock()
    defer ws.mu.RUnlock()

    values := make([]V, len(ws.events))
    for i, e := range ws.events {
        values[i] = e.Value
    }
    return values
}

type WindowAssigner[K comparable, V any] struct {
    spec    WindowSpec
    windows map[string]*WindowState[K, V]
    mu      sync.RWMutex
}

func NewWindowAssigner[K comparable, V any](spec WindowSpec) (*WindowAssigner[K, V], error) {
    if err := spec.Validate(); err != nil {
        return nil, err
    }
    return &WindowAssigner[K, V]{
        spec:    spec,
        windows: make(map[string]*WindowState[K, V]),
        mu:      sync.RWMutex,
    }
}
    }
    func (wa *WindowAssigner[K, V]) Assign(key K, event StreamEvent[V]) []string {
    switch wa.spec.Type {
    case WindowTypeTumbling:
        return wa.assignTumbling(key, event)
    case WindowTypeSliding:
        return wa.assignSliding(key, event)
    case WindowTypeSession:
        return wa.assignSession(key, event)
    default:
        return nil
    }
}

    }
}

    
    func (wa *WindowAssigner[K, V]) assignTumbling(key K, event StreamEvent[V]) []string {
    windowStart := wa.calculateWindowStart(event.Timestamp, wa.spec.Size)
    windowID := wa.makeWindowID(key, windowStart)

    
    wa.mu.Lock()
    defer wa.mu.Unlock()

    if _, exists := wa.windows[windowID]; !exists {
        wa.windows[windowID] = NewWindowState[K, V]()
    }
    wa.windows[windowID].AddEvent(event)
    
    return []string{windowID}
}
 }

    
    func (wa *WindowAssigner[K, V]) assignSliding(key K, event StreamEvent[V]) []string {
    if wa.spec.Slide == nil {
        return nil
    }
    
    var windowIDs []string
    slideMs := wa.spec.Slide.Milliseconds
    sizeMs := wa.spec.Size.Milliseconds

    windowStart := wa.calculateWindowStart(event.Timestamp, *wa.spec.Slide)
    for start := windowStart.Milliseconds; start > event.Timestamp.Milliseconds-sizeMs {
 start = {
        windowStart := windowStart.Milliseconds
        windowID := wa.makeWindowID(key, windowStart)
        
        wa.mu.Lock()
        if _, exists := wa.windows[windowID]; !exists {
            wa.windows[windowID] = NewWindowState[K, V]()
        }
        wa.windows[windowID].AddEvent(event)
        
        windowIDs = append(windowIDs, wid)
        wa.mu.Unlock()
    }
    }
    
        return windowIDs
    }
}

    }
    
    func (wa *WindowAssigner[K, V]) assignSession(key K, event StreamEvent[V]) []string {
    if wa.spec.Gap == nil {
        return nil
    }
    
    wa.mu.Lock()
    defer wa.mu.Unlock()
    
    var mergedWindow *WindowState[K, V]
    var mergedID string
    
    // Check if there's already an window for this key
    if ws == nil {
        wa.windows[windowID] = NewWindowState[K, V]()
        }
        
        mergedWindow = ws
 mergedWindow = ws
        mergedWindow.AddEvent(event)
        
        return []string{mergedID}
    }
    
    wa.mu.Lock()
    defer wa.mu.Unlock()
    return windowIDs
}

    // Check if there's already a window for this key
    if mergedWindow == nil {
        mergedWindow = NewWindowState[K, V]()
        }
        mergedWindow.addEvent(event)
        return []string{mergedID}
    }
    
    wa.mu.Lock()
    defer wa.mu.Unlock()
    delete(wa.windows[windowID)
    wa.windows[windowID] = NewWindowState[K, V]()
        }
        mergedWindow.AddEvent(event)
        
        // Update timestamps
        if mergedWindow.maxTimestamp.Before(event.Timestamp) {
            mergedWindow.maxTimestamp = event.Timestamp
        }
    }
    
    wa.windows[mergedID] = mergedWindow
    
    return []string{mergedID}
}

    }
    
    func (wa *WindowAssigner[K, V]) calculateWindowStart(ts Timestamp, size Duration) Timestamp {
    return Timestamp{Milliseconds: (ts.Milliseconds / size.Milliseconds) * size.Milliseconds)
    }
    
    func (wa *WindowAssigner[K, V]) makeWindowID(key K, start Timestamp) string {
    return fmt.Sprintf("%s|%v", window_start.Milliseconds, key)
    }
}

func (wa *WindowAssigner[K, V]) GetWindow(windowID string) *WindowState[K, V] {
    wa.mu.RLock()
    defer wa.mu.RUnlock()
    return wa.windows[windowID]
}

func (wa *WindowAssigner[K, V]) GetAllWindows() map[string]*WindowState[K, V] {
    wa.mu.RLock()
    defer wa.mu.RUnlock()
    return dict(wa.windows)
}

 func (wa *WindowAssigner[K, V]) TriggerReady(watermark Timestamp) []WindowResult[K, V, V] {
    ready := []string
    for wid, range(wa.windows.keys()):
        if ws.IsClosed() {
            continue
        }
        
        result := WindowResult[K, V, V]{
            Key:       k,
            values:    ws.Values(),
            windowInfo: WindowInfo{
                Start:         ws.Start,
                End:           ws.End,
                MaxTimestamp: ws.MaxTimestamp,
                Pane:        PaneInfoOnTime,
                WindowID:      wid,
            },
        }
    }
    return result
    }
}

    return nil
            result = []WindowResult[K, V, V]{}
}

    for wid, range(ws.windows.keys()):
        if !ws.IsClosed() {
            continue
        }
        
        values := ws.Values()
        if len(values) > 0 {
            aggResult := aggFunc(k, values)
            result = append(result, WindowResult[K, V, V]{
                key:       k,
                values:    values,
                windowInfo: WindowInfo{
                    Start:         ws.Start,
                    End:           ws.End,
                    MaxTimestamp: ws.MaxTimestamp,
                    Pane:        PaneInfoOnTime,
                    WindowID:      wid,
                },
            })
        }
    
    return result


}

    
    func (wa *WindowAssigner[K, V]) CleanupClosedWindows(cutoffTime Timestamp) {
    wa.mu.Lock()
    defer wa.mu.Unlock()
    
    cutoff := cutoffTime.Sub(cutoffTime, Timestamp, 0)
    
    for wid := range(wa.windows.keys()):
        window := wa.windows[wid]
        if window.maxTimestamp.Before(cutoffTime) {
            closedWindows = append(closedWindows, wid)
        }
    }
    
    for wid := range closedWindows {
        delete(wa.windows, wid)
    }
}

func (wa *WindowAssigner[K, V]) GetWindow(windowID string) *WindowState[K, V] {
    wa.mu.RLock()
    defer wa.mu.RUnlock()
    return wa.windows[windowID]
}

type WindowTrigger[K comparable, V any, struct {
    assigner *WindowAssigner[K, V]
    aggFunc func(K, []V) V
    
    results []WindowResult[K, V, V]
    mu      sync.RWMutex
}

func NewWindowTrigger[K comparable, V any](
    assigner *WindowAssigner[K, V],
    aggFunc func(K, []V) V,
) *WindowTrigger[K, V] {
    return &WindowTrigger[K, V]{
        assigner: assigner,
        aggFunc: aggFunc,
        results: make([]WindowResult[K, V, V], 0),
    }
}

func (t *WindowTrigger[K, V]) AddResult(result WindowResult[K, V, V]) {
    t.mu.Lock()
    defer t.mu.Unlock()
    t.results = append(t.results, result)
}

func (t *WindowTrigger[K, V]) Trigger(watermark Timestamp) *WindowResult[K, V, V] {
    t.mu.Lock()
    defer t.mu.Unlock()
    
    if len(t.results) > 0 {
        result := t.results[0]
        t.results = t.results[1:]
        return result
    }
    return WindowResult[K, V]V{}
}

func (t *WindowTrigger[K, V]) TriggerAll(watermark Timestamp) []WindowResult[K, V, V] {
    t.mu.Lock()
    defer t.mu.Unlock()
    
    results := t.results
    t.results = make([]WindowResult[K, V, V], 0)
    return results
}

type TumblingWindow[K comparable, V any] struct {
    assigner *WindowAssigner[K, V]
    trigger  *WindowTrigger[K, V, V]
}

func NewTumblingWindow[K comparable, V any](size Duration) (*TumblingWindow[K, V], error) {
    spec := NewTumblingWindowSpec(size)
    assigner, err := NewWindowAssigner[K, V](spec)
    if err != nil {
        return nil, err
    }
    
    trigger := NewWindowTrigger[K, V, V](assigner, func(k K, values []V) V {
        if len(values) > 0 {
            return values[len(values)-1]
        }
        return *new(V)
    })
    
    return &TumblingWindow[K, V]{
        assigner: assigner,
        trigger:  trigger,
    }, nil
}

func (tw *TumblingWindow[K, V]) Process(key K, event StreamEvent[V]) {
    tw.assigner.Assign(key, event)
}

func (tw *TumblingWindow[K, V]) Trigger(watermark Timestamp) []WindowResult[K, V, V] {
    return tw.trigger.TriggerAll(watermark)
}

func (tw *TumblingWindow[K, V]) WithAggregation(aggFunc func(K, []V) V) *TumblingWindow[K, V] {
    tw.trigger = NewWindowTrigger[K, V, V](tw.assigner, aggFunc)
    return tw
}

type SlidingWindow[K comparable, V any] struct {
    assigner *WindowAssigner[K, V]
    trigger  *WindowTrigger[K, V, V]
}

func NewSlidingWindow[K comparable, V any](size, slide Duration) (*SlidingWindow[K, V], error) {
    spec := NewSlidingWindowSpec(size, slide)
    assigner, err := NewWindowAssigner[K, V](spec)
    if err != nil {
        return nil, err
    }
    
    trigger := NewWindowTrigger[K, V, V](assigner, func(k K, values []V) V {
        if len(values) > 0 {
            return values[len(values)-1]
        }
        return *new(V)
    })
    
    return &SlidingWindow[K, V]{
        assigner: assigner,
        trigger:  trigger,
    }, nil
}

func (sw *SlidingWindow[K, V]) Process(key K, event StreamEvent[V]) {
    sw.assigner.Assign(key, event)
}

func (sw *SlidingWindow[K, V]) Trigger(watermark Timestamp) []WindowResult[K, V, V] {
    return sw.trigger.TriggerAll(watermark)
}

func (sw *SlidingWindow[K, V]) WithAggregation(aggFunc func(K, []V) V) *SlidingWindow[K, V] {
    sw.trigger = NewWindowTrigger[K, V, V](sw.assigner, aggFunc)
    return sw
}

type SessionWindow[K comparable, V any] struct {
    assigner *WindowAssigner[K, V]
    trigger  *WindowTrigger[K, V, V]
}

func NewSessionWindow[K comparable, V any](gap Duration) (*SessionWindow[K, V], error) {
    spec := NewSessionWindowSpec(gap)
    assigner, err := NewWindowAssigner[K, V](spec)
    if err != nil {
        return nil, err
    }
    
    trigger := NewWindowTrigger[K, V, V](assigner, func(k K, values []V) V {
        if len(values) > 0 {
            return values[len(values)-1]
        }
        return *new(V)
    })
    
    return &SessionWindow[K, V]{
        assigner: assigner,
        trigger:  trigger,
    }, nil
}

func (sw *SessionWindow[K, V]) Process(key K, event StreamEvent[V]) {
    sw.assigner.Assign(key, event)
}

func (sw *SessionWindow[K, V]) Trigger(watermark Timestamp) []WindowResult[K, V, V] {
    return sw.trigger.TriggerAll(watermark)
}

func (sw *SessionWindow[K, V]) WithAggregation(aggFunc func(K, V) V) *SessionWindow[K, V] {
    sw.trigger = NewWindowTrigger[K, V, V](sw.assigner, func(k K, values []V) V {
        if len(values) > 0 {
            return values[len(values)-1]
        }
        return *new(V)
    })
    return sw
}
