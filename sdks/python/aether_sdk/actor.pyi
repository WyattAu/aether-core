"""Type stubs for actor module."""

from typing import Callable, Optional, Dict, Any
from abc import ABC, abstractmethod
import asyncio
from .capabilities import Capability, CapabilitySet
from .messaging import Message, MessageType
from .state import StateHandle
from .exceptions import ActorNotFound, RpcError

class Actor(ABC):
    """Base class for Aether actors."""
    
    _capabilities: CapabilitySet
    _state: Optional[StateHandle]
    _mailbox: asyncio.Queue[tuple[str, str, Message]]
    _pending_responses: Dict[str, asyncio.Future[Any]]
    _running: bool
    
    def __init__(self) -> None: ...
    
    @classmethod
    @abstractmethod
    def name(cls) -> str:
        """Actor name for registration."""
        ...
    
    @abstractmethod
    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        """Handle incoming message."""
        ...
    
    async def on_start(self) -> None:
        """Called when actor starts."""
        ...
    
    async def on_stop(self) -> None:
        """Called when actor stops."""
        ...
    
    def require(self, *capabilities: Capability) -> None:
        """Declare required capabilities."""
        ...
    
    async def send(self, target: str, message: Message) -> None:
        """Send message to another actor."""
        ...
    
    async def call(self, target: str, request: Any, timeout: float = 30.0) -> Any:
        """RPC call to another actor."""
        ...
    
    @property
    def state(self) -> StateHandle:
        """Get state handle."""
        ...
    
    async def run(self) -> None:
        """Main actor loop."""
        ...
    
    async def _process_item(self, item: tuple) -> None:
        """Process a mailbox item."""
        ...
    
    async def _dispatch_message(self, target: str, message: Message) -> None:
        """Dispatch outgoing message."""
        ...
    
    async def _handle_incoming(self, sender: str, message: Message) -> None:
        """Handle incoming message."""
        ...
    
    async def stop(self) -> None:
        """Stop the actor."""
        ...
    
    def deliver(self, sender: str, message: Message) -> None:
        """Deliver a message to this actor's mailbox."""
        ...

def actor(cls: type) -> type:
    """Decorator to create an actor from a class."""
    ...
