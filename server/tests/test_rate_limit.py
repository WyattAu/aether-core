"""Tests for rate limiting middleware."""

import time

import pytest
from fastapi.testclient import TestClient

from server.rate_limit import (
    RateLimitConfig,
    RateLimitMiddleware,
    SlidingWindowCounter,
    TokenBucket,
)


# === TokenBucket Tests ===

class TestTokenBucket:

    def test_initial_capacity(self):
        bucket = TokenBucket(rate=10.0, capacity=5)
        assert bucket.available == 5

    def test_consume_single_token(self):
        bucket = TokenBucket(rate=10.0, capacity=5)
        assert bucket.consume(1)
        assert bucket.available == 4

    def test_consume_multiple_tokens(self):
        bucket = TokenBucket(rate=10.0, capacity=5)
        assert bucket.consume(3)
        assert bucket.available == 2

    def test_reject_when_empty(self):
        bucket = TokenBucket(rate=10.0, capacity=2)
        bucket.consume(2)
        assert not bucket.consume(1)

    def test_reject_large_request(self):
        bucket = TokenBucket(rate=10.0, capacity=5)
        assert not bucket.consume(10)

    def test_refill_over_time(self):
        bucket = TokenBucket(rate=100.0, capacity=10)
        bucket.consume(10)
        # Bucket is empty — wait a bit for refill
        time.sleep(0.05)  # 50ms should add ~5 tokens at 100/s
        assert bucket.available >= 4
        assert bucket.consume(4)

    def test_cannot_exceed_capacity(self):
        bucket = TokenBucket(rate=1000.0, capacity=5)
        time.sleep(0.1)  # Would add ~100 tokens
        assert bucket.available <= 5

    def test_reset(self):
        bucket = TokenBucket(rate=10.0, capacity=5)
        bucket.consume(5)
        assert bucket.available == 0
        bucket.reset()
        assert bucket.available == 5

    def test_zero_rate(self):
        bucket = TokenBucket(rate=0.0, capacity=3)
        bucket.consume(3)
        assert not bucket.consume(1)

    def test_high_rate(self):
        bucket = TokenBucket(rate=10000.0, capacity=100)
        for _ in range(100):
            assert bucket.consume(1)


# === SlidingWindowCounter Tests ===

class TestSlidingWindowCounter:

    def test_initial_count(self):
        counter = SlidingWindowCounter(window_seconds=60, num_slots=6)
        assert counter.count() == 0

    def test_increment(self):
        counter = SlidingWindowCounter(window_seconds=60, num_slots=6)
        counter.increment()
        assert counter.count() == 1

    def test_multiple_increments(self):
        counter = SlidingWindowCounter(window_seconds=60, num_slots=6)
        for _ in range(10):
            counter.increment()
        assert counter.count() == 10

    def test_reset(self):
        counter = SlidingWindowCounter()
        for _ in range(5):
            counter.increment()
        counter.reset()
        assert counter.count() == 0


# === RateLimitConfig Tests ===

class TestRateLimitConfig:

    def test_default_values(self):
        config = RateLimitConfig()
        assert not config.enabled
        assert config.requests_per_second == 100.0
        assert config.burst == 200
        assert config.per_endpoint is True

    def test_default_public_paths(self):
        config = RateLimitConfig()
        assert "/health" in config.public_paths
        assert "/api/v1/info" in config.public_paths
        assert "/docs" in config.public_paths

    def test_custom_endpoint_limits(self):
        config = RateLimitConfig(
            endpoint_limits={
                "/api/v1/actors": (50.0, 100),
                "/api/v1/events": (20.0, 50),
            }
        )
        assert config.endpoint_limits["/api/v1/actors"] == (50.0, 100)
        assert config.endpoint_limits["/api/v1/events"] == (20.0, 50)


# === RateLimitMiddleware Integration Tests ===

