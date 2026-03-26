import json
import os

import pytest

from aether_sdk.streaming.types import (
    Duration,
    StreamEvent,
    Timestamp,
    Watermark,
    WindowSpec,
    WindowType,
    LateDataPolicy,
    WatermarkStrategy,
    DeliverySemantics,
    PaneInfo,
)

VECTORS_PATH = os.path.join(os.path.dirname(__file__), "test_vectors.json")


def _load_vectors():
    with open(VECTORS_PATH) as f:
        return json.load(f)


class TestTimestampCompat:
    def test_epoch_zero(self):
        ts = Timestamp(0)
        assert ts.milliseconds == 0
        assert ts.to_seconds() == 0.0

    def test_from_seconds_precision(self):
        ts = Timestamp.from_seconds(1.5)
        assert ts.milliseconds == 1500
        assert ts.to_seconds() == 1.5

    def test_timestamp_json_is_integer_milliseconds(self):
        ts = Timestamp(1700000000000)
        data = {"milliseconds": ts.milliseconds}
        assert data["milliseconds"] == 1700000000000
        assert isinstance(data["milliseconds"], int)

    def test_arithmetic_with_duration(self):
        ts = Timestamp(1000)
        d = Duration.from_seconds(5)
        result = ts + d
        assert result.milliseconds == 6000

    def test_subtraction_yields_duration(self):
        a = Timestamp(10000)
        b = Timestamp(3000)
        diff = a - b
        assert diff.ms == 7000

    def test_comparison_operators(self):
        a = Timestamp(1000)
        b = Timestamp(2000)
        assert a < b
        assert b > a
        assert a <= a
        assert b >= b

    def test_all_timestamp_vectors(self):
        vectors = _load_vectors()
        for vec in vectors["timestamps"]:
            ts = Timestamp(vec["input_ms"])
            assert ts.milliseconds == vec["input_ms"]


class TestDurationCompat:
    def test_zero_duration(self):
        d = Duration.from_millis(0)
        assert d.to_seconds() == 0.0
        assert d.to_millis() == 0

    def test_from_minutes(self):
        d = Duration.from_minutes(5)
        assert d.to_seconds() == 300.0
        assert d.to_millis() == 300000

    def test_from_hours(self):
        d = Duration.from_hours(1)
        assert d.to_millis() == 3600000

    def test_duration_addition(self):
        a = Duration.from_seconds(10)
        b = Duration.from_seconds(20)
        result = a + b
        assert result.to_seconds() == 30.0

    def test_duration_scalar_multiplication(self):
        d = Duration.from_seconds(5)
        result = d * 3
        assert result.to_seconds() == 15.0

    def test_all_duration_vectors(self):
        vectors = _load_vectors()
        for vec in vectors["durations"]:
            d = Duration.from_millis(vec["input_ms"])
            assert d.to_seconds() == vec["expected_seconds"]


class TestWindowSpecCompat:
    def test_tumbling_window_spec(self):
        spec = WindowSpec(type=WindowType.TUMBLING, size=Duration.from_seconds(5))
        assert spec.type == WindowType.TUMBLING
        assert spec.size.to_millis() == 5000
        assert spec.slide is None
        assert spec.gap is None

    def test_sliding_window_spec(self):
        spec = WindowSpec(
            type=WindowType.SLIDING,
            size=Duration.from_seconds(10),
            slide=Duration.from_seconds(5),
        )
        assert spec.slide.to_millis() == 5000

    def test_session_window_spec(self):
        spec = WindowSpec(
            type=WindowType.SESSION,
            size=Duration.from_hours(24),
            gap=Duration.from_seconds(30),
        )
        assert spec.gap.to_millis() == 30000

    def test_sliding_window_requires_slide(self):
        with pytest.raises(ValueError):
            WindowSpec(type=WindowType.SLIDING, size=Duration.from_seconds(10))

    def test_session_window_requires_gap(self):
        with pytest.raises(ValueError):
            WindowSpec(type=WindowType.SESSION, size=Duration.from_hours(24))

    def test_window_spec_serialization(self):
        spec = WindowSpec(
            type=WindowType.TUMBLING,
            size=Duration.from_seconds(5),
            late_tolerance=Duration.from_seconds(1),
            allowed_lateness=Duration.from_seconds(2),
        )
        data = {
            "type": spec.type.name,
            "size_ms": spec.size.to_millis(),
            "late_tolerance_ms": spec.late_tolerance.to_millis(),
            "allowed_lateness_ms": spec.allowed_lateness.to_millis(),
        }
        assert json.dumps(data) is not None

    def test_all_window_vectors(self):
        vectors = _load_vectors()
        for vec in vectors["window_specs"]:
            wt = WindowType[vec["type"].upper()]
            spec = WindowSpec(
                type=wt,
                size=Duration.from_millis(vec["size_ms"]),
                slide=Duration.from_millis(vec["slide_ms"]) if vec["slide_ms"] else None,
                gap=Duration.from_millis(vec["gap_ms"]) if vec["gap_ms"] else None,
            )
            assert spec.size.to_millis() == vec["size_ms"]


