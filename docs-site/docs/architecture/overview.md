# Architecture Overview

Understanding the Aether system architecture.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Client Layer                                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐               │
│  │   CLI    │  │   SDK    │  │  HTTP    │  │  gRPC    │               │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘               │
└───────┼─────────────┼─────────────┼─────────────┼──────────────────────┘
        │             │             │             │
┌───────┴─────────────┴─────────────┴─────────────┴──────────────────────┐
│                           API Gateway                                   │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Authentication │ Rate Limiting │ Routing │ Load Balancing       │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
┌───────────────────────────────────┴─────────────────────────────────────┐
│                         Actor Runtime                                   │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐            │
│  │Scheduler│  │  Mailbox  │  │Supervisor │  │ Registry  │            │
│  └─────────┘  └───────────┘  └───────────┘  └───────────┘            │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐            │
│  │Executor │  │  Context  │  │  Metrics  │  │  Tracing  │            │
│  └─────────┘  └───────────┘  └───────────┘  └───────────┘            │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
┌───────────────────────────────────┴─────────────────────────────────────┐
│                          Core Services                                  │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐            │
│  │  State  │  │ Security  │  │   Mesh    │  │    AI     │            │
│  └─────────┘  └───────────┘  └───────────┘  └───────────┘            │
│  ┌─────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐            │
│  │   MCP   │  │   WASM    │  │  Storage  │  │  Secrets  │            │
│  └─────────┘  └───────────┘  └───────────┘  └───────────┘            │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
┌───────────────────────────────────┴─────────────────────────────────────┐
│                        Infrastructure Layer                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                    │
│  │ Firecracker │  │   Network   │  │  Observability│                   │
│  │   (VMs)     │  │   (QUIC)    │  │  (OTLP)      │                   │
│  └─────────────┘  └─────────────┘  └─────────────┘                    │
└─────────────────────────────────────────────────────────────────────────┘
```

## Components

### Actor Runtime

The core execution engine for actors.

| Component | Description |
|-----------|-------------|
| Scheduler | Distributes actors across worker threads |
| Mailbox | Message queue for each actor |
| Supervisor | Manages actor lifecycle and failure recovery |
| Registry | Tracks actor locations and metadata |
| Executor | Runs actor message handlers |
| Context | Provides actor environment and capabilities |

### Core Services

| Service | Description |
|---------|-------------|
| State | Persistent key-value storage for actors |
| Security | Capability-based access control |
| Mesh | Distributed actor communication |
| AI | LLM and inference integration |
| MCP | Model Context Protocol server |
| WASM | WebAssembly runtime for actors |
| Storage | Pluggable storage backends |
| Secrets | Secure secret management |

### Infrastructure

| Layer | Description |
|-------|-------------|
| Firecracker | MicroVM isolation for actors |
| Network | QUIC-based mesh networking |
| Observability | OpenTelemetry tracing and metrics |

## Data Flow

### Message Processing

```
Client Request
      │
      ▼
┌──────────┐
│ API Gate │
└────┬─────┘
     │
     ▼
┌──────────┐     ┌──────────┐
│  Router  │────▶│ Registry │ (Lookup target actor)
└────┬─────┘     └──────────┘
     │
     ▼
┌──────────┐
│ Mailbox  │ (Queue message)
└────┬─────┘
     │
     ▼
┌──────────┐
│Executor  │ (Process message)
└────┬─────┘
     │
     ▼
  Response
```

### State Access

```
Actor Request
      │
      ▼
┌──────────┐
│Capability│ (Check permissions)
│  Check   │
└────┬─────┘
     │
     ▼
┌──────────┐
│  State   │ (Read/Write)
│ Backend  │
└────┬─────┘
     │
     ▼
┌──────────┐
│ Storage  │ (Persist)
└──────────┘
```

## Deployment Topologies

### Single Node

```
┌──────────────────────────────┐
│          Single Node          │
│  ┌────────┐  ┌────────┐      │
│  │ Actor1 │  │ Actor2 │      │
│  └────────┘  └────────┘      │
│  ┌────────┐  ┌────────┐      │
│  │ Actor3 │  │ Actor4 │      │
│  └────────┘  └────────┘      │
│         Runtime              │
└──────────────────────────────┘
```

### Multi-Node Mesh

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  Node A  │────▶│  Node B  │────▶│  Node C  │
│ (Leader) │◀────│(Follower)│◀────│(Follower)│
└──────────┘     └──────────┘     └──────────┘
     │                │                │
     └────────────────┴────────────────┘
                   Mesh Network
```

### Regional Deployment

```
        ┌─────────────────┐
        │  Global Router  │
        └────────┬────────┘
         ┌───────┼───────┐
         │       │       │
    ┌────▼───┐ ┌─▼────┐ ┌─▼────┐
    │ US-East│ │EU-West│ │AP-East│
    │ Region │ │Region │ │Region │
    └────────┘ └───────┘ └───────┘
```

## Security Model

### Capability System

```
┌─────────────────────────────────────────┐
│              Actor                       │
│  ┌────────────────────────────────────┐ │
│  │         Capability Set              │ │
│  │  • STATE_READ                       │ │
│  │  • STATE_WRITE                      │ │
│  │  • NETWORK_OUTBOUND                 │ │
│  └────────────────────────────────────┘ │
│                   │                      │
│                   ▼                      │
│  ┌────────────────────────────────────┐ │
│  │          Resource Access            │ │
│  │  ✓ State Storage                   │ │
│  │  ✓ External APIs                   │ │
│  │  ✗ File System                     │ │
│  │  ✗ Process Spawn                   │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

### mTLS Mesh Communication

```
┌──────────┐                    ┌──────────┐
│  Node A  │                    │  Node B  │
│ ┌──────┐ │   mTLS Tunnel      │ ┌──────┐ │
│ │Cert A│◀├────────────────────▶┤│Cert B│ │
│ └──────┘ │   Encrypted         │ └──────┘ │
└──────────┘                    └──────────┘
```

## Next Steps

- [Actor Model](actor-model.md) - Deep dive into actors
- [Mesh Network](mesh.md) - Distributed communication
- [Security](security.md) - Security architecture
