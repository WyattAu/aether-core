/**
 * Cache Actor Example
 * 
 * Demonstrates an in-memory cache with TTL support,
 * LRU eviction, and cache statistics.
 */
import { Actor, Message, MessageType, State } from '@aether/sdk';

// ============================================
// Types
// ============================================

interface CacheEntry {
    key: string;
    value: any;
    ttl: number;            // Time to live in seconds (0 = no expiry)
    createdAt: number;      // Timestamp in ms
    expiresAt?: number;     // Timestamp in ms (undefined = no expiry)
    accessCount: number;
    lastAccessedAt: number;
}

interface CacheStats {
    hits: number;
    misses: number;
    sets: number;
    deletes: number;
    evictions: number;
    expirations: number;
    size: number;
    maxSize: number;
    hitRate: number;
}

interface CacheConfig {
    maxSize: number;        // Maximum number of entries
    defaultTtl: number;     // Default TTL in seconds (0 = no expiry)
    cleanupInterval: number; // Cleanup interval in seconds
}

// ============================================
// Cache Actor
// ============================================

class CacheActor extends Actor {
    private cache: Map<string, CacheEntry> = new Map();
    private accessOrder: string[] = [];  // For LRU tracking
    private config: CacheConfig;
    private stats: CacheStats;
    private state: State;
    private stateKey: string;
    private cleanupInterval?: ReturnType<typeof setInterval>;

