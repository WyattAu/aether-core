"""Tests for the gRPC metrics interceptor."""

import threading
from unittest.mock import patch

import grpc
import pytest

from server.grpc_metrics import MetricsServerInterceptor, _grpc_code_to_http
from server.grpc_server import create_grpc_server
from server.metrics import MetricsCollector
from server.proto.aether.v1 import aether_pb2, aether_pb2_grpc


class TestGrpcCodeToHttp:
    """Test gRPC-to-HTTP status code mapping."""

    def test_ok_maps_to_200(self):
        assert _grpc_code_to_http(grpc.StatusCode.OK) == 200

    def test_not_found_maps_to_404(self):
        assert _grpc_code_to_http(grpc.StatusCode.NOT_FOUND) == 404

    def test_unauthenticated_maps_to_401(self):
        assert _grpc_code_to_http(grpc.StatusCode.UNAUTHENTICATED) == 401

    def test_permission_denied_maps_to_403(self):
        assert _grpc_code_to_http(grpc.StatusCode.PERMISSION_DENIED) == 403

    def test_resource_exhausted_maps_to_429(self):
        assert _grpc_code_to_http(grpc.StatusCode.RESOURCE_EXHAUSTED) == 429

    def test_internal_maps_to_500(self):
        assert _grpc_code_to_http(grpc.StatusCode.INTERNAL) == 500

    def test_unavailable_maps_to_503(self):
        assert _grpc_code_to_http(grpc.StatusCode.UNAVAILABLE) == 503

    def test_unknown_maps_to_500(self):
        assert _grpc_code_to_http(grpc.StatusCode.UNKNOWN) == 500

    def test_all_standard_codes_mapped(self):
        """Every standard gRPC code should map to a valid HTTP status."""
        for code in grpc.StatusCode:
            http_status = _grpc_code_to_http(code)
            assert 200 <= http_status < 600, f"{code.name} mapped to invalid HTTP {http_status}"


class TestMetricsServerInterceptorInit:

    def test_default_init(self):
        interceptor = MetricsServerInterceptor()
        assert interceptor._metrics is None
        assert interceptor._call_counts == {}

    def test_init_with_metrics(self):
        metrics = MetricsCollector()
        interceptor = MetricsServerInterceptor(metrics)
        assert interceptor._metrics is metrics


class TestMetricsServerInterceptorCollect:

    def test_collect_empty(self):
        interceptor = MetricsServerInterceptor()
        output = interceptor.collect_grpc()
        assert "# HELP aether_grpc_calls_total" in output
        assert "# TYPE aether_grpc_calls_total counter" in output

    def test_collect_after_calls(self):
        interceptor = MetricsServerInterceptor()
        interceptor._record("/aether.server.v1.ActorService/Register", grpc.StatusCode.OK, 0.01)
        interceptor._record("/aether.server.v1.ActorService/Register", grpc.StatusCode.OK, 0.02)
        interceptor._record("/aether.server.v1.ActorService/Register", grpc.StatusCode.NOT_FOUND, 0.01)
        output = interceptor.collect_grpc()
        assert 'method="/aether.server.v1.ActorService/Register",code="OK"} 2' in output
        assert 'method="/aether.server.v1.ActorService/Register",code="NOT_FOUND"} 1' in output

    def test_reset_clears_counts(self):
        interceptor = MetricsServerInterceptor()
        interceptor._record("/test/Method", grpc.StatusCode.OK, 0.01)
        interceptor.reset()
        assert interceptor._call_counts == {}


class TestMetricsServerInterceptorWithMetricsCollector:
    """Test that the interceptor correctly delegates to MetricsCollector."""

    def test_record_delegates_to_metrics_collector(self):
        metrics = MetricsCollector()
        interceptor = MetricsServerInterceptor(metrics)

        interceptor._record(
            "/aether.server.v1.HealthService/Health",
            grpc.StatusCode.OK,
            0.005,
        )

        output = metrics.collect()
        assert 'method="GRPC"' in output
        assert 'path="/grpc/HealthService/Health"' in output
        assert 'status="200"' in output

    def test_error_recorded_with_correct_http_status(self):
        metrics = MetricsCollector()
        interceptor = MetricsServerInterceptor(metrics)

        interceptor._record(
            "/aether.server.v1.ActorService/GetActor",
            grpc.StatusCode.NOT_FOUND,
            0.002,
        )

        output = metrics.collect()
        assert 'status="404"' in output

    def test_multiple_methods_tracked_separately(self):
        metrics = MetricsCollector()
        interceptor = MetricsServerInterceptor(metrics)

        interceptor._record("/aether.server.v1.ActorService/Register", grpc.StatusCode.OK, 0.01)
        interceptor._record("/aether.server.v1.StateService/SetState", grpc.StatusCode.OK, 0.02)
        interceptor._record("/aether.server.v1.ActorService/Register", grpc.StatusCode.OK, 0.01)

        output = metrics.collect()
        assert 'path="/grpc/ActorService/Register"' in output
        assert 'path="/grpc/StateService/SetState"' in output


