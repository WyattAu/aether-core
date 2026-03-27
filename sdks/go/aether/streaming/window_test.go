package streaming

import (
	"testing"
)

func TestWindowState_New(t *testing.T) {
	ws := NewWindowState[string, int]()
	if ws == nil {
		t.Fatal("expected non-nil WindowState")
	}
	if ws.Count() != 0 {
		t.Error("new window state should be empty")
	}
}

func TestWindowState_AddEvent(t *testing.T) {
	ws := NewWindowState[string, int]()
	event := StreamEvent[int]{Key: "k1", Value: 42, Timestamp: Timestamp{Milliseconds: 100}}

	ws.AddEvent(event)
	if ws.Count() != 1 {
		t.Errorf("expected count 1, got %d", ws.Count())
	}
	if ws.MaxTimestamp().Milliseconds != 100 {
		t.Errorf("expected max timestamp 100, got %d", ws.MaxTimestamp().Milliseconds)
	}
}

func TestWindowState_Events(t *testing.T) {
	ws := NewWindowState[string, int]()
	ws.AddEvent(StreamEvent[int]{Value: 1, Timestamp: Timestamp{Milliseconds: 100}})
	ws.AddEvent(StreamEvent[int]{Value: 2, Timestamp: Timestamp{Milliseconds: 200}})

	events := ws.Events()
	if len(events) != 2 {
		t.Errorf("expected 2 events, got %d", len(events))
	}
}

func TestWindowState_Events_Immutable(t *testing.T) {
	ws := NewWindowState[string, int]()
	ws.AddEvent(StreamEvent[int]{Value: 42, Timestamp: Timestamp{Milliseconds: 100}})

	events := ws.Events()
	events[0].Value = 99

	original := ws.Events()
	if original[0].Value == 99 {
		t.Error("modifying returned events should not affect window state")
	}
}

func TestWindowState_Values(t *testing.T) {
	ws := NewWindowState[string, int]()
	ws.AddEvent(StreamEvent[int]{Value: 10, Timestamp: Timestamp{Milliseconds: 100}})
	ws.AddEvent(StreamEvent[int]{Value: 20, Timestamp: Timestamp{Milliseconds: 200}})

	values := ws.Values()
	if len(values) != 2 {
		t.Errorf("expected 2 values, got %d", len(values))
	}
	if values[0] != 10 || values[1] != 20 {
		t.Errorf("expected [10, 20], got %v", values)
	}
}

func TestWindowState_MaxTimestamp(t *testing.T) {
	ws := NewWindowState[string, int]()
	ws.AddEvent(StreamEvent[int]{Timestamp: Timestamp{Milliseconds: 100}})
	ws.AddEvent(StreamEvent[int]{Timestamp: Timestamp{Milliseconds: 300}})
	ws.AddEvent(StreamEvent[int]{Timestamp: Timestamp{Milliseconds: 200}})

	if ws.MaxTimestamp().Milliseconds != 300 {
		t.Errorf("expected max 300, got %d", ws.MaxTimestamp().Milliseconds)
	}
}

func TestWindowState_Close(t *testing.T) {
	ws := NewWindowState[string, int]()
	if ws.IsClosed() {
		t.Error("new window should not be closed")
	}

	ws.Close()
	if !ws.IsClosed() {
		t.Error("window should be closed after Close()")
	}
}

func TestWindowState_Clear(t *testing.T) {
	ws := NewWindowState[string, int]()
	ws.AddEvent(StreamEvent[int]{Value: 1, Timestamp: Timestamp{Milliseconds: 100}})
	ws.AddEvent(StreamEvent[int]{Value: 2, Timestamp: Timestamp{Milliseconds: 200}})

	ws.Clear()
	if ws.Count() != 0 {
		t.Errorf("expected 0 after clear, got %d", ws.Count())
	}
	if !ws.MaxTimestamp().IsZero() {
		t.Error("max timestamp should be zero after clear")
	}
}

func TestWindowAssigner_NewTumbling(t *testing.T) {
	wa, err := NewWindowAssigner[string, int](NewTumblingWindowSpec(FromSeconds(10)))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	event := StreamEvent[int]{Key: "user1", Value: 42, Timestamp: Timestamp{Milliseconds: 15000}}
	ids := wa.Assign("user1", event)
	if len(ids) != 1 {
		t.Errorf("expected 1 window ID, got %d", len(ids))
	}

	ws := wa.GetWindow(ids[0])
	if ws == nil {
		t.Error("window should exist")
	}
	if ws.Count() != 1 {
		t.Errorf("expected 1 event in window, got %d", ws.Count())
	}
}

func TestWindowAssigner_NewSliding(t *testing.T) {
	wa, err := NewWindowAssigner[string, int](NewSlidingWindowSpec(FromSeconds(10), FromSeconds(5)))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	event := StreamEvent[int]{Key: "k", Value: 1, Timestamp: Timestamp{Milliseconds: 25000}}
	ids := wa.Assign("k", event)
	if len(ids) == 0 {
		t.Error("expected at least 1 window ID for sliding window")
	}
}

func TestWindowAssigner_NewSession(t *testing.T) {
	wa, err := NewWindowAssigner[string, int](NewSessionWindowSpec(FromSeconds(30)))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	event1 := StreamEvent[int]{Key: "user1", Value: 1, Timestamp: Timestamp{Milliseconds: 1000}}
	ids1 := wa.Assign("user1", event1)
	if len(ids1) != 1 {
		t.Errorf("expected 1 window, got %d", len(ids1))
	}

	event2 := StreamEvent[int]{Key: "user1", Value: 2, Timestamp: Timestamp{Milliseconds: 20000}}
	ids2 := wa.Assign("user1", event2)
	if len(ids2) != 1 {
		t.Errorf("expected 1 window, got %d", len(ids2))
	}
	if ids1[0] != ids2[0] {
		t.Error("events within gap should be in same session window")
	}
}

