package streaming

import (
	"context"
	"sync/atomic"
	"testing"
)

func TestStreamState_New(t *testing.T) {
	ss := NewStreamState()
	if ss == nil {
		t.Fatal("expected non-nil StreamState")
	}
	processed, emitted, dropped := ss.GetMetrics()
	if processed != 0 || emitted != 0 || dropped != 0 {
		t.Error("new state should have zero metrics")
	}
}

func TestStreamState_KeyedState(t *testing.T) {
	ss := NewStreamState()

	val := ss.GetKeyedState("key")
	if val != nil {
		t.Errorf("expected nil for missing key, got %v", val)
	}

	ss.SetKeyedState("key", []byte("value"))
	val = ss.GetKeyedState("key")
	if string(val) != "value" {
		t.Errorf("expected 'value', got %q", string(val))
	}

	ss.DeleteKeyedState("key")
	val = ss.GetKeyedState("key")
	if val != nil {
		t.Error("expected nil after delete")
	}
}

func TestStreamState_KeyedState_Isolation(t *testing.T) {
	ss := NewStreamState()
	data := []byte("original")
	ss.SetKeyedState("key", data)
	data[0] = 'X'

	val := ss.GetKeyedState("key")
	if string(val) == "Xriginal" {
		t.Error("mutation of input should not affect stored value")
	}
}

func TestStreamState_OperatorState(t *testing.T) {
	ss := NewStreamState()

	val := ss.GetOperatorState("op-key")
	if val != nil {
		t.Error("expected nil for missing key")
	}

	ss.SetOperatorState("op-key", []byte("op-val"))
	val = ss.GetOperatorState("op-key")
	if string(val) != "op-val" {
		t.Errorf("expected 'op-val', got %q", string(val))
	}
}

func TestStreamState_Watermark(t *testing.T) {
	ss := NewStreamState()

	if !ss.GetWatermark().IsZero() {
		t.Error("initial watermark should be zero")
	}

	ss.UpdateWatermark(Timestamp{Milliseconds: 1000})
	if ss.GetWatermark().Milliseconds != 1000 {
		t.Errorf("expected 1000, got %d", ss.GetWatermark().Milliseconds)
	}

	ss.UpdateWatermark(Timestamp{Milliseconds: 500})
	if ss.GetWatermark().Milliseconds != 1000 {
		t.Error("watermark should not decrease")
	}
}

func TestStreamState_Metrics(t *testing.T) {
	ss := NewStreamState()
	ss.IncrementProcessed()
	ss.IncrementProcessed()
	ss.IncrementProcessed()
	ss.IncrementEmitted()
	ss.IncrementDropped()

	p, e, d := ss.GetMetrics()
	if p != 3 {
		t.Errorf("expected 3 processed, got %d", p)
	}
	if e != 1 {
		t.Errorf("expected 1 emitted, got %d", e)
	}
	if d != 1 {
		t.Errorf("expected 1 dropped, got %d", d)
	}
}

func TestStreamState_WindowManagement(t *testing.T) {
	ss := NewStreamState()

	info := &WindowStateInfo{
		WindowID:  "w1",
		Start:     Timestamp{Milliseconds: 0},
		End:       Timestamp{Milliseconds: 1000},
		EventCount: 5,
		IsClosed:  false,
	}
	ss.RegisterWindow(info)

	active := ss.GetActiveWindows()
	if len(active) != 1 {
		t.Errorf("expected 1 active window, got %d", len(active))
	}

	ss.UpdateWindow("w1", 10)
	if active[0].EventCount != 10 {
		t.Errorf("expected 10 events, got %d", active[0].EventCount)
	}

	ss.CloseWindow("w1")
	active = ss.GetActiveWindows()
	if len(active) != 0 {
		t.Errorf("expected 0 active windows, got %d", len(active))
	}
}

func TestStreamState_Snapshot(t *testing.T) {
	ss := NewStreamState()
	ss.SetKeyedState("k1", []byte("v1"))
	ss.SetOperatorState("op1", []byte("ov1"))

	snapshot := ss.Snapshot()
	if len(snapshot) != 2 {
		t.Errorf("expected 2 entries, got %d", len(snapshot))
	}
}

func TestStreamState_Restore(t *testing.T) {
	ss := NewStreamState()
	ss.SetKeyedState("k1", []byte("v1"))

	snapshot := ss.Snapshot()
	ss2 := NewStreamState()
	ss2.Restore(snapshot)

	val := ss2.GetKeyedState("k1")
	if string(val) != "v1" {
		t.Errorf("expected 'v1' after restore, got %q", string(val))
	}
}

func TestStreamingStateHandle(t *testing.T) {
	ss := NewStreamState()
	handle := NewStreamingStateHandle(ss)

	ctx := context.Background()
	handle.Set(ctx, "key", []byte("value"))
	val := handle.Get(ctx, "key")
	if string(val) != "value" {
		t.Errorf("expected 'value', got %q", string(val))
	}

	handle.Delete(ctx, "key")
	val = handle.Get(ctx, "key")
	if val != nil {
		t.Error("expected nil after delete")
	}
}

