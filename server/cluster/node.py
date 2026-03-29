"""Cluster node representation."""
import time
import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, Optional


class NodeStatus(str, Enum):
    """Health status of a cluster node."""
    ALIVE = "alive"
    SUSPECT = "suspect"
    DEAD = "dead"
    LEAVING = "leaving"
    JOINING = "joining"


@dataclass
class ClusterNode:
    """Represents a node in the Aether cluster.
    
    Attributes:
        node_id: Unique identifier for this node (UUID).
        host: Hostname or IP address.
        gossip_port: Port for gossip protocol communication.
        api_port: Port for REST API / gRPC.
        status: Current health status.
        metadata: Optional key-value metadata (region, zone, labels).
        actor_count: Number of actors currently hosted.
        last_heartbeat: Timestamp of last received heartbeat.
        incarnation: Monotonically increasing number to resolve conflicts.
        joined_at: Timestamp when this node joined the cluster.
    """
    node_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    host: str = "localhost"
    gossip_port: int = 7946
    api_port: int = 8080
    status: NodeStatus = NodeStatus.JOINING
    metadata: Dict[str, str] = field(default_factory=dict)
    actor_count: int = 0
    last_heartbeat: float = field(default_factory=time.time)
    incarnation: int = 0
    joined_at: float = field(default_factory=time.time)
    stepped_down: bool = False  # True if node voluntarily stepped down from leader
    
    @property
    def gossip_address(self) -> str:
        """Address for gossip communication: host:gossip_port."""
        return f"{self.host}:{self.gossip_port}"
    
    @property
    def api_address(self) -> str:
        """Address for API communication: host:api_port."""
        return f"{self.host}:{self.api_port}"
    
    def touch(self) -> None:
        """Update last_heartbeat to now."""
        self.last_heartbeat = time.time()
    
    def is_alive(self) -> bool:
        """Check if this node is considered alive."""
        return self.status in (NodeStatus.ALIVE, NodeStatus.JOINING)
    
    def to_dict(self) -> dict:
        """Serialize to dictionary for gossip transport."""
        return {
            "node_id": self.node_id,
            "host": self.host,
            "gossip_port": self.gossip_port,
            "api_port": self.api_port,
            "status": self.status.value,
            "metadata": self.metadata,
            "actor_count": self.actor_count,
            "last_heartbeat": self.last_heartbeat,
            "incarnation": self.incarnation,
            "joined_at": self.joined_at,
            "stepped_down": self.stepped_down,
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> "ClusterNode":
        """Deserialize from dictionary."""
        data = dict(data)  # copy
        data["status"] = NodeStatus(data.get("status", "alive"))
        return cls(**data)
    
    def __hash__(self) -> int:
        return hash(self.node_id)
    
    def __eq__(self, other) -> bool:
        if isinstance(other, ClusterNode):
            return self.node_id == other.node_id
        return NotImplemented
