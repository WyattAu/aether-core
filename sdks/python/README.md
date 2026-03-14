# Aether Python SDK

Python SDK for the Aether Actor Runtime.

## Installation

```bash
pip install aether-sdk
```

## Quick Start

### Creating an Actor

```python
from aether_sdk import Actor, Message, MessageType

class MyActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "my_actor"
    
    async def handle_message(self, sender: str, message: Message) -> Message:
        if message.type == MessageType.CUSTOM:
            return Message(
                type=MessageType.CUSTOM,
                payload={"echo": message.payload}
            )
```

### Using Capabilities

```python
from aether_sdk import Actor, Capability

class NetworkActor(Actor):
    def __init__(self):
        super().__init__()
        self.require(Capability.NETWORK_OUTBOUND)
```

### State Management

```python
from aether_sdk import Actor, Capability

class StatefulActor(Actor):
    def __init__(self):
        super().__init__()
        self.require(Capability.STATE_READ, Capability.STATE_WRITE)
    
    async def handle_message(self, sender: str, message: Message):
        count = await self.state.get_json("counter") or 0
        count += 1
        await self.state.set_json("counter", count)
        return Message(type=MessageType.CUSTOM, payload={"count": count})
```

## Capabilities

| Capability | Description |
|------------|-------------|
| `NETWORK_OUTBOUND` | Make outbound network requests |
| `NETWORK_INBOUND` | Accept inbound network connections |
| `STATE_READ` | Read from state store |
| `STATE_WRITE` | Write to state store |
| `FS_READ` | Read from filesystem |
| `FS_WRITE` | Write to filesystem |
| `ACTOR_MESSAGING` | Send messages to other actors |
| `LOG` | Write to logs |
| `TIME` | Access time functions |
| `RANDOM` | Access random number generation |
| `ENVIRONMENT` | Access environment variables |
| `HTTP_CLIENT` | Use HTTP client |
| `HTTP_SERVER` | Use HTTP server |

## Message Types

| Type | Description |
|------|-------------|
| `START` | Actor start signal |
| `STOP` | Actor stop signal |
| `SIGNAL` | Generic signal |
| `RPC_REQUEST` | RPC request |
| `RPC_RESPONSE` | RPC response |
| `CUSTOM` | Custom message |

## Development

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Type checking
mypy aether_sdk

# Format code
black aether_sdk tests
```

## License

MIT
