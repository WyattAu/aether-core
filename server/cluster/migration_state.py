"""Migration state tracking.

Tracks actors that are currently being migrated between nodes,
preventing duplicate migrations and routing conflicts.
"""

import enum
import threading
import time
from dataclasses import dataclass, field
from typing import Dict, Optional, Set


class MigrationStatus(str, enum.Enum):
    """Status of an in-flight actor migration."""
    QUEUED = "queued"           # Waiting to be migrated
    QUIESCING = "quiescing"     # Actor is being paused
    TRANSFERRING = "transferring"  # State is being sent to target
    COMPLETED = "completed"     # Migration succeeded
    FAILED = "failed"           # Migration failed
    CANCELLED = "cancelled"     # Migration was cancelled


@dataclass
class MigrationRecord:
    """Record of a single actor migration."""

    actor_id: str
    source_node: str
    target_node: str
    status: MigrationStatus = MigrationStatus.QUEUED
    started_at: float = field(default_factory=time.time)
    completed_at: Optional[float] = None
    error: Optional[str] = None
    state_size: int = 0         # Approximate size of serialized state

    @property
    def duration(self) -> float:
        """Time elapsed since migration started."""
        end = self.completed_at or time.time()
        return end - self.started_at

    def to_dict(self) -> dict:
        """Serialize to dict."""
        return {
            "actor_id": self.actor_id,
            "source_node": self.source_node,
            "target_node": self.target_node,
            "status": self.status.value,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "duration": self.duration,
            "error": self.error,
            "state_size": self.state_size,
        }


class MigrationStateTracker:
    """Thread-safe tracker for in-flight actor migrations.

    Tracks which actors are currently being migrated, from where
    to where, and their current status. Prevents duplicate
    migrations of the same actor.
    """

    def __init__(self):
        self._migrations: Dict[str, MigrationRecord] = {}  # actor_id -> record
        self._lock = threading.Lock()
        self._total_completed = 0
        self._total_failed = 0

    def start_migration(
        self,
        actor_id: str,
        source_node: str,
        target_node: str,
    ) -> bool:
        """Register a new migration.

        Returns False if the actor is already being migrated.

        Args:
            actor_id: The actor being migrated.
            source_node: Node ID of the source.
            target_node: Node ID of the target.
        """
        with self._lock:
            if actor_id in self._migrations:
                existing = self._migrations[actor_id]
                if existing.status not in (MigrationStatus.COMPLETED,
                                           MigrationStatus.FAILED,
                                           MigrationStatus.CANCELLED):
                    return False
            self._migrations[actor_id] = MigrationRecord(
                actor_id=actor_id,
                source_node=source_node,
                target_node=target_node,
            )
            return True

    def update_status(self, actor_id: str, status: MigrationStatus,
                      error: Optional[str] = None) -> None:
        """Update the status of a migration."""
        with self._lock:
            record = self._migrations.get(actor_id)
            if record is None:
                return
            record.status = status
            record.error = error
            if status == MigrationStatus.COMPLETED:
                record.completed_at = time.time()
                self._total_completed += 1
            elif status == MigrationStatus.FAILED:
                record.completed_at = time.time()
                self._total_failed += 1
            elif status == MigrationStatus.CANCELLED:
                record.completed_at = time.time()

    def set_state_size(self, actor_id: str, size: int) -> None:
        """Record the approximate size of the migrated state."""
        with self._lock:
            record = self._migrations.get(actor_id)
            if record is not None:
                record.state_size = size

    def get_migration(self, actor_id: str) -> Optional[MigrationRecord]:
        """Get the migration record for an actor."""
        with self._lock:
            record = self._migrations.get(actor_id)
            if record is None:
                return None
            # Return a copy
            return MigrationRecord(
                actor_id=record.actor_id,
                source_node=record.source_node,
                target_node=record.target_node,
                status=record.status,
                started_at=record.started_at,
                completed_at=record.completed_at,
                error=record.error,
                state_size=record.state_size,
            )

    def is_migrating(self, actor_id: str) -> bool:
        """Check if an actor is currently being migrated."""
        with self._lock:
            record = self._migrations.get(actor_id)
            if record is None:
                return False
            return record.status in (
                MigrationStatus.QUEUED,
                MigrationStatus.QUIESCING,
                MigrationStatus.TRANSFERRING,
            )

    def get_migrating_actors(self) -> Set[str]:
        """Get IDs of all actors currently being migrated."""
        with self._lock:
            return {
                aid for aid, rec in self._migrations.items()
                if rec.status in (
                    MigrationStatus.QUEUED,
                    MigrationStatus.QUIESCING,
                    MigrationStatus.TRANSFERRING,
                )
            }

    def get_active_migrations(self) -> list:
        """Get all active (in-flight) migration records."""
        with self._lock:
            return [
                rec.to_dict() for rec in self._migrations.values()
                if rec.status in (
                    MigrationStatus.QUEUED,
                    MigrationStatus.QUIESCING,
                    MigrationStatus.TRANSFERRING,
                )
            ]

    def get_history(self, limit: int = 50) -> list:
        """Get recent migration history (completed/failed/cancelled)."""
        with self._lock:
            records = sorted(
                [rec for rec in self._migrations.values()
                 if rec.status in (
                     MigrationStatus.COMPLETED,
                     MigrationStatus.FAILED,
                     MigrationStatus.CANCELLED,
                 )],
                key=lambda r: r.completed_at or 0,
                reverse=True,
            )
            return [rec.to_dict() for rec in records[:limit]]

    def cleanup_old_records(self, max_age_seconds: float = 3600.0) -> int:
        """Remove old completed/failed/cancelled records."""
        now = time.time()
        to_remove = []
        with self._lock:
            for aid, rec in self._migrations.items():
                if (rec.status in (
                    MigrationStatus.COMPLETED,
                    MigrationStatus.FAILED,
                    MigrationStatus.CANCELLED,
                ) and rec.completed_at is not None
                    and now - rec.completed_at > max_age_seconds):
                    to_remove.append(aid)
            for aid in to_remove:
                del self._migrations[aid]
        return len(to_remove)

    def get_stats(self) -> dict:
        """Get migration statistics."""
        with self._lock:
            active = sum(
                1 for rec in self._migrations.values()
                if rec.status in (
                    MigrationStatus.QUEUED,
                    MigrationStatus.QUIESCING,
                    MigrationStatus.TRANSFERRING,
                )
            )
            return {
                "active": active,
                "total_completed": self._total_completed,
                "total_failed": self._total_failed,
                "total_records": len(self._migrations),
            }

    def clear(self) -> None:
        """Clear all migration records (for testing)."""
        with self._lock:
            self._migrations.clear()
            self._total_completed = 0
            self._total_failed = 0
