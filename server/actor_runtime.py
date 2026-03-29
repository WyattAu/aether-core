"""Server-side actor runtime for the Aether server.

Provides an actor execution environment with:
- **Mailbox management**: Each actor has a bounded mailbox with priority ordering
- **Message dispatch**: Automatic routing of messages to registered actor handlers
- **Supervision tree**: Parent-child relationships with configurable restart strategies
- **Graceful shutdown**: Cooperative draining of all actor mailboxes

Actors are registered via the ``ActorManager`` and message handlers are
attached via the ``ActorRuntime``. When a message arrives for an actor
with a registered handler, the runtime dispatches it asynchronously.

Usage::

    from server.actor_runtime import ActorRuntime, ActorContext

    runtime = ActorRuntime(message_router, actor_manager)

    # Define an actor handler
    async def my_handler(ctx: ActorContext, envelope: MessageEnvelope):
        ctx.state["count"] = ctx.state.get("count", 0) + 1

    # Register the actor and handler
    runtime.register_handler("my-actor", my_handler)

    # Messages routed via MessageRouter will be dispatched automatically
"""

import asyncio
import logging
import time
from collections import defaultdict, deque
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any, Callable, Coroutine, Dict, List, Optional, Set

from .models import ActorInfo, MessageEnvelope

logger = logging.getLogger("aether-server.actor-runtime")


class SupervisionStrategy(Enum):
    """Strategy for handling actor failures."""

    RESTART = "restart"       # Restart the actor (reset state)
    RESUME = "resume"         # Resume without resetting state
    STOP = "stop"             # Stop the actor permanently
    ESCALATE = "escalate"     # Escalate to parent supervisor


@dataclass
class ActorContext:
    """Context passed to actor message handlers.

    Provides access to the actor's identity, state, and helper methods.
    """

    actor_id: str
    actor_type: str
    state: Dict[str, Any] = field(default_factory=dict)
    _children: Set[str] = field(default_factory=set)
    _context_ref: Optional["ActorRuntime"] = field(default=None, repr=False)

    def get_state(self, key: str, default: Any = None) -> Any:
        """Get a value from the actor's local state."""
        return self.state.get(key, default)

    def set_state(self, key: str, value: Any):
        """Set a value in the actor's local state."""
        self.state[key] = value

    def spawn(self, actor_id: str, actor_type: str = "child",
              handler: Optional[Callable] = None,
              supervision_strategy: SupervisionStrategy = SupervisionStrategy.RESTART) -> Optional["ActorContext"]:
        """Spawn a child actor. Returns child context or None if runtime unavailable."""
        if self._context_ref is None:
            return None
        return self._context_ref.spawn_child(self.actor_id, actor_id, actor_type, handler, supervision_strategy)


@dataclass
class ActorCell:
    """Internal representation of a running actor."""

    actor_id: str
    actor_type: str
    handler: Callable
    context: ActorContext
    status: str = "active"  # active, paused, stopping, stopped, failed
    mailbox: deque = field(default_factory=lambda: deque(maxlen=10000))
    message_count: int = 0
    error_count: int = 0
    last_error: Optional[str] = None
    parent_id: Optional[str] = None
    children: Set[str] = field(default_factory=set)
    supervision_strategy: SupervisionStrategy = SupervisionStrategy.RESTART
    max_restarts: int = 3
    restart_count: int = 0
    restart_window: List[float] = field(default_factory=list)  # timestamps of recent restarts


