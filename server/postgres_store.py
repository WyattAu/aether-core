"""PostgreSQL event store backend for the Aether server.

Provides persistent event sourcing backed by PostgreSQL with optimistic
concurrency control. Uses psycopg (psycopg3) with connection pooling
via ``psycopg_pool.ConnectionPool`` for efficient concurrent access.

The schema is auto-created on first connection via ``_ensure_schema()``.

Usage::

    from server.postgres_store import PostgresEventStore

    store = PostgresEventStore("postgresql://user:pass@localhost/aether")
    record = store.append(aggregate_id="order-1", event_type="Created", data={"item": "x"})
    events = store.get_events("order-1")
"""

import json
import logging
import threading
import time
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional

from .models import EventRecord

logger = logging.getLogger("aether-server.postgres")

# SQL schema for the events table.
_SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS aether_events (
    event_id       TEXT NOT NULL PRIMARY KEY,
    aggregate_id   TEXT NOT NULL,
    event_type     TEXT NOT NULL,
    data           JSONB,
    version        INTEGER NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_aether_events_aggregate
    ON aether_events (aggregate_id, version);

CREATE INDEX IF NOT EXISTS idx_aether_events_type
    ON aether_events (event_type);
"""


class PostgresEventStore:
    """PostgreSQL-backed event store with optimistic concurrency and connection pooling.

    Thread-safe. The connection pool is lazy-opened on first use and shared
    across all operations.

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
        self._counter = 0

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
            "PostgreSQL event store pool opened (min=%d, max=%d)",
            self._pool_min_size,
            self._pool_max_size,
        )
        self._ensure_schema()

    def _ensure_schema(self):
        """Create the events table if it doesn't exist."""
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(_SCHEMA_SQL)
        logger.info("PostgreSQL event store schema verified")

    def append(
        self,
        aggregate_id: str,
        event_type: str,
        data: Any = None,
        expected_version: Optional[int] = None,
    ) -> EventRecord:
        """Append an event with optimistic concurrency.

        Uses a serializable transaction to ensure correctness under
        concurrent writes. Each operation gets its own connection from the pool.

        Raises:
            ValueError: If ``expected_version`` does not match.
        """
        self._ensure_pool()
        self._counter += 1

        with self._lock:
            with self._pool.connection() as conn:
                conn.autocommit = False
                try:
                    with conn.cursor() as cur:
                        # Set isolation level to serializable for strong consistency
                        cur.execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")

                        # Get current version
                        cur.execute(
                            "SELECT COALESCE(MAX(version), 0) FROM aether_events WHERE aggregate_id = %s",
                            (aggregate_id,),
                        )
                        row = cur.fetchone()
                        current_version = row[0] if row else 0

                        # Check optimistic concurrency
                        if expected_version is not None and expected_version != current_version:
                            raise ValueError(
                                f"Version conflict: expected {expected_version}, actual {current_version}"
                            )

                        new_version = current_version + 1
                        event_id = f"evt_{self._counter}"

                        # Serialize data to JSON
                        json_data = json.dumps(data, default=str) if data is not None else None

                        cur.execute(
                            """INSERT INTO aether_events (event_id, aggregate_id, event_type, data, version, created_at)
                               VALUES (%s, %s, %s, %s, %s, now())""",
                            (event_id, aggregate_id, event_type, json_data, new_version),
                        )

                        # Fetch the created_at back
                        cur.execute(
                            "SELECT created_at FROM aether_events WHERE event_id = %s",
                            (event_id,),
                        )
                        created_row = cur.fetchone()

                    conn.commit()

                    created_at = created_row[0] if created_row else datetime.now(timezone.utc)
                    # Convert to timezone-aware if naive
                    if created_at.tzinfo is None:
                        created_at = created_at.replace(tzinfo=timezone.utc)

                    return EventRecord(
                        event_id=event_id,
                        aggregate_id=aggregate_id,
                        event_type=event_type,
                        data=data,
                        version=new_version,
                        timestamp=created_at,
                    )
                except ValueError:
                    conn.rollback()
                    raise
                except Exception:
                    conn.rollback()
                    raise

    def get_events(self, aggregate_id: str) -> List[EventRecord]:
        """Get all events for an aggregate, ordered by version."""
        self._ensure_pool()
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    """SELECT event_id, aggregate_id, event_type, data, version, created_at
                       FROM aether_events
                       WHERE aggregate_id = %s
                       ORDER BY version ASC""",
                    (aggregate_id,),
                )
                rows = cur.fetchall()
        return [self._row_to_record(row) for row in rows]

    def get_events_by_type(self, event_type: str) -> List[EventRecord]:
        """Get all events of a given type, ordered by creation time."""
        self._ensure_pool()
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    """SELECT event_id, aggregate_id, event_type, data, version, created_at
                       FROM aether_events
                       WHERE event_type = %s
                       ORDER BY created_at ASC""",
                    (event_type,),
                )
                rows = cur.fetchall()
        return [self._row_to_record(row) for row in rows]

    def get_version(self, aggregate_id: str) -> int:
        """Get the current version for an aggregate."""
        self._ensure_pool()
        with self._pool.connection() as conn:
            with conn.cursor() as cur:
                cur.execute(
                    "SELECT COALESCE(MAX(version), 0) FROM aether_events WHERE aggregate_id = %s",
                    (aggregate_id,),
                )
                row = cur.fetchone()
                return row[0] if row else 0

    def get_snapshot(self, aggregate_id: str) -> Optional[Dict]:
        """Snapshots are not supported in PostgreSQL backend (use application-level)."""
        return None

    def create_snapshot(self, aggregate_id: str, state: Dict) -> None:
        """Snapshots are not supported in PostgreSQL backend (no-op)."""
        pass

    def close(self):
        """Close the connection pool."""
        if self._pool is not None and self._owns_pool:
            self._pool.close()
            self._pool = None
            logger.info("PostgreSQL event store pool closed")

    @staticmethod
    def _row_to_record(row) -> EventRecord:
        """Convert a database row to an EventRecord."""
        event_id, aggregate_id, event_type, json_data, version, created_at = row
        data = json.loads(json_data) if json_data is not None else None
        if created_at.tzinfo is None:
            created_at = created_at.replace(tzinfo=timezone.utc)
        return EventRecord(
            event_id=event_id,
            aggregate_id=aggregate_id,
            event_type=event_type,
            data=data,
            version=version,
            timestamp=created_at,
        )
