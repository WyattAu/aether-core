"""
Performance benchmarks for the Aether Python SDK.

Run with:
    pytest tests/performance/test_benchmarks.py -m performance -v -s
Skip in CI:
    SKIP_PERF=1 pytest tests/performance/test_benchmarks.py
"""

from __future__ import annotations

import asyncio
import os
import time

import pytest

pytestmark = pytest.mark.skipif(
    os.getenv("SKIP_PERF"), reason="Performance tests skipped"
)


N_STREAM = 100_000
N_RESILIENCE = 10_000
N_STATE = 10_000
N_VALIDATION = 10_000


def _fmt_ops(n: int, elapsed: float) -> str:
    ops = n / elapsed
    return f"{ops:,.0f} ops/sec  ({elapsed:.3f}s for {n:,} ops)"


def _fmt_latency(total_ns: float, n: int) -> str:
    avg_us = (total_ns / n) / 1000
    return f"{avg_us:.2f} us/call  (total {total_ns/1e6:.3f}ms for {n:,} calls)"


@pytest.mark.performance
class TestStreamProcessing:
    def test_window_assigner_throughput(self):
        from aether_sdk.streaming.types import (
            Duration,
            StreamEvent,
            Timestamp,
            WindowSpec,
            WindowType,
        )
        from aether_sdk.streaming.window import WindowAssigner

        spec = WindowSpec(type=WindowType.TUMBLING, size=Duration.from_minutes(5))
        assigner = WindowAssigner(spec)

        events = [
            StreamEvent(
                key=f"key-{i % 100}",
                value={"id": i},
                timestamp=Timestamp(i * 1000),
            )
            for i in range(N_STREAM)
        ]

        t0 = time.perf_counter()
        for ev in events:
            assigner.assign(ev, ev.key)
        elapsed = time.perf_counter() - t0

        result = _fmt_ops(N_STREAM, elapsed)
        print(f"\n  [WindowAssigner] {result}")
        assert elapsed < 10, f"Window assign took {elapsed:.2f}s, too slow"

    def test_tumbling_window_process_throughput(self):
        from aether_sdk.streaming.types import Duration, StreamEvent, Timestamp
        from aether_sdk.streaming.window import TumblingWindow

        fired_count = 0

        def handler(events, info):
            nonlocal fired_count
            fired_count += 1
            return len(events)

        tw = TumblingWindow(Duration.from_minutes(5), handler)

        events = [
            StreamEvent(
                key="k1",
                value={"id": i},
                timestamp=Timestamp(i * 1000),
            )
            for i in range(N_STREAM)
        ]

        t0 = time.perf_counter()
        for ev in events:
            tw.process(ev, "k1")
        elapsed = time.perf_counter() - t0

        result = _fmt_ops(N_STREAM, elapsed)
        print(f"\n  [TumblingWindow.process] {result}")
        assert elapsed < 10


@pytest.mark.performance
class TestBackpressure:
    def test_backpressure_push_throughput(self):
        from aether_sdk.streaming.backpressure import BackpressureController
        from aether_sdk.streaming.types import (
            BackpressureConfig,
            BackpressureStrategy,
            StreamEvent,
            Timestamp,
        )

        config = BackpressureConfig(
            strategy=BackpressureStrategy.BUFFER,
            buffer_size=200_000,
        )
        ctrl = BackpressureController(config)

        events = [
            StreamEvent(key="k", value=i, timestamp=Timestamp(i))
            for i in range(N_STREAM)
        ]

        t0 = time.perf_counter()
        for ev in events:
            ctrl.try_push(ev)
        elapsed = time.perf_counter() - t0

        result = _fmt_ops(N_STREAM, elapsed)
        print(f"\n  [BackpressureController.try_push] {result}")
        assert elapsed < 10

    def test_multilevel_backpressure_push_throughput(self):
        from aether_sdk.streaming.backpressure import MultiLevelBackpressure
        from aether_sdk.streaming.types import StreamEvent, Timestamp

        bp = MultiLevelBackpressure(buffer_size=200_000)

        events = [
            StreamEvent(key="k", value=i, timestamp=Timestamp(i))
            for i in range(N_STREAM)
        ]

        t0 = time.perf_counter()
        for ev in events:
            bp.push(ev, MultiLevelBackpressure.Priority.NORMAL)
        elapsed = time.perf_counter() - t0

        result = _fmt_ops(N_STREAM, elapsed)
        print(f"\n  [MultiLevelBackpressure.push] {result}")
        assert elapsed < 10


