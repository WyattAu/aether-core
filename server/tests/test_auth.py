"""Tests for JWT authentication middleware."""

import time

import pytest
from fastapi.testclient import TestClient

from server.auth import (
    AuthConfig,
    AuthError,
    AuthMiddleware,
    TokenService,
)


# === TokenService Tests ===

class TestTokenService:

    @pytest.fixture
    def service(self):
        config = AuthConfig(
            enabled=True,
            secret="test-secret-key-123456",
            token_ttl=3600,
        )
        return TokenService(config)

    def test_create_and_verify_token(self, service):
        token = service.create_token("user-1")
        claims = service.verify_token(token)
        assert claims["sub"] == "user-1"
        assert "exp" in claims
        assert "iat" in claims

    def test_token_with_extra_claims(self, service):
        token = service.create_token("user-1", extra_claims={"role": "admin", "scope": "read"})
        claims = service.verify_token(token)
        assert claims["sub"] == "user-1"
        assert claims["role"] == "admin"
        assert claims["scope"] == "read"

    def test_token_with_custom_ttl(self, service):
        token = service.create_token("user-1", ttl=60)
        claims = service.verify_token(token)
        # TTL should be roughly 60 seconds from now
        assert claims["exp"] - claims["iat"] == 60

    def test_expired_token(self, service):
        # Create a token that's already expired
        token = service.create_token("user-1", ttl=-1)
        with pytest.raises(AuthError, match="expired"):
            service.verify_token(token)

    def test_tampered_token(self, service):
        token = service.create_token("user-1")
        # Tamper with the payload
        payload_part = token.split(".")[0]
        # Add a character to make signature invalid
        tampered = payload_part + "x." + token.split(".")[1]
        with pytest.raises(AuthError, match="signature"):
            service.verify_token(tampered)

    def test_wrong_secret(self):
        service1 = TokenService(AuthConfig(secret="secret-1"))
        service2 = TokenService(AuthConfig(secret="secret-2"))
        token = service1.create_token("user-1")
        with pytest.raises(AuthError, match="signature"):
            service2.verify_token(token)

    def test_empty_token(self, service):
        with pytest.raises(AuthError, match="Missing"):
            service.verify_token("")

    def test_none_token(self, service):
        with pytest.raises(AuthError, match="Missing"):
            service.verify_token(None)

    def test_malformed_token(self, service):
        with pytest.raises(AuthError, match="format"):
            service.verify_token("not.a.valid.token.format")

    def test_garbage_payload(self, service):
        import base64
        garbage = base64.urlsafe_b64encode(b"not-json").rstrip(b"=").decode()
        # Signature check happens before payload parsing, so expect signature error
        sig = "a" * 64
        with pytest.raises(AuthError, match="signature"):
            service.verify_token(f"{garbage}.{sig}")

    def test_different_algorithms(self):
        sha512_config = AuthConfig(secret="test-secret", algorithm="sha512")
        service = TokenService(sha512_config)
        token = service.create_token("user-1")
        claims = service.verify_token(token)
        assert claims["sub"] == "user-1"


# === AuthConfig Tests ===

class TestAuthConfig:

    def test_default_public_paths(self):
        config = AuthConfig()
        assert "/health" in config.public_paths
        assert "/health/ready" in config.public_paths
        assert "/api/v1/info" in config.public_paths

    def test_custom_public_paths(self):
        config = AuthConfig(public_paths={"/custom", "/public"})
        assert "/custom" in config.public_paths
        assert "/health" not in config.public_paths

    def test_short_secret_warning(self, caplog):
        import logging
        # Ensure the aether-server.auth logger propagates to root for caplog
        with caplog.at_level(logging.WARNING, logger="aether-server.auth"):
            config = AuthConfig(enabled=True, secret="short")
            assert "less than 16 characters" in caplog.text


# === AuthError Tests ===

class TestAuthError:

    def test_default_status_code(self):
        err = AuthError("test")
        assert err.message == "test"
        assert err.status_code == 401

    def test_custom_status_code(self):
        err = AuthError("forbidden", status_code=403)
        assert err.status_code == 403

    def test_is_exception(self):
        assert isinstance(AuthError("test"), Exception)


# === AuthMiddleware Integration Tests ===

