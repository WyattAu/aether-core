/**
 * Tests for Sanitization Functions
 */

import {
  sanitizeString,
  sanitizeHTML,
  sanitizeSQL,
  sanitizeURL,
  sanitizeJSON,
  sanitizeFilename,
  sanitizePath,
  removeControlChars,
  trimAndNormalizeWhitespace,
  sanitizePhone,
  sanitizeAlphanumeric,
  sanitizeSlug,
  redactSensitive,
  escapeRegex,
  escapeShell,
  normalizeLineEndings,
  stripHTML,
  truncate,
} from '../../src/validation/sanitize';

describe('sanitizeString', () => {
  test('removes null bytes', () => {
    const result = sanitizeString('hello\x00world');
    expect(result).toBe('helloworld');
  });

  test('trims whitespace', () => {
    const result = sanitizeString('  hello world  ');
    expect(result).toBe('hello world');
  });

  test('truncates to max length', () => {
    const result = sanitizeString('hello world', 5);
    expect(result).toBe('hello');
  });

  test('throws for non-string input', () => {
    expect(() => sanitizeString(123 as any)).toThrow('Expected string');
  });

  test('handles empty string', () => {
    const result = sanitizeString('');
    expect(result).toBe('');
  });
});

describe('sanitizeHTML', () => {
  test('escapes HTML special characters', () => {
    expect(sanitizeHTML('<script>alert("xss")</script>')).toBe(
      '&lt;script&gt;alert(&quot;xss&quot;)&lt;&#x2F;script&gt;'
    );
  });

  test('escapes ampersand', () => {
    expect(sanitizeHTML('Tom & Jerry')).toBe('Tom &amp; Jerry');
  });

  test('escapes quotes', () => {
    expect(sanitizeHTML('say "hello"')).toBe('say &quot;hello&quot;');
    expect(sanitizeHTML("it's fine")).toBe('it&#x27;s fine');
  });

  test('handles plain text', () => {
    expect(sanitizeHTML('plain text')).toBe('plain text');
  });
});

describe('sanitizeSQL', () => {
  test('removes DROP statements', () => {
    const result = sanitizeSQL('value; DROP TABLE users;');
    expect(result).not.toContain('DROP');
  });

  test('removes DELETE statements', () => {
    const result = sanitizeSQL('value; DELETE FROM users;');
    expect(result).not.toContain('DELETE');
  });

  test('removes UPDATE statements', () => {
    const result = sanitizeSQL('value; UPDATE users SET admin=1;');
    expect(result).not.toContain('UPDATE');
  });

  test('removes INSERT statements', () => {
    const result = sanitizeSQL("value; INSERT INTO users VALUES (1, 'hacker');");
    expect(result).not.toContain('INSERT');
  });

  test('removes SQL comments', () => {
    const result = sanitizeSQL('value -- comment');
    expect(result).not.toContain('--');
  });

  test('removes block comments', () => {
    const result = sanitizeSQL('value /* comment */');
    expect(result).not.toContain('/*');
    expect(result).not.toContain('*/');
  });

  test('handles clean input', () => {
    const result = sanitizeSQL('normal value');
    expect(result).toBe('normal value');
  });
});

describe('sanitizeURL', () => {
  test('validates and returns URL', () => {
    const result = sanitizeURL('https://example.com/path');
    expect(result).toBe('https://example.com/path');
  });

  test('rejects disallowed schemes', () => {
    expect(() => sanitizeURL('javascript:alert(1)')).toThrow();
  });

  test('respects allowed schemes', () => {
    expect(() => sanitizeURL('ftp://example.com', ['http', 'https'])).toThrow();
    expect(sanitizeURL('ftp://example.com', ['ftp'])).toBe('ftp://example.com/');
  });

  test('rejects invalid URLs', () => {
    expect(() => sanitizeURL('not-a-url')).toThrow();
  });
});