@pytest.mark.performance
class TestCircuitBreaker:
    def test_circuit_breaker_overhead(self):
        from aether_sdk.resilience.circuit_breaker import (
            CircuitBreaker,
            CircuitBreakerConfig,
        )

        cb = CircuitBreaker(CircuitBreakerConfig(failure_threshold=1000))

        async def ok():
            return 42

        async def run():
            total = 0.0
            for _ in range(N_RESILIENCE):
                t0 = time.perf_counter()
                await cb.execute(ok)
                total += time.perf_counter() - t0
            return total

        loop = asyncio.new_event_loop()
        total = loop.run_until_complete(run())
        loop.close()

        overhead_us = (total / N_RESILIENCE) * 1_000_000
        print(f"\n  [CircuitBreaker.execute] {overhead_us:.2f} us/call")
        assert overhead_us < 5000

    def test_circuit_breaker_vs_direct(self):
        from aether_sdk.resilience.circuit_breaker import (
            CircuitBreaker,
            CircuitBreakerConfig,
        )

        cb = CircuitBreaker(CircuitBreakerConfig(failure_threshold=1000))

        async def ok():
            return 42

        n = 10_000

        async def run_benchmark():
            t0 = time.perf_counter()
            for _ in range(n):
                await cb.execute(ok)
            with_cb = time.perf_counter() - t0

            t0 = time.perf_counter()
            for _ in range(n):
                await ok()
            without_cb = time.perf_counter() - t0

            return with_cb, without_cb

        loop = asyncio.new_event_loop()
        with_cb, without_cb = loop.run_until_complete(run_benchmark())
        loop.close()

        overhead_us = ((with_cb - without_cb) / n) * 1_000_000
        print(f"\n  [CircuitBreaker overhead vs direct] {overhead_us:.2f} us/call")
        print(f"    with_cb={with_cb*1000:.2f}ms  without_cb={without_cb*1000:.2f}ms")


@pytest.mark.performance
class TestRetry:
    def test_retry_overhead(self):
        from aether_sdk.resilience.retry import RetryConfig, RetryPolicy

        policy = RetryPolicy(RetryConfig(max_attempts=1))

        async def ok():
            return 42

        async def run():
            t0 = time.perf_counter()
            for _ in range(N_RESILIENCE):
                await policy.execute(ok)
            elapsed = time.perf_counter() - t0
            return elapsed

        loop = asyncio.new_event_loop()
        elapsed = loop.run_until_complete(run())
        loop.close()

        overhead_us = (elapsed / N_RESILIENCE) * 1_000_000
        print(f"\n  [RetryPolicy.execute (max_attempts=1)] {overhead_us:.2f} us/call")
        assert overhead_us < 5000


@pytest.mark.performance
class TestStateHandle:
    def test_state_handle_set_get_delete(self):
        from aether_sdk.state import StateHandle

        state = StateHandle()

        async def run():
            t0 = time.perf_counter()
            for i in range(N_STATE):
                await state.set(f"key-{i}", f"value-{i}".encode())
                await state.get(f"key-{i}")
                await state.delete(f"key-{i}")
            elapsed = time.perf_counter() - t0
            return elapsed

        loop = asyncio.new_event_loop()
        elapsed = loop.run_until_complete(run())
        loop.close()

        ops = N_STATE * 3
        result = _fmt_ops(ops, elapsed)
        print(f"\n  [StateHandle set/get/delete] {result}")
        assert elapsed < 10


@pytest.mark.performance
class TestValidation:
    def test_validate_email_throughput(self):
        from aether_sdk.validation import validate_email

        emails = [f"user{i}@example.com" for i in range(N_VALIDATION)]

        t0 = time.perf_counter()
        for email in emails:
            validate_email(email)
        elapsed = time.perf_counter() - t0

        result = _fmt_ops(N_VALIDATION, elapsed)
        print(f"\n  [validate_email] {result}")
        assert elapsed < 5

    def test_validate_uuid_throughput(self):
        from aether_sdk.validation import validate_uuid

        uuids = [f"{i:08x}-1234-5678-1234-{i:012x}" for i in range(N_VALIDATION)]

        t0 = time.perf_counter()
        for u in uuids:
            validate_uuid(u)
        elapsed = time.perf_counter() - t0

        result = _fmt_ops(N_VALIDATION, elapsed)
        print(f"\n  [validate_uuid] {result}")
        assert elapsed < 5
