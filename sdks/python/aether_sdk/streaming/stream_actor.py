"""
Stream Actor Base Class

Extends the base Actor with stream processing capabilities:
- Event-time processing with watermarks
- Windowed aggregation
- Backpressure handling
- State management for streaming
"""

from __future__ import annotations
from abc import abstractmethod
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, Generic, List, Optional, TypeVar, Union
import asyncio

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

K = TypeVar('K')  # Key type
V = TypeVar('V')  # Value type
R = TypeVar('R')  # Result type


@dataclass
class StreamState:
    """State for stream processing."""
    watermarks: Dict[str, Timestamp] = field(default_factory=dict)
    processed_count: int = 0
    late_events_count: int = 0
    last_processed_timestamp: Optional[Timestamp] = None


class StreamingStateHandle:
    """Enhanced state handle for streaming operations.
    
    Provides typed state access methods commonly needed in stream processing:
    - Value state: Single value per key
    - List state: Accumulated values
    - Map state: Key-value mappings
    """
    
    def __init__(self, state: StateHandle):
        self._state = state
    
    async def get_value(self, name: str, default: Any = None) -> Any:
        """Get a single value from state.
        
        Args:
            name: State key name
            default: Default value if not found
            
        Returns:
            The stored value or default
        """
        value = await self._state.get_json(name)
        return value if value is not None else default
    
    async def set_value(self, name: str, value: Any) -> None:
        """Set a single value in state.
        
        Args:
            name: State key name
            value: Value to store
        """
        await self._state.set_json(name, value)
    
    async def get_list(self, name: str) -> List[Any]:
        """Get a list from state.
        
        Args:
            name: State key name
            
        Returns:
            The stored list or empty list
        """
        value = await self._state.get_json(name)
        if value is None:
            return []
        return value if isinstance(value, list) else [value]
    
    async def append_to_list(self, name: str, item: Any) -> None:
        """Append an item to a list in state.
        
        Args:
            name: State key name
            item: Item to append
        """
        lst = await self.get_list(name)
        lst.append(item)
        await self._state.set_json(name, lst)
    
    async def clear_list(self, name: str) -> None:
        """Clear a list in state.
        
        Args:
            name: State key name
        """
        await self._state.set_json(name, [])
    
    async def get_map(self, name: str) -> Dict[str, Any]:
        """Get a map/dict from state.
        
        Args:
            name: State key name
            
        Returns:
            The stored map or empty dict
        """
        value = await self._state.get_json(name)
        if value is None:
            return {}
        return value if isinstance(value, dict) else {}
    
    async def put_in_map(self, name: str, key: str, value: Any) -> None:
        """Put a key-value pair in a map.
        
        Args:
            name: State key name
            key: Map key
            value: Map value
        """
        m = await self.get_map(name)
        m[key] = value
        await self._state.set_json(name, m)
    
    async def remove_from_map(self, name: str, key: str) -> Optional[Any]:
        """Remove a key from a map.
        
        Args:
            name: State key name
            key: Map key to remove
            
        Returns:
            The removed value or None
        """
        m = await self.get_map(name)
        value = m.pop(key, None)
        await self._state.set_json(name, m)
        return value
    
    async def clear_map(self, name: str) -> None:
        """Clear a map in state.
        
        Args:
            name: State key name
        """
        await self._state.set_json(name, {})


