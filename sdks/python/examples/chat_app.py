"""
Chat Application Example

Demonstrates a multi-actor chat room application with room and session actors.
"""
import asyncio
import json
import logging
import time
from typing import Any
from aether_sdk import Actor, Message, MessageType

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class User:
    """Represents a chat user."""
    
    def __init__(self, user_id: str, name: str, joined_at: str = ""):
        self.user_id = user_id
        self.name = name
        self.joined_at = joined_at or time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    
    def to_dict(self) -> dict:
        return {
            "user_id": self.user_id,
            "name": self.name,
            "joined_at": self.joined_at
        }


class ChatMessage:
    """Represents a chat message."""
    
    def __init__(self, msg_id: str, user_id: str, user_name: str, 
                 content: str, room_id: str, timestamp: str = ""):
        self.msg_id = msg_id
        self.user_id = user_id
        self.user_name = user_name
        self.content = content
        self.timestamp = timestamp or time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        self.room_id = room_id
    
    def to_dict(self) -> dict:
        return {
            "id": self.msg_id,
            "user_id": self.user_id,
            "user_name": self.user_name,
            "content": self.content,
            "timestamp": self.timestamp,
            "room_id": self.room_id
        }


class RoomActor(Actor):
    """Manages a single chat room."""
    
    def __init__(self, room_id: str, room_name: str):
        super().__init__(f"room-{room_id}")
        self.room_id = room_id
        self.room_name = room_name
        self.users: dict[str, User] = {}
        self.messages: list[ChatMessage] = []
        self.state_key = f"room_{room_id}_state"
        self.require("STATE_READ", "STATE_WRITE", "ACTOR_MESSAGING", "LOG", "TIME")
    
    async def on_start(self) -> None:
        logger.info(f"[{self.name}] Room '{self.room_name}' starting...")
        
        # Load persisted state
        data = await self.state.read(self.state_key)
        if data:
            try:
                state = json.loads(data)
                for u in state.get("users", []):
                    user = User(u["user_id"], u["name"], u["joined_at"])
                    self.users[user.user_id] = user
                for m in state.get("messages", []):
                    msg = ChatMessage(
                        m["id"], m["user_id"], m["user_name"],
                        m["content"], m["room_id"], m["timestamp"]
                    )
                    self.messages.append(msg)
                logger.info(f"[{self.name}] Restored {len(self.users)} users, {len(self.messages)} messages")
            except Exception as e:
                logger.error(f"[{self.name}] Failed to load state: {e}")
        else:
            await self._save_state()
            logger.info(f"[{self.name}] Initialized new room")
    
    async def on_stop(self) -> None:
        logger.info(f"[{self.name}] Room stopping, saving state...")
        await self._save_state()
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type not in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return None
        
        payload = message.payload
        if not isinstance(payload, dict):
            return Message.response({"error": "invalid payload"})
        
        action = payload.get("action", "")
        
        if action == "join":
            return await self._handle_join(payload)
        elif action == "leave":
            return await self._handle_leave(payload)
        elif action == "send":
            return await self._handle_send(payload)
        elif action == "history":
            return self._handle_history(payload)
        elif action == "users":
            return self._handle_users()
        elif action == "info":
            return self._handle_info()
        
        return Message.response({"error": f"unknown action: {action}"})
    
    async def _handle_join(self, payload: dict) -> Message:
        user_id = payload.get("user_id", "")
        user_name = payload.get("user_name", "")
        
        if not user_id or not user_name:
            return Message.response({"error": "user_id and user_name required"})
        
        user = User(user_id, user_name)
        self.users[user_id] = user
        await self._save_state()
        
        logger.info(f"[{self.name}] User '{user_name}' joined (total: {len(self.users)})")
        
        return Message.response({
            "action": "joined",
            "room_id": self.room_id,
            "room_name": self.room_name,
            "user_id": user_id,
            "user_count": len(self.users)
        })
    
    async def _handle_leave(self, payload: dict) -> Message:
        user_id = payload.get("user_id", "")
        
        user = self.users.pop(user_id, None)
        if user:
            await self._save_state()
            logger.info(f"[{self.name}] User '{user.name}' left (remaining: {len(self.users)})")
        
        return Message.response({
            "action": "left",
            "room_id": self.room_id,
            "user_id": user_id,
            "user_count": len(self.users)
        })
    
    async def _handle_send(self, payload: dict) -> Message:
        user_id = payload.get("user_id", "")
        content = payload.get("content", "")
        
        if not user_id or not content:
            return Message.response({"error": "user_id and content required"})
        
        user = self.users.get(user_id)
        if not user:
            return Message.response({"error": "user not in room, join first"})
        
        msg = ChatMessage(
            msg_id=f"msg-{int(time.time() * 1000000)}",
            user_id=user_id,
            user_name=user.name,
            content=content,
            room_id=self.room_id
        )
        
        self.messages.append(msg)
        await self._save_state()
        
        logger.info(f"[{self.name}] [{msg.msg_id[:12]}] {user.name}: {content[:30]}...")
        
        return Message.response({
            "action": "sent",
            "message_id": msg.msg_id,
            "message_count": len(self.messages)
        })
    
    def _handle_history(self, payload: dict) -> Message:
        limit = payload.get("limit", 50)
        start = max(0, len(self.messages) - limit)
        messages = [m.to_dict() for m in self.messages[start:]]
        
        return Message.response({
            "action": "history",
            "room_id": self.room_id,
            "messages": messages,
            "count": len(messages)
        })
    
    def _handle_users(self) -> Message:
        users = [u.to_dict() for u in self.users.values()]
        return Message.response({
            "action": "users",
            "room_id": self.room_id,
            "users": users,
            "count": len(users)
        })
    
    def _handle_info(self) -> Message:
        return Message.response({
            "action": "info",
            "room_id": self.room_id,
            "room_name": self.room_name,
            "user_count": len(self.users),
            "message_count": len(self.messages)
        })
    
    async def _save_state(self) -> None:
        state = {
            "room_id": self.room_id,
            "room_name": self.room_name,
            "users": [u.to_dict() for u in self.users.values()],
            "messages": [m.to_dict() for m in self.messages]
        }
        await self.state.write(self.state_key, json.dumps(state).encode())


