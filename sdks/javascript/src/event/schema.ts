/**
 * Schema Registry for Event Validation
 *
 * Provides schema registration, validation, compatibility checking, and
 * evolution support for event schemas.
 *
 * @example
 * ```typescript
 * import { SchemaRegistry, SchemaCompatibilityMode } from 'aether-sdk/event';
 *
 * const registry = new SchemaRegistry();
 *
 * await registry.register({
 *   name: 'UserCreated',
 *   version: '1.0.0',
 *   type: 'json',
 *   fields: [
 *     { name: 'userId', type: 'string', required: true },
 *     { name: 'email', type: 'string', required: true },
 *   ],
 *   schema: {
 *     type: 'object',
 *     properties: {
 *       userId: { type: 'string' },
 *       email: { type: 'string' },
 *     },
 *     required: ['userId', 'email'],
 *   },
 * });
 *
 * await registry.validate('UserCreated', { userId: '1', email: 'a@b.com' });
 * ```
 *
 * @module aether/event/schema
 */

import {
  SchemaDefinition,
  SchemaField,
  SchemaCompatibilityMode,
} from './types';

/** @internal */
function generateId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

/**
 * Thrown when schema validation fails.
 */
export class SchemaValidationError extends Error {
  /** Schema name that failed validation. */
  public readonly schemaName: string;
  /** Field-level validation errors. */
  public readonly fieldErrors: FieldValidationError[];

  /**
   * @param message     - Human-readable error summary.
   * @param schemaName  - Name of the schema.
   * @param fieldErrors - Individual field validation failures.
   */
  constructor(
    message: string,
    schemaName: string,
    fieldErrors: FieldValidationError[] = [],
  ) {
    super(message);
    this.name = 'SchemaValidationError';
    this.schemaName = schemaName;
    this.fieldErrors = fieldErrors;
  }
}

/**
 * A single field-level validation error.
 */
export interface FieldValidationError {
  /** Field name that failed validation. */
  field: string;
  /** Human-readable error description. */
  message: string;
  /** Expected type or constraint. */
  expected?: string;
  /** Actual value received. */
  actual?: unknown;
}

/**
 * Schema compatibility check result.
 */
export interface CompatibilityResult {
  /** Whether the schemas are compatible under the requested mode. */
  compatible: boolean;
  /** Detected compatibility level. */
  mode: SchemaCompatibilityMode;
  /** Human-readable description of changes. */
  description: string;
  /** List of specific compatibility issues (empty if compatible). */
  issues: string[];
}

/**
 * Internal versioned schema entry in the registry.
 *
 * @internal
 */
interface VersionedSchema {
  /** Schema ID (UUID). */
  schemaId: string;
  /** Schema definition. */
  schema: SchemaDefinition;
  /** When this version was registered. */
  createdAt: Date;
  /** Whether this version is deprecated. */
  deprecated: boolean;
  /** Compatibility mode enforced at registration time. */
  compatibility: SchemaCompatibilityMode;
}

/**
 * JSON Schema type mapping to JavaScript runtime types.
 *
 * @internal
 */
const JSON_TYPE_MAP: Record<string, (v: unknown) => boolean> = {
  string: (v) => typeof v === 'string',
  number: (v) => typeof v === 'number',
  integer: (v) => typeof v === 'number' && Number.isInteger(v),
  boolean: (v) => typeof v === 'boolean',
  array: (v) => Array.isArray(v),
  object: (v) => typeof v === 'object' && v !== null && !Array.isArray(v),
  null: (v) => v === null,
};

