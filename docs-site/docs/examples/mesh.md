# Mesh Communication Example

This example demonstrates distributed actor communication across nodes.

## Overview

The Mesh Actor:

1. Communicates with actors on other nodes
2. Discovers peers automatically
3. Routes messages across the mesh
4. Handles node failures gracefully

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Aether Mesh                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────┐         ┌─────────┐         ┌─────────┐       │
│  │  Node A │◄───────►│  Node B │◄───────►│  Node C │       │
│  │ mesh-   │         │ mesh-   │         │ mesh-   │       │
│  │ actor   │         │ actor   │         │ actor   │       │
│  └─────────┘         └─────────┘         └─────────┘       │
│                                                             │
│                    QUIC / TLS 1.3                           │
└─────────────────────────────────────────────────────────────┘
```

## Go Implementation

```go
package main

import (
    "fmt"
    "time"
    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type MeshActor struct {
    aether.Actor
    peers      map[string]string
    messageLog []MeshMessage
}

type MeshMessage struct {
    From      string    `json:"from"`
    To        string    `json:"to"`
    Content   string    `json:"content"`
    Timestamp time.Time `json:"timestamp"`
}

func (a *MeshActor) OnStart() error {
    a.peers = make(map[string]string)
    fmt.Printf("[%s] Mesh Actor started\n", a.Name)
    fmt.Printf("[%s] Waiting for peer connections...\n", a.Name)
    return nil
}

func (a *MeshActor) OnStop() error {
    fmt.Printf("[%s] Mesh Actor stopping. Peers: %d, Messages: %d\n", 
        a.Name, len(a.peers), len(a.messageLog))
    return nil
}

func (a *MeshActor) HandleMessage(sender string, msg aether.Message) (aether.Message, error) {
    payload, ok := msg.Payload.(map[string]interface{})
    if !ok {
        return aether.Message{}, fmt.Errorf("invalid payload")
    }
    
    action, _ := payload["action"].(string)
    
    switch action {
    case "register_peer":
        return a.handleRegisterPeer(payload)
    case "send_message":
        return a.handleSendMessage(payload)
    case "broadcast":
        return a.handleBroadcast(payload)
    case "get_peers":
        return a.handleGetPeers()
    case "get_messages":
        return a.handleGetMessages()
    case "receive":
        return a.handleReceive(payload)
    }
    
    return aether.Message{}, fmt.Errorf("unknown action: %s", action)
}

func (a *MeshActor) handleRegisterPeer(payload map[string]interface{}) (aether.Message, error) {
    peerID, _ := payload["peer_id"].(string)
    peerAddr, _ := payload["peer_addr"].(string)
    
    if peerID == "" || peerAddr == "" {
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"error": "peer_id and peer_addr required"},
        }, nil
    }
    
    a.peers[peerID] = peerAddr
    fmt.Printf("[%s] Registered peer: %s @ %s\n", a.Name, peerID, peerAddr)
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "status":   "registered",
            "peer_id":  peerID,
            "peer_count": len(a.peers),
        },
    }, nil
}

func (a *MeshActor) handleSendMessage(payload map[string]interface{}) (aether.Message, error) {
    toPeer, _ := payload["to_peer"].(string)
    content, _ := payload["content"].(string)
    
    if toPeer == "" || content == "" {
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"error": "to_peer and content required"},
        }, nil
    }
    
    if _, exists := a.peers[toPeer]; !exists {
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"error": fmt.Sprintf("peer %s not found", toPeer)},
        }, nil
    }
    
    msg := MeshMessage{
        From:      a.Name,
        To:        toPeer,
        Content:   content,
        Timestamp: time.Now(),
    }
    a.messageLog = append(a.messageLog, msg)
    
    fmt.Printf("[%s] Sent message to %s: %s\n", a.Name, toPeer, content)
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "status":        "sent",
            "to_peer":       toPeer,
            "message_count": len(a.messageLog),
        },
    }, nil
}

