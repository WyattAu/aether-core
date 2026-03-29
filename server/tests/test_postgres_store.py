"""Tests for PostgreSQL event store backend.

All database interactions are mocked since a real PostgreSQL instance
is not available in CI. Tests verify the API contract matches the
in-memory EventStore.
"""

import json
from datetime import datetime, timezone
from unittest.mock import MagicMock, patch, PropertyMock

import pytest

from server.models import EventRecord


def _make_mock_store():
    """Create a PostgresEventStore with mocked psycopg_pool."""
    mock_conn = MagicMock()
    mock_cursor = MagicMock()
    mock_conn.cursor.return_value.__enter__ = MagicMock(return_value=mock_cursor)
    mock_conn.cursor.return_value.__exit__ = MagicMock(return_value=False)

    # Mock the pool: pool.connection() returns a context manager yielding mock_conn
    mock_pool = MagicMock()
    mock_pool.connection.return_value.__enter__ = MagicMock(return_value=mock_conn)
    mock_pool.connection.return_value.__exit__ = MagicMock(return_value=False)

    mock_psycopg_pool = MagicMock()
    mock_psycopg_pool.ConnectionPool.return_value = mock_pool

    with patch.dict("sys.modules", {"psycopg_pool": mock_psycopg_pool, "psycopg": MagicMock()}):
        import importlib
        import server.postgres_store
        importlib.reload(server.postgres_store)
        from server.postgres_store import PostgresEventStore
        store = PostgresEventStore(postgres_url="postgresql://test:test@localhost/test")
        store._ensure_pool()
        return store, mock_conn, mock_cursor, mock_pool, mock_psycopg_pool


# ============================================================
# PostgresEventStore Tests (mocked)
# ============================================================

