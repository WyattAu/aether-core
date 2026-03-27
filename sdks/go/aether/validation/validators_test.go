package validation

import (
	"encoding/json"
	"testing"
	"time"
)

func TestValidator_New(t *testing.T) {
	v := NewValidator()
	if v == nil {
		t.Fatal("expected non-nil validator")
	}
	if !v.IsValid() {
		t.Error("new validator should be valid")
	}
	if v.GetErrors() == nil {
		t.Error("errors map should not be nil")
	}
}

func TestValidator_AddError(t *testing.T) {
	v := NewValidator()
	v.AddError("field", "required")
	if v.IsValid() {
		t.Error("should not be valid after AddError")
	}
	errs := v.GetErrors()
	if len(errs["field"]) != 1 {
		t.Errorf("expected 1 error for 'field', got %d", len(errs["field"]))
	}
}

func TestValidator_Clear(t *testing.T) {
	v := NewValidator()
	v.AddError("f1", "err1")
	v.AddError("f2", "err2")
	v.Clear()
	if !v.IsValid() {
		t.Error("should be valid after clear")
	}
}

func TestValidator_Required(t *testing.T) {
	tests := []struct {
		name  string
		value interface{}
		valid bool
	}{
		{"nil", nil, false},
		{"empty string", "", false},
		{"whitespace string", "  ", false},
		{"non-empty string", "hello", true},
		{"empty slice", []interface{}{}, false},
		{"non-empty slice", []interface{}{1}, true},
		{"empty map", map[string]interface{}{}, false},
		{"non-empty map", map[string]interface{}{"k": "v"}, true},
		{"int", 42, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			v := NewValidator()
			v.Required("field", tt.value)
			if v.IsValid() != tt.valid {
				t.Errorf("Required(%v) validity = %v, want %v", tt.value, v.IsValid(), tt.valid)
			}
		})
	}
}

func TestValidator_TypeChecks(t *testing.T) {
	t.Run("String valid", func(t *testing.T) {
		v := NewValidator()
		v.String("field", "hello")
		if !v.IsValid() {
			t.Error("string value should be valid")
		}
	})
	t.Run("String invalid", func(t *testing.T) {
		v := NewValidator()
		v.String("field", 42)
		if v.IsValid() {
			t.Error("int value should fail String check")
		}
	})
	t.Run("Int valid", func(t *testing.T) {
		v := NewValidator()
		v.Int("field", 42)
		if !v.IsValid() {
			t.Error("int value should be valid")
		}
	})
	t.Run("Int from float64", func(t *testing.T) {
		v := NewValidator()
		v.Int("field", float64(5))
		if !v.IsValid() {
			t.Error("whole float64 should be valid int")
		}
	})
	t.Run("Int from float64 fraction", func(t *testing.T) {
		v := NewValidator()
		v.Int("field", float64(5.5))
		if v.IsValid() {
			t.Error("fractional float64 should fail int check")
		}
	})
	t.Run("Bool valid", func(t *testing.T) {
		v := NewValidator()
		v.Bool("field", true)
		if !v.IsValid() {
			t.Error("bool value should be valid")
		}
	})
	t.Run("Bool invalid", func(t *testing.T) {
		v := NewValidator()
		v.Bool("field", "true")
		if v.IsValid() {
			t.Error("string should fail Bool check")
		}
	})
	t.Run("Array valid", func(t *testing.T) {
		v := NewValidator()
		v.Array("field", []interface{}{1})
		if !v.IsValid() {
			t.Error("slice should pass Array check")
		}
	})
	t.Run("Array invalid", func(t *testing.T) {
		v := NewValidator()
		v.Array("field", "not array")
		if v.IsValid() {
			t.Error("string should fail Array check")
		}
	})
	t.Run("Object valid", func(t *testing.T) {
		v := NewValidator()
		v.Object("field", map[string]interface{}{"k": "v"})
		if !v.IsValid() {
			t.Error("map should pass Object check")
		}
	})
	t.Run("Float valid", func(t *testing.T) {
		v := NewValidator()
		v.Float("field", 3.14)
		if !v.IsValid() {
			t.Error("float should be valid")
		}
	})
}

func TestValidator_MinLength(t *testing.T) {
	v := NewValidator()
	v.MinLength("field", "hi", 5)
	if v.IsValid() {
		t.Error("should fail min length 5")
	}

	v = NewValidator()
	v.MinLength("field", "hello", 3)
	if !v.IsValid() {
		t.Error("should pass min length 3")
	}
}

func TestValidator_MaxLength(t *testing.T) {
	v := NewValidator()
	v.MaxLength("field", "hello world", 5)
	if v.IsValid() {
		t.Error("should fail max length 5")
	}

	v = NewValidator()
	v.MaxLength("field", "hi", 5)
	if !v.IsValid() {
		t.Error("should pass max length 5")
	}
}

