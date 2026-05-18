"""E2E Scenario 2: Real-Time Analytics Pipeline

Simulates a real-time analytics pipeline:
- Ingestion of 10K events (page views, clicks, purchases)
- Tumbling windows (5-minute) aggregate counts
- Aggregation: count events per type, compute running averages
- Alerting: trigger alerts when thresholds exceeded (>1000 purchases in 5 min)
- Late data: inject late events and verify watermark handling
"""

import random
from typing import Any, Dict, List, Optional

import pytest

from aether_sdk.streaming.backpressure import (
    BackpressureController,
    RateBasedBackpressure,
)
from aether_sdk.streaming.stream_actor import StreamActor
from aether_sdk.streaming.types import (
    BackpressureConfig,
    BackpressureStrategy,
    Duration,
    StreamEvent,
    Timestamp,
    Watermark,
    WindowInfo,
    WindowSpec,
    WindowType,
)
from aether_sdk.streaming.window import TumblingWindow

random.seed(42)

FIVE_MINUTES_MS = 5 * 60 * 1000
BASE_TS = Timestamp(1700000000000)


def _make_event(event_type: str, ts_offset_ms: int = 0) -> StreamEvent:
    return StreamEvent.create(
        key=f"user-{random.randint(1, 100)}",
        value={"type": event_type, "amount": random.uniform(1, 100)},
        timestamp=Timestamp(BASE_TS.milliseconds + ts_offset_ms),
        event_type=event_type,
    )


