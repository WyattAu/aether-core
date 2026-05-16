"""
Validation Functions and Validator Class.

Provides standalone validation functions for common patterns (email, UUID,
integer bounds, etc.) and a fluent Validator class for building compound
validation rules with field-level error messages.
"""

from __future__ import annotations

import re
import urllib.parse
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Callable, Dict, List, Optional, Pattern, Union

# ============================================
# Regex Patterns
# ============================================

EMAIL_PATTERN = re.compile(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
UUID_PATTERN = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)
ALPHANUMERIC_PATTERN = re.compile(r"^[a-zA-Z0-9]+$")
USERNAME_PATTERN = re.compile(r"^[a-zA-Z0-9_-]+$")
PHONE_PATTERN = re.compile(r"^\+?[1-9]\d{1,14}$")
SLUG_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
IP_PATTERN = re.compile(
    r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}"
    r"(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$"
)


# ============================================
# ValidationError Dataclass
# ============================================


@dataclass
class ValidationErrorItem:
    """Describes a single validation error for a specific field."""

    field_name: str
    message: str
    value: Any = None


# ============================================
# ValidationErrors Exception
# ============================================


class ValidationErrors(Exception):
    """Raised when validation produces errors.

    Args:
        errors: List of ValidationErrorItem instances.
    """

    def __init__(self, errors: List[ValidationErrorItem]):
        self.error_list = errors
        messages = "; ".join(f"{e.field_name}: {e.message}" for e in errors)
        super().__init__(messages)


# ============================================
# Standalone Validation Functions
# ============================================


def validate_email(value: str) -> bool:
    """Validate an email address."""
    if not value or not isinstance(value, str):
        return False
    return bool(EMAIL_PATTERN.match(value))


def validate_uuid(value: str) -> bool:
    """Validate a UUID string (v1-v5)."""
    if not value or not isinstance(value, str):
        return False
    return bool(UUID_PATTERN.match(value))


def validate_alphanumeric(value: str) -> bool:
    """Validate that string contains only alphanumeric characters."""
    if not value or not isinstance(value, str):
        return False
    return bool(ALPHANUMERIC_PATTERN.match(value))


def validate_username(value: str) -> bool:
    """Validate a username (alphanumeric, underscore, hyphen)."""
    if not value or not isinstance(value, str):
        return False
    return bool(USERNAME_PATTERN.match(value))


def validate_phone(value: str) -> bool:
    """Validate a phone number (E.164 format)."""
    if not value or not isinstance(value, str):
        return False
    return bool(PHONE_PATTERN.match(value))


def validate_slug(value: str) -> bool:
    """Validate a URL slug."""
    if not value or not isinstance(value, str):
        return False
    return bool(SLUG_PATTERN.match(value))


def validate_url(value: str, allowed_schemes: Optional[List[str]] = None) -> bool:
    """Validate a URL with optional scheme restriction."""
    if not value or not isinstance(value, str):
        return False
    try:
        parsed = urllib.parse.urlparse(value)
        schemes = set(allowed_schemes or ["http", "https"])
        if parsed.scheme.lower() not in schemes:
            return False
        if not parsed.netloc:
            return False
        return True
    except Exception:
        return False


def validate_ip(value: str) -> bool:
    """Validate an IPv4 address."""
    if not value or not isinstance(value, str):
        return False
    return bool(IP_PATTERN.match(value))


def validate_integer(
    value: Any,
    min_value: Optional[int] = None,
    max_value: Optional[int] = None,
) -> bool:
    """Validate an integer with optional bounds."""
    if not isinstance(value, int) or isinstance(value, bool):
        return False
    if min_value is not None and value < min_value:
        return False
    if max_value is not None and value > max_value:
        return False
    return True


def validate_float(
    value: Any,
    min_value: Optional[float] = None,
    max_value: Optional[float] = None,
) -> bool:
    """Validate a float with optional bounds."""
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        return False
    if min_value is not None and value < min_value:
        return False
    if max_value is not None and value > max_value:
        return False
    return True


def validate_string(
    value: Any,
    min_length: int = 0,
    max_length: Optional[int] = None,
    pattern: Optional[Pattern] = None,
) -> bool:
    """Validate a string with length and pattern constraints."""
    if not isinstance(value, str):
        return False
    if len(value) < min_length:
        return False
    if max_length is not None and len(value) > max_length:
        return False
    if pattern is not None and not pattern.match(value):
        return False
    return True


def validate_datetime(value: Any, format_str: Optional[str] = None) -> bool:
    """Validate a datetime string."""
    if isinstance(value, datetime):
        return True
    if not isinstance(value, str):
        return False
    try:
        if format_str:
            datetime.strptime(value, format_str)
        else:
            datetime.fromisoformat(value.replace("Z", "+00:00"))
        return True
    except ValueError:
        return False


