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
from dataclasses import dataclass, field
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Optional,
    Pattern,
    TypeVar,
    Union,
)
import re
import html
import urllib.parse
from datetime import datetime

T = TypeVar('T')


# ============================================
# Exceptions
# ============================================

class ValidationError(Exception):
    """Raised when validation fails."""
    
    def __init__(self, errors: Union[List[str], Dict[str, List[str]]]):
        self.errors = errors if isinstance(errors, dict) else {'_global': errors}
        super().__init__(str(errors))
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            'error': 'validation_error',
            'errors': self.errors,
        }


class SchemaValidationError(ValidationError):
    """Raised when schema validation fails."""
    
    def __init__(self, errors: List[Dict[str, Any]]):
        self.schema_errors = errors
        # Convert to string representation for parent class
        error_messages = [str(e) for e in errors]
        super().__init__({'schema': error_messages})


# ============================================
# Sanitization Functions
# ============================================

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
    
    # Remove null bytes
    sanitized = value.replace('\x00', '')
    
    # Strip whitespace
    sanitized = sanitized.strip()
    
    # Truncate if needed
    if max_length is not None:
        sanitized = sanitized[:max_length]
    
    return sanitized


def sanitize_html(value: str) -> str:
    """Escape HTML entities in a string.
    
    Args:
        value: String with potential HTML
        
    Returns:
        String with escaped HTML entities
    """
    return html.escape(value, quote=True)


def sanitize_sql(value: str) -> str:
    """Basic SQL injection prevention (use parameterized queries instead!).
    
    This is a fallback for cases where parameterized queries aren't possible.
    Always prefer parameterized queries.
    
    Args:
        value: String to sanitize
        
    Returns:
        Sanitized string
    """
    # Remove common SQL injection patterns
    dangerous_patterns = [
        r';\s*drop\s+',
        r';\s*delete\s+',
        r';\s*update\s+',
        r';\s*insert\s+',
        r'--',
        r'/\*',
        r'\*/',
        r'xp_cmdshell',
        r'exec\s*\(',
        r'execute\s*\(',
    ]
    
    sanitized = value
    for pattern in dangerous_patterns:
        sanitized = re.sub(pattern, '', sanitized, flags=re.IGNORECASE)
    
    return sanitized


def sanitize_url(value: str) -> str:
    """Sanitize and validate a URL.
    
    Args:
        value: URL string
        
    Returns:
        Sanitized URL
        
    Raises:
        ValueError: If URL is invalid or uses dangerous scheme
    """
    parsed = urllib.parse.urlparse(value)
    
    # Only allow safe schemes
    safe_schemes = {'http', 'https', 'ftp', 'ftps'}
    if parsed.scheme and parsed.scheme.lower() not in safe_schemes:
        raise ValueError(f"URL scheme '{parsed.scheme}' is not allowed")
    
    # Reconstruct URL to normalize
    return urllib.parse.urlunparse(parsed)


def sanitize_json(value: Any) -> Any:
    """Recursively sanitize JSON-like data.
    
    Args:
        value: JSON-like data structure
        
    Returns:
        Sanitized data structure
    """
    if isinstance(value, str):
        return sanitize_string(value)
    elif isinstance(value, dict):
        return {sanitize_string(k): sanitize_json(v) for k, v in value.items()}
    elif isinstance(value, list):
        return [sanitize_json(item) for item in value]
    else:
        return value


# ============================================
# Common Validators
# ============================================

# Regex patterns
EMAIL_PATTERN = re.compile(
    r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'
)
UUID_PATTERN = re.compile(
    r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$',
    re.IGNORECASE
)
ALPHANUMERIC_PATTERN = re.compile(r'^[a-zA-Z0-9]+$')
USERNAME_PATTERN = re.compile(r'^[a-zA-Z0-9_-]+$')
PHONE_PATTERN = re.compile(r'^\+?[1-9]\d{1,14}$')
SLUG_PATTERN = re.compile(r'^[a-z0-9]+(?:-[a-z0-9]+)*$')


def validate_email(value: str) -> bool:
    """Validate an email address."""
    return bool(EMAIL_PATTERN.match(value))


def validate_uuid(value: str) -> bool:
    """Validate a UUID string."""
    return bool(UUID_PATTERN.match(value))


def validate_alphanumeric(value: str) -> bool:
    """Validate that string contains only alphanumeric characters."""
    return bool(ALPHANUMERIC_PATTERN.match(value))


def validate_username(value: str) -> bool:
    """Validate a username (alphanumeric, underscore, hyphen)."""
    return bool(USERNAME_PATTERN.match(value))


