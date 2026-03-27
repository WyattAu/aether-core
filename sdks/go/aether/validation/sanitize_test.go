package validation

import (
	"testing"
)

func TestSanitizeString_Basic(t *testing.T) {
	tests := []struct {
		name      string
		input     string
		maxLen    int
		expected  string
	}{
		{"normal", "hello world", 0, "hello world"},
		{"null bytes", "hello\x00world", 0, "helloworld"},
		{"whitespace", "  hello  ", 0, "hello"},
		{"truncate", "hello world", 5, "hello"},
		{"empty", "", 0, ""},
		{"maxLen zero", "data", 0, "data"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SanitizeString(tt.input, tt.maxLen)
			if result != tt.expected {
				t.Errorf("SanitizeString(%q, %d) = %q, want %q", tt.input, tt.maxLen, result, tt.expected)
			}
		})
	}
}

func TestSanitizeHTML(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"<script>alert('xss')</script>", "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"},
		{"plain text", "plain text"},
		{"a&b", "a&amp;b"},
		{"<b>bold</b>", "&lt;b&gt;bold&lt;/b&gt;"},
		{"", ""},
	}
	for _, tt := range tests {
		result := SanitizeHTML(tt.input)
		if result != tt.expected {
			t.Errorf("SanitizeHTML(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestSanitizeSQL(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"drop", "1; DROP TABLE users", "1 TABLE users"},
		{"comment", "admin'--", "admin'"},
		{"block comment", "1/* comment */", "1"},
		{"delete", "1; DELETE FROM users", "1 FROM users"},
		{"clean", "SELECT * FROM users", "SELECT * FROM users"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SanitizeSQL(tt.input)
			if result != tt.want {
				t.Errorf("SanitizeSQL(%q) = %q, want %q", tt.input, result, tt.want)
			}
		})
	}
}

func TestSanitizeURL(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    string
		wantErr bool
	}{
		{"http", "http://example.com", "http://example.com", false},
		{"https", "https://example.com/path", "https://example.com/path", false},
		{"javascript", "javascript:alert(1)", "", true},
		{"data", "data:text/html,<script>", "", true},
		{"ftp", "ftp://files.example.com", "ftp://files.example.com", false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := SanitizeURL(tt.input)
			if (err != nil) != tt.wantErr {
				t.Errorf("SanitizeURL(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
			}
			if !tt.wantErr && result != tt.want {
				t.Errorf("SanitizeURL(%q) = %q, want %q", tt.input, result, tt.want)
			}
		})
	}
}

func TestSanitizeJSON(t *testing.T) {
	input := map[string]interface{}{
		"key":  "value",
		"num":  42,
		"list": []interface{}{"a", "b"},
		"nested": map[string]interface{}{
			"inner": "data\x00",
		},
	}

	result := SanitizeJSON(input)
	m, ok := result.(map[string]interface{})
	if !ok {
		t.Fatal("expected map result")
	}
	if m["key"] != "value" {
		t.Errorf("expected 'value', got %v", m["key"])
	}
}

func TestSanitizeJSON_String(t *testing.T) {
	result := SanitizeJSON("hello\x00world")
	if result != "helloworld" {
		t.Errorf("expected 'helloworld', got %v", result)
	}
}

func TestSanitizeJSON_Slice(t *testing.T) {
	input := []interface{}{"a\x00b", "c"}
	result := SanitizeJSON(input)
	slice, ok := result.([]interface{})
	if !ok {
		t.Fatal("expected slice result")
	}
	if slice[0] != "ab" {
		t.Errorf("expected 'ab', got %v", slice[0])
	}
}

func TestSanitizeJSON_NonSpecial(t *testing.T) {
	result := SanitizeJSON(42)
	if result != 42 {
		t.Errorf("expected 42, got %v", result)
	}
}

