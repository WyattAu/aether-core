# Cross-SDK Contract Specification

This document defines the contract between the Python and JavaScript Aether SDKs.
Both SDKs MUST conform to these specifications to ensure interoperability.

## Message JSON Format

### Python Message JSON

```json
{
  "type": "<message_type_value>",
  "payload": { ... },
  "sender": "<actor_name>" | null,
  "correlation_id": "<id>" | null
}
```

- `type`: string value from the MessageType enum (see below)
- `payload`: arbitrary JSON-serializable value
- `sender`: actor name string or `null`
- `correlation_id`: string or `null`

### JavaScript Message JSON

```json
{
  "type": "<message_type_value>",
  "payload": { ... },
  "sender": "<actor_name>" | undefined,
  "correlationId": "<id>" | undefined,
  "priority": 1
}
```

- `type`: string value from the MessageType enum
- `payload`: arbitrary JSON-serializable value
- `sender`: actor name string or `undefined` (JSON: omitted)
- `correlationId`: string or `undefined` (JSON: omitted)
- `priority`: integer from Priority enum (0=LOW, 1=NORMAL, 2=HIGH, 3=CRITICAL)

### Cross-SDK Compatibility Notes

| Field | Python | JavaScript | JSON Interop |
|-------|--------|------------|--------------|
| type | `"type"` | `"type"` | Direct match |
| payload | `"payload"` | `"payload"` | Direct match |
| sender | `"sender"` | `"sender"` | Direct match; null vs undefined |
| correlation_id | `"correlation_id"` | `"correlationId"` | **Different key names** |
| priority | N/A | `"priority"` | JS-only field |

When consuming Python JSON in JS or vice versa, the `correlation_id` / `correlationId`
key difference MUST be handled by the consumer.

## Message Type Values

| Python Enum | JS Enum | String Value |
|-------------|---------|--------------|
| `MessageType.START` | `MessageType.START` | `"start"` |
| `MessageType.STOP` | `MessageType.STOP` | `"stop"` |
| `MessageType.SIGNAL` | `MessageType.SIGNAL` | `"signal"` |
| `MessageType.RPC_REQUEST` | `MessageType.RPC_REQUEST` | `"rpc_request"` |
| `MessageType.RPC_RESPONSE` | `MessageType.RPC_RESPONSE` | `"rpc_response"` |
| `MessageType.CUSTOM` | `MessageType.CUSTOM` | `"custom"` |
| `MessageType.STREAM_EVENT` | N/A | `"stream_event"` |
| `MessageType.WATERMARK` | N/A | `"watermark"` |
| `MessageType.CHECKPOINT` | N/A | `"checkpoint"` |
| `MessageType.CHECKPOINT_ACK` | N/A | `"checkpoint_ack"` |

**Common types** (present in both SDKs): `start`, `stop`, `signal`, `rpc_request`, `rpc_response`, `custom`.

**Python-only types**: `stream_event`, `watermark`, `checkpoint`, `checkpoint_ack`.

## Timestamp Format

Both SDKs represent timestamps as **milliseconds since Unix epoch** (integer).

| Python | JavaScript |
|--------|------------|
| `Timestamp.milliseconds: int` | `Timestamp.milliseconds: number` |
| `Timestamp.now()` | `Timestamp.now()` |
| `Timestamp.from_seconds(s)` | `Timestamp.fromSeconds(s)` |
| `ts.to_seconds()` | `ts.toSeconds()` |
| `ts + duration` | `ts.add(duration)` |
| `ts - other_ts` | `ts.subtract(other)` |

### JSON Serialization

- Python: No built-in `toJSON` on Timestamp; serialize as `{"milliseconds": <int>}`
- JavaScript: `Timestamp.toJSON()` returns a raw `number` (millisecond value)

For cross-SDK exchange, the wire format is a plain integer (milliseconds).

## Duration Format

Both SDKs represent durations as **milliseconds** (integer).

| Python | JavaScript |
|--------|------------|
| `Duration.ms: int` | `Duration.milliseconds: number` |
| `Duration.from_millis(ms)` | `Duration.fromMillis(ms)` |
| `Duration.from_seconds(s)` | `Duration.fromSeconds(s)` |
| `Duration.from_minutes(m)` | `Duration.fromMinutes(m)` |
| `Duration.from_hours(h)` | `Duration.fromHours(h)` |
| `d.to_seconds()` | `d.toSeconds()` |
| `d.to_millis()` | `d.toMillis()` |
| `d + other` | `d.add(other)` |
| `d * factor` | `d.multiply(factor)` |