def validate_phone(value: str) -> bool:
    """Validate a phone number (E.164 format)."""
    return bool(PHONE_PATTERN.match(value))


def validate_slug(value: str) -> bool:
    """Validate a URL slug."""
    return bool(SLUG_PATTERN.match(value))


def validate_url(value: str, allowed_schemes: Optional[List[str]] = None) -> bool:
    """Validate a URL.
    
    Args:
        value: URL to validate
        allowed_schemes: List of allowed schemes (default: http, https)
        
    Returns:
        True if valid
    """
    try:
        parsed = urllib.parse.urlparse(value)
        schemes = set(allowed_schemes or ['http', 'https'])
        
        if parsed.scheme.lower() not in schemes:
            return False
        
        if not parsed.netloc:
            return False
        
        return True
    except Exception:
        return False


def validate_integer(value: Any, min_val: Optional[int] = None, max_val: Optional[int] = None) -> bool:
    """Validate an integer with optional bounds."""
    if not isinstance(value, int) or isinstance(value, bool):
        return False
    
    if min_val is not None and value < min_val:
        return False
    
    if max_val is not None and value > max_val:
        return False
    
    return True


def validate_float(value: Any, min_val: Optional[float] = None, max_val: Optional[float] = None) -> bool:
    """Validate a float with optional bounds."""
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        return False
    
    if min_val is not None and value < min_val:
        return False
    
    if max_val is not None and value > max_val:
        return False
    
    return True


def validate_string(value: Any, min_length: int = 0, max_length: Optional[int] = None, 
                    pattern: Optional[Pattern] = None) -> bool:
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
            # Try ISO format
            datetime.fromisoformat(value.replace('Z', '+00:00'))
        return True
    except ValueError:
        return False


def validate_enum(value: Any, allowed: List[Any]) -> bool:
    """Validate that value is in allowed list."""
    return value in allowed


def validate_list(value: Any, min_length: int = 0, max_length: Optional[int] = None,
                  item_validator: Optional[Callable[[Any], bool]] = None) -> bool:
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


def validate_dict(value: Any, required_keys: Optional[List[str]] = None,
                  optional_keys: Optional[List[str]] = None) -> bool:
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


# ============================================
# Validator Class
# ============================================

