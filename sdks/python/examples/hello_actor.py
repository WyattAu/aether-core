from typing import Optional

from aether_sdk import Actor, Message, MessageType


class HelloActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "hello"

    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        if message.payload.get("type") == "greet":
            name = message.payload.get("name", "World")
            return Message(
                type=MessageType.CUSTOM, payload={"greeting": f"Hello, {name}!"}
            )
        return None


if __name__ == "__main__":
    import asyncio

    async def main():
        actor = HelloActor()

        test_msg = Message(
            type=MessageType.CUSTOM, payload={"type": "greet", "name": "Aether"}
        )
        response = await actor.handle_message("test", test_msg)
        if response:
            print(f"Response: {response.payload}")

    asyncio.run(main())
