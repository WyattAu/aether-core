package validation

import (
	"encoding/json"
	"net/mail"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"
	"unicode"
)

// Common regex patterns
var (
	// EmailPattern matches most common email formats
	EmailPattern = regexp.MustCompile(`^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$`)
	
	// UUIDPattern matches UUID format
	UUIDPattern = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)
	
	// AlphanumericPattern matches alphanumeric strings
	AlphanumericPattern = regexp.MustCompile(`^[a-zA-Z0-9]+$`)
	
	// UsernamePattern matches usernames (alphanumeric, underscore, hyphen)
	UsernamePattern = regexp.MustCompile(`^[a-zA-Z0-9_-]+$`)
	
	// PhonePattern matches E.164 phone format
	PhonePattern = regexp.MustCompile(`^\+?[1-9]\d{1,14}$`)
	
	// SlugPattern matches URL slugs
	SlugPattern = regexp.MustCompile(`^[a-z0-9]+(?:-[a-z0-9]+)*$`)
	
	// IPPattern matches IPv4 addresses
	IPPattern = regexp.MustCompile(`^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$`)
	
	// IPv6Pattern matches IPv6 addresses (simplified)
	IPv6Pattern = regexp.MustCompile(`^(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$`)
)

// Required validates that a value is not empty.
func (v *Validator) Required(field string, value interface{}) *Validator {
	if value == nil {
		v.AddError(field, field+" is required")
		return v
	}
	
	switch val := value.(type) {
	case string:
		if strings.TrimSpace(val) == "" {
			v.AddError(field, field+" is required")
		}
	case []interface{}:
		if len(val) == 0 {
			v.AddError(field, field+" is required")
		}
	case map[string]interface{}:
		if len(val) == 0 {
			v.AddError(field, field+" is required")
		}
	}
	
	return v
}

// String validates that a value is a string.
func (v *Validator) String(field string, value interface{}) *Validator {
	if value != nil {
		if _, ok := value.(string); !ok {
			v.AddError(field, field+" must be a string")
		}
	}
	return v
}

// Int validates that a value is an integer.
func (v *Validator) Int(field string, value interface{}) *Validator {
	if value != nil {
		switch value.(type) {
		case int, int8, int16, int32, int64, uint, uint8, uint16, uint32, uint64:
			// Valid integer types
		case float64:
			// JSON unmarshals numbers as float64, check if it's a whole number
			if value.(float64) != float64(int(value.(float64))) {
				v.AddError(field, field+" must be an integer")
			}
		case json.Number:
			// JSON number, try to parse as int
			if _, err := value.(json.Number).Int64(); err != nil {
				v.AddError(field, field+" must be an integer")
			}
		default:
			v.AddError(field, field+" must be an integer")
		}
	}
	return v
}

// Float validates that a value is a number.
func (v *Validator) Float(field string, value interface{}) *Validator {
	if value != nil {
		switch value.(type) {
		case int, int8, int16, int32, int64, uint, uint8, uint16, uint32, uint64, float32, float64:
			// Valid numeric types
		case json.Number:
			// JSON number, valid
		default:
			v.AddError(field, field+" must be a number")
		}
	}
	return v
}

// Bool validates that a value is a boolean.
func (v *Validator) Bool(field string, value interface{}) *Validator {
	if value != nil {
		if _, ok := value.(bool); !ok {
			v.AddError(field, field+" must be a boolean")
		}
	}
	return v
}

// Array validates that a value is an array/slice.
func (v *Validator) Array(field string, value interface{}) *Validator {
	if value != nil {
		if _, ok := value.([]interface{}); !ok {
			v.AddError(field, field+" must be an array")
		}
	}
	return v
}

// Object validates that a value is an object/map.
func (v *Validator) Object(field string, value interface{}) *Validator {
	if value != nil {
		if _, ok := value.(map[string]interface{}); !ok {
			v.AddError(field, field+" must be an object")
		}
	}
	return v
}

// MinLength validates minimum string length.
func (v *Validator) MinLength(field string, value string, minLen int) *Validator {
	if len(value) < minLen {
		v.AddError(field, field+" must be at least "+strconv.Itoa(minLen)+" characters")
	}
	return v
}

// MaxLength validates maximum string length.
func (v *Validator) MaxLength(field string, value string, maxLen int) *Validator {
	if len(value) > maxLen {
		v.AddError(field, field+" must be at most "+strconv.Itoa(maxLen)+" characters")
	}
	return v
}

// Pattern validates string against a regex pattern.
func (v *Validator) Pattern(field string, value string, pattern *regexp.Regexp) *Validator {
	if !pattern.MatchString(value) {
		v.AddError(field, field+" has invalid format")
	}
	return v
}