describe('sanitizeJSON', () => {
  test('sanitizes string values', () => {
    const result = sanitizeJSON({ name: '  hello\x00world  ' });
    // null bytes are removed, then string is trimmed
    expect(result.name).toBe('helloworld');
  });

  test('sanitizes array values', () => {
    const result = sanitizeJSON(['  a  ', '  b  ']);
    expect(result).toEqual(['a', 'b']);
  });

  test('sanitizes nested objects', () => {
    const result = sanitizeJSON({ user: { name: '  john  ' } });
    expect(result.user.name).toBe('john');
  });

  test('sanitizes object keys', () => {
    const result = sanitizeJSON({ '  key  ': 'value' });
    expect(result).toHaveProperty('key');
  });

  test('preserves non-string values', () => {
    const result = sanitizeJSON({ num: 42, bool: true, nil: null });
    expect(result.num).toBe(42);
    expect(result.bool).toBe(true);
    expect(result.nil).toBe(null);
  });
});

describe('sanitizeFilename', () => {
  test('removes path separators', () => {
    expect(sanitizeFilename('file/name.txt')).toBe('filename.txt');
    expect(sanitizeFilename('file\\name.txt')).toBe('filename.txt');
  });

  test('removes null bytes', () => {
    expect(sanitizeFilename('file\x00name.txt')).toBe('filename.txt');
  });

  test('removes leading dots', () => {
    expect(sanitizeFilename('..hidden')).toBe('hidden');
    expect(sanitizeFilename('.env')).toBe('env');
  });

  test('handles normal filename', () => {
    expect(sanitizeFilename('document.pdf')).toBe('document.pdf');
  });
});

describe('sanitizePath', () => {
  test('removes null bytes', () => {
    expect(sanitizePath('/path/to\x00/file')).toBe('/path/to/file');
  });

  test('removes directory traversal attempts', () => {
    expect(sanitizePath('../../../etc/passwd')).toBe('etc/passwd');
    expect(sanitizePath('..\\..\\windows\\system')).toBe('windows\\system');
  });

  test('handles normal paths', () => {
    expect(sanitizePath('/home/user/documents')).toBe('/home/user/documents');
  });
});

describe('removeControlChars', () => {
  test('removes control characters', () => {
    expect(removeControlChars('hello\x00world')).toBe('helloworld');
    expect(removeControlChars('hello\x1bworld')).toBe('helloworld');
  });

  test('preserves newlines and tabs', () => {
    expect(removeControlChars('hello\nworld')).toBe('hello\nworld');
    expect(removeControlChars('hello\tworld')).toBe('hello\tworld');
    expect(removeControlChars('hello\rworld')).toBe('hello\rworld');
  });

  test('handles string without control chars', () => {
    expect(removeControlChars('normal string')).toBe('normal string');
  });
});

describe('trimAndNormalizeWhitespace', () => {
  test('trims leading and trailing whitespace', () => {
    expect(trimAndNormalizeWhitespace('  hello  ')).toBe('hello');
  });

  test('normalizes multiple spaces', () => {
    expect(trimAndNormalizeWhitespace('hello    world')).toBe('hello world');
  });

  test('normalizes various whitespace characters', () => {
    expect(trimAndNormalizeWhitespace('hello\t\nworld')).toBe('hello world');
  });
});

describe('sanitizePhone', () => {
  test('keeps only digits and leading plus', () => {
    expect(sanitizePhone('+1 (555) 123-4567')).toBe('+15551234567');
  });

  test('removes non-numeric characters', () => {
    expect(sanitizePhone('555-123-4567')).toBe('5551234567');
  });

  test('preserves leading plus', () => {
    expect(sanitizePhone('+14155552671')).toBe('+14155552671');
  });

  test('removes plus not at start', () => {
    expect(sanitizePhone('555+123')).toBe('555123');
  });
});

describe('sanitizeAlphanumeric', () => {
  test('keeps only letters and numbers', () => {
    expect(sanitizeAlphanumeric('Hello, World! 123')).toBe('HelloWorld123');
  });

  test('removes special characters', () => {
    expect(sanitizeAlphanumeric('test@example.com')).toBe('testexamplecom');
  });

  test('handles empty result', () => {
    expect(sanitizeAlphanumeric('!@#$%')).toBe('');
  });
});