func TestValidator_Pattern(t *testing.T) {
	v := NewValidator()
	v.Pattern("field", "abc123", AlphanumericPattern)
	if !v.IsValid() {
		t.Error("should pass alphanumeric pattern")
	}

	v = NewValidator()
	v.Pattern("field", "abc 123", AlphanumericPattern)
	if v.IsValid() {
		t.Error("should fail alphanumeric pattern with space")
	}
}

func TestValidator_MinValue(t *testing.T) {
	v := NewValidator()
	v.MinValue("field", 3, 5)
	if v.IsValid() {
		t.Error("3 should fail min value 5")
	}

	v = NewValidator()
	v.MinValue("field", 10, 5)
	if !v.IsValid() {
		t.Error("10 should pass min value 5")
	}
}

func TestValidator_MaxValue(t *testing.T) {
	v := NewValidator()
	v.MaxValue("field", 10, 5)
	if v.IsValid() {
		t.Error("10 should fail max value 5")
	}

	v = NewValidator()
	v.MaxValue("field", 3, 5)
	if !v.IsValid() {
		t.Error("3 should pass max value 5")
	}
}

func TestValidator_Range(t *testing.T) {
	v := NewValidator()
	v.Range("field", 5, 1, 10)
	if !v.IsValid() {
		t.Error("5 should pass range 1-10")
	}

	v = NewValidator()
	v.Range("field", 0, 1, 10)
	if v.IsValid() {
		t.Error("0 should fail range 1-10")
	}
}

func TestValidator_Email(t *testing.T) {
	v := NewValidator()
	v.Email("field", "test@example.com")
	if !v.IsValid() {
		t.Error("valid email should pass")
	}

	v = NewValidator()
	v.Email("field", "invalid")
	if v.IsValid() {
		t.Error("invalid email should fail")
	}
}

func TestValidator_URL(t *testing.T) {
	v := NewValidator()
	v.URL("field", "https://example.com")
	if !v.IsValid() {
		t.Error("valid URL should pass")
	}

	v = NewValidator()
	v.URL("field", "not a url")
	if v.IsValid() {
		t.Error("invalid URL should fail")
	}
}

func TestValidator_UUID(t *testing.T) {
	v := NewValidator()
	v.UUID("field", "550e8400-e29b-41d4-a716-446655440000")
	if !v.IsValid() {
		t.Error("valid UUID should pass")
	}

	v = NewValidator()
	v.UUID("field", "not-a-uuid")
	if v.IsValid() {
		t.Error("invalid UUID should fail")
	}
}

func TestValidator_Phone(t *testing.T) {
	v := NewValidator()
	v.Phone("field", "+15551234567")
	if !v.IsValid() {
		t.Error("valid phone should pass")
	}

	v = NewValidator()
	v.Phone("field", "abc")
	if v.IsValid() {
		t.Error("invalid phone should fail")
	}
}

func TestValidator_Slug(t *testing.T) {
	v := NewValidator()
	v.Slug("field", "my-slug-123")
	if !v.IsValid() {
		t.Error("valid slug should pass")
	}

	v = NewValidator()
	v.Slug("field", "invalid slug!")
	if v.IsValid() {
		t.Error("invalid slug should fail")
	}
}

func TestValidator_Enum(t *testing.T) {
	v := NewValidator()
	v.Enum("field", "b", []interface{}{"a", "b", "c"})
	if !v.IsValid() {
		t.Error("'b' should be in allowed values")
	}

	v = NewValidator()
	v.Enum("field", "d", []interface{}{"a", "b", "c"})
	if v.IsValid() {
		t.Error("'d' should not be in allowed values")
	}
}

func TestValidator_MinItems(t *testing.T) {
	v := NewValidator()
	v.MinItems("field", []interface{}{1}, 2)
	if v.IsValid() {
		t.Error("1 item should fail min items 2")
	}
}

func TestValidator_MaxItems(t *testing.T) {
	v := NewValidator()
	v.MaxItems("field", []interface{}{1, 2, 3}, 2)
	if v.IsValid() {
		t.Error("3 items should fail max items 2")
	}
}

func TestValidator_DateTime(t *testing.T) {
	v := NewValidator()
	v.DateTime("field", "2024-01-15T10:30:00Z", time.RFC3339)
	if !v.IsValid() {
		t.Error("valid datetime should pass")
	}

	v = NewValidator()
	v.DateTime("field", "not a date", time.RFC3339)
	if v.IsValid() {
		t.Error("invalid datetime should fail")
	}
}

