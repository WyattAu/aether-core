package streaming

import (
	"sync"
)

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
	return result
}

func (ws *WindowState[K, V]) Count() int {
	ws.mu.RLock()
	defer ws.mu.RUnlock()
	return len(ws.events)
}

func (ws *WindowState[K, V]) MaxTimestamp() Timestamp {
	ws.mu.RLock()
	defer ws.mu.RUnlock()
	return ws.maxTimestamp
}

func (ws *WindowState[K, V]) IsClosed() bool {
	ws.mu.RLock()
	defer ws.mu.RUnlock()
	return ws.isClosed
}

func (ws *WindowState[K, V]) Close() {
	ws.mu.Lock()
	defer ws.mu.Unlock()
	ws.isClosed = true
}

func (ws *WindowState[K, V]) Clear() {
	ws.mu.Lock()
	defer ws.mu.Unlock()
	ws.events = make([]StreamEvent[V], 0)
	ws.maxTimestamp = Timestamp{}
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

type WindowResult[K comparable, V any] struct {
	Key        K
	Values     []V
	Events     []StreamEvent[V]
	WindowInfo WindowInfo
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
	}, nil
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

func (wa *WindowAssigner[K, V]) assignSliding(key K, event StreamEvent[V]) []string {
	if wa.spec.Slide == nil {
		return nil
	}

	var windowIDs []string
	slideMs := wa.spec.Slide.Milliseconds
	sizeMs := wa.spec.Size.Milliseconds

	windowStart := wa.calculateWindowStart(event.Timestamp, *wa.spec.Slide)

	for start := windowStart.Milliseconds; start > event.Timestamp.Milliseconds-sizeMs; start -= slideMs {
		if start < 0 {
			continue
		}
		wid := wa.makeWindowID(key, Timestamp{Milliseconds: start})

		wa.mu.Lock()
		if _, exists := wa.windows[wid]; !exists {
			wa.windows[wid] = NewWindowState[K, V]()
		}
		wa.windows[wid].AddEvent(event)
		wa.mu.Unlock()

		windowIDs = append(windowIDs, wid)
	}

	return windowIDs
}

func (wa *WindowAssigner[K, V]) assignSession(key K, event StreamEvent[V]) []string {
	if wa.spec.Gap == nil {
		return nil
	}

	wa.mu.Lock()
	defer wa.mu.Unlock()

	var mergedWindow *WindowState[K, V]
	var mergedID string

	for wid, ws := range wa.windows {
		if ws.IsClosed() {
			continue
		}

		gapMs := wa.spec.Gap.Milliseconds
		eventTs := event.Timestamp.Milliseconds
		maxTs := ws.MaxTimestamp().Milliseconds

		if eventTs <= maxTs+gapMs && eventTs+gapMs >= maxTs-gapMs {
			if mergedWindow == nil {
				mergedWindow = ws
				mergedID = wid
			} else {
				for _, e := range ws.Events() {
					mergedWindow.AddEvent(e)
				}
				delete(wa.windows, wid)
			}
		}
	}

	if mergedWindow != nil {
		mergedWindow.AddEvent(event)
		return []string{mergedID}
	}

	windowID := wa.makeWindowID(key, event.Timestamp)
	wa.windows[windowID] = NewWindowState[K, V]()
	wa.windows[windowID].AddEvent(event)

	return []string{windowID}
}

func (wa *WindowAssigner[K, V]) calculateWindowStart(ts Timestamp, size Duration) Timestamp {
	return Timestamp{Milliseconds: (ts.Milliseconds / size.Milliseconds) * size.Milliseconds}
}

func (wa *WindowAssigner[K, V]) makeWindowID(key K, start Timestamp) string {
	return string(any(key).(string)) + "_" + string(start.Milliseconds)
}

func (wa *WindowAssigner[K, V]) GetWindow(windowID string) *WindowState[K, V] {
	wa.mu.RLock()
	defer wa.mu.RUnlock()
	return wa.windows[windowID]
}

func (wa *WindowAssigner[K, V]) GetAllWindows() map[string]*WindowState[K, V] {
	wa.mu.RLock()
	defer wa.mu.RUnlock()

	result := make(map[string]*WindowState[K, V], len(wa.windows))
	for k, v := range wa.windows {
		result[k] = v
	}
	return result
}

func (wa *WindowAssigner[K, V]) RemoveWindow(windowID string) {
	wa.mu.Lock()
	defer wa.mu.Unlock()
	delete(wa.windows, windowID)
}

func (wa *WindowAssigner[K, V]) TriggerReady(watermark Timestamp) []string {
	wa.mu.RLock()
	defer wa.mu.RUnlock()

	var ready []string
	for wid, ws := range wa.windows {
		if ws.IsClosed() {
			continue
		}

		windowEnd := wa.calculateWindowEnd(wid)
		if watermark.Milliseconds >= windowEnd.Milliseconds {
			ready = append(ready, wid)
		}
	}
	return ready
}

func (wa *WindowAssigner[K, V]) calculateWindowEnd(windowID string) Timestamp {
	return Timestamp{}
}

type TriggerResult[K comparable, V any, R any] struct {
	Key    K
	Result R
	Window WindowInfo
}

type WindowTrigger[K comparable, V any, R any] struct {
	assigner *WindowAssigner[K, V]
	aggFunc  func(K, []V) R
}

func NewWindowTrigger[K comparable, V any, R any](
	assigner *WindowAssigner[K, V],
	aggFunc func(K, []V) R,
) *WindowTrigger[K, V, R] {
	return &WindowTrigger[K, V, R]{
		assigner: assigner,
		aggFunc:  aggFunc,
	}
}

func (wt *WindowTrigger[K, V, R]) Process(key K, event StreamEvent[V]) []TriggerResult[K, V, R] {
	wt.assigner.Assign(key, event)
	return nil
}

func (wt *WindowTrigger[K, V, R]) TriggerWindow(windowID string, key K, info WindowInfo) *TriggerResult[K, V, R] {
	ws := wt.assigner.GetWindow(windowID)
	if ws == nil || ws.Count() == 0 {
		return nil
	}

	result := wt.aggFunc(key, ws.Values())
	ws.Close()

	return &TriggerResult[K, V, R]{
		Key:    key,
		Result: result,
		Window: info,
	}
}

func (wt *WindowTrigger[K, V, R]) TriggerAll(watermark Timestamp) []TriggerResult[K, V, R] {
	ready := wt.assigner.TriggerReady(watermark)
	var results []TriggerResult[K, V, R]

	for _, wid := range ready {
		ws := wt.assigner.GetWindow(wid)
		if ws == nil || ws.Count() == 0 {
			continue
		}

		values := ws.Values()
		if len(values) == 0 {
			continue
		}

		result := wt.aggFunc(*new(K), values)
		ws.Close()

		results = append(results, TriggerResult[K, V, R]{
			Result: result,
		})
	}

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

func (tw *TumblingWindow[K, V]) Trigger(watermark Timestamp) []TriggerResult[K, V, V] {
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

func (sw *SlidingWindow[K, V]) Trigger(watermark Timestamp) []TriggerResult[K, V, V] {
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

func (sw *SessionWindow[K, V]) Trigger(watermark Timestamp) []TriggerResult[K, V, V] {
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