class TestAuthMiddleware:
    """Integration tests using FastAPI TestClient with auth-enabled app."""

    AUTH_SECRET = "test-secret-key-123456"

    @pytest.fixture
    def auth_app(self):
        """Create a FastAPI app with auth enabled."""
        from server.app import create_app
        from server.config import ServerConfig
        return create_app(ServerConfig(
            auth_enabled=True,
            auth_secret=self.AUTH_SECRET,
            auth_token_ttl=3600,
        ))

    @pytest.fixture
    def auth_client(self, auth_app):
        """Create a TestClient with auth enabled."""
        with TestClient(auth_app) as c:
            yield c

    @pytest.fixture
    def valid_token(self):
        """Generate a valid auth token."""
        config = AuthConfig(secret=self.AUTH_SECRET, token_ttl=3600)
        return TokenService(config).create_token("test-user")

    @pytest.fixture
    def expired_token(self):
        """Generate an expired auth token."""
        config = AuthConfig(secret=self.AUTH_SECRET, token_ttl=3600)
        return TokenService(config).create_token("test-user", ttl=-1)

    def test_public_path_no_auth_required(self, auth_client):
        """Public paths should pass without auth."""
        resp = auth_client.get("/health")
        assert resp.status_code == 200

    def test_public_info_path(self, auth_client):
        """Info endpoint should be public."""
        resp = auth_client.get("/api/v1/info")
        assert resp.status_code == 200

    def test_protected_path_requires_auth(self, auth_client):
        """Protected paths should return 401 without token."""
        resp = auth_client.get("/api/v1/actors")
        assert resp.status_code == 401
        assert "Authentication required" in resp.json()["detail"]

    def test_bearer_token_auth(self, auth_client, valid_token):
        """Bearer token in Authorization header should work."""
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        assert resp.status_code == 200

    def test_x_aether_token_header(self, auth_client, valid_token):
        """X-Aether-Token header should work."""
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"X-Aether-Token": valid_token},
        )
        assert resp.status_code == 200

    def test_expired_token_rejected(self, auth_client, expired_token):
        """Expired tokens should be rejected."""
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"Authorization": f"Bearer {expired_token}"},
        )
        assert resp.status_code == 401
        assert "expired" in resp.json()["detail"]

    def test_invalid_token_rejected(self, auth_client):
        """Invalid tokens should be rejected."""
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"Authorization": "Bearer invalid.token.here"},
        )
        assert resp.status_code == 401

    def test_wrong_signature_rejected(self, auth_client):
        """Token signed with wrong secret should be rejected."""
        wrong_config = AuthConfig(secret="wrong-secret-key-12345")
        wrong_token = TokenService(wrong_config).create_token("user-1")
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"Authorization": f"Bearer {wrong_token}"},
        )
        assert resp.status_code == 401

    def test_register_actor_with_auth(self, auth_client, valid_token):
        """Actor registration should work with valid auth."""
        resp = auth_client.post(
            "/api/v1/actors",
            json={"actor_id": "auth-actor-1", "actor_type": "worker"},
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        assert resp.status_code == 201
        assert resp.json()["actor_id"] == "auth-actor-1"

    def test_send_message_with_auth(self, auth_client, valid_token):
        """Messaging should work with valid auth."""
        # Register actor first
        auth_client.post(
            "/api/v1/actors",
            json={"actor_id": "msg-actor-1"},
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        resp = auth_client.post(
            "/api/v1/actors/msg-actor-1/messages",
            json={
                "source_actor": "test",
                "target_actor": "msg-actor-1",
                "message_type": "default",
                "payload": {"hello": "world"},
            },
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        assert resp.status_code == 202

    def test_state_operations_with_auth(self, auth_client, valid_token):
        """State operations should work with valid auth."""
        # Register actor
        auth_client.post(
            "/api/v1/actors",
            json={"actor_id": "state-actor-1"},
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        # Set state
        resp = auth_client.put(
            "/api/v1/state/state-actor-1/counter",
            json={"value": 42},
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        assert resp.status_code == 200
        assert resp.json()["value"] == 42

    def test_event_sourcing_with_auth(self, auth_client, valid_token):
        """Event sourcing should work with valid auth."""
        resp = auth_client.post(
            "/api/v1/events/append",
            json={
                "aggregate_id": "agg-1",
                "event_type": "created",
                "data": {"name": "test"},
            },
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        assert resp.status_code == 201

    def test_pubsub_with_auth(self, auth_client, valid_token):
        """Pub/sub should work with valid auth."""
        resp = auth_client.post(
            "/api/v1/events/subscribe",
            json={"topic": "test-topic", "subscriber_id": "sub-1"},
            headers={"Authorization": f"Bearer {valid_token}"},
        )
        assert resp.status_code == 201

    def test_no_auth_header(self, auth_client):
        """Request with no auth header at all should return 401."""
        resp = auth_client.get("/api/v1/state/some-actor/key")
        assert resp.status_code == 401

    def test_empty_bearer_token(self, auth_client):
        """Empty Bearer token should return 401."""
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"Authorization": "Bearer "},
        )
        assert resp.status_code == 401

    def test_bearer_with_wrong_prefix(self, auth_client):
        """Non-Bearer auth scheme should return 401."""
        resp = auth_client.get(
            "/api/v1/actors",
            headers={"Authorization": "Basic somecredentials"},
        )
        assert resp.status_code == 401


class TestAuthDisabled:
    """Tests for auth-disabled mode (default behavior)."""

    @pytest.fixture
    def no_auth_client(self):
        """Create a TestClient without auth (default)."""
        from server.app import app
        with TestClient(app) as c:
            yield c

    def test_all_endpoints_accessible(self, no_auth_client):
        """All endpoints should be accessible without auth when disabled."""
        # List actors (no auth needed)
        resp = no_auth_client.get("/api/v1/actors")
        assert resp.status_code == 200

        # Register actor
        resp = no_auth_client.post("/api/v1/actors", json={"actor_id": "noauth-1"})
        assert resp.status_code == 201

        # Health
        resp = no_auth_client.get("/health")
        assert resp.status_code == 200