For cross-SDK exchange, the wire format is a plain integer (milliseconds).

## Capability Enum Values

Python uses `Flag` with `auto()` values (integers starting from 1).
JavaScript uses explicit bit-flag values `1 << n`.

| Capability | Python (auto) | JavaScript |
|------------|---------------|------------|
| `NETWORK_OUTBOUND` | 1 | `1 << 0` (1) |
| `NETWORK_INBOUND` | 2 | `1 << 1` (2) |
| `STATE_READ` | 4 | `1 << 2` (4) |
| `STATE_WRITE` | 8 | `1 << 3` (8) |
| `FS_READ` | 16 | `1 << 4` (16) |
| `FS_WRITE` | 32 | `1 << 5` (32) |
| `ACTOR_MESSAGING` | 64 | `1 << 6` (64) |
| `LOG` | 128 | `1 << 7` (128) |
| `TIME` | 256 | `1 << 8` (256) |
| `RANDOM` | 512 | `1 << 9` (512) |
| `ENVIRONMENT` | 1024 | `1 << 10` (1024) |
| `HTTP_CLIENT` | 2048 | `1 << 11` (2048) |
| `HTTP_SERVER` | 4096 | `1 << 12` (4096) |

Both SDKs use the same 13 capabilities with matching bit positions. The numeric
values are identical. CapabilitySets can be exchanged as integer bitmasks.

## Streaming Enum Values

### WindowType

| Python | JavaScript |
|--------|------------|
| `WindowType.TUMBLING` (auto=1) | `WindowType.Tumbling` (`"tumbling"`) |
| `WindowType.SLIDING` (auto=2) | `WindowType.Sliding` (`"sliding"`) |
| `WindowType.SESSION` (auto=3) | `WindowType.Session` (`"session"`) |

**Note**: Python uses auto-increment integers; JavaScript uses string values.
For cross-SDK exchange, use string names: `"tumbling"`, `"sliding"`, `"session"`.

### LateDataPolicy

| Python | JavaScript |
|--------|------------|
| `LateDataPolicy.DROP` | `LateDataPolicy.Drop` (`"drop"`) |
| `LateDataPolicy.SIDE_OUTPUT` | `LateDataPolicy.SideOutput` (`"side-output"`) |
| `LateDataPolicy.REPROCESS` | `LateDataPolicy.Reprocess` (`"reprocess"`) |

### WatermarkStrategy

| Python | JavaScript |
|--------|------------|
| `WatermarkStrategy.EVENT_TIME` | `WatermarkStrategy.EventTime` (`"event-time"`) |
| `WatermarkStrategy.PROCESSING_TIME` | `WatermarkStrategy.ProcessingTime` (`"processing-time"`) |
| `WatermarkStrategy.BOUNDED_OUT_OF_ORDER` | `WatermarkStrategy.BoundedOutOfOrder` (`"bounded-out-of-order"`) |

### DeliverySemantics

| Python | JavaScript |
|--------|------------|
| `DeliverySemantics.AT_MOST_ONCE` | `DeliverySemantics.AtMostOnce` (`"at-most-once"`) |
| `DeliverySemantics.AT_LEAST_ONCE` | `DeliverySemantics.AtLeastOnce` (`"at-least-once"`) |
| `DeliverySemantics.EXACTLY_ONCE` | `DeliverySemantics.ExactlyOnce` (`"exactly-once"`) |

### PaneInfo

| Python | JavaScript |
|--------|------------|
| `PaneInfo.EARLY` | `PaneInfo.Early` (`"early"`) |
| `PaneInfo.ON_TIME` | `PaneInfo.OnTime` (`"on-time"`) |
| `PaneInfo.LATE` | `PaneInfo.Late` (`"late"`) |

## State Key Naming Conventions

State keys used across SDKs SHOULD follow these conventions:
- Use `snake_case` for state keys (Python convention)
- Both SDKs accept arbitrary string keys
- No prefix or namespace is required by the framework

## Error Type Naming Conventions

| Python | JavaScript |
|--------|------------|
| `AetherError` | Base error class |
| `ActorError` | `ActorError` |
| `StateError` | `StateError` |
| `TimeoutError` | `TimeoutError` |
| `ValidationError` | `ValidationError` |

Error names follow PascalCase in both SDKs. Error messages are human-readable strings.
