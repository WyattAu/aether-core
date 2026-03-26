/**
 * Tests for Validation Functions and Validator Class
 */

import {
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
} from '../../src/validation/validators';

describe('Regex Patterns', () => {
  describe('EMAIL_PATTERN', () => {
    test('matches valid emails', () => {
      expect(EMAIL_PATTERN.test('user@example.com')).toBe(true);
      expect(EMAIL_PATTERN.test('user.name@example.com')).toBe(true);
      expect(EMAIL_PATTERN.test('user+tag@example.org')).toBe(true);
    });

    test('rejects invalid emails', () => {
      expect(EMAIL_PATTERN.test('invalid')).toBe(false);
      expect(EMAIL_PATTERN.test('user@')).toBe(false);
      expect(EMAIL_PATTERN.test('@example.com')).toBe(false);
    });
  });

  describe('UUID_PATTERN', () => {
    test('matches valid UUIDs', () => {
      expect(UUID_PATTERN.test('123e4567-e89b-12d3-a456-426614174000')).toBe(true);
      expect(UUID_PATTERN.test('00000000-0000-0000-0000-000000000000')).toBe(true);
    });

    test('rejects invalid UUIDs', () => {
      expect(UUID_PATTERN.test('not-a-uuid')).toBe(false);
      expect(UUID_PATTERN.test('123e4567-e89b-12d3-a456')).toBe(false);
    });
  });

  describe('ALPHANUMERIC_PATTERN', () => {
    test('matches alphanumeric strings', () => {
      expect(ALPHANUMERIC_PATTERN.test('abc123')).toBe(true);
      expect(ALPHANUMERIC_PATTERN.test('ABC')).toBe(true);
      expect(ALPHANUMERIC_PATTERN.test('123')).toBe(true);
    });

    test('rejects strings with special characters', () => {
      expect(ALPHANUMERIC_PATTERN.test('abc-123')).toBe(false);
      expect(ALPHANUMERIC_PATTERN.test('test@example')).toBe(false);
    });
  });

  describe('USERNAME_PATTERN', () => {
    test('matches valid usernames', () => {
      expect(USERNAME_PATTERN.test('user123')).toBe(true);
      expect(USERNAME_PATTERN.test('user_name')).toBe(true);
      expect(USERNAME_PATTERN.test('USER-NAME')).toBe(true);
    });

    test('rejects invalid usernames', () => {
      expect(USERNAME_PATTERN.test('user@name')).toBe(false);
      expect(USERNAME_PATTERN.test('user name')).toBe(false);
    });
  });

  describe('PHONE_PATTERN', () => {
    test('matches valid phone numbers (E.164)', () => {
      expect(PHONE_PATTERN.test('+1234567890')).toBe(true);
      expect(PHONE_PATTERN.test('+14155552671')).toBe(true);
    });

    test('rejects invalid phone numbers', () => {
      expect(PHONE_PATTERN.test('not-a-phone')).toBe(false);
      expect(PHONE_PATTERN.test('')).toBe(false);
      expect(PHONE_PATTERN.test('0123')).toBe(false); // starts with 0
    });
  });

  describe('SLUG_PATTERN', () => {
    test('matches valid slugs', () => {
      expect(SLUG_PATTERN.test('my-post')).toBe(true);
      expect(SLUG_PATTERN.test('my-post-123')).toBe(true);
      expect(SLUG_PATTERN.test('simple')).toBe(true);
    });

    test('rejects invalid slugs', () => {
      expect(SLUG_PATTERN.test('My-Post')).toBe(false);
      expect(SLUG_PATTERN.test('my_post')).toBe(false);
      expect(SLUG_PATTERN.test('-my-post')).toBe(false);
    });
  });

  describe('IP_PATTERN', () => {
    test('matches valid IPv4 addresses', () => {
      expect(IP_PATTERN.test('192.168.1.1')).toBe(true);
      expect(IP_PATTERN.test('0.0.0.0')).toBe(true);
      expect(IP_PATTERN.test('255.255.255.255')).toBe(true);
    });

    test('rejects invalid IP addresses', () => {
      expect(IP_PATTERN.test('256.1.1.1')).toBe(false);
      expect(IP_PATTERN.test('192.168.1')).toBe(false);
      expect(IP_PATTERN.test('not-an-ip')).toBe(false);
    });
  });
});

