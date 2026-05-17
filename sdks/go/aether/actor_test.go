package aether

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"
)

func TestNewBaseActor(t *testing.T) {
	actor := NewBaseActor("test-actor")
	if actor.name != "test-actor" {
		t.Errorf("expected name 'test-actor', got %q", actor.name)
	}
	if actor.capabilities == nil {
		t.Error("expected capabilities to be initialized")
	}
	if actor.state == nil {
		t.Error("expected state to be initialized")
	}
	if actor.mailbox == nil {
		t.Error("expected mailbox to be initialized")
	}
	if actor.IsRunning() {
		t.Error("new actor should not be running")
	}
}

func TestBaseActor_Name(t *testing.T) {
	tests := []struct {
		name string
		want string
	}{
		{"", ""},
		{"my-actor", "my-actor"},
		{"actor-123", "actor-123"},
	}
	for _, tt := range tests {
		actor := NewBaseActor(tt.name)
		if got := actor.Name(); got != tt.want {
			t.Errorf("Name() = %q, want %q", got, tt.want)
		}
	}
}

func TestBaseActor_HandleMessage_Default(t *testing.T) {
	actor := NewBaseActor("test")
	resp, err := actor.HandleMessage(context.Background(), "sender", &Message{Type: MessageTypeRequest})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if resp != nil {
		t.Errorf("expected nil response, got %v", resp)
	}
}

func TestBaseActor_OnStart_OnStop(t *testing.T) {
	startCalled := false
	stopCalled := false

	actor := NewBaseActor("test")
	actor.OnStartFunc = func(ctx context.Context) error {
		startCalled = true
		return nil
	}
	actor.OnStopFunc = func(ctx context.Context) error {
		stopCalled = true
		return nil
	}

	actor.OnStart(context.Background())
	if !startCalled {
		t.Error("OnStart should have been called")
	}
	actor.OnStop(context.Background())
	if !stopCalled {
		t.Error("OnStop should have been called")
	}
}

func TestBaseActor_OnStart_Error(t *testing.T) {
	actor := NewBaseActor("test")
	actor.OnStartFunc = func(ctx context.Context) error {
		return errors.New("start failed")
	}

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	err := actor.Run(ctx)
	if err == nil {
		t.Error("expected error from Run when OnStart fails")
	}
}

func TestBaseActor_Require(t *testing.T) {
	actor := NewBaseActor("test")
	actor.Require(CapabilityNetworkOutbound, CapabilityStateRead)

	if !actor.Capabilities().Has(CapabilityNetworkOutbound) {
		t.Error("expected NETWORK_OUTBOUND capability")
	}
	if !actor.Capabilities().Has(CapabilityStateRead) {
		t.Error("expected STATE_READ capability")
	}
}

func TestBaseActor_State(t *testing.T) {
	actor := NewBaseActor("test")
	state := actor.State()

	ctx := context.Background()
	err := state.Write(ctx, "key", []byte("value"))
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	val, err := state.Read(ctx, "key")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if string(val) != "value" {
		t.Errorf("expected 'value', got %q", string(val))
	}
}

func TestBaseActor_Send(t *testing.T) {
	actor := NewBaseActor("sender")
	msg := NewMessage(MessageTypeRequest, "payload")

	err := actor.Send(context.Background(), "target", msg)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if msg.Sender != "sender" {
		t.Errorf("expected sender 'sender', got %q", msg.Sender)
	}
}

func TestBaseActor_Deliver(t *testing.T) {
	actor := NewBaseActor("test")
	msg := NewMessage(MessageTypeEvent, "data")

	actor.Deliver("sender", msg)

	select {
	case item := <-actor.mailbox:
		if item.action != "receive" {
			t.Errorf("expected action 'receive', got %q", item.action)
		}
		if item.sender != "sender" {
			t.Errorf("expected sender 'sender', got %q", item.sender)
		}
	default:
		t.Error("expected item in mailbox")
	}
}

func TestBaseActor_Run_ContextCancel(t *testing.T) {
	actor := NewBaseActor("test")

	ctx, cancel := context.WithCancel(context.Background())
	go func() {
		time.Sleep(50 * time.Millisecond)
		cancel()
	}()

	err := actor.Run(ctx)
	if err == nil {
		t.Error("expected error from Run when context canceled")
	}
	if actor.IsRunning() {
		t.Error("actor should not be running after Run returns")
	}
}

func TestBaseActor_Stop(t *testing.T) {
	actor := NewBaseActor("test")
	actor.running = true

	actor.Stop()
	if actor.IsRunning() {
		t.Error("actor should not be running after Stop")
	}
}

func TestBaseActor_IsRunning(t *testing.T) {
	actor := NewBaseActor("test")

	if actor.IsRunning() {
		t.Error("new actor should not be running")
	}

	actor.running = true
	if !actor.IsRunning() {
		t.Error("actor should be running")
	}
}

func TestBaseActor_Call_Timeout(t *testing.T) {
	actor := NewBaseActor("caller")
	actor.mailbox = make(chan mailboxItem, 1000)

	ctx := context.Background()
	_, err := actor.Call(ctx, "target", "request", 10*time.Millisecond)
	if err == nil {
		t.Error("expected timeout error")
	}
}

func TestBaseActor_Call_ContextCancel(t *testing.T) {
	actor := NewBaseActor("caller")

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := actor.Call(ctx, "target", "request", 5*time.Second)
	if err == nil {
		t.Error("expected context error")
	}
}

func TestBaseActor_Run_ProcessesMessages(t *testing.T) {
	actor := NewBaseActor("test")
	received := false
	actor.HandleMessageFunc = func(ctx context.Context, sender string, msg *Message) (*Message, error) {
		received = true
		return nil, nil
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go actor.Run(ctx)
	time.Sleep(50 * time.Millisecond)

	actor.Deliver("sender", &Message{Type: MessageTypeEvent, Payload: "data"})
	time.Sleep(50 * time.Millisecond)
	cancel()

	if !received {
		t.Error("message should have been processed")
	}
}

func TestBaseActor_ConcurrentAccess(t *testing.T) {
	actor := NewBaseActor("test")
	var wg sync.WaitGroup

	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_ = actor.Name()
			_ = actor.IsRunning()
			_ = actor.Capabilities()
			_ = actor.State()
		}()
	}
	wg.Wait()
}

func TestBaseActor_ImplementsActorInterface(t *testing.T) {
	var _ Actor = NewBaseActor("test")
}
