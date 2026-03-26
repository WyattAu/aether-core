/**
 * Validation Functions and Validator Class.
 *
 * Provides standalone validation functions for common patterns (email, UUID,
 * integer bounds, etc.) and a fluent {@link Validator} class for building
 * compound validation rules with field-level error messages.
 *
 * @module aether/validation/validators
 */

import { ValidationErrors, ValidationFn } from './types';

// ============================================
// Regex Patterns
// ============================================

/** Regular expression for validating email addresses. */
export const EMAIL_PATTERN = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
/** Regular expression for validating UUIDs (v1-v5). */
export const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
/** Regular expression for alphanumeric strings. */
export const ALPHANUMERIC_PATTERN = /^[a-zA-Z0-9]+$/;
/** Regular expression for username format (letters, numbers, underscore, hyphen). */
export const USERNAME_PATTERN = /^[a-zA-Z0-9_-]+$/;
/** Regular expression for E.164 phone numbers. */
export const PHONE_PATTERN = /^\+?[1-9]\d{1,14}$/;
/** Regular expression for URL slugs (lowercase, hyphens). */
export const SLUG_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
/** Regular expression for IPv4 addresses. */
export const IP_PATTERN = /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/;

// ============================================
// Standalone Validation Functions
// ============================================

/**
 * Validate an email address.
 *
 * @param email - The email string to validate.
 * @returns `true` if the email matches the expected format.
 */
export function validateEmail(email: string): boolean {
  if (!email || typeof email !== 'string') return false;
  return EMAIL_PATTERN.test(email);
}

/**
 * Validate a UUID string (v1-v5).
 *
 * @param uuid - The UUID string to validate.
 * @returns `true` if the UUID matches the expected format.
 */
export function validateUUID(uuid: string): boolean {
  if (!uuid || typeof uuid !== 'string') return false;
  return UUID_PATTERN.test(uuid);
}

/**
 * Validate that a string is entirely alphanumeric.
 *
 * @param value - The string to validate.
 * @returns `true` if the string contains only letters and digits.
 */
export function validateAlphanumeric(value: string): boolean {
  if (!value || typeof value !== 'string') return false;
  return ALPHANUMERIC_PATTERN.test(value);
}

/**
 * Validate a username format (letters, digits, underscore, hyphen).
 *
 * @param username - The username to validate.
 * @returns `true` if the username matches the expected format.
 */
export function validateUsername(username: string): boolean {
  if (!username || typeof username !== 'string') return false;
  return USERNAME_PATTERN.test(username);
}

/**
 * Validate a phone number in E.164 format.
 *
 * @param phone - The phone number to validate.
 * @returns `true` if the phone number matches E.164.
 */
export function validatePhone(phone: string): boolean {
  if (!phone || typeof phone !== 'string') return false;
  return PHONE_PATTERN.test(phone);
}

/**
 * Validate a URL slug (lowercase alphanumeric with hyphens).
 *
 * @param slug - The slug to validate.
 * @returns `true` if the slug matches the expected format.
 */
export function validateSlug(slug: string): boolean {
  if (!slug || typeof slug !== 'string') return false;
  return SLUG_PATTERN.test(slug);
}

/**
 * Validate a URL with optional scheme restriction.
 *
 * @param url            - The URL string to validate.
 * @param allowedSchemes - Array of allowed URL schemes (default: `['http', 'https']`).
 * @returns `true` if the URL is valid and uses an allowed scheme.
 */
export function validateURL(url: string, allowedSchemes?: string[]): boolean {
  if (!url || typeof url !== 'string') return false;

  try {
    const parsed = new URL(url);
    const schemes = allowedSchemes ?? ['http', 'https'];

    if (!schemes.includes(parsed.protocol.replace(':', ''))) {
      return false;
    }

    if (!parsed.host) {
      return false;
    }

    return true;
  } catch {
    return false;
  }
}

