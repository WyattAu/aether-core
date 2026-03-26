/**
 * Sanitization Functions for Input Cleaning.
 *
 * Provides functions to clean and normalize user input, preventing injection
 * attacks and normalizing data for safe storage and display.
 *
 * @module aether/validation/sanitize
 */

/**
 * Sanitize a string by removing null bytes, trimming whitespace,
 * and optionally truncating.
 *
 * @param value     - The string to sanitize.
 * @param maxLength - Optional maximum length; the string is truncated if exceeded.
 * @returns The sanitized string.
 * @throws Error If `value` is not a string.
 */
export function sanitizeString(value: string, maxLength?: number): string {
  if (typeof value !== 'string') {
    throw new Error('Expected string');
  }

  // Remove null bytes
  let sanitized = value.replace(/\x00/g, '');

  // Strip whitespace
  sanitized = sanitized.trim();

  // Truncate if needed
  if (maxLength !== undefined && sanitized.length > maxLength) {
    sanitized = sanitized.substring(0, maxLength);
  }

  return sanitized;
}

/**
 * Escape HTML entities to prevent XSS attacks.
 *
 * Replaces `&`, `<`, `>`, `"`, `'`, and `/` with their HTML entity equivalents.
 *
 * @param value - The string to escape.
 * @returns The HTML-safe string.
 *
 * @example
 * ```typescript
 * const safe = sanitizeHTML('<script>alert("xss")</script>');
 * // => '&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;'
 * ```
 */
