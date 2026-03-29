"""Cluster configuration."""
from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class ClusterConfig:
    """Configuration for Aether cluster.
    
    Attributes:
        enabled: Whether clustering is enabled.
        node_id: Unique identifier for this node. Auto-generated if empty.
        seed_nodes: List of seed node addresses (host:port) for bootstrap.
        bind_host: Host address this node advertises to peers.
        gossip_port: Port for gossip protocol.
        gossip_interval_seconds: How often to gossip (seconds).
        failure_timeout_seconds: Time before a silent node is marked suspect.
        dead_timeout_seconds: Time before a suspect node is marked dead.
        suspicion_max: Number of suspicion rounds before declaring dead.
        virtual_nodes: Number of virtual nodes per physical node on hash ring.
        transport: Cross-node message transport ("http" or "grpc").
        cluster_secret: Shared secret for inter-node authentication.
    """
    enabled: bool = False
    node_id: str = ""
    seed_nodes: List[str] = field(default_factory=list)
    bind_host: str = "0.0.0.0"
    gossip_port: int = 7946
    gossip_interval_seconds: float = 1.0
    failure_timeout_seconds: float = 3.0
    dead_timeout_seconds: float = 10.0
    suspicion_max: int = 3
    virtual_nodes: int = 150
    transport: str = "http"
    cluster_secret: str = "aether-cluster-secret-change-me"

    # Migration
    migration_enabled: bool = True
    migration_timeout_seconds: float = 30.0
    migration_drain_timeout_seconds: float = 5.0
    migration_batch_size: int = 10
