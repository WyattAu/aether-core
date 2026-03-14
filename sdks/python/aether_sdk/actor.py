from typing import Callable, Optional, Dict, Any
from abc import ABC, abstractmethod
import asyncio
import uuid

from .capabilities import Capability, CapabilitySet
from .messaging import Message, MessageType
from .state import StateHandle
from .exceptions import ActorNotFound, RpcError


class Actor(ABC):
    """Base class for Aether actors."""
    
    def __init__(self):
        self._capabilities: CapabilitySet = CapabilitySet()
        self._state: Optional[StateHandle] = None
        self._mailbox: asyncio.Queue = asyncio.Queue()
        self._pending_responses: Dict[str, asyncio.Future] = {}
        self._running: bool = False
    
    @classmethod
    @abstractmethod
    def name(cls) -> str:
        """Actor name for registration."""
        pass
    
    @abstractmethod
    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        """Handle incoming message."""
        pass
    
    async def on_start(self) -> None:
        """Called when actor starts."""
        pass
    
    async def on_stop(self) -> None:
        """Called when actor stops."""
        pass
    
    def require(self, *capabilities: Capability) -> None:
        """Declare required capabilities."""
        for cap in capabilities:
            self._capabilities.add(cap)
    
    async def send(self, target: str, message: Message) -> None:
        """Send message to another actor."""
        message.sender = self.name()
        await self._mailbox.put(("send", target, message))
    
    async def call(self, target: str, request: Any, timeout: float = 30.0) -> Any:
        """RPC call to another actor."""
        correlation_id = str(uuid.uuid4())
        message = Message(
            type=MessageType.RPC_REQUEST,
            payload=request,
            sender=self.name(),
            correlation_id=correlation_id,
        )
        
        future: asyncio.Future = asyncio.get_event_loop().create_future()
        self._pending_responses[correlation_id] = future
        
        await self.send(target, message)
        
        try:
            response = await asyncio.wait_for(future, timeout=timeout)
            return response
        except asyncio.TimeoutError:
            del self._pending_responses[correlation_id]
            raise RpcError(f"RPC call to {target} timed out", code="TIMEOUT")
    
    @property
    def state(self) -> StateHandle:
        """Get state handle."""
        if self._state is None:
            self._state = StateHandle()
        return self._state
    
    async def run(self) -> None:
        """Main actor loop."""
        self._running = True
        await self.on_start()
        
        try:
            while self._running:
                try:
                    item = await asyncio.wait_for(self._mailbox.get(), timeout=0.1)
                    await self._process_item(item)
                except asyncio.TimeoutError:
                    continue
        finally:
            await self.on_stop()
    
    async def _process_item(self, item: tuple) -> None:
        """Process a mailbox item."""
        action = item[0]
        
        if action == "send":
            _, target, message = item
            await self._dispatch_message(target, message)
        elif action == "receive":
            _, sender, message = item
            await self._handle_incoming(sender, message)
    
    async def _dispatch_message(self, target: str, message: Message) -> None:
        """Dispatch outgoing message."""
        pass
    
    async def _handle_incoming(self, sender: str, message: Message) -> None:
        """Handle incoming message."""
        if message.type == MessageType.RPC_RESPONSE and message.correlation_id:
            if message.correlation_id in self._pending_responses:
                future = self._pending_responses.pop(message.correlation_id)
                if not future.done():
                    future.set_result(message.payload)
            return
        
        response = await self.handle_message(sender, message)
        if response and message.correlation_id:
            response.type = MessageType.RPC_RESPONSE
            response.correlation_id = message.correlation_id
            response.sender = self.name()
    
    async def stop(self) -> None:
        """Stop the actor."""
        self._running = False
    
    def deliver(self, sender: str, message: Message) -> None:
        """Deliver a message to this actor's mailbox."""
        self._mailbox.put_nowait(("receive", sender, message))


def actor(cls: type) -> type:
    """Decorator to create an actor from a class."""
    
    class ActorWrapper(cls, Actor):
        @classmethod
        def name(cls) -> str:
            return getattr(cls, '_actor_name', cls.__name__.lower())
    
    ActorWrapper.__name__ = cls.__name__
    ActorWrapper.__qualname__ = cls.__qualname__
    ActorWrapper.__module__ = cls.__module__
    
    return ActorWrapper
