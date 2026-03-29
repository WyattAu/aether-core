"""Unit tests for the migration coordinator."""

import asyncio
from unittest.mock import MagicMock, patch, AsyncMock

import pytest

from server.cluster.config import ClusterConfig
from server.cluster.migration import MigrationCoordinator
from server.cluster.migration_state import MigrationStatus
from server.cluster.node import ClusterNode, NodeStatus


def _make_config(**overrides) -> ClusterConfig:
    defaults = {
        "enabled": True,
        "migration_enabled": True,
        "migration_timeout_seconds": 5.0,
        "migration_drain_timeout_seconds": 1.0,
        "migration_batch_size": 5,
    }
    defaults.update(overrides)
    return ClusterConfig(**defaults)


def _make_membership(node_id="node-a", is_leader=True, members=None):
    membership = MagicMock()
    membership.node_id = node_id
    membership.is_leader = is_leader
    membership.is_running = True

    def get_node_for_key(key):
        if members is None:
            return None
        # Simple deterministic routing: if any member is not self, return it
        for nid, node in members.items():
            if nid != node_id and node.is_alive():
                return node
        return None

    membership.get_node_for_key = get_node_for_key
    membership.get_member = lambda nid: members.get(nid) if members else None
    membership.alive_nodes = []
    return membership


def _make_node(node_id="node-b", alive=True):
    return ClusterNode(
        node_id=node_id,
        host="127.0.0.1",
        api_port=8080,
        status=NodeStatus.ALIVE if alive else NodeStatus.DEAD,
        incarnation=1,
    )


def _make_runtime():
    runtime = MagicMock()
    runtime.get_registered_actor_ids = MagicMock(return_value=[])
    runtime.quiesce_actor = MagicMock(return_value=True)
    runtime.drain_actor = MagicMock(return_value=0)
    runtime.snapshot_actor = MagicMock(return_value=None)
    runtime.unregister_handler = MagicMock(return_value=True)
    return runtime