class TestGrpcMethodToPath:

    def test_standard_method(self):
        path = MetricsServerInterceptor._grpc_method_to_path(
            "/aether.server.v1.ActorService/Register"
        )
        assert path == "/grpc/ActorService/Register"

    def test_health_method(self):
        path = MetricsServerInterceptor._grpc_method_to_path(
            "/aether.server.v1.HealthService/Health"
        )
        assert path == "/grpc/HealthService/Health"

    def test_strips_leading_slash(self):
        path = MetricsServerInterceptor._grpc_method_to_path(
            "aether.server.v1.StateService/GetState"
        )
        assert path == "/grpc/StateService/GetState"

    def test_state_service(self):
        path = MetricsServerInterceptor._grpc_method_to_path(
            "/aether.server.v1.StateService/SetState"
        )
        assert path == "/grpc/StateService/SetState"

    def test_event_service(self):
        path = MetricsServerInterceptor._grpc_method_to_path(
            "/aether.server.v1.EventService/Publish"
        )
        assert path == "/grpc/EventService/Publish"

    def test_message_service(self):
        path = MetricsServerInterceptor._grpc_method_to_path(
            "/aether.server.v1.MessageService/Send"
        )
        assert path == "/grpc/MessageService/Send"


class TestMetricsServerInterceptorThreadSafety:

    def test_concurrent_records(self):
        """Verify concurrent _record calls don't corrupt counters."""
        interceptor = MetricsServerInterceptor()
        errors = []

        def recorder(i):
            try:
                for _ in range(50):
                    interceptor._record(f"/test.Service/Method{i}", grpc.StatusCode.OK, 0.001)
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=recorder, args=(i,)) for i in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert not errors, f"Errors during concurrent recording: {errors}"
        # 10 threads * 50 calls = 500 total across 10 unique method paths
        data_lines = [l for l in interceptor.collect_grpc().split("\n")
                       if l.startswith('aether_grpc_calls_total')]
        assert len(data_lines) == 10  # 10 unique methods
        total_count = sum(interceptor._call_counts.values())
        assert total_count == 500


class TestMetricsInterceptorIntegration:
    """Integration tests for the metrics interceptor with a real gRPC server."""

    @pytest.fixture
    def channel_and_metrics(self):
        from server.actor_manager import ActorManager
        from server.event_store import EventStore
        from server.message_router import MessageRouter
        from server.pubsub_service import PubSubService
        from server.state_store import MemoryStateStore
        from server.config import ServerConfig

        config = ServerConfig()
        actors = ActorManager(config)
        messages = MessageRouter(message_ttl=300)
        state = MemoryStateStore()
        pubsub = PubSubService()
        events = EventStore()

        metrics = MetricsCollector()
        interceptor_ref = [None]
        orig_init = MetricsServerInterceptor.__init__

        def capturing_init(self, m=None):
            orig_init(self, m)
            interceptor_ref[0] = self

        with patch.object(MetricsServerInterceptor, '__init__', capturing_init):
            server = create_grpc_server(
                actors, messages, state, pubsub, events,
                max_workers=4,
                metrics=metrics,
            )

        interceptor = interceptor_ref[0]
        port = server.add_insecure_port("localhost:0")
        server.start()
        channel = grpc.insecure_channel(f"localhost:{port}")
        yield channel, metrics, interceptor
        channel.close()
        server.stop(1.0)

    def test_successful_call_recorded(self, channel_and_metrics):
        channel, metrics, interceptor = channel_and_metrics
        stub = aether_pb2_grpc.HealthServiceStub(channel)
        stub.Health(aether_pb2.Empty())
        output = interceptor.collect_grpc()
        assert "OK" in output
        assert "HealthService/Health" in output

    def test_error_call_recorded(self, channel_and_metrics):
        channel, metrics, interceptor = channel_and_metrics
        stub = aether_pb2_grpc.ActorServiceStub(channel)
        try:
            stub.GetActor(aether_pb2.GetActorRequest(actor_id="nonexistent"))
        except grpc.RpcError:
            pass
        output = interceptor.collect_grpc()
        assert "NOT_FOUND" in output
        assert "ActorService/GetActor" in output

    def test_multiple_calls_counted(self, channel_and_metrics):
        channel, metrics, interceptor = channel_and_metrics
        stub = aether_pb2_grpc.HealthServiceStub(channel)
        stub.Health(aether_pb2.Empty())
        stub.Health(aether_pb2.Empty())
        stub.Health(aether_pb2.Empty())
        output = interceptor.collect_grpc()
        assert 'code="OK"} 3' in output

    def test_metrics_delegated_to_collector(self, channel_and_metrics):
        channel, metrics, _ = channel_and_metrics
        stub = aether_pb2_grpc.HealthServiceStub(channel)
        stub.Health(aether_pb2.Empty())
        collector_output = metrics.collect()
        assert 'method="GRPC"' in collector_output
        assert 'path="/grpc/HealthService/Health"' in collector_output
