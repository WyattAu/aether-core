/**
 * Aether SDK Validation Module
 *
 * Provides input validation, schema validation, and sanitization utilities
 * for building secure actor systems.
 *
 * @example
 * ```typescript
 * import { Validator, validateEmail, sanitizeString } from 'aether-sdk/validation';
 *
 * // Validate input
 * const validator = new Validator();
 * validator.required('name', name);
 * validator.email('email', email);
 * validator.minLength('password', password, 8);
 *
 * if (!validator.isValid()) {
 *   throw new Error(JSON.stringify(validator.getErrors()));
 * }
 *
 * // Sanitize input
 * const cleanName = sanitizeString(name, 100);
 * ```
 *
 * @module aether/validation
 */

// Types
export {
  ValidationError,
  ValidationErrors,
  SchemaDefinition,
  SchemaValidationError,
  SchemaValidationErrors,
  ValidationFn,
  CustomValidatorFn,
  SanitizeFn,
} from './types';

// Validators
export {
  // Patterns
  EMAIL_PATTERN,
  UUID_PATTERN,
  ALPHANUMERIC_PATTERN,
  USERNAME_PATTERN,
  PHONE_PATTERN,
  SLUG_PATTERN,
  IP_PATTERN,
  // Functions
  validateEmail,
  validateUUID,
  validateAlphanumeric,
  validateUsername,
  validatePhone,
  validateSlug,
  validateURL,
  validateIP,
  validateInteger,
  validateFloat,
  validateString,
  validateEnum,
  validateList,
  validateObject,
  validateRequired,
  validateDateTime,
  validateNoControlChars,
  // Class
  Validator,
} from './validators';

// Sanitizers
export {
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
  sanitizeAlphaNumeric,
  sanitizeSlug,
  redactSensitive,
  escapeRegex,
  escapeShell,
  normalizeLineEndings,
  stripHTML,
  truncate,
} from './sanitize';