// MinValue validates minimum numeric value.
func (v *Validator) MinValue(field string, value, minVal float64) *Validator {
	if value < minVal {
		v.AddError(field, field+" must be at least "+strconv.FormatFloat(minVal, 'f', -1, 64))
	}
	return v
}

// MaxValue validates maximum numeric value.
func (v *Validator) MaxValue(field string, value, maxVal float64) *Validator {
	if value > maxVal {
		v.AddError(field, field+" must be at most "+strconv.FormatFloat(maxVal, 'f', -1, 64))
	}
	return v
}

// Range validates numeric value is within range.
func (v *Validator) Range(field string, value, minVal, maxVal float64) *Validator {
	if value < minVal || value > maxVal {
		v.AddError(field, field+" must be between "+strconv.FormatFloat(minVal, 'f', -1, 64)+" and "+strconv.FormatFloat(maxVal, 'f', -1, 64))
	}
	return v
}

// Email validates email format.
func (v *Validator) Email(field string, value string) *Validator {
	if !ValidateEmail(value) {
		v.AddError(field, field+" must be a valid email")
	}
	return v
}

// URL validates URL format.
func (v *Validator) URL(field string, value string) *Validator {
	if !ValidateURL(value) {
		v.AddError(field, field+" must be a valid URL")
	}
	return v
}

// UUID validates UUID format.
func (v *Validator) UUID(field string, value string) *Validator {
	if !ValidateUUID(value) {
		v.AddError(field, field+" must be a valid UUID")
	}
	return v
}

// Phone validates phone number format.
func (v *Validator) Phone(field string, value string) *Validator {
	if !ValidatePhone(value) {
		v.AddError(field, field+" must be a valid phone number")
	}
	return v
}

// Slug validates URL slug format.
func (v *Validator) Slug(field string, value string) *Validator {
	if !ValidateSlug(value) {
		v.AddError(field, field+" must be a valid slug")
	}
	return v
}

// Enum validates value is in allowed list.
func (v *Validator) Enum(field string, value interface{}, allowed []interface{}) *Validator {
	for _, a := range allowed {
		if value == a {
			return v
		}
	}
	v.AddError(field, field+" must be one of the allowed values")
	return v
}

// MinItems validates minimum array length.
func (v *Validator) MinItems(field string, value []interface{}, minItems int) *Validator {
	if len(value) < minItems {
		v.AddError(field, field+" must have at least "+strconv.Itoa(minItems)+" items")
	}
	return v
}

// MaxItems validates maximum array length.
func (v *Validator) MaxItems(field string, value []interface{}, maxItems int) *Validator {
	if len(value) > maxItems {
		v.AddError(field, field+" must have at most "+strconv.Itoa(maxItems)+" items")
	}
	return v
}

// DateTime validates datetime format.
func (v *Validator) DateTime(field string, value string, format string) *Validator {
	if _, err := time.Parse(format, value); err != nil {
		v.AddError(field, field+" must be a valid datetime")
	}
	return v
}

// Custom applies a custom validation function.
func (v *Validator) Custom(field string, value interface{}, validator func(interface{}) bool, message string) *Validator {
	if !validator(value) {
		v.AddError(field, message)
	}
	return v
}

// When applies conditional validation.
func (v *Validator) When(condition bool, fn func(*Validator)) *Validator {
	if condition {
		fn(v)
	}
	return v
}

// ============================================
// Standalone Validation Functions
// ============================================

// ValidateEmail validates an email address.
func ValidateEmail(email string) bool {
	if email == "" {
		return false
	}
	
	// Use Go's mail parser for more accurate validation
	_, err := mail.ParseAddress(email)
	if err != nil {
		return false
	}
	
	// Additional regex check
	return EmailPattern.MatchString(email)
}

// ValidateURL validates a URL.
func ValidateURL(rawURL string) bool {
	if rawURL == "" {
		return false
	}
	
	u, err := url.Parse(rawURL)
	if err != nil {
		return false
	}
	
	// Must have scheme and host
	if u.Scheme == "" || u.Host == "" {
		return false
	}
	
	// Only allow safe schemes
	allowedSchemes := map[string]bool{
		"http":  true,
		"https": true,
		"ftp":   true,
		"ftps":  true,
	}
	
	return allowedSchemes[strings.ToLower(u.Scheme)]
}

// ValidateUUID validates a UUID string.
func ValidateUUID(uuid string) bool {
	if uuid == "" {
		return false
	}
	return UUIDPattern.MatchString(uuid)
}

// ValidateAlphanumeric validates alphanumeric string.
func ValidateAlphanumeric(s string) bool {
	if s == "" {
		return false
	}
	return AlphanumericPattern.MatchString(s)
}

// ValidateUsername validates a username.
func ValidateUsername(username string) bool {
	if username == "" {
		return false
	}
	return UsernamePattern.MatchString(username)
}

