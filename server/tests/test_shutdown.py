"""Tests for the graceful shutdown manager."""

import asyncio
import signal
from unittest.mock import patch

import pytest

from server.shutdown import ShutdownManager


class TestShutdownManagerState:

    def test_initial_state_is_running(self):
        mgr = ShutdownManager()
        assert mgr.is_running
        assert not mgr.is_draining
        assert mgr.state == "running"

    def test_trigger_shutdown_transitions_to_draining(self):
        mgr = ShutdownManager()
        mgr.trigger_shutdown("SIGTERM")
        assert not mgr.is_running
        # State transitions draining -> shut_down synchronously
        assert mgr.state == "shut_down"

    def test_trigger_shutdown_idempotent(self):
        mgr = ShutdownManager()
        mgr.trigger_shutdown("SIGTERM")
        mgr.trigger_shutdown("SIGINT")  # Should be ignored
        assert mgr.state == "shut_down"

    def test_reset(self):
        mgr = ShutdownManager()
        mgr.trigger_shutdown("SIGTERM")
        assert mgr.state == "shut_down"
        mgr.reset()
        assert mgr.is_running
        assert not mgr.is_draining


class TestShutdownManagerCleanup:

    def test_cleanup_callbacks_called_on_shutdown(self):
        mgr = ShutdownManager()
        results = []
        mgr.register_cleanup(lambda: results.append("first"))
        mgr.register_cleanup(lambda: results.append("second"))
        mgr.trigger_shutdown("SIGTERM")
        # LIFO order
        assert results == ["second", "first"]

    def test_cleanup_callback_exception_is_caught(self):
        mgr = ShutdownManager()
        results = []
        mgr.register_cleanup(lambda: results.append("ok"))
        mgr.register_cleanup(lambda: (_ for _ in ()).throw(RuntimeError("boom")))
        mgr.register_cleanup(lambda: results.append("after"))
        # Should not raise
        mgr.trigger_shutdown("SIGTERM")
        assert "ok" in results
        assert "after" in results

    def test_reset_clears_callbacks(self):
        mgr = ShutdownManager()
        mgr.register_cleanup(lambda: None)
        mgr.reset()
        assert mgr._cleanup_callbacks == []


class TestShutdownManagerAsync:

    def test_wait_for_shutdown_blocks(self):
        mgr = ShutdownManager()
        ready = False

        async def trigger():
            nonlocal ready
            await asyncio.sleep(0.05)
            ready = True
            mgr.trigger_shutdown("SIGTERM")

        async def main():
            task = asyncio.create_task(trigger())
            await mgr.wait_for_shutdown()
            await task
            return ready

        result = asyncio.run(main())
        assert result

    def test_wait_resolves_immediately_if_shut_down(self):
        mgr = ShutdownManager()
        mgr.trigger_shutdown("SIGTERM")

        async def main():
            await mgr.wait_for_shutdown()
            return True

        assert asyncio.run(main())


class TestShutdownManagerSignals:

    def test_install_signal_handlers(self):
        mgr = ShutdownManager()
        mgr.install_signal_handlers()
        assert signal.SIGTERM in mgr._original_handlers
        assert signal.SIGINT in mgr._original_handlers
        mgr.restore_signal_handlers()

    def test_restore_signal_handlers(self):
        mgr = ShutdownManager()
        original = signal.getsignal(signal.SIGTERM)
        mgr.install_signal_handlers()
        mgr.restore_signal_handlers()
        assert signal.getsignal(signal.SIGTERM) == original

    def test_sigterm_triggers_shutdown(self):
        mgr = ShutdownManager()
        mgr.install_signal_handlers()

        # Simulate SIGTERM
        handler = mgr._make_handler(signal.SIGTERM)
        handler(signal.SIGTERM.value, None)

        assert mgr.state == "shut_down"
        mgr.restore_signal_handlers()

    def test_sigint_triggers_shutdown(self):
        mgr = ShutdownManager()
        mgr.install_signal_handlers()

        handler = mgr._make_handler(signal.SIGINT)
        handler(signal.SIGINT.value, None)

        assert mgr.state == "shut_down"
        mgr.restore_signal_handlers()


class TestShutdownManagerConfig:

    def test_custom_drain_timeout(self):
        mgr = ShutdownManager(drain_timeout_seconds=60.0)
        assert mgr._drain_timeout == 60.0

    def test_default_drain_timeout(self):
        mgr = ShutdownManager()
        assert mgr._drain_timeout == 30.0
