/**
 * Common Types for the Validation Module.
 *
 * Defines interfaces and type aliases for validation errors, schema definitions,
 * and validator function signatures used throughout the validation system.
 *
 * @module aether/validation/types
 */

/**
 * Describes a single validation error for a specific field.
 */
export interface ValidationError {
  /** The field name that failed validation. */
  field: string;
  /** Human-readable error message. */
  message: string;
  /** The invalid value (useful for debugging). */
  value?: unknown;
}

/**
 * Collection of validation errors keyed by field name.
 *
 * Each field maps to an array of error messages.
 */
export interface ValidationErrors {
  [field: string]: string[];
}

/**
 * Schema definition for declarative validation.
 *
 * Inspired by JSON Schema, this interface describes constraints on
 * data structures for validation purposes.
 */
export interface SchemaDefinition {
  /** Expected type of the value. */
  type?: 'string' | 'number' | 'integer' | 'boolean' | 'array' | 'object' | 'null';
  /** Property definitions for object types. */
  properties?: Record<string, SchemaDefinition>;
  /** List of required property names for object types. */
  required?: string[];
  /** Whether additional properties beyond those defined are allowed. */
  additionalProperties?: boolean;
  /** Schema for array item types. */
  items?: SchemaDefinition;
  /** Minimum string length or minimum array items. */
  minLength?: number;
  /** Maximum string length or maximum array items. */
  maxLength?: number;
  /** Minimum numeric value (inclusive). */
  minimum?: number;
  /** Maximum numeric value (inclusive). */
  maximum?: number;
  /** Exclusive minimum numeric value. */
  exclusiveMinimum?: number;
  /** Exclusive maximum numeric value. */
  exclusiveMaximum?: number;
  /** Regex pattern the string must match. */
  pattern?: string;
  /** Semantic format hint (e.g., `'email'`, `'uuid'`, `'date-time'`). */
  format?: string;
  /** Enumerated list of allowed values. */
  enum?: unknown[];
  /** Minimum number of array items. */
  minItems?: number;
  /** Maximum number of array items. */
  maxItems?: number;
  /** Human-readable description of the schema. */
  description?: string;
  /** Default value if the field is absent. */
  default?: unknown;
}

/**
 * Schema validation error with a path indicating where the error occurred.
 */
export interface SchemaValidationError {
  /** Dot-separated path to the invalid field (e.g., `'user.address.zip'`). */
  path: string;
  /** Human-readable error message. */
  message: string;
  /** The invalid value (useful for debugging). */
  value?: unknown;
}

/**
 * Collection of schema validation errors.
 */
export interface SchemaValidationErrors {
  /** Array of individual validation errors. */
  errors: SchemaValidationError[];
}

/**
 * Function type for standalone validation checks.
 *
 * @param value - The value to validate.
 * @returns `true` if the value is valid.
 */
export type ValidationFn = (value: unknown) => boolean;

/**
 * Function type for custom validators that integrate with the {@link Validator} class.
 *
 * @param value     - The value to validate.
 * @param field     - The field name being validated.
 * @param validator - The Validator instance for adding errors.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type CustomValidatorFn = (value: unknown, field: string, validator: any) => void;

/**
 * Function type for sanitization operations.
 *
 * @typeParam T - The input/output type (typically `string`).
 * @param value - The value to sanitize.
 * @returns The sanitized value.
 */
export type SanitizeFn<T = string> = (value: T) => T;
