"""Unit tests for the migration state tracker."""

import time

import pytest

from server.cluster.migration_state import (
    MigrationRecord,
    MigrationStateTracker,
    MigrationStatus,
)


class TestMigrationRecord:
    """Tests for MigrationRecord dataclass."""

    def test_duration_while_active(self):
        rec = MigrationRecord(actor_id="a", source_node="n1", target_node="n2")
        time.sleep(0.05)
        assert rec.duration >= 0.04

    def test_duration_after_completion(self):
        rec = MigrationRecord(actor_id="a", source_node="n1", target_node="n2")
        time.sleep(0.05)
        rec.completed_at = time.time()
        # Duration should be roughly the time between start and completion
        assert 0.04 <= rec.duration <= 0.2

    def test_to_dict(self):
        rec = MigrationRecord(
            actor_id="my-actor",
            source_node="node-a",
            target_node="node-b",
            status=MigrationStatus.TRANSFERRING,
            error=None,
        )
        d = rec.to_dict()
        assert d["actor_id"] == "my-actor"
        assert d["source_node"] == "node-a"
        assert d["target_node"] == "node-b"
        assert d["status"] == "transferring"
        assert d["error"] is None
        assert "duration" in d
        assert "started_at" in d


class TestMigrationStateTracker:
    """Tests for MigrationStateTracker."""

    def test_start_migration_success(self):
        tracker = MigrationStateTracker()
        assert tracker.start_migration("actor-1", "node-a", "node-b")
        assert tracker.is_migrating("actor-1")

    def test_start_migration_duplicate_rejected(self):
        tracker = MigrationStateTracker()
        assert tracker.start_migration("actor-1", "node-a", "node-b")
        assert not tracker.start_migration("actor-1", "node-a", "node-c")

    def test_start_migration_after_completion(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.COMPLETED)
        # Should be allowed after completion
        assert tracker.start_migration("actor-1", "node-b", "node-a")

    def test_start_migration_after_failure(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.FAILED, error="timeout")
        assert tracker.start_migration("actor-1", "node-b", "node-a")

    def test_update_status(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.QUIESCING)
        rec = tracker.get_migration("actor-1")
        assert rec.status == MigrationStatus.QUIESCING

    def test_update_status_with_error(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.FAILED, error="connection refused")
        rec = tracker.get_migration("actor-1")
        assert rec.error == "connection refused"
        assert rec.completed_at is not None

    def test_update_status_nonexistent(self):
        tracker = MigrationStateTracker()
        # Should not raise
        tracker.update_status("nonexistent", MigrationStatus.COMPLETED)

    def test_set_state_size(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.set_state_size("actor-1", 4096)
        rec = tracker.get_migration("actor-1")
        assert rec.state_size == 4096

    def test_get_migration_nonexistent(self):
        tracker = MigrationStateTracker()
        assert tracker.get_migration("nonexistent") is None

    def test_get_migration_returns_copy(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        rec1 = tracker.get_migration("actor-1")
        rec2 = tracker.get_migration("actor-1")
        assert rec1 is not rec2
        assert rec1.actor_id == rec2.actor_id

    def test_is_migrating(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        assert tracker.is_migrating("actor-1")
        assert not tracker.is_migrating("actor-2")

    def test_is_migrating_completed(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.COMPLETED)
        assert not tracker.is_migrating("actor-1")

    def test_get_migrating_actors(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.start_migration("actor-2", "node-a", "node-b")
        tracker.start_migration("actor-3", "node-a", "node-b")
        tracker.update_status("actor-2", MigrationStatus.COMPLETED)
        migrating = tracker.get_migrating_actors()
        assert "actor-1" in migrating
        assert "actor-3" in migrating
        assert "actor-2" not in migrating

    def test_get_active_migrations(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.QUIESCING)
        tracker.start_migration("actor-2", "node-a", "node-b")
        active = tracker.get_active_migrations()
        assert len(active) == 2

    def test_get_active_excludes_completed(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.COMPLETED)
        assert tracker.get_active_migrations() == []

    def test_get_history(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "node-a", "node-b")
        tracker.update_status("actor-1", MigrationStatus.COMPLETED)
        tracker.start_migration("actor-2", "node-a", "node-b")
        tracker.update_status("actor-2", MigrationStatus.FAILED, error="oops")
        history = tracker.get_history()
        assert len(history) == 2
        # Most recent first
        assert history[0]["actor_id"] == "actor-2"

    def test_get_history_limit(self):
        tracker = MigrationStateTracker()
        for i in range(10):
            tracker.start_migration(f"actor-{i}", "n1", "n2")
            tracker.update_status(f"actor-{i}", MigrationStatus.COMPLETED)
        history = tracker.get_history(limit=3)
        assert len(history) == 3

    def test_cleanup_old_records(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "n1", "n2")
        tracker.update_status("actor-1", MigrationStatus.COMPLETED)
        # Set completed_at to the past
        rec = tracker._migrations["actor-1"]
        rec.completed_at = time.time() - 7200  # 2 hours ago
        removed = tracker.cleanup_old_records(max_age_seconds=3600)
        assert removed == 1
        assert tracker.get_migration("actor-1") is None

    def test_cleanup_preserves_active(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "n1", "n2")
        removed = tracker.cleanup_old_records()
        assert removed == 0
        assert tracker.is_migrating("actor-1")

    def test_get_stats(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "n1", "n2")
        tracker.start_migration("actor-2", "n1", "n2")
        tracker.update_status("actor-2", MigrationStatus.COMPLETED)
        stats = tracker.get_stats()
        assert stats["active"] == 1
        assert stats["total_completed"] == 1
        assert stats["total_failed"] == 0
        assert stats["total_records"] == 2

    def test_clear(self):
        tracker = MigrationStateTracker()
        tracker.start_migration("actor-1", "n1", "n2")
        tracker.update_status("actor-1", MigrationStatus.COMPLETED)
        tracker.start_migration("actor-2", "n1", "n2")
        tracker.clear()
        assert tracker.get_stats() == {
            "active": 0,
            "total_completed": 0,
            "total_failed": 0,
            "total_records": 0,
        }
        assert not tracker.is_migrating("actor-2")

    def test_concurrent_access(self):
        """Test thread safety with concurrent access."""
        import threading

        tracker = MigrationStateTracker()
        errors = []

        def writer(i):
            try:
                aid = f"actor-{i}"
                tracker.start_migration(aid, "n1", "n2")
                tracker.update_status(aid, MigrationStatus.COMPLETED)
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=writer, args=(i,)) for i in range(100)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert not errors
        stats = tracker.get_stats()
        assert stats["total_completed"] == 100
