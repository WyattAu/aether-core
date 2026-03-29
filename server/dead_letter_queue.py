import logging
import threading
import time
from collections import OrderedDict
from datetime import datetime, timezone
from typing import Any, Callable, Dict, List, Optional

from .models import MessageEnvelope

logger = logging.getLogger("aether-server.dlq")


class DeadLetterEntry:
    __slots__ = (
        "message", "reason", "first_failed_at", "last_failed_at",
        "retry_count", "source_node", "metadata",
    )

    def __init__(
        self,
        message: MessageEnvelope,
        reason: str,
        first_failed_at: Optional[float] = None,
        last_failed_at: Optional[float] = None,
        retry_count: int = 1,
        source_node: str = "",
        metadata: Optional[Dict[str, Any]] = None,
    ):
        self.message = message
        self.reason = reason
        self.first_failed_at = first_failed_at or time.time()
        self.last_failed_at = last_failed_at or time.time()
        self.retry_count = retry_count
        self.source_node = source_node
        self.metadata = metadata or {}

    @property
    def message_id(self) -> str:
        return self.message.message_id

    def to_dict(self) -> Dict[str, Any]:
        return {
            "message_id": self.message.message_id,
            "source_actor": self.message.source_actor,
            "target_actor": self.message.target_actor,
            "message_type": self.message.message_type,
            "payload": self.message.payload,
            "correlation_id": self.message.correlation_id,
            "timestamp": self.message.timestamp.isoformat() if self.message.timestamp else None,
            "priority": self.message.priority,
            "reason": self.reason,
            "first_failed_at": self.first_failed_at,
            "last_failed_at": self.last_failed_at,
            "retry_count": self.retry_count,
            "source_node": self.source_node,
            "metadata": self.metadata,
        }


class DeadLetterQueue:
    def __init__(
        self,
        max_size: int = 10000,
        ttl_seconds: float = 0,
        on_enqueue: Optional[Callable[[DeadLetterEntry], None]] = None,
        on_replay: Optional[Callable[[DeadLetterEntry], None]] = None,
    ):
        self._max_size = max_size
        self._ttl_seconds = ttl_seconds
        self._on_enqueue = on_enqueue
        self._on_replay = on_replay
        self._lock = threading.Lock()
        self._entries: OrderedDict[str, DeadLetterEntry] = OrderedDict()
        self._total_enqueued = 0
        self._total_replayed = 0
        self._total_purged = 0
        self._total_expired = 0

    @property
    def size(self) -> int:
        return len(self._entries)

    @property
    def total_enqueued(self) -> int:
        return self._total_enqueued

    @property
    def total_replayed(self) -> int:
        return self._total_replayed

    @property
    def total_purged(self) -> int:
        return self._total_purged

    @property
    def total_expired(self) -> int:
        return self._total_expired

    def enqueue(
        self,
        message: MessageEnvelope,
        reason: str,
        source_node: str = "",
        metadata: Optional[Dict[str, Any]] = None,
    ) -> DeadLetterEntry:
        with self._lock:
            now = time.time()
            message_id = message.message_id

            if message_id in self._entries:
                entry = self._entries[message_id]
                entry.last_failed_at = now
                entry.retry_count += 1
                entry.reason = reason
                entry.source_node = source_node or entry.source_node
                if metadata:
                    entry.metadata.update(metadata)
            else:
                entry = DeadLetterEntry(
                    message=message,
                    reason=reason,
                    first_failed_at=now,
                    last_failed_at=now,
                    retry_count=1,
                    source_node=source_node,
                    metadata=metadata,
                )
                self._entries[message_id] = entry
                self._total_enqueued += 1

                while len(self._entries) > self._max_size:
                    evicted_id, _ = self._entries.popitem(last=False)
                    self._total_expired += 1
                    logger.debug("DLQ evicted message %s (queue full)", evicted_id)

            if self._on_enqueue:
                try:
                    self._on_enqueue(entry)
                except Exception as e:
                    logger.error("DLQ on_enqueue callback error: %s", e)

            return entry

    def get(self, message_id: str) -> Optional[DeadLetterEntry]:
        with self._lock:
            return self._entries.get(message_id)

    def list_messages(
        self,
        actor_id: Optional[str] = None,
        source_actor: Optional[str] = None,
        message_type: Optional[str] = None,
        limit: int = 100,
        offset: int = 0,
    ) -> List[DeadLetterEntry]:
        with self._lock:
            self._expire_entries()

            entries = list(self._entries.values())

            if actor_id:
                entries = [e for e in entries if e.message.target_actor == actor_id]
            if source_actor:
                entries = [e for e in entries if e.message.source_actor == source_actor]
            if message_type:
                entries = [e for e in entries if e.message.message_type == message_type]

            entries.sort(key=lambda e: e.last_failed_at, reverse=True)

            return entries[offset:offset + limit]

    def remove(self, message_id: str) -> bool:
        with self._lock:
            if message_id in self._entries:
                del self._entries[message_id]
                return True
            return False

    def replay(self, message_id: str) -> Optional[MessageEnvelope]:
        with self._lock:
            entry = self._entries.pop(message_id, None)
            if entry is None:
                return None

            self._total_replayed += 1

            if self._on_replay:
                try:
                    self._on_replay(entry)
                except Exception as e:
                    logger.error("DLQ on_replay callback error: %s", e)

            return entry.message

    def replay_all(self) -> List[MessageEnvelope]:
        with self._lock:
            entries = list(self._entries.values())
            count = len(entries)
            messages = []
            for entry in entries:
                messages.append(entry.message)
                if self._on_replay:
                    try:
                        self._on_replay(entry)
                    except Exception as e:
                        logger.error("DLQ on_replay callback error: %s", e)
            self._entries.clear()
            self._total_replayed += count
            logger.info("DLQ replayed %d messages", count)
            return messages

    def purge(self) -> int:
        with self._lock:
            count = len(self._entries)
            self._entries.clear()
            self._total_purged += count
            logger.info("DLQ purged %d messages", count)
            return count

    def get_stats(self) -> Dict[str, Any]:
        with self._lock:
            self._expire_entries()

            now = time.time()
            oldest_age = 0.0
            if self._entries:
                oldest = min(e.first_failed_at for e in self._entries.values())
                oldest_age = now - oldest

            actor_counts: Dict[str, int] = {}
            for entry in self._entries.values():
                actor = entry.message.target_actor
                actor_counts[actor] = actor_counts.get(actor, 0) + 1

            return {
                "size": len(self._entries),
                "max_size": self._max_size,
                "total_enqueued": self._total_enqueued,
                "total_replayed": self._total_replayed,
                "total_purged": self._total_purged,
                "total_expired": self._total_expired,
                "oldest_message_age_seconds": round(oldest_age, 1),
                "actors": actor_counts,
            }

    def _expire_entries(self) -> int:
        if self._ttl_seconds <= 0:
            return 0

        now = time.time()
        expired = [
            mid for mid, entry in self._entries.items()
            if now - entry.first_failed_at > self._ttl_seconds
        ]
        for mid in expired:
            del self._entries[mid]

        if expired:
            self._total_expired += len(expired)
            logger.debug("DLQ expired %d entries (TTL=%.0fs)", len(expired), self._ttl_seconds)

        return len(expired)
