import pytest

from server.tracing import TRACING_AVAILABLE, setup_tracing, trace_span, get_trace_id_hex


@pytest.fixture(autouse=True)
def _setup_tracing():
    setup_tracing("test-server")


class TestTracingSetup:
    def test_tracing_module_importable(self):
        from server.tracing import TRACING_AVAILABLE as avail
        assert isinstance(avail, bool)

    def test_setup_tracing_returns_bool(self):
        result = setup_tracing("test-setup")
        assert isinstance(result, bool)


class TestTraceSpan:
    def test_trace_span_context_manager(self):
        with trace_span("test-operation") as span:
            pass

    def test_trace_span_with_attributes(self):
        with trace_span("test-attrs", attributes={"key": "value"}) as span:
            pass

    def test_trace_span_records_error(self):
        import logging
        with pytest.raises(ValueError):
            with trace_span("test-error"):
                raise ValueError("test error")


class TestTraceIdHeader:
    def test_get_trace_id_hex(self):
        result = get_trace_id_hex()
        if TRACING_AVAILABLE:
            with trace_span("inner"):
                result2 = get_trace_id_hex()
        else:
            result2 = None


class TestTracingMiddleware:
    def test_trace_id_in_response_headers(self):
        from fastapi.testclient import TestClient
        from server.app import app
        with TestClient(app) as client:
            resp = client.get("/health")
            assert resp.status_code == 200
            if TRACING_AVAILABLE:
                assert "X-Trace-Id" in resp.headers
                trace_id = resp.headers["X-Trace-Id"]
                assert len(trace_id) == 32
            else:
                assert "X-Request-ID" in resp.headers
