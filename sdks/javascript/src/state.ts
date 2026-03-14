export class StateHandle {
    private store: Map<string, Buffer> = new Map();

    async get(key: string): Promise<Buffer | undefined> {
        return this.store.get(key);
    }

    async set(key: string, value: Buffer): Promise<void> {
        this.store.set(key, value);
    }

    async delete(key: string): Promise<void> {
        this.store.delete(key);
    }

    async getString(key: string): Promise<string | undefined> {
        const value = await this.get(key);
        return value?.toString('utf8');
    }

    async setString(key: string, value: string): Promise<void> {
        await this.set(key, Buffer.from(value, 'utf8'));
    }

    async getJSON<T>(key: string): Promise<T | undefined> {
        const value = await this.getString(key);
        return value ? JSON.parse(value) : undefined;
    }

    async setJSON<T>(key: string, value: T): Promise<void> {
        await this.setString(key, JSON.stringify(value));
    }

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