export function sanitizeHTML(value: string): string {
  const htmlEntities: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#x27;',
    '/': '&#x2F;',
  };

  return value.replace(/[&<>"'/]/g, (char) => htmlEntities[char] ?? char);
}

/**
 * Basic SQL injection prevention by removing dangerous SQL patterns.
 *
 * **WARNING**: This is a defense-in-depth measure only. Always use
 * parameterized queries for database operations.
 *
 * @param value - The string to sanitize.
 * @returns The string with dangerous SQL patterns removed.
 */
export function sanitizeSQL(value: string): string {
  const dangerousPatterns = [
    /;?\s*DROP\s+/gi,
    /;?\s*DELETE\s+/gi,
    /;?\s*UPDATE\s+/gi,
    /;?\s*INSERT\s+/gi,
    /;?\s*EXEC\s*\(/gi,
    /;?\s*EXECUTE\s*\(/gi,
    /--/g,
    /\/\*/g,
    /\*\//g,
    /xp_cmdshell/gi,
  ];

  let sanitized = value;
  for (const pattern of dangerousPatterns) {
    sanitized = sanitized.replace(pattern, '');
  }

  return sanitized;
}

/**
 * Sanitize and validate a URL, ensuring only allowed schemes are used.
 *
 * @param value          - The URL string.
 * @param allowedSchemes - Allowed URL schemes (default: `['http', 'https']`).
 * @returns The sanitized URL string.
 * @throws Error If the URL is invalid or uses a disallowed scheme.
 */
export function sanitizeURL(
  value: string,
  allowedSchemes?: string[]
): string {
  const schemes = allowedSchemes ?? ['http', 'https'];

  try {
    const parsed = new URL(value);

    if (!schemes.includes(parsed.protocol.replace(':', ''))) {
      throw new Error(`URL scheme '${parsed.protocol}' is not allowed`);
    }

    return parsed.toString();
  } catch (error) {
    throw new Error(
      error instanceof Error ? error.message : 'Invalid URL'
    );
  }
}

/**
 * Recursively sanitize JSON-like data structures.
 *
 * Strings are sanitized via {@link sanitizeString}; arrays and objects
 * are traversed recursively. All other types pass through unchanged.
 *
 * @typeParam T - The type of the value.
 * @param value - The value to sanitize.
 * @returns The sanitized value with the same type.
 *
 * @example
 * ```typescript
 * const clean = sanitizeJSON({ name: '<script>', items: [1, 'abc'] });
 * // => { name: 'script', items: [1, 'abc'] }
 * ```
 */
export function sanitizeJSON<T>(value: T): T {
  if (typeof value === 'string') {
    return sanitizeString(value) as T;
  } else if (Array.isArray(value)) {
    return value.map((item) => sanitizeJSON(item)) as T;
  } else if (value !== null && typeof value === 'object') {
    const result: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(value as Record<string, unknown>)) {
      result[sanitizeString(key)] = sanitizeJSON(val);
    }
    return result as T;
  }
  return value;
}

/**
 * Sanitize a filename by removing path separators, null bytes, and leading dots.
 *
 * @param filename - The filename to sanitize.
 * @returns A safe filename string.
 */
export function sanitizeFilename(filename: string): string {
  // Remove path separators
  let sanitized = filename.replace(/[/\\]/g, '');

  // Remove null bytes
  sanitized = sanitized.replace(/\x00/g, '');

  // Remove leading dots (hidden files)
  while (sanitized.startsWith('.')) {
    sanitized = sanitized.substring(1);
  }

  return sanitized;
}

/**
 * Sanitize a file path by removing null bytes and directory traversal attempts.
 *
 * @param path - The file path to sanitize.
 * @returns A sanitized path string.
 */
export function sanitizePath(path: string): string {
  // Remove null bytes
  let sanitized = path.replace(/\x00/g, '');

  // Remove directory traversal attempts
  sanitized = sanitized.replace(/\.\.\//g, '');
  sanitized = sanitized.replace(/\.\.\\/g, '');

  return sanitized;
}

/**
 * Remove control characters from a string, preserving newlines, tabs, and carriage returns.
 *
 * @param s - The string to clean.
 * @returns The string with control characters removed.
 */
export function removeControlChars(s: string): string {
  // eslint-disable-next-line no-control-regex
  return s.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, '');
}

/**
 * Trim leading/trailing whitespace and collapse internal whitespace runs to a single space.
 *
 * @param s - The string to normalize.
 * @returns The whitespace-normalized string.
 */
export function trimAndNormalizeWhitespace(s: string): string {
  return s.trim().replace(/\s+/g, ' ');
}

/**
 * Normalize a phone number by stripping all non-digit characters
 * (except a leading `+`).
 *
 * @param phone - The phone number string.
 * @returns The normalized phone number (digits only, optional leading `+`).
 */
export function sanitizePhone(phone: string): string {
  const result: string[] = [];

  for (const char of phone) {
    if (char >= '0' && char <= '9') {
      result.push(char);
    } else if (char === '+' && result.length === 0) {
      result.push(char);
    }
  }

  return result.join('');
}

/**
 * Remove all non-alphanumeric characters from a string.
 *
 * @param s - The string to filter.
 * @returns The alphanumeric-only string.
 */
export function sanitizeAlphanumeric(s: string): string {
  return s.replace(/[^a-zA-Z0-9]/g, '');
}

/**
 * Create a URL-safe slug from a string.
 *
 * Converts to lowercase, replaces spaces/underscores with hyphens,
 * strips non-alphanumeric characters, and removes consecutive hyphens.
 *
 * @param s - The input string.
 * @returns A URL-safe slug.
 *
 * @example
 * ```typescript
 * sanitizeSlug('Hello World! How are you?');
 * // => 'hello-world-how-are-you'
 * ```
 */
export function sanitizeSlug(s: string): string {
  // Convert to lowercase
  let slug = s.toLowerCase();

  // Replace spaces and underscores with hyphens
  slug = slug.replace(/[\s_]+/g, '-');

  // Keep only alphanumeric and hyphens
  slug = slug.replace(/[^a-z0-9-]/g, '');

  // Remove consecutive hyphens
  slug = slug.replace(/-+/g, '-');

  // Trim hyphens from ends
  slug = slug.replace(/^-+|-+$/g, '');

  return slug;
}

/**
 * Redact sensitive data for logging by replacing middle characters with asterisks.
 *
 * @param value     - The sensitive string.
 * @param showChars - Number of characters to reveal at each end (default: 4).
 * @returns The redacted string, or fully masked if too short.
 *
 * @example
 * ```typescript
 * redactSensitive('sk-1234567890abcdef', 4);
 * // => 'sk-1**************cdef'
 * ```
 */
export function redactSensitive(value: string, showChars = 4): string {
  if (value.length <= showChars * 2) {
    return '*'.repeat(value.length);
  }

  const start = value.substring(0, showChars);
  const end = value.substring(value.length - showChars);
  const middle = '*'.repeat(value.length - showChars * 2);

  return start + middle + end;
}

/**
 * Escape special regex characters in a string for use in a `RegExp` constructor.
 *
 * @param s - The string containing literal text.
 * @returns The escaped string safe for regex use.
 */
export function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Escape a string for safe use in shell commands (single-quote wrapping).
 *
 * @param s - The string to escape.
 * @returns The shell-escaped string wrapped in single quotes.
 */
export function escapeShell(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/**
 * Normalize all line endings in a string to `\n`.
 *
 * Converts `\r\n` (Windows) and `\r` (old Mac) to `\n` (Unix).
 *
 * @param s - The string to normalize.
 * @returns The string with normalized line endings.
 */
export function normalizeLineEndings(s: string): string {
  return s.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

/**
 * Strip all HTML tags from a string.
 *
 * @param s - The string containing HTML.
 * @returns The plain text with tags removed.
 */
export function stripHTML(s: string): string {
  return s.replace(/<[^>]*>/g, '');
}

/**
 * Truncate a string with an ellipsis suffix if it exceeds the maximum length.
 *
 * @param s         - The string to truncate.
 * @param maxLength - Maximum allowed length (including ellipsis).
 * @param ellipsis  - The suffix to append when truncated (default: `'...'`).
 * @returns The original string, or the truncated version with ellipsis.
 */
export function truncate(s: string, maxLength: number, ellipsis = '...'): string {
  if (s.length <= maxLength) {
    return s;
  }

  const truncateAt = maxLength - ellipsis.length;
  return s.substring(0, truncateAt) + ellipsis;
}
