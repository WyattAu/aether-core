/**
 * Validation functions and Validator class.
 * @module aether/validation/validators
 */

import { ValidationErrors, ValidationFn } from './types';

// ============================================
// Regex Patterns
// ============================================

export const EMAIL_PATTERN = /^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
export const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const ALPHANUMERIC_PATTERN = /^[a-zA-Z0-9]+$/;
export const USERNAME_PATTERN = /^[a-zA-Z0-9_-]+$/;
export const PHONE_PATTERN = /^\+?[1-9]\d{1,14}$/;
export const SLUG_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const IP_PATTERN = /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/;

// ============================================
// Standalone Validation Functions
// ============================================

/**
 * Validate an email address.
 */
export function validateEmail(email: string): boolean {
  if (!email || typeof email !== 'string') return false;
  return EMAIL_PATTERN.test(email);
}

/**
 * Validate a UUID.
 */
export function validateUUID(uuid: string): boolean {
  if (!uuid || typeof uuid !== 'string') return false;
  return UUID_PATTERN.test(uuid);
}

/**
 * Validate alphanumeric string.
 */
export function validateAlphanumeric(value: string): boolean {
  if (!value || typeof value !== 'string') return false;
  return ALPHANUMERIC_PATTERN.test(value);
}

/**
 * Validate username format.
 */
export function validateUsername(username: string): boolean {
  if (!username || typeof username !== 'string') return false;
  return USERNAME_PATTERN.test(username);
}

/**
 * Validate phone number (E.164 format).
 */
export function validatePhone(phone: string): boolean {
  if (!phone || typeof phone !== 'string') return false;
  return PHONE_PATTERN.test(phone);
}

/**
 * Validate URL slug.
 */
export function validateSlug(slug: string): boolean {
  if (!slug || typeof slug !== 'string') return false;
  return SLUG_PATTERN.test(slug);
}

/**
 * Validate URL with optional allowed schemes.
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
 * Validate IP address.
 */
export function validateIP(ip: string): boolean {
  if (!ip || typeof ip !== 'string') return false;
  return IP_PATTERN.test(ip);
}

/**
 * Validate integer with optional bounds.
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
 * Validate float with optional bounds.
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
 * Validate string with length and pattern constraints.
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
 * Validate enum value.
 */
export function validateEnum<T>(value: unknown, allowed: T[]): value is T {
  return allowed.includes(value as T);
}

/**
 * Validate list/array with length constraints.
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
 * Validate object/dictionary.
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
 * Validate required field (not null/undefined/empty).
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
 * Validate datetime string.
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
 * Validate no control characters.
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
 * Fluent validator for building validation rules.
 *
 * @example
 * ```typescript
 * const validator = new Validator();
 * validator.required('name', name);
 * validator.email('email', email);
 * validator.minLength('password', password, 8);
 *
 * if (!validator.isValid()) {
 *   throw new Error(JSON.stringify(validator.errors));
 * }
 * ```
 */
export class Validator {
  private errors: Map<string, string[]> = new Map();

  /**
   * Add an error for a field.
   */
  addError(field: string, message: string): this {
    if (!this.errors.has(field)) {
      this.errors.set(field, []);
    }
    this.errors.get(field)!.push(message);
    return this;
  }

  /**
   * Check if all validations passed.
   */
  isValid(): boolean {
    return this.errors.size === 0;
  }

  /**
   * Clear all errors.
   */
  clear(): this {
    this.errors.clear();
    return this;
  }

  /**
   * Get all errors.
   */
  getErrors(): ValidationErrors {
    const result: ValidationErrors = {};
    for (const [field, messages] of this.errors) {
      result[field] = messages;
    }
    return result;
  }

  // Required validation
  required(field: string, value: unknown, message?: string): this {
    if (!validateRequired(value)) {
      this.addError(field, message ?? `${field} is required`);
    }
    return this;
  }