def validate_enum(value: Any, allowed: List[Any]) -> bool:
    """Validate that value is in allowed list."""
    return value in allowed


def validate_list(
    value: Any,
    min_length: int = 0,
    max_length: Optional[int] = None,
    item_validator: Optional[Callable[[Any], bool]] = None,
) -> bool:
    """Validate a list with length and item constraints."""
    if not isinstance(value, list):
        return False
    if len(value) < min_length:
        return False
    if max_length is not None and len(value) > max_length:
        return False
    if item_validator:
        return all(item_validator(item) for item in value)
    return True


def validate_dict(
    value: Any,
    required_keys: Optional[List[str]] = None,
    optional_keys: Optional[List[str]] = None,
) -> bool:
    """Validate a dictionary with key constraints."""
    if not isinstance(value, dict):
        return False
    if required_keys:
        for key in required_keys:
            if key not in value:
                return False
    if required_keys and optional_keys:
        allowed_keys = set(required_keys) | set(optional_keys)
        for key in value:
            if key not in allowed_keys:
                return False
    return True


def validate_required(value: Any) -> bool:
    """Validate that a value is present (not None, empty string, empty list/dict)."""
    if value is None:
        return False
    if isinstance(value, str) and value.strip() == "":
        return False
    if isinstance(value, (list, dict)) and len(value) == 0:
        return False
    return True


def validate_no_control_chars(value: str) -> bool:
    """Validate that string contains no control characters except \\n, \\r, \\t."""
    if not isinstance(value, str):
        return False
    for char in value:
        code = ord(char)
        if (code < 32 or code == 127) and char not in ("\n", "\r", "\t"):
            return False
    return True


# ============================================
# Validator Class (Fluent API)
# ============================================


