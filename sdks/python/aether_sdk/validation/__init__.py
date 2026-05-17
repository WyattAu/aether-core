"""
Aether SDK Validation Module

Provides input validation, schema validation, and sanitization utilities
for building secure actor systems.

Example usage:
    from aether_sdk.validation import (
        Validator,
        SchemaValidator,
        sanitize_string,
        validate_email,
        validate_uuid,
    )

    # Validate input
    validator = Validator()
    validator.required('name', name)
    validator.email('email', email)
    validator.min_length('password', password, 8)

    if not validator.is_valid():
        raise ValidationError(validator.errors)

    # Schema validation
    schema = {
        'type': 'object',
        'properties': {
            'name': {'type': 'string', 'minLength': 1},
            'age': {'type': 'integer', 'minimum': 0},
        },
        'required': ['name'],
    }
    validator = SchemaValidator(schema)
    validator.validate(data)
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional, Pattern, TypeVar, Union

T = TypeVar("T")


# ============================================
# Exceptions
# ============================================


class ValidationError(Exception):
    """Raised when validation fails."""

    def __init__(self, errors: Union[List[str], Dict[str, List[str]]]):
        self.errors = errors if isinstance(errors, dict) else {"_global": errors}
        super().__init__(str(errors))

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "error": "validation_error",
            "errors": self.errors,
        }


class SchemaValidationError(ValidationError):
    """Raised when schema validation fails."""

    def __init__(self, errors: List[Dict[str, Any]]):
        self.schema_errors = errors
        error_messages = [str(e) for e in errors]
        super().__init__({"schema": error_messages})


# ============================================
# Sanitization Functions (re-exported from sanitize module)
# ============================================

from aether_sdk.validation.sanitize import (escape_regex,  # noqa: E402
                                            escape_shell,
                                            normalize_line_endings,
                                            redact_sensitive,
                                            remove_control_chars,
                                            sanitize_alphanumeric,
                                            sanitize_email, sanitize_filename,
                                            sanitize_html, sanitize_json,
                                            sanitize_phone, sanitize_slug,
                                            sanitize_sql, sanitize_string,
                                            sanitize_url, strip_html,
                                            trim_and_normalize_whitespace,
                                            truncate)

# Override sanitize_string to match original signature for backward compat
_orig_sanitize_string = sanitize_string


def sanitize_string(value: str, max_length: Optional[int] = None) -> str:
    """Sanitize a string by removing dangerous characters.

    Args:
        value: String to sanitize
        max_length: Maximum allowed length

    Returns:
        Sanitized string
    """
    if not isinstance(value, str):
        raise ValueError(f"Expected string, got {type(value).__name__}")
    if max_length is None:
        return _orig_sanitize_string(value, max_length=10000)
    return _orig_sanitize_string(value, max_length=max_length)


# Override sanitize_html to match original signature
_orig_sanitize_html = sanitize_html


def sanitize_html(value: str, allowed_tags=None) -> str:
    """Escape HTML entities in a string.

    Args:
        value: String with potential HTML
        allowed_tags: Set of tag names to allow (default None = escape all)

    Returns:
        String with escaped HTML entities
    """
    return _orig_sanitize_html(value, allowed_tags=allowed_tags)


# Override sanitize_json to match original signature
_orig_sanitize_json = sanitize_json


def sanitize_json(value: Any) -> Any:
    """Recursively sanitize JSON-like data.

    Args:
        value: JSON-like data structure

    Returns:
        Sanitized data structure
    """
    return _orig_sanitize_json(value)


# ============================================
# Common Validators (re-exported from validators module)
# ============================================

from aether_sdk.validation.validators import (  # noqa: E402
    ALPHANUMERIC_PATTERN, EMAIL_PATTERN, IP_PATTERN, PHONE_PATTERN,
    SLUG_PATTERN, USERNAME_PATTERN, UUID_PATTERN, ValidationErrorItem,
    ValidationErrors, Validator, validate_alphanumeric, validate_datetime,
    validate_dict, validate_email, validate_enum, validate_float,
    validate_integer, validate_ip, validate_list, validate_no_control_chars,
    validate_phone, validate_required, validate_slug, validate_string,
    validate_url, validate_username, validate_uuid)

# Re-export with backward-compatible parameter names
_orig_validate_integer = validate_integer


def validate_integer(
    value: Any, min_val: Optional[int] = None, max_val: Optional[int] = None
) -> bool:
    """Validate an integer with optional bounds."""
    return _orig_validate_integer(value, min_value=min_val, max_value=max_val)


_orig_validate_float = validate_float


def validate_float(
    value: Any, min_val: Optional[float] = None, max_val: Optional[float] = None
) -> bool:
    """Validate a float with optional bounds."""
    return _orig_validate_float(value, min_value=min_val, max_value=max_val)


_orig_validate_string = validate_string


def validate_string(
    value: Any,
    min_length: int = 0,
    max_length: Optional[int] = None,
    pattern: Optional[Pattern] = None,
) -> bool:
    """Validate a string with length and pattern constraints."""
    return _orig_validate_string(
        value, min_length=min_length, max_length=max_length, pattern=pattern
    )


_orig_validate_list = validate_list


def validate_list(
    value: Any,
    min_length: int = 0,
    max_length: Optional[int] = None,
    item_validator: Optional[Callable[[Any], bool]] = None,
) -> bool:
    """Validate a list with length and item constraints."""
    return _orig_validate_list(
        value,
        min_length=min_length,
        max_length=max_length,
        item_validator=item_validator,
    )


_orig_validate_dict = validate_dict


def validate_dict(
    value: Any,
    required_keys: Optional[List[str]] = None,
    optional_keys: Optional[List[str]] = None,
) -> bool:
    """Validate a dictionary with key constraints."""
    return _orig_validate_dict(
        value, required_keys=required_keys, optional_keys=optional_keys
    )


# ============================================
# Schema Validator (JSON Schema-like)
# ============================================


@dataclass
class SchemaValidator:
    """JSON Schema-like validator."""

    schema: Dict[str, Any]
    errors: List[Dict[str, Any]] = field(default_factory=list)

    def validate(self, data: Any) -> bool:
        """Validate data against the schema."""
        self.errors = []
        self._validate(data, self.schema, "$")
        return len(self.errors) == 0

    def _add_error(self, path: str, message: str, value: Any = None) -> None:
        """Add an error."""
        self.errors.append(
            {
                "path": path,
                "message": message,
                "value": value,
            }
        )

    def _validate(self, value: Any, schema: Dict[str, Any], path: str) -> None:
        """Recursively validate value against schema."""
        if not isinstance(schema, dict):
            return

        schema_type = schema.get("type")
        if schema_type:
            if not self._validate_type(value, schema_type):
                self._add_error(
                    path, f"Expected {schema_type}, got {type(value).__name__}", value
                )
                return

        if schema_type == "object":
            self._validate_object(value, schema, path)
        elif schema_type == "array":
            self._validate_array(value, schema, path)
        elif schema_type == "string":
            self._validate_string(value, schema, path)
        elif schema_type in ("integer", "number"):
            self._validate_number(value, schema, path)

    def _validate_type(self, value: Any, schema_type: str) -> bool:
        """Validate value type."""
        type_map = {
            "string": str,
            "integer": int,
            "number": (int, float),
            "boolean": bool,
            "array": list,
            "object": dict,
            "null": type(None),
        }

        expected = type_map.get(schema_type)
        if expected is None:
            return True

        if schema_type == "integer":
            return isinstance(value, int) and not isinstance(value, bool)

        return isinstance(value, expected)

    def _validate_object(self, value: dict, schema: dict, path: str) -> None:
        """Validate object properties."""
        properties = schema.get("properties", {})
        required = schema.get("required", [])
        additional_props = schema.get("additionalProperties", True)

        for prop in required:
            if prop not in value:
                self._add_error(f"{path}.{prop}", "Required property missing")

        for prop, prop_schema in properties.items():
            if prop in value:
                self._validate(value[prop], prop_schema, f"{path}.{prop}")

        if not additional_props:
            for prop in value:
                if prop not in properties:
                    self._add_error(f"{path}.{prop}", "Additional property not allowed")

    def _validate_array(self, value: list, schema: dict, path: str) -> None:
        """Validate array items."""
        min_items = schema.get("minItems")
        max_items = schema.get("maxItems")
        items_schema = schema.get("items")

        if min_items is not None and len(value) < min_items:
            self._add_error(path, f"Array must have at least {min_items} items")

        if max_items is not None and len(value) > max_items:
            self._add_error(path, f"Array must have at most {max_items} items")

        if items_schema:
            for i, item in enumerate(value):
                self._validate(item, items_schema, f"{path}[{i}]")

    def _validate_string(self, value: str, schema: dict, path: str) -> None:
        """Validate string constraints."""
        min_length = schema.get("minLength")
        max_length = schema.get("maxLength")
        pattern = schema.get("pattern")
        format_str = schema.get("format")
        enum = schema.get("enum")

        if min_length is not None and len(value) < min_length:
            self._add_error(path, f"String must be at least {min_length} characters")

        if max_length is not None and len(value) > max_length:
            self._add_error(path, f"String must be at most {max_length} characters")

        if pattern is not None:
            if not re.match(pattern, value):
                self._add_error(path, f"String does not match pattern: {pattern}")

        if format_str is not None:
            self._validate_format(value, format_str, path)

        if enum is not None and value not in enum:
            self._add_error(path, f"Value must be one of: {enum}")

    def _validate_number(
        self, value: Union[int, float], schema: dict, path: str
    ) -> None:
        """Validate number constraints."""
        minimum = schema.get("minimum")
        maximum = schema.get("maximum")
        exclusive_minimum = schema.get("exclusiveMinimum")
        exclusive_maximum = schema.get("exclusiveMaximum")
        enum = schema.get("enum")

        if minimum is not None and value < minimum:
            self._add_error(path, f"Value must be at least {minimum}")

        if maximum is not None and value > maximum:
            self._add_error(path, f"Value must be at most {maximum}")

        if exclusive_minimum is not None and value <= exclusive_minimum:
            self._add_error(path, f"Value must be greater than {exclusive_minimum}")

        if exclusive_maximum is not None and value >= exclusive_maximum:
            self._add_error(path, f"Value must be less than {exclusive_maximum}")

        if enum is not None and value not in enum:
            self._add_error(path, f"Value must be one of: {enum}")

    def _validate_format(self, value: str, format_str: str, path: str) -> None:
        """Validate string format."""
        format_validators = {
            "email": validate_email,
            "uri": validate_url,
            "url": validate_url,
            "uuid": validate_uuid,
            "phone": validate_phone,
            "slug": validate_slug,
            "date": lambda v: validate_datetime(v, "%Y-%m-%d"),
            "date-time": validate_datetime,
        }

        validator = format_validators.get(format_str)
        if validator and not validator(value):
            self._add_error(path, f"Invalid {format_str} format")


# ============================================
# Decorators
# ============================================


def validated(*validators: Callable[[Any], None]):
    """Decorator to validate function arguments.

    Example:
        @validated(
            lambda name: Validator().required('name', name).min_length('name', name, 3),
            lambda email: Validator().email('email', email),
        )
        def create_user(name: str, email: str):
            ...
    """

    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        def wrapper(*args, **kwargs) -> T:
            for i, validator in enumerate(validators):
                if i < len(args):
                    try:
                        validator(args[i])
                    except ValidationError:
                        raise

            return func(*args, **kwargs)

        return wrapper

    return decorator


# ============================================
# Exports
# ============================================

__all__ = [
    # Exceptions
    "ValidationError",
    "SchemaValidationError",
    "ValidationErrors",
    "ValidationErrorItem",
    # Sanitization
    "sanitize_string",
    "sanitize_html",
    "sanitize_sql",
    "sanitize_url",
    "sanitize_json",
    "sanitize_filename",
    "sanitize_slug",
    "sanitize_email",
    "sanitize_phone",
    "sanitize_alphanumeric",
    "remove_control_chars",
    "strip_html",
    "trim_and_normalize_whitespace",
    "truncate",
    "escape_regex",
    "escape_shell",
    "redact_sensitive",
    "normalize_line_endings",
    # Validators
    "validate_email",
    "validate_uuid",
    "validate_alphanumeric",
    "validate_username",
    "validate_phone",
    "validate_slug",
    "validate_url",
    "validate_ip",
    "validate_integer",
    "validate_float",
    "validate_string",
    "validate_datetime",
    "validate_enum",
    "validate_list",
    "validate_dict",
    "validate_required",
    "validate_no_control_chars",
    # Classes
    "Validator",
    "SchemaValidator",
    # Decorators
    "validated",
    # Patterns
    "EMAIL_PATTERN",
    "UUID_PATTERN",
    "ALPHANUMERIC_PATTERN",
    "USERNAME_PATTERN",
    "PHONE_PATTERN",
    "SLUG_PATTERN",
    "IP_PATTERN",
]