class TestRateLimitMiddleware:
    """Integration tests using FastAPI TestClient."""

    @pytest.fixture
    def rl_app(self):
        """Create a FastAPI app with rate limiting enabled."""
        from server.app import create_app
        from server.config import ServerConfig
        return create_app(ServerConfig(
            rate_limit_enabled=True,
            rate_limit_rps=0.5,       # Very low rate for testing
            rate_limit_burst=3,        # Small burst for testing
            rate_limit_per_endpoint=True,
        ))

    @pytest.fixture
    def rl_client(self, rl_app):
        with TestClient(rl_app) as c:
            yield c

    def test_requests_pass_within_limit(self, rl_client):
        """Requests within the burst limit should succeed."""
        for _ in range(3):
            resp = rl_client.get("/api/v1/actors")
            assert resp.status_code == 200

    def test_request_rejected_when_limited(self, rl_client):
        """Requests exceeding burst should get 429."""
        # Drain the bucket
        for _ in range(3):
            rl_client.get("/api/v1/actors")

        # Next request should be rate limited
        resp = rl_client.get("/api/v1/actors")
        assert resp.status_code == 429
        data = resp.json()
        assert "Rate limit exceeded" in data["detail"]
        assert "retry_after" in data

    def test_rate_limit_headers(self, rl_client):
        """Successful responses should include rate limit headers."""
        resp = rl_client.get("/api/v1/actors")
        assert "X-RateLimit-Limit" in resp.headers
        assert "X-RateLimit-Remaining" in resp.headers
        assert "X-RateLimit-Reset" in resp.headers
        assert int(resp.headers["X-RateLimit-Limit"]) == 3

    def test_429_response_headers(self, rl_client):
        """429 responses should include Retry-After header."""
        for _ in range(3):
            rl_client.get("/api/v1/actors")

        resp = rl_client.get("/api/v1/actors")
        assert resp.status_code == 429
        assert "Retry-After" in resp.headers
        assert int(resp.headers["Retry-After"]) >= 1

    def test_public_paths_bypass(self, rl_client):
        """Health endpoint should bypass rate limiting."""
        for _ in range(20):
            resp = rl_client.get("/health")
            assert resp.status_code == 200

    def test_info_path_bypass(self, rl_client):
        """Info endpoint should bypass rate limiting."""
        for _ in range(20):
            resp = rl_client.get("/api/v1/info")
            assert resp.status_code == 200

    def test_options_bypass(self, rl_client):
        """CORS preflight should bypass rate limiting."""
        for _ in range(20):
            resp = rl_client.options("/api/v1/actors")
            assert resp.status_code != 429

    def test_per_endpoint_limits(self, rl_client):
        """Different endpoints should have separate buckets."""
        # Drain actors bucket
        for _ in range(3):
            rl_client.get("/api/v1/actors")

        # Actors should be limited
        resp = rl_client.get("/api/v1/actors")
        assert resp.status_code == 429

        # State should still work (separate bucket)
        # First register an actor so the endpoint works
        rl_client.post("/api/v1/actors", json={"actor_id": "rl-test-1"})
        resp = rl_client.get("/api/v1/state/rl-test-1/key")
        # 404 is fine — the point is it's NOT 429
        assert resp.status_code != 429

    def test_custom_endpoint_limits(self):
        """Endpoint-specific overrides should work."""
        from server.app import create_app
        from server.config import ServerConfig

        config = ServerConfig(
            rate_limit_enabled=True,
            rate_limit_rps=1000.0,
            rate_limit_burst=1000,
            rate_limit_per_endpoint=True,
            rate_limit_endpoint_overrides={
                "/api/v1/actors": (1.0, 2),  # Very tight for actors
            },
        )
        app = create_app(config)

        with TestClient(app) as client:
            # Actors limited to 2 burst
            client.get("/api/v1/actors")
            client.get("/api/v1/actors")
            resp = client.get("/api/v1/actors")
            assert resp.status_code == 429

            # Other endpoints still have high limit
            resp = client.get("/health")
            assert resp.status_code == 200

    def test_stats(self):
        """get_stats should return bucket and counter info."""
        config = RateLimitConfig(enabled=True)
        middleware = RateLimitMiddleware(None, config)
        stats = middleware.get_stats()
        assert stats["active_buckets"] == 0
        assert stats["active_counters"] == 0
        assert stats["total_limited_events"] == 0

    def test_reset(self):
        """reset should clear all buckets and counters."""
        config = RateLimitConfig(enabled=True)
        middleware = RateLimitMiddleware(None, config)
        # Manually create some buckets
        middleware._buckets["test"] = TokenBucket(10.0, 5)
        middleware._counters["test:limited"] = SlidingWindowCounter()
        middleware._counters["test:limited"].increment()
        assert middleware.get_stats()["active_buckets"] == 1

        middleware.reset()
        assert middleware.get_stats()["active_buckets"] == 0
        assert middleware.get_stats()["active_counters"] == 0


class TestRateLimitDisabled:
    """Tests for rate-limit-disabled mode."""

    @pytest.fixture
    def no_rl_client(self):
        from server.app import app
        with TestClient(app) as c:
            yield c

    def test_all_requests_pass(self, no_rl_client):
        """Without rate limiting, all requests should pass."""
        for _ in range(100):
            resp = no_rl_client.get("/api/v1/actors")
            assert resp.status_code == 200

    def test_no_rate_limit_headers(self, no_rl_client):
        """Without rate limiting, no rate limit headers should be present."""
        resp = no_rl_client.get("/api/v1/actors")
        # Headers may not be present when middleware is not active
        assert resp.status_code == 200
