import pytest
from aether_sdk.exceptions import (
    ActorNotFound,
    AetherError,
    CapabilityDenied,
    RpcError,
    StateError,
)


class TestAetherError:
    """Tests for base AetherError exception."""

    def test_is_exception(self):
        """AetherError should be an Exception."""
        assert issubclass(AetherError, Exception)

    def test_can_be_raised(self):
        """AetherError should be raisable."""
        with pytest.raises(AetherError):
            raise AetherError("test error")

    def test_message_preserved(self):
        """AetherError should preserve error message."""
        error = AetherError("test error message")
        assert "test error message" in str(error)


class TestCapabilityDenied:
    """Tests for CapabilityDenied exception."""

    def test_is_aether_error(self):
        """CapabilityDenied should be an AetherError."""
        assert issubclass(CapabilityDenied, AetherError)

    def test_can_be_raised(self):
        """CapabilityDenied should be raisable."""
        with pytest.raises(CapabilityDenied):
            raise CapabilityDenied("NETWORK_OUTBOUND")

    def test_message_format(self):
        """CapabilityDenied should format message with 'Capability denied:' prefix."""
        error = CapabilityDenied("NETWORK_OUTBOUND required")
        assert "Capability denied:" in str(error)
        assert "NETWORK_OUTBOUND required" in str(error)

    def test_catches_as_aether_error(self):
        """CapabilityDenied should be catchable as AetherError."""
        with pytest.raises(AetherError):
            raise CapabilityDenied("test")


class TestActorNotFound:
    """Tests for ActorNotFound exception."""

    def test_is_aether_error(self):
        """ActorNotFound should be an AetherError."""
        assert issubclass(ActorNotFound, AetherError)

    def test_can_be_raised(self):
        """ActorNotFound should be raisable."""
        with pytest.raises(ActorNotFound):
            raise ActorNotFound("my_actor")

    def test_message_format(self):
        """ActorNotFound should format message with actor name."""
        error = ActorNotFound("my_actor")
        assert "Actor not found:" in str(error)
        assert "my_actor" in str(error)

    def test_catches_as_aether_error(self):
        """ActorNotFound should be catchable as AetherError."""
        with pytest.raises(AetherError):
            raise ActorNotFound("test_actor")


class TestStateError:
    """Tests for StateError exception."""

    def test_is_aether_error(self):
        """StateError should be an AetherError."""
        assert issubclass(StateError, AetherError)

    def test_can_be_raised(self):
        """StateError should be raisable."""
        with pytest.raises(StateError):
            raise StateError("Failed to read state")

    def test_message_preserved(self):
        """StateError should preserve error message."""
        error = StateError("Key not found")
        assert "Key not found" in str(error)

    def test_catches_as_aether_error(self):
        """StateError should be catchable as AetherError."""
        with pytest.raises(AetherError):
            raise StateError("test")


class TestRpcError:
    """Tests for RpcError exception."""

    def test_is_aether_error(self):
        """RpcError should be an AetherError."""
        assert issubclass(RpcError, AetherError)

    def test_can_be_raised(self):
        """RpcError should be raisable."""
        with pytest.raises(RpcError):
            raise RpcError("RPC call failed")

    def test_message_preserved(self):
        """RpcError should preserve error message."""
        error = RpcError("Connection timeout")
        assert "Connection timeout" in str(error)

    def test_code_attribute(self):
        """RpcError should have code attribute."""
        error = RpcError("Timeout", code="TIMEOUT")
        assert error.code == "TIMEOUT"

    def test_code_defaults_to_none(self):
        """RpcError code should default to None."""
        error = RpcError("Generic error")
        assert error.code is None

    def test_catches_as_aether_error(self):
        """RpcError should be catchable as AetherError."""
        with pytest.raises(AetherError):
            raise RpcError("test")

    def test_error_codes_can_be_distinguished(self):
        """Different RpcError codes should be distinguishable."""
        timeout_error = RpcError("Timeout", code="TIMEOUT")
        not_found_error = RpcError("Not found", code="NOT_FOUND")

        assert timeout_error.code == "TIMEOUT"
        assert not_found_error.code == "NOT_FOUND"
        assert timeout_error.code != not_found_error.code


class TestExceptionHierarchy:
    """Tests for exception inheritance hierarchy."""

    def test_all_exceptions_inherit_from_aether_error(self):
        """All custom exceptions should inherit from AetherError."""
        exceptions = [CapabilityDenied, ActorNotFound, StateError, RpcError]
        for exc_class in exceptions:
            assert issubclass(
                exc_class, AetherError
            ), f"{exc_class.__name__} should inherit from AetherError"

    def test_all_exceptions_inherit_from_exception(self):
        """All custom exceptions should ultimately inherit from Exception."""
        exceptions = [
            AetherError,
            CapabilityDenied,
            ActorNotFound,
            StateError,
            RpcError,
        ]
        for exc_class in exceptions:
            assert issubclass(
                exc_class, Exception
            ), f"{exc_class.__name__} should inherit from Exception"

    def test_catch_all_with_aether_error(self):
        """All AetherError subclasses should be catchable with AetherError."""
        errors = [
            CapabilityDenied("test"),
            ActorNotFound("test"),
            StateError("test"),
            RpcError("test"),
        ]

        for error in errors:
            assert isinstance(
                error, AetherError
            ), f"{type(error).__name__} should be instance of AetherError"
