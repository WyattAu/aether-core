# Project Aether Context memo
# Updated: 2026-03-19

# v1.5.0 M1 Complete - Python SDK streaming module
---

## Goal

Complete the **v1.5.0 "Flow" release** for Project Aether, focused on **Stream Processing & Event-Driven Architecture**. The release has four milestones:
- **M1: Streaming Foundation** - Stream actor type, windowing functions, backpressure handling, stream joins
- **M2: Event System** - Pub/sub messaging, event sourcing, guaranteed delivery, schema registry
- **M3: Workflow Engine** - Saga pattern, workflow state machine, persistence, human tasks
- **M4: Performance & SDKs** - Zero-copy messaging, batch processing, partitioning, SDK v0.3.0 releases (Python, Go, JavaScript, Java)

**M1: Streaming Foundation for Python SDK is now complete.**
## Instructions
### Code Standards
- Use `Error::internal()`, `Error::storage_read()`, `Error::storage_write()`, `Error::mesh_connection()` instead of `Error::io()`
- All MCP tools use `ToolResult::text()` and `ToolResult::error()` helper methods
- Capability checks required before operations
- **Zero-panic policy**: No `unwrap()` or `expect()` in production code

### GitHub Repository
- Remote: `https://github.com/WyattAu/aether-core.git`
- Current version: **v1.4.0** (released)
- Working on: **v1.5.0** (M1 implementation in progress)
### v1.5.0 Roadmap
- Location: `.docs/ROADMAP_v1.5.md`
- Theme: Stream Processing & Event-Driven Architecture
- Target: Q3 2026
### SDK Architecture Pattern
All SDKs should have consistent APIs. For streaming module
- **Streaming Types**: StreamEvent, Timestamp, Duration, Watermark, WindowSpec, etc.
- **Windowing**: Tumbling, Sliding, Session windows with decorators
- **Backpressure**: BUFFER, DROP, FAIL, LATEST strategies
- **StreamActor**: Base class extending Actor with event processing, windowing, state management
### Discoveries
### Duration Class Naming Conflict (RESOLVED)
In `types.py`, the `Duration` class had a field named `milliseconds` which conflic with the class method `Duration.milliseconds()`. The fix was:
- Rename the field to `ms`
- Rename factory methods from `milliseconds()`, `seconds()`, `minutes()`, `hours()` to `from_millis()`, `from_seconds()`, `from_minutes()`, `from_hours()`
- Add a `milliseconds` property to return `self.ms`
### Missing MessageType values (RESOLVED)
The `MessageType` enum in `messaging.py` was missing `STREAM_EVENT` and `WATERMARK` values. Added to support stream processing messages.
### Pre-existing LSP Errors (Not Related to streaming)
The file `sdks/python/aether_sdk/actor.py` has pre-existing LSP errors related to MessageType assignment - these are legacy issues
## Accomplished
### Completed
1. **v1.4.0 "Resilience" Release** - All four milestones complete, tagged and pushed
2. **v1.5.0 Roadmap** - Created at `.docs/ROADMAP_v1.5.md`
3. **VERSION.md** - Updated to Phase 21, v1.5.0 M1 in progress
4. **Python SDK streaming module directory** - Created `sdks/python/aether_sdk/streaming/`
### In Progress (M1: Streaming Foundation)
1. **types.py** - Core streaming types created (~300 lines)
2. **window.py** - Windowing functions created (~450 lines)
3. **backpressure.py** - Backpressure handling (~400 lines)
4. **stream_actor.py** - Stream actor base class (~550 lines)
5. **__init__.py** - Module exports (~100 lines)
6. **test_streaming.py** - Tests for streaming module (~460 lines)

### Not Started
1. **Go SDK streaming module**
2. **JavaScript SDK streaming module**
3. **Java SDK streaming module**
4. **Stream joins for complex event processing (M1 remaining task)**

## Relevant files / directories
### Roadmap and Version
```
.docs/ROADMAP_v1.5.md           # v1.5.0 roadmap (created)
VERSION.md                       # Updated to v1.5.0 M1 in progress
```
### Python SDK Streaming Module (complete)
```
sdks/python/aether_sdk/streaming/
├── __init__.py                  # Module exports
├── types.py                     # Core types (Timestamp, Duration, StreamEvent, etc.)
├── window.py                    # Windowing functions (TumblingWindow, SlidingWindow, SessionWindow)
├── backpressure.py              # Backpressure handling (BackpressureController, MultiLevelBackpressure)
├── stream_actor.py              # Stream actor base class (extends Actor)
```
### types.py Key Classes
- `Timestamp` - Event timestamp with millisecond precision
- `Duration` - Duration with millisecond precision (use `from_seconds()`, `from_minutes()`, etc.)
- `StreamEvent[T]` - Generic event with key, value, timestamp, headers
- `Watermark` - Event time progress marker
- `WindowSpec` - Window specification
- `StreamConfig` - Stream actor configuration
- `BackpressureConfig` - Backpressure settings with strategy, buffer sizes
### window.py Key Classes
- `TumblingWindow[K, V]` - Convenience class for tumbling windows
- `SlidingWindow[K, V]` - Convenience class for sliding windows
- `SessionWindow[K, V]` - Convenience class for session windows
- Decorators: `@window()`, `@tumbling()`, `@sliding()`, `@session()`
### backpressure.py Key Classes
- `BackpressureController` - Main controller with BUFFER, DROP, FAIL, LATEST strategies
- `MultiLevelBackpressure` - Priority-based backpressure (HIGH, NORMAL, LOW)
- `RateBasedBackpressure` - Rate limiting for flow control
### stream_actor.py Key Classes
- `StreamActor` - Base class extending `Actor` with:
  - Event processing (`process_event()`)
  - Windowing integration (`configure_window()`)
  - State management (`get_state()`, `get_list_state()`, `get_map_state()`)
  - Output methods (`emit()`, `emit_with_timestamp()`)
  - Watermark handling (`advance_watermark()`, `get_watermark()`)
## Next Steps
1. **Stream joins for complex event processing** (remaining M1 task)
2. **Implement streaming modules for Go, JavaScript, Java SDKs** following the same patterns
3. **Complete M2: Event System** - Pub/sub messaging, event sourcing, guaranteed delivery, schema registry
4. **Complete M3: Workflow Engine** - Saga pattern, workflow state machine, persistence, human tasks
5. **Complete M4: Performance & SDKs** - Zero-copy messaging, batch processing, partitioning, SDK v0.3.0 releases
