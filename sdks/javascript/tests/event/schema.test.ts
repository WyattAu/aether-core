import {
  SchemaRegistry,
  SchemaValidationError,
} from '../../src/event/schema';
import { SchemaCompatibilityMode } from '../../src/event/types';
import type { SchemaDefinition } from '../../src/event/types';

function userCreatedSchema(): SchemaDefinition {
  return {
    name: 'UserCreated',
    version: '1.0.0',
    type: 'json',
    fields: [
      { name: 'userId', type: 'string', required: true },
      { name: 'email', type: 'string', required: true },
      { name: 'role', type: 'string', required: false, defaultValue: 'viewer' },
    ],
    schema: {
      type: 'object',
      properties: {
        userId: { type: 'string' },
        email: { type: 'string' },
        role: { type: 'string' },
      },
      required: ['userId', 'email'],
    },
  };
}

describe('SchemaRegistry', () => {
  let registry: SchemaRegistry;

  beforeEach(() => {
    registry = new SchemaRegistry();
  });

  describe('register', () => {
    it('registers a new schema', async () => {
      const schema = userCreatedSchema();
      const result = await registry.register(schema);
      expect(result.name).toBe('UserCreated');
      expect(result.version).toBe('1.0.0');
    });

    it('registers multiple versions of the same schema', async () => {
      await registry.register(userCreatedSchema());
      const v2 = {
        ...userCreatedSchema(),
        version: '1.1.0',
        fields: [
          ...userCreatedSchema().fields,
          { name: 'phone', type: 'string', required: false } as any,
        ],
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
            email: { type: 'string' },
            role: { type: 'string' },
            phone: { type: 'string' },
          },
          required: ['userId', 'email'],
        },
      };
      const result = await registry.register(v2);
      expect(result.version).toBe('1.1.0');

      const versions = await registry.getVersions('UserCreated');
      expect(versions).toEqual(['1.0.0', '1.1.0']);
    });

    it('throws on incompatible evolution (added required field)', async () => {
      await registry.register(userCreatedSchema());

      const incompatible = {
        ...userCreatedSchema(),
        version: '2.0.0',
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
            email: { type: 'string' },
            role: { type: 'string' },
          },
          required: ['userId', 'email', 'role'],
        },
      };

      await expect(registry.register(incompatible))
        .rejects.toThrow(SchemaValidationError);
    });

    it('throws on incompatible evolution (type change)', async () => {
      await registry.register(userCreatedSchema());

      const incompatible = {
        ...userCreatedSchema(),
        version: '2.0.0',
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'number' },
            email: { type: 'string' },
          },
          required: ['userId', 'email'],
        },
      };

      await expect(registry.register(incompatible))
        .rejects.toThrow(SchemaValidationError);
    });

    it('allows backward-compatible evolution (adding optional field)', async () => {
      await registry.register(userCreatedSchema());

      const compatible = {
        ...userCreatedSchema(),
        version: '1.1.0',
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
            email: { type: 'string' },
            role: { type: 'string' },
            phone: { type: 'string' },
          },
          required: ['userId', 'email'],
        },
      };

      const result = await registry.register(compatible);
      expect(result.version).toBe('1.1.0');
    });
  });

  describe('validate', () => {
    it('validates correct data', async () => {
      await registry.register(userCreatedSchema());
      const result = await registry.validate('UserCreated', {
        userId: '1',
        email: 'a@b.com',
      });
      expect(result).toBe(true);
    });

    it('validate does not mutate data for missing optional fields', async () => {
      await registry.register(userCreatedSchema());
      const data: Record<string, unknown> = { userId: '1', email: 'a@b.com' };
      const result = await registry.validate('UserCreated', data);
      expect(result).toBe(true);
      expect(data.role).toBeUndefined();
    });

    it('throws on missing required field', async () => {
      await registry.register(userCreatedSchema());
      await expect(registry.validate('UserCreated', { userId: '1' }))
        .rejects.toThrow(SchemaValidationError);
    });

    it('throws on wrong type', async () => {
      await registry.register(userCreatedSchema());
      await expect(registry.validate('UserCreated', { userId: '1', email: 123 }))
        .rejects.toThrow(SchemaValidationError);
    });

    it('throws when validating against non-existent schema', async () => {
      await expect(registry.validate('Nonexistent', {}))
        .rejects.toThrow(SchemaValidationError);
    });

    it('throws when data is not an object', async () => {
      await registry.register(userCreatedSchema());
      await expect(registry.validate('UserCreated', 'not-an-object'))
        .rejects.toThrow(SchemaValidationError);
    });

    it('throws when data is null', async () => {
      await registry.register(userCreatedSchema());
      await expect(registry.validate('UserCreated', null))
        .rejects.toThrow(SchemaValidationError);
    });

    it('throws when data is an array', async () => {
      await registry.register(userCreatedSchema());
      await expect(registry.validate('UserCreated', [1, 2, 3]))
        .rejects.toThrow(SchemaValidationError);
    });

    it('validates against specific version', async () => {
      await registry.register(userCreatedSchema());
      const result = await registry.validate('UserCreated', { userId: '1', email: 'a@b.com' }, '1.0.0');
      expect(result).toBe(true);
    });

    it('throws SchemaValidationError with fieldErrors', async () => {
      await registry.register(userCreatedSchema());
      try {
        await registry.validate('UserCreated', {});
      } catch (e) {
        expect(e).toBeInstanceOf(SchemaValidationError);
        const err = e as SchemaValidationError;
        expect(err.schemaName).toBe('UserCreated');
        expect(err.fieldErrors.length).toBeGreaterThan(0);
      }
    });
  });

  describe('getSchema', () => {
    it('returns registered schema', async () => {
      await registry.register(userCreatedSchema());
      const schema = await registry.getSchema('UserCreated');
      expect(schema?.name).toBe('UserCreated');
    });

    it('returns undefined for unknown schema', async () => {
      const schema = await registry.getSchema('Nonexistent');
      expect(schema).toBeUndefined();
    });

    it('returns specific version', async () => {
      await registry.register(userCreatedSchema());
      const schema = await registry.getSchema('UserCreated', '1.0.0');
      expect(schema?.version).toBe('1.0.0');
    });

    it('returns latest version when no version specified', async () => {
      await registry.register(userCreatedSchema());
      const v2 = {
        ...userCreatedSchema(),
        version: '2.0.0',
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
            email: { type: 'string' },
          },
          required: ['userId', 'email'],
        },
      };
      await registry.register(v2);

      const schema = await registry.getSchema('UserCreated');
      expect(schema?.version).toBe('2.0.0');
    });
  });

  describe('getVersions', () => {
    it('returns empty array for unknown schema', async () => {
      const versions = await registry.getVersions('Nonexistent');
      expect(versions).toEqual([]);
    });

    it('returns all versions', async () => {
      await registry.register(userCreatedSchema());
      await registry.register({
        ...userCreatedSchema(),
        version: '1.1.0',
        schema: userCreatedSchema().schema,
      });

      const versions = await registry.getVersions('UserCreated');
      expect(versions).toEqual(['1.0.0', '1.1.0']);
    });
  });

  describe('evolve', () => {
    it('evolves schema with backward compatibility', async () => {
      await registry.register(userCreatedSchema());

      const newSchema = {
        ...userCreatedSchema(),
        version: '1.1.0',
        fields: [
          ...userCreatedSchema().fields,
          { name: 'phone', type: 'string', required: false } as any,
        ],
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
            email: { type: 'string' },
            role: { type: 'string' },
            phone: { type: 'string' },
          },
          required: ['userId', 'email'],
        },
      };

      const evolved = await registry.evolve(newSchema, '1.1.0', SchemaCompatibilityMode.BACKWARD);
      expect(evolved.version).toBe('1.1.0');
    });

    it('evolve registers as new schema if none exist', async () => {
      const schema = userCreatedSchema();
      const evolved = await registry.evolve(schema, '1.0.0', SchemaCompatibilityMode.BACKWARD);
      expect(evolved.version).toBe('1.0.0');
    });

    it('evolve throws on FULL incompatibility', async () => {
      await registry.register(userCreatedSchema());

      const incompatible = {
        ...userCreatedSchema(),
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
            email: { type: 'string' },
          },
          required: ['userId', 'email', 'role'],
        },
      };

      await expect(registry.evolve(incompatible, '2.0.0', SchemaCompatibilityMode.FULL))
        .rejects.toThrow(SchemaValidationError);
    });

    it('evolve with FULL mode accepts fully compatible schemas', async () => {
      await registry.register(userCreatedSchema());

      const sameSchema = {
        ...userCreatedSchema(),
        version: '1.1.0',
      };

      const evolved = await registry.evolve(sameSchema, '1.1.0', SchemaCompatibilityMode.FULL);
      expect(evolved.version).toBe('1.1.0');
    });

    it('evolve with FORWARD mode accepts subset schemas', async () => {
      await registry.register(userCreatedSchema());

      const subset = {
        ...userCreatedSchema(),
        version: '1.1.0',
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
          },
          required: ['userId'],
        },
        fields: [{ name: 'userId', type: 'string', required: true }],
      };

      const evolved = await registry.evolve(subset, '1.1.0', SchemaCompatibilityMode.FORWARD);
      expect(evolved.version).toBe('1.1.0');
    });

    it('evolve with BACKWARD rejects forward-only compatible', async () => {
      await registry.register(userCreatedSchema());

      const subset = {
        ...userCreatedSchema(),
        version: '1.1.0',
        schema: {
          type: 'object',
          properties: {
            userId: { type: 'string' },
          },
          required: ['userId'],
        },
        fields: [{ name: 'userId', type: 'string', required: true }],
      };

      await expect(registry.evolve(subset, '1.1.0', SchemaCompatibilityMode.BACKWARD))
        .rejects.toThrow(SchemaValidationError);
    });
  });

  describe('SchemaValidationError', () => {
    it('has correct properties', () => {
      const err = new SchemaValidationError('test error', 'TestSchema', [
        { field: 'f1', message: 'missing' },
      ]);
      expect(err.name).toBe('SchemaValidationError');
      expect(err.schemaName).toBe('TestSchema');
      expect(err.fieldErrors).toHaveLength(1);
      expect(err.fieldErrors[0].field).toBe('f1');
    });

    it('has empty fieldErrors by default', () => {
      const err = new SchemaValidationError('msg', 'S');
      expect(err.fieldErrors).toEqual([]);
    });
  });
});
