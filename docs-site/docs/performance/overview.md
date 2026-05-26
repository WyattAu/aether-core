# Performance Tuning Guide

> **Note:** Performance tuning examples are being migrated from Go to Rust as part of the v2.0.0 rewrite. The principles apply universally but code samples will be updated to Rust in a future release.

Optimize Aether for maximum performance.

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Actor cold start P99 | < 50µs | Time to first message |
| Actor warm start P99 | < 10µs | Time to first message (pooled) |
| Message latency P99 (local) | < 10µs | End-to-end delivery |
| Message latency P99 (remote) | < 1ms | Cross-node delivery |
| Throughput (local) | > 1M msg/s | Messages per second |
| Throughput (remote) | > 100K msg/s | Cross-node messages |
| Memory per actor | < 2KB | Resident memory |
| CPU overhead | < 5% | Idle system |

## Benchmarking

### Built-in Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench -- actor_spawn

# Run with profiling
cargo bench -- --profile-time=5
```

### Custom Benchmarks

```go
// Go SDK benchmark
func BenchmarkActorMessage(b *testing.B) {
    actor := NewTestActor()
    ctx := context.Background()
    
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        actor.HandleMessage(ctx, "sender", &aether.Message{
            Type:    aether.MessageTypeRequest,
            Payload: "test",
        })
    }
}
```

## Tuning Parameters

### Scheduler Configuration

```toml
[scheduler]
# Number of worker threads (default: CPU cores)
workers = 8

# Work stealing enabled
work_stealing = true

# Actor queue size per worker
queue_size = 1024

# Batch size for message processing
batch_size = 32
```

### Mailbox Configuration

```toml
[mailbox]
# Default mailbox capacity
default_capacity = 1000

# Overflow policy: drop_oldest, drop_newest, block
overflow_policy = "drop_oldest"

# Priority queues enabled
priority_queues = true

# Number of priority levels
priority_levels = 4
```

### Network Configuration

```toml
[network]
# Connection pool size
connection_pool_size = 100

# Connection timeout
connect_timeout = "5s"

# Keep-alive interval
keepalive_interval = "30s"

# QUIC specific
[network.quic]
max_concurrent_streams = 1000
initial_mtu = 1200
send_buffer_size = 1048576
recv_buffer_size = 1048576
```

## Actor-Level Optimization

### Message Processing

```go
// Good: Process messages quickly
func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    // Fast path for common case
    if msg.Type == aether.MessageTypeRequest {
        return a.handleRequest(msg), nil
    }
    
    // Slower path for other types
    return a.handleOther(ctx, msg)
}

// Bad: Blocking operations in handler
func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    // Don't do this!
    time.Sleep(100 * time.Millisecond)
    
    // Don't do this either!
    resp, _ := http.Get("http://slow-service/api")
    
    return response, nil
}
```

### State Access Patterns

```go
// Good: Batch state operations
func (a *MyActor) OnStart(ctx context.Context) error {
    // Load all needed state at once
    keys := []string{"config", "cache", "metadata"}
    for _, key := range keys {
        data, _ := a.State().Read(ctx, key)
        a.cache[key] = data
    }
    return nil
}

// Good: Cache in memory, persist on changes
func (a *MyActor) updateValue(key string, value []byte) {
    a.cache[key] = value
    a.pendingWrites = append(a.pendingWrites, key)
}

func (a *MyActor) flushWrites(ctx context.Context) error {
    for _, key := range a.pendingWrites {
        a.State().Write(ctx, key, a.cache[key])
    }
    a.pendingWrites = a.pendingWrites[:0]
    return nil
}
```

### Actor Pooling

```go
// Pool actors for high-throughput scenarios
type ActorPool struct {
    actors    []*WorkerActor
    current   int
    mu        sync.Mutex
}

func (p *ActorPool) Process(ctx context.Context, msg *aether.Message) (*aether.Message, error) {
    p.mu.Lock()
    actor := p.actors[p.current]
    p.current = (p.current + 1) % len(p.actors)
    p.mu.Unlock()
    
    return actor.HandleMessage(ctx, "pool", msg)
}
```

## Memory Optimization

### Actor Size

```go
// Good: Minimal state
type LeanActor struct {
    *aether.BaseActor
    count int64  // 8 bytes
}

// Bad: Excessive state
type BloatedActor struct {
    *aether.BaseActor
    largeSlice   []byte    // Potentially large
    bigMap       map[string][]byte
    cachedData   [][]byte
}
```

### Object Pooling

```go
var messagePool = sync.Pool{
    New: func() any {
        return &aether.Message{
            Metadata: make(map[string]string, 4),
        }
    },
}

func getMessage() *aether.Message {
    return messagePool.Get().(*aether.Message)
}

func putMessage(msg *aether.Message) {
    // Reset before returning to pool
    msg.Type = ""
    msg.Payload = nil
    msg.Sender = ""
    for k := range msg.Metadata {
        delete(msg.Metadata, k)
    }
    messagePool.Put(msg)
}
```

## CPU Optimization

### Avoid Allocations

```go
// Good: Reuse buffers
var bufPool = sync.Pool{
    New: func() any { return make([]byte, 1024) },
}

func (a *MyActor) process() []byte {
    buf := bufPool.Get().([]byte)
    defer bufPool.Put(buf)
    
    // Use buffer...
    return append([]byte{}, buf[:n]...)
}

// Bad: Frequent allocations
func (a *MyActor) process() []byte {
    buf := make([]byte, 1024)  // Allocated every call
    // ...
    return buf
}
```

### Use Efficient Data Structures

```go
// Good: Map for O(1) lookups
type FastActor struct {
    handlers map[string]HandlerFunc
}

// Bad: Slice with O(n) lookups
type SlowActor struct {
    handlers []struct {
        name string
        fn   HandlerFunc
    }
}
```

## Network Optimization

### Connection Pooling

```toml
[network.pooling]
enabled = true
max_idle_connections = 100
max_idle_time = "90s"
```

### Compression

```toml
[network.compression]
enabled = true
algorithm = "zstd"  # or "gzip", "lz4"
min_size = 1024     # Only compress messages > 1KB
```

### Batching

```go
// Batch multiple messages into one network packet
type BatchSender struct {
    batch     []*aether.Message
    batchSize int
    interval  time.Duration
}

func (s *BatchSender) Send(msg *aether.Message) {
    s.batch = append(s.batch, msg)
    if len(s.batch) >= s.batchSize {
        s.flush()
    }
}
```

## Monitoring Performance

### Key Metrics

```yaml
# Prometheus metrics
- aether_actor_spawn_duration_seconds
- aether_actor_message_processing_seconds
- aether_actor_mailbox_size
- aether_mesh_message_latency_seconds
- aether_mesh_throughput_messages_total
```

### Profiling

```bash
# CPU profile
curl http://localhost:9090/debug/pprof/profile?seconds=30 > cpu.prof

# Memory profile
curl http://localhost:9090/debug/pprof/heap > heap.prof

# Goroutine dump
curl http://localhost:9090/debug/pprof/goroutine > goroutine.prof

# Analyze
go tool pprof cpu.prof
go tool pprof heap.prof
```

## Performance Checklist

- [ ] Set appropriate scheduler worker count
- [ ] Configure mailbox sizes for your workload
- [ ] Enable connection pooling
- [ ] Use actor pooling for high throughput
- [ ] Minimize allocations in hot paths
- [ ] Batch state operations
- [ ] Enable compression for large messages
- [ ] Monitor key metrics
- [ ] Profile before optimizing