class TestPostgresEventStore:

    @pytest.fixture(autouse=True)
    def _setup(self):
        self.store, self.conn, self.cursor, self.pool, self.psycopg_pool = _make_mock_store()
        yield
        # Clean up
        import importlib
        import server.postgres_store
        importlib.reload(server.postgres_store)

    def test_append_creates_event(self):
        call_count = [0]
        def fake_fetchone():
            call_count[0] += 1
            if call_count[0] == 1:
                return [0]  # current version
            return [datetime.now(timezone.utc)]  # created_at

        self.cursor.fetchone.side_effect = fake_fetchone

        record = self.store.append(
            aggregate_id="order-1",
            event_type="OrderCreated",
            data={"item": "widget"},
        )

        assert record.aggregate_id == "order-1"
        assert record.event_type == "OrderCreated"
        assert record.version == 1
        assert record.data == {"item": "widget"}
        assert record.event_id.startswith("evt_")

    def test_append_sequential_versions(self):
        # First append: version 0 -> 1
        call_count = [0]
        def fake_fetchone_v1():
            call_count[0] += 1
            if call_count[0] == 1:
                return [0]
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone_v1
        r1 = self.store.append(aggregate_id="agg-1", event_type="E1")

        # Reset for second append
        call_count[0] = 0
        def fake_fetchone_v2():
            call_count[0] += 1
            if call_count[0] == 1:
                return [1]
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone_v2
        r2 = self.store.append(aggregate_id="agg-1", event_type="E2")

        assert r1.version == 1
        assert r2.version == 2

    def test_append_version_conflict(self):
        self.cursor.fetchone.return_value = [5]

        with pytest.raises(ValueError, match="Version conflict"):
            self.store.append(
                aggregate_id="agg-1",
                event_type="E1",
                expected_version=3,  # Stale
            )

    def test_append_with_no_expected_version(self):
        """No expected_version always succeeds."""
        call_count = [0]
        def fake_fetchone():
            call_count[0] += 1
            if call_count[0] == 1:
                return [5]
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone
        record = self.store.append(aggregate_id="agg-1", event_type="E1")
        assert record.version == 6

    def test_get_events_returns_records(self):
        now = datetime.now(timezone.utc)
        self.cursor.fetchall.return_value = [
            ("evt_1", "agg-1", "Created", json.dumps({"x": 1}), 1, now),
            ("evt_2", "agg-1", "Updated", json.dumps({"x": 2}), 2, now),
        ]

        events = self.store.get_events("agg-1")
        assert len(events) == 2
        assert events[0].event_type == "Created"
        assert events[0].data == {"x": 1}
        assert events[1].version == 2

    def test_get_events_empty(self):
        self.cursor.fetchall.return_value = []
        events = self.store.get_events("nonexistent")
        assert events == []

    def test_get_version(self):
        self.cursor.fetchone.return_value = [5]
        assert self.store.get_version("agg-1") == 5

    def test_get_version_zero(self):
        self.cursor.fetchone.return_value = [0]
        assert self.store.get_version("nonexistent") == 0

    def test_get_events_by_type(self):
        now = datetime.now(timezone.utc)
        self.cursor.fetchall.return_value = [
            ("evt_1", "agg-1", "OrderCreated", None, 1, now),
        ]
        events = self.store.get_events_by_type("OrderCreated")
        assert len(events) == 1
        assert events[0].aggregate_id == "agg-1"

    def test_close_connection(self):
        self.store.close()
        self.pool.close.assert_called_once()

    def test_close_shared_pool_noop(self):
        """Closing a store with an externally-provided pool should not close the pool."""
        external_pool = MagicMock()
        store2 = self.store.__class__.__new__(self.store.__class__)
        store2._pool = external_pool
        store2._owns_pool = False
        store2.close()
        external_pool.close.assert_not_called()

    def test_env_var_fallback(self):
        with patch.dict("os.environ", {"POSTGRES_URL": "postgresql://env:env@localhost/env"}):
            assert self.store._get_url() == "postgresql://test:test@localhost/test"
            # Now create a store without explicit URL
            from server.postgres_store import PostgresEventStore
            store2 = PostgresEventStore.__new__(PostgresEventStore)
            store2._postgres_url = None
            assert store2._get_url() == "postgresql://env:env@localhost/env"

    def test_snapshot_not_supported(self):
        assert self.store.get_snapshot("agg-1") is None
        self.store.create_snapshot("agg-1", {"x": 1})  # no-op

    def test_rollback_on_conflict(self):
        """Transaction is rolled back on version conflict."""
        self.cursor.fetchone.return_value = [5]

        with pytest.raises(ValueError):
            self.store.append(aggregate_id="a", event_type="E", expected_version=3)

        self.conn.rollback.assert_called()

    def test_pool_created_with_correct_config(self):
        """Verify the pool is created with the configured min/max sizes."""
        self.psycopg_pool.ConnectionPool.assert_called_once()
        call_kwargs = self.psycopg_pool.ConnectionPool.call_args
        assert call_kwargs.kwargs["min_size"] == 2
        assert call_kwargs.kwargs["max_size"] == 10
        assert call_kwargs.kwargs["open"] is False


# ============================================================
# Factory Function Tests
# ============================================================

class TestCreateEventStore:

    def test_create_memory_backend(self):
        from server.event_store import create_event_store, EventStore
        store = create_event_store("memory")
        assert isinstance(store, EventStore)

    def test_create_postgres_backend(self):
        mock_psycopg_pool = MagicMock()
        with patch.dict("sys.modules", {"psycopg_pool": mock_psycopg_pool, "psycopg": MagicMock()}):
            import importlib
            import server.postgres_store
            import server.event_store
            importlib.reload(server.postgres_store)
            importlib.reload(server.event_store)

            from server.event_store import create_event_store
            from server.postgres_store import PostgresEventStore
            store = create_event_store("postgres", postgres_url="postgresql://test:test@localhost/test")
            assert type(store).__name__ == "PostgresEventStore"

            importlib.reload(server.postgres_store)
            importlib.reload(server.event_store)

    def test_create_unknown_backend_raises(self):
        from server.event_store import create_event_store
        with pytest.raises(ValueError, match="Unknown event backend"):
            create_event_store("unknown")
