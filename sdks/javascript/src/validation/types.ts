/**
 * Common types for the validation module.
 * @module aether/validation/types
 */

/**
 * Validation error information.
 */
export interface ValidationError {
  field: string;
  message: string;
  value?: unknown;
}

/**
 * Collection of validation errors.
 */
export interface ValidationErrors {
  [field: string]: string[];
}

/**
 * Schema definition for validation.
 */
export interface SchemaDefinition {
  type?: 'string' | 'number' | 'integer' | 'boolean' | 'array' | 'object' | 'null';
  properties?: Record<string, SchemaDefinition>;
  required?: string[];
  additionalProperties?: boolean;
  items?: SchemaDefinition;
  minLength?: number;
  maxLength?: number;
  minimum?: number;
  maximum?: number;
  exclusiveMinimum?: number;
  exclusiveMaximum?: number;
  pattern?: string;
  format?: string;
  enum?: unknown[];
  minItems?: number;
  maxItems?: number;
  description?: string;
  default?: unknown;
}

/**
 * Schema validation error with path.
 */
export interface SchemaValidationError {
  path: string;
  message: string;
  value?: unknown;
}

/**
 * Collection of schema validation errors.
 */
export interface SchemaValidationErrors {
  errors: SchemaValidationError[];
}

/**
 * Validation function type.
 */
export type ValidationFn = (value: unknown) => boolean;

/**
 * Custom validator function type.
 */
export type CustomValidatorFn = (value: unknown, field: string, validator: Validator) => void;

/**
 * Sanitization function type.
 */
export type SanitizeFn<T = string> = (value: T) => T;