@dataclass
class Validator:
    """Fluent validator for building compound validation rules.

    Accumulates field-level errors and provides a chainable API for
    defining multiple rules in sequence.
    """

    _errors: Dict[str, List[str]] = field(default_factory=dict)

    @property
    def errors(self) -> Dict[str, List[str]]:
        return self._errors

    def add_error(self, field_name: str, message: str) -> "Validator":
        """Add a custom error for a field."""
        if field_name not in self._errors:
            self._errors[field_name] = []
        self._errors[field_name].append(message)
        return self

    def clear(self) -> "Validator":
        """Reset all errors."""
        self._errors = {}
        return self

    def is_valid(self) -> bool:
        """Check if no errors have been recorded."""
        return len(self._errors) == 0

    @property
    def error_list(self) -> List[ValidationErrorItem]:
        """Get all errors as ValidationErrorItem list."""
        items: List[ValidationErrorItem] = []
        for field_name, messages in self._errors.items():
            for message in messages:
                items.append(
                    ValidationErrorItem(
                        field_name=field_name,
                        message=message,
                    )
                )
        return items

    @property
    def first_error(self) -> Optional[str]:
        """Get first error message, or None if valid."""
        if self._errors:
            for messages in self._errors.values():
                if messages:
                    return messages[0]
        return None

    def validate_string(
        self,
        name: str,
        value: Any,
        min_length: Optional[int] = None,
        max_length: Optional[int] = None,
        pattern: Optional[Union[str, Pattern]] = None,
    ) -> "Validator":
        """Validate a string field with length and pattern constraints."""
        if value is not None and not isinstance(value, str):
            self.add_error(name, f"{name} must be a string")
            return self
        if value is not None:
            if min_length is not None and len(value) < min_length:
                self.add_error(name, f"{name} must be at least {min_length} characters")
            if max_length is not None and len(value) > max_length:
                self.add_error(name, f"{name} must be at most {max_length} characters")
            if pattern is not None:
                compiled = re.compile(pattern) if isinstance(pattern, str) else pattern
                if not compiled.match(value):
                    self.add_error(name, f"{name} has invalid format")
        return self

    def validate_email(self, name: str, value: Any) -> "Validator":
        """Validate email format."""
        if value is not None and not validate_email(value):
            self.add_error(name, f"{name} must be a valid email")
        return self

    def validate_url(self, name: str, value: Any) -> "Validator":
        """Validate URL format."""
        if value is not None and not validate_url(value):
            self.add_error(name, f"{name} must be a valid URL")
        return self

    def validate_uuid(self, name: str, value: Any) -> "Validator":
        """Validate UUID format."""
        if value is not None and not validate_uuid(value):
            self.add_error(name, f"{name} must be a valid UUID")
        return self

    def validate_integer(
        self,
        name: str,
        value: Any,
        min_value: Optional[int] = None,
        max_value: Optional[int] = None,
    ) -> "Validator":
        """Validate integer with optional bounds."""
        if value is not None and not validate_integer(value, min_value, max_value):
            msg = f"{name} must be a valid integer"
            if min_value is not None or max_value is not None:
                bounds = []
                if min_value is not None:
                    bounds.append(f">= {min_value}")
                if max_value is not None:
                    bounds.append(f"<= {max_value}")
                msg = f'{name} must be an integer ({", ".join(bounds)})'
            self.add_error(name, msg)
        return self

    def validate_float(
        self,
        name: str,
        value: Any,
        min_value: Optional[float] = None,
        max_value: Optional[float] = None,
    ) -> "Validator":
        """Validate float with optional bounds."""
        if value is not None and not validate_float(value, min_value, max_value):
            msg = f"{name} must be a valid number"
            if min_value is not None or max_value is not None:
                bounds = []
                if min_value is not None:
                    bounds.append(f">= {min_value}")
                if max_value is not None:
                    bounds.append(f"<= {max_value}")
                msg = f'{name} must be a number ({", ".join(bounds)})'
            self.add_error(name, msg)
        return self

    def validate_datetime(self, name: str, value: Any) -> "Validator":
        """Validate datetime."""
        if value is not None and not validate_datetime(value):
            self.add_error(name, f"{name} must be a valid datetime")
        return self

    def validate_phone(self, name: str, value: Any) -> "Validator":
        """Validate phone number format."""
        if value is not None and not validate_phone(value):
            self.add_error(name, f"{name} must be a valid phone number")
        return self

    def validate_ip(self, name: str, value: Any) -> "Validator":
        """Validate IP address."""
        if value is not None and not validate_ip(value):
            self.add_error(name, f"{name} must be a valid IP address")
        return self

    def validate_enum(
        self, name: str, value: Any, allowed_values: List[Any]
    ) -> "Validator":
        """Validate that value is one of the allowed values."""
        if not validate_enum(value, allowed_values):
            self.add_error(name, f"{name} must be one of the allowed values")
        return self

    def validate_list(
        self,
        name: str,
        value: Any,
        min_length: Optional[int] = None,
        max_length: Optional[int] = None,
    ) -> "Validator":
        """Validate a list field with length constraints."""
        if value is not None and not isinstance(value, list):
            self.add_error(name, f"{name} must be a list")
            return self
        if value is not None:
            if min_length is not None and len(value) < min_length:
                self.add_error(name, f"{name} must have at least {min_length} items")
            if max_length is not None and len(value) > max_length:
                self.add_error(name, f"{name} must have at most {max_length} items")
        return self

    def validate_object(
        self,
        name: str,
        value: Any,
        required_fields: Optional[List[str]] = None,
    ) -> "Validator":
        """Validate an object/dict with optional required fields."""
        if value is not None and not isinstance(value, dict):
            self.add_error(name, f"{name} must be an object")
            return self
        if value is not None and required_fields:
            for rf in required_fields:
                if rf not in value:
                    self.add_error(f"{name}.{rf}", f"{name}.{rf} is required")
        return self

    def validate_required(self, name: str, value: Any) -> "Validator":
        """Validate that a field is present and non-empty."""
        if not validate_required(value):
            self.add_error(name, f"{name} is required")
        return self

    def validate_slug(self, name: str, value: Any) -> "Validator":
        """Validate URL slug format."""
        if value is not None and not validate_slug(value):
            self.add_error(name, f"{name} must be a valid slug")
        return self

    def validate_username(self, name: str, value: Any) -> "Validator":
        """Validate username format."""
        if value is not None and not validate_username(value):
            self.add_error(name, f"{name} must be a valid username")
        return self

    def validate_alphanumeric(self, name: str, value: Any) -> "Validator":
        """Validate that value is alphanumeric."""
        if value is not None and not validate_alphanumeric(value):
            self.add_error(name, f"{name} must be alphanumeric")
        return self

    def validate_no_control_chars(self, name: str, value: Any) -> "Validator":
        """Validate that string has no control characters."""
        if value is not None and not validate_no_control_chars(value):
            self.add_error(name, f"{name} contains invalid control characters")
        return self

    def when(
        self,
        condition: bool,
        validator_fn: Callable[["Validator"], "Validator"],
    ) -> "Validator":
        """Conditional validation: apply validator_fn only if condition is True."""
        if condition:
            return validator_fn(self)
        return self

    # ---- Backward-compatible aliases matching the original Validator API ----

    def required(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is present and not empty."""
        if value is None or (isinstance(value, str) and not value.strip()):
            self.add_error(field, message or f"{field} is required")
        return self

    def string(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is a string."""
        if value is not None and not isinstance(value, str):
            self.add_error(field, message or f"{field} must be a string")
        return self

    def integer(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is an integer."""
        if value is not None and not isinstance(value, int):
            self.add_error(field, message or f"{field} must be an integer")
        return self

    def float(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is a number."""
        if value is not None and not isinstance(value, (int, float)):
            self.add_error(field, message or f"{field} must be a number")
        return self

    def boolean(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is a boolean."""
        if value is not None and not isinstance(value, bool):
            self.add_error(field, message or f"{field} must be a boolean")
        return self

    def list(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is a list."""
        if value is not None and not isinstance(value, list):
            self.add_error(field, message or f"{field} must be a list")
        return self

    def dict(
        self, field: str, value: Any, message: Optional[str] = None
    ) -> "Validator":
        """Validate that a field is a dictionary."""
        if value is not None and not isinstance(value, dict):
            self.add_error(field, message or f"{field} must be an object")
        return self

    def min_length(
        self,
        field: str,
        value: Optional[str],
        min_len: int,
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate minimum string length."""
        if value is not None and len(value) < min_len:
            self.add_error(
                field, message or f"{field} must be at least {min_len} characters"
            )
        return self

    def max_length(
        self,
        field: str,
        value: Optional[str],
        max_len: int,
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate maximum string length."""
        if value is not None and len(value) > max_len:
            self.add_error(
                field, message or f"{field} must be at most {max_len} characters"
            )
        return self

    def pattern(
        self,
        field: str,
        value: Optional[str],
        regex: Union[str, Pattern],
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate string against a regex pattern."""
        if value is not None:
            compiled = re.compile(regex) if isinstance(regex, str) else regex
            if not compiled.match(value):
                self.add_error(field, message or f"{field} has invalid format")
        return self

    def min_value(
        self,
        field: str,
        value: Optional[Union[int, float]],
        min_val: Union[int, float],
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate minimum numeric value."""
        if value is not None and value < min_val:
            self.add_error(field, message or f"{field} must be at least {min_val}")
        return self

    def max_value(
        self,
        field: str,
        value: Optional[Union[int, float]],
        max_val: Union[int, float],
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate maximum numeric value."""
        if value is not None and value > max_val:
            self.add_error(field, message or f"{field} must be at most {max_val}")
        return self

    def range(
        self,
        field: str,
        value: Optional[Union[int, float]],
        min_val: Union[int, float],
        max_val: Union[int, float],
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate numeric value is within range."""
        if value is not None and (value < min_val or value > max_val):
            self.add_error(
                field, message or f"{field} must be between {min_val} and {max_val}"
            )
        return self

    def email(
        self, field: str, value: Optional[str], message: Optional[str] = None
    ) -> "Validator":
        """Validate email format."""
        if value is not None and not validate_email(value):
            self.add_error(field, message or f"{field} must be a valid email")
        return self

    def url(
        self,
        field: str,
        value: Optional[str],
        allowed_schemes: Optional[List[str]] = None,
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate URL format."""
        if value is not None and not validate_url(value, allowed_schemes):
            self.add_error(field, message or f"{field} must be a valid URL")
        return self

    def uuid(
        self, field: str, value: Optional[str], message: Optional[str] = None
    ) -> "Validator":
        """Validate UUID format."""
        if value is not None and not validate_uuid(value):
            self.add_error(field, message or f"{field} must be a valid UUID")
        return self

    def phone(
        self, field: str, value: Optional[str], message: Optional[str] = None
    ) -> "Validator":
        """Validate phone number format."""
        if value is not None and not validate_phone(value):
            self.add_error(field, message or f"{field} must be a valid phone number")
        return self

    def slug(
        self, field: str, value: Optional[str], message: Optional[str] = None
    ) -> "Validator":
        """Validate URL slug format."""
        if value is not None and not validate_slug(value):
            self.add_error(field, message or f"{field} must be a valid slug")
        return self

    def min_items(
        self,
        field: str,
        value: Optional[List],
        min_items: int,
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate minimum list length."""
        if value is not None and len(value) < min_items:
            self.add_error(
                field, message or f"{field} must have at least {min_items} items"
            )
        return self

    def max_items(
        self,
        field: str,
        value: Optional[List],
        max_items: int,
        message: Optional[str] = None,
    ) -> "Validator":
        """Validate maximum list length."""
        if value is not None and len(value) > max_items:
            self.add_error(
                field, message or f"{field} must have at most {max_items} items"
            )
        return self

    def custom(
        self, field: str, value: Any, validator: Callable[[Any], bool], message: str
    ) -> "Validator":
        """Apply custom validation."""
        if not validator(value):
            self.add_error(field, message)
        return self
