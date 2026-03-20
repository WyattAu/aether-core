"""
Aether SDK Streaming Module

Provides stream processing capabilities for building event-driven applications:
- Event-time processing with watermarks
- Windowed aggregations (tumbling, sliding, session)
- Backpressure handling
- Stream actors

Example:
    from aether_sdk.streaming import (
        StreamActor,
        StreamEvent,
        Duration,
        Timestamp,
        TumblingWindow,
        SlidingWindow,
        SessionWindow,
        @tumbling,
        @sliding,
        @session,
    )
    
    # Create a simple stream processor
    class MyProcessor(StreamActor[str, dict]):
        @classmethod
        def name(cls) -> str:
            return "my_processor"
        
        async def process_event(self, event: StreamEvent[dict]) -> None:
            data = event.value
            # Process the data
            result = transform(data)
            await self.emit("output", result)
    
    # Or use windowing decorators
    @tumbling(size=Duration.from_minutes(5))
    def process_window(events: List[StreamEvent], info: WindowInfo) -> Result:
        # Process batch of events in 5-minute windows
        return Result(aggregate=...)
"""

from __future__ import annotations

import asyncio
from typing import (
    Any,
    Callable,
    Dict,
    Generic,
    List,
    Optional,
    TypeVar,
    Awaitable,
)
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum, auto
import uuid
import json
from pathlib import Path

from typing import Any

from typing import Generic, TypeVar, Awaitable, List, Optional, Union
 Set, Dict, Callable


(End of file - total 304 lines)
    stream_actor.py
)

# Import event module components
from .pubsub import (
    PubSubClient,
    Topic,
    Subscription,
    Publisher,
    Subscriber,
    Event,
    Subscribe,
    publish,
)
from .event_sourcing import (
    EventStore,
    EventSourcedActor,
    EventEnvelope,
    EventVersion,
    Aggregate,
    apply_event,
)
from .delivery import (
    DeliveryGuarantee,
    InMemoryOutbox,
    DeadLetterQueue,
    DeliveryStats,
)
 from .schema import (
    SchemaRegistry,
    Schema,
    SchemaVersion,
    Compatibility,
    SchemaValidator,
)


