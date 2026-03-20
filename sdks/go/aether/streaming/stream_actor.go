package streaming

import (
	"context"
	"sync"
	"sync/atomic"
	"time"
)

type StreamState struct {
	mu              sync.RWMutex
	keyedState      map[string][]byte
	operatorState   map[string][]byte
	watermark       Timestamp
	lastProcessed   Timestamp
	totalProcessed  int64
	totalEmitted    int64
	totalDropped    int64
	windows         map[string]*WindowStateInfo
}

type WindowStateInfo struct {
	WindowID  string
	Start     Timestamp
	End       Timestamp
	EventCount int
	IsClosed  bool
}

func NewStreamState() *StreamState {
	return &StreamState{
		keyedState:    make(map[string][]byte),
		operatorState: make(map[string][]byte),
		windows:       make(map[string]*WindowStateInfo),
	}
}

func (s *StreamState) GetKeyedState(key string) []byte {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if val, ok := s.keyedState[key]; ok {
		result := make([]byte, len(val))
		copy(result, val)
		return result
	}
	return nil
}

func (s *StreamState) SetKeyedState(key string, value []byte) {
	s.mu.Lock()
	defer s.mu.Unlock()

	stored := make([]byte, len(value))
	copy(stored, value)
	s.keyedState[key] = stored
}

func (s *StreamState) DeleteKeyedState(key string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.keyedState, key)
}

func (s *StreamState) GetOperatorState(key string) []byte {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if val, ok := s.operatorState[key]; ok {
		result := make([]byte, len(val))
		copy(result, val)
		return result
	}
	return nil
}

func (s *StreamState) SetOperatorState(key string, value []byte) {
	s.mu.Lock()
	defer s.mu.Unlock()

	stored := make([]byte, len(value))
	copy(stored, value)
	s.operatorState[key] = stored
}

func (s *StreamState) UpdateWatermark(ts Timestamp) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if ts.After(s.watermark) {
		s.watermark = ts
	}
}

func (s *StreamState) GetWatermark() Timestamp {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.watermark
}

func (s *StreamState) IncrementProcessed() {
	atomic.AddInt64(&s.totalProcessed, 1)
}

func (s *StreamState) IncrementEmitted() {
	atomic.AddInt64(&s.totalEmitted, 1)
}

func (s *StreamState) IncrementDropped() {
	atomic.AddInt64(&s.totalDropped, 1)
}

func (s *StreamState) GetMetrics() (processed, emitted, dropped int64) {
	return atomic.LoadInt64(&s.totalProcessed),
		atomic.LoadInt64(&s.totalEmitted),
		atomic.LoadInt64(&s.totalDropped)
}

func (s *StreamState) RegisterWindow(info *WindowStateInfo) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.windows[info.WindowID] = info
}

func (s *StreamState) UpdateWindow(windowID string, eventCount int) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if info, ok := s.windows[windowID]; ok {
		info.EventCount = eventCount
	}
}

func (s *StreamState) CloseWindow(windowID string) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if info, ok := s.windows[windowID]; ok {
		info.IsClosed = true
	}
}

func (s *StreamState) GetActiveWindows() []*WindowStateInfo {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var result []*WindowStateInfo
	for _, info := range s.windows {
		if !info.IsClosed {
			result = append(result, info)
		}
	}
	return result
}

func (s *StreamState) Snapshot() map[string][]byte {
	s.mu.RLock()
	defer s.mu.RUnlock()

	snapshot := make(map[string][]byte)
	for k, v := range s.keyedState {
		snapshot["keyed:"+k] = v
	}
	for k, v := range s.operatorState {
		snapshot["operator:"+k] = v
	}
	return snapshot
}

func (s *StreamState) Restore(snapshot map[string][]byte) {
	s.mu.Lock()
	defer s.mu.Unlock()

	s.keyedState = make(map[string][]byte)
	s.operatorState = make(map[string][]byte)

	for k, v := range snapshot {
		if len(k) > 6 && k[:6] == "keyed:" {
			s.keyedState[k[6:]] = v
		} else if len(k) > 9 && k[:9] == "operator:" {
			s.operatorState[k[9:]] = v
		}
	}
}

type StreamingStateHandle struct {
	state *StreamState
}

func NewStreamingStateHandle(state *StreamState) *StreamingStateHandle {
	return &StreamingStateHandle{state: state}
}

func (h *StreamingStateHandle) Get(ctx context.Context, key string) []byte {
	return h.state.GetKeyedState(key)
}

func (h *StreamingStateHandle) Set(ctx context.Context, key string, value []byte) {
	h.state.SetKeyedState(key, value)
}

func (h *StreamingStateHandle) Delete(ctx context.Context, key string) {
	h.state.DeleteKeyedState(key)
}

