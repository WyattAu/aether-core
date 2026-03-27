package aether

import (
	"encoding/json"
	"testing"
	"time"
)

func TestNewMessage(t *testing.T) {
	msg := NewMessage(MessageTypeRequest, "payload")

	if msg.Type != MessageTypeRequest {
		t.Errorf("expected type %q, got %q", MessageTypeRequest, msg.Type)
	}
	if msg.Payload != "payload" {
		t.Errorf("expected payload 'payload', got %v", msg.Payload)
	}
	if msg.Priority != PriorityNormal {
		t.Errorf("expected normal priority, got %v", msg.Priority)
	}
	if msg.Timestamp.IsZero() {
		t.Error("expected non-zero timestamp")
	}
	if msg.Metadata == nil {
		t.Error("expected initialized metadata")
	}
}

func TestNewMessage_NilPayload(t *testing.T) {
	msg := NewMessage(MessageTypeEvent, nil)
	if msg.Payload != nil {
		t.Errorf("expected nil payload, got %v", msg.Payload)
	}
}

func TestNewResponse(t *testing.T) {
	tests := []struct {
		name          string
		requestSender string
		correlationID string
		priority      Priority
	}{
		{"basic", "caller", "corr-123", PriorityNormal},
		{"high priority", "sender", "c1", PriorityHigh},
		{"empty", "", "", PriorityNormal},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &Message{
				Sender:        tt.requestSender,
				CorrelationID: tt.correlationID,
				Priority:      tt.priority,
			}
			resp := NewResponse(req, "result")

			if resp.Type != MessageTypeResponse {
				t.Errorf("expected type %q, got %q", MessageTypeResponse, resp.Type)
			}
			if resp.CorrelationID != tt.correlationID {
				t.Errorf("expected correlation ID %q, got %q", tt.correlationID, resp.CorrelationID)
			}
			if resp.Priority != tt.priority {
				t.Errorf("expected priority %v, got %v", tt.priority, resp.Priority)
			}
		})
	}
}

func TestNewRPCRequest(t *testing.T) {
	msg := NewRPCRequest("sender", "data", "corr-id")

	if msg.Type != MessageTypeRPCRequest {
		t.Errorf("expected type %q, got %q", MessageTypeRPCRequest, msg.Type)
	}
	if msg.Sender != "sender" {
		t.Errorf("expected sender 'sender', got %q", msg.Sender)
	}
	if msg.CorrelationID != "corr-id" {
		t.Errorf("expected correlation ID 'corr-id', got %q", msg.CorrelationID)
	}
	if msg.Metadata == nil {
		t.Error("expected initialized metadata")
	}
}

func TestNewRPCResponse(t *testing.T) {
	req := &Message{CorrelationID: "c1", Priority: PriorityCritical}
	resp := NewRPCResponse(req, "result")

	if resp.Type != MessageTypeRPCResponse {
		t.Errorf("expected type %q, got %q", MessageTypeRPCResponse, resp.Type)
	}
	if resp.CorrelationID != "c1" {
		t.Errorf("expected correlation ID 'c1', got %q", resp.CorrelationID)
	}
	if resp.Priority != PriorityCritical {
		t.Errorf("expected critical priority, got %v", resp.Priority)
	}
}

func TestMessage_WithPriority(t *testing.T) {
	msg := NewMessage(MessageTypeRequest, "data")
	result := msg.WithPriority(PriorityHigh)

	if msg.Priority != PriorityHigh {
		t.Errorf("expected priority %v, got %v", PriorityHigh, msg.Priority)
	}
	if result != msg {
		t.Error("WithPriority should return same message")
	}
}

func TestMessage_WithMetadata(t *testing.T) {
	msg := NewMessage(MessageTypeRequest, "data")

	result := msg.WithMetadata("key1", "val1").WithMetadata("key2", "val2")

	if result.Metadata["key1"] != "val1" {
		t.Errorf("expected metadata key1=val1, got %q", result.Metadata["key1"])
	}
	if result.Metadata["key2"] != "val2" {
		t.Errorf("expected metadata key2=val2, got %q", result.Metadata["key2"])
	}
}

func TestMessage_WithMetadata_NilMap(t *testing.T) {
	msg := &Message{}
	msg.WithMetadata("k", "v")
	if msg.Metadata["k"] != "v" {
		t.Error("WithMetadata should initialize nil map")
	}
}