/**
 * Validate an IPv4 address.
 *
 * @param ip - The IP address string to validate.
 * @returns `true` if the string is a valid IPv4 address.
 */
export function validateIP(ip: string): boolean {
  if (!ip || typeof ip !== 'string') return false;
  return IP_PATTERN.test(ip);
}

/**
 * Validate that a value is an integer within optional bounds.
 *
 * @param value - The value to validate.
 * @param min   - Optional minimum (inclusive).
 * @param max   - Optional maximum (inclusive).
 * @returns `true` if the value is an integer within bounds.
 */
export function validateInteger(
  value: unknown,
  min?: number,
  max?: number
): boolean {
  if (typeof value !== 'number' || !Number.isInteger(value)) {
    return false;
  }

  if (min !== undefined && value < min) {
    return false;
  }

  if (max !== undefined && value > max) {
    return false;
  }

  return true;
}

/**
 * Validate that a value is a finite number within optional bounds.
 *
 * @param value - The value to validate.
 * @param min   - Optional minimum (inclusive).
 * @param max   - Optional maximum (inclusive).
 * @returns `true` if the value is a valid number within bounds.
 */
export function validateFloat(
  value: unknown,
  min?: number,
  max?: number
): boolean {
  if (typeof value !== 'number' || isNaN(value)) {
    return false;
  }

  if (min !== undefined && value < min) {
    return false;
  }

  if (max !== undefined && value > max) {
    return false;
  }

  return true;
}

/**
 * Validate a string with optional length and pattern constraints.
 *
 * @param value     - The value to validate.
 * @param minLength - Optional minimum length.
 * @param maxLength - Optional maximum length.
 * @param pattern   - Optional regex pattern the string must match.
 * @returns `true` if the string satisfies all constraints.
 */
export function validateString(
  value: unknown,
  minLength?: number,
  maxLength?: number,
  pattern?: RegExp
): boolean {
  if (typeof value !== 'string') {
    return false;
  }

  if (minLength !== undefined && value.length < minLength) {
    return false;
  }

  if (maxLength !== undefined && value.length > maxLength) {
    return false;
  }

  if (pattern && !pattern.test(value)) {
    return false;
  }

  return true;
}

/**
 * Validate that a value is one of a set of allowed enum values.
 *
 * @typeParam T - The type of the allowed values.
 * @param value   - The value to check.
 * @param allowed - Array of allowed values.
 * @returns Type guard that narrows the type to `T`.
 */
export function validateEnum<T>(value: unknown, allowed: T[]): value is T {
  return allowed.includes(value as T);
}

/**
 * Validate an array with optional length and item constraints.
 *
 * @param value          - The value to validate.
 * @param minLength      - Optional minimum number of items.
 * @param maxLength      - Optional maximum number of items.
 * @param itemValidator  - Optional validator applied to each item.
 * @returns `true` if the value is an array satisfying all constraints.
 */
export function validateList(
  value: unknown,
  minLength?: number,
  maxLength?: number,
  itemValidator?: ValidationFn
): boolean {
  if (!Array.isArray(value)) {
    return false;
  }

  if (minLength !== undefined && value.length < minLength) {
    return false;
  }

  if (maxLength !== undefined && value.length > maxLength) {
    return false;
  }

  if (itemValidator) {
    return value.every(itemValidator);
  }

  return true;
}

/**
 * Validate that a value is a plain object with optional key constraints.
 *
 * @param value         - The value to validate.
 * @param requiredKeys  - Keys that must be present.
 * @param optionalKeys  - Keys that may be present (if specified, no extra keys allowed).
 * @returns `true` if the value is an object satisfying the key constraints.
 */
