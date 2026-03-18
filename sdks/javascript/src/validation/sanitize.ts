/**
 * Sanitization functions for input cleaning.
 * @module aether/validation/sanitize
 */

/**
 * Sanitize a string by removing dangerous characters.
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
 * Escape HTML entities in a string.
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
 * Basic SQL injection prevention.
 * WARNING: Always use parameterized queries instead!
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
 * Sanitize and validate a URL.
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
 * Recursively sanitize JSON-like data.
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
 * Sanitize a filename.
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
 * Sanitize a file path.
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
 * Remove control characters except newlines, tabs, and carriage returns.
 */
export function removeControlChars(s: string): string {
  // eslint-disable-next-line no-control-regex
  return s.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, '');
}

/**
 * Trim and normalize whitespace.
 */
export function trimAndNormalizeWhitespace(s: string): string {
  return s.trim().replace(/\s+/g, ' ');
}

/**
 * Normalize a phone number.
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
 * Keep only alphanumeric characters.
 */
export function sanitizeAlphanumeric(s: string): string {
  return s.replace(/[^a-zA-Z0-9]/g, '');
}

/**
 * Create a URL-safe slug from a string.
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
 * Redact sensitive data for logging.
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
 * Escape string for use in regex.
 */
export function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Escape string for use in shell commands.
 */
export function escapeShell(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/**
 * Normalize line endings to \n.
 */
export function normalizeLineEndings(s: string): string {
  return s.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

/**
 * Strip HTML tags from a string.
 */
export function stripHTML(s: string): string {
  return s.replace(/<[^>]*>/g, '');
}

/**
 * Truncate string with ellipsis.
 */
export function truncate(s: string, maxLength: number, ellipsis = '...'): string {
  if (s.length <= maxLength) {
    return s;
  }

  const truncateAt = maxLength - ellipsis.length;
  return s.substring(0, truncateAt) + ellipsis;
}