// ValidatePhone validates a phone number.
func ValidatePhone(phone string) bool {
	if phone == "" {
		return false
	}
	return PhonePattern.MatchString(phone)
}

// ValidateSlug validates a URL slug.
func ValidateSlug(slug string) bool {
	if slug == "" {
		return false
	}
	return SlugPattern.MatchString(slug)
}

// ValidateIP validates an IPv4 address.
func ValidateIP(ip string) bool {
	if ip == "" {
		return false
	}
	return IPPattern.MatchString(ip)
}

// ValidateIPv6 validates an IPv6 address.
func ValidateIPv6(ip string) bool {
	if ip == "" {
		return false
	}
	return IPv6Pattern.MatchString(ip)
}

// ValidateInteger validates an integer with optional bounds.
func ValidateInteger(value interface{}, minVal, maxVal *int64) bool {
	var intVal int64
	
	switch v := value.(type) {
	case int:
		intVal = int64(v)
	case int8:
		intVal = int64(v)
	case int16:
		intVal = int64(v)
	case int32:
		intVal = int64(v)
	case int64:
		intVal = v
	case uint:
		intVal = int64(v)
	case uint8:
		intVal = int64(v)
	case uint16:
		intVal = int64(v)
	case uint32:
		intVal = int64(v)
	case uint64:
		intVal = int64(v)
	case float64:
		if v != float64(int64(v)) {
			return false
		}
		intVal = int64(v)
	case json.Number:
		var err error
		intVal, err = v.Int64()
		if err != nil {
			return false
		}
	default:
		return false
	}
	
	if minVal != nil && intVal < *minVal {
		return false
	}
	
	if maxVal != nil && intVal > *maxVal {
		return false
	}
	
	return true
}

// ValidateFloat validates a float with optional bounds.
func ValidateFloat(value interface{}, minVal, maxVal *float64) bool {
	var floatVal float64
	
	switch v := value.(type) {
	case int:
		floatVal = float64(v)
	case int8:
		floatVal = float64(v)
	case int16:
		floatVal = float64(v)
	case int32:
		floatVal = float64(v)
	case int64:
		floatVal = float64(v)
	case uint:
		floatVal = float64(v)
	case uint8:
		floatVal = float64(v)
	case uint16:
		floatVal = float64(v)
	case uint32:
		floatVal = float64(v)
	case uint64:
		floatVal = float64(v)
	case float32:
		floatVal = float64(v)
	case float64:
		floatVal = v
	case json.Number:
		var err error
		floatVal, err = v.Float64()
		if err != nil {
			return false
		}
	default:
		return false
	}
	
	if minVal != nil && floatVal < *minVal {
		return false
	}
	
	if maxVal != nil && floatVal > *maxVal {
		return false
	}
	
	return true
}

// ValidateString validates a string with length constraints.
func ValidateString(value interface{}, minLength, maxLength *int, pattern *regexp.Regexp) bool {
	str, ok := value.(string)
	if !ok {
		return false
	}
	
	if minLength != nil && len(str) < *minLength {
		return false
	}
	
	if maxLength != nil && len(str) > *maxLength {
		return false
	}
	
	if pattern != nil && !pattern.MatchString(str) {
		return false
	}
	
	return true
}

// ValidateDateTime validates a datetime string.
func ValidateDateTime(value string, format string) bool {
	if value == "" {
		return false
	}
	
	if format == "" {
		// Try RFC3339 (ISO 8601)
		format = time.RFC3339
	}
	
	_, err := time.Parse(format, value)
	return err == nil
}

// ValidateEnum validates value is in allowed list.
func ValidateEnum(value interface{}, allowed []interface{}) bool {
	for _, a := range allowed {
		if value == a {
			return true
		}
	}
	return false
}

// ValidateList validates a list with length constraints.
func ValidateList(value interface{}, minLength, maxLength *int) bool {
	list, ok := value.([]interface{})
	if !ok {
		return false
	}
	
	if minLength != nil && len(list) < *minLength {
		return false
	}
	
	if maxLength != nil && len(list) > *maxLength {
		return false
	}
	
	return true
}

// ValidateRequired validates a value is not empty.
func ValidateRequired(value interface{}) bool {
	if value == nil {
		return false
	}
	
	switch v := value.(type) {
	case string:
		return strings.TrimSpace(v) != ""
	case []interface{}:
		return len(v) > 0
	case map[string]interface{}:
		return len(v) > 0
	default:
		return true
	}
}

// ValidateNoControlChars validates string has no control characters.
func ValidateNoControlChars(s string) bool {
	for _, r := range s {
		if unicode.IsControl(r) && r != '\n' && r != '\r' && r != '\t' {
			return false
		}
	}
	return true
}
