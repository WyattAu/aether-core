"""Cluster management REST API endpoints.

Public endpoints (for operators):
- GET  /cluster/info      - Cluster state summary
- GET  /cluster/nodes     - List all cluster members
- GET  /cluster/nodes/{id} - Get specific node details
- GET  /cluster/ring      - Hash ring distribution stats
- GET  /cluster/router-stats - Cluster router statistics
- GET  /cluster/pubsub-stats - Cluster pub/sub statistics
- GET  /cluster/leader     - Leader election status
- POST /cluster/leader/step-down - Voluntary leader step-down
- POST /cluster/leader/force - Force a specific node to be leader (admin)
- GET  /cluster/migration/status - Migration coordinator status
- GET  /cluster/migration/stats - Migration statistics
- POST /cluster/migration/rebalance - Trigger manual rebalance (leader)

Internal endpoints (for inter-node communication):
- POST /cluster/internal/ping      - Respond to gossip ping
- POST /cluster/internal/ping-req  - Probe a suspect node on behalf of another
- POST /cluster/internal/sync      - Full membership state exchange
- POST /cluster/internal/message   - Receive a forwarded message
- POST /cluster/internal/pubsub/publish - Receive fanned-out pub/sub message
- POST /cluster/internal/migrate/receive - Receive actor migration
"""

import logging

from fastapi import APIRouter, Request
from pydantic import BaseModel

logger = logging.getLogger("aether-server.cluster.api")

router = APIRouter(tags=["cluster"])


class PingRequest(BaseModel):
    node: dict
    target: dict = None


class SyncRequest(BaseModel):
    nodes: dict


class ForwardedMessage(BaseModel):
    source_actor: str
    target_actor: str
    message_type: str = "default"
    payload: object = None
    correlation_id: str = None
    timestamp: str = None
    priority: int = 0
    message_id: str = None


class RemotePublishRequest(BaseModel):
    topic: str
    payload: object = None
    headers: dict = {}
    source_node_id: str = None
    message_id: str = None
    timestamp: str = None


class MigrationReceiveRequest(BaseModel):
    actor_id: str
    actor_type: str = "default"
    state: dict = {}
    persistent_state: dict = {}
    supervision_strategy: str = "restart"
    parent_id: str = None
    children: list = []
    pending_messages: list = []
    source_node: str = None
    migration_timeout: float = 30.0


@router.post("/internal/ping")
async def handle_ping(request: Request, body: PingRequest):
    """Handle gossip ping from a remote node."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    return membership.handle_ping(body.node)


@router.post("/internal/ping-req")
async def handle_ping_request(request: Request, body: PingRequest):
    """Handle ping-req - probe a suspect on behalf of another node."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    if body.target is None:
        return {"error": "target_required"}
    return membership.handle_ping_request(body.node, body.target)


@router.post("/internal/sync")
async def handle_sync(request: Request, body: SyncRequest):
    """Handle full membership state sync."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    return membership.handle_sync(body.nodes)


@router.post("/internal/message")
async def handle_forwarded_message(request: Request, body: ForwardedMessage):
    """Receive a message forwarded from another cluster node.

    This endpoint is called by remote nodes when they determine
    that a message's target actor lives on this node.
    """
    from ..models import DeliveryReceipt, MessageEnvelope

    cluster_router = _get_cluster_router(request)
    if cluster_router is None:
        return {"status": "error", "detail": "Clustering not enabled"}

    envelope = MessageEnvelope(
        source_actor=body.source_actor,
        target_actor=body.target_actor,
        message_type=body.message_type,
        payload=body.payload,
        correlation_id=body.correlation_id,
        priority=body.priority,
        message_id=body.message_id,
    )

    receipt = await cluster_router.route(envelope)
    return {
        "message_id": receipt.message_id,
        "status": receipt.status,
        "correlation_id": receipt.correlation_id,
    }


@router.get("/info")
async def cluster_info(request: Request):
    """Get cluster state summary."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    return membership.get_cluster_info()


@router.get("/nodes")
async def list_nodes(request: Request):
    """List all cluster members."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled", "nodes": [], "total": 0}
    members = membership.get_members()
    return {
        "nodes": [
            {
                "node_id": n.node_id,
                "host": n.host,
                "api_port": n.api_port,
                "status": n.status.value,
                "actor_count": n.actor_count,
                "last_heartbeat": n.last_heartbeat,
                "incarnation": n.incarnation,
            }
            for n in sorted(members.values(), key=lambda x: x.node_id)
        ],
        "total": len(members),
    }


@router.get("/nodes/{node_id}")
async def get_node(request: Request, node_id: str):
    """Get details of a specific cluster node."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    node = membership.get_member(node_id)
    if node is None:
        return {"error": "not_found", "detail": f"Node {node_id} not found"}
    return {
        "node_id": node.node_id,
        "host": node.host,
        "api_port": node.api_port,
        "gossip_port": node.gossip_port,
        "status": node.status.value,
        "actor_count": node.actor_count,
        "metadata": node.metadata,
        "last_heartbeat": node.last_heartbeat,
        "incarnation": node.incarnation,
        "joined_at": node.joined_at,
    }


