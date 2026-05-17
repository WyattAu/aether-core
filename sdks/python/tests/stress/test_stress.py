"""
Stress Tests for Aether Python SDK

Validates performance, memory usage, and stability under high load.

Run:  pytest sdks/python/tests/stress/test_stress.py -v -m stress
Skip:  SKIP_STRESS=1 pytest sdks/python/tests/stress/test_stress.py -v
"""

from __future__ import annotations

import asyncio
import gc
import os
import time
import tracemalloc

import pytest

from aether_sdk.resilience.circuit_breaker import (CircuitBreaker,
                                                   CircuitBreakerConfig,
                                                   CircuitBreakerError,
                                                   CircuitState)
from aether_sdk.resilience.retry import (BackoffStrategy, RetryConfig,
                                         RetryPolicy)
from aether_sdk.streaming.backpressure import BackpressureController
from aether_sdk.streaming.types import (BackpressureConfig,
                                        BackpressureStrategy, Duration,
                                        StreamEvent, Timestamp, WindowSpec,
                                        WindowType)
from aether_sdk.streaming.window import WindowAssigner

SKIP_STRESS = os.environ.get("SKIP_STRESS", "0") == "1"
pytestmark = pytest.mark.stress


def _skip():
    if SKIP_STRESS:
        pytest.skip("SKIP_STRESS=1")


def _linear_slope(xs: list[float], ys: list[float]) -> float:
    n = len(xs)
    if n < 2:
        return 0.0
    sx = sum(xs)
    sy = sum(ys)
    sxy = sum(x * y for x, y in zip(xs, ys))
    sx2 = sum(x * x for x in xs)
    d = n * sx2 - sx * sx
    return (n * sxy - sx * sy) / d if d != 0 else 0.0


# ------------------------------------------------------------------ #
#  1. 1M Stream Events Through Windowing                              #
# ------------------------------------------------------------------ #


def test_1m_stream_events_windowing():
    _skip()

    tracemalloc.start()
    mem_before, _ = tracemalloc.get_traced_memory()

    spec = WindowSpec(type=WindowType.TUMBLING, size=Duration.from_seconds(1))
    assigner = WindowAssigner(spec)

    start = time.perf_counter()
    for i in range(1_000_000):
        event = StreamEvent.create(
            key=f"k{i % 100}",
            value=i,
            timestamp=Timestamp(i % 10_000),
        )
        assigner.assign(event, f"k{i % 100}")
    elapsed = time.perf_counter() - start

    mem_after, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    growth_mb = (mem_after - mem_before) / (1024 * 1024)
    eps = 1_000_000 / elapsed

    print("\n=== 1M Stream Events Windowing ===")
    print(f"  Time:       {elapsed:.2f}s")
    print(f"  Events/sec: {eps:,.0f}")
    print(f"  Mem before: {mem_before / 1024 ** 2:.1f} MB")
    print(f"  Mem after:  {mem_after / 1024 ** 2:.1f} MB")
    print(f"  Mem growth: {growth_mb:.1f} MB")
    print(f"  Peak mem:   {peak / 1024 ** 2:.1f} MB")
    print(f"  Windows:    {len(assigner._windows)}")

    assert elapsed < 60, f"Took {elapsed:.1f}s (limit 60s)"


# ------------------------------------------------------------------ #
#  2. 100K Concurrent Circuit Breaker Operations                     #
# ------------------------------------------------------------------ #


async def _cb_ok():
    return "ok"


async def _cb_fail():
    raise RuntimeError("ECONNRESET")


@pytest.mark.asyncio
async def test_100k_concurrent_circuit_breaker():
    _skip()

    cb = CircuitBreaker(CircuitBreakerConfig(failure_threshold=5))

    success_count = 0
    fail_count = 0
    reject_count = 0

    async def call(idx: int):
        nonlocal success_count, fail_count, reject_count
        fn = _cb_ok if idx % 2 == 0 else _cb_fail
        try:
            await cb.execute(fn)
            success_count += 1
        except CircuitBreakerError:
            reject_count += 1
        except Exception:
            fail_count += 1

    start = time.perf_counter()
    await asyncio.gather(*[call(i) for i in range(100_000)])
    elapsed = time.perf_counter() - start

    print("\n=== 100K Concurrent Circuit Breaker ===")
    print(f"  Time:      {elapsed:.2f}s")
    print(f"  Successes: {success_count:,}")
    print(f"  Failures:  {fail_count:,}")
    print(f"  Rejected:  {reject_count:,}")
    print(f"  State:     {cb.state.value}")
    print(f"  Ops/sec:   {100_000 / elapsed:,.0f}")

    assert success_count + fail_count + reject_count == 100_000
    assert cb.state in (
        CircuitState.OPEN,
        CircuitState.HALF_OPEN,
        CircuitState.CLOSED,
    )


