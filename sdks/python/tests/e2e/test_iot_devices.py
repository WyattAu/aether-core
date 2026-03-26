"""E2E Scenario 3: IoT Device Management

Simulates IoT device lifecycle:
- Device Registration with metadata (type, firmware version)
- Telemetry Processing (temperature, humidity, pressure)
- Alert Generation for abnormal readings
- Firmware Update Coordination with batch update and rollback on failure
- Device Health via periodic ping
"""

import pytest
import asyncio
import random
from dataclasses import dataclass, field
from typing import Dict, Any, List, Optional

from aether_sdk.actor import Actor
from aether_sdk.messaging import Message, MessageType
from aether_sdk.state import StateHandle
from aether_sdk.resilience.circuit_breaker import CircuitBreaker, CircuitBreakerConfig, CircuitState
from aether_sdk.resilience.retry import RetryPolicy, RetryConfig, BackoffStrategy
from aether_sdk.resilience.health_check import (
    HealthChecker,
    HealthCheckResult,
    HealthStatus,
    HealthCheckOptions,
    ping_health_check,
    dependency_health_check,
)
from aether_sdk.workflow.saga import Saga, SagaExecutor
from aether_sdk.workflow.types import SagaStatus, RetryPolicy as SagaRetryPolicy, Duration as SagaDuration

random.seed(42)


@pytest.mark.e2e
class TestIoTDeviceRegistration:
    """Test device registration and metadata management."""

    @pytest.mark.asyncio
    async def test_register_single_device(self):
        """Register a single device and verify its metadata is persisted."""

        class DeviceRegistry(Actor):
            def __init__(self):
                super().__init__()
                self._state = StateHandle()

            @classmethod
            def name(cls) -> str:
                return "device-registry"

            async def handle_message(self, sender: str, message: Message) -> Optional[Message]:
                action = message.payload.get("action")
                if action == "register":
                    device_id = message.payload["device_id"]
                    metadata = message.payload.get("metadata", {})
                    await self._state.set_json(f"device:{device_id}", {
                        "device_id": device_id,
                        "registered": True,
                        "metadata": metadata,
                    })
                    return Message(
                        type=MessageType.CUSTOM,
                        payload={"status": "registered", "device_id": device_id},
                    )
                elif action == "get":
                    device_id = message.payload["device_id"]
                    device = await self._state.get_json(f"device:{device_id}")
                    return Message(
                        type=MessageType.CUSTOM,
                        payload={"device": device},
                    )
                return None

        registry = DeviceRegistry()
        reg_msg = Message(
            type=MessageType.CUSTOM,
            payload={
                "action": "register",
                "device_id": "sensor-001",
                "metadata": {"type": "temperature", "firmware": "v2.1.0", "location": "warehouse-a"},
            },
        )
        response = await registry.handle_message("admin", reg_msg)

        assert response.payload["status"] == "registered"
        assert response.payload["device_id"] == "sensor-001"

        get_msg = Message(
            type=MessageType.CUSTOM,
            payload={"action": "get", "device_id": "sensor-001"},
        )
        get_response = await registry.handle_message("admin", get_msg)
        device = get_response.payload["device"]
        assert device["registered"] is True
        assert device["metadata"]["type"] == "temperature"
        assert device["metadata"]["firmware"] == "v2.1.0"

        print("\n=== Device Registration Summary ===")
        print(f"  Device ID: sensor-001")
        print(f"  Type: temperature")
        print(f"  Firmware: v2.1.0")
        print(f"  Location: warehouse-a")
        print(f"  Status: registered")

    @pytest.mark.asyncio
    async def test_register_multiple_devices(self):
        """Register multiple devices of different types."""
        registry_state = StateHandle()
        devices = [
            {"device_id": "temp-001", "type": "temperature", "firmware": "v1.0"},
            {"device_id": "temp-002", "type": "temperature", "firmware": "v1.0"},
            {"device_id": "hum-001", "type": "humidity", "firmware": "v2.0"},
            {"device_id": "pres-001", "type": "pressure", "firmware": "v1.5"},
            {"device_id": "pres-002", "type": "pressure", "firmware": "v1.5"},
        ]

        for device in devices:
            await registry_state.set_json(f"device:{device['device_id']}", {
                "device_id": device["device_id"],
                "type": device["type"],
                "firmware": device["firmware"],
                "registered": True,
            })

        all_devices: List[Dict[str, Any]] = []
        for device in devices:
            d = await registry_state.get_json(f"device:{device['device_id']}")
            assert d is not None
            all_devices.append(d)

        assert len(all_devices) == 5
        types = {d["type"] for d in all_devices}
        assert types == {"temperature", "humidity", "pressure"}

        print(f"\n=== Multi-Device Registration Summary ===")
        print(f"  Total devices: {len(all_devices)}")
        for t in sorted(types):
            count = sum(1 for d in all_devices if d["type"] == t)
            print(f"  {t}: {count} devices")


