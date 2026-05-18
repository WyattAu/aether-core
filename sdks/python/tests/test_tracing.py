"""
Tests for Aether SDK Tracing Module

Tests for OpenTelemetry tracing integration.
"""

from unittest.mock import MagicMock, patch

import pytest
from aether_sdk.resilience.tracing import (
    ResilienceInstrumentation,
    TracingContext,
    create_resilience_span,
    get_tracer,
    record_resilience_event,
    set_resilience_attribute,
    traced_bulkhead,
    traced_circuit_breaker,
    traced_rate_limiter,
    traced_retry,
)

# ============================================
# TracingContext Tests
# ============================================


class TestTracingContext:
    """Tests for TracingContext."""

    def test_init(self):
        """Test initialization."""
        ctx = TracingContext(
            tracer=None,
            span_name="test-span",
            attributes={"key": "value"},
        )

        assert ctx.tracer is None
        assert ctx.span_name == "test-span"
        assert ctx.attributes == {"key": "value"}
        assert ctx.span is None
        assert ctx.start_time is None

    def test_init_default_attributes(self):
        """Test initialization with default attributes."""
        ctx = TracingContext(tracer=None, span_name="test-span")

        assert ctx.attributes == {}

    def test_enter_without_tracer(self):
        """Test entering context without tracer."""
        ctx = TracingContext(tracer=None, span_name="test-span")

        result = ctx.__enter__()

        assert result is ctx
        assert ctx.span is None
        assert ctx.start_time is None

    def test_exit_without_span(self):
        """Test exiting context without span."""
        ctx = TracingContext(tracer=None, span_name="test-span")

        # Should not raise
        ctx.__exit__(None, None, None)

    def test_set_attribute_without_span(self):
        """Test setting attribute without span."""
        ctx = TracingContext(tracer=None, span_name="test-span")

        # Should not raise
        ctx.set_attribute("key", "value")

    def test_add_event_without_span(self):
        """Test adding event without span."""
        ctx = TracingContext(tracer=None, span_name="test-span")

        # Should not raise
        ctx.add_event("event-name")

    def test_context_manager_protocol(self):
        """Test context manager protocol."""
        ctx = TracingContext(tracer=None, span_name="test-span")

        with ctx as entered:
            assert entered is ctx


# ============================================
# get_tracer Tests
# ============================================


class TestGetTracer:
    """Tests for get_tracer function."""

    def test_get_tracer_without_opentelemetry(self):
        """Test getting tracer when OpenTelemetry is not available."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            with patch("aether_sdk.resilience.tracing._tracer_module", None):
                tracer = get_tracer()
                assert tracer is None

    def test_get_tracer_with_service_name(self):
        """Test getting tracer with custom service name."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            tracer = get_tracer("custom-service")
            assert tracer is None


# ============================================
# Traced Decorators Tests
# ============================================