export function validateObject(
  value: unknown,
  requiredKeys?: string[],
  optionalKeys?: string[]
): boolean {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  const obj = value as Record<string, unknown>;

  // Check required keys
  if (requiredKeys) {
    for (const key of requiredKeys) {
      if (!(key in obj)) {
        return false;
      }
    }
  }

  // Check no extra keys if both required and optional specified
  if (requiredKeys && optionalKeys) {
    const allowedKeys = new Set([...requiredKeys, ...optionalKeys]);
    for (const key of Object.keys(obj)) {
      if (!allowedKeys.has(key)) {
        return false;
      }
    }
  }

  return true;
}

/**
 * Validate that a value is present (not null, undefined, or empty).
 *
 * Returns `false` for empty strings, empty arrays, and empty objects.
 *
 * @param value - The value to validate.
 * @returns `true` if the value is present and non-empty.
 */
export function validateRequired(value: unknown): boolean {
  if (value === null || value === undefined) {
    return false;
  }

  if (typeof value === 'string' && value.trim() === '') {
    return false;
  }

  if (Array.isArray(value) && value.length === 0) {
    return false;
  }

  if (typeof value === 'object' && Object.keys(value).length === 0) {
    return false;
  }

  return true;
}

/**
 * Validate a datetime string.
 *
 * Attempts to parse the string as a Date. If a format is provided,
 * it is currently ignored (future extension point).
 *
 * @param value  - The datetime string.
 * @param format - Reserved for future use; currently ignored.
 * @returns `true` if the string represents a valid date.
 */
export function validateDateTime(value: string, format?: string): boolean {
  if (!value || typeof value !== 'string') return false;

  try {
    if (format) {
      // Custom format parsing would go here
      // For now, just check ISO format
      const date = new Date(value);
      return !isNaN(date.getTime());
    } else {
      const date = new Date(value);
      return !isNaN(date.getTime());
    }
  } catch {
    return false;
  }
}

/**
 * Validate that a string contains no control characters (except \n, \r, \t).
 *
 * @param value - The string to validate.
 * @returns `true` if no disallowed control characters are present.
 */
export function validateNoControlChars(value: string): boolean {
  for (const char of value) {
    const code = char.charCodeAt(0);
    if ((code < 32 || code === 127) && char !== '\n' && char !== '\r' && char !== '\t') {
      return false;
    }
  }
  return true;
}

// ============================================
// Validator Class
// ============================================

/**
 * Fluent validator for building compound validation rules.
 *
 * Accumulates field-level errors and provides a chainable API for
 * defining multiple rules in sequence.
 *
 * @example
 * ```typescript
 * const validator = new Validator();
 * validator.required('name', name);
 * validator.email('email', email);
 * validator.minLength('password', password, 8);
 * validator.when(role === 'admin', v => v.required('permissions', perms));
 *
 * if (!validator.isValid()) {
 *   throw new Error(JSON.stringify(validator.getErrors()));
 * }
 * ```
 */
export class Validator {
  private errors: Map<string, string[]> = new Map();

  /**
   * Add an error message for a field.
   *
   * @param field   - The field name.
   * @param message - The error message.
   * @returns This validator for chaining.
   */
  addError(field: string, message: string): this {
    if (!this.errors.has(field)) {
      this.errors.set(field, []);
    }
    this.errors.get(field)!.push(message);
    return this;
  }

  /**
   * Check if all validations have passed (no errors).
   *
   * @returns `true` if no errors have been recorded.
   */
  isValid(): boolean {
    return this.errors.size === 0;
  }

  /**
   * Clear all recorded errors.
   *
   * @returns This validator for chaining.
   */
  clear(): this {
    this.errors.clear();
    return this;
  }

  /**
   * Get all recorded errors as a field-keyed map.
   *
   * @returns An object mapping field names to arrays of error messages.
   */
  getErrors(): ValidationErrors {
    const result: ValidationErrors = {};
    for (const [field, messages] of this.errors) {
      result[field] = messages;
    }
    return result;
  }