@pytest.mark.e2e
class TestTelemetryProcessing:
    """Test telemetry data processing with threshold alerting."""

    @pytest.mark.asyncio
    async def test_telemetry_with_threshold_alerts(self):
        """Process telemetry readings and generate alerts for abnormal values."""
        alerts: List[Dict[str, Any]] = []
        readings: List[Dict[str, Any]] = []
        thresholds = {
            "temperature": {"min": -10, "max": 50},
            "humidity": {"min": 10, "max": 90},
            "pressure": {"min": 950, "max": 1050},
        }

        def process_telemetry(device_id: str, sensor_type: str, value: float):
            reading = {"device_id": device_id, "type": sensor_type, "value": value}
            readings.append(reading)

            if sensor_type in thresholds:
                t = thresholds[sensor_type]
                if value < t["min"] or value > t["max"]:
                    alert = {
                        "device_id": device_id,
                        "type": sensor_type,
                        "value": value,
                        "severity": "critical" if abs(value - (t["min"] + t["max"]) / 2) > (t["max"] - t["min"]) * 0.4 else "warning",
                        "message": f"{sensor_type} out of range: {value} (allowed: {t['min']}-{t['max']})",
                    }
                    alerts.append(alert)

        normal_readings = [
            ("temp-001", "temperature", 22.5),
            ("temp-001", "temperature", 23.1),
            ("hum-001", "humidity", 45.0),
            ("pres-001", "pressure", 1013.0),
        ]

        abnormal_readings = [
            ("temp-001", "temperature", 85.0),
            ("hum-001", "humidity", 95.0),
            ("pres-001", "pressure", 900.0),
            ("temp-002", "temperature", -25.0),
        ]

        for device_id, sensor_type, value in normal_readings + abnormal_readings:
            process_telemetry(device_id, sensor_type, value)

        assert len(readings) == 8
        assert len(alerts) == 4

        for alert in alerts:
            assert alert["severity"] in ("critical", "warning")
            assert "out of range" in alert["message"]

        print(f"\n=== Telemetry Alerting Summary ===")
        print(f"  Total readings: {len(readings)}")
        print(f"  Alerts generated: {len(alerts)}")
        for a in alerts:
            print(f"  [{a['severity'].upper()}] {a['device_id']}: {a['message']}")

    @pytest.mark.asyncio
    async def test_telemetry_aggregation(self):
        """Aggregate telemetry over multiple readings per device."""
        device_readings: Dict[str, List[float]] = {}

        devices = ["temp-001", "temp-002", "hum-001"]
        for _ in range(100):
            device = random.choice(devices)
            value = random.uniform(10, 40)
            device_readings.setdefault(device, []).append(value)

        stats: Dict[str, Dict[str, float]] = {}
        for device_id, values in device_readings.items():
            stats[device_id] = {
                "count": len(values),
                "avg": sum(values) / len(values),
                "min": min(values),
                "max": max(values),
            }

        for device_id in devices:
            assert device_id in stats
            s = stats[device_id]
            assert s["count"] > 0
            assert s["min"] <= s["avg"] <= s["max"]

        print(f"\n=== Telemetry Aggregation Summary ===")
        for device_id, s in stats.items():
            print(f"  {device_id}: count={s['count']}, avg={s['avg']:.1f}, min={s['min']:.1f}, max={s['max']:.1f}")


