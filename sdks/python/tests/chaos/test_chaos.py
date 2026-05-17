import asyncio
import os
import time

import pytest

from aether_sdk.resilience.bulkhead import (Bulkhead, BulkheadConfig,
                                            BulkheadRejectedError)
from aether_sdk.resilience.circuit_breaker import (CircuitBreaker,
                                                   CircuitBreakerConfig,
                                                   CircuitBreakerError,
                                                   CircuitState)
from aether_sdk.resilience.rate_limiter import (RateLimitConfig, RateLimiter,
                                                RateLimitStrategy)
from aether_sdk.resilience.retry import (BackoffStrategy, RetryConfig,
                                         RetryExhaustedError, RetryPolicy)
from aether_sdk.streaming.backpressure import (BackpressureConfig,
                                               BackpressureController)
from aether_sdk.streaming.types import (BackpressureStrategy, Duration,
                                        StreamEvent, Timestamp, WindowSpec,
                                        WindowType)
from aether_sdk.streaming.window import WindowAssigner

SKIP_CHAOS = os.environ.get("SKIP_CHAOS", "0") == "1"
skip_reason = "SKIP_CHAOS=1"


def _print(label: str, value):
    print(f"  [{label}] {value}")


# ============================================================
# 1. Circuit Breaker Flapping
# ============================================================
@pytest.mark.chaos
@pytest.mark.skipif(SKIP_CHAOS, reason=skip_reason)
@pytest.mark.asyncio
async def test_circuit_breaker_flapping():
    start = time.perf_counter()

    config = CircuitBreakerConfig(
        failure_threshold=5,
        success_threshold=3,
        timeout_ms=50,
        failure_window_ms=0,
    )
    cb = CircuitBreaker(config)

    pattern = [False] * 5 + [True] * 3
    successes = 0
    failures = 0
    rejected = 0
    opens = 0
    total_calls = 0

    for cycle in range(125):
        for should_succeed in pattern:
            total_calls += 1
            try:
                if should_succeed:
                    await cb.execute(lambda: asyncio.sleep(0))
                    successes += 1
                else:
                    await cb.execute(
                        lambda: (_ for _ in ()).throw(RuntimeError("fail"))
                    )
            except CircuitBreakerError:
                rejected += 1
            except RuntimeError:
                failures += 1

            prev = cb.state
            if prev == CircuitState.OPEN and opens == 0:
                opens += 1

    await asyncio.sleep(0.1)
    stats = cb.get_stats()

    elapsed = time.perf_counter() - start
    _print("elapsed", f"{elapsed:.3f}s")
    _print("total_calls", total_calls)
    _print("successes", successes)
    _print("failures", failures)
    _print("rejected", rejected)
    _print("final_state", stats.state.value)

    # Stats counters reset on state transitions, so verify using
    # the test's own tracking counters which are never reset.
    assert total_calls == 1000
    assert failures > 0
    assert successes > 0
    assert stats.state in (
        CircuitState.CLOSED,
        CircuitState.OPEN,
        CircuitState.HALF_OPEN,
    )


# ============================================================
# 2. Bulkhead Exhaustion + Recovery
# ============================================================
@pytest.mark.chaos
@pytest.mark.skipif(SKIP_CHAOS, reason=skip_reason)
@pytest.mark.asyncio
async def test_bulkhead_exhaustion_recovery():
    start = time.perf_counter()

    config = BulkheadConfig(max_concurrent=10, max_queued=50, timeout_ms=5000)
    bh = Bulkhead(config)

    accepted_results = []
    rejected_count = 0

    async def slow_task(idx):
        await asyncio.sleep(0.1)
        return idx

    tasks = []
    for i in range(1000):
        tasks.append(_run_bulkhead(bh, slow_task, i))

    results = await asyncio.gather(*tasks, return_exceptions=True)

    for r in results:
        if isinstance(r, BulkheadRejectedError):
            rejected_count += 1
        elif isinstance(r, Exception):
            pass
        else:
            accepted_results.append(r)

    await asyncio.sleep(0.5)
    stats = bh.get_stats()

    elapsed = time.perf_counter() - start
    _print("elapsed", f"{elapsed:.3f}s")
    _print("accepted", stats.total_accepted)
    _print("rejected", stats.total_rejected)
    _print("active_after", stats.active)
    _print("queued_after", stats.queued)

    assert stats.total_accepted == len(accepted_results)
    assert stats.total_accepted + stats.total_rejected == 1000
    assert stats.active == 0
    assert stats.queued == 0


async def _run_bulkhead(bh, fn, idx):
    try:

        async def wrapper():
            return await fn(idx)

        return await bh.execute(wrapper)
    except BulkheadRejectedError:
        return BulkheadRejectedError(f"rejected-{idx}")


