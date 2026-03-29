"""State store backends for the Aether server.

Provides a pluggable state management system with two backends:
- ``MemoryStateStore``: In-memory storage (default, no dependencies)
- ``RedisStateStore``: Redis-backed storage (requires ``redis`` package)

The backend is selected via ``ServerConfig.state_backend`` (``"memory"`` or ``"redis"``).
"""

import json
import logging
import threading
from abc import ABC, abstractmethod
from collections import defaultdict
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from .models import StateEntry

logger = logging.getLogger("aether-server.state")


class StateStore(ABC):
    """Abstract base class for state storage backends.

    All backends must implement ``get``, ``set``, ``delete``, and ``get_all``.
    Change callbacks are supported via ``on_change``.
    """

    def __init__(self):
        self._change_callbacks: list = []

    @abstractmethod
    def get(self, actor_id: str, key: str) -> Optional[Any]:
        """Get the value for a state entry. Returns ``None`` if not found."""
        ...

    @abstractmethod
    def set(self, actor_id: str, key: str, value: Any,
            expected_version: Optional[int] = None) -> StateEntry:
        """Set a state value, optionally checking version for optimistic concurrency.

        Raises:
            ValueError: If ``expected_version`` does not match the current version.
        """
        ...

    @abstractmethod
    def delete(self, actor_id: str, key: str) -> bool:
        """Delete a state entry. Returns ``True`` if found and deleted."""
        ...

    @abstractmethod
    def get_all(self, actor_id: str) -> Dict[str, Any]:
        """Get all state entries for an actor as a ``{key: value}`` dict."""
        ...

    def on_change(self, callback):
        """Register a callback invoked on state changes.

        Callback signature: ``callback(actor_id, key, value, version)``.
        """
        self._change_callbacks.append(callback)

    def _notify_change(self, actor_id: str, key: str, value: Any, version: int):
        """Fire all registered change callbacks."""
        for cb in self._change_callbacks:
            try:
                cb(actor_id, key, value, version)
            except Exception:
                pass


class MemoryStateStore(StateStore):
    """In-memory state store using a nested dict.

    Thread-safe via a ``threading.Lock``. All data is lost on process exit.
    """

    def __init__(self):
        super().__init__()
        self._store: Dict[str, Dict[str, StateEntry]] = defaultdict(dict)
        self._lock = threading.Lock()

    def get(self, actor_id: str, key: str) -> Optional[Any]:
        entry = self._store.get(actor_id, {}).get(key)
        return entry.value if entry else None

    def set(self, actor_id: str, key: str, value: Any,
            expected_version: Optional[int] = None) -> StateEntry:
        with self._lock:
            existing = self._store.get(actor_id, {}).get(key)
            if existing is not None:
                if expected_version is not None and existing.version != expected_version:
                    raise ValueError(
                        f"Version conflict: expected {expected_version}, actual {existing.version}"
                    )
                new_version = existing.version + 1
            else:
                if expected_version is not None and expected_version != 0:
                    raise ValueError(
                        f"Version conflict: expected {expected_version}, but entry does not exist"
                    )
                new_version = 1

            entry = StateEntry(
                actor_id=actor_id,
                key=key,
                value=value,
                version=new_version,
                updated_at=datetime.now(timezone.utc),
            )
            self._store[actor_id][key] = entry
            self._notify_change(actor_id, key, value, new_version)
            return entry

    def delete(self, actor_id: str, key: str) -> bool:
        with self._lock:
            bucket = self._store.get(actor_id)
            if bucket and key in bucket:
                del bucket[key]
                return True
            return False

    def get_all(self, actor_id: str) -> Dict[str, Any]:
        bucket = self._store.get(actor_id, {})
        return {k: v.value for k, v in bucket.items()}