@pytest.mark.e2e
class TestFirmwareUpdate:
    """Test firmware update coordination with rollback."""

    @pytest.mark.asyncio
    async def test_batch_firmware_update_success(self):
        """Batch update firmware for all devices of a type."""
        updated_devices: List[str] = []
        audit_log: List[str] = []

        async def update_firmware(device_id: str, new_version: str):
            audit_log.append(f"FIRMWARE: Updating {device_id} to {new_version}")
            await asyncio.sleep(0.001)
            updated_devices.append(device_id)
            audit_log.append(f"FIRMWARE: {device_id} updated successfully")

        devices = [f"temp-{i:03d}" for i in range(1, 6)]
        tasks = [update_firmware(d, "v3.0.0") for d in devices]
        await asyncio.gather(*tasks)

        assert len(updated_devices) == 5
        assert all(d in updated_devices for d in devices)
        assert len(audit_log) == 10

        print(f"\n=== Batch Firmware Update Summary ===")
        print(f"  Devices updated: {len(updated_devices)}/{len(devices)}")
        print(f"  New version: v3.0.0")
        for d in updated_devices:
            print(f"  {d}: SUCCESS")

    @pytest.mark.asyncio
    async def test_firmware_update_rollback_on_failure(self):
        """Rollback firmware update when one device fails."""
        audit_log: List[str] = []
        updated_devices: List[str] = []
        rolled_back: List[str] = []

        fail_device = "temp-003"

        async def update_device(device_id: str, version: str):
            if device_id == fail_device:
                audit_log.append(f"FIRMWARE: {device_id} FAILED")
                raise RuntimeError(f"Update failed for {device_id}")
            audit_log.append(f"FIRMWARE: {device_id} updated to {version}")
            updated_devices.append(device_id)

        async def rollback_device(device_id: str, old_version: str):
            audit_log.append(f"FIRMWARE: {device_id} rolled back to {old_version}")
            rolled_back.append(device_id)

        devices = [f"temp-{i:03d}" for i in range(1, 6)]
        failed_device = None

        for device in devices:
            try:
                await update_device(device, "v4.0.0")
            except RuntimeError:
                failed_device = device
                break

        if failed_device:
            for d in updated_devices:
                await rollback_device(d, "v2.1.0")

        assert failed_device == fail_device
        assert len(updated_devices) == 2
        assert len(rolled_back) == 2
        assert "FIRMWARE: temp-003 FAILED" in audit_log

        print(f"\n=== Firmware Rollback Summary ===")
        print(f"  Failed device: {failed_device}")
        print(f"  Updated before failure: {updated_devices}")
        print(f"  Rolled back: {rolled_back}")
        print(f"  Audit log: {audit_log}")

    @pytest.mark.asyncio
    async def test_firmware_update_with_saga(self):
        """Use saga pattern for firmware update with automatic compensation."""
        updated: List[str] = []
        rolled_back: List[str] = []

        async def update_device_1(ctx) -> Dict[str, str]:
            updated.append("temp-001")
            return {"device": "temp-001", "status": "updated"}

        async def rollback_1(ctx):
            rolled_back.append("temp-001")

        async def update_device_2(ctx) -> Dict[str, str]:
            updated.append("temp-002")
            return {"device": "temp-002", "status": "updated"}

        async def rollback_2(ctx):
            rolled_back.append("temp-002")

        async def update_device_3(ctx) -> Dict[str, str]:
            raise RuntimeError("Device temp-003 offline")

        saga_def = (
            Saga("firmware-update")
            .step("update-temp-001")
            .action(update_device_1)
            .compensate(rollback_1)
            .step("update-temp-002")
            .action(update_device_2)
            .compensate(rollback_2)
            .step("update-temp-003")
            .action(update_device_3)
            .build()
        )

        executor = SagaExecutor()
        result = await executor.execute(saga_def, {"version": "v5.0.0"})

        assert result.status == SagaStatus.COMPENSATED
        assert "temp-001" in updated
        assert "temp-002" in updated
        assert "temp-003" not in updated
        assert "temp-002" in rolled_back
        assert "temp-001" in rolled_back

        print(f"\n=== Saga Firmware Update Summary ===")
        print(f"  Status: {result.status.value}")
        print(f"  Updated before failure: {updated}")
        print(f"  Rolled back: {rolled_back}")