  // Type validations
  string(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null && typeof value !== 'string') {
      this.addError(field, message ?? `${field} must be a string`);
    }
    return this;
  }

  integer(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null) {
      if (typeof value !== 'number' || !Number.isInteger(value)) {
        this.addError(field, message ?? `${field} must be an integer`);
      }
    }
    return this;
  }

  float(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null) {
      if (typeof value !== 'number' || isNaN(value)) {
        this.addError(field, message ?? `${field} must be a number`);
      }
    }
    return this;
  }

  boolean(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null && typeof value !== 'boolean') {
      this.addError(field, message ?? `${field} must be a boolean`);
    }
    return this;
  }

  array(field: string, value: unknown, message?: string): this {
    if (value !== undefined && value !== null && !Array.isArray(value)) {
      this.addError(field, message ?? `${field} must be an array`);
    }
    return this;
  }

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

  // String validations
  minLength(field: string, value: string | undefined, minLen: number, message?: string): this {
    if (value !== undefined && value.length < minLen) {
      this.addError(field, message ?? `${field} must be at least ${minLen} characters`);
    }
    return this;
  }

  maxLength(field: string, value: string | undefined, maxLen: number, message?: string): this {
    if (value !== undefined && value.length > maxLen) {
      this.addError(field, message ?? `${field} must be at most ${maxLen} characters`);
    }
    return this;
  }

  pattern(field: string, value: string | undefined, regex: RegExp, message?: string): this {
    if (value !== undefined && !regex.test(value)) {
      this.addError(field, message ?? `${field} has invalid format`);
    }
    return this;
  }

  // Numeric validations
  minValue(field: string, value: number | undefined, minVal: number, message?: string): this {
    if (value !== undefined && value < minVal) {
      this.addError(field, message ?? `${field} must be at least ${minVal}`);
    }
    return this;
  }

  maxValue(field: string, value: number | undefined, maxVal: number, message?: string): this {
    if (value !== undefined && value > maxVal) {
      this.addError(field, message ?? `${field} must be at most ${maxVal}`);
    }
    return this;
  }

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

  // Format validations
  email(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validateEmail(value)) {
      this.addError(field, message ?? `${field} must be a valid email`);
    }
    return this;
  }

  url(field: string, value: string | undefined, allowedSchemes?: string[], message?: string): this {
    if (value !== undefined && !validateURL(value, allowedSchemes)) {
      this.addError(field, message ?? `${field} must be a valid URL`);
    }
    return this;
  }

  uuid(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validateUUID(value)) {
      this.addError(field, message ?? `${field} must be a valid UUID`);
    }
    return this;
  }

  phone(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validatePhone(value)) {
      this.addError(field, message ?? `${field} must be a valid phone number`);
    }
    return this;
  }

  slug(field: string, value: string | undefined, message?: string): this {
    if (value !== undefined && !validateSlug(value)) {
      this.addError(field, message ?? `${field} must be a valid slug`);
    }
    return this;
  }

  // List validations
  minItems(field: string, value: unknown[] | undefined, minItems: number, message?: string): this {
    if (value !== undefined && value.length < minItems) {
      this.addError(field, message ?? `${field} must have at least ${minItems} items`);
    }
    return this;
  }

  maxItems(field: string, value: unknown[] | undefined, maxItems: number, message?: string): this {
    if (value !== undefined && value.length > maxItems) {
      this.addError(field, message ?? `${field} must have at most ${maxItems} items`);
    }
    return this;
  }

  // Enum validation
  enum<T>(field: string, value: unknown, allowed: T[], message?: string): this {
    if (!allowed.includes(value as T)) {
      this.addError(field, message ?? `${field} must be one of the allowed values`);
    }
    return this;
  }

  // Custom validation
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

  // Conditional validation
  when(condition: boolean, fn: (v: Validator) => void): this {
    if (condition) {
      fn(this);
    }
    return this;
  }
}
