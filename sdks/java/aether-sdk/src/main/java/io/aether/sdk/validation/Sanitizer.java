package io.aether.sdk.validation;

/**
 * Sanitization utilities for input cleaning.
 */
public final class Sanitizer {
    
    private Sanitizer() {} // Utility class
    
    /**
     * Sanitize a string by removing dangerous characters.
     */
    public static String sanitizeString(String value) {
        return sanitizeString(value, -1);
    }
    
    /**
     * Sanitize a string with max length.
     */
    public static String sanitizeString(String value, int maxLength) {
        if (value == null) {
            return null;
        }
        
        // Remove null bytes
        String sanitized = value.replace("\0", "");
        
        // Trim whitespace
        sanitized = sanitized.trim();
        
        // Truncate if needed
        if (maxLength > 0 && sanitized.length() > maxLength) {
            sanitized = sanitized.substring(0, maxLength);
        }
        
        return sanitized;
    }
    
    /**
     * Escape HTML entities in a string.
     */
    public static String sanitizeHTML(String value) {
        if (value == null) {
            return null;
        }
        
        return value
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&#x27;")
            .replace("/", "&#x2F;");
    }
    
    /**
     * Basic SQL injection prevention.
     * WARNING: Always use parameterized queries instead!
     */
    public static String sanitizeSQL(String value) {
        if (value == null) {
            return null;
        }
        
        return value
            .replaceAll("(?i);\\s*DROP\\s+", "")
            .replaceAll("(?i);\\s*DELETE\\s+", "")
            .replaceAll("(?i);\\s*UPDATE\\s+", "")
            .replaceAll("(?i);\\s*INSERT\\s+", "")
            .replaceAll("(?i);\\s*EXEC\\s*\\(", "")
            .replaceAll("(?i);\\s*EXECUTE\\s*\\(", "")
            .replace("--", "")
            .replace("/*", "")
            .replace("*/", "")
            .replaceAll("(?i)xp_cmdshell", "");
    }
    
    /**
     * Sanitize a filename.
     */
    public static String sanitizeFilename(String filename) {
        if (filename == null) {
            return null;
        }
        
        // Remove path separators
        String sanitized = filename.replace("/", "").replace("\\", "");
        
        // Remove null bytes
        sanitized = sanitized.replace("\0", "");
        
        // Remove leading dots
        while (sanitized.startsWith(".")) {
            sanitized = sanitized.substring(1);
        }
        
        return sanitized;
    }
    
    /**
     * Sanitize a file path.
     */
    public static String sanitizePath(String path) {
        if (path == null) {
            return null;
        }
        
        // Remove null bytes
        String sanitized = path.replace("\0", "");
        
        // Remove directory traversal attempts
        sanitized = sanitized.replace("../", "").replace("..\\", "");
        
        return sanitized;
    }
    
    /**
     * Create a URL-safe slug from a string.
     */
    public static String sanitizeSlug(String s) {
        if (s == null) {
            return null;
        }
        
        // Convert to lowercase
        String slug = s.toLowerCase();
        
        // Replace spaces and underscores with hyphens
        slug = slug.replace(" ", "-").replace("_", "-");
        
        // Keep only alphanumeric and hyphens
        slug = slug.replaceAll("[^a-z0-9-]", "");
        
        // Remove consecutive hyphens
        while (slug.contains("--")) {
            slug = slug.replace("--", "-");
        }
        
        // Trim hyphens from ends
        slug = slug.replaceAll("^-+|-$", "");
        
        return slug;
    }
    
    /**
     * Normalize a phone number.
     */
    public static String sanitizePhone(String phone) {
        if (phone == null) {
            return null;
        }
        
        StringBuilder result = new StringBuilder();
        for (char c : phone.toCharArray()) {
            if (c >= '0' && c <= '9') {
                result.append(c);
            } else if (c == '+' && result.length() == 0) {
                result.append(c);
            }
        }
        return result.toString();
    }
    
    /**
     * Remove control characters except newlines, tabs, and carriage returns.
     */
    public static String removeControlChars(String s) {
        if (s == null) {
            return null;
        }
        
        StringBuilder result = new StringBuilder();
        for (char c : s.toCharArray()) {
            if (c == '\n' || c == '\r' || c == '\t' || (c >= 32 && c != 127)) {
                result.append(c);
            }
        }
        return result.toString();
    }
    
    /**
     * Trim and normalize whitespace.
     */
    public static String trimAndNormalizeWhitespace(String s) {
        if (s == null) {
            return null;
        }
        return s.trim().replaceAll("\\s+", " ");
    }
    
    /**
     * Redact sensitive data for logging.
     */
    public static String redactSensitive(String value) {
        return redactSensitive(value, 4);
    }
    
    /**
     * Redact sensitive data for logging with custom visible chars.
     */
    public static String redactSensitive(String value, int showChars) {
        if (value == null) {
            return null;
        }
        
        if (value.length() <= showChars * 2) {
            return "*".repeat(value.length());
        }
        
        String start = value.substring(0, showChars);
        String end = value.substring(value.length() - showChars);
        String middle = "*".repeat(value.length() - showChars * 2);
        
        return start + middle + end;
    }
    
    /**
     * Truncate string with ellipsis.
     */
    public static String truncate(String s, int maxLength) {
        return truncate(s, maxLength, "...");
    }
    
    /**
     * Truncate string with custom ellipsis.
     */
    public static String truncate(String s, int maxLength, String ellipsis) {
        if (s == null) {
            return null;
        }
        
        if (s.length() <= maxLength) {
            return s;
        }
        
        int truncateAt = maxLength - ellipsis.length();
        return s.substring(0, truncateAt) + ellipsis;
    }
}