@pytest.mark.e2e
class TestDeviceHealth:
    """Test device health monitoring."""

    @pytest.mark.asyncio
    async def test_periodic_health_check(self):
        """Simulate periodic health checks for devices."""
        checker = HealthChecker(service_id="iot-gateway", version="1.0.0")

        healthy_devices = ["temp-001", "hum-001", "pres-001"]
        unhealthy_devices = ["temp-002"]

        await checker.register_check(
            "temp-001",
            lambda: HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="temp-001",
                component_type="device",
            ),
            HealthCheckOptions(timeout_ms=100, critical=False),
        )

        await checker.register_check(
            "temp-002",
            lambda: HealthCheckResult(
                status=HealthStatus.UNHEALTHY,
                component_id="temp-002",
                component_type="device",
                output="Device offline",
            ),
            HealthCheckOptions(timeout_ms=100, critical=True),
        )

        await checker.register_check(
            "hum-001",
            lambda: HealthCheckResult(
                status=HealthStatus.HEALTHY,
                component_id="hum-001",
                component_type="device",
            ),
            HealthCheckOptions(timeout_ms=100, critical=False),
        )

        report = await checker.run_all()
        assert report.status == HealthStatus.UNHEALTHY
        assert report.checks["temp-001"].status == HealthStatus.HEALTHY
        assert report.checks["temp-002"].status == HealthStatus.UNHEALTHY
        assert report.checks["hum-001"].status == HealthStatus.HEALTHY

        liveness = await checker.get_liveness()
        assert liveness["alive"] is True

        print(f"\n=== Device Health Check Summary ===")
        print(f"  Overall status: {report.status.value}")
        for name, check in report.checks.items():
            print(f"  {name}: {check.status.value}")

        await checker.shutdown()

    @pytest.mark.asyncio
    async def test_device_ping_with_circuit_breaker(self):
        """Use circuit breaker for device ping to handle flaky devices."""
        ping_attempts: Dict[str, int] = {}
        fail_device = "temp-flaky"

        async def ping_device(device_id: str) -> bool:
            ping_attempts[device_id] = ping_attempts.get(device_id, 0) + 1
            if device_id == fail_device and ping_attempts[device_id] <= 3:
                raise ConnectionError(f"Device {device_id} unreachable")
            return True

        cb = CircuitBreaker(CircuitBreakerConfig(
            failure_threshold=3,
            success_threshold=2,
            timeout_ms=60000,
            failure_window_ms=60000,
        ))

        healthy_results = []
        for _ in range(5):
            try:
                result = await cb.execute(lambda did="temp-stable": ping_device(did))
                healthy_results.append(result)
            except Exception:
                pass

        assert cb.state == CircuitState.CLOSED

        for _ in range(3):
            try:
                await cb.execute(lambda did=fail_device: ping_device(did))
            except Exception:
                pass

        assert cb.state == CircuitState.OPEN
        stats = cb.get_stats()
        assert stats.failures >= 3

        print(f"\n=== Circuit Breaker Device Ping Summary ===")
        print(f"  Stable device pings: {len(healthy_results)} successful")
        print(f"  Flaky device failures: 3")
        print(f"  Circuit breaker state: {cb.state.value}")
        print(f"  Stats: failures={stats.failures}, successes={stats.successes}")

    @pytest.mark.asyncio
    async def test_device_communication_with_retry(self):
        """Use retry policy for resilient device communication."""
        attempt_counts: Dict[str, int] = {}

        async def send_command(device_id: str, command: str):
            key = f"{device_id}:{command}"
            attempt_counts[key] = attempt_counts.get(key, 0) + 1
            if attempt_counts[key] < 3:
                raise ConnectionError("Timeout")
            return {"device_id": device_id, "command": command, "status": "ok"}

        policy = RetryPolicy(RetryConfig(
            max_attempts=5,
            backoff=BackoffStrategy.EXPONENTIAL_JITTER,
            base_delay_ms=10,
            max_delay_ms=100,
        ))

        result = await policy.execute(lambda did="temp-001", cmd="read": send_command(did, cmd))
        assert result.result["status"] == "ok"
        assert attempt_counts["temp-001:read"] == 3

        print(f"\n=== Device Retry Communication Summary ===")
        print(f"  Attempts needed: {result.attempts}")
        print(f"  Total delay: {result.total_delay_ms}ms")
        print(f"  Result: {result.result}")

    @pytest.mark.asyncio
    async def test_full_device_lifecycle(self):
        """Complete device lifecycle: register -> telemetry -> alert -> update -> health."""
        device_state = StateHandle()
        alerts: List[Dict[str, Any]] = []
        audit_log: List[str] = []

        device_id = "sensor-lifecycle-001"

        await device_state.set_json(f"device:{device_id}", {
            "device_id": device_id,
            "type": "temperature",
            "firmware": "v1.0.0",
            "status": "active",
            "readings_count": 0,
        })
        audit_log.append(f"REGISTERED: {device_id}")

        for i in range(10):
            temp = random.uniform(-5, 60)
            if temp > 50:
                alerts.append({
                    "device_id": device_id,
                    "type": "temperature",
                    "value": temp,
                    "severity": "critical",
                })
            device = await device_state.get_json(f"device:{device_id}")
            device["readings_count"] = device.get("readings_count", 0) + 1
            await device_state.set_json(f"device:{device_id}", device)
        audit_log.append(f"TELEMETRY: 10 readings processed")

        device = await device_state.get_json(f"device:{device_id}")
        device["firmware"] = "v2.0.0"
        await device_state.set_json(f"device:{device_id}", device)
        audit_log.append(f"FIRMWARE: Updated to v2.0.0")

        health_ok = True
        audit_log.append(f"HEALTH: {'healthy' if health_ok else 'unhealthy'}")

        final_device = await device_state.get_json(f"device:{device_id}")
        assert final_device["firmware"] == "v2.0.0"
        assert final_device["readings_count"] == 10
        assert final_device["status"] == "active"

        print(f"\n=== Full Device Lifecycle Summary ===")
        print(f"  Device: {device_id}")
        print(f"  Firmware: v1.0.0 -> v2.0.0")
        print(f"  Readings processed: {final_device['readings_count']}")
        print(f"  Alerts generated: {len(alerts)}")
        print(f"  Health: healthy")
        print(f"  Audit log:")
        for entry in audit_log:
            print(f"    {entry}")
