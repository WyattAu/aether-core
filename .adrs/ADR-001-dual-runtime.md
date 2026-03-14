# ADR-001: Dual Runtime Architecture

## Status

**Accepted** - 2026-03-05

## Context

Project Aether requires both high-performance data plane processing and reliable control plane operations. The system must handle:

1. **Data Plane Requirements**:
   - Sub-microsecond I/O latency
   - Millions of concurrent connections
   - Zero-copy data paths
   - Thread-per-core scalability

2. **Control Plane Requirements**:
   - Rich ecosystem integration
   - Complex orchestration logic
   - Database client compatibility
   - Standard async/await patterns

No single async runtime optimally addresses both requirements:
- **Tokio**: Excellent ecosystem, but work-stealing adds overhead for data plane
- **Monoio**: Excellent performance, but limited ecosystem for control plane

## Decision

We adopt a **dual runtime architecture**:

### Data Plane: Monoio
- Used for: I/O-intensive operations, network stack, serialization
- Characteristics: Thread-per-core, io_uring-native, zero-copy
- Components: Network Mesh, State Manager I/O, Actor message routing

### Control Plane: Tokio
- Used for: Orchestration, configuration, external integrations
- Characteristics: Work-stealing, rich ecosystem, proven reliability
- Components: Host Runtime orchestration, FDB client, service discovery

### Boundary Rules
1. **No cross-runtime async calls**: Communication via channels only
2. **Clear ownership**: Each resource owned by one runtime
3. **Explicit handoffs**: Data transfers at runtime boundaries are explicit
4. **No mixed await**: Never hold await points across runtimes

## Implementation

```rust
struct HostRuntime {
    monoio_handle: monoio::Runtime,
    tokio_handle: tokio::runtime::Handle,
    data_plane_tx: mpsc::Sender<DataPlaneCommand>,
    control_plane_tx: mpsc::Sender<ControlPlaneCommand>,
}

impl HostRuntime {
    fn spawn_data_plane<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.monoio_handle.spawn(fut);
    }
    
    fn spawn_control_plane<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tokio_handle.spawn(fut);
    }
}
```

## Consequences

### Positive
- **Optimal performance**: Each runtime optimized for its workload
- **Clear separation**: Data plane vs control plane boundaries explicit
- **Ecosystem access**: Can use Tokio ecosystem for control plane
- **Scalability**: Data plane scales linearly with cores
- **Isolation**: Control plane bugs don't impact data plane performance

### Negative
- **Complexity**: Two runtimes to understand and debug
- **Communication overhead**: Cross-runtime channel hops
- **Resource duplication**: Two thread pools, two reactors
- **Learning curve**: Developers must understand both models
- **Testing complexity**: Must test cross-runtime interactions

### Neutral
- **Memory usage**: Slightly higher due to duplicate infrastructure
- **Binary size**: Both runtime libraries included

## Alternatives Considered

### 1. Tokio Only
- **Pros**: Simpler architecture, single runtime
- **Cons**: Work-stealing overhead unsuitable for data plane, higher tail latency
- **Rejected**: Performance requirements too strict

### 2. Monoio Only
- **Pros**: Maximum performance, simpler architecture
- **Cons**: Poor ecosystem support, many libraries incompatible
- **Rejected**: Control plane integration too difficult

### 3. Glommio
- **Pros**: Thread-per-core, good performance
- **Cons**: Less mature than Monoio, smaller community
- **Rejected**: Monoio better aligned with our needs

### 4. Custom Runtime
- **Pros**: Optimized for exact requirements
- **Cons**: Massive development effort, unproven
- **Rejected**: Not feasible with timeline

## References

- [Monoio Documentation](https://github.com/bytedance/monoio)
- [Tokio Documentation](https://tokio.rs/)
- [Thread-Per-Core Architecture](https://library.fivehq.com/thread-per-core)
- YP-ASYNC-IOURING-001: Async I/O Yellow Paper
- BP-HOST-RUNTIME-001: Host Runtime Blue Paper

## Notes

- Revisit if Tokio adds thread-per-core mode
- Revisit if Monoio ecosystem matures significantly
- Monitor performance metrics across runtime boundary
