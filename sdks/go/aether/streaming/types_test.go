package streaming

import (
	"testing"
)

func TestTimestamp_Now(t *testing.T) {
	ts := Now()
	if ts.Milliseconds <= 0 {
		t.Error("expected positive milliseconds")
	}
}

func TestTimestamp_FromSeconds(t *testing.T) {
	ts := TimestampFromSeconds(1.5)
	if ts.Milliseconds != 1500 {
		t.Errorf("expected 1500ms, got %d", ts.Milliseconds)
	}
}

func TestTimestamp_FromTime(t *testing.T) {
	ts := TimestampFromTime(Now().ToTime())
	if ts.Milliseconds <= 0 {
		t.Error("expected positive milliseconds")
	}
}

func TestTimestamp_ToTime(t *testing.T) {
	ts := Timestamp{Milliseconds: 1700000000000}
	tm := ts.ToTime()
	if tm.IsZero() {
		t.Error("expected non-zero time")
	}
	if tm.UnixMilli() != ts.Milliseconds {
		t.Errorf("milliseconds mismatch: %d vs %d", tm.UnixMilli(), ts.Milliseconds)
	}
}

func TestTimestamp_ToSeconds(t *testing.T) {
	ts := Timestamp{Milliseconds: 2500}
	if ts.ToSeconds() != 2.5 {
		t.Errorf("expected 2.5, got %f", ts.ToSeconds())
	}
}

func TestTimestamp_Add(t *testing.T) {
	ts := Timestamp{Milliseconds: 1000}
	result := ts.Add(Duration{Milliseconds: 500})
	if result.Milliseconds != 1500 {
		t.Errorf("expected 1500, got %d", result.Milliseconds)
	}
}

func TestTimestamp_Sub(t *testing.T) {
	ts1 := Timestamp{Milliseconds: 1500}
	ts2 := Timestamp{Milliseconds: 1000}
	dur := ts1.Sub(ts2)
	if dur.Milliseconds != 500 {
		t.Errorf("expected 500, got %d", dur.Milliseconds)
	}
}

func TestTimestamp_Before(t *testing.T) {
	tests := []struct {
		a, b Timestamp
		want bool
	}{
		{Timestamp{100}, Timestamp{200}, true},
		{Timestamp{200}, Timestamp{100}, false},
		{Timestamp{100}, Timestamp{100}, false},
	}
	for _, tt := range tests {
		if got := tt.a.Before(tt.b); got != tt.want {
			t.Errorf("%v.Before(%v) = %v, want %v", tt.a, tt.b, got, tt.want)
		}
	}
}

func TestTimestamp_After(t *testing.T) {
	tests := []struct {
		a, b Timestamp
		want bool
	}{
		{Timestamp{200}, Timestamp{100}, true},
		{Timestamp{100}, Timestamp{200}, false},
		{Timestamp{100}, Timestamp{100}, false},
	}
	for _, tt := range tests {
		if got := tt.a.After(tt.b); got != tt.want {
			t.Errorf("%v.After(%v) = %v, want %v", tt.a, tt.b, got, tt.want)
		}
	}
}

func TestTimestamp_Equal(t *testing.T) {
	tests := []struct {
		a, b Timestamp
		want bool
	}{
		{Timestamp{100}, Timestamp{100}, true},
		{Timestamp{100}, Timestamp{200}, false},
	}
	for _, tt := range tests {
		if got := tt.a.Equal(tt.b); got != tt.want {
			t.Errorf("%v.Equal(%v) = %v, want %v", tt.a, tt.b, got, tt.want)
		}
	}
}

func TestDuration_FromMillis(t *testing.T) {
	d := FromMillis(1500)
	if d.Milliseconds != 1500 {
		t.Errorf("expected 1500, got %d", d.Milliseconds)
	}
}

func TestDuration_FromSeconds(t *testing.T) {
	d := FromSeconds(2.5)
	if d.Milliseconds != 2500 {
		t.Errorf("expected 2500, got %d", d.Milliseconds)
	}
}

func TestDuration_FromMinutes(t *testing.T) {
	d := FromMinutes(1.5)
	if d.Milliseconds != 90000 {
		t.Errorf("expected 90000, got %d", d.Milliseconds)
	}
}

func TestDuration_FromHours(t *testing.T) {
	d := FromHours(2)
	if d.Milliseconds != 2*3600*1000 {
		t.Errorf("expected %d, got %d", 2*3600*1000, d.Milliseconds)
	}
}

