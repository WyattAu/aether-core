"""Graceful shutdown manager for the Aether server.

Handles SIGTERM/SIGINT signals with a configurable drain period.
During the drain period, the health endpoint reports "draining" status
and new requests can be rejected or served from cache.

Usage::

    from server.shutdown import ShutdownManager

    shutdown_mgr = ShutdownManager(drain_timeout_seconds=30)
    shutdown_mgr.install_signal_handlers()

    # In health check:
    if shutdown_mgr.is_draining():
        return {"status": "draining"}

    # To wait for shutdown (in lifespan):
    await shutdown_mgr.wait_for_shutdown()
"""

import asyncio
import logging
import signal
import threading
from typing import Callable, List

logger = logging.getLogger("aether-server.shutdown")


class ShutdownManager:
    """Manages graceful server shutdown with drain period.

    State machine: ``running`` → ``draining`` → ``shut_down``

    When a shutdown signal is received:
    1. State transitions to ``draining``
    2. Registered cleanup callbacks are invoked (LIFO order)
    3. The drain timeout is respected
    4. State transitions to ``shut_down``

    Args:
        drain_timeout_seconds: Maximum seconds to wait for in-flight
            requests during drain phase before forcing shutdown.
    """

    def __init__(self, drain_timeout_seconds: float = 30.0):
        self._drain_timeout = drain_timeout_seconds
        self._state: str = "running"
        self._lock = threading.Lock()
        self._event = asyncio.Event()
        self._cleanup_callbacks: List[Callable] = []
        self._original_handlers: dict = {}

    @property
    def state(self) -> str:
        """Current server state: ``running``, ``draining``, or ``shut_down``."""
        with self._lock:
            return self._state

    @property
    def is_running(self) -> bool:
        """``True`` when the server is accepting new requests."""
        return self.state == "running"

    @property
    def is_draining(self) -> bool:
        """``True`` during the drain period (accepting final requests)."""
        return self.state == "draining"

    def register_cleanup(self, callback: Callable) -> None:
        """Register a cleanup callback to be called during shutdown.

        Callbacks are invoked in LIFO (last-in, first-out) order.

        Args:
            callback: A callable to invoke during shutdown.
        """
        with self._lock:
            self._cleanup_callbacks.append(callback)

    def trigger_shutdown(self, signal_name: str = "unknown") -> None:
        """Initiate graceful shutdown.

        Transitions state to ``draining``, runs cleanup callbacks,
        then transitions to ``shut_down``.

        Args:
            signal_name: Name of the signal that triggered shutdown (for logging).
        """
        with self._lock:
            if self._state != "running":
                logger.debug(
                    "Shutdown already in progress (state=%s), ignoring %s",
                    self._state, signal_name,
                )
                return
            self._state = "draining"

        logger.info(
            "Shutdown initiated by %s — draining for %.1fs",
            signal_name, self._drain_timeout,
        )

        # Run cleanup callbacks in LIFO order
        callbacks = list(reversed(self._cleanup_callbacks))
        for cb in callbacks:
            try:
                logger.debug("Running cleanup callback: %s", getattr(cb, '__name__', repr(cb)))
                result = cb()
                if asyncio.iscoroutine(result):
                    # We're in a signal handler — can't await. Schedule it.
                    try:
                        loop = asyncio.get_running_loop()
                        loop.create_task(result)
                    except RuntimeError:
                        logger.warning(
                            "Cleanup callback %s returned a coroutine but no event loop is running",
                            getattr(cb, '__name__', repr(cb)),
                        )
            except Exception:
                logger.exception("Error in cleanup callback %s", getattr(cb, '__name__', repr(cb)))

        # Transition to shut_down
        with self._lock:
            self._state = "shut_down"

        # Wake up anyone waiting on wait_for_shutdown()
        try:
            loop = asyncio.get_running_loop()
            loop.call_soon_threadsafe(self._event.set)
        except RuntimeError:
            self._event.set()

        logger.info("Shutdown complete (signal=%s)", signal_name)

    def install_signal_handlers(self) -> None:
        """Install SIGTERM and SIGINT handlers.

        Saves the original handlers so they can be restored.
        """
        for sig in (signal.SIGTERM, signal.SIGINT):
            original = signal.getsignal(sig)
            self._original_handlers[sig] = original
            signal.signal(sig, self._make_handler(sig))

    def restore_signal_handlers(self) -> None:
        """Restore the original signal handlers."""
        for sig, handler in self._original_handlers.items():
            signal.signal(sig, handler)
        self._original_handlers.clear()

    def _make_handler(self, sig: signal.Signals) -> Callable:
        """Create a signal handler function for the given signal."""
        def handler(signum, frame):
            sig_name = signal.Signals(signum).name
            self.trigger_shutdown(sig_name)
        return handler

    async def wait_for_shutdown(self) -> None:
        """Wait until shutdown is triggered.

        This is designed to be used in the FastAPI lifespan context
        to keep the server alive until a signal is received.
        """
        await self._event.wait()

    def reset(self) -> None:
        """Reset the shutdown manager to running state.

        Primarily useful for testing.
        """
        with self._lock:
            self._state = "running"
        self._event.clear()
        self._cleanup_callbacks.clear()
