import pytest
from aether_sdk import Actor, Message, MessageType, actor


class TestActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "test_actor"
    
    async def handle_message(self, sender: str, message: Message):
        if message.type == MessageType.CUSTOM:
            return Message(
                type=MessageType.CUSTOM,
                payload={"echo": message.payload}
            )
        return None


class TestActorClass:
    def test_actor_name(self):
        assert TestActor.name() == "test_actor"
    
    def test_actor_has_mailbox(self):
        actor = TestActor()
        assert hasattr(actor, '_mailbox')
    
    def test_actor_has_capabilities(self):
        actor = TestActor()
        assert hasattr(actor, '_capabilities')
    
    @pytest.mark.asyncio
    async def test_actor_on_start(self):
        actor = TestActor()
        await actor.on_start()
    
    @pytest.mark.asyncio
    async def test_actor_on_stop(self):
        actor = TestActor()
        await actor.on_stop()
    
    @pytest.mark.asyncio
    async def test_actor_handle_message(self):
        actor = TestActor()
        msg = Message(type=MessageType.CUSTOM, payload={"hello": "world"})
        response = await actor.handle_message("sender", msg)
        assert response is not None
        assert response.payload == {"echo": {"hello": "world"}}


class TestActorDecorator:
    def test_actor_decorator(self):
        @actor
        class MyActor:
            pass
        
        assert hasattr(MyActor, 'name')
        assert MyActor.name() == "myactor"