func TestSanitizeFilename(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"file.txt", "file.txt"},
		{"path/to/file.txt", "pathtofile.txt"},
		{"..\\secret", "secret"},
		{".hidden", "hidden"},
		{"..dotdot", "dotdot"},
		{"file\x00name", "filename"},
	}
	for _, tt := range tests {
		result := SanitizeFilename(tt.input)
		if result != tt.expected {
			t.Errorf("SanitizeFilename(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestSanitizePath(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"normal/path", "normal/path"},
		{"../etc/passwd", "etc/passwd"},
		{"..\\windows\\system32", "windowssystem32"},
		{"path\x00inject", "pathinject"},
		{"", ""},
	}
	for _, tt := range tests {
		result := SanitizePath(tt.input)
		if result != tt.expected {
			t.Errorf("SanitizePath(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestRemoveControlChars(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"normal text", "normal text"},
		{"text\x00with\x01control", "textwithcontrol"},
		{"line\nbreak", "line\nbreak"},
		{"tab\there", "tab\there"},
		{"cr\rlf", "cr\rlf"},
		{"", ""},
	}
	for _, tt := range tests {
		result := RemoveControlChars(tt.input)
		if result != tt.expected {
			t.Errorf("RemoveControlChars(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestTrimAndNormalizeWhitespace(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"  hello   world  ", "hello world"},
		{"hello", "hello"},
		{"  spaces  ", "spaces"},
		{"tabs\there", "tabs here"},
		{"multiple   spaces", "multiple spaces"},
		{"", ""},
	}
	for _, tt := range tests {
		result := TrimAndNormalizeWhitespace(tt.input)
		if result != tt.expected {
			t.Errorf("TrimAndNormalizeWhitespace(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestSanitizePhone(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"+1-555-123-4567", "+15551234567"},
		{"(555) 123-4567", "5551234567"},
		{"555.123.4567", "5551234567"},
		{"+44 20 7946 0958", "+442079460958"},
		{"", ""},
		{"abc-def", ""},
		{"+123", "+123"},
	}
	for _, tt := range tests {
		result := SanitizePhone(tt.input)
		if result != tt.expected {
			t.Errorf("SanitizePhone(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestSanitizeAlphaNumeric(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"hello123", "hello123"},
		{"hello world!", "helloworld"},
		{"ABC123", "ABC123"},
		{"", ""},
		{"!@#$%", ""},
		{"user_name", "username"},
	}
	for _, tt := range tests {
		result := SanitizeAlphaNumeric(tt.input)
		if result != tt.expected {
			t.Errorf("SanitizeAlphaNumeric(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestSanitizeSlug(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"Hello World", "hello-world"},
		{"hello_world", "hello-world"},
		{"Hello  World!", "hello-world"},
		{"  leading dashes  ", "leading-dashes"},
		{"multiple---dashes", "multiple-dashes"},
		{"UPPERCASE", "uppercase"},
		{"123 numbers", "123-numbers"},
		{"", ""},
	}
	for _, tt := range tests {
		result := SanitizeSlug(tt.input)
		if result != tt.expected {
			t.Errorf("SanitizeSlug(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestRedactSensitive(t *testing.T) {
	tests := []struct {
		name      string
		value     string
		showChars int
		expected  string
	}{
		{"long", "secret-password-value", 2, "se*************ue"},
		{"short", "ab", 2, "**"},
		{"exact double", "abcd", 2, "abcd"},
		{"single char", "a", 1, "*"},
		{"empty", "", 2, ""},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := RedactSensitive(tt.value, tt.showChars)
			if result != tt.expected {
				t.Errorf("RedactSensitive(%q, %d) = %q, want %q", tt.value, tt.showChars, result, tt.expected)
			}
		})
	}
}

func TestIsControlChar(t *testing.T) {
	if !isControlChar(0x00) {
		t.Error("null byte should be control char")
	}
	if !isControlChar(0x1F) {
		t.Error("0x1F should be control char")
	}
	if isControlChar('\n') {
		t.Error("newline should not be control char for this function")
	}
	if isControlChar('a') {
		t.Error("'a' should not be control char")
	}
	if isControlChar(' ') {
		t.Error("space should not be control char")
	}
}