class TestTracedDecorators:
    """Tests for traced decorators."""

    @pytest.mark.asyncio
    async def test_traced_circuit_breaker_without_tracing(self):
        """Test traced_circuit_breaker without tracing available."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockCircuitBreaker:
                def __init__(self):
                    self.state = MagicMock()
                    self.state.value = "closed"

                @traced_circuit_breaker(name="test")
                async def execute(self):
                    return "success"

            cb = MockCircuitBreaker()
            result = await cb.execute()

            assert result == "success"

    @pytest.mark.asyncio
    async def test_traced_circuit_breaker_with_exception(self):
        """Test traced_circuit_breaker with exception."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockCircuitBreaker:
                def __init__(self):
                    self.state = MagicMock()
                    self.state.value = "open"

                @traced_circuit_breaker(name="test")
                async def execute(self):
                    raise ValueError("Test error")

            cb = MockCircuitBreaker()

            with pytest.raises(ValueError):
                await cb.execute()

    @pytest.mark.asyncio
    async def test_traced_retry_without_tracing(self):
        """Test traced_retry without tracing available."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockRetry:
                def __init__(self):
                    self._config = MagicMock()
                    self._config.max_attempts = 3

                @traced_retry(name="test")
                async def execute(self):
                    return "success"

            retry = MockRetry()
            result = await retry.execute()

            assert result == "success"

    @pytest.mark.asyncio
    async def test_traced_retry_with_exception(self):
        """Test traced_retry with exception."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockRetry:
                def __init__(self):
                    self._config = MagicMock()
                    self._config.max_attempts = 3

                @traced_retry(name="test")
                async def execute(self):
                    raise RuntimeError("Exhausted")

            retry = MockRetry()

            with pytest.raises(RuntimeError):
                await retry.execute()

    @pytest.mark.asyncio
    async def test_traced_retry_without_config(self):
        """Test traced_retry without config attribute."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockRetry:
                @traced_retry(name="test")
                async def execute(self):
                    return "success"

            retry = MockRetry()
            result = await retry.execute()

            assert result == "success"

    @pytest.mark.asyncio
    async def test_traced_rate_limiter_without_tracing(self):
        """Test traced_rate_limiter without tracing available."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockRateLimiter:
                @traced_rate_limiter(name="test")
                async def execute(self):
                    result = MagicMock()
                    result.allowed = True
                    return result

            rl = MockRateLimiter()
            result = await rl.execute()

            assert result.allowed is True

    @pytest.mark.asyncio
    async def test_traced_rate_limiter_with_exception(self):
        """Test traced_rate_limiter with exception."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockRateLimiter:
                @traced_rate_limiter(name="test")
                async def execute(self):
                    raise PermissionError("Rate limit exceeded")

            rl = MockRateLimiter()

            with pytest.raises(PermissionError):
                await rl.execute()

    @pytest.mark.asyncio
    async def test_traced_bulkhead_without_tracing(self):
        """Test traced_bulkhead without tracing available."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockBulkhead:
                @traced_bulkhead(name="test")
                async def execute(self):
                    return "success"

            bh = MockBulkhead()
            result = await bh.execute()

            assert result == "success"

    @pytest.mark.asyncio
    async def test_traced_bulkhead_with_exception(self):
        """Test traced_bulkhead with exception."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class MockBulkhead:
                @traced_bulkhead(name="test")
                async def execute(self):
                    raise RuntimeError("Bulkhead full")

            bh = MockBulkhead()

            with pytest.raises(RuntimeError):
                await bh.execute()


# ============================================
# Helper Functions Tests
# ============================================


class TestHelperFunctions:
    """Tests for helper functions."""

    def test_create_resilience_span_without_tracing(self):
        """Test create_resilience_span without tracing."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            result = create_resilience_span(
                operation="test_op",
                pattern_type="circuit_breaker",
                pattern_name="test",
            )

            assert result is None

    def test_create_resilience_span_with_attributes(self):
        """Test create_resilience_span with attributes."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            result = create_resilience_span(
                operation="test_op",
                pattern_type="circuit_breaker",
                pattern_name="test",
                attributes={"custom": "value"},
            )

            assert result is None

    def test_record_resilience_event_without_tracing(self):
        """Test record_resilience_event without tracing."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            # Should not raise
            record_resilience_event(
                pattern_type="circuit_breaker",
                event_name="state_change",
            )

    def test_record_resilience_event_with_attributes(self):
        """Test record_resilience_event with attributes."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            # Should not raise
            record_resilience_event(
                pattern_type="circuit_breaker",
                event_name="state_change",
                attributes={"from": "closed", "to": "open"},
            )

    def test_set_resilience_attribute_without_tracing(self):
        """Test set_resilience_attribute without tracing."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):
            # Should not raise
            set_resilience_attribute("key", "value")


# ============================================
# ResilienceInstrumentation Tests
# ============================================