describe('Validation Functions', () => {
  describe('validateEmail', () => {
    test('returns true for valid emails', () => {
      expect(validateEmail('user@example.com')).toBe(true);
      expect(validateEmail('test.user@domain.org')).toBe(true);
    });

    test('returns false for invalid emails', () => {
      expect(validateEmail('invalid')).toBe(false);
      expect(validateEmail('')).toBe(false);
      expect(validateEmail(null as any)).toBe(false);
    });
  });

  describe('validateUUID', () => {
    test('returns true for valid UUIDs', () => {
      expect(validateUUID('123e4567-e89b-12d3-a456-426614174000')).toBe(true);
    });

    test('returns false for invalid UUIDs', () => {
      expect(validateUUID('not-a-uuid')).toBe(false);
      expect(validateUUID('')).toBe(false);
    });
  });

  describe('validateAlphanumeric', () => {
    test('returns true for alphanumeric strings', () => {
      expect(validateAlphanumeric('abc123')).toBe(true);
      expect(validateAlphanumeric('ABCDEF')).toBe(true);
    });

    test('returns false for non-alphanumeric strings', () => {
      expect(validateAlphanumeric('abc-123')).toBe(false);
      expect(validateAlphanumeric('')).toBe(false);
    });
  });

  describe('validateUsername', () => {
    test('returns true for valid usernames', () => {
      expect(validateUsername('user123')).toBe(true);
      expect(validateUsername('user_name')).toBe(true);
    });

    test('returns false for invalid usernames', () => {
      expect(validateUsername('user@name')).toBe(false);
      expect(validateUsername('')).toBe(false);
    });
  });

  describe('validatePhone', () => {
    test('returns true for valid phone numbers', () => {
      expect(validatePhone('+14155552671')).toBe(true);
    });

    test('returns false for invalid phone numbers', () => {
      expect(validatePhone('not-a-phone')).toBe(false);
      expect(validatePhone('')).toBe(false);
      expect(validatePhone('0123')).toBe(false); // starts with 0
    });
  });

  describe('validateSlug', () => {
    test('returns true for valid slugs', () => {
      expect(validateSlug('my-post')).toBe(true);
      expect(validateSlug('post-123')).toBe(true);
    });

    test('returns false for invalid slugs', () => {
      expect(validateSlug('My-Post')).toBe(false);
      expect(validateSlug('')).toBe(false);
    });
  });

  describe('validateURL', () => {
    test('returns true for valid URLs', () => {
      expect(validateURL('https://example.com')).toBe(true);
      expect(validateURL('http://example.com/path')).toBe(true);
    });

    test('returns false for invalid URLs', () => {
      expect(validateURL('not-a-url')).toBe(false);
      expect(validateURL('')).toBe(false);
    });

    test('respects allowed schemes', () => {
      expect(validateURL('https://example.com', ['https'])).toBe(true);
      expect(validateURL('http://example.com', ['https'])).toBe(false);
      expect(validateURL('ftp://example.com', ['http', 'https'])).toBe(false);
    });
  });

  describe('validateIP', () => {
    test('returns true for valid IPv4 addresses', () => {
      expect(validateIP('192.168.1.1')).toBe(true);
      expect(validateIP('10.0.0.1')).toBe(true);
    });

    test('returns false for invalid IP addresses', () => {
      expect(validateIP('256.1.1.1')).toBe(false);
      expect(validateIP('')).toBe(false);
    });
  });

  describe('validateInteger', () => {
    test('returns true for valid integers', () => {
      expect(validateInteger(42)).toBe(true);
      expect(validateInteger(0)).toBe(true);
      expect(validateInteger(-10)).toBe(true);
    });

    test('returns false for non-integers', () => {
      expect(validateInteger(3.14)).toBe(false);
      expect(validateInteger('42')).toBe(false);
      expect(validateInteger(null)).toBe(false);
    });

    test('respects min/max bounds', () => {
      expect(validateInteger(5, 0, 10)).toBe(true);
      expect(validateInteger(-1, 0, 10)).toBe(false);
      expect(validateInteger(11, 0, 10)).toBe(false);
    });
  });

  describe('validateFloat', () => {
    test('returns true for valid floats', () => {
      expect(validateFloat(3.14)).toBe(true);
      expect(validateFloat(42)).toBe(true);
      expect(validateFloat(-1.5)).toBe(true);
    });

    test('returns false for non-numbers', () => {
      expect(validateFloat(NaN)).toBe(false);
      expect(validateFloat('3.14')).toBe(false);
      expect(validateFloat(null)).toBe(false);
    });

    test('respects min/max bounds', () => {
      expect(validateFloat(5.5, 0, 10)).toBe(true);
      expect(validateFloat(-0.5, 0, 10)).toBe(false);
      expect(validateFloat(10.5, 0, 10)).toBe(false);
    });
  });

  describe('validateString', () => {
    test('returns true for valid strings', () => {
      expect(validateString('hello')).toBe(true);
      expect(validateString('')).toBe(true);
    });

    test('returns false for non-strings', () => {
      expect(validateString(42)).toBe(false);
      expect(validateString(null)).toBe(false);
    });

    test('respects length constraints', () => {
      expect(validateString('hello', 1, 10)).toBe(true);
      expect(validateString('hi', 5, 10)).toBe(false);
      expect(validateString('very long string', 1, 5)).toBe(false);
    });

    test('respects pattern constraint', () => {
      expect(validateString('abc123', undefined, undefined, /^[a-z0-9]+$/)).toBe(true);
      expect(validateString('abc!', undefined, undefined, /^[a-z0-9]+$/)).toBe(false);
    });
  });

  describe('validateEnum', () => {
    test('returns true for values in enum', () => {
      expect(validateEnum('a', ['a', 'b', 'c'])).toBe(true);
      expect(validateEnum(1, [1, 2, 3])).toBe(true);
    });

    test('returns false for values not in enum', () => {
      expect(validateEnum('d', ['a', 'b', 'c'])).toBe(false);
      expect(validateEnum(4, [1, 2, 3])).toBe(false);
    });
  });

  describe('validateList', () => {
    test('returns true for valid arrays', () => {
      expect(validateList([1, 2, 3])).toBe(true);
      expect(validateList([])).toBe(true);
    });

    test('returns false for non-arrays', () => {
      expect(validateList('not-an-array')).toBe(false);
      expect(validateList(null)).toBe(false);
    });

    test('respects length constraints', () => {
      expect(validateList([1, 2], 1, 5)).toBe(true);
      expect(validateList([], 1, 5)).toBe(false);
      expect(validateList([1, 2, 3, 4, 5, 6], 1, 5)).toBe(false);
    });

    test('validates items with validator', () => {
      expect(validateList([1, 2, 3], undefined, undefined, (x: unknown) => (x as number) > 0)).toBe(true);
      expect(validateList([1, -1, 3], undefined, undefined, (x: unknown) => (x as number) > 0)).toBe(false);
    });
  });

  describe('validateObject', () => {
    test('returns true for valid objects', () => {
      expect(validateObject({})).toBe(true);
      expect(validateObject({ a: 1 })).toBe(true);
    });

    test('returns false for non-objects', () => {
      expect(validateObject(null)).toBe(false);
      expect(validateObject([1, 2, 3])).toBe(false);
      expect(validateObject('string')).toBe(false);
    });

    test('checks required keys', () => {
      expect(validateObject({ a: 1, b: 2 }, ['a', 'b'])).toBe(true);
      expect(validateObject({ a: 1 }, ['a', 'b'])).toBe(false);
    });

    test('checks for extra keys when both required and optional specified', () => {
      expect(validateObject({ a: 1, b: 2 }, ['a'], ['b'])).toBe(true);
      expect(validateObject({ a: 1, b: 2, c: 3 }, ['a'], ['b'])).toBe(false);
    });
  });

  describe('validateRequired', () => {
    test('returns true for truthy values', () => {
      expect(validateRequired('value')).toBe(true);
      expect(validateRequired(0)).toBe(true);
      expect(validateRequired(false)).toBe(true);
    });

    test('returns false for null/undefined', () => {
      expect(validateRequired(null)).toBe(false);
      expect(validateRequired(undefined)).toBe(false);
    });

    test('returns false for empty containers', () => {
      expect(validateRequired('')).toBe(false);
      expect(validateRequired([])).toBe(false);
      expect(validateRequired({})).toBe(false);
    });
  });

  describe('validateDateTime', () => {
    test('returns true for valid ISO dates', () => {
      expect(validateDateTime('2024-01-15T10:30:00Z')).toBe(true);
      expect(validateDateTime('2024-01-15')).toBe(true);
    });

    test('returns false for invalid dates', () => {
      expect(validateDateTime('not-a-date')).toBe(false);
      expect(validateDateTime('')).toBe(false);
    });
  });

  describe('validateNoControlChars', () => {
    test('returns true for strings without control chars', () => {
      expect(validateNoControlChars('normal string')).toBe(true);
      expect(validateNoControlChars('with\nnewline')).toBe(true);
      expect(validateNoControlChars('with\ttab')).toBe(true);
    });

    test('returns false for strings with control chars', () => {
      expect(validateNoControlChars('with\x00null')).toBe(false);
      expect(validateNoControlChars('with\x1besc')).toBe(false);
    });
  });
});

