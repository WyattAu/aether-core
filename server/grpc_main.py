"""gRPC server entry point for the Aether reference server.

Run alongside the REST server to serve gRPC clients::

    # Terminal 1: REST
    python -m server.app --host 0.0.0.0 --port 8080

    # Terminal 2: gRPC
    python -m server.grpc_main --host 0.0.0.0 --port 50051

Or run standalone with just gRPC::

    python -m server.grpc_main

The gRPC server shares the same in-memory backends (ActorManager,
MessageRouter, StateStore, PubSubService, EventStore) and serves all
five Aether gRPC services: Actor, Message, State, Event, Health.
"""

import argparse
import logging
import signal
import sys

from .actor_manager import ActorManager
from .config import ServerConfig
from .event_store import EventStore
from .grpc_server import create_grpc_server
from .message_router import MessageRouter
from .pubsub_service import PubSubService
from .state_store import create_state_store

logger = logging.getLogger("aether-server.grpc")


def main():
    parser = argparse.ArgumentParser(description="Aether gRPC Server")
    parser.add_argument("--host", default="0.0.0.0", help="Bind address")
    parser.add_argument("--port", type=int, default=50051, help="Bind port")
    parser.add_argument("--workers", type=int, default=10, help="Thread pool workers")
    args = parser.parse_args()

    config = ServerConfig()

    # Setup structured logging
    from .logging_config import setup_logging
    setup_logging(level=config.log_level, json_enabled=config.json_logging_enabled)

    # Create shared server components
    actors = ActorManager(config)
    messages = MessageRouter(message_ttl=config.message_ttl_seconds)
    state = create_state_store(
        config.state_backend,
        redis_url=config.redis_url,
        key_prefix=config.redis_key_prefix,
        ttl_seconds=config.redis_ttl_seconds,
    )
    pubsub = PubSubService()
    events = EventStore()

    # Create and start gRPC server
    auth_config = None
    if config.auth_enabled:
        from .auth import AuthConfig
        auth_config = AuthConfig(
            enabled=True,
            secret=config.auth_secret,
            token_ttl=config.auth_token_ttl,
        )

    from .metrics import MetricsCollector

    metrics = MetricsCollector()

    # Setup clustering (if enabled)
    cluster_membership = None
    cluster_transport = None
    if config.cluster_enabled:
        from .cluster.config import ClusterConfig
        from .cluster.membership import ClusterMembership
        from .cluster.router import ClusterRouter
        from .cluster.transport import ClusterTransport

        cluster_config = ClusterConfig(
            enabled=True,
            node_id=config.cluster_node_id,
            seed_nodes=config.cluster_seed_nodes,
            bind_host=config.cluster_bind_host,
            gossip_port=config.cluster_gossip_port,
            gossip_interval_seconds=config.cluster_gossip_interval,
            failure_timeout_seconds=config.cluster_failure_timeout,
            dead_timeout_seconds=config.cluster_dead_timeout,
            suspicion_max=config.cluster_suspicion_max,
            virtual_nodes=config.cluster_virtual_nodes,
            transport=config.cluster_transport,
            cluster_secret=config.cluster_secret,
        )

        cluster_membership = ClusterMembership(cluster_config)
        import asyncio
        loop = asyncio.new_event_loop()
        try:
            cluster_node = loop.run_until_complete(cluster_membership.start(
                host=config.cluster_bind_host or args.host,
                api_port=args.port,
            ))
        finally:
            loop.close()

        cluster_transport = ClusterTransport(
            timeout=config.cluster_failure_timeout,
            cluster_secret=config.cluster_secret,
        )

        messages = ClusterRouter(messages, cluster_membership, cluster_transport)

        logger.info("Clustering enabled (node=%s, peers=%d)",
                     cluster_node.node_id, cluster_membership.member_count)

    server = create_grpc_server(
        actor_manager=actors,
        message_router=messages,
        state_store=state,
        pubsub=pubsub,
        event_store=events,
        max_workers=args.workers,
        auth_config=auth_config,
        metrics=metrics,
    )

    # Graceful shutdown using ShutdownManager
    from .shutdown import ShutdownManager
    shutdown_mgr = ShutdownManager(drain_timeout_seconds=config.drain_timeout_seconds)

    def _cleanup_grpc():
        logger.info("Stopping gRPC server (drain=%.1fs)...", config.drain_timeout_seconds)
        server.stop(grace=config.drain_timeout_seconds)
        logger.info("gRPC server stopped")

    def _cleanup_state():
        if hasattr(state, 'close'):
            state.close()
            logger.info("State store closed")

    def _cleanup_events():
        if hasattr(events, 'close'):
            events.close()
            logger.info("Event store closed")

    shutdown_mgr.register_cleanup(_cleanup_grpc)
    shutdown_mgr.register_cleanup(_cleanup_state)
    shutdown_mgr.register_cleanup(_cleanup_events)

    if cluster_membership is not None:
        def _cleanup_cluster():
            import asyncio
            logger.info("Stopping cluster membership...")
            if cluster_transport:
                cluster_transport.close()
            loop = asyncio.new_event_loop()
            try:
                loop.run_until_complete(cluster_membership.stop())
            finally:
                loop.close()
            logger.info("Cluster membership stopped")
        shutdown_mgr.register_cleanup(_cleanup_cluster)

    shutdown_mgr.install_signal_handlers()

    from .grpc_server import serve_grpc
    logger.info("Starting Aether gRPC server on %s:%d", args.host, args.port)
    serve_grpc(server, host=args.host, port=args.port)

    # Cleanup on normal exit (if serve_grpc returns)
    shutdown_mgr.restore_signal_handlers()


if __name__ == "__main__":
    main()
