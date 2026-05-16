"""
Event Sourcing for State Persistence

Provides event sourcing capabilities for building event-sourced aggregates
and persisting state as a sequence of events.

Example:
    from aether_sdk.event import EventStore, Aggregate, EventEnvelope

    class Order(Aggregate):
        def __init__(self):
            super().__init__()
            self.status = "pending"
            self.items = []

        def apply_order_created(self, event):
            self.status = "created"
            self.items = event["items"]

        def apply_order_shipped(self, event):
            self.status = "shipped"

    # Store and replay events
    store = InMemoryEventStore()
    await store.append("order-123", {"type": "order_created", "items": [...]})
    events = await store.get_events("order-123")
"""

from __future__ import annotations

import asyncio
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Dict, List, Optional, TypeVar

from ..exceptions import AetherError


@dataclass(frozen=True)
class EventVersion:
    """
    Represents the version of an event or aggregate.

    Versions are monotonically increasing and used for optimistic concurrency.
    """

    major: int = 1
    minor: int = 0

    def __str__(self) -> str:
        return f"v{self.major}.{self.minor}"

    def __lt__(self, other: EventVersion) -> bool:
        return (self.major, self.minor) < (other.major, other.minor)

    def __le__(self, other: EventVersion) -> bool:
        return (self.major, self.minor) <= (other.major, other.minor)

    def __gt__(self, other: EventVersion) -> bool:
        return (self.major, self.minor) > (other.major, other.minor)

    def __ge__(self, other: EventVersion) -> bool:
        return (self.major, self.minor) >= (other.major, other.minor)

    @classmethod
    def parse(cls, version_str: str) -> EventVersion:
        """Parse version string like 'v1.2' or '1.2'."""
        version_str = version_str.lstrip("v")
        parts = version_str.split(".")
        return cls(major=int(parts[0]), minor=int(parts[1]) if len(parts) > 1 else 0)


