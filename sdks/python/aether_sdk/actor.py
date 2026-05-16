"""Core Actor abstraction for the Aether SDK.

This module provides the base :class:`Actor` class and the :func:`actor`
decorator for building message-driven actors in the Aether framework.
Actors communicate asynchronously via messages, support RPC calls, and
manage local state through a :class:`~aether_sdk.state.StateHandle`.

Example:
    >>> from aether_sdk.actor import Actor, actor
    >>> from aether_sdk.messaging import Message, MessageType
    >>>
    >>> class GreetActor(Actor):
    ...     @classmethod
    ...     def name(cls) -> str:
    ...         return "greeter"
    ...
    ...     async def handle_message(self, sender, message):
    ...         return Message(type=MessageType.CUSTOM, payload=f"Hello, {message.payload}!")
"""

import asyncio
import uuid
from abc import ABC, abstractmethod
from typing import Any, Dict, Optional

from .capabilities import Capability, CapabilitySet
from .exceptions import RpcError
from .messaging import Message, MessageType
from .state import StateHandle


class Actor(ABC):
    """Base class for Aether actors.

    Actors are independent units of computation that communicate exclusively
    through asynchronous message passing. Each actor maintains its own
    private state and processes messages one at a time from its mailbox.

    Subclasses must implement :meth:`name` and :meth:`handle_message`.
    Optionally override :meth:`on_start` and :meth:`on_stop` for lifecycle
    hooks.

    Attributes:
        _capabilities: Set of capabilities granted to this actor.
        _state: Persistent state handle (lazily initialized).
        _mailbox: Async queue for incoming and outgoing messages.
        _pending_responses: Map of correlation IDs to futures for RPC calls.
        _running: Whether the actor's main loop is active.

    Example:
        >>> class MyActor(Actor):
        ...     @classmethod
        ...     def name(cls):
        ...         return "my_actor"
        ...
        ...     async def handle_message(self, sender, message):
        ...         print(f"From {sender}: {message.payload}")
    """

    def __init__(self):
        """Initialize the actor with default capabilities and an empty mailbox."""
        self._capabilities: CapabilitySet = CapabilitySet()
        self._state: Optional[StateHandle] = None
        self._mailbox: asyncio.Queue = asyncio.Queue()
        self._pending_responses: Dict[str, asyncio.Future] = {}
        self._running: bool = False

    @classmethod
    @abstractmethod
    def name(cls) -> str:
        """Return the unique name used for actor registration.

        Returns:
            A string identifier for this actor type.
        """
        pass

    @abstractmethod
    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        """Handle an incoming message.

        Subclasses must implement this method to define how the actor
        processes messages received from other actors.

        Args:
            sender: The name of the actor that sent the message.
            message: The message to process.

        Returns:
            An optional response message. If provided and the original
            message has a ``correlation_id``, the response will be
            routed back to the sender as an RPC response.
        """
        pass

    async def on_start(self) -> None:
        """Lifecycle hook called once when the actor starts.

        Override this to initialize resources such as database connections,
        timers, or subscriptions. The default implementation does nothing.
        """
        pass

    async def on_stop(self) -> None:
        """Lifecycle hook called when the actor stops.

        Override this to flush buffers, close connections, or release
        resources. The default implementation does nothing.
        """
        pass

    def require(self, *capabilities: Capability) -> None:
        """Declare capabilities required by this actor.

        Call this in the constructor to request permissions such as
        network access or state storage. Capabilities are enforced at
        runtime — attempting an operation without the required capability
        raises :class:`~aether_sdk.exceptions.CapabilityDenied`.

        Args:
            *capabilities: One or more :class:`~aether_sdk.capabilities.Capability`
                flags to request.

        Example:
            >>> self.require(Capability.NETWORK_OUTBOUND, Capability.STATE_READ)
        """
        for cap in capabilities:
            self._capabilities.add(cap)

    async def send(self, target: str, message: Message) -> None:
        """Send a fire-and-forget message to another actor.

        The message is placed in the actor's outbox for dispatch.
        The sender field is automatically set to ``self.name()``.

        Args:
            target: The registered name of the destination actor.
            message: The message to send.
        """
        message.sender = self.name()
        await self._mailbox.put(("send", target, message))

    async def call(self, target: str, request: Any, timeout: float = 30.0) -> Any:
        """Perform an RPC call to another actor and await the response.

        Creates a ``RPC_REQUEST`` message with a unique correlation ID and
        waits for the matching ``RPC_RESPONSE``. The caller is blocked
        until a response arrives or the timeout expires.

        Args:
            target: The registered name of the destination actor.
            request: The request payload to send.
            timeout: Maximum time in seconds to wait for a response.

        Returns:
            The response payload from the remote actor.

        Raises:
            RpcError: If the call times out before a response is received.
                The error's ``code`` attribute is ``"TIMEOUT"``.

        Example:
            >>> result = await self.call("calculator", {"op": "add", "a": 1, "b": 2})
        """
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
        """Get the state handle for this actor.

        Lazily creates a :class:`~aether_sdk.state.StateHandle` on first
        access. Use the returned handle to persist key-value data across
        message handling cycles.

        Returns:
            The actor's state handle.
        """
        if self._state is None:
            self._state = StateHandle()
        return self._state

    async def run(self) -> None:
        """Run the actor's main event loop.

        Calls :meth:`on_start`, then continuously processes mailbox items
        (both outgoing sends and incoming receives) until :meth:`stop` is
        called. Finally calls :meth:`on_stop`.

        Raises:
            Exception: Any exception raised during message processing will
                propagate after :meth:`on_stop` completes.
        """
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
        """Route a mailbox item to the appropriate handler.

        Args:
            item: A tuple whose first element is either ``"send"`` or
                ``"receive"`` followed by the relevant arguments.
        """
        action = item[0]

        if action == "send":
            _, target, message = item
            await self._dispatch_message(target, message)
        elif action == "receive":
            _, sender, message = item
            await self._handle_incoming(sender, message)

    async def _dispatch_message(self, target: str, message: Message) -> None:
        """Dispatch an outgoing message to the target actor.

        This is a stub — concrete implementations or the actor system
        should override this to perform actual message delivery.

        Args:
            target: Destination actor name.
            message: The message to deliver.
        """
        pass

    async def _handle_incoming(self, sender: str, message: Message) -> None:
        """Handle an incoming message from the mailbox.

        If the message is an RPC response with a known correlation ID,
        the pending future is resolved. Otherwise, the message is
        delegated to :meth:`handle_message`.

        Args:
            sender: Name of the sending actor.
            message: The received message.
        """
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
        """Signal the actor's main loop to exit.

        The loop will finish processing the current item, call
        :meth:`on_stop`, and then return from :meth:`run`.
        """
        self._running = False

    def deliver(self, sender: str, message: Message) -> None:
        """Deliver a message directly to this actor's mailbox.

        Unlike :meth:`send` (which places an outgoing item in the mailbox),
        this places an incoming item so it will be processed by
        :meth:`handle_message` on the next loop iteration.

        Args:
            sender: Name of the sending actor.
            message: The message to deliver.
        """
        self._mailbox.put_nowait(("receive", sender, message))


def actor(cls: type) -> type:
    """Decorator to turn a plain class into an Aether actor.

    The decorated class is wrapped so that it inherits from :class:`Actor`
    and provides a default :meth:`name` implementation based on the
    ``_actor_name`` class attribute or the lowercase class name.

    Args:
        cls: The class to decorate.

    Returns:
        A new class that subclasses both ``cls`` and :class:`Actor`.

    Example:
        >>> @actor
        ... class Echo:
        ...     _actor_name = "echo"
        ...
        ...     async def handle_message(self, sender, message):
        ...         return message
    """

    class ActorWrapper(cls, Actor):
        @classmethod
        def name(cls) -> str:
            return getattr(cls, "_actor_name", cls.__name__.lower())

    ActorWrapper.__name__ = cls.__name__
    ActorWrapper.__qualname__ = cls.__qualname__
    ActorWrapper.__module__ = cls.__module__

    return ActorWrapper