func TestMessage_ToJSON(t *testing.T) {
	msg := NewMessage(MessageTypeRequest, "data")
	msg.Sender = "test"
	msg.WithMetadata("key", "value")

	data, err := msg.ToJSON()
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if len(data) == 0 {
		t.Error("expected non-empty JSON")
	}

	var parsed map[string]interface{}
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Errorf("failed to parse JSON: %v", err)
	}
	if parsed["type"] != "request" {
		t.Errorf("expected type 'request', got %v", parsed["type"])
	}
}

func TestFromJSON(t *testing.T) {
	msg := NewMessage(MessageTypeRequest, "data")
	msg.Sender = "sender"
	msg.CorrelationID = "corr-1"

	data, _ := msg.ToJSON()

	parsed, err := FromJSON(data)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if parsed.Type != MessageTypeRequest {
		t.Errorf("expected type %q, got %q", MessageTypeRequest, parsed.Type)
	}
	if parsed.Sender != "sender" {
		t.Errorf("expected sender 'sender', got %q", parsed.Sender)
	}
}

func TestFromJSON_InvalidInput(t *testing.T) {
	_, err := FromJSON([]byte("not json"))
	if err == nil {
		t.Error("expected error for invalid JSON")
	}
	_, err = FromJSON(nil)
	if err == nil {
		t.Error("expected error for nil input")
	}
}

func TestMessage_IsRPC(t *testing.T) {
	tests := []struct {
		msgType MessageType
		want    bool
	}{
		{MessageTypeRPCRequest, true},
		{MessageTypeRPCResponse, true},
		{MessageTypeRequest, false},
		{MessageTypeResponse, false},
		{MessageTypeEvent, false},
		{MessageTypeError, false},
	}
	for _, tt := range tests {
		msg := &Message{Type: tt.msgType}
		if got := msg.IsRPC(); got != tt.want {
			t.Errorf("IsRPC() for %q = %v, want %v", tt.msgType, got, tt.want)
		}
	}
}

func TestMessage_IsRequest(t *testing.T) {
	tests := []struct {
		msgType MessageType
		want    bool
	}{
		{MessageTypeRequest, true},
		{MessageTypeRPCRequest, true},
		{MessageTypeResponse, false},
		{MessageTypeRPCResponse, false},
		{MessageTypeEvent, false},
	}
	for _, tt := range tests {
		msg := &Message{Type: tt.msgType}
		if got := msg.IsRequest(); got != tt.want {
			t.Errorf("IsRequest() for %q = %v, want %v", tt.msgType, got, tt.want)
		}
	}
}

func TestMessage_IsResponse(t *testing.T) {
	tests := []struct {
		msgType MessageType
		want    bool
	}{
		{MessageTypeResponse, true},
		{MessageTypeRPCResponse, true},
		{MessageTypeRequest, false},
		{MessageTypeRPCRequest, false},
		{MessageTypeEvent, false},
	}
	for _, tt := range tests {
		msg := &Message{Type: tt.msgType}
		if got := msg.IsResponse(); got != tt.want {
			t.Errorf("IsResponse() for %q = %v, want %v", tt.msgType, got, tt.want)
		}
	}
}

func TestPriority_Values(t *testing.T) {
	if PriorityLow != 0 {
		t.Error("PriorityLow should be 0")
	}
	if PriorityNormal != 1 {
		t.Error("PriorityNormal should be 1")
	}
	if PriorityHigh != 2 {
		t.Error("PriorityHigh should be 2")
	}
	if PriorityCritical != 3 {
		t.Error("PriorityCritical should be 3")
	}
}

func TestMessage_JSONRoundTrip(t *testing.T) {
	original := NewMessage(MessageTypeRPCRequest, map[string]int{"key": 42})
	original.Sender = "test-sender"
	original.CorrelationID = "test-corr"

	data, err := original.ToJSON()
	if err != nil {
		t.Fatalf("ToJSON error: %v", err)
	}

	parsed, err := FromJSON(data)
	if err != nil {
		t.Fatalf("FromJSON error: %v", err)
	}

	if parsed.Type != original.Type {
		t.Errorf("type mismatch: %q vs %q", parsed.Type, original.Type)
	}
	if parsed.Sender != original.Sender {
		t.Errorf("sender mismatch: %q vs %q", parsed.Sender, original.Sender)
	}
}

func TestMessage_Timestamp(t *testing.T) {
	before := time.Now()
	msg := NewMessage(MessageTypeEvent, "data")
	after := time.Now()

	if msg.Timestamp.Before(before) || msg.Timestamp.After(after) {
		t.Error("message timestamp should be between before and after")
	}
}
