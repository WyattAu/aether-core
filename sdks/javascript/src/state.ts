/**
 * Actor State Management.
 *
 * Provides the {@link StateHandle} class for managing per-actor state with
 * typed accessors for raw buffers, UTF-8 strings, and JSON objects.
 *
 * @module aether/state
 */

/**
 * In-memory key-value store for actor state.
 *
 * Each actor owns a `StateHandle` instance that provides asynchronous
 * access to string-keyed state entries. Data is stored internally as
 * `Buffer` values, with convenience methods for strings and JSON.
 *
 * @example
 * ```typescript
 * const state = new StateHandle();
 *
 * // Store a JSON object
 * await state.setJSON('config', { theme: 'dark', lang: 'en' });
 *
 * // Retrieve it later
 * const config = await state.getJSON<{ theme: string; lang: string }>('config');
 *
 * // List all keys with a prefix
 * const keys = await state.list('user:');
 * ```
 */
export class StateHandle {
    private store: Map<string, Buffer> = new Map();

    /**
     * Retrieve a raw buffer value by key.
     *
     * @param key - The state key.
     * @returns The buffer value, or `undefined` if the key does not exist.
     */
    async get(key: string): Promise<Buffer | undefined> {
        return this.store.get(key);
    }

    /**
     * Store a raw buffer value.
     *
     * @param key   - The state key.
     * @param value - The buffer value to store.
     */
    async set(key: string, value: Buffer): Promise<void> {
        this.store.set(key, value);
    }

    /**
     * Delete a key from the state store.
     *
     * @param key - The state key to remove.
     */
    async delete(key: string): Promise<void> {
        this.store.delete(key);
    }

    /**
     * Retrieve a value as a UTF-8 string.
     *
     * @param key - The state key.
     * @returns The string value, or `undefined` if the key does not exist.
     */
    async getString(key: string): Promise<string | undefined> {
        const value = await this.get(key);
        return value?.toString('utf8');
    }

    /**
     * Store a UTF-8 string value.
     *
     * @param key   - The state key.
     * @param value - The string value to store.
     */
    async setString(key: string, value: string): Promise<void> {
        await this.set(key, Buffer.from(value, 'utf8'));
    }

    /**
     * Retrieve a value parsed from JSON.
     *
     * @typeParam T - The expected type of the parsed JSON value.
     * @param key - The state key.
     * @returns The parsed value, or `undefined` if the key does not exist.
     * @throws SyntaxError If the stored value is not valid JSON.
     */
    async getJSON<T>(key: string): Promise<T | undefined> {
        const value = await this.getString(key);
        return value ? JSON.parse(value) : undefined;
    }

    /**
     * Serialize a value to JSON and store it.
     *
     * @typeParam T - The type of the value to store.
     * @param key   - The state key.
     * @param value - The value to serialize and store.
     */
    async setJSON<T>(key: string, value: T): Promise<void> {
        await this.setString(key, JSON.stringify(value));
    }

    /**
     * List all keys matching a given prefix.
     *
     * Useful for enumerating state entries that share a common namespace.
     *
     * @param prefix - The key prefix to filter by.
     * @returns An array of matching keys.
     *
     * @example
     * ```typescript
     * await state.setString('user:1:name', 'Alice');
     * await state.setString('user:2:name', 'Bob');
     * const keys = await state.list('user:');
     * // => ['user:1:name', 'user:2:name']
     * ```
     */
    async list(prefix: string): Promise<string[]> {
        const keys: string[] = [];
        for (const key of this.store.keys()) {
            if (key.startsWith(prefix)) {
                keys.push(key);
            }
        }
        return keys;
    }
}
