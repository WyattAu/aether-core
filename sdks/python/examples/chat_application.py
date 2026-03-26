"""
Real-Time Chat Application using the Aether Actor Model.

This example demonstrates:
  - Actor-based chat rooms (each room is an actor)
  - Message routing between users
  - State management (room members, message history)
  - Capabilities (messaging capability required)
  - Error handling (unknown rooms, full rooms)

Each ChatRoom is an Actor that manages a list of members and a message
history. Users interact with rooms by sending Message objects. The
actor system dispatches messages synchronously in-process to keep the
example self-contained and runnable without a distributed runtime.

Usage:
    python chat_application.py
"""

import asyncio
from typing import Optional

from aether_sdk.actor import Actor
from aether_sdk.messaging import Message, MessageType
from aether_sdk.state import StateHandle
from aether_sdk.capabilities import Capability


MAX_ROOM_MEMBERS = 5


class ChatRoom(Actor):
    """
    An actor that represents a single chat room.

    Each ChatRoom maintains:
      - A set of joined members (stored in state as JSON)
      - A list of past messages (stored in state as JSON)

    Capabilities:
      - ACTOR_MESSAGING: required to send/receive messages
      - STATE_READ / STATE_WRITE: required to persist room state
    """

    def __init__(self, room_name: str):
        super().__init__()
        self._room_name = room_name
        self.require(
            Capability.ACTOR_MESSAGING,
            Capability.STATE_READ,
            Capability.STATE_WRITE,
        )

    @classmethod
    def name(cls) -> str:
        return "chat_room"

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    async def on_start(self) -> None:
        await self.state.set_json("members", [])
        await self.state.set_json("messages", [])
        print(f"[Room '{self._room_name}'] Started.")

    # ------------------------------------------------------------------
    # Message handling
    # ------------------------------------------------------------------

    async def handle_message(
        self, sender: str, message: Message
    ) -> Optional[Message]:
        action = message.payload.get("action")

        if action == "join":
            return await self._handle_join(sender, message)
        elif action == "leave":
            return await self._handle_leave(sender, message)
        elif action == "say":
            return await self._handle_say(sender, message)
        elif action == "history":
            return await self._handle_history()
        elif action == "members":
            return await self._handle_members()
        else:
            return Message(
                type=MessageType.CUSTOM,
                payload={"error": f"Unknown action: {action}"},
            )

    # ------------------------------------------------------------------
    # Action handlers
    # ------------------------------------------------------------------

    async def _handle_join(self, sender: str, message: Message) -> Message:
        members: list = (await self.state.get_json("members")) or []

        if sender in members:
            return Message(
                type=MessageType.CUSTOM,
                payload={"info": f"'{sender}' is already in the room."},
            )

        if len(members) >= MAX_ROOM_MEMBERS:
            return Message(
                type=MessageType.CUSTOM,
                payload={"error": f"Room is full ({MAX_ROOM_MEMBERS} members max)."},
            )

        members.append(sender)
        await self.state.set_json("members", members)

        await self._append_message("system", f"'{sender}' joined the room.")

        print(f"[Room '{self._room_name}'] {sender} joined. ({len(members)} members)")
        return Message(
            type=MessageType.CUSTOM,
            payload={"info": f"'{sender}' joined '{self._room_name}'."},
        )

    async def _handle_leave(self, sender: str, message: Message) -> Message:
        members: list = (await self.state.get_json("members")) or []

        if sender not in members:
            return Message(
                type=MessageType.CUSTOM,
                payload={"error": f"'{sender}' is not in the room."},
            )

        members.remove(sender)
        await self.state.set_json("members", members)

        await self._append_message("system", f"'{sender}' left the room.")

        print(f"[Room '{self._room_name}'] {sender} left. ({len(members)} members)")
        return Message(
            type=MessageType.CUSTOM,
            payload={"info": f"'{sender}' left '{self._room_name}'."},
        )

    async def _handle_say(self, sender: str, message: Message) -> Message:
        members: list = (await self.state.get_json("members")) or []

        if sender not in members:
            return Message(
                type=MessageType.CUSTOM,
                payload={"error": f"'{sender}' is not a member of the room."},
            )

        text = message.payload.get("text", "")
        if not text:
            return Message(
                type=MessageType.CUSTOM,
                payload={"error": "Message text cannot be empty."},
            )

        await self._append_message(sender, text)
        print(f"[Room '{self._room_name}'] {sender}: {text}")

        return Message(
            type=MessageType.CUSTOM,
            payload={"info": "Message delivered."},
        )

    async def _handle_history(self) -> Message:
        messages: list = (await self.state.get_json("messages")) or []
        return Message(
            type=MessageType.CUSTOM,
            payload={"history": messages},
        )

    async def _handle_members(self) -> Message:
        members: list = (await self.state.get_json("members")) or []
        return Message(
            type=MessageType.CUSTOM,
            payload={"members": list(members)},
        )

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    async def _append_message(self, sender: str, text: str) -> None:
        messages: list = (await self.state.get_json("messages")) or []
        messages.append({"sender": sender, "text": text})
        await self.state.set_json("messages", messages)


