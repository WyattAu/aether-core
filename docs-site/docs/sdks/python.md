# Python SDK

The Python SDK provides an async-first interface for building Aether actors.

## Installation

```bash
pip install aether-sdk
```

## Quick Start

```python
import asyncio
from aether_sdk import Actor, Message, MessageType

class HelloActor(Actor):
    def __init__(self):
        super().__init__("hello-actor")
        self.require("ACTOR_MESSAGING", "LOG")
    
    async def on_start(self) -> None:
        print(f"[{self.name}] Actor started")
    
    async def on_stop(self) -> None:
        print(f"[{self.name}] Actor stopped")
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            payload = message.payload
            name = payload.get("name", "World") if isinstance(payload, dict) else str(payload)
            return Message.response({"greeting": f"Hello, {name}!"})
        return None

async def main():
    actor = HelloActor()
    await actor.start()
    
    # Run the actor
    await actor.run()

if __name__ == "__main__":
    asyncio.run(main())
```

## Core Types

### Actor

The `Actor` base class provides the foundation:

```python
from aether_sdk import Actor, Capability

class MyActor(Actor):
    def __init__(self, name: str):
        super().__init__(name)
        self.require("STATE_READ", "STATE_WRITE", "ACTOR_MESSAGING")
    
    async def on_start(self) -> None:
        """Called when actor starts."""
        pass
    
    async def on_stop(self) -> None:
        """Called when actor stops."""
        pass
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        """Handle incoming messages."""
        pass
```

### Message

Messages are the primary communication mechanism:

```python
from aether_sdk import Message, MessageType

# Create different message types
request = Message.request({"action": "get", "key": "my-key"})
response = Message.response({"value": "my-value"})
event = Message.event({"type": "state_changed"})
rpc_request = Message.rpc_request("method_name", {"arg": "value"})

# Message properties
print(request.type)      # MessageType.REQUEST
print(request.payload)   # {"action": "get", "key": "my-key"}
print(request.sender)    # Set by the runtime
```

### MessageType Enum

```python
from aether_sdk import MessageType

class MessageType(Enum):
    REQUEST = "request"        # Request expecting response
    RESPONSE = "response"      # Response to a request
    EVENT = "event"           # Fire-and-forget event
    RPC_REQUEST = "rpc_request"    # RPC method call
    RPC_RESPONSE = "rpc_response"  # RPC method response
```

### Capability

Capabilities control what an actor can do:

```python
from aether_sdk import Capability

class Capability(Enum):
    STATE_READ = "STATE_READ"
    STATE_WRITE = "STATE_WRITE"
    NETWORK_OUTBOUND = "NETWORK_OUTBOUND"
    ACTOR_MESSAGING = "ACTOR_MESSAGING"
    LOG = "LOG"
    TIME = "TIME"
    RANDOM = "RANDOM"
    AI_USE = "AI_USE"
```

### State

State provides persistent storage:

```python
class State:
    async def read(self, key: str) -> bytes | None:
        """Read value by key."""
        pass
    
    async def write(self, key: str, value: bytes) -> None:
        """Write value to key."""
        pass
    
    async def delete(self, key: str) -> None:
        """Delete key."""
        pass
    
    async def list_keys(self, prefix: str = "") -> list[str]:
        """List keys with optional prefix filter."""
        pass
    
    async def exists(self, key: str) -> bool:
        """Check if key exists."""
        pass
    
    async def clear(self) -> None:
        """Clear all state."""
        pass
```

## Examples

### Counter Actor with State Persistence

```python
import asyncio
import json
from aether_sdk import Actor, Message

class CounterActor(Actor):
    def __init__(self):
        super().__init__("counter-actor")
        self.count = 0
        self.state_key = "counter_state"
        self.require("STATE_READ", "STATE_WRITE", "ACTOR_MESSAGING", "LOG")
    
    async def on_start(self) -> None:
        # Load persisted state
        data = await self.state.read(self.state_key)
        if data:
            state = json.loads(data)
            self.count = state.get("count", 0)
            print(f"[{self.name}] Restored count: {self.count}")
    
    async def on_stop(self) -> None:
        # Save state on shutdown
        await self._save_state()
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        payload = message.payload
        if not isinstance(payload, dict):
            return Message.response({"error": "invalid payload"})
        
        action = payload.get("action")
        
        if action == "increment":
            self.count += 1
            await self._save_state()
            return Message.response({"count": self.count})
        
        elif action == "decrement":
            self.count -= 1
            await self._save_state()
            return Message.response({"count": self.count})
        
        elif action == "get":
            return Message.response({"count": self.count})
        
        return Message.response({"error": f"unknown action: {action}"})
    
    async def _save_state(self) -> None:
        state = {"count": self.count}
        await self.state.write(self.state_key, json.dumps(state).encode())

async def main():
    actor = CounterActor()
    await actor.start()
    await actor.run()

if __name__ == "__main__":
    asyncio.run(main())
```

## Error Handling

```python
from aether_sdk.errors import (
    AetherError,
    InternalError,
    StorageReadError,
    StorageWriteError,
)

try:
    data = await self.state.read(key)
except StorageReadError as e:
    return Message.response({"error": f"Failed to read: {e}"})
except AetherError as e:
    return Message.response({"error": f"Aether error: {e}"})
```

## Best Practices

1. **Use async/await**: All actor methods are async
2. **Declare capabilities**: Call `require()` in `__init__`
3. **Handle all message types**: Check `message.type` in `handle_message`
4. **Persist state**: Save after modifications
5. **Graceful shutdown**: Override `on_stop()` for cleanup

## API Reference

Full API documentation is available at [readthedocs.io](https://aether-sdk.readthedocs.io).
