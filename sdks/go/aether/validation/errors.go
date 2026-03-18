package validation

import (
	"encoding/json"
	"errors"
	"fmt"
)

// Common validation errors.
var (
	ErrInvalidScheme      = errors.New("invalid URL scheme")
	ErrInvalidEmail       = errors.New("invalid email format")
	ErrInvalidURL         = errors.New("invalid URL format")
	ErrInvalidUUID        = errors.New("invalid UUID format")
	ErrInvalidPhone       = errors.New("invalid phone number format")
	ErrInvalidSlug        = errors.New("invalid slug format")
	ErrRequired           = errors.New("field is required")
	ErrMinLength          = errors.New("minimum length not met")
	ErrMaxLength          = errors.New("maximum length exceeded")
	ErrMinValue           = errors.New("minimum value not met")
	ErrMaxValue           = errors.New("maximum value exceeded")
	ErrInvalidType        = errors.New("invalid type")
	ErrInvalidFormat      = errors.New("invalid format")
	ErrInvalidEnum        = errors.New("invalid enum value")
)

// ValidationError represents a validation error.
type ValidationError struct {
	Field   string   `json:"field"`
	Message string   `json:"message"`
}

func (e ValidationError) Error() string {
	return fmt.Sprintf("%s: %s", e.Field, e.Message)
}

// ValidationErrors is a collection of validation errors.
type ValidationErrors struct {
	Errors []ValidationError `json:"errors"`
}

func (e ValidationErrors) Error() string {
	if len(e.Errors) == 0 {
		return "validation failed"
	}
	if len(e.Errors) == 1 {
		return e.Errors[0].Error()
	}
	return fmt.Sprintf("%d validation errors", len(e.Errors))
}

// Add adds a validation error.
func (e *ValidationErrors) Add(field, message string) {
	e.Errors = append(e.Errors, ValidationError{
		Field:   field,
		Message: message,
	})
}

// HasErrors returns true if there are any errors.
func (e *ValidationErrors) HasErrors() bool {
	return len(e.Errors) > 0
}

// ToMap converts errors to a map.
func (e *ValidationErrors) ToMap() map[string][]string {
	result := make(map[string][]string)
	for _, err := range e.Errors {
		result[err.Field] = append(result[err.Field], err.Message)
	}
	return result
}

// ToJSON converts errors to JSON.
func (e *ValidationErrors) ToJSON() string {
	data, _ := json.Marshal(e)
	return string(data)
}

// SchemaValidationError represents a schema validation error.
type SchemaValidationError struct {
	Path    string      `json:"path"`
	Message string      `json:"message"`
	Value   interface{} `json:"value,omitempty"`
}

func (e SchemaValidationError) Error() string {
	return fmt.Sprintf("%s: %s", e.Path, e.Message)
}

// SchemaValidationErrors is a collection of schema validation errors.
type SchemaValidationErrors struct {
	Errors []SchemaValidationError `json:"errors"`
}

func (e SchemaValidationErrors) Error() string {
	if len(e.Errors) == 0 {
		return "schema validation failed"
	}
	return fmt.Sprintf("schema validation failed with %d errors", len(e.Errors))
}

// Add adds a schema validation error.
func (e *SchemaValidationErrors) Add(path, message string, value interface{}) {
	e.Errors = append(e.Errors, SchemaValidationError{
		Path:    path,
		Message: message,
		Value:   value,
	})
}
