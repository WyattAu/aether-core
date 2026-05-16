"""
Delivery Guarantee Components

Provides message delivery guarantees, retry policies, and outbox pattern
for reliable message delivery.

Example:
    from aether_sdk.event import DeliveryGuarantee, RetryPolicy, InMemoryOutbox

    # Configure delivery guarantee
    policy = RetryPolicy(max_retries=3, backoff_ms=100)
    outbox = InMemoryOutbox()
"""

from __future__ import annotations

import asyncio
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any, Dict, List, Optional


class DeliveryGuarantee(Enum):
    """Message delivery guarantee levels."""

    AT_MOST_ONCE = "at_most_once"  # Fire and forget
    AT_LEAST_ONCE = "at_least_once"  # May duplicate
    EXACTLY_ONCE = "exactly_once"  # No duplicates


@dataclass
class RetryPolicy:
    """Retry policy for failed message delivery."""

    max_retries: int = 3
    initial_backoff_ms: int = 100
    max_backoff_ms: int = 30000
    backoff_multiplier: float = 2.0

    def get_backoff_ms(self, attempt: int) -> int:
        """Calculate backoff time for given attempt."""
        backoff = self.initial_backoff_ms * (self.backoff_multiplier**attempt)
        return min(int(backoff), self.max_backoff_ms)


@dataclass
class OutboxEntry:
    """Entry in the outbox for pending deliveries."""

    id: str = field(default_factory=lambda: str(uuid.uuid4()))
    topic: str = ""
    key: Optional[str] = None
    value: Any = None
    headers: Dict[str, str] = field(default_factory=dict)
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    attempts: int = 0
    last_error: Optional[str] = None
    next_retry_at: Optional[datetime] = None


@dataclass
class DeliveryStats:
    """Statistics for message delivery."""

    total_sent: int = 0
    total_failed: int = 0
    total_retries: int = 0
    last_sent_at: Optional[datetime] = None
    last_failed_at: Optional[datetime] = None


class DeadLetterQueue(ABC):
    """Abstract base class for dead letter queues."""

    @abstractmethod
    async def add(self, entry: OutboxEntry, error: Exception) -> None:
        """Add a failed message to the DLQ."""
        pass

    @abstractmethod
    async def get_pending(self, limit: int = 100) -> List[OutboxEntry]:
        """Get pending messages from the DLQ."""
        pass

    @abstractmethod
    async def remove(self, entry_id: str) -> bool:
        """Remove a message from the DLQ."""
        pass


class InMemoryDeadLetterQueue(DeadLetterQueue):
    """In-memory implementation of DeadLetterQueue."""

    def __init__(self, max_size: int = 1000):
        self._entries: List[tuple[OutboxEntry, Exception]] = []
        self._max_size = max_size
        self._lock = asyncio.Lock()

    async def add(self, entry: OutboxEntry, error: Exception) -> None:
        async with self._lock:
            if len(self._entries) >= self._max_size:
                self._entries.pop(0)  # Remove oldest
            self._entries.append((entry, error))

    async def get_pending(self, limit: int = 100) -> List[OutboxEntry]:
        async with self._lock:
            return [e for e, _ in self._entries[:limit]]

    async def remove(self, entry_id: str) -> bool:
        async with self._lock:
            for i, (entry, _) in enumerate(self._entries):
                if entry.id == entry_id:
                    self._entries.pop(i)
                    return True
            return False


class Outbox(ABC):
    """Abstract base class for outbox pattern implementation."""

    @abstractmethod
    async def add(self, entry: OutboxEntry) -> None:
        """Add an entry to the outbox."""
        pass

    @abstractmethod
    async def get_pending(self, limit: int = 100) -> List[OutboxEntry]:
        """Get pending entries for delivery."""
        pass

    @abstractmethod
    async def mark_delivered(self, entry_id: str) -> None:
        """Mark an entry as delivered."""
        pass

    @abstractmethod
    async def mark_failed(self, entry_id: str, error: Exception) -> None:
        """Mark an entry as failed."""
        pass


class InMemoryOutbox(Outbox):
    """In-memory implementation of Outbox."""

    def __init__(self, retry_policy: Optional[RetryPolicy] = None):
        self._pending: Dict[str, OutboxEntry] = {}
        self._retry_policy = retry_policy or RetryPolicy()
        self._stats = DeliveryStats()
        self._lock = asyncio.Lock()

    async def add(self, entry: OutboxEntry) -> None:
        async with self._lock:
            self._pending[entry.id] = entry

    async def get_pending(self, limit: int = 100) -> List[OutboxEntry]:
        async with self._lock:
            entries = list(self._pending.values())
            return entries[:limit]

    async def mark_delivered(self, entry_id: str) -> None:
        async with self._lock:
            if entry_id in self._pending:
                del self._pending[entry_id]
                self._stats.total_sent += 1
                self._stats.last_sent_at = datetime.now(timezone.utc)

    async def mark_failed(self, entry_id: str, error: Exception) -> None:
        async with self._lock:
            if entry_id in self._pending:
                entry = self._pending[entry_id]
                entry.attempts += 1
                entry.last_error = str(error)

                if entry.attempts >= self._retry_policy.max_retries:
                    del self._pending[entry_id]
                    self._stats.total_failed += 1
                    self._stats.last_failed_at = datetime.now(timezone.utc)


__all__ = [
    "DeliveryGuarantee",
    "RetryPolicy",
    "OutboxEntry",
    "DeliveryStats",
    "DeadLetterQueue",
    "InMemoryDeadLetterQueue",
    "Outbox",
    "InMemoryOutbox",
]
