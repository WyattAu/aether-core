// Package validation provides input validation, schema validation, and sanitization utilities.
package validation

// Validator provides fluent validation for building validation rules.
//
// Example:
//
//	v := NewValidator()
//	v.Required("name", name)
//	v.Email("email", email)
//	v.MinLength("password", password, 8)
//
//	if !v.IsValid() {
//	    return ValidationError{Errors: v.Errors}
//	}
type Validator struct {
	Errors map[string][]string
}

// NewValidator creates a new validator instance.
func NewValidator() *Validator {
	return &Validator{
		Errors: make(map[string][]string),
	}
}

// AddError adds an error for a field.
func (v *Validator) AddError(field, message string) *Validator {
	if _, exists := v.Errors[field]; !exists {
		v.Errors[field] = []string{}
	}
	v.Errors[field] = append(v.Errors[field], message)
	return v
}

// IsValid returns true if all validations passed.
func (v *Validator) IsValid() bool {
	return len(v.Errors) == 0
}

// Clear removes all errors.
func (v *Validator) Clear() *Validator {
	v.Errors = make(map[string][]string)
	return v
}

// GetErrors returns all validation errors.
func (v *Validator) GetErrors() map[string][]string {
	return v.Errors
}