class TestMigrationCoordinatorInit:
    """Tests for coordinator initialization."""

    def test_init_with_defaults(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        assert coord.get_stats()["active"] == 0
        assert coord.get_stats()["migrated_in"] == 0
        assert coord.get_stats()["migrated_out"] == 0

    def test_handler_registry_empty(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        assert coord._get_handler("nonexistent") is None

    def test_register_handler_factory(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()

        def factory():
            return lambda ctx, env: None

        coord = MigrationCoordinator(membership, transport, config)
        coord.register_handler_factory("my-type", factory)
        handler = coord._get_handler("my-type")
        assert handler is not None
        assert callable(handler)

    def test_handler_factory_exception(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()

        def bad_factory():
            raise RuntimeError("no handler")

        coord = MigrationCoordinator(membership, transport, config)
        coord.register_handler_factory("bad-type", bad_factory)
        assert coord._get_handler("bad-type") is None


class TestShouldMigrate:
    """Tests for migration eligibility checking."""

    def test_should_migrate_yes(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        # Hash ring says node-b owns this key, and we're node-a (leader)
        target = coord.should_migrate("some-actor")
        assert target == "node-b"

    def test_should_migrate_no_already_local(self):
        config = _make_config()
        membership = _make_membership(node_id="node-a", members={})
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        # No other nodes — stays local
        assert coord.should_migrate("some-actor") is None

    def test_should_migrate_no_not_leader(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            is_leader=False,
            members={"node-b": node_b},
        )
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        assert coord.should_migrate("some-actor") is None

    def test_should_migrate_no_cluster_not_running(self):
        config = _make_config()
        membership = _make_membership()
        membership.is_running = False
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        assert coord.should_migrate("some-actor") is None

    def test_should_migrate_no_dead_target(self):
        config = _make_config()
        node_b = _make_node("node-b", alive=False)
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        assert coord.should_migrate("some-actor") is None


class TestComputeRebalance:
    """Tests for rebalance computation."""

    def test_empty_local_actors(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        plans = coord.compute_rebalance([])
        assert plans == []

    def test_no_migration_needed(self):
        config = _make_config()
        membership = _make_membership(node_id="node-a", members={})
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        plans = coord.compute_rebalance(["actor-1", "actor-2"])
        assert plans == []

    def test_migration_needed(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        plans = coord.compute_rebalance(["actor-1"])
        assert len(plans) == 1
        assert plans[0]["actor_id"] == "actor-1"
        assert plans[0]["target_node_id"] == "node-b"

    def test_already_migrating_skipped(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        coord._tracker.start_migration("actor-1", "node-a", "node-b")
        plans = coord.compute_rebalance(["actor-1"])
        assert plans == []


class TestReceiveMigration:
    """Tests for inbound migration handling."""

    def test_receive_success(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        runtime = _make_runtime()
        runtime.get_cell_info = MagicMock(return_value=None)  # No existing actor
        runtime.restore_actor = MagicMock(return_value=MagicMock())

        coord = MigrationCoordinator(membership, transport, config, actor_runtime=runtime)

        handler_called = False
        def factory():
            nonlocal handler_called
            handler_called = True
            return lambda ctx, env: None
        coord.register_handler_factory("default", factory)

        payload = {
            "actor_id": "actor-1",
            "actor_type": "default",
            "state": {"key": "value"},
            "persistent_state": {},
            "pending_messages": [],
            "source_node": "node-b",
        }
        result = coord.receive_migration(payload)
        assert result["status"] == "accepted"
        assert handler_called
        assert coord._total_migrated_in == 1

    def test_receive_rejected_missing_actor_id(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        result = coord.receive_migration({})
        assert result["status"] == "rejected"
        assert "missing actor_id" in result["error"]

    def test_receive_rejected_already_exists(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        runtime = _make_runtime()
        runtime.get_cell_info = MagicMock(return_value={"actor_id": "actor-1"})
        coord = MigrationCoordinator(membership, transport, config, actor_runtime=runtime)

        def factory():
            return lambda ctx, env: None
        coord.register_handler_factory("default", factory)

        payload = {"actor_id": "actor-1", "actor_type": "default"}
        result = coord.receive_migration(payload)
        assert result["status"] == "rejected"
        assert "already exists" in result["error"]

    def test_receive_rejected_no_handler(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        runtime = _make_runtime()
        runtime.get_cell_info = MagicMock(return_value=None)
        coord = MigrationCoordinator(membership, transport, config, actor_runtime=runtime)
        # No handler registered for type "unknown-type"

        payload = {"actor_id": "actor-1", "actor_type": "unknown-type"}
        result = coord.receive_migration(payload)
        assert result["status"] == "rejected"
        assert "no handler" in result["error"]

    def test_receive_restores_persistent_state(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        runtime = _make_runtime()
        runtime.get_cell_info = MagicMock(return_value=None)
        runtime.restore_actor = MagicMock(return_value=MagicMock())

        state_store = MagicMock()
        coord = MigrationCoordinator(
            membership, transport, config,
            actor_runtime=runtime, state_store=state_store,
        )

        def factory():
            return lambda ctx, env: None
        coord.register_handler_factory("default", factory)

        payload = {
            "actor_id": "actor-1",
            "actor_type": "default",
            "state": {},
            "persistent_state": {"count": 42, "name": "test"},
            "pending_messages": [],
        }
        result = coord.receive_migration(payload)
        assert result["status"] == "accepted"
        assert state_store.set.call_count == 2


class TestMigrateActor:
    """Tests for outbound migration."""

    @pytest.mark.asyncio
    async def test_migrate_success(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        transport.migrate_actor = MagicMock(return_value={
            "status": "accepted",
        })

        runtime = _make_runtime()
        runtime.snapshot_actor = MagicMock(return_value={
            "actor_id": "actor-1",
            "actor_type": "default",
            "state": {"count": 10},
            "status": "paused",
            "mailbox_size": 0,
            "message_count": 5,
            "error_count": 0,
            "supervision_strategy": "restart",
            "parent_id": None,
            "children": [],
        })

        coord = MigrationCoordinator(
            membership, transport, config, actor_runtime=runtime,
        )

        result = await coord.migrate_actor("actor-1", "node-b")
        assert result["status"] == "completed"
        assert result["source_node"] == "node-a"
        assert result["target_node"] == "node-b"
        assert coord._total_migrated_out == 1
        runtime.quiesce_actor.assert_called_once_with("actor-1")
        runtime.snapshot_actor.assert_called_once_with("actor-1")
        runtime.unregister_handler.assert_called_once_with("actor-1")

    @pytest.mark.asyncio
    async def test_migrate_already_migrating(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        coord._tracker.start_migration("actor-1", "node-a", "node-b")

        result = await coord.migrate_actor("actor-1", "node-b")
        assert result["status"] == "skipped"

    @pytest.mark.asyncio
    async def test_migrate_quiesce_fails(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        runtime = _make_runtime()
        runtime.quiesce_actor = MagicMock(return_value=False)

        coord = MigrationCoordinator(
            membership, transport, config, actor_runtime=runtime,
        )

        result = await coord.migrate_actor("actor-1", "node-b")
        assert result["status"] == "failed"
        assert "not found or not active" in result["error"]

    @pytest.mark.asyncio
    async def test_migrate_transfer_fails(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        transport.migrate_actor = MagicMock(return_value=None)

        runtime = _make_runtime()
        runtime.snapshot_actor = MagicMock(return_value={
            "actor_id": "actor-1",
            "actor_type": "default",
            "state": {},
            "status": "paused",
            "mailbox_size": 0,
            "message_count": 0,
            "error_count": 0,
            "supervision_strategy": "restart",
            "parent_id": None,
            "children": [],
        })

        coord = MigrationCoordinator(
            membership, transport, config, actor_runtime=runtime,
        )

        result = await coord.migrate_actor("actor-1", "node-b")
        assert result["status"] == "failed"
        assert "Failed to transfer" in result["error"]

    @pytest.mark.asyncio
    async def test_migrate_target_rejects(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        transport.migrate_actor = MagicMock(return_value={
            "status": "rejected",
            "error": "actor already exists",
        })

        runtime = _make_runtime()
        runtime.snapshot_actor = MagicMock(return_value={
            "actor_id": "actor-1",
            "actor_type": "default",
            "state": {},
            "status": "paused",
            "mailbox_size": 0,
            "message_count": 0,
            "error_count": 0,
            "supervision_strategy": "restart",
            "parent_id": None,
            "children": [],
        })

        coord = MigrationCoordinator(
            membership, transport, config, actor_runtime=runtime,
        )

        result = await coord.migrate_actor("actor-1", "node-b")
        assert result["status"] == "failed"
        assert "rejected" in result["error"]

    @pytest.mark.asyncio
    async def test_migrate_no_runtime(self):
        config = _make_config()
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        # No actor_runtime set

        result = await coord.migrate_actor("actor-1", "node-b")
        assert result["status"] == "failed"
        assert "No actor runtime" in result["error"]


class TestRebalance:
    """Tests for the rebalance batch operation."""

    @pytest.mark.asyncio
    async def test_rebalance_not_leader(self):
        config = _make_config()
        membership = _make_membership(is_leader=False)
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        result = await coord.rebalance(["actor-1"])
        assert result["status"] == "not_leader"

    @pytest.mark.asyncio
    async def test_rebalance_already_balanced(self):
        config = _make_config()
        membership = _make_membership(node_id="node-a", members={})
        transport = MagicMock()
        runtime = _make_runtime()
        coord = MigrationCoordinator(
            membership, transport, config, actor_runtime=runtime,
        )
        result = await coord.rebalance(["actor-1"])
        assert result["status"] == "balanced"

    @pytest.mark.asyncio
    async def test_rebalance_executes_batch(self):
        config = _make_config(migration_batch_size=2)
        node_b = _make_node("node-b")
        membership = _make_membership(
            node_id="node-a",
            members={"node-b": node_b},
        )
        transport = MagicMock()
        transport.migrate_actor = MagicMock(return_value={"status": "accepted"})

        runtime = _make_runtime()
        runtime.snapshot_actor = MagicMock(return_value={
            "actor_id": "x",
            "actor_type": "default",
            "state": {},
            "status": "paused",
            "mailbox_size": 0,
            "message_count": 0,
            "error_count": 0,
            "supervision_strategy": "restart",
            "parent_id": None,
            "children": [],
        })

        coord = MigrationCoordinator(
            membership, transport, config, actor_runtime=runtime,
        )

        result = await coord.rebalance(["actor-1", "actor-2", "actor-3"])
        assert result["status"] == "rebalanced"
        assert result["planned"] == 3
        assert result["executed"] == 2  # batch_size=2
        assert result["remaining"] == 1
        assert result["completed"] == 2


class TestMigrationStats:
    """Tests for migration statistics and status."""

    def test_initial_stats(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        stats = coord.get_stats()
        assert stats["active"] == 0
        assert stats["migrated_in"] == 0
        assert stats["migrated_out"] == 0

    def test_get_status(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        status = coord.get_status()
        assert status["is_leader"] is True
        assert status["node_id"] == "node-a"
        assert "stats" in status
        assert "active_migrations" in status

    def test_is_migrating_delegates_to_tracker(self):
        config = _make_config()
        membership = _make_membership()
        transport = MagicMock()
        coord = MigrationCoordinator(membership, transport, config)
        assert not coord.is_migrating("nonexistent")
        coord._tracker.start_migration("actor-1", "n1", "n2")
        assert coord.is_migrating("actor-1")
