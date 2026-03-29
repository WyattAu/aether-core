"""Tests for the gRPC authentication interceptor."""

import json

import grpc
import pytest

from server.auth import AuthConfig, TokenService
from server.config import ServerConfig
from server.grpc_auth import AuthServerInterceptor
from server.grpc_server import create_grpc_server
from server.proto.aether.v1 import aether_pb2, aether_pb2_grpc


def _make_valid_token(config: AuthConfig, subject: str = "test-user") -> str:
    """Create a valid signed token for testing."""
    svc = TokenService(config)
    return svc.create_token(subject)


@pytest.fixture
def services():
    """Create server components."""
    from server.actor_manager import ActorManager
    from server.event_store import EventStore
    from server.message_router import MessageRouter
    from server.pubsub_service import PubSubService
    from server.state_store import MemoryStateStore

    config = ServerConfig()
    actors = ActorManager(config)
    messages = MessageRouter(message_ttl=300)
    state = MemoryStateStore()
    pubsub = PubSubService()
    events = EventStore()
    return actors, messages, state, pubsub, events


@pytest.fixture
def auth_config():
    return AuthConfig(
        enabled=True,
        secret="test-secret-key-16chars!",
        token_ttl=3600,
    )


@pytest.fixture
def grpc_channel_with_auth(services, auth_config):
    """Create an in-process gRPC channel with auth interceptor enabled."""
    actors, messages, state, pubsub, events = services
    server = create_grpc_server(
        actors, messages, state, pubsub, events,
        max_workers=4,
        auth_config=auth_config,
    )
    port = server.add_insecure_port("localhost:0")
    server.start()
    channel = grpc.insecure_channel(f"localhost:{port}")
    yield channel, auth_config
    channel.close()
    server.stop(1.0)


class TestGrpcAuthInterceptor:

    def test_health_bypasses_auth(self, grpc_channel_with_auth):
        """Health endpoints should be accessible without authentication."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.HealthServiceStub(channel)
        resp = stub.Health(aether_pb2.Empty())
        assert resp.status == "ok"

    def test_ready_bypasses_auth(self, grpc_channel_with_auth):
        """Ready endpoint should be accessible without authentication."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.HealthServiceStub(channel)
        resp = stub.Ready(aether_pb2.Empty())
        assert resp.status == "ok"

    def test_info_bypasses_auth(self, grpc_channel_with_auth):
        """Info endpoint should be accessible without authentication."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.HealthServiceStub(channel)
        resp = stub.Info(aether_pb2.Empty())
        assert resp.status == "ok"

    def test_unauthenticated_call_rejected(self, grpc_channel_with_auth):
        """Calls without auth token should be rejected."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.ListActors(aether_pb2.ListActorsRequest())
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED

    def test_authenticated_call_succeeds(self, grpc_channel_with_auth):
        """Calls with valid Bearer token should succeed."""
        channel, auth_config = grpc_channel_with_auth
        token = _make_valid_token(auth_config, "user-1")
        metadata = [("authorization", f"Bearer {token}")]
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        resp = stub.ListActors(aether_pb2.ListActorsRequest(), metadata=metadata)
        assert resp.total == 0

    def test_x_aether_token_header_accepted(self, grpc_channel_with_auth):
        """Calls with x-aether-token metadata should succeed."""
        channel, auth_config = grpc_channel_with_auth
        token = _make_valid_token(auth_config, "user-2")
        metadata = [("x-aether-token", token)]
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        resp = stub.Register(aether_pb2.RegisterActorRequest(
            actor_id="auth-actor-1",
            actor_type="worker",
        ), metadata=metadata)
        assert resp.actor_id == "auth-actor-1"

    def test_invalid_token_rejected(self, grpc_channel_with_auth):
        """Calls with tampered token should be rejected."""
        channel, _ = grpc_channel_with_auth
        metadata = [("authorization", "Bearer invalid.token.here")]
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.ListActors(aether_pb2.ListActorsRequest(), metadata=metadata)
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED

    def test_expired_token_rejected(self, grpc_channel_with_auth):
        """Calls with expired token should be rejected."""
        channel, auth_config = grpc_channel_with_auth
        svc = TokenService(auth_config)
        token = svc.create_token("user-3", ttl=-1)  # Already expired
        metadata = [("authorization", f"Bearer {token}")]
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.ListActors(aether_pb2.ListActorsRequest(), metadata=metadata)
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED

    def test_wrong_secret_rejected(self, grpc_channel_with_auth):
        """Calls with token signed by different secret should be rejected."""
        channel, _ = grpc_channel_with_auth
        wrong_config = AuthConfig(enabled=True, secret="wrong-secret-16ch!", token_ttl=3600)
        token = _make_valid_token(wrong_config, "attacker")
        metadata = [("authorization", f"Bearer {token}")]
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.ListActors(aether_pb2.ListActorsRequest(), metadata=metadata)
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED

    def test_auth_disabled_all_calls_succeed(self, services):
        """When auth is disabled, all calls should succeed without tokens."""
        actors, messages, state, pubsub, events = services
        auth_config = AuthConfig(enabled=False)
        server = create_grpc_server(
            actors, messages, state, pubsub, events,
            max_workers=4,
            auth_config=auth_config,
        )
        port = server.add_insecure_port("localhost:0")
        server.start()
        channel = grpc.insecure_channel(f"localhost:{port}")

        try:
            # Actor call without token
            actor_stub = aether_pb2_grpc.ActorServiceStub(channel)
            resp = actor_stub.Register(aether_pb2.RegisterActorRequest(actor_id="noauth-1"))
            assert resp.actor_id == "noauth-1"

            # State call without token
            state_stub = aether_pb2_grpc.StateServiceStub(channel)
            resp = state_stub.SetState(aether_pb2.SetStateRequest(
                actor_id="noauth-1", key="k", value=json.dumps(1).encode(),
            ))
            assert resp.version == 1
        finally:
            channel.close()
            server.stop(1.0)

    def test_message_service_requires_auth(self, grpc_channel_with_auth):
        """Message service should require authentication."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.MessageServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.GetPending(aether_pb2.GetPendingMessagesRequest(actor_id="x"))
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED

    def test_state_service_requires_auth(self, grpc_channel_with_auth):
        """State service should require authentication."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.StateServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.GetState(aether_pb2.GetStateRequest(actor_id="x", key="y"))
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED

    def test_event_service_requires_auth(self, grpc_channel_with_auth):
        """Event service should require authentication."""
        channel, _ = grpc_channel_with_auth
        stub = aether_pb2_grpc.EventServiceStub(channel)
        with pytest.raises(grpc.RpcError) as exc_info:
            stub.ListTopics(aether_pb2.ListTopicsRequest())
        assert exc_info.value.code() == grpc.StatusCode.UNAUTHENTICATED