class RedisStateStore(StateStore):
    """Redis-backed state store.

    Uses Redis hash-per-actor for state entries, with JSON serialization.
    Version tracking uses a separate Redis key per entry.

    Args:
        redis_url: Redis connection URL (e.g. ``redis://localhost:6379/0``).
            Defaults to ``REDIS_URL`` env var or ``redis://localhost:6379/0``.
        key_prefix: Prefix for all Redis keys. Defaults to ``aether:state:``.
        ttl_seconds: Optional TTL for state entries in seconds. ``None`` means no expiry.

    Raises:
        ImportError: If the ``redis`` package is not installed.
    """

    def __init__(self, redis_url: Optional[str] = None,
                 key_prefix: str = "aether:state:",
                 ttl_seconds: Optional[int] = None):
        super().__init__()
        try:
            import redis
        except ImportError:
            raise ImportError(
                "Redis backend requires the 'redis' package. "
                "Install it with: pip install 'aether-server[redis]'"
            )

        self._redis_url = redis_url
        self._key_prefix = key_prefix
        self._ttl = ttl_seconds
        self._redis: Optional[Any] = None

    def _ensure_connection(self):
        """Lazy-connect to Redis on first use."""
        if self._redis is None:
            import redis
            url = self._redis_url
            if url is None:
                import os
                url = os.environ.get("REDIS_URL", "redis://localhost:6379/0")
            self._redis = redis.from_url(url, decode_responses=True)
            logger.info("Connected to Redis at %s", url)

    def _entry_key(self, actor_id: str, key: str) -> str:
        """Build the Redis key for a state entry."""
        return f"{self._key_prefix}{actor_id}:{key}"

    def _actor_keys_pattern(self, actor_id: str) -> str:
        """Build the Redis key pattern for all keys of an actor."""
        return f"{self._key_prefix}{actor_id}:*"

    def get(self, actor_id: str, key: str) -> Optional[Any]:
        self._ensure_connection()
        redis_key = self._entry_key(actor_id, key)
        raw = self._redis.get(redis_key)
        if raw is None:
            return None
        data = json.loads(raw)
        return data.get("value")

    def set(self, actor_id: str, key: str, value: Any,
            expected_version: Optional[int] = None) -> StateEntry:
        self._ensure_connection()
        redis_key = self._entry_key(actor_id, key)

        with self._redis.lock(f"{redis_key}:lock", timeout=10):
            existing_raw = self._redis.get(redis_key)
            existing = json.loads(existing_raw) if existing_raw else None

            if existing is not None:
                current_version = existing.get("version", 0)
                if expected_version is not None and current_version != expected_version:
                    raise ValueError(
                        f"Version conflict: expected {expected_version}, actual {current_version}"
                    )
                new_version = current_version + 1
            else:
                if expected_version is not None and expected_version != 0:
                    raise ValueError(
                        f"Version conflict: expected {expected_version}, but entry does not exist"
                    )
                new_version = 1

            now = datetime.now(timezone.utc)
            entry_data = {
                "actor_id": actor_id,
                "key": key,
                "value": value,
                "version": new_version,
                "updated_at": now.isoformat(),
            }

            serialized = json.dumps(entry_data, default=str)
            if self._ttl:
                self._redis.setex(redis_key, self._ttl, serialized)
            else:
                self._redis.set(redis_key, serialized)

        entry = StateEntry(
            actor_id=actor_id,
            key=key,
            value=value,
            version=new_version,
            updated_at=now,
        )
        self._notify_change(actor_id, key, value, new_version)
        return entry

    def delete(self, actor_id: str, key: str) -> bool:
        self._ensure_connection()
        redis_key = self._entry_key(actor_id, key)
        result = self._redis.delete(redis_key)
        return result > 0

    def get_all(self, actor_id: str) -> Dict[str, Any]:
        self._ensure_connection()
        pattern = self._actor_keys_pattern(actor_id)
        keys = self._redis.keys(pattern)
        result: Dict[str, Any] = {}
        for key in keys:
            raw = self._redis.get(key)
            if raw:
                data = json.loads(raw)
                entry_key = data.get("key", "")
                result[entry_key] = data.get("value")
        return result


def create_state_store(backend: str = "memory", **kwargs) -> StateStore:
    """Factory function for creating state store backends.

    Args:
        backend: Backend type (``"memory"``, ``"redis"``, or ``"postgres"``).
        **kwargs: Additional arguments passed to the backend constructor.
            For ``"postgres"``: ``postgres_url``, ``pool_min_size``,
            ``pool_max_size``, ``pool``.

    Returns:
        A ``StateStore`` instance.

    Raises:
        ValueError: If the backend name is unknown.
    """
    if backend == "memory":
        return MemoryStateStore()
    elif backend == "redis":
        return RedisStateStore(**kwargs)
    elif backend == "postgres":
        from .postgres_state_store import PostgresStateStore
        return PostgresStateStore(**kwargs)
    else:
        raise ValueError(f"Unknown state backend: {backend}. Use 'memory', 'redis', or 'postgres'.")