func TestValidator_Custom(t *testing.T) {
	v := NewValidator()
	v.Custom("field", "test-value", func(v interface{}) bool {
		return v.(string) == "test-value"
	}, "value must be test-value")
	if !v.IsValid() {
		t.Error("custom validator should pass for matching value")
	}

	v = NewValidator()
	v.Custom("field", "other", func(v interface{}) bool {
		return v.(string) == "test-value"
	}, "value must be test-value")
	if v.IsValid() {
		t.Error("custom validator should fail for non-matching value")
	}
}

func TestValidator_When(t *testing.T) {
	v := NewValidator()
	v.When(true, func(v *Validator) {
		v.Required("field", "")
	})
	if v.IsValid() {
		t.Error("conditional validation should have run")
	}

	v = NewValidator()
	v.When(false, func(v *Validator) {
		v.Required("field", "")
	})
	if !v.IsValid() {
		t.Error("conditional validation should not run when false")
	}
}

func TestValidateEmail(t *testing.T) {
	tests := []struct {
		email string
		valid bool
	}{
		{"test@example.com", true},
		{"user.name+tag@domain.co.uk", true},
		{"", false},
		{"invalid", false},
		{"@", false},
		{"@domain.com", false},
	}
	for _, tt := range tests {
		if got := ValidateEmail(tt.email); got != tt.valid {
			t.Errorf("ValidateEmail(%q) = %v, want %v", tt.email, got, tt.valid)
		}
	}
}

func TestValidateURL(t *testing.T) {
	tests := []struct {
		url   string
		valid bool
	}{
		{"https://example.com", true},
		{"http://example.com/path", true},
		{"ftp://files.example.com", true},
		{"", false},
		{"not-a-url", false},
		{"javascript:alert(1)", false},
		{"//example.com", false},
	}
	for _, tt := range tests {
		if got := ValidateURL(tt.url); got != tt.valid {
			t.Errorf("ValidateURL(%q) = %v, want %v", tt.url, got, tt.valid)
		}
	}
}

func TestValidateUUID(t *testing.T) {
	tests := []struct {
		uuid  string
		valid bool
	}{
		{"550e8400-e29b-41d4-a716-446655440000", true},
		{"", false},
		{"not-a-uuid", false},
		{"550e8400-e29b-41d4-a716", false},
	}
	for _, tt := range tests {
		if got := ValidateUUID(tt.uuid); got != tt.valid {
			t.Errorf("ValidateUUID(%q) = %v, want %v", tt.uuid, got, tt.valid)
		}
	}
}

func TestValidateAlphanumeric(t *testing.T) {
	tests := []struct {
		s     string
		valid bool
	}{
		{"abc123", true},
		{"ABC", true},
		{"", false},
		{"abc 123", false},
	}
	for _, tt := range tests {
		if got := ValidateAlphanumeric(tt.s); got != tt.valid {
			t.Errorf("ValidateAlphanumeric(%q) = %v, want %v", tt.s, got, tt.valid)
		}
	}
}

func TestValidateUsername(t *testing.T) {
	tests := []struct {
		u     string
		valid bool
	}{
		{"user_name-123", true},
		{"", false},
		{"user name", false},
	}
	for _, tt := range tests {
		if got := ValidateUsername(tt.u); got != tt.valid {
			t.Errorf("ValidateUsername(%q) = %v, want %v", tt.u, got, tt.valid)
		}
	}
}

func TestValidatePhone(t *testing.T) {
	tests := []struct {
		phone string
		valid bool
	}{
		{"+15551234567", true},
		{"", false},
		{"abc", false},
	}
	for _, tt := range tests {
		if got := ValidatePhone(tt.phone); got != tt.valid {
			t.Errorf("ValidatePhone(%q) = %v, want %v", tt.phone, got, tt.valid)
		}
	}
}

func TestValidateSlug(t *testing.T) {
	tests := []struct {
		slug  string
		valid bool
	}{
		{"my-slug-123", true},
		{"", false},
		{"invalid slug", false},
	}
	for _, tt := range tests {
		if got := ValidateSlug(tt.slug); got != tt.valid {
			t.Errorf("ValidateSlug(%q) = %v, want %v", tt.slug, got, tt.valid)
		}
	}
}

func TestValidateIP(t *testing.T) {
	tests := []struct {
		ip    string
		valid bool
	}{
		{"192.168.1.1", true},
		{"255.255.255.255", true},
		{"", false},
		{"999.999.999.999", false},
	}
	for _, tt := range tests {
		if got := ValidateIP(tt.ip); got != tt.valid {
			t.Errorf("ValidateIP(%q) = %v, want %v", tt.ip, got, tt.valid)
		}
	}
}

