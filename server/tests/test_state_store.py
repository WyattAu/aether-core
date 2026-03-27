"""Tests for state store backends and factory."""

import json
import sys
import threading
from datetime import datetime, timezone

import pytest

from server.state_store import (
    MemoryStateStore,
    RedisStateStore,
    StateStore,
    create_state_store,
)

# Check if redis is available for integration tests
REDIS_AVAILABLE = False
try:
    import redis  # noqa: F401
    REDIS_AVAILABLE = True
except ImportError:
    pass


# === Abstract StateStore contract ===

class StateStoreContract:
    """Shared tests for all StateStore backends."""

    @pytest.fixture
    def store(self) -> StateStore:
        raise NotImplementedError

    def test_get_returns_none_for_missing(self, store):
        assert store.get("actor-1", "key") is None

    def test_set_and_get(self, store):
        entry = store.set("actor-1", "counter", 42)
        assert entry.actor_id == "actor-1"
        assert entry.key == "counter"
        assert entry.value == 42
        assert entry.version == 1
        assert store.get("actor-1", "counter") == 42

    def test_set_overwrite_increments_version(self, store):
        store.set("actor-1", "counter", 1)
        entry = store.set("actor-1", "counter", 2)
        assert entry.value == 2
        assert entry.version == 2
        assert store.get("actor-1", "counter") == 2

    def test_set_with_expected_version(self, store):
        entry = store.set("actor-1", "k", "v1")
        store.set("actor-1", "k", "v2", expected_version=entry.version)
        assert store.get("actor-1", "k") == "v2"

    def test_set_version_conflict(self, store):
        store.set("actor-1", "k", "v1")
        with pytest.raises(ValueError, match="Version conflict"):
            store.set("actor-1", "k", "v2", expected_version=999)

    def test_set_expected_version_zero_for_new(self, store):
        store.set("actor-1", "k", "v", expected_version=0)

    def test_set_expected_version_nonzero_for_new_fails(self, store):
        with pytest.raises(ValueError, match="Version conflict"):
            store.set("actor-1", "k", "v", expected_version=5)

    def test_delete_existing(self, store):
        store.set("actor-1", "k", "v")
        assert store.delete("actor-1", "k") is True
        assert store.get("actor-1", "k") is None

    def test_delete_missing(self, store):
        assert store.delete("actor-1", "k") is False

    def test_get_all_empty(self, store):
        assert store.get_all("actor-1") == {}

    def test_get_all_multiple_keys(self, store):
        store.set("actor-1", "a", 1)
        store.set("actor-1", "b", 2)
        store.set("actor-1", "c", 3)
        result = store.get_all("actor-1")
        assert result == {"a": 1, "b": 2, "c": 3}

    def test_get_all_isolated_by_actor(self, store):
        store.set("actor-1", "k", "v1")
        store.set("actor-2", "k", "v2")
        assert store.get_all("actor-1") == {"k": "v1"}
        assert store.get_all("actor-2") == {"k": "v2"}

    def test_on_change_callback(self, store):
        changes = []
        store.on_change(lambda a, k, v, ver: changes.append((a, k, v, ver)))
        store.set("actor-1", "k", "v1")
        assert len(changes) == 1
        assert changes[0] == ("actor-1", "k", "v1", 1)

    def test_on_change_callback_on_overwrite(self, store):
        changes = []
        store.on_change(lambda a, k, v, ver: changes.append((a, k, v, ver)))
        store.set("actor-1", "k", "v1")
        store.set("actor-1", "k", "v2")
        assert len(changes) == 2
        assert changes[1] == ("actor-1", "k", "v2", 2)

    def test_on_change_callback_error_is_swallowed(self, store):
        def bad_callback(*args):
            raise RuntimeError("callback error")

        store.on_change(bad_callback)
        # Should not raise despite the bad callback
        store.set("actor-1", "k", "v")

    def test_set_various_value_types(self, store):
        for value in [42, 3.14, "hello", True, None, [1, 2, 3], {"nested": "dict"}]:
            entry = store.set("actor-1", f"key_{type(value).__name__}", value)
            assert entry.value == value
            assert store.get("actor-1", f"key_{type(value).__name__}") == value

    def test_state_entry_has_timestamp(self, store):
        before = datetime.now(timezone.utc)
        entry = store.set("actor-1", "k", "v")
        after = datetime.now(timezone.utc)
        assert before <= entry.updated_at <= after


# === MemoryStateStore Tests ===

class TestMemoryStateStore(StateStoreContract):

    @pytest.fixture
    def store(self):
        return MemoryStateStore()

    def test_thread_safety(self):
        """Concurrent writes to different actors should not corrupt state."""
        store = MemoryStateStore()
        errors = []

        def writer(actor_id, count):
            try:
                for i in range(count):
                    store.set(actor_id, "counter", i)
            except Exception as e:
                errors.append(e)

        threads = [
            threading.Thread(target=writer, args=(f"actor-{i}", 100))
            for i in range(10)
        ]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
        # Each actor should have the last value written
        for i in range(10):
            assert store.get(f"actor-{i}", "counter") == 99

    def test_get_all_returns_snapshot(self):
        """get_all returns values at call time, not affected by later writes."""
        store = MemoryStateStore()
        store.set("actor-1", "a", 1)
        snapshot = store.get_all("actor-1")
        store.set("actor-1", "b", 2)
        assert snapshot == {"a": 1}


# === Factory Tests ===

class TestCreateStateStore:

    def test_creates_memory_backend(self):
        store = create_state_store("memory")
        assert isinstance(store, MemoryStateStore)

    def test_creates_memory_by_default(self):
        store = create_state_store()
        assert isinstance(store, MemoryStateStore)

    def test_unknown_backend_raises(self):
        with pytest.raises(ValueError, match="Unknown state backend"):
            create_state_store("unknown")

    def test_creates_redis_backend(self):
        """RedisStateStore can be created (connection is lazy)."""
        pytest.importorskip("redis", reason="redis package not installed")
        store = create_state_store("redis", redis_url="redis://localhost:6379/0")
        assert isinstance(store, RedisStateStore)


# === RedisStateStore Unit Tests (no actual Redis) ===

@pytest.mark.skipif(not REDIS_AVAILABLE, reason="redis package not installed")
class TestRedisStateStore:

    def test_lazy_connection(self):
        """Construction should not fail — connection is lazy."""
        store = RedisStateStore(redis_url="redis://localhost:6379/0")
        assert store._redis is None

    def test_key_construction(self):
        store = RedisStateStore(key_prefix="test:")
        assert store._entry_key("actor-1", "counter") == "test:actor-1:counter"
        assert store._actor_keys_pattern("actor-1") == "test:actor-1:*"

    def test_default_key_prefix(self):
        store = RedisStateStore()
        assert store._key_prefix == "aether:state:"

    def test_import_error_without_redis(self, monkeypatch):
        """Verify ImportError is raised when redis module is missing."""
        # Temporarily hide the redis module
        redis_module = sys.modules.pop("redis", None)
        try:
            with pytest.raises(ImportError, match="redis"):
                RedisStateStore(redis_url="redis://localhost:6379/0")
        finally:
            if redis_module is not None:
                sys.modules["redis"] = redis_module

    def test_factory_passes_kwargs(self):
        store = create_state_store(
            "redis",
            redis_url="redis://myhost:6380/1",
            key_prefix="custom:",
            ttl_seconds=3600,
        )
        assert isinstance(store, RedisStateStore)
        assert store._redis_url == "redis://myhost:6380/1"
        assert store._key_prefix == "custom:"
        assert store._ttl == 3600
