package aether_test

import (
	"testing"

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

func TestCapabilityString(t *testing.T) {
	tests := []struct {
		cap      aether.Capability
		expected string
	}{
		{aether.CapabilityNetworkOutbound, "NETWORK_OUTBOUND"},
		{aether.CapabilityStateRead, "STATE_READ"},
		{aether.CapabilityLog, "LOG"},
		{aether.Capability(999), "UNKNOWN"},
	}

	for _, tt := range tests {
		if got := tt.cap.String(); got != tt.expected {
			t.Errorf("Capability(%d).String() = %s, want %s", tt.cap, got, tt.expected)
		}
	}
}

func TestErrors(t *testing.T) {
	// Test capability denied error
	err := aether.CapabilityDenied("NETWORK_OUTBOUND")
	if !aether.IsCapabilityDenied(err) {
		t.Error("expected capability denied error")
	}
	if err.Code != "CAPABILITY_DENIED" {
		t.Errorf("expected code CAPABILITY_DENIED, got %s", err.Code)
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

func TestAllCapabilities(t *testing.T) {
	all := aether.AllCapabilities()
	caps := make(map[aether.Capability]bool)
	for _, cap := range all.All() {
		caps[cap] = true
	}

	// Check some key capabilities
	if !caps[aether.CapabilityNetworkOutbound] {
		t.Error("expected NETWORK_OUTBOUND in all capabilities")
	}
	if !caps[aether.CapabilityStateRead] {
		t.Error("expected STATE_READ in all capabilities")
	}
}

func TestEmptyCapabilitySet(t *testing.T) {
	empty := aether.EmptyCapabilitySet()
	if empty.Has(aether.CapabilityLog) {
		t.Error("expected empty capability set")
	}
}
