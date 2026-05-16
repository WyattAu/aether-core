package streaming

import (
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
}

func (ts Timestamp) ToSeconds() float64 {
	return float64(ts.Milliseconds) / 1000.0
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

func (ts Timestamp) IsZero() bool {
	return ts.Milliseconds == 0
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

func (d Duration) ToTimeDuration() time.Duration {
	return time.Duration(d.Milliseconds) * time.Millisecond
}

func (d Duration) ToSeconds() float64 {
	return float64(d.Milliseconds) / 1000.0
}

func (d Duration) ToMillis() int64 {
	return d.Milliseconds
}

func (d Duration) Add(other Duration) Duration {
	return Duration{Milliseconds: d.Milliseconds + other.Milliseconds}
}

func (d Duration) Mul(factor int64) Duration {
	return Duration{Milliseconds: d.Milliseconds * factor}
}

type StreamEvent[T any] struct {
	Key       string
	Value     T
	Timestamp Timestamp
	Headers   map[string]string
	Partition *int
	Offset    *int64
	EventType string
}

func NewStreamEvent[T any](key string, value T) StreamEvent[T] {
	return StreamEvent[T]{
		Key:       key,
		Value:     value,
		Timestamp: Now(),
		Headers:   make(map[string]string),
	}
}

func NewStreamEventWithTimestamp[T any](key string, value T, ts Timestamp) StreamEvent[T] {
	return StreamEvent[T]{
		Key:       key,
		Value:     value,
		Timestamp: ts,
		Headers:   make(map[string]string),
	}
}

func (e StreamEvent[T]) WithHeader(key, value string) StreamEvent[T] {
	if e.Headers == nil {
		e.Headers = make(map[string]string)
	}
	e.Headers[key] = value
	return e
}

func (e StreamEvent[T]) WithPartition(p int) StreamEvent[T] {
	e.Partition = &p
	return e
}

func (e StreamEvent[T]) WithOffset(o int64) StreamEvent[T] {
	e.Offset = &o
	return e
}

type Watermark struct {
	Timestamp Timestamp
	StreamID  string
	Partition *int
}

func NewWatermark(ts Timestamp, streamID string) Watermark {
	return Watermark{
		Timestamp: ts,
		StreamID:  streamID,
	}
}

func (w Watermark) IsLate(eventTimestamp Timestamp) bool {
	return eventTimestamp.Before(w.Timestamp)
}

type WindowType int

const (
	WindowTypeTumbling WindowType = iota
	WindowTypeSliding
	WindowTypeSession
)

func (wt WindowType) String() string {
	switch wt {
	case WindowTypeTumbling:
		return "Tumbling"
	case WindowTypeSliding:
		return "Sliding"
	case WindowTypeSession:
		return "Session"
	default:
		return "Unknown"
	}
}

type LateDataPolicy int

const (
	LateDataPolicyDrop LateDataPolicy = iota
	LateDataPolicySideOutput
	LateDataPolicyReprocess
)

func (p LateDataPolicy) String() string {
	switch p {
	case LateDataPolicyDrop:
		return "Drop"
	case LateDataPolicySideOutput:
		return "SideOutput"
	case LateDataPolicyReprocess:
		return "Reprocess"
	default:
		return "Unknown"
	}
}

type WatermarkStrategy int

const (
	WatermarkStrategyEventTime WatermarkStrategy = iota
	WatermarkStrategyProcessingTime
	WatermarkStrategyBoundedOutOfOrder
)

type BackpressureStrategy int

const (
	BackpressureStrategyBuffer BackpressureStrategy = iota
	BackpressureStrategyDrop
	BackpressureStrategyFail
	BackpressureStrategyLatest
)

func (s BackpressureStrategy) String() string {
	switch s {
	case BackpressureStrategyBuffer:
		return "Buffer"
	case BackpressureStrategyDrop:
		return "Drop"
	case BackpressureStrategyFail:
		return "Fail"
	case BackpressureStrategyLatest:
		return "Latest"
	default:
		return "Unknown"
	}
}

type DeliverySemantics int

const (
	DeliverySemanticsAtMostOnce DeliverySemantics = iota
	DeliverySemanticsAtLeastOnce
	DeliverySemanticsExactlyOnce
)

type PaneInfo int

const (
	PaneInfoEarly PaneInfo = iota
	PaneInfoOnTime
	PaneInfoLate
)

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
		Type:  WindowTypeSliding,
		Size:  size,
		Slide: &slide,
	}
}

func NewSessionWindowSpec(gap Duration) WindowSpec {
	return WindowSpec{
		Type: WindowTypeSession,
		Gap:  &gap,
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

type WindowSpecError struct {
	Message string
}

func (e *WindowSpecError) Error() string {
	return e.Message
}

type WindowInfo struct {
	Start         Timestamp
	End           Timestamp
	MaxTimestamp  Timestamp
	Pane          PaneInfo
	WindowID      string
}

func (i WindowInfo) Contains(ts Timestamp) bool {
	return (ts.Equal(i.Start) || ts.After(i.Start)) && ts.Before(i.End)
}

func (i WindowInfo) IsLate(ts Timestamp) bool {
	return ts.Before(i.Start)
}

type StreamConfig struct {
	InputStreams      []string
	OutputStreams     []string
	Parallelism       int
	PartitionStrategy string

	WatermarkStrategy   WatermarkStrategy
	WatermarkInterval   Duration
	OutOfOrderness      Duration

	CheckpointingEnabled bool
	CheckpointInterval   Duration

	LateDataPolicy  LateDataPolicy
	LateDataOutput  string

	BufferCapacity int
	BufferTimeout  Duration
}

func DefaultStreamConfig() StreamConfig {
	return StreamConfig{
		Parallelism:         1,
		PartitionStrategy:   "key",
		WatermarkStrategy:   WatermarkStrategyProcessingTime,
		WatermarkInterval:   FromSeconds(1),
		OutOfOrderness:      FromMillis(0),
		CheckpointingEnabled: false,
		CheckpointInterval:  FromMinutes(1),
		LateDataPolicy:      LateDataPolicyDrop,
		BufferCapacity:      10000,
		BufferTimeout:       FromSeconds(30),
	}
}

type BackpressureConfig struct {
	Strategy       BackpressureStrategy
	BufferSize     int
	HighWatermark  float64
	LowWatermark   float64
}

func DefaultBackpressureConfig() BackpressureConfig {
	return BackpressureConfig{
		Strategy:      BackpressureStrategyBuffer,
		BufferSize:    10000,
		HighWatermark: 0.9,
		LowWatermark:  0.5,
	}
}

type DeliveryConfig struct {
	Semantics        DeliverySemantics
	MaxRetries       int
	RetryBackoff     Duration
	DeadLetterTopic  string
	EnableIdempotence bool
}

func DefaultDeliveryConfig() DeliveryConfig {
	return DeliveryConfig{
		Semantics:    DeliverySemanticsAtLeastOnce,
		MaxRetries:   3,
		RetryBackoff: FromSeconds(1),
	}
}
