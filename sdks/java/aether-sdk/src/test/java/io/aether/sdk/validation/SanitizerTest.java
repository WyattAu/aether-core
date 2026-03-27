package io.aether.sdk.validation;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

class SanitizerTest {

    @Test
    @DisplayName("sanitizeString removes null bytes and trims")
    void testSanitizeString() {
        assertEquals("hello", Sanitizer.sanitizeString("  hello  "));
        assertEquals("hello", Sanitizer.sanitizeString("hel\0lo"));
        assertEquals("hello", Sanitizer.sanitizeString("  hel\0lo  "));
    }

    @Test
    @DisplayName("sanitizeString null returns null")
    void testSanitizeStringNull() {
        assertNull(Sanitizer.sanitizeString(null));
    }

    @Test
    @DisplayName("sanitizeString with max length")
    void testSanitizeStringMaxLength() {
        assertEquals("hel", Sanitizer.sanitizeString("hello", 3));
        assertEquals("hello", Sanitizer.sanitizeString("hello", 10));
    }

    @Test
    @DisplayName("sanitizeString empty returns empty")
    void testSanitizeStringEmpty() {
        assertEquals("", Sanitizer.sanitizeString(""));
    }

    @Test
    @DisplayName("sanitizeHTML escapes entities")
    void testSanitizeHTML() {
        assertEquals("&lt;script&gt;", Sanitizer.sanitizeHTML("<script>"));
        assertEquals("a&amp;b", Sanitizer.sanitizeHTML("a&b"));
        assertEquals("&quot;hi&quot;", Sanitizer.sanitizeHTML("\"hi\""));
        assertEquals("&#x27;hi&#x27;", Sanitizer.sanitizeHTML("'hi'"));
        assertEquals("path&#x2F;to", Sanitizer.sanitizeHTML("path/to"));
        assertNull(Sanitizer.sanitizeHTML(null));
    }

    @Test
    @DisplayName("sanitizeSQL removes dangerous patterns")
    void testSanitizeSQL() {
        String result = Sanitizer.sanitizeSQL("SELECT * FROM users; DROP TABLE users");
        assertFalse(result.contains("DROP"));
        assertNull(Sanitizer.sanitizeSQL(null));
    }

    @Test
    @DisplayName("sanitizeSQL removes comments")
    void testSanitizeSQLComments() {
        String result = Sanitizer.sanitizeSQL("SELECT -- comment\n1");
        assertFalse(result.contains("--"));
        assertFalse(result.contains("/*"));
    }

    @Test
    @DisplayName("sanitizeFilename removes path separators and dots")
    void testSanitizeFilename() {
        assertEquals("file.txt", Sanitizer.sanitizeFilename("path/to/file.txt"));
        assertEquals("file.txt", Sanitizer.sanitizeFilename("..\\file.txt"));
        assertEquals("file.txt", Sanitizer.sanitizeFilename("...file.txt"));
        assertNull(Sanitizer.sanitizeFilename(null));
    }

    @Test
    @DisplayName("sanitizePath removes directory traversal")
    void testSanitizePath() {
        assertEquals("a/b/c", Sanitizer.sanitizePath("a/../b/./c"));
        assertNull(Sanitizer.sanitizePath(null));
    }

    @Test
    @DisplayName("sanitizeSlug creates URL-safe slug")
    void testSanitizeSlug() {
        assertEquals("hello-world", Sanitizer.sanitizeSlug("Hello World"));
        assertEquals("foo-bar", Sanitizer.sanitizeSlug("foo_bar"));
        assertEquals("test123", Sanitizer.sanitizeSlug("Test 123!!"));
        assertEquals("a-b", Sanitizer.sanitizeSlug("A--B"));
        assertNull(Sanitizer.sanitizeSlug(null));
    }

    @Test
    @DisplayName("sanitizePhone keeps digits and leading plus")
    void testSanitizePhone() {
        assertEquals("+1234567890", Sanitizer.sanitizePhone("+1 (234) 567-890"));
        assertEquals("1234567890", Sanitizer.sanitizePhone("123-456-7890"));
        assertNull(Sanitizer.sanitizePhone(null));
    }

    @Test
    @DisplayName("removeControlChars keeps newlines and tabs")
    void testRemoveControlChars() {
        assertEquals("a\nb\tc", Sanitizer.removeControlChars("a\nb\tc"));
        assertEquals("abc", Sanitizer.removeControlChars("a\u0000b\u0001c"));
        assertNull(Sanitizer.removeControlChars(null));
    }

    @Test
    @DisplayName("trimAndNormalizeWhitespace")
    void testTrimAndNormalizeWhitespace() {
        assertEquals("a b c", Sanitizer.trimAndNormalizeWhitespace("  a   b   c  "));
        assertNull(Sanitizer.trimAndNormalizeWhitespace(null));
    }

    @Test
    @DisplayName("redactSensitive shows partial")
    void testRedactSensitive() {
        assertEquals("1234********5678", Sanitizer.redactSensitive("1234567890123456"));
        assertEquals("1234567890123456", Sanitizer.redactSensitive("1234"));
    }

    @Test
    @DisplayName("redactSensitive with custom show chars")
    void testRedactSensitiveCustom() {
        assertEquals("12****89", Sanitizer.redactSensitive("12345678", 2));
    }

    @Test
    @DisplayName("redactSensitive null returns null")
    void testRedactSensitiveNull() {
        assertNull(Sanitizer.redactSensitive(null));
    }

    @Test
    @DisplayName("truncate with ellipsis")
    void testTruncate() {
        assertEquals("hel...", Sanitizer.truncate("hello world", 6));
        assertEquals("hello", Sanitizer.truncate("hello", 10));
    }

    @Test
    @DisplayName("truncate with custom ellipsis")
    void testTruncateCustomEllipsis() {
        assertEquals("he..", Sanitizer.truncate("hello", 4, ".."));
    }

    @Test
    @DisplayName("truncate null returns null")
    void testTruncateNull() {
        assertNull(Sanitizer.truncate(null, 10));
    }
}