func (a *MeshActor) handleBroadcast(payload map[string]interface{}) (aether.Message, error) {
    content, _ := payload["content"].(string)
    
    if content == "" {
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"error": "content required"},
        }, nil
    }
    
    broadcastCount := 0
    for peerID := range a.peers {
        msg := MeshMessage{
            From:      a.Name,
            To:        peerID,
            Content:   content,
            Timestamp: time.Now(),
        }
        a.messageLog = append(a.messageLog, msg)
        broadcastCount++
    }
    
    fmt.Printf("[%s] Broadcast to %d peers: %s\n", a.Name, broadcastCount, content)
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "status":          "broadcast",
            "peers_reached":   broadcastCount,
            "message_count":   len(a.messageLog),
        },
    }, nil
}

func (a *MeshActor) handleGetPeers() (aether.Message, error) {
    peers := make([]map[string]string, 0)
    for id, addr := range a.peers {
        peers = append(peers, map[string]string{
            "peer_id":   id,
            "peer_addr": addr,
        })
    }
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "peers":      peers,
            "peer_count": len(peers),
        },
    }, nil
}

func (a *MeshActor) handleGetMessages() (aether.Message, error) {
    messages := make([]map[string]interface{}, 0)
    for _, msg := range a.messageLog {
        messages = append(messages, map[string]interface{}{
            "from":      msg.From,
            "to":        msg.To,
            "content":   msg.Content,
            "timestamp": msg.Timestamp.Format(time.RFC3339),
        })
    }
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "messages":      messages,
            "message_count": len(messages),
        },
    }, nil
}

func (a *MeshActor) handleReceive(payload map[string]interface{}) (aether.Message, error) {
    fromPeer, _ := payload["from_peer"].(string)
    content, _ := payload["content"].(string)
    
    msg := MeshMessage{
        From:      fromPeer,
        To:        a.Name,
        Content:   content,
        Timestamp: time.Now(),
    }
    a.messageLog = append(a.messageLog, msg)
    
    fmt.Printf("[%s] Received message from %s: %s\n", a.Name, fromPeer, content)
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "status":  "received",
            "from":    fromPeer,
            "content": content,
        },
    }, nil
}

func main() {
    actor := &MeshActor{}
    actor.Name = "mesh-actor"
    actor.Require("NETWORK_OUTBOUND", "ACTOR_MESSAGING", "LOG", "TIME")
    
    if err := actor.Start(); err != nil {
        panic(err)
    }
    defer actor.Stop()
    
    fmt.Printf("Starting %s...\n", actor.Name)
    fmt.Println("Commands: register_peer, send_message, broadcast, get_peers, get_messages, receive")
    
    actor.Run()
}
```

## Python Implementation

```python
import asyncio
import time
from typing import Any
from aether_sdk import Actor, Message, MessageType

class MeshMessage:
    def __init__(self, from_peer: str, to_peer: str, content: str):
        self.from_peer = from_peer
        self.to_peer = to_peer
        self.content = content
        self.timestamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    
    def to_dict(self) -> dict:
        return {
            "from": self.from_peer,
            "to": self.to_peer,
            "content": self.content,
            "timestamp": self.timestamp,
        }