class TestStreamEventCompat:
    def test_event_creation(self):
        event = StreamEvent.create(key="order-1", value={"total": 42.0})
        assert event.key == "order-1"
        assert event.value == {"total": 42.0}
        assert isinstance(event.timestamp, Timestamp)
        assert event.headers == {}

    def test_event_with_metadata(self):
        ts = Timestamp(1700000000000)
        event = StreamEvent.create(
            key="user-1",
            value={"name": "Alice"},
            timestamp=ts,
            headers={"trace-id": "abc"},
            partition=0,
            offset=42,
            event_type="user_created",
        )
        assert event.timestamp.milliseconds == 1700000000000
        assert event.headers["trace-id"] == "abc"
        assert event.partition == 0
        assert event.offset == 42
        assert event.event_type == "user_created"

    def test_event_serialization(self):
        ts = Timestamp(1000)
        event = StreamEvent.create(key="k", value={"x": 1}, timestamp=ts)
        data = {
            "key": event.key,
            "value": event.value,
            "timestamp_ms": event.timestamp.milliseconds,
            "headers": event.headers,
        }
        parsed = json.loads(json.dumps(data))
        assert parsed["key"] == "k"
        assert parsed["timestamp_ms"] == 1000


class TestWatermarkCompat:
    def test_watermark_creation(self):
        ts = Timestamp(5000)
        wm = Watermark(timestamp=ts, stream_id="input-stream")
        assert wm.timestamp.milliseconds == 5000
        assert wm.stream_id == "input-stream"

    def test_watermark_late_detection(self):
        wm = Watermark(timestamp=Timestamp(10000), stream_id="s")
        assert wm.is_late(Timestamp(5000)) is True
        assert wm.is_late(Timestamp(10000)) is False
        assert wm.is_late(Timestamp(15000)) is False

    def test_watermark_serialization(self):
        wm = Watermark(timestamp=Timestamp(7000), stream_id="stream-1", partition=3)
        data = {
            "timestamp_ms": wm.timestamp.milliseconds,
            "stream_id": wm.stream_id,
            "partition": wm.partition,
        }
        assert data["timestamp_ms"] == 7000
        assert data["stream_id"] == "stream-1"
        assert data["partition"] == 3


class TestEnumValueConsistency:
    def test_late_data_policy_names(self):
        expected = {"DROP", "SIDE_OUTPUT", "REPROCESS"}
        actual = {e.name for e in LateDataPolicy}
        assert actual == expected

    def test_watermark_strategy_names(self):
        expected = {"EVENT_TIME", "PROCESSING_TIME", "BOUNDED_OUT_OF_ORDER"}
        actual = {e.name for e in WatermarkStrategy}
        assert actual == expected

    def test_delivery_semantics_names(self):
        expected = {"AT_MOST_ONCE", "AT_LEAST_ONCE", "EXACTLY_ONCE"}
        actual = {e.name for e in DeliverySemantics}
        assert actual == expected

    def test_pane_info_names(self):
        expected = {"EARLY", "ON_TIME", "LATE"}
        actual = {e.name for e in PaneInfo}
        assert actual == expected