  /**
   * Validate that a field is present and non-empty.
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  required(field: string, value: unknown, message?: string): this {
    if (!validateRequired(value)) {
      this.addError(field, message ?? `${field} is required`);
    }
    return this;
  }

  /**
   * Validate that a field value is a string (when present).
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  string(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null && typeof value !== 'string') {
      this.addError(field, message ?? `${field} must be a string`);
    }
    return this;
  }

  /**
   * Validate that a field value is an integer (when present).
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  integer(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null) {
      if (typeof value !== 'number' || !Number.isInteger(value)) {
        this.addError(field, message ?? `${field} must be an integer`);
      }
    }
    return this;
  }

  /**
   * Validate that a field value is a number (when present).
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  float(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null) {
      if (typeof value !== 'number' || isNaN(value)) {
        this.addError(field, message ?? `${field} must be a number`);
      }
    }
    return this;
  }

  /**
   * Validate that a field value is a boolean (when present).
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  boolean(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null && typeof value !== 'boolean') {
      this.addError(field, message ?? `${field} must be a boolean`);
    }
    return this;
  }

  /**
   * Validate that a field value is an array (when present).
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  array(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null && !Array.isArray(value)) {
      this.addError(field, message ?? `${field} must be an array`);
    }
    return this;
  }

  /**
   * Validate that a field value is a plain object (when present).
   *
   * @param field   - The field name.
   * @param value   - The value to validate.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  object(field: string, value: unknown, message?: string): this {
    if (
      value !== undefined &&
      value !== null &&
      (typeof value !== 'object' || Array.isArray(value))
    ) {
      this.addError(field, message ?? `${field} must be an object`);
    }
    return this;
  }

  /**
   * Validate that a string field meets a minimum length.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param minLen  - Minimum allowed length.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  minLength(field: string, value: string | undefined, minLen: number, message?: string): this {
    if (value !== undefined && value.length < minLen) {
      this.addError(field, message ?? `${field} must be at least ${minLen} characters`);
    }
    return this;
  }

  /**
   * Validate that a string field does not exceed a maximum length.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param maxLen  - Maximum allowed length.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  maxLength(field: string, value: string | undefined, maxLen: number, message?: string): this {
    if (value !== undefined && value.length > maxLen) {
      this.addError(field, message ?? `${field} must be at most ${maxLen} characters`);
    }
    return this;
  }

  /**
   * Validate that a string field matches a regex pattern.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param regex   - The pattern to match.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  pattern(field: string, value: string | undefined, regex: RegExp, message?: string): this {
    if (value !== undefined && !regex.test(value)) {
      this.addError(field, message ?? `${field} has invalid format`);
    }
    return this;
  }

  /**
   * Validate that a numeric field meets a minimum value.
   *
   * @param field   - The field name.
   * @param value   - The numeric value.
   * @param minVal  - Minimum allowed value (inclusive).
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  minValue(field: string, value: number | undefined, minVal: number, message?: string): this {
    if (value !== undefined && value < minVal) {
      this.addError(field, message ?? `${field} must be at least ${minVal}`);
    }
    return this;
  }

  /**
   * Validate that a numeric field does not exceed a maximum value.
   *
   * @param field   - The field name.
   * @param value   - The numeric value.
   * @param maxVal  - Maximum allowed value (inclusive).
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  maxValue(field: string, value: number | undefined, maxVal: number, message?: string): this {
    if (value !== undefined && value > maxVal) {
      this.addError(field, message ?? `${field} must be at most ${maxVal}`);
    }
    return this;
  }

  /**
   * Validate that a numeric field is within a range.
   *
   * @param field   - The field name.
   * @param value   - The numeric value.
   * @param minVal  - Minimum allowed value (inclusive).
   * @param maxVal  - Maximum allowed value (inclusive).
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  range(
    field: string,
    value: number | undefined,
    minVal: number,
    maxVal: number,
    message?: string
  ): this {
    if (value !== undefined && (value < minVal || value > maxVal)) {
      this.addError(field, message ?? `${field} must be between ${minVal} and ${maxVal}`);
    }
    return this;
  }

  /**
   * Validate that a field value is a valid email address.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  email(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validateEmail(value)) {
      this.addError(field, message ?? `${field} must be a valid email`);
    }
    return this;
  }

  /**
   * Validate that a field value is a valid URL.
   *
   * @param field          - The field name.
   * @param value          - The string value.
   * @param allowedSchemes - Optional allowed URL schemes.
   * @param message        - Optional custom error message.
   * @returns This validator for chaining.
   */
  url(field: string, value: string | undefined, allowedSchemes?: string[], message?: string): this {
    if (value !== undefined && !validateURL(value, allowedSchemes)) {
      this.addError(field, message ?? `${field} must be a valid URL`);
    }
    return this;
  }