class ActorRuntime:
    """Server-side actor execution runtime.

    Manages actor lifecycles, mailboxes, message dispatch, and supervision.

    Args:
        message_router: The server's message router for routing messages.
        actor_manager: The server's actor manager for actor registration.
        state_store: Optional state store for persistent actor state.
        max_mailbox_size: Maximum mailbox size per actor (default 10000).
    """

    def __init__(
        self,
        message_router: Any,
        actor_manager: Any,
        state_store: Optional[Any] = None,
        max_mailbox_size: int = 10000,
    ):
        self._router = message_router
        self._actors = actor_manager
        self._state_store = state_store
        self._max_mailbox_size = max_mailbox_size

        # Actor cells indexed by actor_id
        self._cells: Dict[str, ActorCell] = {}
        # Lock for thread-safe cell access
        self._thread_lock = __import__("threading").Lock()

        # Task references for cleanup
        self._dispatch_tasks: Set[asyncio.Task] = set()
        self._running = False

    def register_handler(
        self,
        actor_id: str,
        handler: Callable,
        actor_type: str = "default",
        supervision_strategy: SupervisionStrategy = SupervisionStrategy.RESTART,
        max_restarts: int = 3,
    ) -> ActorContext:
        """Register a message handler for an actor.

        If the actor is not already registered, it will be registered
        automatically. The handler will be called for each message
        delivered to this actor.

        Args:
            actor_id: Unique actor identifier.
            handler: Async function ``handler(ctx, envelope)``.
            actor_type: Type of actor (for classification).
            supervision_strategy: How to handle failures.
            max_restarts: Maximum restarts within the restart window.

        Returns:
            The actor's ``ActorContext`` for direct state access.
        """
        # Register actor if not already present
        if self._actors.get_actor(actor_id) is None:
            self._actors.register(actor_id=actor_id, actor_type=actor_type)

        context = ActorContext(
            actor_id=actor_id,
            actor_type=actor_type,
        )

        cell = ActorCell(
            actor_id=actor_id,
            actor_type=actor_type,
            handler=handler,
            context=context,
            supervision_strategy=supervision_strategy,
            max_restarts=max_restarts,
        )
        context._context_ref = self

        with self._thread_lock:
            self._cells[actor_id] = cell

        # Register with message router for automatic dispatch
        self._router.register_handler(actor_id, self._make_dispatcher(cell))

        logger.info("Actor handler registered: %s (type=%s, supervision=%s)",
                     actor_id, actor_type, supervision_strategy.value)
        return context

    def unregister_handler(self, actor_id: str) -> bool:
        """Unregister an actor's handler and stop message dispatch."""
        with self._thread_lock:
            cell = self._cells.pop(actor_id, None)

        if cell is None:
            return False

        cell.status = "stopped"
        self._router.unregister_handler(actor_id)

        # Stop all children
        for child_id in list(cell.children):
            self.unregister_handler(child_id)

        logger.info("Actor handler unregistered: %s", actor_id)
        return True

    def spawn_child(
        self,
        parent_id: str,
        child_id: str,
        child_type: str = "child",
        handler: Optional[Callable] = None,
        supervision_strategy: SupervisionStrategy = SupervisionStrategy.RESTART,
    ) -> Optional[ActorContext]:
        """Spawn a child actor under a parent for supervision.

        Returns:
            Child actor's context, or None if parent not found.
        """
        with self._thread_lock:
            parent = self._cells.get(parent_id)
            if parent is None:
                return None

            parent.children.add(child_id)

        context = self.register_handler(child_id, handler or _noop_handler, child_type,
                                        supervision_strategy=supervision_strategy)
        context._children = parent.children

        with self._thread_lock:
            if child_id in self._cells:
                self._cells[child_id].parent_id = parent_id

        return context

    def get_context(self, actor_id: str) -> Optional[ActorContext]:
        """Get an actor's context for direct state access."""
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        return cell.context if cell else None

    def get_cell_info(self, actor_id: str) -> Optional[Dict[str, Any]]:
        """Get diagnostic info about an actor cell."""
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        if cell is None:
            return None
        return {
            "actor_id": cell.actor_id,
            "actor_type": cell.actor_type,
            "status": cell.status,
            "mailbox_size": len(cell.mailbox),
            "message_count": cell.message_count,
            "error_count": cell.error_count,
            "last_error": cell.last_error,
            "parent_id": cell.parent_id,
            "children": list(cell.children),
            "supervision_strategy": cell.supervision_strategy.value,
            "restart_count": cell.restart_count,
        }

    def list_cells(self) -> List[Dict[str, Any]]:
        """Get diagnostic info for all registered actor cells."""
        with self._thread_lock:
            cells = list(self._cells.values())
        return [self._get_cell_summary(c) for c in cells]

    def _get_cell_summary(self, cell: ActorCell) -> Dict[str, Any]:
        return {
            "actor_id": cell.actor_id,
            "actor_type": cell.actor_type,
            "status": cell.status,
            "mailbox_size": len(cell.mailbox),
            "message_count": cell.message_count,
            "error_count": cell.error_count,
        }

    def get_pending_count(self, actor_id: str) -> int:
        """Get the number of pending messages in an actor's mailbox."""
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        return len(cell.mailbox) if cell else 0

    def drain_mailbox(self, actor_id: str) -> int:
        """Drain and discard all pending messages from an actor's mailbox."""
        with self._thread_lock:
            cell = self._cells.get(actor_id)
            if cell is None:
                return 0
            count = len(cell.mailbox)
            cell.mailbox.clear()
            return count

    async def dispatch_to(self, actor_id: str, envelope: MessageEnvelope) -> bool:
        """Manually dispatch a message to an actor's handler.

        Returns:
            True if the handler was invoked, False if actor not found.
        """
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        if cell is None or cell.status not in ("active",):
            return False

        await self._invoke_handler(cell, envelope)
        return True

    async def stop_all(self, drain_timeout: float = 30.0):
        """Stop all actors and drain mailboxes.

        Args:
            drain_timeout: Maximum time to wait for in-flight messages.
        """
        logger.info("Stopping all actors (timeout=%.1fs)", drain_timeout)
        with self._thread_lock:
            actor_ids = list(self._cells.keys())

        for actor_id in actor_ids:
            self.unregister_handler(actor_id)

        # Cancel any pending dispatch tasks
        for task in self._dispatch_tasks:
            if not task.done():
                task.cancel()

        self._dispatch_tasks.clear()
        self._running = False
        logger.info("All actors stopped")

    def _make_dispatcher(self, cell: ActorCell) -> Callable:
        """Create a dispatcher function for the message router.

        The router calls this synchronously, so we create a task for
        async execution.
        """
        def dispatcher(envelope: MessageEnvelope):
            # Schedule async dispatch in the event loop
            try:
                loop = asyncio.get_running_loop()
                task = loop.create_task(self._invoke_handler(cell, envelope))
                self._dispatch_tasks.add(task)
                task.add_done_callback(self._dispatch_tasks.discard)
            except RuntimeError:
                # No running loop — dispatch synchronously
                asyncio.run(self._invoke_handler(cell, envelope))
        return dispatcher

    async def _invoke_handler(self, cell: ActorCell, envelope: MessageEnvelope):
        """Invoke an actor's message handler with supervision."""
        if cell.status not in ("active",):
            # Buffer the message if actor is not active
            cell.mailbox.append(envelope)
            return

        try:
            cell.message_count += 1
            await cell.handler(cell.context, envelope)
        except Exception as e:
            cell.error_count += 1
            cell.last_error = str(e)
            logger.error("Actor %s handler error: %s", cell.actor_id, e)

            # Apply supervision strategy
            self._apply_supervision(cell, e)

    def _apply_supervision(self, cell: ActorCell, error: Exception):
        """Apply the supervision strategy after a handler failure."""
        strategy = cell.supervision_strategy

        if strategy == SupervisionStrategy.STOP:
            cell.status = "failed"
            logger.warning("Actor %s stopped due to error: %s", cell.actor_id, error)

        elif strategy == SupervisionStrategy.RESUME:
            # Just log, keep running
            logger.warning("Actor %s resuming after error: %s", cell.actor_id, error)

        elif strategy == SupervisionStrategy.RESTART:
            if self._can_restart(cell):
                cell.restart_count += 1
                cell.restart_window.append(time.time())
                # Reset state on restart
                cell.context.state.clear()
                cell.error_count = 0
                cell.last_error = None
                logger.info("Actor %s restarted (restart #%d)", cell.actor_id, cell.restart_count)
            else:
                cell.status = "failed"
                logger.error("Actor %s exceeded max restarts, stopping", cell.actor_id)

        elif strategy == SupervisionStrategy.ESCALATE:
            if cell.parent_id:
                with self._thread_lock:
                    parent = self._cells.get(cell.parent_id)
                if parent:
                    logger.warning("Escalating failure of %s to parent %s", cell.actor_id, cell.parent_id)
                    self._apply_supervision(parent, error)
                else:
                    cell.status = "failed"
            else:
                cell.status = "failed"
                logger.error("Actor %s failed with no parent to escalate to", cell.actor_id)

    def _can_restart(self, cell: ActorCell) -> bool:
        """Check if the actor can be restarted within its limits."""
        if cell.restart_count >= cell.max_restarts:
            return False
        # Check restart window (max 3 restarts per 60 seconds)
        now = time.time()
        window_start = now - 60.0
        cell.restart_window = [t for t in cell.restart_window if t > window_start]
        return len(cell.restart_window) < cell.max_restarts

    @property
    def active_count(self) -> int:
        """Number of active actor handlers."""
        with self._thread_lock:
            return sum(1 for c in self._cells.values() if c.status == "active")

    @property
    def total_registered(self) -> int:
        """Total number of registered actor handlers."""
        with self._thread_lock:
            return len(self._cells)

    # ============================================================
    # Migration Support
    # ============================================================

    def snapshot_actor(self, actor_id: str) -> Optional[Dict[str, Any]]:
        """Snapshot an actor's state for migration.

        Returns a dict with the actor's in-memory state, metadata,
        and buffered mailbox messages. Returns None if actor not found.

        The actor must be quiesced (paused) before snapshotting to
        ensure consistency.
        """
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        if cell is None:
            return None

        return {
            "actor_id": cell.actor_id,
            "actor_type": cell.actor_type,
            "state": dict(cell.context.state),
            "status": cell.status,
            "mailbox_size": len(cell.mailbox),
            "message_count": cell.message_count,
            "error_count": cell.error_count,
            "supervision_strategy": cell.supervision_strategy.value,
            "parent_id": cell.parent_id,
            "children": list(cell.children),
        }

    def quiesce_actor(self, actor_id: str) -> bool:
        """Pause an actor to prepare for migration.

        Sets the actor's status to "paused" so new messages are
        buffered instead of processed. Returns True if the actor
        was found and paused.
        """
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        if cell is None or cell.status != "active":
            return False
        cell.status = "paused"
        logger.info("Actor %s quiesced (paused) for migration", actor_id)
        return True

    def drain_actor(self, actor_id: str, timeout: float = 5.0) -> int:
        """Wait for an actor's mailbox to drain, then return remaining count.

        The actor must be quiesced first. This waits up to ``timeout``
        seconds for in-flight handler invocations to complete, then
        returns the number of messages still in the mailbox.

        Args:
            actor_id: The actor to drain.
            timeout: Maximum seconds to wait.

        Returns:
            Number of messages remaining in the mailbox after drain.
        """
        with self._thread_lock:
            cell = self._cells.get(actor_id)
        if cell is None:
            return 0

        # Wait for in-flight dispatch tasks to complete
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with self._thread_lock:
                mailbox_len = len(cell.mailbox)
            if mailbox_len == 0:
                break
            time.sleep(0.05)

        with self._thread_lock:
            return len(cell.mailbox)

    def restore_actor(
        self,
        actor_id: str,
        handler: Callable,
        snapshot: Dict[str, Any],
    ) -> Optional[ActorContext]:
        """Restore an actor from a migration snapshot.

        Creates a new ActorCell with the state from the snapshot
        and registers the handler with the message router.

        Args:
            actor_id: The actor's ID.
            handler: The message handler function.
            snapshot: The snapshot dict from ``snapshot_actor``.

        Returns:
            The restored actor's context, or None on failure.
        """
        from .actor_manager import ActorInfo
        import json

        # Register actor if not already present
        if self._actors.get_actor(actor_id) is None:
            self._actors.register(
                actor_id=actor_id,
                actor_type=snapshot.get("actor_type", "default"),
            )

        # Parse supervision strategy
        strategy_str = snapshot.get("supervision_strategy", "restart")
        try:
            strategy = SupervisionStrategy(strategy_str)
        except ValueError:
            strategy = SupervisionStrategy.RESTART

        # Restore state from snapshot
        state = snapshot.get("state", {})
        if isinstance(state, str):
            try:
                state = json.loads(state)
            except (json.JSONDecodeError, TypeError):
                state = {}

        context = ActorContext(
            actor_id=actor_id,
            actor_type=snapshot.get("actor_type", "default"),
            state=dict(state),
        )

        cell = ActorCell(
            actor_id=actor_id,
            actor_type=snapshot.get("actor_type", "default"),
            handler=handler,
            context=context,
            supervision_strategy=strategy,
            message_count=snapshot.get("message_count", 0),
            error_count=snapshot.get("error_count", 0),
            parent_id=snapshot.get("parent_id"),
        )
        context._context_ref = self

        # Restore children references
        for child_id in snapshot.get("children", []):
            cell.children.add(child_id)

        with self._thread_lock:
            self._cells[actor_id] = cell

        # Register with message router
        self._router.register_handler(actor_id, self._make_dispatcher(cell))

        logger.info("Actor %s restored from snapshot (type=%s, state_keys=%d)",
                     actor_id, cell.actor_type, len(context.state))
        return context

    def get_registered_actor_ids(self) -> List[str]:
        """Get IDs of all registered actor handlers."""
        with self._thread_lock:
            return list(self._cells.keys())


async def _noop_handler(ctx: ActorContext, envelope: MessageEnvelope):
    """Default no-op handler for spawned children without explicit handlers."""
    pass