/**
 * Schema registry for managing event schema definitions.
 *
 * Supports registration, validation, compatibility checking, and
 * schema evolution with configurable compatibility modes.
 *
 * @example
 * ```typescript
 * const registry = new SchemaRegistry();
 *
 * // Register initial schema
 * await registry.register({
 *   name: 'OrderCreated',
 *   version: '1.0.0',
 *   type: 'json',
 *   fields: [
 *     { name: 'orderId', type: 'string', required: true },
 *     { name: 'total', type: 'number', required: true },
 *   ],
 *   schema: {
 *     type: 'object',
 *     properties: {
 *       orderId: { type: 'string' },
 *       total: { type: 'number' },
 *     },
 *     required: ['orderId', 'total'],
 *   },
 * });
 *
 * // Validate an event
 * try {
 *   await registry.validate('OrderCreated', { orderId: '123', total: 99.99 });
 * } catch (e) {
 *   if (e instanceof SchemaValidationError) {
 *     console.error(e.fieldErrors);
 *   }
 * }
 *
 * // Evolve the schema
 * const evolved = await registry.evolve(
 *   {
 *     name: 'OrderCreated',
 *     version: '1.1.0',
 *     type: 'json',
 *     fields: [
 *       { name: 'orderId', type: 'string', required: true },
 *       { name: 'total', type: 'number', required: true },
 *       { name: 'currency', type: 'string', required: false, defaultValue: 'USD' },
 *     ],
 *     schema: {
 *       type: 'object',
 *       properties: {
 *         orderId: { type: 'string' },
 *         total: { type: 'number' },
 *         currency: { type: 'string' },
 *       },
 *       required: ['orderId', 'total'],
 *     },
 *   },
 *   '1.1.0',
 *   SchemaCompatibilityMode.BACKWARD,
 * );
 * ```
 */
export class SchemaRegistry {
  private readonly schemas = new Map<string, VersionedSchema[]>();

  /**
   * Register a new schema definition.
   *
   * If a schema with the same name already exists, this registers a new
   * version. The first registration always succeeds. Subsequent registrations
   * are checked for compatibility unless the mode is `NONE`.
   *
   * @param schema - The schema definition to register.
   * @returns The registered schema definition (with assigned version).
   * @throws {SchemaValidationError} If the new schema is incompatible.
   */
  async register(schema: SchemaDefinition): Promise<SchemaDefinition> {
    const name = schema.name;
    let versions = this.schemas.get(name);

    if (!versions) {
      versions = [];
      this.schemas.set(name, versions);
    }

    if (versions.length > 0) {
      const last = versions[versions.length - 1];
      const result = this.checkCompatibility(
        last.schema.schema,
        schema.schema,
        last.schema.fields,
        schema.fields,
      );

      if (!result.compatible && last.compatibility !== SchemaCompatibilityMode.NONE) {
        throw new SchemaValidationError(
          `Schema '${name}' evolution is not compatible: ${result.description}`,
          name,
          result.issues.map((issue) => ({
            field: '',
            message: issue,
          })),
        );
      }
    }

    const versionedSchema: VersionedSchema = {
      schemaId: generateId(),
      schema,
      createdAt: new Date(),
      deprecated: false,
      compatibility: SchemaCompatibilityMode.BACKWARD,
    };

    versions.push(versionedSchema);
    return schema;
  }

  /**
   * Validate data against a registered schema.
   *
   * Performs required-field checks, type checks, and applies default values
   * for missing optional fields.
   *
   * @param eventName - Schema name to validate against.
   * @param data      - Data to validate.
   * @param version   - Optional specific schema version (defaults to latest).
   * @returns `true` if validation passes.
   * @throws {SchemaValidationError} If validation fails or schema not found.
   */
  async validate(
    eventName: string,
    data: unknown,
    version?: string,
  ): Promise<boolean> {
    const schema = this.getSchemaInternal(eventName, version);
    if (!schema) {
      throw new SchemaValidationError(
        `Schema not found: ${eventName}`,
        eventName,
      );
    }

    const errors = this.validateData(data, {
      schemaId: '',
      schema,
      createdAt: new Date(),
      deprecated: false,
      compatibility: SchemaCompatibilityMode.BACKWARD,
    });
    if (errors.length > 0) {
      throw new SchemaValidationError(
        `Validation failed for '${eventName}': ${errors.map((e) => e.message).join('; ')}`,
        eventName,
        errors,
      );
    }

    return true;
  }

  /**
   * Get a schema definition by name and optional version.
   *
   * @param eventName - Schema name.
   * @param version   - Optional version string (latest if omitted).
   * @returns The schema definition, or `undefined` if not found.
   */
  async getSchema(
    eventName: string,
    version?: string,
  ): Promise<SchemaDefinition | undefined> {
    return this.getSchemaInternal(eventName, version);
  }

