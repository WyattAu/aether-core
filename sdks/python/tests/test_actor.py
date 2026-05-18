import asyncio
from typing import Optional
from uuid import uuid4

import pytest
from aether_sdk import Actor, Message, MessageType, actor
from aether_sdk.capabilities import Capability
from aether_sdk.exceptions import RpcError
from aether_sdk.state import StateHandle


class TestActor(Actor):
    @classmethod
    def name(cls) -> str:
        return "test_actor"

    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        if message.type == MessageType.CUSTOM:
            return Message(type=MessageType.CUSTOM, payload={"echo": message.payload})
        return None


class TestActorClass:
    def test_actor_name(self):
        assert TestActor.name() == "test_actor"

    def test_actor_has_mailbox(self):
        actor = TestActor()
        assert hasattr(actor, "_mailbox")

    def test_actor_has_capabilities(self):
        actor = TestActor()
        assert hasattr(actor, "_capabilities")

    def test_actor_initial_state(self):
        actor = TestActor()
        assert actor._running is False
        assert actor._state is None

    @pytest.mark.asyncio
    async def test_actor_on_start(self):
        actor = TestActor()
        await actor.on_start()  # Should not raise

    @pytest.mark.asyncio
    async def test_actor_on_stop(self):
        actor = TestActor()
        await actor.on_stop()  # Should not raise

    @pytest.mark.asyncio
    async def test_actor_handle_message(self):
        actor = TestActor()
        msg = Message(type=MessageType.CUSTOM, payload={"hello": "world"})
        response = await actor.handle_message("sender", msg)
        assert response is not None
        assert response.payload == {"echo": {"hello": "world"}}

    @pytest.mark.asyncio
    async def test_actor_handle_message_returns_none(self):
        actor = TestActor()
        msg = Message(type=MessageType.START, payload={})
        response = await actor.handle_message("sender", msg)
        assert response is None

    def test_actor_require_capability(self):
        actor = TestActor()
        actor.require(Capability.STATE_WRITE)

        # Just check that the capability was added
        assert actor._capabilities.has(Capability.STATE_WRITE)

    def test_actor_require_multiple_capabilities(self):
        actor = TestActor()
        actor.require(Capability.STATE_READ, Capability.STATE_WRITE)

        assert actor._capabilities.has(Capability.STATE_READ)
        assert actor._capabilities.has(Capability.STATE_WRITE)

    @pytest.mark.asyncio
    async def test_actor_state_property(self):
        actor = TestActor()
        state = actor.state
        assert isinstance(state, StateHandle)
        # Same instance on subsequent access
        state2 = actor.state
        assert state is state2

    @pytest.mark.asyncio
    async def test_actor_deliver_message(self):
        actor = TestActor()
        msg = Message(type=MessageType.CUSTOM, payload="test")
        actor.deliver("sender", msg)
        # Message should be in mailbox
        assert actor._mailbox.qsize() == 1

    @pytest.mark.asyncio
    async def test_actor_send_message(self):
        actor = TestActor()
        await actor.send("target", Message(type=MessageType.CUSTOM, payload="test"))
        # Message should be in mailbox with "send" action
        item = await actor._mailbox.get()
        assert item[0] == "send"
        assert item[1] == "target"

    @pytest.mark.asyncio
    async def test_actor_call_rpc_success(self):
        """Test successful RPC call with mock response."""
        actor = TestActor()

        # Create a pending response that will be resolved
        correlation_id = str(uuid4())

        # Create a response future
        loop = asyncio.get_event_loop()
        future = loop.create_future()
        actor._pending_responses[correlation_id] = future

        # Simulate receiving a response for a pending call
        response_msg = Message(
            type=MessageType.RPC_RESPONSE,
            payload={"result": "success"},
            correlation_id=correlation_id,
        )

        # Handle the incoming response
        await actor._handle_incoming("target", response_msg)

        # Now the future should be resolved
        assert future.done()
        assert future.result() == {"result": "success"}

    @pytest.mark.asyncio
    async def test_actor_call_rpc_timeout(self):
        """Test RPC call timeout."""
        actor = TestActor()
        # Make a call that will timeout
        with pytest.raises(RpcError) as exc_info:
            await actor.call("target", {"method": "test"}, timeout=0.01)
        assert "timed out" in str(exc_info).lower()

    @pytest.mark.asyncio
    async def test_actor_run_starts_and_stops(self):
        """Test actor run lifecycle."""
        actor = TestActor()
        started = []
        stopped = []

        original_on_start = actor.on_start
        original_on_stop = actor.on_stop

        async def on_start():
            started.append(True)
            await original_on_start()

        async def on_stop():
            stopped.append(True)
            await original_on_stop()

        actor.on_start = on_start
        actor.on_stop = on_stop

        # Start the actor
        asyncio.create_task(actor.run())
        # Wait for start
        await asyncio.sleep(0.1)
        assert started == [True]
        assert actor._running is True

        # Stop the actor
        await actor.stop()
        # Wait for stop
        await asyncio.sleep(0.1)
        assert stopped == [True]
        assert actor._running is False

    @pytest.mark.asyncio
    async def test_actor_process_item_send(self):
        """Test _process_item with send action."""
        actor = TestActor()
        dispatched = []

        async def mock_dispatch(target, message):
            dispatched.append((target, message))

        actor._dispatch_message = mock_dispatch

        # Put a send item in the mailbox
        await actor._mailbox.put(
            ("send", "target", Message(type=MessageType.CUSTOM, payload="test"))
        )

        # Process the item
        await actor._process_item(
            ("send", "target", Message(type=MessageType.CUSTOM, payload="test"))
        )

        assert len(dispatched) == 1
        assert dispatched[0][0] == "target"

    @pytest.mark.asyncio
    async def test_actor_process_item_receive(self):
        """Test _process_item with receive action."""
        actor = TestActor()
        handled = []

        original_handle_message = actor.handle_message

        async def mock_handle_message(sender, message):
            handled.append((sender, message))
            return await original_handle_message(sender, message)

        actor.handle_message = mock_handle_message

        # Put a receive item in the mailbox
        msg = Message(type=MessageType.CUSTOM, payload="test")
        await actor._process_item(("receive", "sender", msg))

        assert len(handled) == 1
        assert handled[0][0] == "sender"

    @pytest.mark.asyncio
    async def test_actor_handle_incoming_rpc_response(self):
        """Test _handle_incoming with RPC_RESPONSE."""
        actor = TestActor()

        # Create a pending response future
        loop = asyncio.get_event_loop()
        future = loop.create_future()

        correlation_id = str(uuid4())
        actor._pending_responses[correlation_id] = future

        # Handle incoming response
        response_msg = Message(
            type=MessageType.RPC_RESPONSE,
            payload={"result": "data"},
            correlation_id=correlation_id,
        )
        await actor._handle_incoming("sender", response_msg)

        # Future should be resolved
        assert future.done()
        assert future.result() == {"result": "data"}

        # Should be removed from pending
        assert correlation_id not in actor._pending_responses


class TestActorDecorator:
    def test_actor_decorator(self):
        @actor
        class MyActor:
            pass

        assert hasattr(MyActor, "name")
        assert MyActor.name() == "myactor"

    def test_actor_decorator_preserves_class_attributes(self):
        @actor
        class MyActor:
            """Test actor class."""

            pass

        # The decorator creates a wrapper class with the same name
        assert MyActor.__name__ == "MyActor"
        # qualname will include the test context

    def test_actor_decorator_creates_wrapper(self):
        @actor
        class MyActor:
            pass

        # The decorated class should have a name method
        assert hasattr(MyActor, "name")
        assert MyActor.name() == "myactor"

    def test_actor_decorator_name_from_class(self):
        @actor
        class CustomProcessor:
            pass

        assert CustomProcessor.name() == "customprocessor"

    def test_actor_decorator_with_underscore(self):
        @actor
        class my_custom_actor:
            pass

        # Should lowercase the class name
        assert my_custom_actor.name() == "my_custom_actor"
