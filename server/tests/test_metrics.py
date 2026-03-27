"""Tests for Prometheus-compatible metrics."""

import re

import pytest

from server.metrics import MetricsCollector, DEFAULT_BUCKETS


class TestMetricsCollectorRequestCounting:

    def test_single_request(self):
        m = MetricsCollector()
        m.observe_request("GET", "/api/v1/actors", 200, 0.042)
        output = m.collect()
        assert 'method="GET"' in output
        assert 'path="/api/v1/actors"' in output
        assert 'status="200"' in output
        assert " 1\n" in output  # count of 1

    def test_multiple_requests_increment(self):
        m = MetricsCollector()
        m.observe_request("GET", "/api/v1/actors", 200, 0.01)
        m.observe_request("GET", "/api/v1/actors", 200, 0.02)
        m.observe_request("POST", "/api/v1/actors", 201, 0.03)
        output = m.collect()
        # GET /actors 200 should be 2
        assert re.search(r'aether_http_requests_total\{method="GET",path="/api/v1/actors",status="200"\} 2', output)
        # POST /actors 201 should be 1
        assert re.search(r'aether_http_requests_total\{method="POST",path="/api/v1/actors",status="201"\} 1', output)

    def test_different_status_codes_tracked_separately(self):
        m = MetricsCollector()
        m.observe_request("GET", "/api/v1/actors", 200, 0.01)
        m.observe_request("GET", "/api/v1/actors", 404, 0.01)
        m.observe_request("GET", "/api/v1/actors", 429, 0.01)
        output = m.collect()
        assert 'status="200"' in output
        assert 'status="404"' in output
        assert 'status="429"' in output


class TestMetricsCollectorHistogram:

    def test_histogram_buckets_cumulative(self):
        m = MetricsCollector(buckets=(0.01, 0.05, 0.1))
        m.observe_request("GET", "/health", 200, 0.005)  # <= 0.01
        m.observe_request("GET", "/health", 200, 0.03)   # <= 0.05
        m.observe_request("GET", "/health", 200, 0.08)   # <= 0.1
        output = m.collect()
        # le=0.01: 1
        assert re.search(r'le="0.01"\} 1', output)
        # le=0.05: 2 (cumulative)
        assert re.search(r'le="0.05"\} 2', output)
        # le=0.1: 3 (cumulative)
        assert re.search(r'le="0.1"\} 3', output)
        # le=+Inf: 3
        assert re.search(r'le="\+Inf"\} 3', output)
        # _count: 3
        assert re.search(r'_count.*\} 3', output)

    def test_histogram_sum(self):
        m = MetricsCollector(buckets=(0.1,))
        m.observe_request("GET", "/health", 200, 0.02)
        m.observe_request("GET", "/health", 200, 0.03)
        output = m.collect()
        assert re.search(r'_sum\{.*\} 0.050000', output)

    def test_empty_histogram(self):
        m = MetricsCollector()
        output = m.collect()
        # Should have HELP and TYPE but no data lines for histogram
        assert "aether_http_request_duration_seconds" in output


class TestMetricsCollectorGauges:

    def test_actor_count_gauge(self):
        m = MetricsCollector()
        m.set_actor_count(42)
        output = m.collect()
        assert "aether_active_actors 42" in output

    def test_actor_count_updates(self):
        m = MetricsCollector()
        m.set_actor_count(10)
        m.set_actor_count(20)
        output = m.collect()
        assert "aether_active_actors 20" in output
        # Should NOT have 10
        assert "aether_active_actors 10\n" not in output


class TestMetricsCollectorCounters:

    def test_message_counter(self):
        m = MetricsCollector()
        m.inc_messages()
        m.inc_messages()
        m.inc_messages(5)
        output = m.collect()
        assert "aether_messages_total 7" in output

    def test_rate_limit_counter(self):
        m = MetricsCollector()
        m.inc_rate_limit_rejections()
        m.inc_rate_limit_rejections(3)
        output = m.collect()
        assert "aether_rate_limit_rejections_total 4" in output


class TestMetricsCollectorPathNormalization:

    def test_strips_trailing_slash(self):
        m = MetricsCollector()
        m.observe_request("GET", "/api/v1/actors/", 200, 0.01)
        output = m.collect()
        assert 'path="/api/v1/actors"' in output
        assert 'path="/api/v1/actors/"' not in output

    def test_replaces_uuids(self):
        m = MetricsCollector()
        uuid_path = "/api/v1/actors/550e8400-e29b-41d4-a716-446655440000"
        m.observe_request("GET", uuid_path, 200, 0.01)
        output = m.collect()
        assert 'path="/api/v1/actors/:id"' in output

    def test_replaces_long_numeric_ids(self):
        m = MetricsCollector()
        m.observe_request("GET", "/api/v1/state/123456789/key", 200, 0.01)
        output = m.collect()
        assert 'path="/api/v1/state/:id/key"' in output

    def test_keeps_short_paths_unchanged(self):
        m = MetricsCollector()
        m.observe_request("GET", "/api/v1/actors", 200, 0.01)
        output = m.collect()
        assert 'path="/api/v1/actors"' in output


class TestMetricsCollectorOutput:

    def test_output_has_help_and_type(self):
        m = MetricsCollector()
        output = m.collect()
        assert "# HELP aether_http_requests_total" in output
        assert "# TYPE aether_http_requests_total counter" in output
        assert "# HELP aether_http_request_duration_seconds" in output
        assert "# TYPE aether_http_request_duration_seconds histogram" in output
        assert "# HELP aether_active_actors" in output
        assert "# TYPE aether_active_actors gauge" in output
        assert "# HELP aether_messages_total" in output
        assert "# TYPE aether_messages_total counter" in output
        assert "# HELP aether_rate_limit_rejections_total" in output
        assert "# TYPE aether_rate_limit_rejections_total counter" in output

    def test_empty_output_is_valid(self):
        m = MetricsCollector()
        output = m.collect()
        # Should be a valid Prometheus format with no data values
        assert output.startswith("#")

    def test_reset_clears_all(self):
        m = MetricsCollector()
        m.observe_request("GET", "/test", 200, 0.01)
        m.inc_messages()
        m.set_actor_count(5)
        m.reset()
        output = m.collect()
        # Should only have HELP/TYPE lines, no data
        assert "aether_active_actors 5" not in output
        assert "aether_messages_total 1" not in output


class TestMetricsCollectorThreadSafety:

    def test_concurrent_updates(self):
        import threading
        m = MetricsCollector()
        errors = []

        def writer():
            try:
                for i in range(100):
                    m.observe_request("GET", "/test", 200, 0.01 * i)
                    m.inc_messages()
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=writer) for _ in range(5)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert not errors
        output = m.collect()
        assert "aether_messages_total 500" in output


class TestMetricsCollectorCustomBuckets:

    def test_custom_buckets(self):
        m = MetricsCollector(buckets=(0.1, 1.0, 10.0))
        m.observe_request("GET", "/test", 200, 0.5)
        output = m.collect()
        assert 'le="0.1"' in output
        assert 'le="1.0"' in output
        assert 'le="10.0"' in output
