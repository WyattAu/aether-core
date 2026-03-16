package aether_test

import (
	"context"
	"testing"
	"time"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

func TestCapabilitySet(t *testing.T) {
	caps := aether.NewCapabilitySet(
		aether.CapabilityNetworkOutbound,
		aether.CapabilityStateRead,
	)

	if !caps.Has(aether.CapabilityNetworkOutbound) {
		t.Error("expected NETWORK_OUTBOUND capability")
	}

	if !caps.HasNetwork() {
		t.Error("expected HasNetwork to return true")
	}

	if !caps.HasState() {
		t.Error("expected HasState to return true")
	}

	if caps.Has(aether.CapabilityFSRead) {
		t.Error("did not expect FS_READ capability")
	}
}

func TestMessage(t *testing.T) {
	msg := aether.NewMessage(aether.MessageTypeRequest, "hello")

	if msg.Type != aether.MessageTypeRequest {
		t.Errorf("expected type %s, got %s", aether.MessageTypeRequest, msg.Type)
	}

	if msg.Payload != "hello" {
		t.Errorf("expected payload 'hello', got %v", msg.Payload)
	}

	if msg.Priority != aether.PriorityNormal {
		t.Errorf("expected normal priority, got %v", msg.Priority)
	}
}

func TestMessageResponse(t *testing.T) {
	request := aether.NewMessage(aether.MessageTypeRequest, "ping")
	request.Sender = "caller"
	request.CorrelationID = "123"

	response := aether.NewResponse(request, "pong")

	if response.Type != aether.MessageTypeResponse {
		t.Errorf("expected type %s, got %s", aether.MessageTypeResponse, response.Type)
	}

	if response.CorrelationID != "123" {
		t.Errorf("expected correlation ID '123', got %s", response.CorrelationID)
	}
}

func TestStateHandle(t *testing.T) {
	ctx := context.Background()
	state := aether.NewStateHandle()

	// Test write
	err := state.Write(ctx, "key1", []byte("value1"))
	if err != nil {
		t.Fatalf("write failed: %v", err)
	}

	// Test read
	val, err := state.Read(ctx, "key1")
	if err != nil {
		t.Fatalf("read failed: %v", err)
	}

	if string(val) != "value1" {
		t.Errorf("expected 'value1', got '%s'", string(val))
	}

	// Test exists
	exists, err := state.Exists(ctx, "key1")
	if err != nil {
		t.Fatalf("exists failed: %v", err)
	}

	if !exists {
		t.Error("expected key1 to exist")
	}

	// Test delete
	err = state.Delete(ctx, "key1")
	if err != nil {
		t.Fatalf("delete failed: %v", err)
	}

	exists, _ = state.Exists(ctx, "key1")
	if exists {
		t.Error("expected key1 to not exist after delete")
	}
}

func TestStateHandleListKeys(t *testing.T) {
	ctx := context.Background()
	state := aether.NewStateHandle()

	state.Write(ctx, "prefix/key1", []byte("1"))
	state.Write(ctx, "prefix/key2", []byte("2"))
	state.Write(ctx, "other/key3", []byte("3"))

	keys, err := state.ListKeys(ctx, "prefix/")
	if err != nil {
		t.Fatalf("list keys failed: %v", err)
	}

	if len(keys) != 2 {
		t.Errorf("expected 2 keys, got %d", len(keys))
	}
}

func TestBaseActor(t *testing.T) {
	actor := aether.NewBaseActor("test-actor")

	if actor.Name() != "test-actor" {
		t.Errorf("expected name 'test-actor', got '%s'", actor.Name())
	}

	actor.Require(aether.CapabilityLog, aether.CapabilityTime)

	caps := actor.Capabilities()
	if !caps.Has(aether.CapabilityLog) {
		t.Error("expected LOG capability")
	}

	if !caps.Has(aether.CapabilityTime) {
		t.Error("expected TIME capability")
	}
}

func TestErrors(t *testing.T) {
	// Test capability denied error
	err := aether.CapabilityDenied("NETWORK_OUTBOUND")
	if !aether.IsCapabilityDenied(err) {
		t.Error("expected capability denied error")
	}

	// Test actor not found error
	err = aether.ActorNotFound("missing-actor")
	if !aether.IsActorNotFound(err) {
		t.Error("expected actor not found error")
	}

	// Test timeout error
	err = aether.Timeout("operation")
	if !aether.IsTimeout(err) {
		t.Error("expected timeout error")
	}
}

func TestVersion(t *testing.T) {
	version := aether.GetVersion()
	if version == "" {
		t.Error("expected non-empty version")
	}
}

func TestAllCapabilities(t *testing.T) {
	caps := aether.AllCapabilities()

	allCaps := []aether.Capability{
		aether.CapabilityNetworkOutbound,
		aether.CapabilityNetworkInbound,
		aether.CapabilityStateRead,
		aether.CapabilityStateWrite,
		aether.CapabilityFSRead,
		aether.CapabilityFSWrite,
		aether.CapabilityActorMessaging,
		aether.CapabilityLog,
		aether.CapabilityTime,
		aether.CapabilityRandom,
		aether.CapabilityEnvironment,
		aether.CapabilityHTTPClient,
		aether.CapabilityHTTPServer,
		aether.CapabilityProcessSpawn,
	}

	for _, cap := range allCaps {
		if !caps.Has(cap) {
			t.Errorf("expected %s capability in AllCapabilities", cap)
		}
	}
}

func TestMessagePriority(t *testing.T) {
	msg := aether.NewMessage(aether.MessageTypeRequest, "test")
	msg = msg.WithPriority(aether.PriorityCritical)

	if msg.Priority != aether.PriorityCritical {
		t.Errorf("expected critical priority, got %v", msg.Priority)
	}
}

func TestMessageMetadata(t *testing.T) {
	msg := aether.NewMessage(aether.MessageTypeRequest, "test")
	msg = msg.WithMetadata("trace-id", "abc123")

	if msg.Metadata["trace-id"] != "abc123" {
		t.Errorf("expected metadata trace-id 'abc123', got '%s'", msg.Metadata["trace-id"])
	}
}

func TestBaseActorState(t *testing.T) {
	actor := aether.NewBaseActor("stateful-actor")
	state := actor.State()

	ctx := context.Background()
	err := state.Write(ctx, "counter", []byte("42"))
	if err != nil {
		t.Fatalf("write failed: %v", err)
	}

	val, err := state.Read(ctx, "counter")
	if err != nil {
		t.Fatalf("read failed: %v", err)
	}

	if string(val) != "42" {
		t.Errorf("expected '42', got '%s'", string(val))
	}
}

func TestActorOnStartStop(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	actor := aether.NewBaseActor("lifecycle-actor")
	started := false
	stopped := false

	// Override lifecycle methods via custom actor
	customActor := &LifecycleActor{
		BaseActor: actor,
		onStart:   func() { started = true },
		onStop:    func() { stopped = true },
	}

	// Run should call OnStart and OnStop
	go func() {
		_ = customActor.Run(ctx)
	}()

	// Wait for context to timeout
	<-ctx.Done()

	if !started {
		t.Error("expected OnStart to be called")
	}

	// Give time for cleanup
	time.Sleep(50 * time.Millisecond)

	if !stopped {
		t.Error("expected OnStop to be called")
	}
}

// LifecycleActor is a test actor with custom lifecycle hooks.
type LifecycleActor struct {
	*aether.BaseActor
	onStart func()
	onStop  func()
}

func (a *LifecycleActor) OnStart(ctx context.Context) error {
	if a.onStart != nil {
		a.onStart()
	}
	return nil
}

func (a *LifecycleActor) OnStop(ctx context.Context) error {
	if a.onStop != nil {
		a.onStop()
	}
	return nil
}