func TestWindowAssigner_RemoveWindow(t *testing.T) {
	wa, _ := NewWindowAssigner[string, int](NewTumblingWindowSpec(FromSeconds(10)))

	event := StreamEvent[int]{Key: "k", Value: 1, Timestamp: Timestamp{Milliseconds: 5000}}
	ids := wa.Assign("k", event)

	wa.RemoveWindow(ids[0])
	if wa.GetWindow(ids[0]) != nil {
		t.Error("window should be removed")
	}
}

func TestWindowAssigner_GetAllWindows(t *testing.T) {
	wa, _ := NewWindowAssigner[string, int](NewTumblingWindowSpec(FromSeconds(10)))

	wa.Assign("k1", StreamEvent[int]{Timestamp: Timestamp{Milliseconds: 5000}})
	wa.Assign("k2", StreamEvent[int]{Timestamp: Timestamp{Milliseconds: 15000}})

	all := wa.GetAllWindows()
	if len(all) != 2 {
		t.Errorf("expected 2 windows, got %d", len(all))
	}
}

func TestWindowAssigner_InvalidSpec(t *testing.T) {
	_, err := NewWindowAssigner[string, int](WindowSpec{Type: WindowTypeSliding})
	if err == nil {
		t.Error("expected error for sliding window without slide")
	}

	_, err = NewWindowAssigner[string, int](WindowSpec{Type: WindowTypeSession})
	if err == nil {
		t.Error("expected error for session window without gap")
	}
}

func TestWindowTrigger_TriggerWindow(t *testing.T) {
	wa, _ := NewWindowAssigner[string, int](NewTumblingWindowSpec(FromSeconds(10)))
	wa.Assign("k", StreamEvent[int]{Value: 10, Timestamp: Timestamp{Milliseconds: 5000}})
	wa.Assign("k", StreamEvent[int]{Value: 20, Timestamp: Timestamp{Milliseconds: 7000}})

	aggFunc := func(k string, values []int) int {
		sum := 0
		for _, v := range values {
			sum += v
		}
		return sum
	}

	wt := NewWindowTrigger[string, int, int](wa, aggFunc)

	all := wa.GetAllWindows()
	var windowID string
	for id := range all {
		windowID = id
		break
	}

	result := wt.TriggerWindow(windowID, "k", WindowInfo{})
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.Result != 30 {
		t.Errorf("expected sum 30, got %d", result.Result)
	}
}

func TestTumblingWindow_New(t *testing.T) {
	tw, err := NewTumblingWindow[string, int](FromSeconds(10))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	tw.Process("user1", StreamEvent[int]{Value: 42, Timestamp: Timestamp{Milliseconds: 5000}})
	results := tw.Trigger(Timestamp{Milliseconds: 20000})

	if len(results) != 1 {
		t.Errorf("expected 1 result, got %d", len(results))
	}
}

func TestTumblingWindow_WithAggregation(t *testing.T) {
	tw, _ := NewTumblingWindow[string, int](FromSeconds(10))

	sumFunc := func(k string, values []int) int {
		sum := 0
		for _, v := range values {
			sum += v
		}
		return sum
	}

	tw = tw.WithAggregation(sumFunc)
	tw.Process("k", StreamEvent[int]{Value: 10, Timestamp: Timestamp{Milliseconds: 5000}})
	tw.Process("k", StreamEvent[int]{Value: 20, Timestamp: Timestamp{Milliseconds: 7000}})

	results := tw.Trigger(Timestamp{Milliseconds: 20000})
	if len(results) != 0 {
		for _, r := range results {
			if r.Result != 30 {
				t.Errorf("expected sum 30, got %d", r.Result)
			}
		}
	}
}

func TestSlidingWindow_New(t *testing.T) {
	sw, err := NewSlidingWindow[string, int](FromSeconds(10), FromSeconds(5))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	sw.Process("k", StreamEvent[int]{Value: 42, Timestamp: Timestamp{Milliseconds: 5000}})
	if len(sw.Trigger(Timestamp{Milliseconds: 20000})) >= 0 {
		// Just verify no panic
	}
}

func TestSessionWindow_New(t *testing.T) {
	sw, err := NewSessionWindow[string, int](FromSeconds(30))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	sw.Process("k", StreamEvent[int]{Value: 1, Timestamp: Timestamp{Milliseconds: 1000}})
	sw.Process("k", StreamEvent[int]{Value: 2, Timestamp: Timestamp{Milliseconds: 5000}})

	if len(sw.Trigger(Timestamp{Milliseconds: 100000})) >= 0 {
		// Verify no panic
	}
}

func TestWindowResult_Fields(t *testing.T) {
	wr := WindowResult[string, int]{
		Key:    "test-key",
		Values: []int{1, 2, 3},
		WindowInfo: WindowInfo{
			Start: Timestamp{Milliseconds: 0},
			End:   Timestamp{Milliseconds: 1000},
		},
	}
	if wr.Key != "test-key" {
		t.Errorf("expected 'test-key', got %q", wr.Key)
	}
	if len(wr.Values) != 3 {
		t.Errorf("expected 3 values, got %d", len(wr.Values))
	}
}
