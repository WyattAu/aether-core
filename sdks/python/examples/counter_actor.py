"""
Stateful Counter Actor Example

Demonstrates persistent state that survives actor restarts.
"""

import asyncio
import json
import logging
from datetime import datetime

from aether_sdk import Actor, Message

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class CounterState:
    """Persistent state for the counter actor."""

    def __init__(self, value: int = 0, last_updated: str = ""):
        self.value = value
        self.last_updated = last_updated

    def to_dict(self) -> dict:
        return {"value": self.value, "last_updated": self.last_updated}

    @classmethod
    def from_dict(cls, data: dict) -> "CounterState":
        return cls(
            value=data.get("value", 0), last_updated=data.get("last_updated", "")
        )


class CounterActor(Actor):
    """A stateful counter actor with persistent state."""

    def __init__(self):
        super().__init__("counter-actor")
        self.state_key = "counter_state"
        self.state_data = CounterState()
        self.require("STATE_READ", "STATE_WRITE", "ACTOR_MESSAGING", "LOG")

    async def on_start(self) -> None:
        """Load persisted state on startup."""
        logger.info(f"[{self.name}] Starting counter actor...")

        # Load existing state
        data = await self.state.read(self.state_key)
        if data:
            self.state_data = CounterState.from_dict(json.loads(data))
            logger.info(f"[{self.name}] Restored state: value={self.state_data.value}")
        else:
            await self._save_state()
            logger.info(f"[{self.name}] Initialized new counter state")

    async def on_stop(self) -> None:
        """Save state on shutdown."""
        logger.info(f"[{self.name}] Counter actor stopping")

    async def handle_message(self, sender: str, message: Message) -> Message | None:
        """Handle counter commands."""
        payload = message.payload

        if isinstance(payload, str):
            return await self._handle_string_command(payload)
        elif isinstance(payload, dict):
            return await self._handle_dict_command(payload)
        else:
            return Message.response(
                {"error": "unknown payload type", "value": self.state_data.value}
            )

    async def _handle_string_command(self, command: str) -> Message:
        """Handle string commands."""
        commands = {
            "increment": self._increment,
            "decrement": self._decrement,
            "reset": self._reset,
            "get": self._get,
        }

        handler = commands.get(command)
        if handler:
            await handler()
            return Message.response({"action": command, "value": self.state_data.value})

        return Message.response(
            {
                "error": f"unknown command: {command}",
                "value": self.state_data.value,
                "usage": "commands: increment, decrement, reset, get",
            }
        )

    async def _handle_dict_command(self, payload: dict) -> Message:
        """Handle dict commands with parameters."""
        command = payload.get("command", "")

        if command == "add":
            amount = payload.get("amount", 0)
            self.state_data.value += amount
            await self._save_state()
            return Message.response(
                {"action": "add", "amount": amount, "value": self.state_data.value}
            )

        elif command == "subtract":
            amount = payload.get("amount", 0)
            self.state_data.value -= amount
            await self._save_state()
            return Message.response(
                {"action": "subtract", "amount": amount, "value": self.state_data.value}
            )

        elif command == "set":
            value = payload.get("value", 0)
            self.state_data.value = value
            await self._save_state()
            return Message.response({"action": "set", "value": self.state_data.value})

        # Fall back to string command handler
        return await self._handle_string_command(command)

    async def _increment(self) -> None:
        self.state_data.value += 1
        await self._save_state()

    async def _decrement(self) -> None:
        self.state_data.value -= 1
        await self._save_state()

    async def _reset(self) -> None:
        self.state_data.value = 0
        await self._save_state()

    async def _get(self) -> None:
        pass  # No state change needed

    async def _save_state(self) -> None:
        """Persist state to storage."""
        self.state_data.last_updated = datetime.utcnow().isoformat()
        data = json.dumps(self.state_data.to_dict())
        await self.state.write(self.state_key, data.encode())


async def main():
    """Run the counter actor."""
    actor = CounterActor()

    try:
        await actor.start()
        logger.info(f"Starting {actor.name}...")
        logger.info("Commands: increment, decrement, reset, get, add, subtract, set")
        await actor.run()
    except asyncio.CancelledError:
        logger.info("Actor cancelled")
    finally:
        await actor.stop()
        logger.info("Actor stopped")


if __name__ == "__main__":
    asyncio.run(main())