class SessionActor(Actor):
    """Manages a user session across rooms."""
    
    def __init__(self, user_id: str, user_name: str):
        super().__init__(f"session-{user_id}")
        self.user_id = user_id
        self.user_name = user_name
        self.rooms: set[str] = set()
        self.require("ACTOR_MESSAGING", "LOG")
    
    async def on_start(self) -> None:
        logger.info(f"[{self.name}] Session started for user '{self.user_name}'")
    
    async def on_stop(self) -> None:
        logger.info(f"[{self.name}] Session ended for user '{self.user_name}'")
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type not in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return None
        
        payload = message.payload
        if not isinstance(payload, dict):
            return Message.response({"error": "invalid payload"})
        
        action = payload.get("action", "")
        
        if action == "status":
            return Message.response({
                "action": "status",
                "user_id": self.user_id,
                "user_name": self.user_name,
                "rooms": list(self.rooms)
            })
        elif action == "join_room":
            room_id = payload.get("room_id", "")
            self.rooms.add(room_id)
            logger.info(f"[{self.name}] Joined room '{room_id}' (total: {len(self.rooms)})")
            return Message.response({
                "action": "joined_room",
                "room_id": room_id,
                "room_count": len(self.rooms)
            })
        elif action == "leave_room":
            room_id = payload.get("room_id", "")
            self.rooms.discard(room_id)
            logger.info(f"[{self.name}] Left room '{room_id}' (remaining: {len(self.rooms)})")
            return Message.response({
                "action": "left_room",
                "room_id": room_id,
                "room_count": len(self.rooms)
            })
        
        return Message.response({"error": f"unknown action: {action}"})


class ChatApp:
    """Main chat application coordinating actors."""
    
    def __init__(self):
        self.room_actor: RoomActor | None = None
        self.session_actor: SessionActor | None = None
        self._running = False
    
    async def start(self) -> None:
        """Start the chat application."""
        # Create actors
        self.room_actor = RoomActor("general", "General Chat")
        self.session_actor = SessionActor("demo-user", "Demo User")
        
        # Start actors
        await self.room_actor.start()
        await self.session_actor.start()
        
        # Demo: Auto-join the room
        await self.session_actor.handle_message("system", Message.request({
            "action": "join_room",
            "room_id": "general"
        }))
        
        await self.room_actor.handle_message("system", Message.request({
            "action": "join",
            "user_id": "demo-user",
            "user_name": "Demo User"
        }))
        
        self._running = True
        logger.info("=== Aether Chat Application ===")
        logger.info("Room: 'General Chat', User: 'Demo User'")
        logger.info("Commands: join, leave, send, history, users, info, status, quit")
    
    async def stop(self) -> None:
        """Stop the chat application."""
        self._running = False
        if self.room_actor:
            await self.room_actor.stop()
        if self.session_actor:
            await self.session_actor.stop()
    
    async def demo(self) -> None:
        """Run a demo of the chat application."""
        await asyncio.sleep(0.5)
        
        # Send a welcome message
        if self.room_actor:
            await self.room_actor.handle_message("demo-user", Message.request({
                "action": "send",
                "user_id": "demo-user",
                "content": "Hello, Aether Chat!"
            }))
        
        await asyncio.sleep(0.3)
        
        # Get room info
        if self.room_actor:
            info = await self.room_actor.handle_message("demo-user", Message.request({
                "action": "info"
            }))
            logger.info(f"Room info: {info.payload}")
        
        await asyncio.sleep(0.3)
        
        # Get message history
        if self.room_actor:
            history = await self.room_actor.handle_message("demo-user", Message.request({
                "action": "history",
                "limit": 10
            }))
            logger.info(f"Message count: {history.payload.get('count', 0)}")
        
        # Keep running
        while self._running:
            await asyncio.sleep(1)


async def main():
    """Run the chat application."""
    app = ChatApp()
    
    try:
        await app.start()
        await app.demo()
    except asyncio.CancelledError:
        logger.info("Application cancelled")
    finally:
        await app.stop()
        logger.info("Application stopped")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Shutting down...")