@dataclass
class Validator:
    """Fluent validator for building validation rules.
    
    Example:
        validator = Validator()
        validator.required('name', name)
        validator.email('email', email)
        validator.min_length('password', password, 8)
        
        if not validator.is_valid():
            raise ValidationError(validator.errors)
    """
    
    errors: Dict[str, List[str]] = field(default_factory=dict)
    
    def add_error(self, field: str, message: str) -> 'Validator':
        """Add an error for a field."""
        if field not in self.errors:
            self.errors[field] = []
        self.errors[field].append(message)
        return self
    
    def is_valid(self) -> bool:
        """Check if all validations passed."""
        return len(self.errors) == 0
    
    def clear(self) -> 'Validator':
        """Clear all errors."""
        self.errors = {}
        return self
    
    # Required validation
    def required(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is present and not empty."""
        if value is None or (isinstance(value, str) and not value.strip()):
            self.add_error(field, message or f'{field} is required')
        return self
    
    # Type validations
    def string(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is a string."""
        if value is not None and not isinstance(value, str):
            self.add_error(field, message or f'{field} must be a string')
        return self
    
    def integer(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is an integer."""
        if value is not None and not isinstance(value, int):
            self.add_error(field, message or f'{field} must be an integer')
        return self
    
    def float(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is a float."""
        if value is not None and not isinstance(value, (int, float)):
            self.add_error(field, message or f'{field} must be a number')
        return self
    
    def boolean(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is a boolean."""
        if value is not None and not isinstance(value, bool):
            self.add_error(field, message or f'{field} must be a boolean')
        return self
    
    def list(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is a list."""
        if value is not None and not isinstance(value, list):
            self.add_error(field, message or f'{field} must be a list')
        return self
    
    def dict(self, field: str, value: Any, message: Optional[str] = None) -> 'Validator':
        """Validate that a field is a dictionary."""
        if value is not None and not isinstance(value, dict):
            self.add_error(field, message or f'{field} must be an object')
        return self
    
    # String validations
    def min_length(self, field: str, value: Optional[str], min_len: int,
                   message: Optional[str] = None) -> 'Validator':
        """Validate minimum string length."""
        if value is not None and len(value) < min_len:
            self.add_error(field, message or f'{field} must be at least {min_len} characters')
        return self
    
    def max_length(self, field: str, value: Optional[str], max_len: int,
                   message: Optional[str] = None) -> 'Validator':
        """Validate maximum string length."""
        if value is not None and len(value) > max_len:
            self.add_error(field, message or f'{field} must be at most {max_len} characters')
        return self
    
    def pattern(self, field: str, value: Optional[str], regex: Union[str, Pattern],
                message: Optional[str] = None) -> 'Validator':
        """Validate string against a regex pattern."""
        if value is not None:
            pattern = re.compile(regex) if isinstance(regex, str) else regex
            if not pattern.match(value):
                self.add_error(field, message or f'{field} has invalid format')
        return self
    
    # Numeric validations
    def min_value(self, field: str, value: Optional[Union[int, float]], min_val: Union[int, float],
                  message: Optional[str] = None) -> 'Validator':
        """Validate minimum numeric value."""
        if value is not None and value < min_val:
            self.add_error(field, message or f'{field} must be at least {min_val}')
        return self
    
    def max_value(self, field: str, value: Optional[Union[int, float]], max_val: Union[int, float],
                  message: Optional[str] = None) -> 'Validator':
        """Validate maximum numeric value."""
        if value is not None and value > max_val:
            self.add_error(field, message or f'{field} must be at most {max_val}')
        return self
    
    def range(self, field: str, value: Optional[Union[int, float]], min_val: Union[int, float],
              max_val: Union[int, float], message: Optional[str] = None) -> 'Validator':
        """Validate numeric value is within range."""
        if value is not None and (value < min_val or value > max_val):
            self.add_error(field, message or f'{field} must be between {min_val} and {max_val}')
        return self
    
    # Format validations
    def email(self, field: str, value: Optional[str], message: Optional[str] = None) -> 'Validator':
        """Validate email format."""
        if value is not None and not validate_email(value):
            self.add_error(field, message or f'{field} must be a valid email')
        return self
    
    def url(self, field: str, value: Optional[str], allowed_schemes: Optional[List[str]] = None,
            message: Optional[str] = None) -> 'Validator':
        """Validate URL format."""
        if value is not None and not validate_url(value, allowed_schemes):
            self.add_error(field, message or f'{field} must be a valid URL')
        return self
    
    def uuid(self, field: str, value: Optional[str], message: Optional[str] = None) -> 'Validator':
        """Validate UUID format."""
        if value is not None and not validate_uuid(value):
            self.add_error(field, message or f'{field} must be a valid UUID')
        return self
    
    def phone(self, field: str, value: Optional[str], message: Optional[str] = None) -> 'Validator':
        """Validate phone number format."""
        if value is not None and not validate_phone(value):
            self.add_error(field, message or f'{field} must be a valid phone number')
        return self
    
    def slug(self, field: str, value: Optional[str], message: Optional[str] = None) -> 'Validator':
        """Validate URL slug format."""
        if value is not None and not validate_slug(value):
            self.add_error(field, message or f'{field} must be a valid slug')
        return self
    
    # List validations
    def min_items(self, field: str, value: Optional[List], min_items: int,
                  message: Optional[str] = None) -> 'Validator':
        """Validate minimum list length."""
        if value is not None and len(value) < min_items:
            self.add_error(field, message or f'{field} must have at least {min_items} items')
        return self
    
    def max_items(self, field: str, value: Optional[List], max_items: int,
                  message: Optional[str] = None) -> 'Validator':
        """Validate maximum list length."""
        if value is not None and len(value) > max_items:
            self.add_error(field, message or f'{field} must have at most {max_items} items')
        return self
    
    # Custom validation
    def custom(self, field: str, value: Any, validator: Callable[[Any], bool],
               message: str) -> 'Validator':
        """Apply custom validation."""
        if not validator(value):
            self.add_error(field, message)
        return self
    
    def when(self, condition: bool, validation: Callable[['Validator'], 'Validator']) -> 'Validator':
        """Conditional validation."""
        if condition:
            return validation(self)
        return self


# ============================================
# Schema Validator (JSON Schema-like)
# ============================================

@dataclass
class SchemaValidator:
    """JSON Schema-like validator.
    
    Example:
        schema = {
            'type': 'object',
            'properties': {
                'name': {'type': 'string', 'minLength': 1, 'maxLength': 100},
                'age': {'type': 'integer', 'minimum': 0, 'maximum': 150},
                'email': {'type': 'string', 'format': 'email'},
            },
            'required': ['name', 'email'],
        }
        
        validator = SchemaValidator(schema)
        if not validator.validate(data):
            raise SchemaValidationError(validator.errors)
    """
    
    schema: Dict[str, Any]
    errors: List[Dict[str, Any]] = field(default_factory=list)
    
    def validate(self, data: Any) -> bool:
        """Validate data against the schema."""
        self.errors = []
        self._validate(data, self.schema, '$')
        return len(self.errors) == 0
    
    def _add_error(self, path: str, message: str, value: Any = None) -> None:
        """Add an error."""
        self.errors.append({
            'path': path,
            'message': message,
            'value': value,
        })
    
    def _validate(self, value: Any, schema: Dict[str, Any], path: str) -> None:
        """Recursively validate value against schema."""
        if not isinstance(schema, dict):
            return
        
        # Type validation
        schema_type = schema.get('type')
        if schema_type:
            if not self._validate_type(value, schema_type):
                self._add_error(path, f"Expected {schema_type}, got {type(value).__name__}", value)
                return
        
        # Handle each type
        if schema_type == 'object':
            self._validate_object(value, schema, path)
        elif schema_type == 'array':
            self._validate_array(value, schema, path)
        elif schema_type == 'string':
            self._validate_string(value, schema, path)
        elif schema_type in ('integer', 'number'):
            self._validate_number(value, schema, path)
    
    def _validate_type(self, value: Any, schema_type: str) -> bool:
        """Validate value type."""
        type_map = {
            'string': str,
            'integer': int,
            'number': (int, float),
            'boolean': bool,
            'array': list,
            'object': dict,
            'null': type(None),
        }
        
        expected = type_map.get(schema_type)
        if expected is None:
            return True
        
        if schema_type == 'integer':
            return isinstance(value, int) and not isinstance(value, bool)
        
        return isinstance(value, expected)
    
    def _validate_object(self, value: dict, schema: dict, path: str) -> None:
        """Validate object properties."""
        properties = schema.get('properties', {})
        required = schema.get('required', [])
        additional_props = schema.get('additionalProperties', True)
        
        # Check required properties
        for prop in required:
            if prop not in value:
                self._add_error(f"{path}.{prop}", "Required property missing")
        
        # Validate properties
        for prop, prop_schema in properties.items():
            if prop in value:
                self._validate(value[prop], prop_schema, f"{path}.{prop}")
        
        # Check additional properties
        if not additional_props:
            for prop in value:
                if prop not in properties:
                    self._add_error(f"{path}.{prop}", "Additional property not allowed")
    
    def _validate_array(self, value: list, schema: dict, path: str) -> None:
        """Validate array items."""
        min_items = schema.get('minItems')
        max_items = schema.get('maxItems')
        items_schema = schema.get('items')
        
        if min_items is not None and len(value) < min_items:
            self._add_error(path, f"Array must have at least {min_items} items")
        
        if max_items is not None and len(value) > max_items:
            self._add_error(path, f"Array must have at most {max_items} items")
        
        if items_schema:
            for i, item in enumerate(value):
                self._validate(item, items_schema, f"{path}[{i}]")
    
    def _validate_string(self, value: str, schema: dict, path: str) -> None:
        """Validate string constraints."""
        min_length = schema.get('minLength')
        max_length = schema.get('maxLength')
        pattern = schema.get('pattern')
        format_str = schema.get('format')
        enum = schema.get('enum')
        
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
    
    def _validate_number(self, value: Union[int, float], schema: dict, path: str) -> None:
        """Validate number constraints."""
        minimum = schema.get('minimum')
        maximum = schema.get('maximum')
        exclusive_minimum = schema.get('exclusiveMinimum')
        exclusive_maximum = schema.get('exclusiveMaximum')
        enum = schema.get('enum')
        
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
            'email': validate_email,
            'uri': validate_url,
            'url': validate_url,
            'uuid': validate_uuid,
            'phone': validate_phone,
            'slug': validate_slug,
            'date': lambda v: validate_datetime(v, '%Y-%m-%d'),
            'date-time': validate_datetime,
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
            # Validate positional arguments
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
    'ValidationError',
    'SchemaValidationError',
    
    # Sanitization
    'sanitize_string',
    'sanitize_html',
    'sanitize_sql',
    'sanitize_url',
    'sanitize_json',
    
    # Validators
    'validate_email',
    'validate_uuid',
    'validate_alphanumeric',
    'validate_username',
    'validate_phone',
    'validate_slug',
    'validate_url',
    'validate_integer',
    'validate_float',
    'validate_string',
    'validate_datetime',
    'validate_enum',
    'validate_list',
    'validate_dict',
    
    # Classes
    'Validator',
    'SchemaValidator',
    
    # Decorators
    'validated',
    
    # Patterns
    'EMAIL_PATTERN',
    'UUID_PATTERN',
    'ALPHANUMERIC_PATTERN',
    'USERNAME_PATTERN',
    'PHONE_PATTERN',
    'SLUG_PATTERN',
]