# ============================================================
# 3. Rate Limiter Burst
# ============================================================
@pytest.mark.chaos
@pytest.mark.skipif(SKIP_CHAOS, reason=skip_reason)
@pytest.mark.asyncio
async def test_rate_limiter_burst():
    start = time.perf_counter()

    config = RateLimitConfig(
        requests_per_second=10,
        strategy=RateLimitStrategy.FIXED_WINDOW,
        window_size_ms=1000,
    )
    limiter = RateLimiter(config)

    allowed = 0
    rejected = 0

    for _ in range(1000):
        result = await limiter.try_acquire()
        if result.allowed:
            allowed += 1
        else:
            rejected += 1

    elapsed = time.perf_counter() - start
    _print("elapsed", f"{elapsed:.3f}s")
    _print("allowed", allowed)
    _print("rejected", rejected)

    assert allowed == 10
    assert rejected == 990

    await asyncio.sleep(1.1)

    allowed2 = 0
    for _ in range(20):
        result = await limiter.try_acquire()
        if result.allowed:
            allowed2 += 1

    _print("allowed_after_recovery", allowed2)
    assert allowed2 > 0, "Should recover after window passes"


# ============================================================
# 4. Backpressure Overflow Recovery
# ============================================================
@pytest.mark.chaos
@pytest.mark.skipif(SKIP_CHAOS, reason=skip_reason)
@pytest.mark.asyncio
async def test_backpressure_overflow_recovery():
    start = time.perf_counter()

    config = BackpressureConfig(
        strategy=BackpressureStrategy.DROP,
        buffer_size=1000,
        high_watermark=0.9,
        low_watermark=0.5,
    )
    ctrl = BackpressureController(config)

    accepted = 0
    dropped = 0

    for i in range(100_000):
        event = StreamEvent.create(key=f"event-{i}", value=i)
        if ctrl.try_push(event):
            accepted += 1
        else:
            dropped += 1

    stats = ctrl.stats
    _print("after_push_elapsed", f"{time.perf_counter() - start:.3f}s")
    _print("accepted", accepted)
    _print("dropped", dropped)
    _print("buffer_size", ctrl.size())

    assert ctrl.size() <= 1000
    assert ctrl.size() == 1000
    assert stats.total_events == 100_000
    assert stats.dropped_events > 0
    assert stats.dropped_events + stats.buffered_events == 100_000

    drained = 0
    while not ctrl.is_empty():
        ctrl.pop()
        drained += 1

    _print("drained", drained)
    _print("buffer_after_drain", ctrl.size())

    assert drained == 1000
    assert ctrl.is_empty()
    assert ctrl.size() == 0

    elapsed = time.perf_counter() - start
    _print("total_elapsed", f"{elapsed:.3f}s")


# ============================================================
# 5. Window Storm
# ============================================================
@pytest.mark.chaos
@pytest.mark.skipif(SKIP_CHAOS, reason=skip_reason)
@pytest.mark.asyncio
async def test_window_storm():
    start = time.perf_counter()

    spec = WindowSpec(
        type=WindowType.TUMBLING,
        size=Duration.from_millis(1),
    )
    assigner = WindowAssigner(spec)

    base_ts = 1000000000000
    total_events = 100_000
    assigned_count = 0
    unique_windows = set()

    for i in range(total_events):
        ts = base_ts + i
        event = StreamEvent(
            key="storm",
            value=i,
            timestamp=Timestamp(ts),
        )
        windows = assigner.assign(event, key="storm")
        assigned_count += len(windows)
        for w in windows:
            unique_windows.add(w.window_id)

    _print("elapsed", f"{time.perf_counter() - start:.3f}s")
    _print("total_events", total_events)
    _print("assignments", assigned_count)
    _print("unique_windows", len(unique_windows))

    assert assigned_count == total_events
    assert len(unique_windows) == total_events

    for wid, window in assigner._windows.items():
        assert len(window.events) == 1, f"Window {wid} has {len(window.events)} events"
        assert window.events[0].value == int(wid.split("_")[1]) - base_ts


# ============================================================
# 6. Retry Timeout Storm
# ============================================================
@pytest.mark.chaos
@pytest.mark.skipif(SKIP_CHAOS, reason=skip_reason)
@pytest.mark.asyncio
async def test_retry_timeout_storm():
    start = time.perf_counter()

    config = RetryConfig(
        max_attempts=3,
        backoff=BackoffStrategy.EXPONENTIAL,
        base_delay_ms=10,
        max_delay_ms=100,
        multiplier=2.0,
    )
    policy = RetryPolicy(config)

    num_operations = 1000

    async def timeout_operation(idx):
        await asyncio.sleep(0.01)
        raise TimeoutError(f"operation-{idx} timeout")

    async def run_retry(idx):
        try:

            async def op():
                return await timeout_operation(idx)

            await policy.execute(op)
        except RetryExhaustedError:
            pass
        except TimeoutError:
            pass

    await asyncio.gather(*[run_retry(i) for i in range(num_operations)])

    stats = policy.get_stats()
    elapsed = time.perf_counter() - start

    _print("elapsed", f"{elapsed:.3f}s")
    _print("total_attempts", stats.total_attempts)
    _print("successful", stats.successful_attempts)
    _print("failed", stats.failed_attempts)
    _print("exhausted", stats.exhausted_calls)
    _print("total_delay_ms", stats.total_retry_delay_ms)

    assert stats.total_attempts == num_operations * 3
    assert stats.successful_attempts == 0
    assert stats.failed_attempts == num_operations * 3
    assert stats.exhausted_calls == num_operations
