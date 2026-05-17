package validation

import (
	"html"
	"net/url"
	"regexp"
	"strings"
)

// Sanitization functions for input cleaning.

// SanitizeString removes dangerous characters from a string.
func SanitizeString(value string, maxLength int) string {
	// Remove null bytes
	sanitized := strings.ReplaceAll(value, "\x00", "")
	
	// Strip whitespace
	sanitized = strings.TrimSpace(sanitized)
	
	// Truncate if needed
	if maxLength > 0 && len(sanitized) > maxLength {
		sanitized = sanitized[:maxLength]
	}
	
	return sanitized
}

// SanitizeHTML escapes HTML entities in a string.
func SanitizeHTML(value string) string {
	return html.EscapeString(value)
}

// SanitizeSQL removes common SQL injection patterns.
// WARNING: Always use parameterized queries instead!
func SanitizeSQL(value string) string {
	re := regexp.MustCompile(`(?i)/\*.*?\*/`)
	sanitized := re.ReplaceAllString(value, "")

	re = regexp.MustCompile(`--.*`)
	sanitized = re.ReplaceAllString(sanitized, "")

	dangerousPatterns := []string{
		`;\s*DROP\s+`,
		`;\s*DELETE\s+`,
		`;\s*UPDATE\s+`,
		`;\s*INSERT\s+`,
		`;\s*EXEC\s*\(`,
		`;\s*EXECUTE\s*\(`,
		`xp_cmdshell`,
	}

	for _, pattern := range dangerousPatterns {
		re = regexp.MustCompile("(?i)" + pattern)
		sanitized = re.ReplaceAllString(sanitized, " ")
	}

	return sanitized
}

// SanitizeURL sanitizes and validates a URL.
func SanitizeURL(value string) (string, error) {
	parsed, err := url.Parse(value)
	if err != nil {
		return "", err
	}
	
	// Only allow safe schemes
	safeSchemes := map[string]bool{
		"http":  true,
		"https": true,
		"ftp":   true,
		"ftps":  true,
	}
	
	if parsed.Scheme != "" && !safeSchemes[strings.ToLower(parsed.Scheme)] {
		return "", ErrInvalidScheme
	}
	
	// Reconstruct URL to normalize
	return parsed.String(), nil
}

// SanitizeJSON recursively sanitizes JSON-like data.
func SanitizeJSON(value interface{}) interface{} {
	switch v := value.(type) {
	case string:
		return SanitizeString(v, 0)
	case map[string]interface{}:
		result := make(map[string]interface{})
		for key, val := range v {
			result[SanitizeString(key, 0)] = SanitizeJSON(val)
		}
		return result
	case []interface{}:
		result := make([]interface{}, len(v))
		for i, item := range v {
			result[i] = SanitizeJSON(item)
		}
		return result
	default:
		return value
	}
}

// SanitizeFilename sanitizes a filename.
func SanitizeFilename(filename string) string {
	// Remove path separators
	filename = strings.ReplaceAll(filename, "/", "")
	filename = strings.ReplaceAll(filename, "\\", "")
	
	// Remove null bytes
	filename = strings.ReplaceAll(filename, "\x00", "")
	
	// Remove leading dots (hidden files)
	for strings.HasPrefix(filename, ".") {
		filename = strings.TrimPrefix(filename, ".")
	}
	
	return filename
}

// SanitizePath sanitizes a file path.
func SanitizePath(path string) string {
	path = strings.ReplaceAll(path, "\x00", "")
	path = strings.ReplaceAll(path, "\\", "/")
	path = strings.ReplaceAll(path, "../", "")
	path = strings.ReplaceAll(path, "..", "")
	return path
}

// RemoveControlChars removes control characters except newlines, tabs, and carriage returns.
func RemoveControlChars(s string) string {
	var result strings.Builder
	for _, r := range s {
		if r == '\n' || r == '\r' || r == '\t' || !isControlChar(r) {
			result.WriteRune(r)
		}
	}
	return result.String()
}

func isControlChar(r rune) bool {
	// Exclude common whitespace characters (tab, newline, carriage return)
	if r == '\t' || r == '\n' || r == '\r' {
		return false
	}
	return r < 32 || (r >= 127 && r <= 159)
}

// TrimAndNormalizeWhitespace trims and normalizes whitespace.
func TrimAndNormalizeWhitespace(s string) string {
	// Trim leading/trailing whitespace
	s = strings.TrimSpace(s)
	
	// Replace multiple spaces with single space
	re := regexp.MustCompile(`\s+`)
	return re.ReplaceAllString(s, " ")
}

// SanitizePhone normalizes a phone number.
func SanitizePhone(phone string) string {
	// Keep only digits and plus sign
	var result strings.Builder
	for _, r := range phone {
		if r >= '0' && r <= '9' {
			result.WriteRune(r)
		} else if r == '+' && result.Len() == 0 {
			result.WriteRune(r)
		}
	}
	return result.String()
}

// SanitizeAlphaNumeric keeps only alphanumeric characters.
func SanitizeAlphaNumeric(s string) string {
	var result strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') {
			result.WriteRune(r)
		}
	}
	return result.String()
}

// SanitizeSlug creates a URL-safe slug from a string.
func SanitizeSlug(s string) string {
	// Convert to lowercase
	s = strings.ToLower(s)
	
	// Replace spaces and underscores with hyphens
	s = strings.ReplaceAll(s, " ", "-")
	s = strings.ReplaceAll(s, "_", "-")
	
	// Keep only alphanumeric and hyphens
	var result strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '-' {
			result.WriteRune(r)
		}
	}
	
	// Remove consecutive hyphens
	slug := result.String()
	for strings.Contains(slug, "--") {
		slug = strings.ReplaceAll(slug, "--", "-")
	}
	
	// Trim hyphens from ends
	return strings.Trim(slug, "-")
}

// RedactSensitive redacts sensitive data for logging.
func RedactSensitive(value string, showChars int) string {
	if len(value) < showChars*2 {
		return strings.Repeat("*", len(value))
	}
	
	return value[:showChars] + strings.Repeat("*", len(value)-showChars*2) + value[len(value)-showChars:]
}