  /**
   * Evolve a schema to a new version with compatibility enforcement.
   *
   * Validates that the new schema is compatible with the latest version
   * under the specified compatibility mode, then registers it.
   *
   * @param schema             - New schema definition.
   * @param newVersion         - Target version string.
   * @param compatibilityMode  - Compatibility mode to enforce.
   * @returns The evolved schema definition.
   * @throws {SchemaValidationError} If the schemas are not compatible.
   */
  async evolve(
    schema: SchemaDefinition,
    newVersion: string,
    compatibilityMode: SchemaCompatibilityMode,
  ): Promise<SchemaDefinition> {
    const name = schema.name;
    const versions = this.schemas.get(name);

    if (!versions || versions.length === 0) {
      return this.register(schema);
    }

    const last = versions[versions.length - 1];
    const result = this.checkCompatibility(
      last.schema.schema,
      schema.schema,
      last.schema.fields,
      schema.fields,
    );

    if (compatibilityMode === SchemaCompatibilityMode.FULL) {
      if (result.mode !== SchemaCompatibilityMode.FULL) {
        throw new SchemaValidationError(
          `Schema '${name}' is not fully compatible: ${result.description}`,
          name,
          result.issues.map((issue) => ({ field: '', message: issue })),
        );
      }
    } else if (compatibilityMode === SchemaCompatibilityMode.BACKWARD) {
      if (
        result.mode !== SchemaCompatibilityMode.BACKWARD &&
        result.mode !== SchemaCompatibilityMode.FULL
      ) {
        throw new SchemaValidationError(
          `Schema '${name}' is not backward compatible: ${result.description}`,
          name,
          result.issues.map((issue) => ({ field: '', message: issue })),
        );
      }
    } else if (compatibilityMode === SchemaCompatibilityMode.FORWARD) {
      if (
        result.mode !== SchemaCompatibilityMode.FORWARD &&
        result.mode !== SchemaCompatibilityMode.FULL
      ) {
        throw new SchemaValidationError(
          `Schema '${name}' is not forward compatible: ${result.description}`,
          name,
          result.issues.map((issue) => ({ field: '', message: issue })),
        );
      }
    }

    const evolved: SchemaDefinition = {
      ...schema,
      version: newVersion,
    };

    const versionedSchema: VersionedSchema = {
      schemaId: generateId(),
      schema: evolved,
      createdAt: new Date(),
      deprecated: false,
      compatibility: compatibilityMode,
    };

    versions.push(versionedSchema);
    return evolved;
  }

  /**
   * Get all registered versions for a schema.
   *
   * @param eventName - Schema name.
   * @returns Array of version strings.
   */
  async getVersions(eventName: string): Promise<string[]> {
    const versions = this.schemas.get(eventName);
    if (!versions) {
      return [];
    }
    return versions.map((v) => v.schema.version);
  }

  /**
   * Check compatibility between two JSON Schema definitions.
   *
   * @param oldSchema    - Old schema definition object.
   * @param newSchema    - New schema definition object.
   * @param oldFields    - Old field definitions.
   * @param newFields    - New field definitions.
   * @returns Compatibility check result.
   */
  private checkCompatibility(
    oldSchema: Record<string, unknown>,
    newSchema: Record<string, unknown>,
    oldFields: SchemaField[],
    newFields: SchemaField[],
  ): CompatibilityResult {
    const issues: string[] = [];

    const oldDef = oldSchema as Record<string, unknown>;
    const newDef = newSchema as Record<string, unknown>;

    const oldProps = (oldDef.properties as Record<string, unknown>) ?? {};
    const newProps = (newDef.properties as Record<string, unknown>) ?? {};

    const oldRequired = new Set(
      (oldDef.required as string[]) ?? [],
    );
    const newRequired = new Set(
      (newDef.required as string[]) ?? [],
    );

    const oldFieldMap = new Map(oldFields.map((f) => [f.name, f]));
    const newFieldMap = new Map(newFields.map((f) => [f.name, f]));

    const oldPropNames = new Set(Object.keys(oldProps));
    const newPropNames = new Set(Object.keys(newProps));

    const addedRequired = [...newRequired].filter((f) => !oldRequired.has(f));
    if (addedRequired.length > 0) {
      issues.push(`Added required fields: ${addedRequired.join(', ')}`);
    }

    for (const [name, oldProp] of Object.entries(oldProps)) {
      const newProp = newProps[name];
      if (newProp) {
        const oldType = (oldProp as Record<string, unknown>).type;
        const newType = (newProp as Record<string, unknown>).type;
        if (oldType !== newType) {
          issues.push(`Field '${name}' type changed from ${oldType} to ${newType}`);
        }
      }
    }

    const removedFields = [...oldPropNames].filter((f) => !newPropNames.has(f));
    const addedFields = [...newPropNames].filter((f) => !oldPropNames.has(f));

    const hasTypeChanges = issues.some((i) => i.includes('type changed'));
    const hasBreakingChanges = addedRequired.length > 0 || hasTypeChanges;

    if (hasBreakingChanges) {
      return {
        compatible: false,
        mode: SchemaCompatibilityMode.NONE,
        description: 'Breaking changes detected',
        issues,
      };
    }

    const isSubset = removedFields.length > 0 && addedFields.length === 0;
    const isSuperset = addedFields.length > 0 && removedFields.length === 0;

    if (isSuperset) {
      return {
        compatible: true,
        mode: SchemaCompatibilityMode.BACKWARD,
        description: 'New schema is a superset (backward compatible)',
        issues,
      };
    }

    if (isSubset) {
      return {
        compatible: true,
        mode: SchemaCompatibilityMode.FORWARD,
        description: 'New schema is a subset (forward compatible)',
        issues,
      };
    }

    if (addedFields.length === 0 && removedFields.length === 0) {
      return {
        compatible: true,
        mode: SchemaCompatibilityMode.FULL,
        description: 'No field changes (fully compatible)',
        issues,
      };
    }

    return {
      compatible: false,
      mode: SchemaCompatibilityMode.NONE,
      description: 'Mixed field additions and removals',
      issues: [...issues, 'Both fields added and removed'],
    };
  }

