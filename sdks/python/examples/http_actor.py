from aether_sdk import Actor, Message, MessageType, Capability, HttpClient


class HttpActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "http_actor"
    
    def __init__(self):
        super().__init__()
        self.require(Capability.NETWORK_OUTBOUND)
        self._http_client: HttpClient = None
    
    async def on_start(self) -> None:
        self._http_client = HttpClient(self._capabilities)
    
    async def on_stop(self) -> None:
        if self._http_client:
            await self._http_client.close()
    
    async def handle_message(self, sender: str, message: Message) -> Message:
        if message.type == MessageType.CUSTOM:
            action = message.payload.get("action")
            
            if action == "fetch":
                url = message.payload.get("url")
                response = await self._http_client.get(url)
                data = await response.text()
                
                return Message(
                    type=MessageType.CUSTOM,
                    payload={"status": response.status, "data": data[:100]}
                )
        
        return None


if __name__ == "__main__":
    import asyncio
    
    async def main():
        actor = HttpActor()
        await actor.on_start()
        
        test_msg = Message(
            type=MessageType.CUSTOM,
            payload={"action": "fetch", "url": "https://httpbin.org/get"}
        )
        
        try:
            response = await actor.handle_message("test", test_msg)
            print(f"Response status: {response.payload.get('status')}")
        finally:
            await actor.on_stop()
    
    asyncio.run(main())
