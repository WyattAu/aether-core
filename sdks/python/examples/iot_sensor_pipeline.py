"""
IoT Sensor Pipeline Example

Demonstrates:
- Tumbling windows for time-based aggregation of sensor events
- Watermark-based late data handling
- Backpressure for burst sensor data
- Alert generation when thresholds are exceeded
- StreamEvent processing with StreamActor

Simulates three sensor types (temperature, humidity, pressure) sending
events at realistic intervals through a pipeline that aggregates
per-minute windows and generates alerts.
"""

import asyncio
import random
import time
from dataclasses import dataclass
from typing import Dict, List, Optional

from aether_sdk.streaming.backpressure import (
    BackpressureController,
    MultiLevelBackpressure,
    RateBasedBackpressure,
)
from aether_sdk.streaming.types import (
    BackpressureConfig,
    BackpressureStrategy,
    Duration,
    StreamEvent,
    Timestamp,
    WindowInfo,
)
from aether_sdk.streaming.window import TumblingWindow

AUDIT_LOG = []


def log(msg: str):
    ts = time.strftime("%H:%M:%S")
    AUDIT_LOG.append(f"[{ts}] {msg}")
    print(f"  [{ts}] {msg}")


# ========================================
# Sensor Event Types
# ========================================


@dataclass
class SensorReading:
    sensor_id: str
    sensor_type: str
    value: float
    unit: str


THRESHOLDS = {
    "temperature": {"min": -10.0, "max": 50.0, "unit": "C"},
    "humidity": {"min": 0.0, "max": 100.0, "unit": "%"},
    "pressure": {"min": 950.0, "max": 1050.0, "unit": "hPa"},
}

SENSOR_IDS = {
    "temperature": ["temp-001", "temp-002"],
    "humidity": ["hum-001", "hum-002"],
    "pressure": ["press-001"],
}


def generate_reading(sensor_type: str) -> SensorReading:
    ranges = {
        "temperature": (15.0, 35.0),
        "humidity": (30.0, 80.0),
        "pressure": (990.0, 1030.0),
    }
    low, high = ranges[sensor_type]
    value = round(random.uniform(low, high), 1)
    sensor_id = random.choice(SENSOR_IDS[sensor_type])
    return SensorReading(
        sensor_id=sensor_id,
        sensor_type=sensor_type,
        value=value,
        unit=THRESHOLDS[sensor_type]["unit"],
    )


def check_threshold(reading: SensorReading) -> Optional[str]:
    threshold = THRESHOLDS[reading.sensor_type]
    if reading.value > threshold["max"]:
        return f"HIGH: {reading.sensor_type}={reading.value}{threshold['unit']} exceeds {threshold['max']}{threshold['unit']}"
    if reading.value < threshold["min"]:
        return f"LOW: {reading.sensor_type}={reading.value}{threshold['unit']} below {threshold['min']}{threshold['unit']}"
    return None


# ========================================
# Window Aggregation
# ========================================


@dataclass
class AggregatedWindow:
    sensor_type: str
    avg: float
    min_val: float
    max_val: float
    count: int
    window_start: int
    window_end: int


def aggregate_events(
    events: List[StreamEvent[SensorReading]], window_info: WindowInfo
) -> AggregatedWindow:
    if not events:
        return AggregatedWindow(
            sensor_type="",
            avg=0,
            min_val=0,
            max_val=0,
            count=0,
            window_start=0,
            window_end=0,
        )

    values = [e.value.value for e in events]
    sensor_type = events[0].value.sensor_type

    return AggregatedWindow(
        sensor_type=sensor_type,
        avg=round(sum(values) / len(values), 1),
        min_val=min(values),
        max_val=max(values),
        count=len(values),
        window_start=window_info.start.milliseconds,
        window_end=window_info.end.milliseconds,
    )


# ========================================
# Pipeline
# ========================================