func TestStreamingStateHandle_OperatorState(t *testing.T) {
	ss := NewStreamState()
	handle := NewStreamingStateHandle(ss)

	ctx := context.Background()
	handle.SetOperatorState(ctx, "op", []byte("data"))
	val := handle.GetOperatorState(ctx, "op")
	if string(val) != "data" {
		t.Errorf("expected 'data', got %q", string(val))
	}
}

func TestStreamingStateHandle_Watermark(t *testing.T) {
	ss := NewStreamState()
	handle := NewStreamingStateHandle(ss)

	ss.UpdateWatermark(Timestamp{Milliseconds: 42})
	if handle.GetWatermark().Milliseconds != 42 {
		t.Errorf("expected 42, got %d", handle.GetWatermark().Milliseconds)
	}
}

func TestBaseStreamActor_New(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test-actor", config)

	if actor.Name() != "test-actor" {
		t.Errorf("expected 'test-actor', got %q", actor.Name())
	}
	if actor.IsRunning() {
		t.Error("new actor should not be running")
	}
}

func TestBaseStreamActor_Lifecycle(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	ctx := context.Background()
	err := actor.OnStart(ctx)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if !actor.IsRunning() {
		t.Error("actor should be running after OnStart")
	}

	actor.OnStop(ctx)
	if actor.IsRunning() {
		t.Error("actor should not be running after OnStop")
	}
}

func TestBaseStreamActor_Process(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	ctx := context.Background()
	event := NewStreamEvent("key", 42)
	err := actor.Process(ctx, event)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	val, ok := actor.Poll()
	if !ok {
		t.Error("expected event in buffer")
	}
	if val.Value != 42 {
		t.Errorf("expected 42, got %d", val.Value)
	}
}

func TestBaseStreamActor_PollBatch(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	for i := 0; i < 5; i++ {
		actor.Process(context.Background(), NewStreamEvent("k", i))
	}

	batch := actor.PollBatch(3)
	if len(batch) != 3 {
		t.Errorf("expected batch of 3, got %d", len(batch))
	}
}

func TestBaseStreamActor_Poll_Empty(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	_, ok := actor.Poll()
	if ok {
		t.Error("poll from empty should return false")
	}
}

func TestBaseStreamActor_Emit(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	var emitted int64
	actor.SetOutput(func(ctx context.Context, key string, value int) error {
		atomic.AddInt64(&emitted, 1)
		return nil
	})

	ctx := context.Background()
	actor.Emit(ctx, "key", 42)
	if atomic.LoadInt64(&emitted) != 1 {
		t.Error("emit should call output function")
	}
}

func TestBaseStreamActor_OnWatermark(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	ctx := context.Background()
	actor.OnWatermark(ctx, Timestamp{Milliseconds: 5000})
	if actor.GetState().GetWatermark().Milliseconds != 5000 {
		t.Errorf("expected watermark 5000, got %d", actor.GetState().GetWatermark().Milliseconds)
	}
}

func TestBaseStreamActor_GetState(t *testing.T) {
	config := DefaultStreamConfig()
	actor := NewBaseStreamActor[string, int]("test", config)

	state := actor.GetState()
	if state == nil {
		t.Error("expected non-nil state")
	}
}

func TestKeyedStreamActor_New(t *testing.T) {
	config := DefaultStreamConfig()
	var processed int64
	actor := NewKeyedStreamActor("test", config, func(ctx context.Context, key string, event StreamEvent[int]) error {
		atomic.AddInt64(&processed, 1)
		return nil
	})

	if actor.Name() != "test" {
		t.Errorf("expected 'test', got %q", actor.Name())
	}

	actor.Process(context.Background(), NewStreamEvent("key", 1))
	event, ok := actor.Poll()
	if !ok {
		t.Fatal("expected event")
	}

	actor.processEvent(context.Background(), event)
	if atomic.LoadInt64(&processed) != 1 {
		t.Error("processFunc should have been called")
	}
}

func TestSourceStreamActor_New(t *testing.T) {
	config := DefaultStreamConfig()
	count := 0
	actor := NewSourceStreamActor("source", config, func(ctx context.Context) (StreamEvent[int], error) {
		count++
		if count > 1 {
			ctx.Done()
			return StreamEvent[int]{}, ctx.Err()
		}
		return NewStreamEvent("k", count), nil
	})

	if actor.Name() != "source" {
		t.Errorf("expected 'source', got %q", actor.Name())
	}
}

func TestSinkStreamActor_New(t *testing.T) {
	config := DefaultStreamConfig()
	var received int
	actor := NewSinkStreamActor("sink", config, func(ctx context.Context, event StreamEvent[int]) error {
		received++
		return nil
	})

	if actor.Name() != "sink" {
		t.Errorf("expected 'sink', got %q", actor.Name())
	}

	event := NewStreamEvent("k", 42)
	actor.processEvent(context.Background(), event)
	if received != 1 {
		t.Error("sinkFunc should have been called")
	}
}

func TestStreamPipeline_New(t *testing.T) {
	p := NewStreamPipeline()
	if p == nil {
		t.Fatal("expected non-nil pipeline")
	}
}

func TestStreamPipeline_StartStop(t *testing.T) {
	p := NewStreamPipeline()
	ctx := context.Background()

	err := p.Start(ctx)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	err = p.Stop(ctx)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}
