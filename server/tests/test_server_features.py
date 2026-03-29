"""Integration tests for shutdown, logging, and metrics with the server."""

import json

import pytest
from fastapi.testclient import TestClient


class TestShutdownHealthIntegration:
    """Tests that the health endpoint reflects shutdown state."""

    @pytest.fixture
    def app_with_shutdown(self):
        from server.app import create_app
        from server.config import ServerConfig
        config = ServerConfig()
        app = create_app(config)
        return app

    @pytest.fixture
    def client(self, app_with_shutdown):
        with TestClient(app_with_shutdown) as c:
            yield c

    def test_health_ok_when_running(self, client):
        resp = client.get("/health")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "ok"

    def test_health_draining_when_shutting_down(self, app_with_shutdown):
        with TestClient(app_with_shutdown) as c:
            # Get the shutdown manager created by the lifespan
            shutdown_mgr = app_with_shutdown.state.shutdown_manager
            # Trigger shutdown
            shutdown_mgr.trigger_shutdown("SIGTERM")
            resp = c.get("/health")
            data = resp.json()
            assert data["status"] == "draining"

    def test_info_includes_status(self, client):
        resp = client.get("/api/v1/info")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "ok"
        assert "uptime" in data
        assert "actor_count" in data


class TestMetricsEndpoint:
    """Tests for the /metrics Prometheus endpoint."""

    @pytest.fixture
    def metrics_client(self):
        from server.app import create_app
        from server.config import ServerConfig
        config = ServerConfig(metrics_enabled=True)
        app = create_app(config)
        with TestClient(app) as c:
            yield c

    def test_metrics_endpoint_exists(self, metrics_client):
        resp = metrics_client.get("/metrics")
        assert resp.status_code == 200

    def test_metrics_content_type(self, metrics_client):
        resp = metrics_client.get("/metrics")
        assert "text/plain" in resp.headers["content-type"]

    def test_metrics_has_help_lines(self, metrics_client):
        resp = metrics_client.get("/metrics")
        body = resp.text
        assert "# HELP" in body
        assert "# TYPE" in body

    def test_metrics_disabled_returns_404(self):
        from server.app import create_app
        from server.config import ServerConfig
        config = ServerConfig(metrics_enabled=False)
        app = create_app(config)
        with TestClient(app) as c:
            resp = c.get("/metrics")
            # When disabled, the endpoint isn't registered, FastAPI returns 404
            assert resp.status_code == 404

    def test_metrics_records_requests(self, metrics_client):
        # Make some requests to generate metrics
        metrics_client.get("/health")
        metrics_client.get("/api/v1/actors")
        metrics_client.get("/api/v1/actors")

        resp = metrics_client.get("/metrics")
        body = resp.text
        # Should have request counters for our endpoints
        assert 'path="/health"' in body
        assert 'path="/api/v1/actors"' in body

    def test_metrics_public_path_not_rate_limited(self, metrics_client):
        """Metrics endpoint should bypass rate limiting."""
        for _ in range(20):
            resp = metrics_client.get("/metrics")
            assert resp.status_code == 200


class TestJsonLoggingIntegration:
    """Tests that JSON logging can be enabled via config."""

    def test_json_logging_config_field(self):
        from server.config import ServerConfig
        config = ServerConfig(json_logging_enabled=True)
        assert config.json_logging_enabled is True

    def test_default_json_logging_enabled(self):
        from server.config import ServerConfig
        config = ServerConfig()
        assert config.json_logging_enabled is True

    def test_log_level_config(self):
        from server.config import ServerConfig
        config = ServerConfig(log_level="DEBUG")
        assert config.log_level == "DEBUG"


class TestServerConfigNewFields:
    """Tests for new ServerConfig fields."""

    def test_drain_timeout_default(self):
        from server.config import ServerConfig
        config = ServerConfig()
        assert config.drain_timeout_seconds == 30.0

    def test_drain_timeout_custom(self):
        from server.config import ServerConfig
        config = ServerConfig(drain_timeout_seconds=60.0)
        assert config.drain_timeout_seconds == 60.0

    def test_metrics_enabled_default(self):
        from server.config import ServerConfig
        config = ServerConfig()
        assert config.metrics_enabled is True

    def test_log_level_default(self):
        from server.config import ServerConfig
        config = ServerConfig()
        assert config.log_level == "INFO"
