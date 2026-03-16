# Mesh Network Architecture

Aether's mesh network provides secure, efficient communication between actors across distributed nodes.

## Overview

The mesh network layer enables:

- **Multi-node communication**: Actors communicate seamlessly across nodes
- **Automatic discovery**: Nodes find each other via gossip protocol
- **Load balancing**: Messages are distributed across available nodes
- **Fault tolerance**: Automatic failover and recovery

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Aether Mesh                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐                │
│  │  Node A │◄──►│  Node B │◄──►│  Node C │                │
│  │ (Actor1)│    │ (Actor2)│    │ (Actor3)│                │
│  │ (Actor4)│    │ (Actor5)│    │ (Actor6)│                │
│  └────┬────┘    └────┬────┘    └────┬────┘                │
│       │              │              │                      │
│       └──────────────┴──────────────┘                      │
│                    QUIC/TLS 1.3                             │
└─────────────────────────────────────────────────────────────┘
```

## QUIC Protocol

Aether uses QUIC as the transport protocol:

### Benefits

1. **Low latency**: 0-RTT connection establishment
2. **Multiplexing**: Multiple streams without head-of-line blocking
3. **Built-in encryption**: TLS 1.3 integrated
4. **Connection migration**: Survives network changes

### Configuration

```toml
[mesh]
# Mesh listen address
listen_addr = "0.0.0.0:7000"

# Bootstrap nodes for discovery
bootstrap_nodes = ["node1.example.com:7000", "node2.example.com:7000"]

# Connection settings
max_connections = 1000
connection_timeout_ms = 5000
idle_timeout_ms = 30000

# Backpressure settings
max_pending_messages = 10000
max_memory_mb = 512
```

## Actor Addressing

Actors are addressed using a hierarchical scheme:

```
<node-id>/<namespace>/<actor-name>

Examples:
- node-abc123/default/hello-actor
- node-abc123/production/payment-processor
- node-def456/staging/ai-assistant
```

### Address Resolution

1. **Local actors**: Resolved directly via actor registry
2. **Remote actors**: Routed through mesh to target node
3. **Broadcast**: Sent to all matching actors

## Message Routing

### Routing Flow

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Sender  │───►│  Router  │───►│   Mesh   │───►│ Receiver │
│  Actor   │    │  Local   │    │  Network │    │  Actor   │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
```

### Routing Rules

1. **Direct routing**: Messages to known actors go directly
2. **Load balancing**: Round-robin for actor pools
3. **Affinity**: Sticky routing for stateful actors
4. **Broadcast**: Fan-out to all matching actors

## Security

### mTLS Configuration

All mesh connections require mutual TLS:

```toml
[mesh.tls]
# Certificate configuration
cert_path = "/etc/aether/certs/node.crt"
key_path = "/etc/aether/certs/node.key"
ca_path = "/etc/aether/certs/ca.crt"

# Certificate settings
min_tls_version = "1.3"
verify_peer = true
```

### Certificate Rotation

Certificates are automatically rotated:

- **Actor certificates**: 24-hour lifetime
- **Node certificates**: 7-day lifetime
- **CA certificates**: 365-day lifetime

## Backpressure

The mesh implements backpressure to prevent overload:

### Flow Control

```rust
// Per-connection flow control
max_concurrent_streams = 100
max_data_per_stream = 1_000_000  // bytes

// Per-node flow control
max_pending_messages = 10_000
max_memory_usage_mb = 512
```

### Backpressure Response

When backpressure is triggered:

1. **Pause reading**: Stop accepting new messages
2. **Buffer existing**: Queue pending messages
3. **Apply timeout**: Drop messages exceeding TTL
4. **Resume**: Continue when capacity available

## Discovery

### Gossip Protocol

Nodes discover each other via gossip:

```
┌─────────────────────────────────────────┐
│            Gossip Protocol              │
├─────────────────────────────────────────┤
│                                         │
│  1. Node joins with bootstrap nodes     │
│  2. Exchange membership lists           │
│  3. Periodic heartbeat (30s)            │
│  4. Detect failures (3 missed beats)    │
│  5. Propagate membership changes        │
│                                         │
└─────────────────────────────────────────┘
```

### Node Metadata

Each node advertises:

```json
{
    "node_id": "node-abc123",
    "address": "10.0.0.1:7000",
    "namespace": ["default", "production"],
    "actor_count": 150,
    "load": 0.75,
    "version": "1.3.0",
    "capabilities": ["ai", "gpu"]
}
```

## Performance

### Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Latency (P50) | < 1ms | Same datacenter |
| Latency (P99) | < 10ms | Cross-region |
| Throughput | 100K msg/s | Per node |
| Connection setup | < 50ms | With TLS |

### Optimization

1. **Connection pooling**: Reuse QUIC connections
2. **Message batching**: Batch small messages
3. **Compression**: LZ4 for large payloads
4. **Zero-copy**: Minimize serialization overhead

## Monitoring

### Metrics

```yaml
# Prometheus metrics
aether_mesh_connections_total
aether_mesh_messages_sent_total
aether_mesh_messages_received_total
aether_mesh_latency_seconds
aether_mesh_backpressure_events_total
aether_mesh_bytes_transmitted_total
```

### Health Checks

```bash
# Check mesh health
aether mesh status

# View connected nodes
aether mesh peers

# Check message queues
aether mesh queues
```

## Troubleshooting

### Common Issues

1. **Connection refused**: Check firewall rules
2. **Certificate errors**: Verify mTLS configuration
3. **High latency**: Check network conditions
4. **Backpressure**: Scale nodes or optimize actors

### Debug Commands

```bash
# Enable debug logging
RUST_LOG=aether::mesh=debug aether run

# Trace specific actor
aether trace actor <actor-name>

# Mesh diagnostics
aether mesh diagnose
```
