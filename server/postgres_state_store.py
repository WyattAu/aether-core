"""PostgreSQL state store backend for the Aether server.

Provides persistent state management backed by PostgreSQL with optimistic
concurrency control and connection pooling via ``psycopg_pool.ConnectionPool``.
Follows the same ``StateStore`` interface as ``MemoryStateStore`` and
``RedisStateStore``.

Usage::

    from server.postgres_state_store import PostgresStateStore

    store = PostgresStateStore("postgresql://user:pass@localhost/aether")
    entry = store.set(actor_id="actor-1", key="counter", value=42)
    value = store.get("actor-1", "counter")  # 42
"""

import json
import logging
import threading
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from .models import StateEntry

logger = logging.getLogger("aether-server.postgres")

# SQL schema for the state table.
_STATE_SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS aether_state (
    actor_id     TEXT NOT NULL,
    key          TEXT NOT NULL,
    value        JSONB,
    version      INTEGER NOT NULL DEFAULT 1,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (actor_id, key)
);
"""


class PostgresStateStore:
    """PostgreSQL-backed state store with optimistic concurrency and connection pooling.

    Thread-safe. The connection pool is lazy-opened on first use.

    Args:
        postgres_url: PostgreSQL connection URL
            (e.g. ``postgresql://user:pass@localhost:5432/aether``).
            Defaults to ``POSTGRES_URL`` env var.
        pool_min_size: Minimum number of connections in the pool. Defaults to 2.
        pool_max_size: Maximum number of connections in the pool. Defaults to 10.
        pool: Optional pre-configured ``psycopg_pool.ConnectionPool`` instance.
            When provided, ``pool_min_size`` and ``pool_max_size`` are ignored
            and the caller is responsible for closing the pool.
    """

    def __init__(
        self,
        postgres_url: Optional[str] = None,
        pool_min_size: int = 2,
        pool_max_size: int = 10,
        pool: Optional[Any] = None,
    ):
        self._postgres_url = postgres_url
        self._pool_min_size = pool_min_size
        self._pool_max_size = pool_max_size
        self._pool = pool
        self._owns_pool = pool is None
        self._lock = threading.Lock()
        self._change_callbacks: list = []

    def _get_url(self) -> str:
        if self._postgres_url is not None:
            return self._postgres_url
        import os
        return os.environ.get("POSTGRES_URL", "postgresql://postgres:postgres@localhost:5432/aether")

    def _ensure_pool(self):
        """Lazily create the connection pool and run schema init."""
        if self._pool is not None:
            return

        try:
            import psycopg_pool
        except ImportError:
            raise ImportError(
                "PostgreSQL connection pooling requires 'psycopg[pool]'. "
                "Install with: pip install 'aether-server[postgres]'"
            )

        self._pool = psycopg_pool.ConnectionPool(
            self._get_url(),
            min_size=self._pool_min_size,
            max_size=self._pool_max_size,
            open=False,
        )
        self._pool.open()
        logger.info(
            "PostgreSQL state store pool opened (min=%d, max=%d)",
            self._pool_min_size,
            self._pool_max_size,
        )
        self._ensure_schema()

    def _ensure_schema(self):
        """Create the state table if it doesn't exist."""
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(_STATE_SCHEMA_SQL)
        logger.info("PostgreSQL state store schema verified")

    def get(self, actor_id: str, key: str) -> Optional[Any]:
        """Get the value for a state entry. Returns ``None`` if not found."""
        self._ensure_pool()
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "SELECT value FROM aether_state WHERE actor_id = %s AND key = %s",
                    (actor_id, key),
                )
                row = cur.fetchone()
        if row is None:
            return None
        return json.loads(row[0]) if row[0] is not None else None

    def set(
        self,
        actor_id: str,
        key: str,
        value: Any,
        expected_version: Optional[int] = None,
    ) -> StateEntry:
        """Set a state value with optional optimistic concurrency.

        Uses an upsert (INSERT ... ON CONFLICT UPDATE) with serializable
        isolation for correctness under concurrent writes.

        Raises:
            ValueError: If ``expected_version`` does not match.
        """
        self._ensure_pool()

        json_value = json.dumps(value, default=str) if value is not None else None

        with self._lock:
            with self._pool.connection() as conn:
                conn.autocommit = False
                try:
                    with conn.cursor() as cur:
                        cur.execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")

                        # Check current version
                        cur.execute(
                            "SELECT version FROM aether_state WHERE actor_id = %s AND key = %s",
                            (actor_id, key),
                        )
                        row = cur.fetchone()

                        if row is not None:
                            current_version = row[0]
                            if expected_version is not None and expected_version != current_version:
                                raise ValueError(
                                    f"Version conflict: expected {expected_version}, actual {current_version}"
                                )
                            new_version = current_version + 1
                            cur.execute(
                                """UPDATE aether_state
                                   SET value = %s, version = %s, updated_at = now()
                                   WHERE actor_id = %s AND key = %s""",
                                (json_value, new_version, actor_id, key),
                            )
                        else:
                            if expected_version is not None and expected_version != 0:
                                raise ValueError(
                                    f"Version conflict: expected {expected_version}, but entry does not exist"
                                )
                            new_version = 1
                            cur.execute(
                                """INSERT INTO aether_state (actor_id, key, value, version, updated_at)
                                   VALUES (%s, %s, %s, %s, now())""",
                                (actor_id, key, json_value, new_version),
                            )

                        # Fetch updated_at back
                        cur.execute(
                            "SELECT updated_at FROM aether_state WHERE actor_id = %s AND key = %s",
                            (actor_id, key),
                        )
                        ts_row = cur.fetchone()

                    conn.commit()

                    updated_at = ts_row[0] if ts_row else datetime.now(timezone.utc)
                    if updated_at.tzinfo is None:
                        updated_at = updated_at.replace(tzinfo=timezone.utc)

                    entry = StateEntry(
                        actor_id=actor_id,
                        key=key,
                        value=value,
                        version=new_version,
                        updated_at=updated_at,
                    )
                    self._notify_change(actor_id, key, value, new_version)
                    return entry
                except ValueError:
                    conn.rollback()
                    raise
                except Exception:
                    conn.rollback()
                    raise

    def delete(self, actor_id: str, key: str) -> bool:
        """Delete a state entry. Returns ``True`` if found and deleted."""
        self._ensure_pool()
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "DELETE FROM aether_state WHERE actor_id = %s AND key = %s",
                    (actor_id, key),
                )
                deleted = cur.rowcount > 0
        return deleted

    def get_all(self, actor_id: str) -> Dict[str, Any]:
        """Get all state entries for an actor as a ``{key: value}`` dict."""
        self._ensure_pool()
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "SELECT key, value FROM aether_state WHERE actor_id = %s",
                    (actor_id,),
                )
                rows = cur.fetchall()
        return {row[0]: json.loads(row[1]) if row[1] is not None else None for row in rows}

    def on_change(self, callback):
        """Register a callback invoked on state changes."""
        self._change_callbacks.append(callback)

    def _notify_change(self, actor_id: str, key: str, value: Any, version: int):
        """Fire all registered change callbacks."""
        for cb in self._change_callbacks:
            try:
                cb(actor_id, key, value, version)
            except Exception:
                pass

    def close(self):
        """Close the connection pool."""
        if self._pool is not None and self._owns_pool:
            self._pool.close()
            self._pool = None
            logger.info("PostgreSQL state store pool closed")