    constructor() {
        super('cache-actor');
        this.state = new State();
        this.stateKey = 'cache_state';
        
        this.config = {
            maxSize: 1000,
            defaultTtl: 3600,  // 1 hour
            cleanupInterval: 60 // 1 minute
        };

        this.stats = {
            hits: 0,
            misses: 0,
            sets: 0,
            deletes: 0,
            evictions: 0,
            expirations: 0,
            size: 0,
            maxSize: this.config.maxSize,
            hitRate: 0
        };

        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG', 'TIME');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Cache Actor starting...`);
        console.log(`[${this.name}] Max size: ${this.config.maxSize}, Default TTL: ${this.config.defaultTtl}s`);
        
        await this.loadState();
        this.startCleanup();
        
        console.log(`[${this.name}] Loaded ${this.cache.size} cached entries`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Cache Actor stopping...`);
        
        if (this.cleanupInterval) {
            clearInterval(this.cleanupInterval);
        }
        
        await this.saveState();
        
        console.log(`[${this.name}] Final stats: ${this.stats.hits} hits, ${this.stats.misses} misses, ${((this.stats.hitRate) * 100).toFixed(1)}% hit rate`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type !== MessageType.REQUEST && message.type !== MessageType.RPC_REQUEST) {
            return null;
        }

        const payload = message.payload as Record<string, any> | null;
        if (!payload || typeof payload !== 'object') {
            return Message.response({ error: 'invalid payload' });
        }

        const action = payload.action || '';

        switch (action) {
            case 'get':
                return this.handleGet(payload);
            case 'set':
                return this.handleSet(payload);
            case 'delete':
                return this.handleDelete(payload);
            case 'exists':
                return this.handleExists(payload);
            case 'keys':
                return this.handleKeys(payload);
            case 'clear':
                return this.handleClear();
            case 'stats':
                return this.handleStats();
            case 'config':
                return this.handleConfig(payload);
            case 'ttl':
                return this.handleTtl(payload);
            case 'incr':
                return this.handleIncr(payload);
            case 'decr':
                return this.handleDecr(payload);
            case 'mget':
                return this.handleMget(payload);
            case 'mset':
                return this.handleMset(payload);
            default:
                return Message.response({ error: `unknown action: ${action}` });
        }
    }

    private startCleanup(): void {
        this.cleanupInterval = setInterval(() => {
            this.cleanup();
        }, this.config.cleanupInterval * 1000);
    }

    private cleanup(): void {
        const now = Date.now();
        let expiredCount = 0;

        for (const [key, entry] of this.cache) {
            if (entry.expiresAt && entry.expiresAt <= now) {
                this.cache.delete(key);
                this.accessOrder = this.accessOrder.filter(k => k !== key);
                expiredCount++;
            }
        }

        if (expiredCount > 0) {
            this.stats.expirations += expiredCount;
            this.stats.size = this.cache.size;
            console.log(`[${this.name}] Cleaned up ${expiredCount} expired entries`);
        }
    }

    private evictLRU(): void {
        if (this.accessOrder.length === 0) return;

        // Remove least recently used
        const lruKey = this.accessOrder.shift();
        if (lruKey) {
            this.cache.delete(lruKey);
            this.stats.evictions++;
        }
    }

    private updateAccessOrder(key: string): void {
        // Remove from current position
        const index = this.accessOrder.indexOf(key);
        if (index > -1) {
            this.accessOrder.splice(index, 1);
        }
        // Add to end (most recently used)
        this.accessOrder.push(key);
    }

    private getEntry(key: string): CacheEntry | null {
        const entry = this.cache.get(key);
        if (!entry) {
            return null;
        }

        // Check expiration
        if (entry.expiresAt && entry.expiresAt <= Date.now()) {
            this.cache.delete(key);
            this.accessOrder = this.accessOrder.filter(k => k !== key);
            this.stats.expirations++;
            this.stats.size = this.cache.size;
            return null;
        }

        // Update access tracking
        entry.accessCount++;
        entry.lastAccessedAt = Date.now();
        this.updateAccessOrder(key);

        return entry;
    }

    private updateHitRate(): void {
        const total = this.stats.hits + this.stats.misses;
        this.stats.hitRate = total > 0 ? this.stats.hits / total : 0;
    }

    private handleGet(payload: Record<string, any>): Message {
        const key = payload.key;
        if (!key) {
            return Message.response({ error: 'key is required' });
        }

        const entry = this.getEntry(key);
        
        if (entry) {
            this.stats.hits++;
            this.updateHitRate();
            return Message.response({
                action: 'get',
                key,
                value: entry.value,
                found: true,
                ttl: entry.ttl,
                created_at: new Date(entry.createdAt).toISOString()
            });
        } else {
            this.stats.misses++;
            this.updateHitRate();
            return Message.response({
                action: 'get',
                key,
                found: false
            });
        }
    }

    private handleSet(payload: Record<string, any>): Message {
        const key = payload.key;
        const value = payload.value;
        const ttl = payload.ttl ?? this.config.defaultTtl;

        if (key === undefined || key === null) {
            return Message.response({ error: 'key is required' });
        }

        // Evict if at capacity and key doesn't exist
        if (!this.cache.has(key) && this.cache.size >= this.config.maxSize) {
            this.evictLRU();
        }

        const now = Date.now();
        const entry: CacheEntry = {
            key,
            value,
            ttl,
            createdAt: now,
            expiresAt: ttl > 0 ? now + (ttl * 1000) : undefined,
            accessCount: 0,
            lastAccessedAt: now
        };

        this.cache.set(key, entry);
        this.updateAccessOrder(key);
        
        this.stats.sets++;
        this.stats.size = this.cache.size;

        console.log(`[${this.name}] Set key '${key}' with TTL ${ttl}s`);

        return Message.response({
            action: 'set',
            key,
            ttl,
            expires_at: entry.expiresAt ? new Date(entry.expiresAt).toISOString() : null
        });
    }

    private handleDelete(payload: Record<string, any>): Message {
        const key = payload.key;
        if (!key) {
            return Message.response({ error: 'key is required' });
        }

        const existed = this.cache.delete(key);
        if (existed) {
            this.accessOrder = this.accessOrder.filter(k => k !== key);
            this.stats.deletes++;
            this.stats.size = this.cache.size;
        }

        return Message.response({
            action: 'delete',
            key,
            deleted: existed
        });
    }

    private handleExists(payload: Record<string, any>): Message {
        const key = payload.key;
        if (!key) {
            return Message.response({ error: 'key is required' });
        }

        const entry = this.getEntry(key);
        return Message.response({
            action: 'exists',
            key,
            exists: entry !== null
        });
    }

    private handleKeys(payload: Record<string, any>): Message {
        const pattern = payload.pattern || '*';
        const keys: string[] = [];

        for (const key of this.cache.keys()) {
            if (this.matchPattern(key, pattern)) {
                keys.push(key);
            }
        }

        return Message.response({
            action: 'keys',
            pattern,
            keys,
            count: keys.length
        });
    }

    private matchPattern(key: string, pattern: string): boolean {
        if (pattern === '*') return true;
        const regex = new RegExp('^' + pattern.replace(/\*/g, '.*') + '$');
        return regex.test(key);
    }

    private handleClear(): Message {
        const count = this.cache.size;
        this.cache.clear();
        this.accessOrder = [];
        this.stats.size = 0;

        console.log(`[${this.name}] Cleared ${count} entries`);

        return Message.response({
            action: 'clear',
            cleared: count
        });
    }

    private handleStats(): Message {
        return Message.response({
            action: 'stats',
            stats: {
                ...this.stats,
                hit_rate: this.stats.hitRate,
                hit_rate_percent: (this.stats.hitRate * 100).toFixed(2) + '%'
            }
        });
    }

    private handleConfig(payload: Record<string, any>): Message {
        if (payload.max_size !== undefined) {
            this.config.maxSize = Math.max(1, payload.max_size);
            this.stats.maxSize = this.config.maxSize;
        }
        if (payload.default_ttl !== undefined) {
            this.config.defaultTtl = Math.max(0, payload.default_ttl);
        }
        if (payload.cleanup_interval !== undefined) {
            this.config.cleanupInterval = Math.max(1, payload.cleanup_interval);
            // Restart cleanup with new interval
            if (this.cleanupInterval) {
                clearInterval(this.cleanupInterval);
            }
            this.startCleanup();
        }

        return Message.response({
            action: 'config',
            config: this.config
        });
    }

    private handleTtl(payload: Record<string, any>): Message {
        const key = payload.key;
        const ttl = payload.ttl;

        if (!key) {
            return Message.response({ error: 'key is required' });
        }

        const entry = this.cache.get(key);
        if (!entry) {
            return Message.response({
                action: 'ttl',
                key,
                found: false
            });
        }

        // Check if expired
        if (entry.expiresAt && entry.expiresAt <= Date.now()) {
            return Message.response({
                action: 'ttl',
                key,
                found: false
            });
        }

        if (ttl !== undefined) {
            // Update TTL
            entry.ttl = ttl;
            entry.expiresAt = ttl > 0 ? Date.now() + (ttl * 1000) : undefined;
        }

        // Calculate remaining TTL
        let remainingTtl = -1; // No expiry
        if (entry.expiresAt) {
            remainingTtl = Math.max(0, Math.floor((entry.expiresAt - Date.now()) / 1000));
        }

        return Message.response({
            action: 'ttl',
            key,
            found: true,
            ttl: remainingTtl
        });
    }

    private handleIncr(payload: Record<string, any>): Message {
        const key = payload.key;
        const amount = payload.amount || 1;

        if (!key) {
            return Message.response({ error: 'key is required' });
        }

        const entry = this.getEntry(key);
        if (!entry) {
            // Create new entry with value 0, then increment
            this.cache.set(key, {
                key,
                value: 0,
                ttl: this.config.defaultTtl,
                createdAt: Date.now(),
                expiresAt: this.config.defaultTtl > 0 
                    ? Date.now() + (this.config.defaultTtl * 1000) 
                    : undefined,
                accessCount: 0,
                lastAccessedAt: Date.now()
            });
            this.accessOrder.push(key);
            return this.handleIncr({ key, amount });
        }

        if (typeof entry.value !== 'number') {
            return Message.response({ error: 'value is not a number' });
        }

        entry.value += amount;
        this.stats.sets++;

        return Message.response({
            action: 'incr',
            key,
            value: entry.value
        });
    }

    private handleDecr(payload: Record<string, any>): Message {
        const amount = payload.amount || 1;
        return this.handleIncr({ ...payload, amount: -amount });
    }

    private handleMget(payload: Record<string, any>): Message {
        const keys = payload.keys;
        if (!Array.isArray(keys)) {
            return Message.response({ error: 'keys must be an array' });
        }

        const results: Record<string, { value: any; found: boolean }> = {};
        
        for (const key of keys) {
            const entry = this.getEntry(key);
            if (entry) {
                this.stats.hits++;
                results[key] = { value: entry.value, found: true };
            } else {
                this.stats.misses++;
                results[key] = { value: null, found: false };
            }
        }

        this.updateHitRate();

        return Message.response({
            action: 'mget',
            results,
            count: keys.length
        });
    }

    private handleMset(payload: Record<string, any>): Message {
        const entries = payload.entries;
        if (!entries || typeof entries !== 'object') {
            return Message.response({ error: 'entries must be an object' });
        }

        const ttl = payload.ttl ?? this.config.defaultTtl;
        const now = Date.now();
        let setCount = 0;

        for (const [key, value] of Object.entries(entries)) {
            // Evict if at capacity
            if (!this.cache.has(key) && this.cache.size >= this.config.maxSize) {
                this.evictLRU();
            }

            const entry: CacheEntry = {
                key,
                value,
                ttl,
                createdAt: now,
                expiresAt: ttl > 0 ? now + (ttl * 1000) : undefined,
                accessCount: 0,
                lastAccessedAt: now
            };

            this.cache.set(key, entry);
            this.updateAccessOrder(key);
            setCount++;
        }

        this.stats.sets += setCount;
        this.stats.size = this.cache.size;

        return Message.response({
            action: 'mset',
            count: setCount
        });
    }

    private async loadState(): Promise<void> {
        try {
            const data = await this.state.read(this.stateKey);
            if (data) {
                const state = JSON.parse(data);
                if (state.cache) {
                    for (const [key, entry] of Object.entries(state.cache)) {
                        this.cache.set(key, entry as CacheEntry);
                    }
                }
                if (state.accessOrder) {
                    this.accessOrder = state.accessOrder;
                }
                if (state.stats) {
                    this.stats = { ...this.stats, ...state.stats };
                }
            }
        } catch (error) {
            console.error(`[${this.name}] Failed to load state:`, error);
        }
    }

    private async saveState(): Promise<void> {
        const state = {
            cache: Object.fromEntries(this.cache),
            accessOrder: this.accessOrder,
            stats: this.stats
        };
        await this.state.write(this.stateKey, JSON.stringify(state));
    }
}

// ============================================
// Main Entry Point
// ============================================

async function main(): Promise<void> {
    const actor = new CacheActor();

    process.on('SIGINT', async () => {
        console.log('\nShutting down cache actor...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    console.log('Actions: get, set, delete, exists, keys, clear, stats, config, ttl, incr, decr, mget, mset');

    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
