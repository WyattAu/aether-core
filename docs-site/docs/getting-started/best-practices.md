# Aether Best Practices Guide

This guide covers best practices for building reliable, performant, and maintainable actor-based applications.

## Actor Design

### Single Responsibility

Each actor should have a single, well-defined responsibility:

```typescript
// [PASS] Good: Focused responsibility
class OrderActor extends Actor {
    // Only handles order-related operations
}

class InventoryActor extends Actor {
    // Only handles inventory operations
}

// [FAIL] Bad: Multiple responsibilities
class OrderAndInventoryActor extends Actor {
    // Handles both orders AND inventory
}
```

### Stateless When Possible

Prefer stateless actors for better scalability:

```typescript
// [PASS] Good: Stateless transformation
class TransformerActor extends Actor {
    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        const result = this.transform(message.payload);
        return Message.response(result);
    }
}

// [FAIL] Bad: Unnecessary state
class TransformerWithStateActor extends Actor {
    private callCount = 0;  // Only needed if count matters
    
    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        this.callCount++;  // Unnecessary state mutation
        const result = this.transform(message.payload);
        return Message.response(result);
    }
}
```

### Explicit Capabilities

Always declare required capabilities explicitly:

```typescript
// [PASS] Good: Explicit capabilities
class DataActor extends Actor {
    constructor() {
        super('data-actor');
        this.require(
            'STATE_READ',
            'STATE_WRITE',
            'ACTOR_MESSAGING',
            'LOG'
        );
    }
}

// [FAIL] Bad: Missing capabilities
class DataActor extends Actor {
    constructor() {
        super('data-actor');
        // Capabilities not declared
    }
}
```

## State Management

### Immediate Persistence

Save state immediately after modifications:

```typescript
// [PASS] Good: Save immediately
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const payload = message.payload as Record<string, any>;
    
    this.data = payload.value;
    await this.saveState();  // Save immediately
    
    return Message.response({ success: true });
}

// [FAIL] Bad: Delayed persistence
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const payload = message.payload as Record<string, any>;
    
    this.data = payload.value;
    this.pendingSave = true;  // Risk of data loss
    
    return Message.response({ success: true });
}
```

### Namespace State Keys

Use namespaced keys to avoid collisions:

```typescript
// [PASS] Good: Namespaced keys
private getStateKey(key: string): string {
    return `${this.name}:${key}`;
}

// Usage
const stateKey = this.getStateKey('user_data');
// Result: 'user-actor:user_data'

// [FAIL] Bad: Generic keys
const stateKey = 'data';  // Could collide with other actors
```

### Handle Missing State Gracefully

```typescript
// [PASS] Good: Graceful handling
async onStart(): Promise<void> {
    const data = await this.state.read(this.stateKey);
    if (data) {
        try {
            this.state = JSON.parse(data);
        } catch (e) {
            console.error('Failed to parse state, using defaults');
            this.state = this.getDefaultState();
        }
    } else {
        this.state = this.getDefaultState();
    }
}
```

## Message Handling

### Validate Input

Always validate incoming messages:

```typescript
// [PASS] Good: Input validation
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const payload = message.payload;
    
    if (!payload || typeof payload !== 'object') {
        return Message.response({ error: 'Invalid payload' });
    }
    
    const action = payload.action;
    if (!action || typeof action !== 'string') {
        return Message.response({ error: 'Action required' });
    }
    
    // Process validated input
}
```

### Use Typed Payloads

Define interfaces for message payloads:

```typescript
// [PASS] Good: Typed payloads
interface CreateOrderPayload {
    action: 'create';
    customer: string;
    items: Array<{ item: string; quantity: number }>;
}

interface GetOrderPayload {
    action: 'get';
    order_id: string;
}

type OrderPayload = CreateOrderPayload | GetOrderPayload;

async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const payload = message.payload as OrderPayload;
    
    switch (payload.action) {
        case 'create':
            // TypeScript knows payload has customer and items
            return this.createOrder(payload);
        case 'get':
            // TypeScript knows payload has order_id
            return this.getOrder(payload);
    }
}
```

### Return Consistent Responses

```typescript
// [PASS] Good: Consistent response format
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    try {
        const result = await this.processAction(message.payload);
        return Message.response({
            success: true,
            data: result
        });
    } catch (error) {
        return Message.response({
            success: false,
            error: error instanceof Error ? error.message : 'Unknown error'
        });
    }
}
```

## Error Handling

### Never Panic

Avoid throwing uncaught exceptions:

```typescript
// [PASS] Good: Return error responses
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    try {
        const result = await this.riskyOperation();
        return Message.response({ success: true, result });
    } catch (error) {
        console.error('Operation failed:', error);
        return Message.response({ 
            success: false, 
            error: error instanceof Error ? error.message : 'Unknown error' 
        });
    }
}

// [FAIL] Bad: Uncaught exceptions
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const result = await this.riskyOperation();  // Could throw!
    return Message.response({ result });
}
```

### Use Structured Errors

```typescript
// [PASS] Good: Structured errors
class ActorError extends Error {
    constructor(
        public code: string,
        message: string,
        public details?: any
    ) {
        super(message);
    }
}

async handleMessage(sender: string, message: Message): Promise<Message | null> {
    try {
        // ...
    } catch (error) {
        if (error instanceof ActorError) {
            return Message.response({
                error: {
                    code: error.code,
                    message: error.message,
                    details: error.details
                }
            });
        }
        // ...
    }
}
```

## Performance

### Avoid Blocking Operations