  /**
   * Validate data against a schema definition.
   *
   * @param data   - Data to validate.
   * @param schema - Schema definition to validate against.
   * @returns Array of field validation errors (empty if valid).
   */
  private validateData(
    data: unknown,
    schema: VersionedSchema,
  ): FieldValidationError[] {
    const errors: FieldValidationError[] = [];

    if (typeof data !== 'object' || data === null || Array.isArray(data)) {
      errors.push({
        field: '',
        message: 'Expected object, received non-object value',
        expected: 'object',
        actual: data,
      });
      return errors;
    }

    const record = data as Record<string, unknown>;
    const schemaDef = schema.schema.schema as unknown as Record<string, unknown>;
    const properties = (schemaDef.properties as Record<string, unknown>) ?? {};
    const requiredFields = new Set(
      (schemaDef.required as string[]) ?? [],
    );

    for (const field of schema.schema.fields) {
      if (field.required && !(field.name in record)) {
        if (field.defaultValue !== undefined) {
          record[field.name] = field.defaultValue;
        } else {
          errors.push({
            field: field.name,
            message: `Missing required field '${field.name}'`,
            expected: field.type,
            actual: undefined,
          });
        }
      }
    }

    for (const required of requiredFields) {
      if (!(required in record)) {
        const fieldDef = schema.schema.fields.find((f) => f.name === required);
        if (fieldDef?.defaultValue !== undefined) {
          record[required] = fieldDef.defaultValue;
        } else {
          errors.push({
            field: required,
            message: `Missing required field '${required}'`,
            expected: 'present',
            actual: undefined,
          });
        }
      }
    }

    for (const [propName, propDef] of Object.entries(properties)) {
      if (!(propName in record)) {
        continue;
      }

      const propRecord = propDef as Record<string, unknown>;
      const expectedType = propRecord.type as string | undefined;

      if (expectedType) {
        const checker = JSON_TYPE_MAP[expectedType];
        if (checker && !checker(record[propName])) {
          errors.push({
            field: propName,
            message: `Field '${propName}' has wrong type: expected ${expectedType}`,
            expected: expectedType,
            actual: typeof record[propName],
          });
        }
      }
    }

    return errors;
  }

  /**
   * Internal helper to retrieve a schema by name and version.
   *
   * @param eventName - Schema name.
   * @param version   - Optional version string.
   * @returns Schema definition or `undefined`.
   */
  private getSchemaInternal(
    eventName: string,
    version?: string,
  ): SchemaDefinition | undefined {
    const versions = this.schemas.get(eventName);
    if (!versions || versions.length === 0) {
      return undefined;
    }

    if (version) {
      const match = versions.find((v) => v.schema.version === version);
      return match?.schema;
    }

    return versions[versions.length - 1].schema;
  }
}