describe('Validator Class', () => {
  let validator: Validator;

  beforeEach(() => {
    validator = new Validator();
  });

  describe('addError', () => {
    test('adds error for field', () => {
      validator.addError('field', 'error message');

      expect(validator.isValid()).toBe(false);
      expect(validator.getErrors()).toEqual({ field: ['error message'] });
    });

    test('adds multiple errors for same field', () => {
      validator.addError('field', 'error 1');
      validator.addError('field', 'error 2');

      expect(validator.getErrors()).toEqual({ field: ['error 1', 'error 2'] });
    });

    test('returns this for chaining', () => {
      const result = validator.addError('field', 'error');

      expect(result).toBe(validator);
    });
  });

  describe('isValid', () => {
    test('returns true when no errors', () => {
      expect(validator.isValid()).toBe(true);
    });

    test('returns false when errors exist', () => {
      validator.addError('field', 'error');

      expect(validator.isValid()).toBe(false);
    });
  });

  describe('clear', () => {
    test('clears all errors', () => {
      validator.addError('field', 'error');
      validator.clear();

      expect(validator.isValid()).toBe(true);
    });

    test('returns this for chaining', () => {
      const result = validator.clear();

      expect(result).toBe(validator);
    });
  });

  describe('getErrors', () => {
    test('returns empty object when no errors', () => {
      expect(validator.getErrors()).toEqual({});
    });

    test('returns all errors', () => {
      validator.addError('field1', 'error1');
      validator.addError('field2', 'error2');

      const errors = validator.getErrors();

      expect(errors.field1).toContain('error1');
      expect(errors.field2).toContain('error2');
    });
  });

  describe('required', () => {
    test('adds error for missing value', () => {
      validator.required('name', null);

      expect(validator.isValid()).toBe(false);
      expect(validator.getErrors().name).toContain('name is required');
    });

    test('uses custom message', () => {
      validator.required('name', null, 'Name is mandatory');

      expect(validator.getErrors().name).toContain('Name is mandatory');
    });

    test('passes for present value', () => {
      validator.required('name', 'John');

      expect(validator.isValid()).toBe(true);
    });
  });

  describe('type validations', () => {
    test('string validation', () => {
      validator.string('field', 'text');
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.string('field', 123);
      expect(validator.isValid()).toBe(false);
    });

    test('integer validation', () => {
      validator.integer('field', 42);
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.integer('field', 3.14);
      expect(validator.isValid()).toBe(false);
    });

    test('float validation', () => {
      validator.float('field', 3.14);
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.float('field', 'not a number');
      expect(validator.isValid()).toBe(false);
    });

    test('boolean validation', () => {
      validator.boolean('field', true);
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.boolean('field', 'true');
      expect(validator.isValid()).toBe(false);
    });

    test('array validation', () => {
      validator.array('field', [1, 2, 3]);
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.array('field', 'not an array');
      expect(validator.isValid()).toBe(false);
    });

    test('object validation', () => {
      validator.object('field', { a: 1 });
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.object('field', [1, 2, 3]);
      expect(validator.isValid()).toBe(false);
    });
  });

  describe('string validations', () => {
    test('minLength', () => {
      validator.minLength('field', 'ab', 3);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.minLength('field', 'abc', 3);
      expect(validator.isValid()).toBe(true);
    });

    test('maxLength', () => {
      validator.maxLength('field', 'abcd', 3);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.maxLength('field', 'abc', 3);
      expect(validator.isValid()).toBe(true);
    });

    test('pattern', () => {
      validator.pattern('field', 'abc123', /^[a-z]+$/);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.pattern('field', 'abc', /^[a-z]+$/);
      expect(validator.isValid()).toBe(true);
    });
  });

  describe('numeric validations', () => {
    test('minValue', () => {
      validator.minValue('field', 5, 10);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.minValue('field', 15, 10);
      expect(validator.isValid()).toBe(true);
    });

    test('maxValue', () => {
      validator.maxValue('field', 15, 10);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.maxValue('field', 5, 10);
      expect(validator.isValid()).toBe(true);
    });

    test('range', () => {
      validator.range('field', 5, 10, 20);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.range('field', 25, 10, 20);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.range('field', 15, 10, 20);
      expect(validator.isValid()).toBe(true);
    });
  });

  describe('format validations', () => {
    test('email', () => {
      validator.email('field', 'invalid');
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.email('field', 'user@example.com');
      expect(validator.isValid()).toBe(true);
    });

    test('url', () => {
      validator.url('field', 'invalid');
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.url('field', 'https://example.com');
      expect(validator.isValid()).toBe(true);
    });

    test('uuid', () => {
      validator.uuid('field', 'invalid');
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.uuid('field', '123e4567-e89b-12d3-a456-426614174000');
      expect(validator.isValid()).toBe(true);
    });

    test('phone', () => {
      validator.phone('field', 'invalid');
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.phone('field', '+14155552671');
      expect(validator.isValid()).toBe(true);
    });

    test('slug', () => {
      validator.slug('field', 'Invalid Slug');
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.slug('field', 'valid-slug');
      expect(validator.isValid()).toBe(true);
    });
  });

  describe('list validations', () => {
    test('minItems', () => {
      validator.minItems('field', [1], 2);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.minItems('field', [1, 2], 2);
      expect(validator.isValid()).toBe(true);
    });

    test('maxItems', () => {
      validator.maxItems('field', [1, 2, 3], 2);
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.maxItems('field', [1], 2);
      expect(validator.isValid()).toBe(true);
    });
  });

  describe('enum validation', () => {
    test('validates against allowed values', () => {
      validator.enum('field', 'a', ['a', 'b', 'c']);
      expect(validator.isValid()).toBe(true);

      validator.clear();
      validator.enum('field', 'd', ['a', 'b', 'c']);
      expect(validator.isValid()).toBe(false);
    });
  });

  describe('custom validation', () => {
    test('uses custom validator function', () => {
      validator.custom('field', 5, (x: unknown) => (x as number) > 10, 'Must be greater than 10');
      expect(validator.isValid()).toBe(false);

      validator.clear();
      validator.custom('field', 15, (x: unknown) => (x as number) > 10, 'Must be greater than 10');
      expect(validator.isValid()).toBe(true);
    });
  });

  describe('conditional validation', () => {
    test('when condition is true', () => {
      validator.when(true, (v) => {
        v.required('field', null);
      });
      expect(validator.isValid()).toBe(false);
    });

    test('when condition is false', () => {
      validator.when(false, (v) => {
        v.required('field', null);
      });
      expect(validator.isValid()).toBe(true);
    });
  });

  describe('method chaining', () => {
    test('all methods return this for chaining', () => {
      const result = validator
        .required('name', 'John')
        .email('email', 'user@example.com')
        .minLength('password', 'password123', 8);

      expect(result).toBe(validator);
      expect(validator.isValid()).toBe(true);
    });

    test('chained validations accumulate errors', () => {
      validator
        .required('name', null)
        .email('email', 'invalid')
        .minLength('password', 'short', 8);

      expect(validator.isValid()).toBe(false);
      expect(Object.keys(validator.getErrors())).toHaveLength(3);
    });
  });
});
