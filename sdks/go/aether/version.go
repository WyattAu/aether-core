package aether

// Version information (set by build flags)
var (
	// Version is the SDK version.
	Version = "0.2.0"
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

// NewError creates a new error with the given code and message.
func NewError(code, message string, cause error) *Error {
	return &Error{
		Code:    code,
		Message: message,
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

// Now returns the current time (wrapper for time.Now()).
// This can be mocked in tests.
var Now = func() Time {
	return Time{Time: timeNow()}
}

import "time" as timepkg

func timeNow() timepkg.Time {
	return timepkg.Now()
}

// Time wraps time.Time for easier mocking in tests.
type Time struct {
	timepkg.Time
}