async def run_sensor_pipeline():
    print("=" * 70)
    print("  AETHER IOT SENSOR PIPELINE")
    print("=" * 70)
    print()

    window_size = Duration.from_seconds(60)
    all_aggregations: List[AggregatedWindow] = []
    all_alerts: List[str] = []

    print("--- Step 1: Set up tumbling windows per sensor type ---")
    temp_window = TumblingWindow(window_size, aggregate_events)
    hum_window = TumblingWindow(window_size, aggregate_events)
    press_window = TumblingWindow(window_size, aggregate_events)
    log(f"Created tumbling windows (size={window_size.to_seconds()}s)")
    print()

    print("--- Step 2: Set up backpressure controller ---")
    bp_config = BackpressureConfig(
        strategy=BackpressureStrategy.BUFFER,
        buffer_size=50,
        high_watermark=0.8,
        low_watermark=0.3,
    )
    backpressure = BackpressureController(bp_config)
    overflow_count = [0]

    def on_overflow():
        overflow_count[0] += 1
        log(f"BACKPRESSURE: High watermark reached (overflow #{overflow_count[0]})")

    backpressure.set_overflow_callback(on_overflow)

    resume_count = [0]

    def on_resume():
        resume_count[0] += 1
        log(f"BACKPRESSURE: Buffer recovered (resume #{resume_count[0]})")

    backpressure.set_resume_callback(on_resume)
    log(
        f"Backpressure configured: strategy={bp_config.strategy.value}, buffer={bp_config.buffer_size}"
    )
    print()

    print("--- Step 3: Set up watermark tracking ---")
    watermarks: Dict[str, Timestamp] = {}
    late_events: List[StreamEvent] = []

    log("Watermark tracking initialized")
    print()

    print("--- Step 4: Generate and process sensor events ---")
    print()

    base_time_ms = int(time.time() * 1000)
    base_time_ms = (base_time_ms // 60000) * 60000

    window_map = {
        "temperature": temp_window,
        "humidity": hum_window,
        "pressure": press_window,
    }

    events_accepted = 0
    events_dropped = 0
    events_late = 0

    for second_offset in range(0, 125, 2):
        event_time_ms = base_time_ms + (second_offset * 1000)
        event_ts = Timestamp(event_time_ms)

        num_readings = random.choices([1, 2, 3], weights=[5, 3, 1])[0]

        for _ in range(num_readings):
            sensor_type = random.choice(["temperature", "humidity", "pressure"])

            is_late = random.random() < 0.05
            if is_late and second_offset > 30:
                reading = generate_reading(sensor_type)
                late_ts = Timestamp(event_time_ms - random.randint(90000, 120000))
                late_event = StreamEvent.create(
                    key=reading.sensor_id,
                    value=reading,
                    timestamp=late_ts,
                    event_type=sensor_type,
                )

                current_wm = watermarks.get(sensor_type, Timestamp(0))
                if late_event.timestamp < current_wm:
                    events_late += 1
                    late_events.append(late_event)
                    log(
                        f"LATE DATA: {sensor_type} from {reading.sensor_id} "
                        f"at {late_ts.to_datetime().strftime('%H:%M:%S')} "
                        f"(watermark: {current_wm.to_datetime().strftime('%H:%M:%S')})"
                    )
                    continue

            reading = generate_reading(sensor_type)
            event = StreamEvent.create(
                key=reading.sensor_id,
                value=reading,
                timestamp=event_ts,
                event_type=sensor_type,
            )

            alert = check_threshold(reading)
            if alert:
                all_alerts.append(alert)
                log(f"ALERT: {alert}")

            if not backpressure.try_push(event):
                events_dropped += 1
                log(f"DROPPED: Event from {reading.sensor_id} (buffer full)")
                continue

            events_accepted += 1

            buffered_event = backpressure.pop()
            if buffered_event is not None:
                window = window_map[buffered_event.value.sensor_type]
                results = window.process(buffered_event, buffered_event.event_type)
                for r in results:
                    all_aggregations.append(r)

        watermark_ts = Timestamp(event_time_ms + 5000)
        for stype in ["temperature", "humidity", "pressure"]:
            watermarks[stype] = watermark_ts
            window = window_map[stype]
            results = window.advance_watermark(watermark_ts)
            for r in results:
                all_aggregations.append(r)

        if second_offset % 20 == 0:
            stats = backpressure.stats
            log(
                f"PROGRESS: t={second_offset}s | accepted={events_accepted} "
                f"dropped={events_dropped} late={events_late} "
                f"buffer={stats.current_buffer_size}/{bp_config.buffer_size} "
                f"windows_fired={len(all_aggregations)}"
            )

    print()

    print("--- Step 5: Fire remaining windows ---")
    final_watermark = Timestamp(base_time_ms + 180000)
    for stype, window in window_map.items():
        results = window.advance_watermark(final_watermark)
        for r in results:
            all_aggregations.append(r)
    log(f"Fired remaining windows, total aggregations: {len(all_aggregations)}")
    print()

    print("--- Step 6: Multi-level backpressure demo ---")
    mlbp = MultiLevelBackpressure(buffer_size=10)
    for i in range(15):
        priority = (
            MultiLevelBackpressure.Priority.LOW
            if i < 8
            else MultiLevelBackpressure.Priority.HIGH
        )
        event = StreamEvent.create(
            key=f"sensor-{i}", value=i, timestamp=Timestamp.now()
        )
        accepted = mlbp.push(event, priority)
        if not accepted:
            log(f"MLBP: Event {i} dropped (priority={priority})")

    mlbp_stats = mlbp.size()
    log(f"Multi-level BP: buffered={mlbp_stats}, " f"dropped={15 - mlbp_stats}")

    while not mlbp.is_empty():
        mlbp.pop()
    log("Multi-level BP: drained")
    print()

    print("--- Step 7: Rate-based backpressure demo ---")
    rate_bp = RateBasedBackpressure(max_rate=5, window_size=1.0, cooldown=0.2)
    allowed = 0
    rejected = 0
    for _ in range(20):
        if await rate_bp.try_acquire():
            allowed += 1
        else:
            rejected += 1
    log(
        f"Rate-based BP: allowed={allowed}, rejected={rejected}, "
        f"active={rate_bp.is_backpressure_active}"
    )
    print()

    print("=" * 70)
    print("  PIPELINE SUMMARY")
    print("=" * 70)
    log(f"Total events accepted: {events_accepted}")
    log(f"Total events dropped:  {events_dropped}")
    log(f"Total late events:     {events_late}")
    log(f"Total alerts:          {len(all_alerts)}")
    log(f"Total window results:  {len(all_aggregations)}")
    log(f"Backpressure overflows: {overflow_count[0]}")
    log(f"Backpressure resumes:   {resume_count[0]}")
    print()

    if all_aggregations:
        print("  Window Aggregations:")
        for agg in all_aggregations[:12]:
            duration_s = (agg.window_end - agg.window_start) / 1000
            log(
                f"  [{agg.sensor_type:11s}] avg={agg.avg:6.1f} "
                f"min={agg.min_val:6.1f} max={agg.max_val:6.1f} "
                f"count={agg.count:3d} window={duration_s:.0f}s"
            )
        if len(all_aggregations) > 12:
            log(f"  ... and {len(all_aggregations) - 12} more")
    print()

    if all_alerts:
        print("  Alerts Generated:")
        for alert in all_alerts[:8]:
            log(f"  ! {alert}")
        if len(all_alerts) > 8:
            log(f"  ... and {len(all_alerts) - 8} more")
    else:
        log("No threshold alerts generated (all readings within range)")
    print()

    print(f"Total audit entries: {len(AUDIT_LOG)}")


if __name__ == "__main__":
    asyncio.run(run_sensor_pipeline())