func TestDuration_ToTimeDuration(t *testing.T) {
	d := FromSeconds(5)
	td := d.ToTimeDuration()
	if td.Seconds() != 5.0 {
		t.Errorf("expected 5s, got %v", td)
	}
}

func TestDuration_ToSeconds(t *testing.T) {
	d := Duration{Milliseconds: 3500}
	if d.ToSeconds() != 3.5 {
		t.Errorf("expected 3.5, got %f", d.ToSeconds())
	}
}

func TestDuration_ToMillis(t *testing.T) {
	d := Duration{Milliseconds: 42}
	if d.ToMillis() != 42 {
		t.Errorf("expected 42, got %d", d.ToMillis())
	}
}

func TestDuration_Add(t *testing.T) {
	d := Duration{Milliseconds: 100}
	result := d.Add(Duration{Milliseconds: 300})
	if result.Milliseconds != 400 {
		t.Errorf("expected 400, got %d", result.Milliseconds)
	}
}

func TestDuration_Mul(t *testing.T) {
	d := Duration{Milliseconds: 100}
	result := d.Mul(3)
	if result.Milliseconds != 300 {
		t.Errorf("expected 300, got %d", result.Milliseconds)
	}
}

func TestStreamEvent_New(t *testing.T) {
	event := NewStreamEvent("key1", "value1")
	if event.Key != "key1" {
		t.Errorf("expected key 'key1', got %q", event.Key)
	}
	if event.Value != "value1" {
		t.Errorf("expected value 'value1', got %v", event.Value)
	}
	if event.Timestamp.Milliseconds <= 0 {
		t.Error("expected non-zero timestamp")
	}
	if event.Headers == nil {
		t.Error("expected initialized headers")
	}
}

func TestStreamEvent_NewWithTimestamp(t *testing.T) {
	ts := Timestamp{Milliseconds: 12345}
	event := NewStreamEventWithTimestamp("k", "v", ts)
	if event.Timestamp.Milliseconds != 12345 {
		t.Errorf("expected 12345, got %d", event.Timestamp.Milliseconds)
	}
}

func TestStreamEvent_WithHeader(t *testing.T) {
	event := NewStreamEvent("k", "v")
	result := event.WithHeader("h1", "v1").WithHeader("h2", "v2")
	if result.Headers["h1"] != "v1" || result.Headers["h2"] != "v2" {
		t.Error("headers not set correctly")
	}
}

func TestStreamEvent_WithHeader_Nil(t *testing.T) {
	event := StreamEvent[int]{}
	event = event.WithHeader("k", "v")
	if event.Headers["k"] != "v" {
		t.Error("WithHeader should initialize nil headers")
	}
}

func TestStreamEvent_WithPartition(t *testing.T) {
	event := NewStreamEvent("k", "v")
	result := event.WithPartition(3)
	if result.Partition == nil || *result.Partition != 3 {
		t.Error("partition not set")
	}
}

func TestStreamEvent_WithOffset(t *testing.T) {
	event := NewStreamEvent("k", "v")
	result := event.WithOffset(42)
	if result.Offset == nil || *result.Offset != 42 {
		t.Error("offset not set")
	}
}

func TestWatermark_New(t *testing.T) {
	ts := Timestamp{Milliseconds: 1000}
	wm := NewWatermark(ts, "stream-1")
	if wm.Timestamp.Milliseconds != 1000 {
		t.Errorf("expected 1000, got %d", wm.Timestamp.Milliseconds)
	}
	if wm.StreamID != "stream-1" {
		t.Errorf("expected 'stream-1', got %q", wm.StreamID)
	}
}

func TestWatermark_IsLate(t *testing.T) {
	wm := Watermark{Timestamp: Timestamp{Milliseconds: 500}}
	if !wm.IsLate(Timestamp{Milliseconds: 300}) {
		t.Error("event before watermark should be late")
	}
	if wm.IsLate(Timestamp{Milliseconds: 700}) {
		t.Error("event after watermark should not be late")
	}
}

func TestWindowType_String(t *testing.T) {
	tests := []struct {
		wt   WindowType
		want string
	}{
		{WindowTypeTumbling, "Tumbling"},
		{WindowTypeSliding, "Sliding"},
		{WindowTypeSession, "Session"},
		{WindowType(99), "Unknown"},
	}
	for _, tt := range tests {
		if got := tt.wt.String(); got != tt.want {
			t.Errorf("WindowType(%d).String() = %q, want %q", tt.wt, got, tt.want)
		}
	}
}

