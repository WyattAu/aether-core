"""
Mesh Communication Actor Example

Demonstrates distributed messaging across the mesh network.
"""

import asyncio
import logging
import os
import random
from datetime import datetime

from aether_sdk import Actor, Message, MessageType

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class MeshNode:
    """Represents a node in the mesh network."""

    def __init__(
        self, node_id: str, region: str, endpoint: str, status: str = "active"
    ):
        self.node_id = node_id
        self.region = region
        self.endpoint = endpoint
        self.status = status
        self.metadata = {"joined_at": datetime.utcnow().isoformat()}


class MeshMessage:
    """Message sent across the mesh."""

    def __init__(
        self,
        source_node: str,
        content: str,
        target_node: str = None,
        hop_count: int = 0,
    ):
        self.source_node = source_node
        self.target_node = target_node  # None = broadcast
        self.content = content
        self.timestamp = datetime.utcnow().isoformat()
        self.hop_count = hop_count


class MeshActor(Actor):
    """An actor that participates in mesh communication."""

    def __init__(self, region: str = "local"):
        node_id = f"node-{region}-{random.randint(1000, 9999)}"
        super().__init__(f"mesh-{node_id}")
        self.node_id = node_id
        self.region = region
        self.known_nodes: dict[str, MeshNode] = {}
        self.message_log: list[MeshMessage] = []
        self.is_leader = False
        self.leader_id: str | None = None
        self.require("NETWORK_OUTBOUND", "ACTOR_MESSAGING", "LOG", "TIME")

    async def on_start(self) -> None:
        """Initialize mesh actor."""
        logger.info(f"[{self.node_id}] Starting mesh actor in region: {self.region}")

        # Register self
        self.known_nodes[self.node_id] = MeshNode(
            node_id=self.node_id,
            region=self.region,
            endpoint=f"localhost:{random.randint(4000, 5000)}",
        )

    async def on_stop(self) -> None:
        """Cleanup on shutdown."""
        logger.info(f"[{self.node_id}] Mesh actor stopping")
        logger.info(
            f"[{self.node_id}] Known nodes: {len(self.known_nodes)}, Messages: {len(self.message_log)}"
        )

    async def handle_message(self, sender: str, message: Message) -> Message | None:
        """Handle incoming messages."""
        if message.type == MessageType.REQUEST:
            return await self._handle_request(sender, message)
        elif message.type == MessageType.EVENT:
            await self._handle_event(sender, message)
            return None
        elif message.type == MessageType.RESPONSE:
            logger.info(f"[{self.node_id}] Received response from {sender}")
            return None

        return Message.response({"error": "unknown message type"})

    async def _handle_request(self, sender: str, message: Message) -> Message:
        """Handle request messages."""
        payload = message.payload
        if not isinstance(payload, dict):
            return Message.response({"error": "invalid payload format"})

        action = payload.get("action", "")

        if action == "ping":
            return Message.response(
                {
                    "action": "pong",
                    "node_id": self.node_id,
                    "region": self.region,
                    "timestamp": datetime.utcnow().isoformat(),
                    "status": "healthy",
                }
            )

        elif action == "discover":
            nodes = [
                {"id": n.node_id, "region": n.region, "status": n.status}
                for n in self.known_nodes.values()
            ]
            return Message.response(
                {
                    "action": "discover_response",
                    "node_id": self.node_id,
                    "known_nodes": nodes,
                    "count": len(nodes),
                }
            )

        elif action == "broadcast":
            content = payload.get("content", "")
            source = payload.get("source_node", sender)
            hop_count = payload.get("hop_count", 0)

            mesh_msg = MeshMessage(
                source_node=source, content=content, hop_count=hop_count
            )
            self.message_log.append(mesh_msg)

            logger.info(f"[{self.node_id}] Broadcast from {source}: {content[:30]}...")

            return Message.response(
                {"action": "broadcast_ack", "node_id": self.node_id, "received": True}
            )

        elif action == "direct_message":
            content = payload.get("content", "")
            source = payload.get("source_node", sender)

            mesh_msg = MeshMessage(
                source_node=source, target_node=self.node_id, content=content
            )
            self.message_log.append(mesh_msg)

            logger.info(f"[{self.node_id}] Direct message from {source}: {content}")

            return Message.response(
                {
                    "action": "direct_message_ack",
                    "node_id": self.node_id,
                    "received": True,
                    "timestamp": datetime.utcnow().isoformat(),
                }
            )

        elif action == "get_status":
            return Message.response(
                {
                    "action": "status",
                    "node_id": self.node_id,
                    "region": self.region,
                    "status": "active",
                    "known_nodes": len(self.known_nodes),
                    "messages_handled": len(self.message_log),
                    "is_leader": self.is_leader,
                    "leader_id": self.leader_id,
                }
            )

        elif action == "elect_leader":
            candidate_id = payload.get("candidate_id", "")

            # Simple election: highest node ID wins
            if candidate_id > self.node_id:
                self.leader_id = candidate_id
                self.is_leader = False
                logger.info(f"[{self.node_id}] Acknowledging {candidate_id} as leader")
            else:
                self.leader_id = self.node_id
                self.is_leader = True
                logger.info(f"[{self.node_id}] Claiming leadership")

            return Message.response(
                {
                    "action": "election_vote",
                    "voter_id": self.node_id,
                    "leader_id": self.leader_id,
                    "is_leader": self.is_leader,
                    "timestamp": datetime.utcnow().isoformat(),
                }
            )

        else:
            return Message.response(
                {"error": f"unknown action: {action}", "node_id": self.node_id}
            )

    async def _handle_event(self, sender: str, message: Message) -> None:
        """Handle event messages."""
        payload = message.payload
        if not isinstance(payload, dict):
            return

        event_type = payload.get("type", "")

        if event_type == "node_join":
            node_data = payload.get("node", {})
            node = MeshNode(
                node_id=node_data.get("id", ""),
                region=node_data.get("region", ""),
                endpoint=node_data.get("endpoint", ""),
            )
            if node.node_id:
                self.known_nodes[node.node_id] = node
                logger.info(f"[{self.node_id}] Node joined: {node.node_id}")

        elif event_type == "node_leave":
            node_id = payload.get("node_id", "")
            if node_id in self.known_nodes:
                del self.known_nodes[node_id]
                logger.info(f"[{self.node_id}] Node left: {node_id}")

                # Trigger re-election if leader left
                if node_id == self.leader_id:
                    self.leader_id = None
                    self.is_leader = False
                    logger.info(f"[{self.node_id}] Leader left, re-election needed")


async def main():
    """Main entry point."""
    # Get region from environment or use default
    region = os.environ.get("AETHER_REGION", "us-east-1")

    actor = MeshActor(region)

    logger.info("Starting mesh actor...")
    logger.info(f"Node ID: {actor.node_id}")
    logger.info(f"Region: {actor.region}")
    logger.info(
        "Supported actions: ping, discover, broadcast, direct_message, get_status, elect_leader"
    )

    try:
        await actor.start()
        # Run until cancelled
        await actor.run()
    except asyncio.CancelledError:
        logger.info("Actor cancelled")
    finally:
        await actor.stop()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Shutting down...")