class ChatServer:
    """
    A simple in-process router that simulates the actor system.

    In a real deployment the actor system handles routing. Here we
    maintain a registry of ChatRoom actors and deliver messages
    directly to their ``deliver`` method.
    """

    def __init__(self):
        self._rooms: dict[str, ChatRoom] = {}

    async def create_room(self, room_name: str) -> ChatRoom:
        room = ChatRoom(room_name)
        self._rooms[room_name] = room
        await room.on_start()
        return room

    async def send(self, room_name: str, sender: str, message: Message) -> Optional[Message]:
        room = self._rooms.get(room_name)
        if room is None:
            return Message(
                type=MessageType.CUSTOM,
                payload={"error": f"Room '{room_name}' does not exist."},
            )
        return await room.handle_message(sender, message)


async def main() -> None:
    print("=" * 60)
    print("  Aether SDK - Real-Time Chat Application Example")
    print("=" * 60)
    print()

    server = ChatServer()

    # Create a chat room
    await server.create_room("general")
    print()

    # --- Scenario 1: Users join the room ---
    print("--- Users Joining ---")
    for user in ["alice", "bob", "charlie"]:
        resp = await server.send(
            "general",
            user,
            Message(type=MessageType.CUSTOM, payload={"action": "join"}),
        )
        print(f"  -> {user}: {resp.payload}")
    print()

    # --- Scenario 2: Chat messages ---
    print("--- Chat Messages ---")
    chat_messages = [
        ("alice", "Hey everyone! Welcome to the room."),
        ("bob", "Thanks Alice! Happy to be here."),
        ("charlie", "This actor model is pretty cool."),
        ("alice", "Agreed! Each room is its own actor with isolated state."),
    ]
    for sender, text in chat_messages:
        resp = await server.send(
            "general",
            sender,
            Message(type=MessageType.CUSTOM, payload={"action": "say", "text": text}),
        )
        print(f"  -> {sender}: {resp.payload}")
    print()

    # --- Scenario 3: Retrieve message history ---
    print("--- Message History ---")
    resp = await server.send(
        "general",
        "alice",
        Message(type=MessageType.CUSTOM, payload={"action": "history"}),
    )
    for msg in resp.payload["history"]:
        print(f"  [{msg['sender']}] {msg['text']}")
    print()

    # --- Scenario 4: List members ---
    print("--- Room Members ---")
    resp = await server.send(
        "general",
        "alice",
        Message(type=MessageType.CUSTOM, payload={"action": "members"}),
    )
    print(f"  Members: {resp.payload['members']}")
    print()

    # --- Scenario 5: User leaves ---
    print("--- User Leaves ---")
    resp = await server.send(
        "general",
        "bob",
        Message(type=MessageType.CUSTOM, payload={"action": "leave"}),
    )
    print(f"  -> bob: {resp.payload}")
    print()

    # --- Scenario 6: Error - non-member tries to speak ---
    print("--- Error: Non-member speaks ---")
    resp = await server.send(
        "general",
        "bob",
        Message(type=MessageType.CUSTOM, payload={"action": "say", "text": "Can I still talk?"}),
    )
    print(f"  -> bob: {resp.payload}")
    print()

    # --- Scenario 7: Error - room does not exist ---
    print("--- Error: Unknown room ---")
    resp = await server.send(
        "random",
        "alice",
        Message(type=MessageType.CUSTOM, payload={"action": "join"}),
    )
    print(f"  -> alice: {resp.payload}")
    print()

    # --- Scenario 8: Error - room is full ---
    print("--- Error: Room full ---")
    await server.create_room("small", )
    await server.send("small", "u1", Message(type=MessageType.CUSTOM, payload={"action": "join"}))
    await server.send("small", "u2", Message(type=MessageType.CUSTOM, payload={"action": "join"}))
    await server.send("small", "u3", Message(type=MessageType.CUSTOM, payload={"action": "join"}))
    await server.send("small", "u4", Message(type=MessageType.CUSTOM, payload={"action": "join"}))
    await server.send("small", "u5", Message(type=MessageType.CUSTOM, payload={"action": "join"}))
    resp = await server.send("small", "u6", Message(type=MessageType.CUSTOM, payload={"action": "join"}))
    print(f"  -> u6: {resp.payload}")
    print()

    print("=" * 60)
    print("  Chat application demo complete!")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