# ------------------------------------------------------------------ #
#  3. 1M Backpressure Push/Pop Cycles                                #
# ------------------------------------------------------------------ #


def test_1m_backpressure_push_pop():
    _skip()

    ctrl = BackpressureController(
        BackpressureConfig(
            strategy=BackpressureStrategy.BUFFER,
            buffer_size=2_000_000,
        )
    )

    start = time.perf_counter()
    pushed = 0
    for i in range(1_000_000):
        event = StreamEvent.create(key="k", value=i, timestamp=Timestamp(i))
        if ctrl.try_push(event):
            pushed += 1

    popped = 0
    while True:
        e = ctrl.pop()
        if e is None:
            break
        popped += 1
    elapsed = time.perf_counter() - start

    print("\n=== 1M Backpressure Push/Pop ===")
    print(f"  Time:   {elapsed:.2f}s")
    print(f"  Pushed: {pushed:,}")
    print(f"  Popped: {popped:,}")
    print(f"  Ops/s:  {2_000_000 / elapsed:,.0f}")

    assert pushed == 1_000_000
    assert popped == 1_000_000
    assert elapsed < 30, f"Took {elapsed:.1f}s (limit 30s)"


# ------------------------------------------------------------------ #
#  4. Memory Stability                                               #
# ------------------------------------------------------------------ #


def test_memory_stability():
    _skip()

    tracemalloc.start()
    gc.collect()

    samples_x: list[float] = []
    samples_y: list[float] = []
    iterations = 10_000

    for i in range(iterations):
        w = WindowAssigner(
            WindowSpec(type=WindowType.TUMBLING, size=Duration.from_seconds(1))
        )
        for j in range(10):
            w.assign(
                StreamEvent.create(key="k", value=j, timestamp=Timestamp(j)),
                "k",
            )
        del w

        bp = BackpressureController(
            BackpressureConfig(
                strategy=BackpressureStrategy.BUFFER,
                buffer_size=1000,
            )
        )
        for j in range(100):
            bp.try_push(StreamEvent.create(key="k", value=j, timestamp=Timestamp(j)))
        for _ in range(100):
            bp.pop()
        del bp

        if i % 100 == 0:
            gc.collect()
            current, _ = tracemalloc.get_traced_memory()
            samples_x.append(float(i))
            samples_y.append(float(current))

    tracemalloc.stop()

    slope = _linear_slope(samples_x, samples_y)
    slope_per_iter = slope * 100

    print("\n=== Memory Stability ===")
    print(f"  Iterations:  {iterations:,}")
    print(f"  Samples:     {len(samples_x)}")
    print(f"  Slope:       {slope_per_iter:.1f} bytes/iteration")
    print(f"  First mem:   {samples_y[0] / 1024:.1f} KB")
    print(f"  Last mem:    {samples_y[-1] / 1024:.1f} KB")
    print(f"  Mem range:   {(max(samples_y) - min(samples_y)) / 1024:.1f} KB")

    assert (
        slope_per_iter < 1024
    ), f"Memory growth {slope_per_iter:.1f}B/iter (limit 1KB/iter)"


# ------------------------------------------------------------------ #
#  5. Retry Storm                                                    #
# ------------------------------------------------------------------ #


@pytest.mark.asyncio
async def test_retry_storm():
    _skip()

    call_counts: dict[int, int] = {}

    async def flaky_fn(call_id: int) -> str:
        call_counts[call_id] = call_counts.get(call_id, 0) + 1
        if call_counts[call_id] < 4:
            raise RuntimeError("ECONNRESET")
        return f"ok-{call_id}"

    policy = RetryPolicy(
        RetryConfig(
            max_attempts=4,
            base_delay_ms=1,
            max_delay_ms=50,
            backoff=BackoffStrategy.EXPONENTIAL,
            multiplier=2.0,
            is_retryable=lambda err, attempt: "econnreset" in str(err).lower(),
        )
    )

    start = time.perf_counter()

    tasks = []
    for i in range(10_000):
        call_counts[i] = 0
        tasks.append(policy.execute(lambda i=i: flaky_fn(i)))

    results = await asyncio.gather(*tasks, return_exceptions=True)
    elapsed = time.perf_counter() - start

    successes = sum(1 for r in results if not isinstance(r, BaseException))
    failures = sum(1 for r in results if isinstance(r, BaseException))

    print("\n=== Retry Storm (10K operations) ===")
    print(f"  Time:      {elapsed:.2f}s")
    print(f"  Successes: {successes:,}")
    print(f"  Failures:  {failures:,}")
    print(f"  Ops/sec:   {10_000 / elapsed:,.0f}")

    stats = policy.get_stats()
    print(f"  Stats: attempts={stats.total_attempts}, retried={stats.retried_calls}")

    assert successes == 10_000, f"Expected 10K successes, got {successes}"
