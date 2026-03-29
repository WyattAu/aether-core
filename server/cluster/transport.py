"""HTTP transport for inter-node cluster communication.

Provides synchronous and asynchronous HTTP calls between cluster nodes
for gossip protocol operations (ping, ping-req, sync).
"""

import logging
from typing import Any, Dict, Optional

import httpx

logger = logging.getLogger("aether-server.cluster.transport")


class ClusterTransport:
    """HTTP client for inter-node cluster communication.
    
    Each node exposes internal cluster endpoints:
    - POST /cluster/internal/ping — Respond to heartbeat ping
    - POST /cluster/internal/ping-req — Forwarded ping request
    - POST /cluster/internal/sync — Gossip membership sync
    
    This transport calls those endpoints on remote nodes.
    """
    
    def __init__(self, timeout: float = 3.0, cluster_secret: str = ""):
        self._timeout = timeout
        self._headers = {"Content-Type": "application/json"}
        if cluster_secret:
            self._headers["X-Cluster-Secret"] = cluster_secret
        self._client = httpx.Client(timeout=timeout, headers=self._headers)
    
    def ping(self, host: str, port: int, node: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Send a ping to a remote node."""
        url = f"http://{host}:{port}/cluster/internal/ping"
        try:
            resp = self._client.post(url, json={"node": node}, timeout=self._timeout)
            if resp.status_code == 200:
                return resp.json()
            return None
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            logger.debug("Ping to %s:%d failed: %s", host, port, e)
            return None
    
    def ping_request(
        self, target_host: str, target_port: int, 
        suspect_host: str, suspect_port: int,
        node: Dict[str, Any],
    ) -> Optional[Dict[str, Any]]:
        """Ask a target node to ping a suspect node on our behalf."""
        url = f"http://{target_host}:{target_port}/cluster/internal/ping-req"
        try:
            resp = self._client.post(url, json={
                "node": node,
                "target": {"host": suspect_host, "port": suspect_port},
            }, timeout=self._timeout)
            if resp.status_code == 200:
                return resp.json()
            return None
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            logger.debug("Ping-req to %s:%d failed: %s", target_host, target_port, e)
            return None
    
    def sync(
        self, host: str, port: int, 
        nodes: Dict[str, Dict[str, Any]],
    ) -> Optional[Dict[str, Any]]:
        """Send membership sync (full state exchange) to a remote node."""
        url = f"http://{host}:{port}/cluster/internal/sync"
        try:
            resp = self._client.post(url, json={"nodes": nodes}, timeout=self._timeout)
            if resp.status_code == 200:
                return resp.json()
            return None
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            logger.debug("Sync to %s:%d failed: %s", host, port, e)
            return None
    
    def forward_message(
        self, host: str, port: int, 
        message: Dict[str, Any],
    ) -> Optional[Dict[str, Any]]:
        """Forward a message to a remote node for delivery."""
        url = f"http://{host}:{port}/cluster/internal/message"
        try:
            resp = self._client.post(url, json=message, timeout=self._timeout)
            if resp.status_code == 200:
                return resp.json()
            return None
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            logger.debug("Message forward to %s:%d failed: %s", host, port, e)
            return None
    
    def forward_pubsub(
        self, host: str, port: int,
        publish_data: Dict[str, Any],
    ) -> Optional[Dict[str, Any]]:
        """Forward a pub/sub publish to a remote node for fan-out delivery."""
        url = f"http://{host}:{port}/cluster/internal/pubsub/publish"
        try:
            resp = self._client.post(url, json=publish_data, timeout=self._timeout)
            if resp.status_code == 200:
                return resp.json()
            return None
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            logger.debug("PubSub fan-out to %s:%d failed: %s", host, port, e)
            return None

    def migrate_actor(
        self, host: str, port: int,
        payload: Dict[str, Any],
    ) -> Optional[Dict[str, Any]]:
        """Send an actor migration payload to a target node.

        Args:
            host: Target node host.
            port: Target node API port.
            payload: Migration payload with actor state.

        Returns:
            Response dict with status, or None on failure.
        """
        url = f"http://{host}:{port}/cluster/internal/migrate/receive"
        # Use a longer timeout for migrations (they may include large state)
        try:
            resp = self._client.post(
                url, json=payload,
                timeout=max(self._timeout, 30.0),
            )
            if resp.status_code == 200:
                return resp.json()
            return None
        except (httpx.HTTPError, httpx.TimeoutException) as e:
            logger.debug("Migration transfer to %s:%d failed: %s", host, port, e)
            return None
    
    def close(self) -> None:
        """Close the HTTP client."""
        self._client.close()
