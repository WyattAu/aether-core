# @aether/sdk

JavaScript/TypeScript SDK for the Aether Actor Runtime.

## Installation

```bash
npm install @aether/sdk
```

## Quick Start

```typescript
import { Actor, Message, MessageType } from '@aether/sdk';

class HelloActor extends Actor {
    static get name() { return 'hello'; }

    async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.type === MessageType.CUSTOM) {
            const name = message.payload.name || 'World';
            return Message.custom({ greeting: `Hello, ${name}!` });
        }
    }
}
```

## Core Concepts

### Actor

The `Actor` class is the base class for all actors in Aether.

```typescript
class MyActor extends Actor {
    static get name() { return 'my-actor'; }

    async handle(sender: string, message: Message): Promise<Message | void> {
        // Handle messages
    }

    async onStart(): Promise<void> {
        // Initialization logic
    }

    async onStop(): Promise<void> {
        // Cleanup logic
    }
}
```

### Capabilities

Actors must declare required capabilities:

```typescript
import { Capability } from '@aether/sdk';

this.require(
    Capability.NETWORK_OUTBOUND,
    Capability.STATE_READ,
    Capability.STATE_WRITE
);
```

Available capabilities:
- `NETWORK_OUTBOUND` - Outbound network access
- `NETWORK_INBOUND` - Inbound network access
- `STATE_READ` - Read from state store
- `STATE_WRITE` - Write to state store
- `FS_READ` - Filesystem read access
- `FS_WRITE` - Filesystem write access
- `ACTOR_MESSAGING` - Inter-actor messaging
- `LOG` - Logging capability
- `TIME` - Time access
- `RANDOM` - Random number generation
- `ENVIRONMENT` - Environment variable access
- `HTTP_CLIENT` - HTTP client access
- `HTTP_SERVER` - HTTP server access

### Messaging

Send and receive messages between actors:

```typescript
// Create a message
const msg = Message.custom({ action: 'process', data: value });

// Send (fire-and-forget)
await this.send('target-actor', msg);

// Call (RPC-style with response)
const response = await this.call<Response>('target-actor', request);
```

Message types:
- `START` - Actor start signal
- `STOP` - Actor stop signal
- `SIGNAL` - Generic signal
- `RPC_REQUEST` - RPC request
- `RPC_RESPONSE` - RPC response
- `CUSTOM` - Custom message

### State Management

Persistent state for actors:

```typescript
// Store and retrieve values
await this.state.setString('key', 'value');
const value = await this.state.getString('key');

// JSON support
await this.state.setJSON('config', { setting: true });
const config = await this.state.getJSON('config');

// List keys by prefix
const keys = await this.state.list('user:');
```

### HTTP Client

Make HTTP requests (requires `NETWORK_OUTBOUND` capability):

```typescript
import { HttpClient, Capability } from '@aether/sdk';

class HttpActor extends Actor {
    private httpClient!: HttpClient;

    async onStart() {
        this.require(Capability.NETWORK_OUTBOUND);
        this.httpClient = new HttpClient(this.capabilities);
    }

    async fetchData() {
        const response = await this.httpClient.get('https://api.example.com/data');
        return response.body;
    }
}
```

## Examples

See the `examples/` directory for complete examples:
- `hello-actor.ts` - Basic actor example
- `http-actor.ts` - HTTP client usage
- `stateful-actor.ts` - State management

## Development

```bash
# Build
npm run build

# Test
npm test

# Lint
npm run lint
```

## License

MIT
