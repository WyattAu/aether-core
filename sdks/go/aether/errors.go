package aether

import "fmt"

// Error represents an error in the Aether SDK.
type Error struct {
	// Code is the error code.
	Code string
	// Message is the error message.
	Message string
	// Cause is the underlying error.
	Cause error
}

// Error implements the error interface.
func (e *Error) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("[%s] %s: %v", e.Code, e.Message, e.Cause)
	}
	return fmt.Sprintf("[%s] %s", e.Code, e.Message)
}

// Unwrap returns the underlying error.
func (e *Error) Unwrap() error {
	return e.Cause
}

// NewError creates a new Error.
func NewError(code, message string, cause error) *Error {
	return &Error{
		Code:    code,
		Message: message,
		Cause:   cause,
	}
}

// Error codes.
const (
	// ErrCodeCapabilityDenied is returned when a capability is not granted.
	ErrCodeCapabilityDenied = "CAPABILITY_DENIED"
	// ErrCodeActorNotFound is returned when an actor is not found.
	ErrCodeActorNotFound = "ACTOR_NOT_FOUND"
	// ErrCodeRpcError is returned when an RPC call fails.
	ErrCodeRpcError = "RPC_ERROR"
	// ErrCodeTimeout is returned when an operation times out.
	ErrCodeTimeout = "TIMEOUT"
	// ErrCodeInternal is returned for internal errors.
	ErrCodeInternal = "INTERNAL_ERROR"
	// ErrCodeInvalidMessage is returned for invalid messages.
	ErrCodeInvalidMessage = "INVALID_MESSAGE"
	// ErrCodeStateError is returned for state operation errors.
	ErrCodeStateError = "STATE_ERROR"
	// ErrCodeRpcError is returned for RPC errors.
	ErrCodeRpcError = "RPC_ERROR"
)

// CapabilityDenied creates a capability denied error.
func CapabilityDenied(capability string) *Error {
	return &Error{
		Code:    ErrCodeCapabilityDenied,
		Message: fmt.Sprintf("capability denied: %s", capability),
	}
}

// ActorNotFound creates an actor not found error.
func ActorNotFound(actorID string) *Error {
	return &Error{
		Code:    ErrCodeActorNotFound,
		Message: fmt.Sprintf("actor not found: %s", actorID),
	}
}

// RpcError creates an RPC error.
func RpcError(message string, cause error) *Error {
	return &Error{
		Code:    ErrCodeRpcError,
		Message: message,
		Cause:   cause,
	}
}

// Timeout creates a timeout error.
func Timeout(operation string) *Error {
	return &Error{
		Code:    ErrCodeTimeout,
		Message: fmt.Sprintf("operation timed out: %s", operation),
	}
}

// InternalError creates an internal error.
func InternalError(message string, cause error) *Error {
	return &Error{
		Code:    ErrCodeInternal,
		Message: message,
		Cause:   cause,
	}
}

// InvalidMessage creates an invalid message error.
func InvalidMessage(reason string) *Error {
	return &Error{
		Code:    ErrCodeInvalidMessage,
		Message: fmt.Sprintf("invalid message: %s", reason),
	}
}

// StateError creates a state operation error.
func StateError(operation string, cause error) *Error {
	return &Error{
		Code:    ErrCodeStateError,
		Message: fmt.Sprintf("state operation failed: %s", operation),
		Cause:   cause,
	}
}

// IsCapabilityDenied checks if an error is a capability denied error.
func IsCapabilityDenied(err error) bool {
	if e, ok := err.(*Error); ok {
		return e.Code == ErrCodeCapabilityDenied
	}
	return false
}

// IsActorNotFound checks if an error is an actor not found error.
func IsActorNotFound(err error) bool {
	if e, ok := err.(*Error); ok {
		return e.Code == ErrCodeActorNotFound
	}
	return false
}

// IsTimeout checks if an error is a timeout error.
func IsTimeout(err error) bool {
	if e, ok := err.(*Error); ok {
		return e.Code == ErrCodeTimeout
	}
	return false
}