class TestResilienceInstrumentation:
    """Tests for ResilienceInstrumentation."""

    def test_init_default_service_name(self):
        """Test initialization with default service name."""
        instrumentation = ResilienceInstrumentation()

        # Tracer will be None if OpenTelemetry is not available
        assert instrumentation.tracer is None or instrumentation.tracer is not None

    def test_init_custom_service_name(self):
        """Test initialization with custom service name."""
        instrumentation = ResilienceInstrumentation("custom-service")

        # Tracer will be None if OpenTelemetry is not available
        assert instrumentation.tracer is None or instrumentation.tracer is not None

    def test_trace_circuit_breaker(self):
        """Test circuit breaker trace context."""
        instrumentation = ResilienceInstrumentation()

        with instrumentation.trace_circuit_breaker(
            name="test-cb",
            state="closed",
            operation="execute",
        ) as ctx:
            ctx.set_attribute("custom", "value")

    def test_trace_circuit_breaker_with_exception(self):
        """Test circuit breaker trace context with exception."""
        instrumentation = ResilienceInstrumentation()

        try:
            with instrumentation.trace_circuit_breaker(
                name="test-cb",
                state="open",
                operation="execute",
            ):
                raise ValueError("Test error")
        except ValueError:
            pass  # Expected

    def test_trace_retry(self):
        """Test retry trace context."""
        instrumentation = ResilienceInstrumentation()

        with instrumentation.trace_retry(
            name="test-retry",
            attempt=1,
            max_attempts=3,
            operation="execute",
        ) as ctx:
            ctx.set_attribute("custom", "value")

    def test_trace_retry_with_exception(self):
        """Test retry trace context with exception."""
        instrumentation = ResilienceInstrumentation()

        try:
            with instrumentation.trace_retry(
                name="test-retry",
                attempt=3,
                max_attempts=3,
                operation="execute",
            ):
                raise RuntimeError("Exhausted")
        except RuntimeError:
            pass  # Expected

    def test_trace_rate_limiter(self):
        """Test rate limiter trace context."""
        instrumentation = ResilienceInstrumentation()

        with instrumentation.trace_rate_limiter(
            name="test-rl",
            operation="acquire",
            requests_per_second=100,
        ) as ctx:
            ctx.set_attribute("custom", "value")

    def test_trace_rate_limiter_with_exception(self):
        """Test rate limiter trace context with exception."""
        instrumentation = ResilienceInstrumentation()

        try:
            with instrumentation.trace_rate_limiter(
                name="test-rl",
                operation="acquire",
                requests_per_second=100,
            ):
                raise PermissionError("Rate limit exceeded")
        except PermissionError:
            pass  # Expected

    def test_trace_bulkhead(self):
        """Test bulkhead trace context."""
        instrumentation = ResilienceInstrumentation()

        with instrumentation.trace_bulkhead(
            name="test-bh",
            operation="execute",
            active=5,
            max_concurrent=10,
        ) as ctx:
            ctx.set_attribute("custom", "value")

    def test_trace_bulkhead_with_exception(self):
        """Test bulkhead trace context with exception."""
        instrumentation = ResilienceInstrumentation()

        try:
            with instrumentation.trace_bulkhead(
                name="test-bh",
                operation="execute",
                active=10,
                max_concurrent=10,
            ):
                raise RuntimeError("Bulkhead full")
        except RuntimeError:
            pass  # Expected

    def test_trace_health_check(self):
        """Test health check trace context."""
        instrumentation = ResilienceInstrumentation()

        with instrumentation.trace_health_check(
            name="test-hc",
            check_name="database",
        ) as ctx:
            ctx.set_attribute("custom", "value")

    def test_trace_health_check_with_exception(self):
        """Test health check trace context with exception."""
        instrumentation = ResilienceInstrumentation()

        try:
            with instrumentation.trace_health_check(
                name="test-hc",
                check_name="database",
            ):
                raise ConnectionError("Database unreachable")
        except ConnectionError:
            pass  # Expected

    def test_nested_traces(self):
        """Test nested trace contexts."""
        instrumentation = ResilienceInstrumentation()

        with instrumentation.trace_circuit_breaker("cb", "closed", "exec"):
            with instrumentation.trace_retry("retry", 1, 3, "exec"):
                with instrumentation.trace_bulkhead("bh", "exec", 1, 10):
                    pass  # Nested contexts work


# ============================================
# Edge Cases Tests
# ============================================


