import { StateHandle } from '../src';

describe('StateHandle', () => {
    let state: StateHandle;

    beforeEach(() => {
        state = new StateHandle();
    });

    test('should set and get buffer values', async () => {
        const buffer = Buffer.from('test data', 'utf8');
        await state.set('key1', buffer);
        const result = await state.get('key1');
        expect(result?.toString('utf8')).toBe('test data');
    });

    test('should set and get string values', async () => {
        await state.setString('key2', 'hello world');
        const result = await state.getString('key2');
        expect(result).toBe('hello world');
    });

    test('should set and get JSON values', async () => {
        const data = { name: 'test', value: 42 };
        await state.setJSON('key3', data);
        const result = await state.getJSON<{ name: string; value: number }>('key3');
        expect(result).toEqual(data);
    });

    test('should delete values', async () => {
        await state.set('key4', Buffer.from('data'));
        await state.delete('key4');
        const result = await state.get('key4');
        expect(result).toBeUndefined();
    });

    test('should list keys with prefix', async () => {
        await state.setString('user:1', 'alice');
        await state.setString('user:2', 'bob');
        await state.setString('config:theme', 'dark');
        const keys = await state.list('user:');
        expect(keys).toHaveLength(2);
        expect(keys).toContain('user:1');
        expect(keys).toContain('user:2');
    });

    test('should return undefined for missing keys', async () => {
        const result = await state.get('nonexistent');
        expect(result).toBeUndefined();
    });
});
