from aether_sdk import Actor, Message, MessageType, Capability


class StatefulActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "stateful"
    
    def __init__(self):
        super().__init__()
        self.require(Capability.STATE_READ, Capability.STATE_WRITE)
    
    async def on_start(self) -> None:
        count = await self.state.get_json("counter")
        if count is None:
            await self.state.set_json("counter", 0)
    
    async def handle_message(self, sender: str, message: Message) -> Message:
        if message.type == MessageType.CUSTOM:
            action = message.payload.get("action")
            
            if action == "increment":
                count = await self.state.get_json("counter") or 0
                count += 1
                await self.state.set_json("counter", count)
                
                return Message(
                    type=MessageType.CUSTOM,
                    payload={"counter": count}
                )
            
            elif action == "get":
                count = await self.state.get_json("counter") or 0
                
                return Message(
                    type=MessageType.CUSTOM,
                    payload={"counter": count}
                )
            
            elif action == "reset":
                await self.state.set_json("counter", 0)
                
                return Message(
                    type=MessageType.CUSTOM,
                    payload={"counter": 0}
                )
        
        return None


if __name__ == "__main__":
    import asyncio
    
    async def main():
        actor = StatefulActor()
        await actor.on_start()
        
        for i in range(3):
            msg = Message(
                type=MessageType.CUSTOM,
                payload={"action": "increment"}
            )
            response = await actor.handle_message("test", msg)
            print(f"After increment {i+1}: counter = {response.payload['counter']}")
        
        msg = Message(
            type=MessageType.CUSTOM,
            payload={"action": "get"}
        )
        response = await actor.handle_message("test", msg)
        print(f"Final counter value: {response.payload['counter']}")
    
    asyncio.run(main())
