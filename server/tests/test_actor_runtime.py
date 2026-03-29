"""Tests for the server-side actor runtime."""

import asyncio
import pytest

from server.actor_runtime import (
    ActorCell,
    ActorContext,
    ActorRuntime,
    SupervisionStrategy,
    _noop_handler,
)
from server.config import ServerConfig
from server.event_store import EventStore
from server.message_router import MessageRouter
from server.models import MessageEnvelope
from server.state_store import MemoryStateStore
from server.actor_manager import ActorManager


# ============================================================
# Fixtures
# ============================================================

@pytest.fixture
def components():
    config = ServerConfig()
    actors = ActorManager(config)
    messages = MessageRouter(message_ttl=300)
    state = MemoryStateStore()
    events = EventStore()
    runtime = ActorRuntime(
        message_router=messages,
        actor_manager=actors,
        state_store=state,
    )
    yield runtime, actors, messages, state


# ============================================================
# Handler Registration Tests
# ============================================================

class TestHandlerRegistration:

    def test_register_handler_creates_context(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        ctx = runtime.register_handler("actor-1", handler, "worker")

        assert ctx.actor_id == "actor-1"
        assert ctx.actor_type == "worker"
        assert ctx.state == {}

    def test_register_auto_registers_actor(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("actor-1", handler)
        info = actors.get_actor("actor-1")
        assert info is not None
        assert info.status == "active"

    def test_register_existing_actor_uses_existing(self, components):
        runtime, actors, messages, state = components
        actors.register(actor_id="actor-1", actor_type="pre-registered")

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("actor-1", handler, "worker")
        info = actors.get_actor("actor-1")
        assert info.actor_type == "pre-registered"

    def test_unregister_handler(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("actor-1", handler)
        assert runtime.unregister_handler("actor-1") is True
        assert runtime.get_context("actor-1") is None

    def test_unregister_nonexistent_returns_false(self, components):
        runtime, actors, messages, state = components
        assert runtime.unregister_handler("ghost") is False

    def test_register_multiple_handlers(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("a1", handler)
        runtime.register_handler("a2", handler)
        runtime.register_handler("a3", handler)
        assert runtime.total_registered == 3


# ============================================================
# Message Dispatch Tests
# ============================================================

class TestMessageDispatch:

    @pytest.mark.asyncio
    async def test_dispatch_invokes_handler(self, components):
        runtime, actors, messages, state = components
        received = []

        async def handler(ctx, envelope):
            received.append(envelope)

        runtime.register_handler("actor-1", handler)

        envelope = MessageEnvelope(
            source_actor="sender",
            target_actor="actor-1",
            message_type="test",
            payload={"key": "value"},
        )
        result = await runtime.dispatch_to("actor-1", envelope)

        assert result is True
        assert len(received) == 1
        assert received[0].payload == {"key": "value"}

    @pytest.mark.asyncio
    async def test_dispatch_nonexistent_returns_false(self, components):
        runtime, actors, messages, state = components
        envelope = MessageEnvelope(source_actor="s", target_actor="ghost")
        result = await runtime.dispatch_to("ghost", envelope)
        assert result is False

    @pytest.mark.asyncio
    async def test_dispatch_via_router(self, components):
        runtime, actors, messages, state = components
        received = []

        async def handler(ctx, envelope):
            received.append(envelope)

        runtime.register_handler("actor-1", handler)

        # Send via message router (which triggers the registered dispatcher)
        receipt = await messages.route(MessageEnvelope(
            source_actor="sender",
            target_actor="actor-1",
            message_type="test",
        ))

        assert receipt.status == "delivered"
        # Give async task time to complete
        await asyncio.sleep(0.1)
        assert len(received) == 1


# ============================================================
# Actor State Tests
# ============================================================

class TestActorState:

    @pytest.mark.asyncio
    async def test_handler_can_modify_state(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            count = ctx.get_state("count", 0)
            ctx.set_state("count", count + 1)

        runtime.register_handler("counter", handler)

        await runtime.dispatch_to("counter", MessageEnvelope(source_actor="s", target_actor="counter"))
        await runtime.dispatch_to("counter", MessageEnvelope(source_actor="s", target_actor="counter"))
        await runtime.dispatch_to("counter", MessageEnvelope(source_actor="s", target_actor="counter"))

        ctx = runtime.get_context("counter")
        assert ctx.get_state("count") == 3


# ============================================================
# Supervision Tests
# ============================================================

class TestSupervision:

    @pytest.mark.asyncio
    async def test_restart_strategy_resets_state(self, components):
        runtime, actors, messages, state = components

        call_count = [0]

        async def failing_handler(ctx, envelope):
            call_count[0] += 1
            ctx.set_state("data", f"attempt_{call_count[0]}")
            if call_count[0] <= 1:
                raise RuntimeError("Intentional failure")

        runtime.register_handler("flaky", failing_handler,
                                  supervision_strategy=SupervisionStrategy.RESTART)

        await runtime.dispatch_to("flaky", MessageEnvelope(source_actor="s", target_actor="flaky"))

        # After restart, state should be cleared
        ctx = runtime.get_context("flaky")
        assert ctx.get_state("data") is None  # State was reset on restart
        info = runtime.get_cell_info("flaky")
        assert info["restart_count"] == 1

    @pytest.mark.asyncio
    async def test_resume_strategy_keeps_state(self, components):
        runtime, actors, messages, state = components

        async def failing_handler(ctx, envelope):
            ctx.set_state("data", "preserved")
            raise RuntimeError("Intentional failure")

        runtime.register_handler("resuming", failing_handler,
                                  supervision_strategy=SupervisionStrategy.RESUME)

        await runtime.dispatch_to("resuming", MessageEnvelope(source_actor="s", target_actor="resuming"))

        ctx = runtime.get_context("resuming")
        assert ctx.get_state("data") == "preserved"  # State preserved
        info = runtime.get_cell_info("resuming")
        assert info["status"] == "active"

    @pytest.mark.asyncio
    async def test_stop_strategy_stops_actor(self, components):
        runtime, actors, messages, state = components

        async def failing_handler(ctx, envelope):
            raise RuntimeError("Fatal error")

        runtime.register_handler("stopping", failing_handler,
                                  supervision_strategy=SupervisionStrategy.STOP)

        await runtime.dispatch_to("stopping", MessageEnvelope(source_actor="s", target_actor="stopping"))

        info = runtime.get_cell_info("stopping")
        assert info["status"] == "failed"

    @pytest.mark.asyncio
    async def test_max_restarts_exceeded(self, components):
        runtime, actors, messages, state = components

        async def always_failing_handler(ctx, envelope):
            raise RuntimeError("Always fails")

        runtime.register_handler("doomed", always_failing_handler,
                                  supervision_strategy=SupervisionStrategy.RESTART,
                                  max_restarts=2)

        # Trigger failures to exhaust restart limit
        for _ in range(5):
            await runtime.dispatch_to("doomed", MessageEnvelope(source_actor="s", target_actor="doomed"))

        info = runtime.get_cell_info("doomed")
        assert info["status"] == "failed"
        assert info["restart_count"] >= 2


# ============================================================
# Child Actor / Supervision Tree Tests
# ============================================================

class TestSupervisionTree:

    @pytest.mark.asyncio
    async def test_spawn_child(self, components):
        runtime, actors, messages, state = components

        async def parent_handler(ctx, envelope):
            pass

        async def child_handler(ctx, envelope):
            pass

        ctx = runtime.register_handler("parent", parent_handler)
        child_ctx = ctx.spawn("child-1", "worker", child_handler)

        assert child_ctx is not None
        assert child_ctx.actor_id == "child-1"

        parent_info = runtime.get_cell_info("parent")
        assert "child-1" in parent_info["children"]

        child_info = runtime.get_cell_info("child-1")
        assert child_info["parent_id"] == "parent"

    @pytest.mark.asyncio
    async def test_stop_parent_stops_children(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        ctx = runtime.register_handler("parent", handler)
        ctx.spawn("child-1", "worker", handler)
        ctx.spawn("child-2", "worker", handler)

        runtime.unregister_handler("parent")

        assert runtime.get_context("parent") is None
        assert runtime.get_context("child-1") is None
        assert runtime.get_context("child-2") is None

    @pytest.mark.asyncio
    async def test_spawn_without_runtime_returns_none(self):
        ctx = ActorContext(actor_id="orphan", actor_type="test")
        result = ctx.spawn("child", "test")
        assert result is None

    @pytest.mark.asyncio
    async def test_escalate_to_parent(self, components):
        runtime, actors, messages, state = components

        async def child_handler(ctx, envelope):
            raise RuntimeError("Child failure")

        async def parent_handler(ctx, envelope):
            pass

        ctx = runtime.register_handler("parent", parent_handler,
                                        supervision_strategy=SupervisionStrategy.STOP)
        ctx.spawn("child", "worker", child_handler,
                  supervision_strategy=SupervisionStrategy.ESCALATE)

        await runtime.dispatch_to("child", MessageEnvelope(source_actor="s", target_actor="child"))

        # Parent should be stopped due to escalation
        parent_info = runtime.get_cell_info("parent")
        assert parent_info["status"] == "failed"


# ============================================================
# Mailbox Management Tests
# ============================================================

class TestMailbox:

    @pytest.mark.asyncio
    async def test_get_pending_count(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            raise RuntimeError("Always fails")

        runtime.register_handler("mailbox-actor", handler,
                                  supervision_strategy=SupervisionStrategy.RESUME)

        # Send multiple messages
        for i in range(5):
            await runtime.dispatch_to("mailbox-actor",
                                      MessageEnvelope(source_actor="s", target_actor="mailbox-actor"))

        # Handler was invoked for each (even though it fails)
        info = runtime.get_cell_info("mailbox-actor")
        assert info["message_count"] == 5
        assert info["error_count"] == 5

    def test_drain_mailbox(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("drain-me", handler)
        count = runtime.drain_mailbox("drain-me")
        assert count == 0  # Empty mailbox

        # Nonexistent actor
        count = runtime.drain_mailbox("ghost")
        assert count == 0


# ============================================================
# Diagnostic Tests
# ============================================================

class TestDiagnostics:

    def test_list_cells_empty(self, components):
        runtime, actors, messages, state = components
        assert runtime.list_cells() == []

    def test_list_cells(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("a1", handler, "worker")
        runtime.register_handler("a2", handler, "scheduler")

        cells = runtime.list_cells()
        assert len(cells) == 2
        actor_ids = {c["actor_id"] for c in cells}
        assert actor_ids == {"a1", "a2"}

    def test_active_count(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        assert runtime.active_count == 0
        runtime.register_handler("a1", handler)
        assert runtime.active_count == 1
        runtime.register_handler("a2", handler)
        assert runtime.active_count == 2

    def test_get_cell_info_nonexistent(self, components):
        runtime, actors, messages, state = components
        assert runtime.get_cell_info("ghost") is None

    def test_get_cell_info(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("info-actor", handler, "worker")
        info = runtime.get_cell_info("info-actor")

        assert info["actor_id"] == "info-actor"
        assert info["actor_type"] == "worker"
        assert info["status"] == "active"
        assert info["message_count"] == 0
        assert info["error_count"] == 0


# ============================================================
# Lifecycle Tests
# ============================================================

class TestLifecycle:

    @pytest.mark.asyncio
    async def test_stop_all(self, components):
        runtime, actors, messages, state = components

        async def handler(ctx, envelope):
            pass

        runtime.register_handler("a1", handler)
        runtime.register_handler("a2", handler)
        runtime.register_handler("a3", handler)

        await runtime.stop_all()

        assert runtime.total_registered == 0
        assert runtime.active_count == 0


# ============================================================
# ActorContext Tests
# ============================================================

class TestActorContext:

    def test_get_set_state(self):
        ctx = ActorContext(actor_id="test", actor_type="test")
        assert ctx.get_state("x") is None
        assert ctx.get_state("x", 42) == 42

        ctx.set_state("x", 100)
        assert ctx.get_state("x") == 100

    def test_spawn_without_runtime(self):
        ctx = ActorContext(actor_id="test", actor_type="test")
        result = ctx.spawn("child", "test")
        assert result is None
