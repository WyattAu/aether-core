"""
Stream Actor Base Class

Extends the base Actor with stream processing capabilities:
- Event-time processing with watermarks
- Windowed aggregation
- Backpressure handling
- State management for streaming

Example:
    >>> from aether_sdk.streaming.stream_actor import StreamActor
    >>> from aether_sdk.streaming.types import StreamEvent, StreamConfig
    >>>
    >>> class MyProcessor(StreamActor[str, dict]):
    ...     @classmethod
    ...     def name(cls):
    ...         return "my-processor"
    ...
    ...     async def process_event(self, event: StreamEvent[dict]):
    ...         await self.emit("output", event.value)
"""

from __future__ import annotations
from abc import abstractmethod
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, Generic, List, Optional, TypeVar, Union
import asyncio
import inspect

from ..actor import Actor
from ..messaging import Message, MessageType
from ..state import StateHandle
from .types import (
    Duration,
    Timestamp,
    StreamEvent,
    Watermark,
    WindowSpec,
    WindowInfo,
    WindowType,
    PaneInfo,
    StreamConfig,
    BackpressureConfig,
    PartitionConfig,
    DeliveryConfig,
    LateDataPolicy,
    WatermarkStrategy,
    BackpressureStrategy,
    DeliverySemantics,
)
from .window import (
    WindowAssigner,
    WindowTrigger,
    WindowState,
    TumblingWindow,
    SlidingWindow,
    SessionWindow,
)
from .backpressure import (
    BackpressureController,
    BackpressureStats,
    BackpressureError,
    BufferFullError,
    MultiLevelBackpressure,
    RateBasedBackpressure,
)
from ..state import StateHandle
from .types import (
    Duration,
    Timestamp,
    StreamEvent,
    Watermark,
    WindowSpec,
    WindowInfo,
    StreamConfig,
    BackpressureConfig,
    BackpressureStrategy,
    WatermarkStrategy,
    LateDataPolicy,
)
from .window import WindowAssigner, WindowTrigger, WindowState
from .backpressure import BackpressureController, BufferFullError

K = TypeVar('K')
V = TypeVar('V')
R = TypeVar('R')


@dataclass
class StreamState:
    """Internal state tracked by a :class:`StreamActor`.

    Attributes:
        watermarks: Current watermark per stream ID or event type.
        processed_count: Total number of events processed.
        late_events_count: Total number of late events received.
        last_processed_timestamp: Timestamp of the most recently
            processed event.
    """
    watermarks: Dict[str, Timestamp] = field(default_factory=dict)
    processed_count: int = 0
    late_events_count: int = 0
    last_processed_timestamp: Optional[Timestamp] = None


class StreamingStateHandle:
    """Enhanced state handle for streaming operations.

    Provides typed state access methods commonly needed in stream
    processing: value state, list state, and map state.

    Example:
        >>> ssh = StreamingStateHandle(base_state_handle)
        >>> await ssh.set_value("counter", 0)
        >>> counter = await ssh.get_value("counter")
    """

    def __init__(self, state: StateHandle):
        """Initialize with a base :class:`StateHandle`.

        Args:
            state: The underlying state handle.
        """
        self._state = state

    async def get_value(self, name: str, default: Any = None) -> Any:
        """Get a single value from state.

        Args:
            name: State key name.
            default: Default value if the key is not found.

        Returns:
            The stored value or *default*.
        """
        value = await self._state.get_json(name)
        return value if value is not None else default

    async def set_value(self, name: str, value: Any) -> None:
        """Set a single value in state.

        Args:
            name: State key name.
            value: Value to store (must be JSON-serializable).
        """
        await self._state.set_json(name, value)

    async def get_list(self, name: str) -> List[Any]:
        """Get a list from state.

        Args:
            name: State key name.

        Returns:
            The stored list or an empty list if not found.
        """
        value = await self._state.get_json(name)
        if value is None:
            return []
        return value if isinstance(value, list) else [value]

    async def append_to_list(self, name: str, item: Any) -> None:
        """Append an item to a list in state.

        If the key does not exist, a new list is created.

        Args:
            name: State key name.
            item: Item to append.
        """
        lst = await self.get_list(name)
        lst.append(item)
        await self._state.set_json(name, lst)

    async def clear_list(self, name: str) -> None:
        """Clear a list in state (sets it to an empty list).

        Args:
            name: State key name.
        """
        await self._state.set_json(name, [])

    async def get_map(self, name: str) -> Dict[str, Any]:
        """Get a map from state.

        Args:
            name: State key name.

        Returns:
            The stored dict or an empty dict if not found.
        """
        value = await self._state.get_json(name)
        if value is None:
            return {}
        return value if isinstance(value, dict) else {}

    async def put_in_map(self, name: str, key: str, value: Any) -> None:
        """Put a key-value pair into a map in state.

        Args:
            name: State key name.
            key: Map key.
            value: Map value.
        """
        m = await self.get_map(name)
        m[key] = value
        await self._state.set_json(name, m)

    async def remove_from_map(self, name: str, key: str) -> Optional[Any]:
        """Remove a key from a map in state.

        Args:
            name: State key name.
            key: Map key to remove.

        Returns:
            The removed value, or ``None`` if the key was not present.
        """
        m = await self.get_map(name)
        value = m.pop(key, None)
        await self._state.set_json(name, m)
        return value

    async def clear_map(self, name: str) -> None:
        """Clear a map in state (sets it to an empty dict).

        Args:
            name: State key name.
        """
        await self._state.set_json(name, {})


