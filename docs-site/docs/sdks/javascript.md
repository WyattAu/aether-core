# JavaScript SDK

The JavaScript/TypeScript SDK provides a modern async interface for building Aether actors.

## Installation

```bash
npm install @aether/sdk
# or
yarn add @aether/sdk
```

## Quick Start

```typescript
import { Actor, Message, MessageType } from '@aether/sdk';

class HelloActor extends Actor {
    constructor() {
        super('hello-actor');
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Actor started`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Actor stopped`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type === MessageType.REQUEST || message.type === MessageType.RPC_REQUEST) {
            const payload = message.payload as Record<string, any>;
            const name = payload?.name || 'World';
            return Message.response({ greeting: `Hello, ${name}!` });
        }
        return null;
    }
}

async function main(): Promise<void> {
    const actor = new HelloActor();
    
    // Handle shutdown
    process.on('SIGINT', async () => {
        await actor.stop();
        process.exit(0);
    });
    
    await actor.start();
    await actor.run();
}

main();
```

## Core Types

### Actor

The `Actor` base class provides the foundation:

```typescript
import { Actor, Message, State } from '@aether/sdk';

class MyActor extends Actor {
    private state: State;

    constructor(name: string) {
        super(name);
        this.state = new State();
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING');
    }

    async onStart(): Promise<void> {
        // Called when actor starts
    }

    async onStop(): Promise<void> {
        // Called when actor stops
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        // Handle incoming messages
        return null;
    }
}
```

### Message

Messages are the primary communication mechanism:

```typescript
import { Message, MessageType } from '@aether/sdk';

// Create different message types
const request = Message.request({ action: 'get', key: 'my-key' });
const response = Message.response({ value: 'my-value' });
const event = Message.event({ type: 'state_changed' });
const rpcRequest = Message.rpcRequest('method_name', { arg: 'value' });

// Message properties
console.log(request.type);      // MessageType.REQUEST
console.log(request.payload);   // { action: 'get', key: 'my-key' }
console.log(request.sender);    // Set by the runtime
```

### MessageType Enum

```typescript
enum MessageType {
    REQUEST = 'request',           // Request expecting response
    RESPONSE = 'response',         // Response to a request
    EVENT = 'event',              // Fire-and-forget event
    RPC_REQUEST = 'rpc_request',   // RPC method call
    RPC_RESPONSE = 'rpc_response', // RPC method response
}
```

### Capability

Capabilities control what an actor can do:

```typescript
enum Capability {
    STATE_READ = 'STATE_READ',
    STATE_WRITE = 'STATE_WRITE',
    NETWORK_OUTBOUND = 'NETWORK_OUTBOUND',
    ACTOR_MESSAGING = 'ACTOR_MESSAGING',
    LOG = 'LOG',
    TIME = 'TIME',
    RANDOM = 'RANDOM',
    AI_USE = 'AI_USE',
}
```

### State

State provides persistent storage:

```typescript
import { State } from '@aether/sdk';

const state = new State();

// Read value
const data: string | null = await state.read('my-key');

// Write value
await state.write('my-key', JSON.stringify({ foo: 'bar' }));

// Delete key
await state.delete('my-key');

// List keys
const keys: string[] = await state.listKeys('prefix-');

// Check existence
const exists: boolean = await state.exists('my-key');

// Clear all state
await state.clear();
```

## Examples

### Counter Actor with State Persistence

```typescript
import { Actor, Message, MessageType, State } from '@aether/sdk';

interface CounterState {
    count: number;
}

class CounterActor extends Actor {
    private count: number = 0;
    private stateKey: string = 'counter_state';
    private state: State;

    constructor() {
        super('counter-actor');
        this.state = new State();
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        // Load persisted state
        const data = await this.state.read(this.stateKey);
        if (data) {
            const state: CounterState = JSON.parse(data);
            this.count = state.count;
            console.log(`[${this.name}] Restored count: ${this.count}`);
        }
    }

    async onStop(): Promise<void> {
        // Save state on shutdown
        await this.saveState();
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type !== MessageType.REQUEST && message.type !== MessageType.RPC_REQUEST) {
            return null;
        }

        const payload = message.payload as Record<string, any>;
        const action = payload?.action;

        switch (action) {
            case 'increment':
                this.count++;
                await this.saveState();
                return Message.response({ count: this.count });

            case 'decrement':
                this.count--;
                await this.saveState();
                return Message.response({ count: this.count });

            case 'get':
                return Message.response({ count: this.count });

            default:
                return Message.response({ error: `unknown action: ${action}` });
        }
    }

    private async saveState(): Promise<void> {
        const state: CounterState = { count: this.count };
        await this.state.write(this.stateKey, JSON.stringify(state));
    }
}

async function main(): Promise<void> {
    const actor = new CounterActor();

    process.on('SIGINT', async () => {
        await actor.stop();
        process.exit(0);
    });

    await actor.start();
    await actor.run();
}

main();
```

## Error Handling

```typescript
import { AetherError, InternalError, StorageReadError, StorageWriteError } from '@aether/sdk';

try {
    const data = await this.state.read(key);
} catch (e) {
    if (e instanceof StorageReadError) {
        return Message.response({ error: `Failed to read: ${e.message}` });
    }
    return Message.response({ error: `Unknown error: ${e}` });
}
```

## Best Practices

1. **Use TypeScript**: Full type safety with TypeScript
2. **Declare capabilities**: Call `require()` in constructor
3. **Handle all message types**: Check `message.type` in `handleMessage`
4. **Persist state**: Save after modifications
5. **Graceful shutdown**: Implement `onStop()` for cleanup
6. **Handle signals**: Listen for SIGINT/SIGTERM

## API Reference

Full API documentation is available in the [API Reference](../api-reference.md) section.