class StreamActor(Actor, Generic[K, V]):
    """Base class for stream processing actors.
    
    Extends Actor with:
    - Event-time processing and watermarks
    - Windowed aggregation
    - Backpressure handling
    - Stream state management
    
    Example:
        >>> class MyStreamProcessor(StreamActor[str, Event]):
        ...     @classmethod
        ...     def name(cls) -> str:
        ...         return "my_stream_processor"
        ...     
        ...     async def process_event(self, event: StreamEvent[Event]) -> None:
        ...         # Process the event
        ...         data = event.value
        ...         
        ...         # Emit results
        ...         await self.emit("output", result)
        ...
        >>> # With windowing
        >>> class WindowedProcessor(StreamActor[str, Event]):
        ...     def __init__(self):
        ...         super().__init__()
        ...         self._window = TumblingWindow(
        ...             size=Duration.from_minutes(5),
        ...             handler=self.process_window,
        ...         )
        ...     
        ...     async def process_window(
        ...         self,
        ...         events: List[StreamEvent[Event]],
        ...         info: WindowInfo
        ...     ) -> Result:
        ...         # Process batch of events
        ...         return Result(aggregate=...)
    """
    
    def __init__(
        self,
        config: Optional[StreamConfig] = None,
        backpressure_config: Optional[BackpressureConfig] = None,
    ):
        super().__init__()
        
        self._stream_config = config or StreamConfig()
        self._stream_state = StreamState()
        self._streaming_state: Optional[StreamingStateHandle] = None
        
        # Backpressure controller
        self._backpressure = BackpressureController(
            backpressure_config or BackpressureConfig()
        )
        
        # Window management (if configured)
        self._window_assigner: Optional[WindowAssigner[K, V]] = None
        self._window_trigger: Optional[WindowTrigger[K, V, Any]] = None
        
        # Output collectors
        self._output_handlers: Dict[str, Callable] = {}
        
        # Late data output
        self._late_data_handler: Optional[Callable] = None
    
    @property
    def stream_state(self) -> StreamingStateHandle:
        """Get streaming state handle."""
        if self._streaming_state is None:
            self._streaming_state = StreamingStateHandle(self.state)
        return self._streaming_state
    
    @property
    def backpressure(self) -> BackpressureController:
        """Get backpressure controller."""
        return self._backpressure
    
    @property
    def stream_config(self) -> StreamConfig:
        """Get stream configuration."""
        return self._stream_config
    
    # ============================================
    # Abstract Methods
    # ============================================
    
    @abstractmethod
    async def process_event(self, event: StreamEvent[V]) -> None:
        """Process a single stream event.
        
        Override this method to implement event processing logic.
        
        Args:
            event: The stream event to process
        """
        pass
    
    # ============================================
    # Event Processing
    # ============================================
    
    async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
        """Handle incoming message (overrides Actor.handle_message)."""
        if message.type == MessageType.STREAM_EVENT:
            # Extract stream event
            event_data = message.payload
            if isinstance(event_data, StreamEvent):
                await self._process_with_backpressure(event_data)
            elif isinstance(event_data, dict):
                # Reconstruct from dict
                event = self._dict_to_event(event_data)
                if event:
                    await self._process_with_backpressure(event)
            
        elif message.type == MessageType.WATERMARK:
            # Handle watermark
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
        """Convert dictionary to StreamEvent."""
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
        """Process event with backpressure handling."""
        # Check backpressure
        if not self._backpressure.try_push(event):
            # Event was dropped based on strategy
            return
        
        # Pop and process
        while True:
            buffered_event = self._backpressure.pop()
            if buffered_event is None:
                break
            
            try:
                await self._process_event_internal(buffered_event)
            except Exception as e:
                # Log error and continue
                print(f"Error processing event: {e}")
    
    async def _process_event_internal(self, event: StreamEvent[V]) -> None:
        """Internal event processing with watermark and window handling."""
        self._stream_state.processed_count += 1
        
        # Check if event is late
        current_watermark = self._stream_state.watermarks.get(
            event.event_type or 'default',
            Timestamp(0)
        )
        
        if event.timestamp < current_watermark:
            self._stream_state.late_events_count += 1
            await self._handle_late_event(event)
            return
        
        # Process through windowing if configured
        if self._window_trigger:
            # Extract key for windowing
            key = self._extract_key(event)
            results = self._window_trigger.process(event, key)
            
            # Emit window results
            for result in results:
                await self.emit("window_output", result)
        
        # Call user's process_event
        await self.process_event(event)
        
        # Update last processed timestamp
        self._stream_state.last_processed_timestamp = event.timestamp
    
    def _extract_key(self, event: StreamEvent[V]) -> K:
        """Extract key from event for windowing."""
        return event.key  # type: ignore
    
    async def _handle_late_event(self, event: StreamEvent[V]) -> None:
        """Handle late-arriving event based on policy."""
        policy = self._stream_config.late_data_policy
        
        if policy == LateDataPolicy.DROP:
            # Silently drop
            return
        
        elif policy == LateDataPolicy.SIDE_OUTPUT:
            # Route to side output
            if self._late_data_handler:
                await self._late_data_handler(event)
            elif self._stream_config.late_data_output:
                await self.emit(self._stream_config.late_data_output, event)
        
        elif policy == LateDataPolicy.REPROCESS:
            # Trigger reprocessing of affected windows
            if self._window_assigner:
                key = self._extract_key(event)
                self._window_assigner.assign(event, key)
    
    # ============================================
    # Watermark Management
    # ============================================
    
    async def advance_watermark(self, watermark: Watermark) -> None:
        """Advance watermark for a stream.
        
        Args:
            watermark: The new watermark
        """
        stream_id = watermark.stream_id
        old_watermark = self._stream_state.watermarks.get(stream_id)
        
        # Only advance if new watermark is ahead
        if old_watermark is None or watermark.timestamp > old_watermark:
            self._stream_state.watermarks[stream_id] = watermark.timestamp
            
            # Fire any windows triggered by this watermark
            if self._window_trigger:
                results = self._window_trigger.advance_watermark(watermark.timestamp)
                for result in results:
                    await self.emit("window_output", result)
    
    def get_watermark(self, stream_id: str) -> Optional[Timestamp]:
        """Get current watermark for a stream.
        
        Args:
            stream_id: Stream identifier
            
        Returns:
            Current watermark or None if not set
        """
        return self._stream_state.watermarks.get(stream_id)
    
    # ============================================
    # Output Methods
    # ============================================
    
    async def emit(self, stream: str, value: Any) -> None:
        """Emit a value to an output stream.
        
        Args:
            stream: Output stream name
            value: Value to emit
        """
        # Create stream event with current timestamp
        event = StreamEvent.create(
            key=str(hash(value)),  # Simple key generation
            value=value,
        )
        await self._do_emit(stream, event)
    
    async def emit_with_timestamp(
        self,
        stream: str,
        value: Any,
        timestamp: Timestamp
    ) -> None:
        """Emit a value with specific timestamp.
        
        Args:
            stream: Output stream name
            value: Value to emit
            timestamp: Event timestamp
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
            stream: Output stream name
            event: Stream event to emit
        """
        await self._do_emit(stream, event)
    
    async def _do_emit(self, stream: str, event: StreamEvent) -> None:
        """Internal emit implementation."""
        # Check if there's a registered handler
        if stream in self._output_handlers:
            handler = self._output_handlers[stream]
            if asyncio.iscoroutinefunction(handler):
                await handler(event)
            else:
                handler(event)
        else:
            # Send to output stream (would be connected to downstream actors)
            message = Message(
                type=MessageType.STREAM_EVENT,
                payload=event,
            )
            # In a real implementation, this would route to the appropriate stream
            # For now, we just put it in the mailbox for downstream processing
            pass
    
    def register_output_handler(
        self,
        stream: str,
        handler: Callable[[StreamEvent], None]
    ) -> None:
        """Register a handler for output stream.
        
        Args:
            stream: Stream name
            handler: Async or sync function to handle output events
        """
        self._output_handlers[stream] = handler
    
    def register_late_data_handler(self, handler: Callable[[StreamEvent[V]], None]) -> None:
        """Register handler for late-arriving data.
        
        Args:
            handler: Async or sync function to handle late events
        """
        self._late_data_handler = handler
    
    # ============================================
    # Window Configuration
    # ============================================
    
    def configure_window(
        self,
        spec: WindowSpec,
        handler: Callable[[List[StreamEvent[V]], WindowInfo], R]
    ) -> None:
        """Configure windowing for this stream actor.
        
        Args:
            spec: Window specification
            handler: Function to process window contents
        """
        self._window_assigner = WindowAssigner(spec)
        self._window_trigger = WindowTrigger(self._window_assigner, handler)
    
    # ============================================
    # State Access Methods (Flink-style)
    # ============================================
    
    async def get_state(self, name: str, default: Any = None) -> Any:
        """Get value state.
        
        Args:
            name: State name
            default: Default value if not found
            
        Returns:
            Stored value or default
        """
        return await self.stream_state.get_value(name, default)
    
    async def set_state(self, name: str, value: Any) -> None:
        """Set value state.
        
        Args:
            name: State name
            value: Value to store
        """
        await self.stream_state.set_value(name, value)
    
    async def get_list_state(self, name: str) -> List[Any]:
        """Get list state.
        
        Args:
            name: State name
            
        Returns:
            Stored list or empty list
        """
        return await self.stream_state.get_list(name)
    
    async def update_list_state(self, name: str, item: Any) -> None:
        """Add item to list state.
        
        Args:
            name: State name
            item: Item to add
        """
        await self.stream_state.append_to_list(name, item)
    
    async def get_map_state(self, name: str) -> Dict[str, Any]:
        """Get map state.
        
        Args:
            name: State name
            
        Returns:
            Stored map or empty dict
        """
        return await self.stream_state.get_map(name)
    
    async def update_map_state(self, name: str, key: str, value: Any) -> None:
        """Update map state.
        
        Args:
            name: State name
            key: Map key
            value: Map value
        """
        await self.stream_state.put_in_map(name, key, value)
    
    # ============================================
    # Lifecycle Hooks
    # ============================================
    
    async def on_start(self) -> None:
        """Called when stream actor starts.
        
        Override to initialize resources, register handlers, etc.
        """
        pass
    
    async def on_stop(self) -> None:
        """Called when stream actor stops.
        
        Override to clean up resources, flush buffers, etc.
        """
        # Process remaining buffered events
        while not self._backpressure.is_empty():
            event = self._backpressure.pop()
            if event:
                try:
                    await self._process_event_internal(event)
                except Exception as e:
                    print(f"Error processing buffered event on stop: {e}")
        
        # Fire any remaining windows
        if self._window_trigger:
            # Force fire all windows with max timestamp
            max_ts = Timestamp.now()
            results = self._window_trigger.advance_watermark(max_ts)
            for result in results:
                await self.emit("window_output", result)
    
    # ============================================
    # Metrics
    # ============================================
    
    def get_metrics(self) -> Dict[str, Any]:
        """Get stream processing metrics.
        
        Returns:
            Dictionary of metrics
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
