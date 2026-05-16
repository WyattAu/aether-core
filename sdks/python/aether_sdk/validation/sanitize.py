"""
Sanitization Functions for Input Cleaning.

Provides functions to clean and normalize user input, preventing injection
attacks and normalizing data for safe storage and display.
"""

from __future__ import annotations

import html
import re
import urllib.parse
from typing import Any, List, Optional, Set


def sanitize_string(value: str, max_length: int = 10000) -> str:
    """Trim whitespace, remove null bytes, collapse multiple spaces, truncate.

    Args:
        value: String to sanitize.
        max_length: Maximum allowed length (default 10000).

    Returns:
        Sanitized string.

    Raises:
        ValueError: If value is not a string.
    """
    if not isinstance(value, str):
        raise ValueError(f"Expected string, got {type(value).__name__}")

    sanitized = value.replace("\x00", "")
    sanitized = sanitized.strip()
    sanitized = re.sub(r"[ \t]+", " ", sanitized)

    if len(sanitized) > max_length:
        sanitized = sanitized[:max_length]

    return sanitized


def sanitize_html(value: str, allowed_tags: Optional[Set[str]] = None) -> str:
    """Escape HTML entities except allowed tags.

    Args:
        value: String with potential HTML.
        allowed_tags: Set of tag names to allow (default None = escape all).

    Returns:
        String with escaped HTML entities.
    """
    if allowed_tags is None:
        return html.escape(value, quote=True)

    tag_pattern = re.compile(
        r"<(/?)(\w+)([^>]*)>",
        re.IGNORECASE,
    )

    def _replace(match: re.Match) -> str:
        is_closing = match.group(1)
        tag_name = match.group(2).lower()
        attrs = match.group(3)

        if tag_name in allowed_tags:
            if is_closing:
                return f"</{tag_name}>"
            safe_attrs = ""
            if attrs:
                for attr_match in re.finditer(r'(\w+)=["\']([^"\']*)["\']', attrs):
                    attr_name = attr_match.group(1).lower()
                    if attr_name in {"href", "src", "alt", "title", "class"}:
                        safe_attrs += (
                            f' {attr_name}="{html.escape(attr_match.group(2))}"'
                        )
            return f"<{tag_name}{safe_attrs}>"
        return html.escape(match.group(0), quote=True)

    return tag_pattern.sub(_replace, value)


def sanitize_sql(value: str) -> str:
    """Remove common SQL injection patterns and escape quotes.

    WARNING: This is defense-in-depth only. Always use parameterized queries.

    Args:
        value: String to sanitize.

    Returns:
        Sanitized string.
    """
    dangerous_patterns = [
        r";?\s*DROP\s+",
        r";?\s*DELETE\s+",
        r";?\s*UPDATE\s+",
        r";?\s*INSERT\s+",
        r";?\s*EXEC\s*\(",
        r";?\s*EXECUTE\s*\(",
        r"--",
        r"/\*",
        r"\*/",
        r"xp_cmdshell",
    ]

    sanitized = value
    for pattern in dangerous_patterns:
        sanitized = re.sub(pattern, "", sanitized, flags=re.IGNORECASE)

    sanitized = sanitized.replace("'", "''")
    return sanitized


def sanitize_url(value: str) -> str:
    """Validate and normalize URL, strip dangerous protocols.

    Args:
        value: URL string.

    Returns:
        Sanitized URL.

    Raises:
        ValueError: If URL uses a dangerous scheme.
    """
    stripped = value.strip()

    if stripped.lower().startswith("javascript:"):
        raise ValueError("URL scheme 'javascript' is not allowed")

    if stripped.lower().startswith("data:"):
        raise ValueError("URL scheme 'data' is not allowed")

    if stripped.lower().startswith("vbscript:"):
        raise ValueError("URL scheme 'vbscript' is not allowed")

    parsed = urllib.parse.urlparse(stripped)

    safe_schemes = {"http", "https", "ftp", "ftps"}
    if parsed.scheme and parsed.scheme.lower() not in safe_schemes:
        raise ValueError(f"URL scheme '{parsed.scheme}' is not allowed")

    return urllib.parse.urlunparse(parsed)


def sanitize_json(value: Any) -> Any:
    """Validate JSON structure, remove dangerous patterns recursively.

    Args:
        value: JSON-like data structure.

    Returns:
        Sanitized data structure.
    """
    if isinstance(value, str):
        return sanitize_string(value)
    elif isinstance(value, dict):
        return {sanitize_string(k): sanitize_json(v) for k, v in value.items()}
    elif isinstance(value, list):
        return [sanitize_json(item) for item in value]
    else:
        return value


def sanitize_filename(value: str) -> str:
    """Remove path traversal, limit characters.

    Args:
        value: Filename to sanitize.

    Returns:
        Safe filename string.
    """
    sanitized = value.replace("/", "").replace("\\", "")
    sanitized = sanitized.replace("\x00", "")

    while sanitized.startswith("."):
        sanitized = sanitized[1:]

    sanitized = re.sub(r"[^\w\s.\-]", "", sanitized)
    sanitized = sanitized.strip()
    sanitized = sanitized[:255]

    return sanitized