@dataclass
class EventEnvelope:
    """
    Envelope wrapping an event with metadata.

    Contains the event payload along with metadata needed for
    persistence, replay, and auditing.
    """

    event_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    aggregate_id: str = ""
    aggregate_type: str = ""
    event_type: str = ""
    version: int = 1  # Sequence number within aggregate
    timestamp: datetime = field(default_factory=datetime.utcnow)
    payload: Dict[str, Any] = field(default_factory=dict)
    metadata: Dict[str, Any] = field(default_factory=dict)
    schema_version: EventVersion = field(default_factory=EventVersion)
    causation_id: Optional[str] = None  # ID of event that caused this
    correlation_id: Optional[str] = None  # Correlates related events

    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary."""
        return {
            "event_id": self.event_id,
            "aggregate_id": self.aggregate_id,
            "aggregate_type": self.aggregate_type,
            "event_type": self.event_type,
            "version": self.version,
            "timestamp": self.timestamp.isoformat(),
            "payload": self.payload,
            "metadata": self.metadata,
            "schema_version": str(self.schema_version),
            "causation_id": self.causation_id,
            "correlation_id": self.correlation_id,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> EventEnvelope:
        """Deserialize from dictionary."""
        return cls(
            event_id=data["event_id"],
            aggregate_id=data["aggregate_id"],
            aggregate_type=data["aggregate_type"],
            event_type=data["event_type"],
            version=data["version"],
            timestamp=datetime.fromisoformat(data["timestamp"]),
            payload=data["payload"],
            metadata=data.get("metadata", {}),
            schema_version=EventVersion.parse(data.get("schema_version", "v1.0")),
            causation_id=data.get("causation_id"),
            correlation_id=data.get("correlation_id"),
        )


@dataclass
class Snapshot:
    """
    Point-in-time snapshot of an aggregate's state.

    Snapshots are used to optimize replay by providing a starting point
    instead of replaying all events from the beginning.
    """

    aggregate_id: str
    aggregate_type: str
    version: int  # Version at which snapshot was taken
    state: Dict[str, Any]
    timestamp: datetime = field(default_factory=datetime.utcnow)
    metadata: Dict[str, Any] = field(default_factory=dict)


T = TypeVar("T")


class Aggregate(ABC):
    """
    Base class for event-sourced aggregates.

    Aggregates maintain their state by applying events. Subclasses
    should implement apply_* methods for each event type.

    Example:
        class Order(Aggregate):
            def __init__(self):
                super().__init__()
                self.status = "pending"
                self.total = 0.0

            def apply_order_created(self, event: Dict[str, Any]):
                self.status = "created"
                self.total = event.get("total", 0)

            def apply_item_added(self, event: Dict[str, Any]):
                self.total += event.get("price", 0)
    """

    def __init__(self):
        self._id: str = ""
        self._version: int = 0
        self._uncommitted_events: List[EventEnvelope] = []
        self._snapshot_version: int = 0

    @property
    def id(self) -> str:
        """Aggregate ID."""
        return self._id

    @id.setter
    def id(self, value: str):
        self._id = value

    @property
    def version(self) -> int:
        """Current version (number of applied events)."""
        return self._version

    @property
    def uncommitted_events(self) -> List[EventEnvelope]:
        """Events not yet persisted."""
        return list(self._uncommitted_events)

    def apply_event(self, envelope: EventEnvelope) -> None:
        """
        Apply an event to update aggregate state.

        Looks for an apply_* method based on event type.
        """
        event_type = envelope.event_type
        method_name = f"apply_{event_type}"

        method = getattr(self, method_name, None)
        if method is None:
            # No handler for this event type - that's okay
            return

        method(envelope.payload)
        self._version = envelope.version

        # Set ID from first event
        if not self._id:
            self._id = envelope.aggregate_id

    def emit_event(
        self,
        event_type: str,
        payload: Dict[str, Any],
        metadata: Optional[Dict[str, Any]] = None,
        schema_version: Optional[EventVersion] = None,
    ) -> EventEnvelope:
        """
        Create and apply a new event.

        The event is added to uncommitted events list.
        """
        self._version += 1

        envelope = EventEnvelope(
            aggregate_id=self._id,
            aggregate_type=self.__class__.__name__,
            event_type=event_type,
            version=self._version,
            payload=payload,
            metadata=metadata or {},
            schema_version=schema_version or EventVersion(),
        )

        self._uncommitted_events.append(envelope)
        self.apply_event(envelope)

        return envelope

    def mark_events_committed(self) -> None:
        """Clear uncommitted events after persistence."""
        self._uncommitted_events.clear()

    def load_from_history(
        self, events: List[EventEnvelope], snapshot: Optional[Snapshot] = None
    ) -> None:
        """
        Rebuild aggregate state from event history.

        Optionally start from a snapshot.
        """
        if snapshot:
            self._load_snapshot(snapshot)

        for event in events:
            if event.version > self._version:
                self.apply_event(event)

    def _load_snapshot(self, snapshot: Snapshot) -> None:
        """Load state from a snapshot."""
        self._id = snapshot.aggregate_id
        self._version = snapshot.version
        self._snapshot_version = snapshot.version

        for key, value in snapshot.state.items():
            setattr(self, key, value)

    def create_snapshot(self) -> Snapshot:
        """
        Create a snapshot of current state.

        Subclasses should override to control what state is captured.
        """
        state = {}
        for key, value in self.__dict__.items():
            if not key.startswith("_"):
                state[key] = value

        return Snapshot(
            aggregate_id=self._id,
            aggregate_type=self.__class__.__name__,
            version=self._version,
            state=state,
        )


class EventStore(ABC):
    """
    Abstract base class for event stores.

    Provides persistence for event streams and snapshots.
    """

    @abstractmethod
    async def append(
        self,
        aggregate_id: str,
        events: List[Dict[str, Any]],
        expected_version: Optional[int] = None,
    ) -> int:
        """
        Append events to an aggregate's stream.

        Args:
            aggregate_id: The aggregate ID
            events: List of event payloads
            expected_version: For optimistic concurrency check

        Returns:
            The new version number

        Raises:
            ConcurrencyError: If version mismatch
        """
        pass

    @abstractmethod
    async def get_events(
        self, aggregate_id: str, after_version: int = 0
    ) -> List[EventEnvelope]:
        """
        Get events for an aggregate.

        Args:
            aggregate_id: The aggregate ID
            after_version: Only return events after this version

        Returns:
            List of event envelopes
        """
        pass

    @abstractmethod
    async def get_events_after(
        self, aggregate_id: str, after_event_id: str
    ) -> List[EventEnvelope]:
        """
        Get events after a specific event.

        Args:
            aggregate_id: The aggregate ID
            after_event_id: Get events after this event ID

        Returns:
            List of event envelopes
        """
        pass

    @abstractmethod
    async def get_events_between_versions(
        self, aggregate_id: str, from_version: int, to_version: int
    ) -> List[EventEnvelope]:
        """
        Get events within a version range.

        Args:
            aggregate_id: The aggregate ID
            from_version: Start version (inclusive)
            to_version: End version (inclusive)

        Returns:
            List of event envelopes
        """
        pass

    @abstractmethod
    async def get_all_events(
        self,
        aggregate_type: Optional[str] = None,
        from_timestamp: Optional[datetime] = None,
    ) -> List[EventEnvelope]:
        """
        Get all events, optionally filtered.

        Args:
            aggregate_type: Filter by aggregate type
            from_timestamp: Filter events after this time

        Returns:
            List of event envelopes
        """
        pass

    @abstractmethod
    async def save_snapshot(self, snapshot: Snapshot) -> None:
        """Save a snapshot for an aggregate."""
        pass

    @abstractmethod
    async def load_snapshot(self, aggregate_id: str) -> Optional[Snapshot]:
        """Load the latest snapshot for an aggregate."""
        pass


class InMemoryEventStore(EventStore):
    """
    In-memory implementation of EventStore for testing and development.
    """

    def __init__(self):
        self._events: Dict[str, List[EventEnvelope]] = {}
        self._snapshots: Dict[str, Snapshot] = {}
        self._all_events: List[EventEnvelope] = []
        self._lock = asyncio.Lock()

    async def append(
        self,
        aggregate_id: str,
        events: List[Dict[str, Any]],
        expected_version: Optional[int] = None,
    ) -> int:
        """Append events to an aggregate's stream."""
        async with self._lock:
            if aggregate_id not in self._events:
                self._events[aggregate_id] = []

            current_version = len(self._events[aggregate_id])

            # Optimistic concurrency check
            if expected_version is not None and current_version != expected_version:
                raise ConcurrencyError(
                    f"Expected version {expected_version}, but was {current_version}"
                )

            envelopes = []
            for i, event in enumerate(events):
                envelope = EventEnvelope(
                    aggregate_id=aggregate_id,
                    event_type=event.get("type", "unknown"),
                    version=current_version + i + 1,
                    payload=event,
                )
                envelopes.append(envelope)
                self._all_events.append(envelope)

            self._events[aggregate_id].extend(envelopes)
            return len(self._events[aggregate_id])

    async def get_events(
        self, aggregate_id: str, after_version: int = 0
    ) -> List[EventEnvelope]:
        """Get events for an aggregate."""
        async with self._lock:
            if aggregate_id not in self._events:
                return []

            events = self._events[aggregate_id]
            return [e for e in events if e.version > after_version]

    async def get_events_after(
        self, aggregate_id: str, after_event_id: str
    ) -> List[EventEnvelope]:
        """Get events after a specific event."""
        async with self._lock:
            if aggregate_id not in self._events:
                return []

            events = self._events[aggregate_id]
            found = False
            result = []

            for event in events:
                if found:
                    result.append(event)
                elif event.event_id == after_event_id:
                    found = True

            return result

    async def get_events_between_versions(
        self, aggregate_id: str, from_version: int, to_version: int
    ) -> List[EventEnvelope]:
        """Get events within a version range."""
        async with self._lock:
            if aggregate_id not in self._events:
                return []

            events = self._events[aggregate_id]
            return [e for e in events if from_version <= e.version <= to_version]

    async def get_all_events(
        self,
        aggregate_type: Optional[str] = None,
        from_timestamp: Optional[datetime] = None,
    ) -> List[EventEnvelope]:
        """Get all events, optionally filtered."""
        async with self._lock:
            events = list(self._all_events)

            if aggregate_type:
                events = [e for e in events if e.aggregate_type == aggregate_type]

            if from_timestamp:
                events = [e for e in events if e.timestamp >= from_timestamp]

            return events

    async def save_snapshot(self, snapshot: Snapshot) -> None:
        """Save a snapshot."""
        async with self._lock:
            self._snapshots[snapshot.aggregate_id] = snapshot

    async def load_snapshot(self, aggregate_id: str) -> Optional[Snapshot]:
        """Load the latest snapshot."""
        async with self._lock:
            return self._snapshots.get(aggregate_id)


