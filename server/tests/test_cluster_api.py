"""Tests for cluster API endpoints."""

import pytest
from fastapi import FastAPI
from starlette.testclient import TestClient

from server.cluster.config import ClusterConfig
from server.cluster.membership import ClusterMembership
from server.cluster.node import ClusterNode, NodeStatus


def _make_app(cluster_enabled=False, cluster_config=None):
    """Create a test app with cluster support."""
    app = FastAPI()

    if cluster_enabled:
        cluster_config = cluster_config or ClusterConfig(enabled=True, node_id="test-node")
        app.state.cluster_config = cluster_config
    else:
        app.state.cluster_config = ClusterConfig(enabled=False)

    from server.api.cluster import router as cluster_router
    app.include_router(cluster_router, prefix="/cluster")

    return app


def _init_membership(node_id="test"):
    """Create a membership with self-node initialized (no gossip loop)."""
    membership = ClusterMembership(ClusterConfig(enabled=True, node_id=node_id))
    membership._self = ClusterNode(node_id=node_id, host="localhost", api_port=8080, status=NodeStatus.ALIVE)
    membership._members[node_id] = membership._self
    membership._ring.add_node(membership._self)
    return membership


class TestClusterInfoEndpoint:

    def test_cluster_info_without_membership(self):
        app = _make_app(cluster_enabled=False)
        client = TestClient(app)
        resp = client.get("/cluster/info")
        assert resp.status_code == 200
        data = resp.json()
        assert data.get("error") == "not_enabled"

    def test_cluster_info_with_membership(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        app.state.cluster_membership = _init_membership("api-test")
        resp = client.get("/cluster/info")
        assert resp.status_code == 200
        data = resp.json()
        assert "node_id" in data
        assert "members" in data
        assert "status" in data


class TestClusterNodesEndpoint:

    def test_list_nodes_with_membership(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        membership = _init_membership("test")
        membership.register_node(ClusterNode(
            node_id="node-a", host="10.0.0.1", api_port=8080, status=NodeStatus.ALIVE,
        ))
        membership.register_node(ClusterNode(
            node_id="node-b", host="10.0.0.2", api_port=8080, status=NodeStatus.SUSPECT,
        ))
        app.state.cluster_membership = membership

        resp = client.get("/cluster/nodes")
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 3
        assert len(data["nodes"]) == 3

    def test_get_specific_node(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        membership = _init_membership("test")
        membership.register_node(ClusterNode(
            node_id="target", host="10.0.0.5", api_port=8080, status=NodeStatus.ALIVE,
        ))
        app.state.cluster_membership = membership

        resp = client.get("/cluster/nodes/target")
        assert resp.status_code == 200
        data = resp.json()
        assert data["node_id"] == "target"
        assert data["host"] == "10.0.0.5"

    def test_get_nonexistent_node(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        membership = _init_membership("test")
        app.state.cluster_membership = membership

        resp = client.get("/cluster/nodes/ghost")
        assert resp.status_code == 200
        data = resp.json()
        assert "error" in data


class TestClusterInternalEndpoints:

    def test_internal_ping(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        membership = _init_membership("test")
        app.state.cluster_membership = membership

        sender = ClusterNode(node_id="sender", host="10.0.0.1", api_port=8080)
        resp = client.post("/cluster/internal/ping", json={"node": sender.to_dict()})

        assert resp.status_code == 200
        data = resp.json()
        assert data["node"]["node_id"] == "test"
        assert "sender" in data["nodes"]

    def test_internal_sync(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        membership = _init_membership("test")
        app.state.cluster_membership = membership

        remote_nodes = {
            "r1": ClusterNode(node_id="r1", host="10.0.0.2", api_port=8080).to_dict(),
        }
        resp = client.post("/cluster/internal/sync", json={"nodes": remote_nodes})

        assert resp.status_code == 200
        data = resp.json()
        assert "r1" in data["nodes"]

    def test_internal_message_without_router(self):
        app = _make_app(cluster_enabled=True)
        client = TestClient(app)

        resp = client.post("/cluster/internal/message", json={
            "source_actor": "sender",
            "target_actor": "target",
            "message_type": "test",
            "payload": {"data": 1},
        })

        assert resp.status_code == 200
        data = resp.json()
        assert data.get("status") == "error"