func TestLateDataPolicy_String(t *testing.T) {
	tests := []struct {
		p    LateDataPolicy
		want string
	}{
		{LateDataPolicyDrop, "Drop"},
		{LateDataPolicySideOutput, "SideOutput"},
		{LateDataPolicyReprocess, "Reprocess"},
		{LateDataPolicy(99), "Unknown"},
	}
	for _, tt := range tests {
		if got := tt.p.String(); got != tt.want {
			t.Errorf("LateDataPolicy(%d).String() = %q, want %q", tt.p, got, tt.want)
		}
	}
}

func TestBackpressureStrategy_String(t *testing.T) {
	tests := []struct {
		s    BackpressureStrategy
		want string
	}{
		{BackpressureStrategyBuffer, "Buffer"},
		{BackpressureStrategyDrop, "Drop"},
		{BackpressureStrategyFail, "Fail"},
		{BackpressureStrategyLatest, "Latest"},
		{BackpressureStrategy(99), "Unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.String(); got != tt.want {
			t.Errorf("BackpressureStrategy(%d).String() = %q, want %q", tt.s, got, tt.want)
		}
	}
}

func TestWindowSpec_Validate(t *testing.T) {
	tests := []struct {
		name    string
		spec    WindowSpec
		wantErr bool
	}{
		{"valid tumbling", NewTumblingWindowSpec(FromSeconds(10)), false},
		{"valid sliding", NewSlidingWindowSpec(FromSeconds(10), FromSeconds(5)), false},
		{"valid session", NewSessionWindowSpec(FromSeconds(30)), false},
		{"sliding without slide", WindowSpec{Type: WindowTypeSliding, Size: FromSeconds(10)}, true},
		{"session without gap", WindowSpec{Type: WindowTypeSession}, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.spec.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestWindowSpecError(t *testing.T) {
	err := &WindowSpecError{Message: "test error"}
	if err.Error() != "test error" {
		t.Errorf("expected 'test error', got %q", err.Error())
	}
}

func TestWindowInfo_Contains(t *testing.T) {
	wi := WindowInfo{
		Start: Timestamp{Milliseconds: 100},
		End:   Timestamp{Milliseconds: 200},
	}
	if !wi.Contains(Timestamp{150}) {
		t.Error("150 should be in [100, 200)")
	}
	if !wi.Contains(Timestamp{100}) {
		t.Error("100 (start) should be contained")
	}
	if wi.Contains(Timestamp{200}) {
		t.Error("200 (end) should not be contained")
	}
}

func TestWindowInfo_IsLate(t *testing.T) {
	wi := WindowInfo{
		Start: Timestamp{Milliseconds: 100},
		End:   Timestamp{Milliseconds: 200},
	}
	if !wi.IsLate(Timestamp{50}) {
		t.Error("50 should be late for window starting at 100")
	}
	if wi.IsLate(Timestamp{150}) {
		t.Error("150 should not be late")
	}
}

func TestDefaultStreamConfig(t *testing.T) {
	cfg := DefaultStreamConfig()
	if cfg.Parallelism != 1 {
		t.Errorf("expected 1, got %d", cfg.Parallelism)
	}
	if cfg.BufferCapacity != 10000 {
		t.Errorf("expected 10000, got %d", cfg.BufferCapacity)
	}
}

func TestDefaultBackpressureConfig(t *testing.T) {
	cfg := DefaultBackpressureConfig()
	if cfg.BufferSize != 10000 {
		t.Errorf("expected 10000, got %d", cfg.BufferSize)
	}
	if cfg.Strategy != BackpressureStrategyBuffer {
		t.Errorf("expected buffer strategy, got %v", cfg.Strategy)
	}
}

func TestDeliveryConfig_Defaults(t *testing.T) {
	cfg := DefaultDeliveryConfig()
	if cfg.Semantics != DeliverySemanticsAtLeastOnce {
		t.Errorf("expected at-least-once, got %v", cfg.Semantics)
	}
	if cfg.MaxRetries != 3 {
		t.Errorf("expected 3 retries, got %d", cfg.MaxRetries)
	}
}

func TestDeliverySemantics_Values(t *testing.T) {
	if DeliverySemanticsAtMostOnce != 0 {
		t.Error("AtMostOnce should be 0")
	}
	if DeliverySemanticsAtLeastOnce != 1 {
		t.Error("AtLeastOnce should be 1")
	}
	if DeliverySemanticsExactlyOnce != 2 {
		t.Error("ExactlyOnce should be 2")
	}
}