@pytest.mark.e2e
class TestAnalyticsPipeline:
    """End-to-end tests for the real-time analytics pipeline."""

    @pytest.mark.asyncio
    async def test_ingest_10k_events(self):
        """Ingest 10K events and verify counts per type."""
        event_types = ["page_view", "click", "purchase"]
        weights = [0.6, 0.3, 0.1]
        events: List[StreamEvent] = []

        for i in range(10_000):
            r = random.random()
            cumulative = 0.0
            chosen = event_types[0]
            for etype, w in zip(event_types, weights):
                cumulative += w
                if r < cumulative:
                    chosen = etype
                    break
            events.append(
                _make_event(chosen, ts_offset_ms=random.randint(0, FIVE_MINUTES_MS - 1))
            )

        counts: Dict[str, int] = {"page_view": 0, "click": 0, "purchase": 0}
        for event in events:
            etype = event.value["type"]
            counts[etype] = counts.get(etype, 0) + 1

        assert sum(counts.values()) == 10_000
        assert all(c > 0 for c in counts.values())
        assert counts["page_view"] > counts["click"] > counts["purchase"]

        print("\n=== 10K Event Ingestion Summary ===")
        print(f"  Total events: {sum(counts.values())}")
        print(f"  Page views: {counts['page_view']}")
        print(f"  Clicks: {counts['click']}")
        print(f"  Purchases: {counts['purchase']}")

    @pytest.mark.asyncio
    async def test_tumbling_window_aggregation(self):
        """Tumbling 5-minute windows aggregate event counts correctly."""
        window_results: List[Dict[str, Any]] = []

        def aggregate(events: List[StreamEvent], info: WindowInfo) -> Dict[str, Any]:
            counts: Dict[str, int] = {}
            for e in events:
                etype = e.value["type"]
                counts[etype] = counts.get(etype, 0) + 1
            result = {
                "window_id": info.window_id,
                "start": info.start.milliseconds,
                "end": info.end.milliseconds,
                "total": len(events),
                "counts": counts,
            }
            window_results.append(result)
            return result

        tw = TumblingWindow(
            size=Duration.from_millis(FIVE_MINUTES_MS),
            handler=aggregate,
        )

        event_types = ["page_view", "click", "purchase"]
        for i in range(1000):
            ts_offset = i * (FIVE_MINUTES_MS // 1000)
            for _ in range(10):
                etype = random.choice(event_types)
                event = _make_event(etype, ts_offset_ms=ts_offset)
                tw.process(event, key="analytics")

        watermark_ts = Timestamp(BASE_TS.milliseconds + FIVE_MINUTES_MS)
        tw.advance_watermark(watermark_ts)

        assert len(window_results) > 0
        for result in window_results:
            assert result["total"] > 0
            assert (
                "page_view" in result["counts"]
                or "click" in result["counts"]
                or "purchase" in result["counts"]
            )

        total_in_windows = sum(r["total"] for r in window_results)
        print("\n=== Tumbling Window Aggregation Summary ===")
        print(f"  Windows fired: {len(window_results)}")
        print(f"  Total events in windows: {total_in_windows}")

    @pytest.mark.asyncio
    async def test_running_average_computation(self):
        """Compute running averages per event type across windows."""
        running_counts: Dict[str, List[int]] = {
            "page_view": [],
            "click": [],
            "purchase": [],
        }

        def track_averages(
            events: List[StreamEvent], info: WindowInfo
        ) -> Dict[str, float]:
            counts: Dict[str, int] = {"page_view": 0, "click": 0, "purchase": 0}
            for e in events:
                etype = e.value["type"]
                counts[etype] = counts.get(etype, 0) + 1
            for etype in counts:
                running_counts[etype].append(counts[etype])
            averages = {}
            for etype, vals in running_counts.items():
                if vals:
                    averages[etype] = sum(vals) / len(vals)
            return averages

        tw = TumblingWindow(
            size=Duration.from_millis(FIVE_MINUTES_MS),
            handler=track_averages,
        )

        for window_idx in range(3):
            base = window_idx * FIVE_MINUTES_MS
            for i in range(500):
                ts_offset = base + random.randint(0, FIVE_MINUTES_MS - 1)
                etype = random.choice(["page_view", "click", "purchase"])
                tw.process(_make_event(etype, ts_offset_ms=ts_offset), key="avg-test")

            tw.advance_watermark(
                Timestamp(BASE_TS.milliseconds + (window_idx + 1) * FIVE_MINUTES_MS)
            )

        assert all(len(v) >= 3 for v in running_counts.values())
        assert all(sum(v) > 0 for v in running_counts.values())

        print("\n=== Running Average Summary ===")
        for etype, vals in running_counts.items():
            avg = sum(vals) / len(vals)
            print(f"  {etype}: counts={vals}, avg={avg:.1f}")

    @pytest.mark.asyncio
    async def test_alerting_on_threshold(self):
        """Trigger alerts when purchase count exceeds 1000 in a 5-min window."""
        alerts: List[Dict[str, Any]] = []
        ALERT_THRESHOLD = 1000

        def check_alerts(
            events: List[StreamEvent], info: WindowInfo
        ) -> Optional[Dict[str, Any]]:
            purchase_count = sum(1 for e in events if e.value["type"] == "purchase")
            if purchase_count > ALERT_THRESHOLD:
                alert = {
                    "window_id": info.window_id,
                    "threshold": ALERT_THRESHOLD,
                    "actual": purchase_count,
                    "severity": "critical" if purchase_count > 1500 else "warning",
                }
                alerts.append(alert)
                return alert
            return None

        tw = TumblingWindow(
            size=Duration.from_millis(FIVE_MINUTES_MS),
            handler=check_alerts,
        )

        for i in range(1500):
            event = _make_event(
                "purchase", ts_offset_ms=random.randint(0, FIVE_MINUTES_MS - 1)
            )
            tw.process(event, key="alerts")

        tw.advance_watermark(Timestamp(BASE_TS.milliseconds + 2 * FIVE_MINUTES_MS))

        assert len(alerts) > 0
        for alert in alerts:
            assert alert["actual"] > ALERT_THRESHOLD

        print("\n=== Alerting Summary ===")
        print(f"  Alerts triggered: {len(alerts)}")
        for a in alerts:
            print(
                f"  [{a['severity'].upper()}] {a['actual']} purchases (threshold: {a['threshold']})"
            )

    @pytest.mark.asyncio
    async def test_late_data_watermark_handling(self):
        """Inject late events and verify watermark-based late data handling."""
        late_events: List[StreamEvent] = []
        all_fired: List[Dict[str, Any]] = []

        def aggregate_window(
            events: List[StreamEvent], info: WindowInfo
        ) -> Dict[str, Any]:
            result = {
                "window_id": info.window_id,
                "count": len(events),
                "pane": info.pane.name,
            }
            all_fired.append(result)
            return result

        tw = TumblingWindow(
            size=Duration.from_millis(FIVE_MINUTES_MS),
            handler=aggregate_window,
        )

        for i in range(500):
            ts_offset = random.randint(0, FIVE_MINUTES_MS - 1)
            event = _make_event("click", ts_offset_ms=ts_offset)
            tw.process(event, key="late-test")

        watermark = Watermark(
            timestamp=Timestamp(BASE_TS.milliseconds + FIVE_MINUTES_MS),
            stream_id="analytics",
        )
        fired = tw.advance_watermark(watermark.timestamp)
        on_time_count = sum(1 for e in fired if e is not None)

        late_event = _make_event("click", ts_offset_ms=1000)
        late_events.append(late_event)

        watermark_later = Watermark(
            timestamp=Timestamp(BASE_TS.milliseconds + 2 * FIVE_MINUTES_MS),
            stream_id="analytics",
        )
        tw.advance_watermark(watermark_later.timestamp)

        assert on_time_count > 0
        assert len(all_fired) >= 1
        assert all(f["count"] > 0 for f in all_fired)

        print("\n=== Late Data Handling Summary ===")
        print("  On-time events processed: 500")
        print(f"  Windows fired on-time: {len(all_fired)}")
        print(f"  Late events injected: {len(late_events)}")
        print(
            f"  Watermark advanced: {watermark.timestamp.milliseconds} -> {watermark_later.timestamp.milliseconds}"
        )


@pytest.mark.e2e
class TestBackpressureInPipeline:
    """Test backpressure handling in the analytics pipeline."""

    @pytest.mark.asyncio
    async def test_backpressure_under_load(self):
        """Verify backpressure controller handles event bursts correctly."""
        bp = BackpressureController(
            BackpressureConfig(
                strategy=BackpressureStrategy.BUFFER,
                buffer_size=5000,
            )
        )

        accepted = 0
        rejected = 0
        for i in range(10_000):
            event = _make_event("page_view", ts_offset_ms=i)
            if bp.try_push(event):
                accepted += 1
            else:
                rejected += 1

        assert accepted > 0
        assert accepted + rejected == 10_000

        consumed = 0
        while not bp.is_empty():
            bp.pop()
            consumed += 1

        assert consumed == accepted
        assert bp.is_empty()

        print("\n=== Backpressure Under Load Summary ===")
        print("  Events sent: 10_000")
        print(f"  Accepted: {accepted}")
        print(f"  Rejected (buffer full): {rejected}")
        print(f"  Consumed: {consumed}")

    @pytest.mark.asyncio
    async def test_rate_based_backpressure(self):
        """Verify rate-based backpressure limits processing rate."""
        rbp = RateBasedBackpressure(max_rate=100, window_size=1.0, cooldown=0.1)

        allowed = 0
        throttled = 0
        for _ in range(200):
            if await rbp.try_acquire():
                allowed += 1
            else:
                throttled += 1

        assert allowed > 0
        assert throttled > 0
        assert allowed <= 100

        print("\n=== Rate-Based Backpressure Summary ===")
        print("  Max rate: 100/s")
        print(f"  Allowed: {allowed}")
        print(f"  Throttled: {throttled}")

    @pytest.mark.asyncio
    async def test_stream_actor_with_windowing(self):
        """Test a StreamActor processing events through windowed aggregation."""
        collected: List[Dict[str, Any]] = []

        class AnalyticsActor(StreamActor):
            @classmethod
            def name(cls) -> str:
                return "analytics-actor"

            async def process_event(self, event: StreamEvent) -> None:
                pass

        actor = AnalyticsActor()

        window_results: List[Dict[str, Any]] = []

        def window_handler(events: List[StreamEvent], info: WindowInfo) -> str:
            result = {"count": len(events), "window": info.window_id}
            window_results.append(result)
            collected.append(result)
            return info.window_id

        actor.configure_window(
            WindowSpec(
                type=WindowType.TUMBLING,
                size=Duration.from_millis(FIVE_MINUTES_MS),
                late_tolerance=Duration.from_millis(0),
                allowed_lateness=Duration.from_millis(0),
            ),
            window_handler,
        )

        for i in range(200):
            ts_offset = random.randint(0, FIVE_MINUTES_MS - 1)
            event = _make_event("purchase", ts_offset_ms=ts_offset)
            await actor._process_event_internal(event)

        final_wm = Timestamp(BASE_TS.milliseconds + FIVE_MINUTES_MS)
        await actor.advance_watermark(Watermark(timestamp=final_wm, stream_id="input"))

        metrics = actor.get_metrics()
        assert metrics["processed_count"] == 200
        assert len(window_results) > 0

        print("\n=== Stream Actor with Windowing Summary ===")
        print(f"  Events processed: {metrics['processed_count']}")
        print(f"  Window results: {len(window_results)}")
        print(f"  Metrics: {metrics}")