func (h *StreamingStateHandle) GetOperatorState(ctx context.Context, key string) []byte {
	return h.state.GetOperatorState(key)
}

func (h *StreamingStateHandle) SetOperatorState(ctx context.Context, key string, value []byte) {
	h.state.SetOperatorState(key, value)
}

func (h *StreamingStateHandle) GetWatermark() Timestamp {
	return h.state.GetWatermark()
}

type StreamActor[K comparable, V any] interface {
	Name() string
	Process(ctx context.Context, event StreamEvent[V]) error
	OnWatermark(ctx context.Context, watermark Timestamp) error
	OnStart(ctx context.Context) error
	OnStop(ctx context.Context) error
	GetState() *StreamState
}

type BaseStreamActor[K comparable, V any] struct {
	name        string
	config      StreamConfig
	state       *StreamState
	stateHandle *StreamingStateHandle
	backpressure *BackpressureController[StreamEvent[V]]
	running     atomic.Bool
	output      func(ctx context.Context, key K, value V) error
}

func NewBaseStreamActor[K comparable, V any](name string, config StreamConfig) *BaseStreamActor[K, V] {
	state := NewStreamState()
	return &BaseStreamActor[K, V]{
		name:    name,
		config:  config,
		state:   state,
		stateHandle: NewStreamingStateHandle(state),
		backpressure: NewBackpressureController[StreamEvent[V]](BackpressureConfig{
			Strategy:      BackpressureStrategyBuffer,
			BufferSize:    config.BufferCapacity,
			HighWatermark: 0.9,
			LowWatermark:  0.5,
		}),
	}
}

func (a *BaseStreamActor[K, V]) Name() string {
	return a.name
}

func (a *BaseStreamActor[K, V]) Process(ctx context.Context, event StreamEvent[V]) error {
	return a.backpressure.Offer(event)
}

func (a *BaseStreamActor[K, V]) OnWatermark(ctx context.Context, watermark Timestamp) error {
	a.state.UpdateWatermark(watermark)
	return nil
}

func (a *BaseStreamActor[K, V]) OnStart(ctx context.Context) error {
	a.running.Store(true)
	return nil
}

func (a *BaseStreamActor[K, V]) OnStop(ctx context.Context) error {
	a.running.Store(false)
	return nil
}

func (a *BaseStreamActor[K, V]) GetState() *StreamState {
	return a.state
}

func (a *BaseStreamActor[K, V]) SetOutput(fn func(ctx context.Context, key K, value V) error) {
	a.output = fn
}

func (a *BaseStreamActor[K, V]) Emit(ctx context.Context, key K, value V) error {
	if a.output == nil {
		return nil
	}
	a.state.IncrementEmitted()
	return a.output(ctx, key, value)
}

func (a *BaseStreamActor[K, V]) Poll() (StreamEvent[V], bool) {
	return a.backpressure.Poll()
}

func (a *BaseStreamActor[K, V]) PollBatch(maxSize int) []StreamEvent[V] {
	return a.backpressure.PollBatch(maxSize)
}

func (a *BaseStreamActor[K, V]) StateHandle() *StreamingStateHandle {
	return a.stateHandle
}

func (a *BaseStreamActor[K, V]) IsRunning() bool {
	return a.running.Load()
}

func (a *BaseStreamActor[K, V]) Run(ctx context.Context) error {
	if err := a.OnStart(ctx); err != nil {
		return err
	}

	defer func() { _ = a.OnStop(ctx) }()

	ticker := time.NewTicker(a.config.BufferTimeout.ToTimeDuration())
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			a.processBatch(ctx)
		default:
			if event, ok := a.Poll(); ok {
				if err := a.processEvent(ctx, event); err != nil {
					a.state.IncrementDropped()
				} else {
					a.state.IncrementProcessed()
				}
			}
		}
	}
}

func (a *BaseStreamActor[K, V]) processBatch(ctx context.Context) {
	batch := a.PollBatch(100)
	for _, event := range batch {
		if err := a.processEvent(ctx, event); err != nil {
			a.state.IncrementDropped()
		} else {
			a.state.IncrementProcessed()
		}
	}
}

func (a *BaseStreamActor[K, V]) processEvent(ctx context.Context, event StreamEvent[V]) error {
	return nil
}

type KeyedStreamActor[K comparable, V any] struct {
	*BaseStreamActor[K, V]
	processFunc func(ctx context.Context, key K, event StreamEvent[V]) error
}

func NewKeyedStreamActor[K comparable, V any](
	name string,
	config StreamConfig,
	processFunc func(ctx context.Context, key K, event StreamEvent[V]) error,
) *KeyedStreamActor[K, V] {
	return &KeyedStreamActor[K, V]{
		BaseStreamActor: NewBaseStreamActor[K, V](name, config),
		processFunc:     processFunc,
	}
}