class StreamActor(Actor, Generic[K, V]):
    """Base class for stream processing actors.

    Extends :class:`~aether_sdk.actor.Actor` with event-time processing,
    windowed aggregation, backpressure handling, and Flink-style state
    access.

    Subclasses must implement :meth:`process_event`. Optionally override
    :meth:`on_start` and :meth:`on_stop` for lifecycle hooks.

    Example:
        >>> class MyProcessor(StreamActor[str, Event]):
        ...     @classmethod
        ...     def name(cls):
        ...         return "my-processor"
        ...
        ...     async def process_event(self, event: StreamEvent[Event]):
        ...         await self.emit("output", event.value)
    """

    def __init__(
        self,
        config: Optional[StreamConfig] = None,
        backpressure_config: Optional[BackpressureConfig] = None,
    ):
        """Initialize the stream actor.

        Args:
            config: Optional stream configuration.
            backpressure_config: Optional backpressure configuration.
        """
        super().__init__()

        self._stream_config = config or StreamConfig()
        self._stream_state = StreamState()
        self._streaming_state: Optional[StreamingStateHandle] = None

        self._backpressure = BackpressureController(
            backpressure_config or BackpressureConfig()
        )

        self._window_assigner: Optional[WindowAssigner[K, V]] = None
        self._window_trigger: Optional[WindowTrigger[K, V, Any]] = None

        self._output_handlers: Dict[str, Callable] = {}

        self._late_data_handler: Optional[Callable] = None

    @property
    def stream_state(self) -> StreamingStateHandle:
        """Get the streaming state handle (lazily created).

        Returns:
            A :class:`StreamingStateHandle` backed by the actor's
            :attr:`~Actor.state`.
        """
        if self._streaming_state is None:
            self._streaming_state = StreamingStateHandle(self.state)
        return self._streaming_state

    @property
    def backpressure(self) -> BackpressureController:
        """Get the backpressure controller.

        Returns:
            The :class:`BackpressureController` instance.
        """
        return self._backpressure

    @property
    def stream_config(self) -> StreamConfig:
        """Get the stream configuration.

        Returns:
            The :class:`StreamConfig` for this actor.
        """
        return self._stream_config

    @abstractmethod
    async def process_event(self, event: StreamEvent[V]) -> None:
        """Process a single stream event.

        Subclasses must implement this method to define event
        processing logic.

        Args:
            event: The stream event to process.
        """
        pass

    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        """Handle an incoming message (overrides Actor.handle_message).

        Dispatches ``STREAM_EVENT`` and ``WATERMARK`` messages to the
        appropriate internal handlers.

        Args:
            sender: Name of the sending actor.
            message: The received message.

        Returns:
            Always ``None`` for stream actors.
        """
        if message.type == MessageType.STREAM_EVENT:
            event_data = message.payload
            if isinstance(event_data, StreamEvent):
                await self._process_with_backpressure(event_data)
            elif isinstance(event_data, dict):
                event = self._dict_to_event(event_data)
                if event:
                    await self._process_with_backpressure(event)

        elif message.type == MessageType.WATERMARK:
            watermark_data = message.payload
            if isinstance(watermark_data, Watermark):
                await self.advance_watermark(watermark_data)
            elif isinstance(watermark_data, dict):
                watermark = Watermark(
                    timestamp=Timestamp(watermark_data.get('timestamp', 0)),
                    stream_id=watermark_data.get('stream_id', ''),
                    partition=watermark_data.get('partition'),
                )
                await self.advance_watermark(watermark)

        return None

    def _dict_to_event(self, data: dict) -> Optional[StreamEvent[V]]:
        """Reconstruct a :class:`StreamEvent` from a plain dict.

        Args:
            data: Dictionary with event fields.

        Returns:
            A :class:`StreamEvent`, or ``None`` if reconstruction
            fails.
        """
        try:
            return StreamEvent(
                key=data.get('key', ''),
                value=data.get('value'),
                timestamp=Timestamp(data.get('timestamp', 0)),
                headers=data.get('headers', {}),
                partition=data.get('partition'),
                offset=data.get('offset'),
                event_type=data.get('event_type'),
            )
        except (KeyError, TypeError):
            return None

    async def _process_with_backpressure(self, event: StreamEvent[V]) -> None:
        """Push an event through the backpressure controller and process it."""
        if not self._backpressure.try_push(event):
            return

        while True:
            buffered_event = self._backpressure.pop()
            if buffered_event is None:
                break

            try:
                await self._process_event_internal(buffered_event)
            except Exception as e:
                print(f"Error processing event: {e}")

    async def _process_event_internal(self, event: StreamEvent[V]) -> None:
        """Internal event processing with watermark and window handling."""
        self._stream_state.processed_count += 1

        current_watermark = self._stream_state.watermarks.get(
            event.event_type or 'default',
            Timestamp(0)
        )

        if event.timestamp < current_watermark:
            self._stream_state.late_events_count += 1
            await self._handle_late_event(event)
            return

        if self._window_trigger:
            key = self._extract_key(event)
            results = self._window_trigger.process(event, key)

            for result in results:
                await self.emit("window_output", result)

        await self.process_event(event)

        self._stream_state.last_processed_timestamp = event.timestamp

    def _extract_key(self, event: StreamEvent[V]) -> K:
        """Extract the windowing key from an event.

        Args:
            event: The stream event.

        Returns:
            The key to use for window assignment.
        """
        return event.key

    async def _handle_late_event(self, event: StreamEvent[V]) -> None:
        """Handle a late-arriving event according to the configured policy."""
        policy = self._stream_config.late_data_policy

        if policy == LateDataPolicy.DROP:
            return

        elif policy == LateDataPolicy.SIDE_OUTPUT:
            if self._late_data_handler:
                await self._late_data_handler(event)
            elif self._stream_config.late_data_output:
                await self.emit(self._stream_config.late_data_output, event)

        elif policy == LateDataPolicy.REPROCESS:
            if self._window_assigner:
                key = self._extract_key(event)
                self._window_assigner.assign(event, key)

    async def advance_watermark(self, watermark: Watermark) -> None:
        """Advance the watermark for a stream and fire any completed windows.

        Args:
            watermark: The new watermark.
        """
        stream_id = watermark.stream_id
        old_watermark = self._stream_state.watermarks.get(stream_id)

        if old_watermark is None or watermark.timestamp > old_watermark:
            self._stream_state.watermarks[stream_id] = watermark.timestamp

            if self._window_trigger:
                results = self._window_trigger.advance_watermark(watermark.timestamp)
                for result in results:
                    await self.emit("window_output", result)

    def get_watermark(self, stream_id: str) -> Optional[Timestamp]:
        """Get the current watermark for a stream.

        Args:
            stream_id: Stream identifier.

        Returns:
            The current watermark, or ``None`` if not set.
        """
        return self._stream_state.watermarks.get(stream_id)

    async def emit(self, stream: str, value: Any) -> None:
        """Emit a value to an output stream with the current timestamp.

        Args:
            stream: Output stream name.
            value: Value to emit.
        """
        event = StreamEvent.create(
            key=str(hash(value)),
            value=value,
        )
        await self._do_emit(stream, event)

    async def emit_with_timestamp(
        self,
        stream: str,
        value: Any,
        timestamp: Timestamp
    ) -> None:
        """Emit a value with a specific event timestamp.

        Args:
            stream: Output stream name.
            value: Value to emit.
            timestamp: Event timestamp.
        """
        event = StreamEvent.create(
            key=str(hash(value)),
            value=value,
            timestamp=timestamp,
        )
        await self._do_emit(stream, event)

    async def emit_event(self, stream: str, event: StreamEvent) -> None:
        """Emit a pre-constructed stream event.

        Args:
            stream: Output stream name.
            event: The :class:`StreamEvent` to emit.
        """
        await self._do_emit(stream, event)

    async def _do_emit(self, stream: str, event: StreamEvent) -> None:
        """Internal emit implementation that dispatches to registered handlers."""
        if stream in self._output_handlers:
            handler = self._output_handlers[stream]
            if inspect.iscoroutinefunction(handler):
                await handler(event)
            else:
                handler(event)
        else:
            message = Message(
                type=MessageType.STREAM_EVENT,
                payload=event,
            )
            pass

    def register_output_handler(
        self,
        stream: str,
        handler: Callable[[StreamEvent], None]
    ) -> None:
        """Register a handler for an output stream.

        Args:
            stream: Stream name.
            handler: Sync or async callable receiving a
                :class:`StreamEvent`.
        """
        self._output_handlers[stream] = handler

    def register_late_data_handler(self, handler: Callable[[StreamEvent[V]], None]) -> None:
        """Register a handler for late-arriving events.

        Args:
            handler: Sync or async callable.
        """
        self._late_data_handler = handler

    def configure_window(
        self,
        spec: WindowSpec,
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R]
    ) -> None:
        """Configure windowing for this stream actor.

        Args:
            spec: The window specification.
            handler: Callable invoked with ``(events, window_info)``
                when a window fires.
        """
        self._window_assigner = WindowAssigner(spec)
        self._window_trigger = WindowTrigger(self._window_assigner, handler)

    async def get_state(self, name: str, default: Any = None) -> Any:
        """Get a value state entry.

        Args:
            name: State name.
            default: Default if not found.

        Returns:
            The stored value or *default*.
        """
        return await self.stream_state.get_value(name, default)

    async def set_state(self, name: str, value: Any) -> None:
        """Set a value state entry.

        Args:
            name: State name.
            value: Value to store.
        """
        await self.stream_state.set_value(name, value)

    async def get_list_state(self, name: str) -> List[Any]:
        """Get a list state entry.

        Args:
            name: State name.

        Returns:
            The stored list or an empty list.
        """
        return await self.stream_state.get_list(name)

    async def update_list_state(self, name: str, item: Any) -> None:
        """Append an item to a list state entry.

        Args:
            name: State name.
            item: Item to append.
        """
        await self.stream_state.append_to_list(name, item)

    async def get_map_state(self, name: str) -> Dict[str, Any]:
        """Get a map state entry.

        Args:
            name: State name.

        Returns:
            The stored dict or an empty dict.
        """
        return await self.stream_state.get_map(name)

    async def update_map_state(self, name: str, key: str, value: Any) -> None:
        """Put a key-value pair into a map state entry.

        Args:
            name: State name.
            key: Map key.
            value: Map value.
        """
        await self.stream_state.put_in_map(name, key, value)

    async def on_start(self) -> None:
        """Called when the stream actor starts.

        Override to initialize resources, register handlers, etc.
        """
        pass

    async def on_stop(self) -> None:
        """Called when the stream actor stops.

        Flushes remaining buffered events and fires any pending
        windows. Override to add additional cleanup logic.
        """
        while not self._backpressure.is_empty():
            event = self._backpressure.pop()
            if event:
                try:
                    await self._process_event_internal(event)
                except Exception as e:
                    print(f"Error processing buffered event on stop: {e}")

        if self._window_trigger:
            max_ts = Timestamp.now()
            results = self._window_trigger.advance_watermark(max_ts)
            for result in results:
                await self.emit("window_output", result)

    def get_metrics(self) -> Dict[str, Any]:
        """Get stream processing metrics.

        Returns:
            A dict with ``processed_count``, ``late_events_count``,
            ``last_processed_timestamp``, ``watermarks``, and
            ``backpressure`` stats.
        """
        return {
            'processed_count': self._stream_state.processed_count,
            'late_events_count': self._stream_state.late_events_count,
            'last_processed_timestamp': (
                self._stream_state.last_processed_timestamp.milliseconds
                if self._stream_state.last_processed_timestamp else None
            ),
            'watermarks': {
                k: v.milliseconds
                for k, v in self._stream_state.watermarks.items()
            },
            'backpressure': self._backpressure.stats.to_dict(),
        }