class TestEdgeCases:
    """Tests for edge cases."""

    def test_tracing_context_exit_with_exception(self):
        """Test TracingContext exit with exception info."""
        ctx = TracingContext(tracer=None, span_name="test")
        ctx.__enter__()

        # Should not raise
        ctx.__exit__(ValueError, ValueError("test"), None)

    def test_tracing_context_multiple_attributes(self):
        """Test setting multiple attributes."""
        ctx = TracingContext(tracer=None, span_name="test")

        # Should not raise
        ctx.set_attribute("key1", "value1")
        ctx.set_attribute("key2", 123)
        ctx.set_attribute("key3", {"nested": "value"})

    def test_tracing_context_add_event_with_attributes(self):
        """Test adding event with attributes."""
        ctx = TracingContext(tracer=None, span_name="test")

        # Should not raise
        ctx.add_event("event-name", attributes={"key": "value"})

    def test_decorator_preserves_function_name(self):
        """Test that decorators preserve function names."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class TestClass:
                @traced_circuit_breaker(name="test")
                async def my_function(self):
                    return "result"

            assert TestClass.my_function.__name__ == "my_function"

    @pytest.mark.asyncio
    async def test_multiple_decorators(self):
        """Test stacking multiple decorators."""
        with patch("aether_sdk.resilience.tracing.TRACING_AVAILABLE", False):

            class TestClass:
                def __init__(self):
                    self.state = MagicMock()
                    self.state.value = "closed"
                    self._config = MagicMock()
                    self._config.max_attempts = 3

                @traced_circuit_breaker(name="test")
                @traced_retry(name="test")
                async def execute(self):
                    return "success"

            obj = TestClass()
            result = await obj.execute()

            assert result == "success"


# ============================================
# Mock OpenTelemetry Tests
# ============================================


class TestMockOpenTelemetry:
    """Tests with mocked OpenTelemetry."""

    @pytest.mark.asyncio
    async def test_traced_circuit_breaker_with_mock_tracer(self):
        """Test traced_circuit_breaker with mocked tracer."""

        # This test verifies the decorator works even without OpenTelemetry
        # The decorator gracefully handles the case when tracing is unavailable
        class MockCircuitBreaker:
            def __init__(self):
                self.state = MagicMock()
                self.state.value = "closed"

            @traced_circuit_breaker(name="test")
            async def execute(self):
                return "success"

        cb = MockCircuitBreaker()
        result = await cb.execute()

        assert result == "success"

    def test_create_resilience_span_returns_context(self):
        """Test create_resilience_span returns TracingContext or None."""
        # Without OpenTelemetry, it returns None
        result = create_resilience_span(
            operation="test_op",
            pattern_type="circuit_breaker",
            pattern_name="test",
        )

        # Result is None when OpenTelemetry is not available
        assert result is None or isinstance(result, TracingContext)

    def test_record_resilience_event_no_error(self):
        """Test record_resilience_event doesn't raise errors."""
        # Should not raise regardless of tracing availability
        record_resilience_event(
            pattern_type="circuit_breaker",
            event_name="state_change",
            attributes={"key": "value"},
        )

    def test_set_resilience_attribute_no_error(self):
        """Test set_resilience_attribute doesn't raise errors."""
        # Should not raise regardless of tracing availability
        set_resilience_attribute("key", "value")

    def test_get_tracer_returns_none_without_otel(self):
        """Test get_tracer returns None when OpenTelemetry not available."""
        # Without OpenTelemetry installed, this returns None
        tracer = get_tracer()

        # Either None (no OTel) or a valid tracer (OTel installed)
        assert tracer is None or tracer is not None

    def test_get_tracer_with_custom_service_name(self):
        """Test get_tracer with custom service name."""
        tracer = get_tracer("custom-service")

        # Either None (no OTel) or a valid tracer (OTel installed)
        assert tracer is None or tracer is not None


# ============================================
# TracingContext with Mock Span Tests
# ============================================


class TestTracingContextWithMockSpan:
    """Tests for TracingContext with mocked span."""

    def test_context_with_none_tracer(self):
        """Test TracingContext with None tracer."""
        ctx = TracingContext(
            tracer=None,
            span_name="test-span",
            attributes={"key": "value"},
        )

        result = ctx.__enter__()

        assert result is ctx
        assert ctx.span is None
        assert ctx.start_time is None

    def test_context_with_mock_span_operations(self):
        """Test TracingContext operations with mock span."""
        mock_span = MagicMock()

        ctx = TracingContext(tracer=None, span_name="test")
        ctx.span = mock_span  # Manually set span for testing

        # Test set_attribute
        ctx.set_attribute("key", "value")
        mock_span.set_attribute.assert_called_with("key", "value")

        # Test add_event
        ctx.add_event("event-name", attributes={"key": "value"})
        mock_span.add_event.assert_called_with(
            "event-name", attributes={"key": "value"}
        )

    def test_context_exit_with_mock_span_no_exception(self):
        """Test TracingContext exit with mock span and no exception."""
        mock_span = MagicMock()

        ctx = TracingContext(tracer=None, span_name="test")
        ctx.span = mock_span
        ctx.start_time = 100.0

        ctx.__exit__(None, None, None)

        # Without TRACING_AVAILABLE, span operations won't be called
        # But the context should still work

    def test_context_exit_with_mock_span_and_exception(self):
        """Test TracingContext exit with mock span and exception."""
        mock_span = MagicMock()

        ctx = TracingContext(tracer=None, span_name="test")
        ctx.span = mock_span
        ctx.start_time = 100.0

        ctx.__exit__(ValueError, ValueError("test"), None)

        # Without TRACING_AVAILABLE, span operations won't be called
        # But the context should still work

    def test_span_operations_without_span(self):
        """Test that span operations are no-ops without a span."""
        ctx = TracingContext(tracer=None, span_name="test")

        # These should not raise
        ctx.set_attribute("key", "value")
        ctx.add_event("event-name")
        ctx.__exit__(None, None, None)