class ConcurrencyError(AetherError):
    """Raised when optimistic concurrency check fails."""

    pass


class EventSourcedActor:
    """
    Mixin for actors that use event sourcing.

    Combines actor behavior with event sourcing patterns.
    """

    def __init__(self, event_store: Optional[EventStore] = None):
        self._event_store = event_store or InMemoryEventStore()
        self._aggregates: Dict[str, Aggregate] = {}

    async def load_aggregate(
        self, aggregate_id: str, aggregate_class: type
    ) -> Aggregate:
        """
        Load an aggregate from the event store.
        """
        if aggregate_id in self._aggregates:
            return self._aggregates[aggregate_id]

        aggregate = aggregate_class()
        aggregate.id = aggregate_id

        # Try to load from snapshot first
        snapshot = await self._event_store.load_snapshot(aggregate_id)

        # Load events after snapshot
        after_version = snapshot.version if snapshot else 0
        events = await self._event_store.get_events(aggregate_id, after_version)

        # Rebuild state
        aggregate.load_from_history(events, snapshot)

        self._aggregates[aggregate_id] = aggregate
        return aggregate

    async def save_aggregate(self, aggregate: Aggregate) -> None:
        """
        Persist uncommitted events from an aggregate.
        """
        events = aggregate.uncommitted_events
        if not events:
            return

        # Convert envelopes to event dicts
        event_dicts = [{"type": e.event_type, **e.payload} for e in events]

        expected_version = aggregate.version - len(events)
        await self._event_store.append(
            aggregate.id,
            event_dicts,
            expected_version if expected_version > 0 else None,
        )

        aggregate.mark_events_committed()

    async def save_snapshot(self, aggregate_id: str) -> None:
        """
        Create and save a snapshot for an aggregate.
        """
        if aggregate_id not in self._aggregates:
            return

        aggregate = self._aggregates[aggregate_id]
        snapshot = aggregate.create_snapshot()
        await self._event_store.save_snapshot(snapshot)


def apply_event(aggregate: Aggregate, envelope: EventEnvelope) -> None:
    """
    Helper function to apply an event to an aggregate.

    Args:
        aggregate: The aggregate to update
        envelope: The event envelope to apply
    """
    aggregate.apply_event(envelope)


__all__ = [
    "EventVersion",
    "EventEnvelope",
    "Snapshot",
    "Aggregate",
    "EventStore",
    "InMemoryEventStore",
    "EventSourcedActor",
    "ConcurrencyError",
    "apply_event",
]
