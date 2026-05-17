package aether

import (
	"errors"
	"fmt"
	"testing"
)

func TestNewError(t *testing.T) {
	err := NewError("CODE", "message", nil)
	if err.Code != "CODE" {
		t.Errorf("expected code 'CODE', got %q", err.Code)
	}
	if err.Message != "message" {
		t.Errorf("expected message 'message', got %q", err.Message)
	}
	if err.Cause != nil {
		t.Error("expected nil cause")
	}
}

func TestNewError_WithCause(t *testing.T) {
	cause := errors.New("underlying")
	err := NewError("CODE", "message", cause)

	if err.Cause != cause {
		t.Error("expected cause to be set")
	}
	if !errors.Is(err, cause) {
		t.Error("errors.Is should match cause")
	}
}

func TestError_ErrorFormat(t *testing.T) {
	tests := []struct {
		name string
		err  *Error
		want string
	}{
		{"no cause", NewError("CODE", "msg", nil), "[CODE] msg"},
		{"with cause", NewError("CODE", "msg", errors.New("cause")), "[CODE] msg: cause"},
	}
	for _, tt := range tests {
		if got := tt.err.Error(); got != tt.want {
			t.Errorf("%s: Error() = %q, want %q", tt.name, got, tt.want)
		}
	}
}

func TestError_Unwrap(t *testing.T) {
	cause := errors.New("underlying")
	err := NewError("CODE", "msg", cause)

	unwrapped := err.Unwrap()
	if unwrapped != cause {
		t.Error("Unwrap should return the cause")
	}
}

func TestError_Unwrap_NilCause(t *testing.T) {
	err := NewError("CODE", "msg", nil)
	if err.Unwrap() != nil {
		t.Error("Unwrap should return nil for nil cause")
	}
}

func TestCapabilityDenied(t *testing.T) {
	err := CapabilityDenied("NETWORK_OUTBOUND")
	if !IsCapabilityDenied(err) {
		t.Error("IsCapabilityDenied should return true")
	}
	if err.Code != ErrCodeCapabilityDenied {
		t.Errorf("expected code %q, got %q", ErrCodeCapabilityDenied, err.Code)
	}
	if !IsCapabilityDenied(err) {
		t.Error("IsCapabilityDenied check failed")
	}
}

func TestActorNotFound(t *testing.T) {
	err := ActorNotFound("my-actor")
	if !IsActorNotFound(err) {
		t.Error("IsActorNotFound should return true")
	}
	if err.Code != ErrCodeActorNotFound {
		t.Errorf("expected code %q, got %q", ErrCodeActorNotFound, err.Code)
	}
}

func TestRpcError(t *testing.T) {
	cause := errors.New("connection refused")
	err := RpcError("call failed", cause)
	if err.Code != ErrCodeRpcError {
		t.Errorf("expected code %q, got %q", ErrCodeRpcError, err.Code)
	}
	if err.Cause != cause {
		t.Error("expected cause to be set")
	}
	if !errors.Is(err, cause) {
		t.Error("errors.Is should match cause")
	}
}

func TestTimeout(t *testing.T) {
	err := Timeout("database query")
	if !IsTimeout(err) {
		t.Error("IsTimeout should return true")
	}
	if err.Code != ErrCodeTimeout {
		t.Errorf("expected code %q, got %q", ErrCodeTimeout, err.Code)
	}
}

func TestInternalError(t *testing.T) {
	err := InternalError("something broke", errors.New("root"))
	if err.Code != ErrCodeInternal {
		t.Errorf("expected code %q, got %q", ErrCodeInternal, err.Code)
	}
	if err.Cause == nil {
		t.Error("expected cause to be set")
	}
}

func TestInvalidMessage(t *testing.T) {
	err := InvalidMessage("missing payload")
	if err.Code != ErrCodeInvalidMessage {
		t.Errorf("expected code %q, got %q", ErrCodeInvalidMessage, err.Code)
	}
}

func TestStateError(t *testing.T) {
	cause := errors.New("io error")
	err := StateError("write failed", cause)
	if err.Code != ErrCodeStateError {
		t.Errorf("expected code %q, got %q", ErrCodeStateError, err.Code)
	}
	if !errors.Is(err, cause) {
		t.Error("errors.Is should match cause")
	}
}

func TestIsCapabilityDenied_NonAetherError(t *testing.T) {
	if IsCapabilityDenied(errors.New("other")) {
		t.Error("should return false for non-Aether error")
	}
	if IsCapabilityDenied(nil) {
		t.Error("should return false for nil")
	}
}

func TestIsActorNotFound_NonAetherError(t *testing.T) {
	if IsActorNotFound(errors.New("other")) {
		t.Error("should return false for non-Aether error")
	}
}

func TestIsTimeout_NonAetherError(t *testing.T) {
	if IsTimeout(errors.New("other")) {
		t.Error("should return false for non-Aether error")
	}
}

func TestError_ImplementsError(t *testing.T) {
	var _ error = NewError("CODE", "msg", nil)
	var _ error = CapabilityDenied("cap")
	var _ error = ActorNotFound("id")
	var _ error = Timeout("op")
}

func TestError_ImplementsUnwrap(t *testing.T) {
	cause := errors.New("cause")
	err := NewError("CODE", "msg", cause)
	if !errors.Is(err, cause) {
		t.Error("should support errors.Is via Unwrap")
	}
}

func TestError_FprintfFormat(t *testing.T) {
	err := CapabilityDenied("FS_WRITE")
	expected := fmt.Sprintf("capability denied: FS_WRITE")
	if err.Message != expected {
		t.Errorf("expected %q, got %q", expected, err.Message)
	}
}

func TestError_ErrorCodes(t *testing.T) {
	codes := []string{
		ErrCodeCapabilityDenied,
		ErrCodeActorNotFound,
		ErrCodeRpcError,
		ErrCodeTimeout,
		ErrCodeInternal,
		ErrCodeInvalidMessage,
		ErrCodeStateError,
	}
	for _, code := range codes {
		if code == "" {
			t.Errorf("error code should not be empty")
		}
	}
}