@router.get("/ring")
async def ring_stats(request: Request):
    """Get hash ring distribution statistics."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    stats = membership._ring.get_partition_stats(num_keys=1000)
    return {
        "ring_nodes": membership._ring.node_count,
        "virtual_nodes": membership._ring.virtual_node_count,
        "distribution": stats,
    }


@router.get("/router-stats")
async def router_stats(request: Request):
    """Get cluster router statistics."""
    cluster_router = _get_cluster_router(request)
    if cluster_router is None:
        return {"error": "not_enabled"}
    return cluster_router.get_stats()


def _get_membership(request: Request):
    """Get ClusterMembership from app state."""
    return getattr(request.app.state, "cluster_membership", None)


def _get_cluster_router(request: Request):
    """Get ClusterRouter from app state."""
    return getattr(request.app.state, "cluster_router", None)


def _get_cluster_pubsub(request: Request):
    """Get ClusterPubSub from app state."""
    return getattr(request.app.state, "cluster_pubsub", None)


def _get_migration_coordinator(request: Request):
    """Get MigrationCoordinator from app state."""
    return getattr(request.app.state, "migration_coordinator", None)


@router.post("/internal/pubsub/publish")
async def handle_remote_publish(request: Request, body: RemotePublishRequest):
    """Handle a pub/sub publish forwarded from a remote cluster node.

    This is the receiving end of the fan-out mechanism. When Node A
    publishes a message, it forwards to all peers. Each peer calls
    this endpoint, which delivers to the peer's local subscribers.
    """
    cluster_pubsub = _get_cluster_pubsub(request)
    if cluster_pubsub is None:
        # Clustering not enabled — just deliver via local pubsub
        pubsub = _get_pubsub_local(request)
        if pubsub is None:
            return {"error": "not_enabled"}
        count = pubsub.publish(
            topic=body.topic,
            payload=body.payload,
            headers=body.headers,
        )
        return {"local_subscriber_count": count}

    count = cluster_pubsub.handle_remote_publish(
        topic=body.topic,
        payload=body.payload,
        headers=body.headers,
        source_node_id=body.source_node_id,
    )
    return {"local_subscriber_count": count}


@router.get("/pubsub-stats")
async def pubsub_stats(request: Request):
    """Get cluster pub/sub statistics."""
    cluster_pubsub = _get_cluster_pubsub(request)
    if cluster_pubsub is None:
        return {"error": "not_enabled"}
    return cluster_pubsub.get_stats()


@router.get("/leader")
async def leader_status(request: Request):
    """Get leader election status."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    return membership.get_leader_status()


@router.post("/leader/step-down")
async def leader_step_down(request: Request):
    """Voluntarily step down if this node is the leader."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    result = membership.leader_step_down()
    return {
        "previous_leader": result["previous_leader"],
        "new_leader": result["new_leader"],
        "is_leader": membership.is_leader,
    }


class ForceLeaderRequest(BaseModel):
    node_id: str


@router.post("/leader/force")
async def leader_force(request: Request, body: ForceLeaderRequest):
    """Force a specific node to be leader (admin operation)."""
    membership = _get_membership(request)
    if membership is None:
        return {"error": "not_enabled"}
    success = membership.leader_force(body.node_id)
    if not success:
        return {"error": "invalid_target", "detail": f"Node {body.node_id} is not alive"}
    return {
        "leader_id": membership.leader_id,
        "is_leader": membership.is_leader,
    }


# ============================================================
# Migration Endpoints
# ============================================================


@router.post("/internal/migrate/receive")
async def receive_migration(request: Request, body: MigrationReceiveRequest):
    """Receive an actor migration from a remote node.

    Called by the source node during Phase 2 of the migration protocol.
    The target node restores the actor with its state and pending messages.
    """
    coordinator = _get_migration_coordinator(request)
    if coordinator is None:
        return {"status": "rejected", "error": "migration_not_enabled"}

    result = coordinator.receive_migration(body.model_dump())
    return result


@router.get("/migration/status")
async def migration_status(request: Request):
    """Get migration coordinator status and statistics."""
    coordinator = _get_migration_coordinator(request)
    if coordinator is None:
        return {"error": "not_enabled"}
    return coordinator.get_status()


@router.get("/migration/stats")
async def migration_stats(request: Request):
    """Get migration statistics."""
    coordinator = _get_migration_coordinator(request)
    if coordinator is None:
        return {"error": "not_enabled"}
    return coordinator.get_stats()


@router.post("/migration/rebalance")
async def trigger_rebalance(request: Request):
    """Trigger a manual rebalance (leader only)."""
    coordinator = _get_migration_coordinator(request)
    if coordinator is None:
        return {"error": "not_enabled"}
    result = await coordinator.rebalance()
    return result


def _get_pubsub_local(request: Request):
    """Get local PubSubService from app state (fallback)."""
    return getattr(request.app.state, "pubsub_service", None)
