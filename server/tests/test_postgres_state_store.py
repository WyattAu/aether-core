"""Tests for PostgreSQL state store backend.

All database interactions are mocked since a real PostgreSQL instance
is not available in CI. Tests verify the API contract matches the
in-memory StateStore.
"""

import json
from datetime import datetime, timezone
from unittest.mock import MagicMock, patch

import pytest

from server.models import StateEntry


def _make_mock_store():
    """Create a PostgresStateStore with mocked psycopg_pool."""
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

    # Save original class references before reload
    from server import state_store as ss_mod
    orig_memory = ss_mod.MemoryStateStore

    with patch.dict("sys.modules", {"psycopg_pool": mock_psycopg_pool, "psycopg": MagicMock()}):
        import importlib
        import server.postgres_state_store
        import server.state_store
        importlib.reload(server.postgres_state_store)
        importlib.reload(server.state_store)

        # Restore the original MemoryStateStore class identity
        ss_mod.MemoryStateStore = orig_memory
        server.state_store.MemoryStateStore = orig_memory

        from server.postgres_state_store import PostgresStateStore
        store = PostgresStateStore(postgres_url="postgresql://test:test@localhost/test")
        store._ensure_pool()
        return store, mock_conn, mock_cursor, mock_pool, mock_psycopg_pool


# ============================================================
# PostgresStateStore Tests (mocked)
# ============================================================

class TestPostgresStateStore:

    @pytest.fixture(autouse=True)
    def _setup(self):
        # Save original class reference before mock reload
        from server import state_store as ss_mod
        self._orig_memory_store = ss_mod.MemoryStateStore

        self.store, self.conn, self.cursor, self.pool, self.psycopg_pool = _make_mock_store()
        yield

        # Restore original class identity so other tests' isinstance checks pass
        ss_mod.MemoryStateStore = self._orig_memory_store

    def test_get_returns_none_for_missing(self):
        self.cursor.fetchone.return_value = None
        assert self.store.get("actor-1", "missing_key") is None

    def test_get_returns_value(self):
        self.cursor.fetchone.return_value = [json.dumps({"count": 42})]
        assert self.store.get("actor-1", "counter") == {"count": 42}

    def test_get_returns_null_json(self):
        self.cursor.fetchone.return_value = [None]
        assert self.store.get("actor-1", "empty") is None

    def test_set_creates_new_entry(self):
        call_count = [0]
        def fake_fetchone():
            call_count[0] += 1
            if call_count[0] == 1:
                return None  # No existing entry
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone

        entry = self.store.set("actor-1", "counter", value=0)

        assert entry.actor_id == "actor-1"
        assert entry.key == "counter"
        assert entry.version == 1
        assert entry.value == 0

    def test_set_updates_existing(self):
        call_count = [0]
        def fake_fetchone():
            call_count[0] += 1
            if call_count[0] == 1:
                return [3]  # Existing version 3
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone

        entry = self.store.set("actor-1", "counter", value=99)
        assert entry.version == 4

    def test_set_version_conflict(self):
        self.cursor.fetchone.return_value = [5]
        with pytest.raises(ValueError, match="Version conflict"):
            self.store.set("actor-1", "counter", value=99, expected_version=3)

    def test_set_version_conflict_new_entry(self):
        self.cursor.fetchone.return_value = None
        with pytest.raises(ValueError, match="Version conflict"):
            self.store.set("actor-1", "counter", value=0, expected_version=1)

    def test_set_no_expected_version_always_succeeds(self):
        call_count = [0]
        def fake_fetchone():
            call_count[0] += 1
            if call_count[0] == 1:
                return [5]  # Existing version 5
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone
        entry = self.store.set("actor-1", "counter", value=99)
        assert entry.version == 6

    def test_delete_existing(self):
        self.cursor.rowcount = 1
        assert self.store.delete("actor-1", "temp") is True

    def test_delete_missing(self):
        self.cursor.rowcount = 0
        assert self.store.delete("actor-1", "ghost") is False

    def test_get_all_empty(self):
        self.cursor.fetchall.return_value = []
        assert self.store.get_all("nonexistent") == {}

    def test_get_all_multiple_keys(self):
        self.cursor.fetchall.return_value = [
            ("a", json.dumps(1)),
            ("b", json.dumps(2)),
        ]
        result = self.store.get_all("actor-1")
        assert result == {"a": 1, "b": 2}

    def test_on_change_callback(self):
        changes = []

        def on_change(actor_id, key, value, version):
            changes.append((actor_id, key, value, version))

        self.store.on_change(on_change)

        call_count = [0]
        def fake_fetchone():
            call_count[0] += 1
            if call_count[0] == 1:
                return None  # New entry
            return [datetime.now(timezone.utc)]

        self.cursor.fetchone.side_effect = fake_fetchone
        self.store.set("actor-1", "x", value=42)

        assert len(changes) == 1
        assert changes[0] == ("actor-1", "x", 42, 1)

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
            from server.postgres_state_store import PostgresStateStore
            store2 = PostgresStateStore.__new__(PostgresStateStore)
            store2._postgres_url = None
            assert store2._get_url() == "postgresql://env:env@localhost/env"

    def test_rollback_on_conflict(self):
        self.cursor.fetchone.return_value = [5]
        with pytest.raises(ValueError):
            self.store.set("a", "k", value=0, expected_version=3)
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

class TestCreateStateStore:

    def test_create_postgres_backend(self):
        mock_psycopg_pool = MagicMock()
        with patch.dict("sys.modules", {"psycopg_pool": mock_psycopg_pool, "psycopg": MagicMock()}):
            import importlib
            import server.postgres_state_store
            import server.state_store
            importlib.reload(server.postgres_state_store)
            importlib.reload(server.state_store)

            from server.state_store import create_state_store
            from server.postgres_state_store import PostgresStateStore
            store = create_state_store("postgres", postgres_url="postgresql://test:test@localhost/test")
            assert type(store).__name__ == "PostgresStateStore"

            importlib.reload(server.postgres_state_store)
            importlib.reload(server.state_store)

    def test_create_unknown_backend_raises(self):
        from server.state_store import create_state_store
        with pytest.raises(ValueError, match="Unknown state backend"):
            create_state_store("cassandra")