  /**
   * Validate that a field value is a valid UUID.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  uuid(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validateUUID(value)) {
      this.addError(field, message ?? `${field} must be a valid UUID`);
    }
    return this;
  }

  /**
   * Validate that a field value is a valid phone number.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  phone(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validatePhone(value)) {
      this.addError(field, message ?? `${field} must be a valid phone number`);
    }
    return this;
  }

  /**
   * Validate that a field value is a valid URL slug.
   *
   * @param field   - The field name.
   * @param value   - The string value.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  slug(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validateSlug(value)) {
      this.addError(field, message ?? `${field} must be a valid slug`);
    }
    return this;
  }

  /**
   * Validate that an array field has a minimum number of items.
   *
   * @param field    - The field name.
   * @param value    - The array value.
   * @param minItems - Minimum allowed item count.
   * @param message  - Optional custom error message.
   * @returns This validator for chaining.
   */
  minItems(field: string, value: unknown[] | undefined, minItems: number, message?: string): this {
    if (value !== undefined && value.length < minItems) {
      this.addError(field, message ?? `${field} must have at least ${minItems} items`);
    }
    return this;
  }

  /**
   * Validate that an array field has at most a maximum number of items.
   *
   * @param field    - The field name.
   * @param value    - The array value.
   * @param maxItems - Maximum allowed item count.
   * @param message  - Optional custom error message.
   * @returns This validator for chaining.
   */
  maxItems(field: string, value: unknown[] | undefined, maxItems: number, message?: string): this {
    if (value !== undefined && value.length > maxItems) {
      this.addError(field, message ?? `${field} must have at most ${maxItems} items`);
    }
    return this;
  }

  /**
   * Validate that a field value is one of a set of allowed enum values.
   *
   * @typeParam T - The type of allowed values.
   * @param field   - The field name.
   * @param value   - The value to check.
   * @param allowed - Array of allowed values.
   * @param message - Optional custom error message.
   * @returns This validator for chaining.
   */
  enum<T>(field: string, value: unknown, allowed: T[], message?: string): this {
    if (!allowed.includes(value as T)) {
      this.addError(field, message ?? `${field} must be one of the allowed values`);
    }
    return this;
  }

  /**
   * Apply a custom validation function.
   *
   * @param field     - The field name.
   * @param value     - The value to validate.
   * @param validator - A function returning `true` if valid.
   * @param message   - The error message if validation fails.
   * @returns This validator for chaining.
   */
  custom(
    field: string,
    value: unknown,
    validator: ValidationFn,
    message: string
  ): this {
    if (!validator(value)) {
      this.addError(field, message);
    }
    return this;
  }

  /**
   * Conditionally apply validations.
   *
   * @param condition - If `true`, the validation function is invoked.
   * @param fn        - Function receiving this validator for chaining.
   * @returns This validator for chaining.
   *
   * @example
   * ```typescript
   * validator.when(user.role === 'admin', v => {
   *   v.required('permissions', user.permissions);
   * });
   * ```
   */
  when(condition: boolean, fn: (v: Validator) => void): this {
    if (condition) {
      fn(this);
    }
    return this;
  }
}