def sanitize_slug(value: str) -> str:
    """Create URL-friendly slug (lowercase, hyphens, alphanumeric).

    Args:
        value: Input string.

    Returns:
        URL-safe slug string.
    """
    slug = value.lower()
    slug = re.sub(r"[\s_]+", "-", slug)
    slug = re.sub(r"[^a-z0-9-]", "", slug)
    slug = re.sub(r"-+", "-", slug)
    slug = slug.strip("-")
    return slug


def sanitize_email(value: str) -> str:
    """Lowercase, trim, basic format check.

    Args:
        value: Email string.

    Returns:
        Lowercased, trimmed email.

    Raises:
        ValueError: If email format is invalid.
    """
    email = value.strip().lower()

    pattern = re.compile(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
    if not pattern.match(email):
        raise ValueError(f"Invalid email format: {email}")

    return email


def sanitize_phone(value: str) -> str:
    """Strip non-digit characters, keep leading +.

    Args:
        value: Phone number string.

    Returns:
        Normalized phone number.
    """
    result: List[str] = []
    for char in value:
        if char.isdigit():
            result.append(char)
        elif char == "+" and len(result) == 0:
            result.append(char)

    return "".join(result)


def sanitize_alphanumeric(value: str) -> str:
    """Keep only alphanumeric characters and spaces.

    Args:
        value: Input string.

    Returns:
        Alphanumeric string with spaces preserved.
    """
    return re.sub(r"[^a-zA-Z0-9 ]", "", value)


def remove_control_chars(value: str) -> str:
    """Remove ASCII control characters except \\n, \\r, \\t.

    Args:
        value: Input string.

    Returns:
        String with control characters removed.
    """
    return re.sub(r"[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]", "", value)


def strip_html(value: str) -> str:
    """Remove ALL HTML tags.

    Args:
        value: String containing HTML.

    Returns:
        Plain text with tags removed.
    """
    return re.sub(r"<[^>]*>", "", value)


def trim_and_normalize_whitespace(value: str) -> str:
    """Normalize unicode whitespace: trim and collapse runs to single space.

    Args:
        value: Input string.

    Returns:
        Whitespace-normalized string.
    """
    stripped = value.strip().replace("\t", " ").replace("\n", " ").replace("\r", " ")
    normalized = re.sub(r"[ \t]+", " ", stripped)
    return normalized


def truncate(value: str, max_length: int, suffix: str = "...") -> str:
    """Smart truncation with suffix.

    Args:
        value: String to truncate.
        max_length: Maximum length including suffix.
        suffix: Suffix to append when truncated (default '...').

    Returns:
        Original or truncated string.
    """
    if len(value) <= max_length:
        return value

    truncate_at = max_length - len(suffix)
    if truncate_at < 0:
        return value[:max_length]

    return value[:truncate_at] + suffix


def escape_regex(value: str) -> str:
    """Escape regex special characters.

    Args:
        value: String containing literal text.

    Returns:
        Escaped string safe for regex use.
    """
    return re.sub(r"[.*+?^${}()|[\]\\]", r"\\\g<0>", value)


def escape_shell(value: str) -> str:
    """Escape shell special characters by single-quote wrapping.

    Args:
        value: String to escape.

    Returns:
        Shell-escaped string wrapped in single quotes.
    """
    return "'" + value.replace("'", "'\\''") + "'"


def redact_sensitive(value: str, pattern: Optional[str] = None) -> str:
    """Redact sensitive data patterns.

    Args:
        value: String potentially containing sensitive data.
        pattern: Regex pattern to match sensitive data.
                 If None, uses common patterns (API keys, emails, phone numbers).

    Returns:
        String with sensitive data redacted.
    """
    if pattern is not None:
        return re.sub(pattern, "[REDACTED]", value)

    patterns = [
        (r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b", "[REDACTED-EMAIL]"),
        (
            r"\b(?:sk|pk|api[_-]?key|secret|token|password)\s*[=:]\s*\S+",
            "[REDACTED-KEY]",
        ),
        (r"\b\d{3}[-.]?\d{2}[-.]?\d{4}\b", "[REDACTED-SSN]"),
        (
            r"\b(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b",
            "[REDACTED-PHONE]",
        ),
        (r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b", "[REDACTED-CARD]"),
    ]

    result = value
    for pat, replacement in patterns:
        result = re.sub(pat, replacement, result, flags=re.IGNORECASE)

    return result


def normalize_line_endings(value: str) -> str:
    """Normalize line endings: \\\\r\\\\n and \\\\r to \\\\n.

    Args:
        value: Input string.

    Returns:
        String with normalized line endings.
    """
    return value.replace("\r\n", "\n").replace("\r", "\n")