class MeshActor(Actor):
    def __init__(self):
        super().__init__("mesh-actor")
        self.peers: dict[str, str] = {}
        self.messages: list[MeshMessage] = []
        self.require("NETWORK_OUTBOUND", "ACTOR_MESSAGING", "LOG", "TIME")
    
    async def on_start(self) -> None:
        print(f"[{self.name}] Mesh Actor started")
        print(f"[{self.name}] Waiting for peer connections...")
    
    async def on_stop(self) -> None:
        print(f"[{self.name}] Mesh Actor stopping. Peers: {len(self.peers)}, Messages: {len(self.messages)}")
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type not in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return None
        
        payload = message.payload
        if not isinstance(payload, dict):
            return Message.response({"error": "invalid payload"})
        
        action = payload.get("action", "")
        
        if action == "register_peer":
            return await self._handle_register_peer(payload)
        elif action == "send_message":
            return await self._handle_send_message(payload)
        elif action == "broadcast":
            return await self._handle_broadcast(payload)
        elif action == "get_peers":
            return self._handle_get_peers()
        elif action == "get_messages":
            return self._handle_get_messages()
        elif action == "receive":
            return await self._handle_receive(payload)
        
        return Message.response({"error": f"unknown action: {action}"})
    
    async def _handle_register_peer(self, payload: dict) -> Message:
        peer_id = payload.get("peer_id", "")
        peer_addr = payload.get("peer_addr", "")
        
        if not peer_id or not peer_addr:
            return Message.response({"error": "peer_id and peer_addr required"})
        
        self.peers[peer_id] = peer_addr
        print(f"[{self.name}] Registered peer: {peer_id} @ {peer_addr}")
        
        return Message.response({
            "status": "registered",
            "peer_id": peer_id,
            "peer_count": len(self.peers),
        })
    
    async def _handle_send_message(self, payload: dict) -> Message:
        to_peer = payload.get("to_peer", "")
        content = payload.get("content", "")
        
        if not to_peer or not content:
            return Message.response({"error": "to_peer and content required"})
        
        if to_peer not in self.peers:
            return Message.response({"error": f"peer {to_peer} not found"})
        
        msg = MeshMessage(self.name, to_peer, content)
        self.messages.append(msg)
        
        print(f"[{self.name}] Sent message to {to_peer}: {content}")
        
        return Message.response({
            "status": "sent",
            "to_peer": to_peer,
            "message_count": len(self.messages),
        })
    
    async def _handle_broadcast(self, payload: dict) -> Message:
        content = payload.get("content", "")
        
        if not content:
            return Message.response({"error": "content required"})
        
        broadcast_count = 0
        for peer_id in self.peers:
            msg = MeshMessage(self.name, peer_id, content)
            self.messages.append(msg)
            broadcast_count += 1
        
        print(f"[{self.name}] Broadcast to {broadcast_count} peers: {content}")
        
        return Message.response({
            "status": "broadcast",
            "peers_reached": broadcast_count,
            "message_count": len(self.messages),
        })
    
    def _handle_get_peers(self) -> Message:
        peers = [{"peer_id": k, "peer_addr": v} for k, v in self.peers.items()]
        return Message.response({"peers": peers, "peer_count": len(peers)})
    
    def _handle_get_messages(self) -> Message:
        messages = [m.to_dict() for m in self.messages]
        return Message.response({"messages": messages, "message_count": len(messages)})
    
    async def _handle_receive(self, payload: dict) -> Message:
        from_peer = payload.get("from_peer", "")
        content = payload.get("content", "")
        
        msg = MeshMessage(from_peer, self.name, content)
        self.messages.append(msg)
        
        print(f"[{self.name}] Received message from {from_peer}: {content}")
        
        return Message.response({
            "status": "received",
            "from": from_peer,
            "content": content,
        })

async def main():
    actor = MeshActor()
    await actor.start()
    
    print(f"Starting {actor.name}...")
    print("Commands: register_peer, send_message, broadcast, get_peers, get_messages, receive")
    
    await actor.run()

if __name__ == "__main__":
    asyncio.run(main())
```

## Running the Example

### Start Multiple Nodes

```bash
# Terminal 1 - Node A
aether run --node-id node-a --listen 0.0.0.0:7000

# Terminal 2 - Node B
aether run --node-id node-b --listen 0.0.0.0:7001 --bootstrap node-a:7000

# Terminal 3 - Node C
aether run --node-id node-c --listen 0.0.0.0:7002 --bootstrap node-a:7000
```

### Test Communication

```bash
# Register peers
aether invoke mesh-actor '{"action": "register_peer", "peer_id": "node-b", "peer_addr": "localhost:7001"}'

# Send message
aether invoke mesh-actor '{"action": "send_message", "to_peer": "node-b", "content": "Hello from node-a!"}'

# Broadcast
aether invoke mesh-actor '{"action": "broadcast", "content": "Hello everyone!"}'

# Get peers
aether invoke mesh-actor '{"action": "get_peers"}'

# Get messages
aether invoke mesh-actor '{"action": "get_messages"}'
```

## Key Concepts

### Mesh Actions

| Action | Description |
|--------|-------------|
| `register_peer` | Register a new peer |
| `send_message` | Send message to specific peer |
| `broadcast` | Send message to all peers |
| `get_peers` | List registered peers |
| `get_messages` | Get message history |
| `receive` | Receive message from peer |

### Capabilities

| Capability | Description |
|------------|-------------|
| `NETWORK_OUTBOUND` | Connect to other nodes |
| `ACTOR_MESSAGING` | Send/receive actor messages |

### Message Format

```json
{
    "from": "mesh-actor@node-a",
    "to": "mesh-actor@node-b",
    "content": "Hello!",
    "timestamp": "2026-03-16T12:00:00Z"
}
```