func (a *KeyedStreamActor[K, V]) processEvent(ctx context.Context, event StreamEvent[V]) error {
	if a.processFunc == nil {
		return nil
	}

	var key K
	if keyStr, ok := any(event.Key).(K); ok {
		key = keyStr
	}

	return a.processFunc(ctx, key, event)
}

type WindowedStreamActor[K comparable, V any, R any] struct {
	*BaseStreamActor[K, V]
	assigner *WindowAssigner[K, V]
	trigger  *WindowTrigger[K, V, R]
	aggFunc  func(K, []V) R
}

func NewWindowedStreamActor[K comparable, V any, R any](
	name string,
	config StreamConfig,
	spec WindowSpec,
	aggFunc func(K, []V) R,
) (*WindowedStreamActor[K, V, R], error) {
	assigner, err := NewWindowAssigner[K, V](spec)
	if err != nil {
		return nil, err
	}

	trigger := NewWindowTrigger[K, V, R](assigner, aggFunc)

	return &WindowedStreamActor[K, V, R]{
		BaseStreamActor: NewBaseStreamActor[K, V](name, config),
		assigner:        assigner,
		trigger:         trigger,
		aggFunc:         aggFunc,
	}, nil
}

func (a *WindowedStreamActor[K, V, R]) OnWatermark(ctx context.Context, watermark Timestamp) error {
	a.BaseStreamActor.OnWatermark(ctx, watermark)

	results := a.trigger.TriggerAll(watermark)
	for _, result := range results {
		var key K
		if err := a.Emit(ctx, key, result.Result); err != nil {
			return err
		}
	}

	return nil
}

func (a *WindowedStreamActor[K, V, R]) processEvent(ctx context.Context, event StreamEvent[V]) error {
	var key K
	if keyStr, ok := any(event.Key).(K); ok {
		key = keyStr
	}

	a.assigner.Assign(key, event)
	return nil
}

type SourceStreamActor[V any] struct {
	*BaseStreamActor[string, V]
	sourceFunc func(ctx context.Context) (StreamEvent[V], error)
}

func NewSourceStreamActor[V any](
	name string,
	config StreamConfig,
	sourceFunc func(ctx context.Context) (StreamEvent[V], error),
) *SourceStreamActor[V] {
	return &SourceStreamActor[V]{
		BaseStreamActor: NewBaseStreamActor[string, V](name, config),
		sourceFunc:      sourceFunc,
	}
}

func (a *SourceStreamActor[V]) Run(ctx context.Context) error {
	if err := a.OnStart(ctx); err != nil {
		return err
	}
	defer func() { _ = a.OnStop(ctx) }()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
			event, err := a.sourceFunc(ctx)
			if err != nil {
				continue
			}

			if err := a.Emit(ctx, event.Key, event.Value); err != nil {
				a.state.IncrementDropped()
			} else {
				a.state.IncrementEmitted()
			}
		}
	}
}

type SinkStreamActor[V any] struct {
	*BaseStreamActor[string, V]
	sinkFunc func(ctx context.Context, event StreamEvent[V]) error
}

func NewSinkStreamActor[V any](
	name string,
	config StreamConfig,
	sinkFunc func(ctx context.Context, event StreamEvent[V]) error,
) *SinkStreamActor[V] {
	return &SinkStreamActor[V]{
		BaseStreamActor: NewBaseStreamActor[string, V](name, config),
		sinkFunc:        sinkFunc,
	}
}

func (a *SinkStreamActor[V]) processEvent(ctx context.Context, event StreamEvent[V]) error {
	if a.sinkFunc == nil {
		return nil
	}
	return a.sinkFunc(ctx, event)
}

type StreamPipeline struct {
	actors []StreamActor[string, interface{}]
	mu     sync.RWMutex
}

func NewStreamPipeline() *StreamPipeline {
	return &StreamPipeline{
		actors: make([]StreamActor[string, interface{}], 0),
	}
}

func (p *StreamPipeline) AddActor(actor StreamActor[string, interface{}]) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.actors = append(p.actors, actor)
}

func (p *StreamPipeline) Start(ctx context.Context) error {
	p.mu.RLock()
	defer p.mu.RUnlock()

	for _, actor := range p.actors {
		if err := actor.OnStart(ctx); err != nil {
			return err
		}
	}
	return nil
}

func (p *StreamPipeline) Stop(ctx context.Context) error {
	p.mu.RLock()
	defer p.mu.RUnlock()

	var lastErr error
	for _, actor := range p.actors {
		if err := actor.OnStop(ctx); err != nil {
			lastErr = err
		}
	}
	return lastErr
}
