package aether

import (
	"encoding/json"
	"time"
)

// MessageType represents the type of a message.
type MessageType string

const (
	// MessageTypeRequest is a request message.
	MessageTypeRequest MessageType = "request"
	// MessageTypeResponse is a response message.
	MessageTypeResponse MessageType = "response"
	// MessageTypeEvent is an event notification.
	MessageTypeEvent MessageType = "event"
	// MessageTypeRPCRequest is an RPC request.
	MessageTypeRPCRequest MessageType = "rpc_request"
	// MessageTypeRPCResponse is an RPC response.
	MessageTypeRPCResponse MessageType = "rpc_response"
	// MessageTypeError is an error message.
	MessageTypeError MessageType = "error"
)

// Priority represents message priority.
type Priority int

const (
	// PriorityLow is low priority.
	PriorityLow Priority = iota
	// PriorityNormal is normal priority.
	PriorityNormal
	// PriorityHigh is high priority.
	PriorityHigh
	// PriorityCritical is critical priority.
	PriorityCritical
)

// Message represents a message sent between actors.
type Message struct {
	// Type is the message type.
	Type MessageType `json:"type"`
	// Payload is the message payload.
	Payload any `json:"payload"`
	// Sender is the sender actor ID.
	Sender string `json:"sender,omitempty"`
	// CorrelationID is used for request-response correlation.
	CorrelationID string `json:"correlation_id,omitempty"`
	// Priority is the message priority.
	Priority Priority `json:"priority,omitempty"`
	// Timestamp is when the message was created.
	Timestamp time.Time `json:"timestamp"`
	// Metadata contains additional message metadata.
	Metadata map[string]string `json:"metadata,omitempty"`
}

// NewMessage creates a new message with the given type and payload.
func NewMessage(msgType MessageType, payload any) *Message {
	return &Message{
		Type:      msgType,
		Payload:   payload,
		Priority:  PriorityNormal,
		Timestamp: time.Now(),
		Metadata:  make(map[string]string),
	}
}

// NewResponse creates a response message for a request.
func NewResponse(request *Message, payload any) *Message {
	return &Message{
		Type:          MessageTypeResponse,
		Payload:       payload,
		Sender:        request.Sender,
		CorrelationID: request.CorrelationID,
		Priority:      request.Priority,
		Timestamp:     time.Now(),
		Metadata:      make(map[string]string),
	}
}

// NewRPCRequest creates an RPC request message.
func NewRPCRequest(sender string, payload any, correlationID string) *Message {
	return &Message{
		Type:          MessageTypeRPCRequest,
		Payload:       payload,
		Sender:        sender,
		CorrelationID: correlationID,
		Priority:      PriorityNormal,
		Timestamp:     time.Now(),
		Metadata:      make(map[string]string),
	}
}

// NewRPCResponse creates an RPC response message.
func NewRPCResponse(request *Message, payload any) *Message {
	return &Message{
		Type:          MessageTypeRPCResponse,
		Payload:       payload,
		CorrelationID: request.CorrelationID,
		Priority:      request.Priority,
		Timestamp:     time.Now(),
		Metadata:      make(map[string]string),
	}
}

// WithPriority sets the message priority.
func (m *Message) WithPriority(p Priority) *Message {
	m.Priority = p
	return m
}

// WithMetadata adds metadata to the message.
func (m *Message) WithMetadata(key, value string) *Message {
	if m.Metadata == nil {
		m.Metadata = make(map[string]string)
	}
	m.Metadata[key] = value
	return m
}

// ToJSON serializes the message to JSON.
func (m *Message) ToJSON() ([]byte, error) {
	return json.Marshal(m)
}

// FromJSON deserializes a message from JSON.
func FromJSON(data []byte) (*Message, error) {
	var msg Message
	if err := json.Unmarshal(data, &msg); err != nil {
		return nil, err
	}
	return &msg, nil
}

// IsRPC returns true if this is an RPC message.
func (m *Message) IsRPC() bool {
	return m.Type == MessageTypeRPCRequest || m.Type == MessageTypeRPCResponse
}

// IsRequest returns true if this is a request message.
func (m *Message) IsRequest() bool {
	return m.Type == MessageTypeRequest || m.Type == MessageTypeRPCRequest
}

// IsResponse returns true if this is a response message.
func (m *Message) IsResponse() bool {
	return m.Type == MessageTypeResponse || m.Type == MessageTypeRPCResponse
}
