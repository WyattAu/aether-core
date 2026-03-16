package aether

// Version information (set by build flags)
var (
	// Version is the SDK version.
	Version = "0.1.0"
	// GitCommit is the git commit hash.
	GitCommit = "unknown"
	// BuildDate is the build date.
	BuildDate = "unknown"
)

// GetVersion returns the SDK version.
func GetVersion() string {
	return Version
}

// NewRPCRequest creates a new RPC request message.
func NewRPCRequest(sender string, payload any, correlationID string) *Message {
	return &Message{
		Type:          MessageTypeRPCRequest,
		Payload:       payload,
		Sender:        sender,
		CorrelationID: correlationID,
		Priority:      PriorityNormal,
		Timestamp:     Now(),
		Metadata:      make(map[string]string),
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

// CapabilityDenied creates a capability denied error.
func CapabilityDenied(capability string) *Error {
	return &Error{
		Code:    ErrCodeCapabilityDenied,
		Message: "capability denied: " + capability,
	}
}

// ActorNotFound creates an actor not found error.
func ActorNotFound(actorID string) *Error {
	return &Error{
		Code:    ErrCodeActorNotFound,
		Message: "actor not found: " + actorID,
	}
}

// Timeout creates a timeout error.
func Timeout(operation string) *Error {
	return &Error{
		Code:    ErrCodeTimeout,
		Message: "operation timed out: " + operation,
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