describe('sanitizeSlug', () => {
  test('converts to lowercase', () => {
    expect(sanitizeSlug('Hello World')).toBe('hello-world');
  });

  test('replaces spaces with hyphens', () => {
    expect(sanitizeSlug('my blog post')).toBe('my-blog-post');
  });

  test('replaces underscores with hyphens', () => {
    expect(sanitizeSlug('my_blog_post')).toBe('my-blog-post');
  });

  test('removes special characters', () => {
    expect(sanitizeSlug('Hello, World!')).toBe('hello-world');
  });

  test('removes consecutive hyphens', () => {
    expect(sanitizeSlug('hello---world')).toBe('hello-world');
  });

  test('trims hyphens from ends', () => {
    expect(sanitizeSlug('-hello-world-')).toBe('hello-world');
  });
});

describe('redactSensitive', () => {
  test('redacts middle of string', () => {
    // Implementation shows first 4 and last 4 chars with asterisks in between
    expect(redactSensitive('1234567890')).toBe('1234**7890');
  });

  test('fully redacts short strings', () => {
    expect(redactSensitive('abc')).toBe('***');
    expect(redactSensitive('abcd')).toBe('****');
  });

  test('respects showChars parameter', () => {
    // With showChars=2, shows first 2 and last 2 chars
    expect(redactSensitive('1234567890', 2)).toBe('12******90');
  });

  test('handles empty string', () => {
    expect(redactSensitive('')).toBe('');
  });
});

describe('escapeRegex', () => {
  test('escapes regex special characters', () => {
    expect(escapeRegex('.')).toBe('\\.');
    expect(escapeRegex('*')).toBe('\\*');
    expect(escapeRegex('+')).toBe('\\+');
    expect(escapeRegex('?')).toBe('\\?');
  });

  test('escapes brackets and parentheses', () => {
    expect(escapeRegex('[test]')).toBe('\\[test\\]');
    expect(escapeRegex('(test)')).toBe('\\(test\\)');
  });

  test('escapes other special chars', () => {
    expect(escapeRegex('^$|\\')).toBe('\\^\\$\\|\\\\');
  });

  test('handles plain text', () => {
    expect(escapeRegex('hello')).toBe('hello');
  });
});

describe('escapeShell', () => {
  test('wraps in single quotes', () => {
    expect(escapeShell('hello')).toBe("'hello'");
  });

  test('escapes single quotes', () => {
    expect(escapeShell("it's fine")).toBe("'it'\\''s fine'");
  });

  test('handles empty string', () => {
    expect(escapeShell('')).toBe("''");
  });
});

describe('normalizeLineEndings', () => {
  test('converts CRLF to LF', () => {
    expect(normalizeLineEndings('line1\r\nline2')).toBe('line1\nline2');
  });

  test('converts CR to LF', () => {
    expect(normalizeLineEndings('line1\rline2')).toBe('line1\nline2');
  });

  test('handles mixed line endings', () => {
    expect(normalizeLineEndings('line1\r\nline2\rline3')).toBe('line1\nline2\nline3');
  });

  test('handles LF only', () => {
    expect(normalizeLineEndings('line1\nline2')).toBe('line1\nline2');
  });
});

describe('stripHTML', () => {
  test('removes HTML tags', () => {
    expect(stripHTML('<p>Hello <b>World</b></p>')).toBe('Hello World');
  });

  test('removes self-closing tags', () => {
    expect(stripHTML('Line 1<br/>Line 2')).toBe('Line 1Line 2');
  });

  test('handles nested tags', () => {
    expect(stripHTML('<div><span><a href="#">Link</a></span></div>')).toBe('Link');
  });

  test('handles plain text', () => {
    expect(stripHTML('plain text')).toBe('plain text');
  });
});

describe('truncate', () => {
  test('returns string if shorter than max', () => {
    expect(truncate('hello', 10)).toBe('hello');
  });

  test('truncates with ellipsis', () => {
    expect(truncate('hello world', 8)).toBe('hello...');
  });

  test('respects custom ellipsis', () => {
    expect(truncate('hello world', 10, '…')).toBe('hello wor…');
  });

  test('handles exact length', () => {
    expect(truncate('hello', 5)).toBe('hello');
  });

  test('handles empty string', () => {
    expect(truncate('', 10)).toBe('');
  });
});
