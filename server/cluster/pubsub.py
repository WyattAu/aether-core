"""Cluster-aware pub/sub service.

Wraps the in-memory PubSubService and adds cross-node message fan-out
using the existing HTTP cluster transport. When clustering is enabled,
published messages are delivered to local subscribers AND forwarded to
all alive peer nodes, which then deliver to their local subscribers.

This design requires no external dependencies (no Redis) — it purely
uses the gossip-style HTTP transport already used by the cluster module.

Message flow:
    Publisher -> ClusterPubSub.publish()
        -> Local PubSubService (local subscribers)
        -> HTTP POST to each alive peer
            -> Peer's /cluster/internal/pubsub/publish endpoint
            -> Peer's local PubSubService (peer's local subscribers)

Subscription tracking:
    Each node independently tracks its own local subscriptions.
    When a message is fanned out to a peer, the peer checks if
    it has local subscribers and delivers accordingly.
"""

import logging
import threading
from datetime import datetime, timezone
from typing import Any, Callable, Dict, List, Optional

from ..models import PubSubMessage, Subscription
from ..pubsub_service import PubSubService
from .membership import ClusterMembership
from .transport import ClusterTransport

logger = logging.getLogger("aether-server.cluster.pubsub")


class ClusterPubSub:
    """Cluster-aware pub/sub that fans out publishes to all nodes.

    This class has the same public interface as ``PubSubService`` so
    it can be used as a drop-in replacement when clustering is enabled.

    All operations are thread-safe.

    Args:
        local_pubsub: The underlying in-memory PubSubService.
        membership: Cluster membership for discovering alive peers.
        transport: HTTP transport for cross-node communication.
        node_id: This node's ID (to skip self in fan-out).
    """

    def __init__(
        self,
        local_pubsub: PubSubService,
        membership: ClusterMembership,
        transport: ClusterTransport,
        node_id: str,
    ):
        self._local = local_pubsub
        self._membership = membership
        self._transport = transport
        self._node_id = node_id
        self._lock = threading.Lock()

        # Fan-out statistics
        self._fan_out_count = 0
        self._fan_out_errors = 0
        self._remote_delivered = 0

    # ================================================================
    # PubSubService-compatible API
    # ================================================================

    def publish(
        self,
        topic: str,
        payload: Any = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> int:
        """Publish a message to the topic.

        Delivers to local subscribers first, then fans out to all
        alive cluster peers.

        Returns:
            Total subscriber count (local only — remote counts are
            best-effort and not included to avoid latency).
        """
        msg = PubSubMessage(topic=topic, payload=payload, headers=headers or {})
        local_count = self._local.publish(topic, payload=payload, headers=headers)

        # Fan out to alive peers
        self._fan_out_to_peers(msg)

        return local_count

    def publish_with_handler(
        self,
        topic: str,
        handler: Callable,
        payload: Any = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> int:
        """Publish a message and invoke handler for each local subscriber.

        Also fans out to remote peers (remote peers invoke their own
        local handlers if applicable).

        Returns:
            Total local subscriber count.
        """
        msg = PubSubMessage(topic=topic, payload=payload, headers=headers or {})
        local_count = self._local.publish_with_handler(
            topic, handler, payload=payload, headers=headers,
        )

        # Fan out to alive peers
        self._fan_out_to_peers(msg)

        return local_count

    def subscribe(
        self,
        topic: str,
        subscriber_id: str,
        filter: Optional[str] = None,
    ) -> str:
        """Subscribe to a topic (local only).

        Returns:
            The subscription ID.
        """
        return self._local.subscribe(topic, subscriber_id, filter=filter)

    def unsubscribe(self, subscription_id: str) -> bool:
        """Unsubscribe from a topic (local only)."""
        return self._local.unsubscribe(subscription_id)

    def list_topics(self) -> List[str]:
        """List topics that have local subscribers."""
        return self._local.list_topics()

    def list_subscribers(self, topic: str) -> List[str]:
        """List local subscribers for a topic."""
        return self._local.list_subscribers(topic)

    def get_history(self, topic: str) -> List[PubSubMessage]:
        """Get local message history for a topic."""
        return self._local.get_history(topic)

    def get_matching_subscribers(self, topic: str) -> List[Subscription]:
        """Get local subscriptions matching a topic pattern."""
        return self._local.get_matching_subscribers(topic)

    # ================================================================
    # Remote publish handler (called by cluster API endpoint)
    # ================================================================

    def handle_remote_publish(
        self,
        topic: str,
        payload: Any = None,
        headers: Optional[Dict[str, str]] = None,
        source_node_id: Optional[str] = None,
    ) -> int:
        """Handle a publish forwarded from a remote cluster node.

        This is called by the ``/cluster/internal/pubsub/publish``
        endpoint when another node fans out a message.

        Returns:
            Number of local subscribers that received the message.
        """
        return self._local.publish(topic, payload=payload, headers=headers)

    # ================================================================
    # Fan-out logic
    # ================================================================

    def _fan_out_to_peers(self, msg: PubSubMessage) -> None:
        """Forward a published message to all alive cluster peers.

        Uses best-effort delivery — errors are logged but do not
        block the publisher. Each peer independently decides whether
        it has local subscribers for the topic.
        """
        if not self._membership.is_running:
            return

        peers = self._membership.alive_nodes
        if not peers:
            return

        publish_data = {
            "topic": msg.topic,
            "payload": msg.payload,
            "headers": msg.headers,
            "source_node_id": self._node_id,
            "message_id": msg.message_id,
            "timestamp": msg.timestamp.isoformat() if msg.timestamp else None,
        }

        with self._lock:
            self._fan_out_count += 1

        for peer in peers:
            if peer.node_id == self._node_id:
                continue
            try:
                response = self._transport.forward_pubsub(
                    peer.host, peer.api_port, publish_data,
                )
                if response is not None:
                    remote_count = response.get("local_subscriber_count", 0)
                    with self._lock:
                        self._remote_delivered += remote_count
                else:
                    with self._lock:
                        self._fan_out_errors += 1
            except Exception as e:
                with self._lock:
                    self._fan_out_errors += 1
                logger.debug(
                    "Failed to fan-out publish to %s: %s",
                    peer.node_id, e,
                )

    # ================================================================
    # Statistics
    # ================================================================

    def get_stats(self) -> Dict[str, Any]:
        """Get cluster pub/sub statistics."""
        return {
            "local_topics": len(self._local.list_topics()),
            "local_subscriptions": sum(
                len(subs) for subs in self._local._subscriptions.values()
            ),
            "fan_out_count": self._fan_out_count,
            "fan_out_errors": self._fan_out_errors,
            "remote_delivered": self._remote_delivered,
            "cluster_peers": len(self._membership.alive_nodes) if self._membership.is_running else 0,
        }
