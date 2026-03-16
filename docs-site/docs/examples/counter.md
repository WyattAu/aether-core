# Stateful Counter Example

This example demonstrates state persistence in Aether actors.

## Overview

The Counter Actor:

1. Maintains a count value
2. Persists state to disk
3. Handles increment/decrement/get operations
4. Recovers state on restart

## Go Implementation

```go
package main

import (
    "encoding/json"
    "fmt"
    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type CounterState struct {
    Count int `json:"count"`
}

type CounterActor struct {
    aether.Actor
    count     int
    stateKey  string
}

func (a *CounterActor) OnStart() error {
    a.stateKey = fmt.Sprintf("counter_%s_state", a.Name)
    
    // Load persisted state
    data, err := a.State.Read(a.stateKey)
    if err == nil && data != nil {
        var state CounterState
        if err := json.Unmarshal(data, &state); err == nil {
            a.count = state.Count
            fmt.Printf("[%s] Restored count: %d\n", a.Name, a.count)
        }
    }
    
    fmt.Printf("[%s] Counter Actor started\n", a.Name)
    return nil
}

func (a *CounterActor) OnStop() error {
    // Save state on shutdown
    a.saveState()
    fmt.Printf("[%s] Counter Actor stopped (final count: %d)\n", a.Name, a.count)
    return nil
}

func (a *CounterActor) HandleMessage(sender string, msg aether.Message) (aether.Message, error) {
    payload, ok := msg.Payload.(map[string]interface{})
    if !ok {
        return aether.Message{}, fmt.Errorf("invalid payload")
    }
    
    action, _ := payload["action"].(string)
    
    switch action {
    case "increment":
        a.count++
        a.saveState()
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"count": a.count},
        }, nil
        
    case "decrement":
        a.count--
        a.saveState()
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"count": a.count},
        }, nil
        
    case "get":
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"count": a.count},
        }, nil
        
    case "reset":
        a.count = 0
        a.saveState()
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"count": a.count},
        }, nil
    }
    
    return aether.Message{}, fmt.Errorf("unknown action: %s", action)
}

func (a *CounterActor) saveState() {
    state := CounterState{Count: a.count}
    data, _ := json.Marshal(state)
    a.State.Write(a.stateKey, data)
}

func main() {
    actor := &CounterActor{}
    actor.Name = "counter-actor"
    actor.Require("STATE_READ", "STATE_WRITE", "ACTOR_MESSAGING", "LOG")
    
    if err := actor.Start(); err != nil {
        panic(err)
    }
    defer actor.Stop()
    
    actor.Run()
}
```

## Python Implementation

```python
import asyncio
import json
from aether_sdk import Actor, Message, MessageType

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
        print(f"[{self.name}] Counter Actor started")
    
    async def on_stop(self) -> None:
        await self._save_state()
        print(f"[{self.name}] Counter Actor stopped (final count: {self.count})")
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type not in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return None
        
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
        
        elif action == "reset":
            self.count = 0
            await self._save_state()
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

## JavaScript Implementation

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
        console.log(`[${this.name}] Counter Actor started`);
    }

    async onStop(): Promise<void> {
        await this.saveState();
        console.log(`[${this.name}] Counter Actor stopped (final count: ${this.count})`);
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

            case 'reset':
                this.count = 0;
                await this.saveState();
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

## Running the Example

### Build and Run

```bash
# Go
cd sdks/go/examples/counter_actor
go run main.go

# Python
cd sdks/python/examples
python counter_actor.py

# JavaScript
cd sdks/js/examples
npx ts-node counter_actor.ts
```

### Testing

```bash
# Increment
aether invoke counter-actor '{"action": "increment"}'
# Response: {"count": 1}

# Get current value
aether invoke counter-actor '{"action": "get"}'
# Response: {"count": 1}

# Decrement
aether invoke counter-actor '{"action": "decrement"}'
# Response: {"count": 0}

# Reset
aether invoke counter-actor '{"action": "reset"}'
# Response: {"count": 0}
```

## Key Concepts

### State Persistence

The actor persists its count to storage:

| Method | Description |
|--------|-------------|
| `state.read(key)` | Read value from storage |
| `state.write(key, value)` | Write value to storage |
| `state.delete(key)` | Delete key from storage |

### Lifecycle Hooks

| Hook | When Called |
|------|-------------|
| `onStart()` | Actor starts, load state |
| `onStop()` | Actor stops, save state |

### Message Handling

| Action | Description |
|--------|-------------|
| `increment` | Increase count by 1 |
| `decrement` | Decrease count by 1 |
| `get` | Return current count |
| `reset` | Reset count to 0 |

## Best Practices

1. **Save state immediately**: Persist after each modification
2. **Handle load errors**: Gracefully handle missing/corrupt state
3. **Use typed state**: Define state structs/interfaces
4. **Atomic operations**: Each action is self-contained