func TestValidateIPv6(t *testing.T) {
	tests := []struct {
		ip    string
		valid bool
	}{
		{"2001:0db8:85a3:0000:0000:8a2e:0370:7334", true},
		{"", false},
		{"not-ipv6", false},
	}
	for _, tt := range tests {
		if got := ValidateIPv6(tt.ip); got != tt.valid {
			t.Errorf("ValidateIPv6(%q) = %v, want %v", tt.ip, got, tt.valid)
		}
	}
}

func TestValidateInteger(t *testing.T) {
	tests := []struct {
		name  string
		value interface{}
		min   *int64
		max   *int64
		valid bool
	}{
		{"int", 5, nil, nil, true},
		{"int64", int64(100), nil, nil, true},
		{"float64 whole", float64(5), nil, nil, true},
		{"float64 fraction", float64(5.5), nil, nil, false},
		{"json.Number", json.Number("42"), nil, nil, true},
		{"string", "not int", nil, nil, false},
		{"nil", nil, nil, nil, false},
		{"with min", 5, ptrInt64(3), nil, true},
		{"below min", 2, ptrInt64(3), nil, false},
		{"above max", 10, nil, ptrInt64(5), false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ValidateInteger(tt.value, tt.min, tt.max); got != tt.valid {
				t.Errorf("ValidateInteger() = %v, want %v", got, tt.valid)
			}
		})
	}
}

func TestValidateFloat(t *testing.T) {
	tests := []struct {
		name  string
		value interface{}
		min   *float64
		max   *float64
		valid bool
	}{
		{"int", 5, nil, nil, true},
		{"float64", 3.14, nil, nil, true},
		{"string", "not float", nil, nil, false},
		{"with bounds", 5.0, ptrFloat64(1.0), ptrFloat64(10.0), true},
		{"below min", 0.5, ptrFloat64(1.0), nil, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ValidateFloat(tt.value, tt.min, tt.max); got != tt.valid {
				t.Errorf("ValidateFloat() = %v, want %v", got, tt.valid)
			}
		})
	}
}

func TestValidateString(t *testing.T) {
	tests := []struct {
		name     string
		value    interface{}
		minLen   *int
		maxLen   *int
		pattern  bool
		valid    bool
	}{
		{"valid", "hello", nil, nil, false, true},
		{"too short", "hi", ptrInt(5), nil, false, false},
		{"too long", "hello world", nil, ptrInt(5), false, false},
		{"pattern fail", "abc 123", nil, nil, true, false},
		{"pattern pass", "abc123", nil, nil, true, true},
		{"not string", 42, nil, nil, false, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var pat interface{} = AlphanumericPattern
			if !tt.pattern {
				pat = nil
			}
			if got := ValidateString(tt.value, tt.minLen, tt.maxLen, pat.(*import_regexp.Regexp)); got != tt.valid {
				t.Errorf("ValidateString() = %v, want %v", got, tt.valid)
			}
		})
	}
}

func TestValidateDateTime(t *testing.T) {
	if !ValidateDateTime("2024-01-15T10:30:00Z", time.RFC3339) {
		t.Error("valid RFC3339 should pass")
	}
	if ValidateDateTime("not a date", time.RFC3339) {
		t.Error("invalid date should fail")
	}
	if ValidateDateTime("", time.RFC3339) {
		t.Error("empty string should fail")
	}
}

func TestValidateEnum(t *testing.T) {
	if !ValidateEnum("a", []interface{}{"a", "b", "c"}) {
		t.Error("'a' should be in allowed values")
	}
	if ValidateEnum("d", []interface{}{"a", "b", "c"}) {
		t.Error("'d' should not be in allowed values")
	}
}

func TestValidateList(t *testing.T) {
	if !ValidateList([]interface{}{1, 2}, ptrInt(1), ptrInt(5)) {
		t.Error("list of 2 should pass min 1")
	}
	if ValidateList([]interface{}{1}, nil, ptrInt(0)) {
		t.Error("list of 1 should fail max 0")
	}
	if ValidateList("not list", nil, nil) {
		t.Error("non-list should fail")
	}
}

func TestValidateRequired(t *testing.T) {
	if !ValidateRequired("hello") {
		t.Error("non-empty string should be required")
	}
	if ValidateRequired("") {
		t.Error("empty string should not be required")
	}
	if ValidateRequired(nil) {
		t.Error("nil should not be required")
	}
}

func TestValidateNoControlChars(t *testing.T) {
	if !ValidateNoControlChars("hello world") {
		t.Error("normal string should pass")
	}
	if ValidateNoControlChars("hello\x00world") {
		t.Error("string with null byte should fail")
	}
	if !ValidateNoControlChars("line\nbreak") {
		t.Error("newline should be allowed")
	}
}

func ptrInt(v int) *int    { return &v }
func ptrInt64(v int64) *int64 { return &v }
func ptrFloat64(v float64) *float64 { return &v }