```typescript
// [PASS] Good: Non-blocking
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const result = await this.asyncOperation();
    return Message.response(result);
}

// [FAIL] Bad: Blocking
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const result = this.blockingOperation();  // Blocks the event loop!
    return Message.response(result);
}
```

### Batch Operations

```typescript
// [PASS] Good: Batch processing
async handleBatch(items: any[]): Promise<Message> {
    const results = await Promise.all(
        items.map(item => this.processItem(item))
    );
    return Message.response({ results });
}
```

### Cache Frequently Accessed Data

```typescript
// [PASS] Good: Caching
class CachedActor extends Actor {
    private cache: Map<string, { data: any; expiry: number }> = new Map();
    private cacheTTL = 60000; // 1 minute

    private async getData(key: string): Promise<any> {
        const cached = this.cache.get(key);
        if (cached && cached.expiry > Date.now()) {
            return cached.data;
        }

        const data = await this.expensiveFetch(key);
        this.cache.set(key, {
            data,
            expiry: Date.now() + this.cacheTTL
        });
        return data;
    }
}
```

## Logging

### Structured Logging

```typescript
// [PASS] Good: Structured logging
class MyActor extends Actor {
    private log(level: string, message: string, data?: any): void {
        console.log(JSON.stringify({
            timestamp: new Date().toISOString(),
            level,
            actor: this.name,
            message,
            ...data
        }));
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        this.log('info', 'Processing message', { 
            sender, 
            type: message.type 
        });
        // ...
    }
}
```

### Log Levels

Use appropriate log levels:

| Level | Use Case |
|-------|----------|
| DEBUG | Detailed diagnostic information |
| INFO | General operational events |
| WARN | Unexpected but handled situations |
| ERROR | Failures that don't stop the actor |
| FATAL | Critical failures |

## Testing

### Test All Message Types

```typescript
describe('MyActor', () => {
    let actor: MyActor;

    beforeEach(async () => {
        actor = new MyActor();
        await actor.start();
    });

    afterEach(async () => {
        await actor.stop();
    });

    it('should handle create action', async () => {
        const result = await actor.handleMessage('test', 
            Message.request({ action: 'create', name: 'test' }));
        
        expect(result?.payload).toHaveProperty('id');
    });

    it('should handle get action', async () => {
        // First create
        await actor.handleMessage('test', 
            Message.request({ action: 'create', name: 'test' }));
        
        // Then get
        const result = await actor.handleMessage('test', 
            Message.request({ action: 'get', id: 'test-id' }));
        
        expect(result?.payload).toHaveProperty('name', 'test');
    });

    it('should return error for unknown action', async () => {
        const result = await actor.handleMessage('test', 
            Message.request({ action: 'unknown' }));
        
        expect(result?.payload).toHaveProperty('error');
    });
});
```

### Test Error Cases

```typescript
it('should handle invalid payload', async () => {
    const result = await actor.handleMessage('test', 
        Message.request('not an object'));
    
    expect(result?.payload).toHaveProperty('error');
});

it('should handle missing required fields', async () => {
    const result = await actor.handleMessage('test', 
        Message.request({ action: 'create' }));  // Missing 'name'
    
    expect(result?.payload).toHaveProperty('error');
});
```

## Security

### Validate All Inputs

```typescript
// [PASS] Good: Input validation
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    const payload = message.payload as Record<string, any>;
    
    // Validate types
    if (payload.id && typeof payload.id !== 'string') {
        return Message.response({ error: 'Invalid id type' });
    }
    
    // Validate ranges
    if (payload.count && (payload.count < 0 || payload.count > 1000)) {
        return Message.response({ error: 'Count out of range' });
    }
    
    // Validate patterns
    if (payload.email && !this.isValidEmail(payload.email)) {
        return Message.response({ error: 'Invalid email format' });
    }
}
```

### Don't Expose Internal State

```typescript
// [PASS] Good: Return copies
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    return Message.response({
        items: [...this.items],  // Copy, not reference
        count: this.items.length
    });
}

// [FAIL] Bad: Expose internal state
async handleMessage(sender: string, message: Message): Promise<Message | null> {
    return Message.response({
        items: this.items  // Direct reference to internal state!
    });
}
```

## Lifecycle Management

### Clean Up Resources

```typescript
// [PASS] Good: Clean up in onStop
class ResourceActor extends Actor {
    private timer?: ReturnType<typeof setInterval>;
    private connections: Connection[] = [];

    async onStart(): Promise<void> {
        this.timer = setInterval(() => this.tick(), 1000);
    }

    async onStop(): Promise<void> {
        if (this.timer) {
            clearInterval(this.timer);
        }
        
        for (const conn of this.connections) {
            await conn.close();
        }
        this.connections = [];
    }
}
```

### Graceful Shutdown

```typescript
// [PASS] Good: Graceful shutdown
async function main(): Promise<void> {
    const actor = new MyActor();
    await actor.start();

    const shutdown = async () => {
        console.log('\nShutting down gracefully...');
        await actor.stop();
        process.exit(0);
    };

    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);

    await actor.run();
}
```

## Summary

Following these best practices will help you build:

- **Reliable** actors that handle errors gracefully
- **Performant** actors that scale well
- **Maintainable** actors that are easy to understand
- **Secure** actors that protect data and resources
- **Testable** actors that can be verified

## Checklist

Before deploying your actor, verify:

- [ ] All capabilities are declared
- [ ] Input validation is in place
- [ ] State is persisted after modifications
- [ ] Errors are handled gracefully
- [ ] Resources are cleaned up on shutdown
- [ ] Logging is structured and appropriate
- [ ] Tests cover all message types and error cases
