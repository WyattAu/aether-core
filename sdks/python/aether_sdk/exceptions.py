"""Exception classes for the Aether SDK.

This module defines all custom exceptions used throughout the Aether SDK.
All exceptions inherit from AetherError, making it easy to catch all
Aether-related errors with a single except clause.

Example:
    >>> from aether_sdk.exceptions import AetherError, CapabilityDenied
    >>> try:
    ...     raise CapabilityDenied("NETWORK_OUTBOUND")
    ... except AetherError as e:
    ...     print(f"Caught: {e}")
    Caught: Capability denied: NETWORK_OUTBOUND
"""

from typing import Optional


class AetherError(Exception):
    """Base exception for all Aether SDK errors.

    All custom exceptions in the Aether SDK inherit from this class,
    allowing callers to catch all Aether-related errors with a single
    except clause.

    Example:
        >>> try:
        ...     # Some Aether operation
        ...     pass
        ... except AetherError as e:
        ...     print(f"Aether error: {e}")
    """

    pass


class CapabilityDenied(AetherError):
    """Raised when an actor attempts an operation without required capability.

    This exception indicates that the actor has not been granted the
    necessary capability to perform the requested operation.

    Args:
        message: Description of the denied capability.

    Example:
        >>> raise CapabilityDenied("HTTP client requires NETWORK_OUTBOUND capability")
    """

    def __init__(self, message: str):
        super().__init__(f"Capability denied: {message}")


class ActorNotFound(AetherError):
    """Raised when attempting to communicate with a non-existent actor.

    This exception is raised when trying to send a message or make an
    RPC call to an actor that doesn't exist or has been stopped.

    Args:
        actor: Name of the actor that was not found.

    Example:
        >>> raise ActorNotFound("my_actor")
    """

    def __init__(self, actor: str):
        super().__init__(f"Actor not found: {actor}")


class StateError(AetherError):
    """Raised when a state operation fails.

    This exception indicates a problem with reading from or writing to
    the actor's state storage, such as serialization failures or
    storage backend errors.

    Example:
        >>> raise StateError("Failed to deserialize state value")
    """

    pass


class RpcError(AetherError):
    """Raised when an RPC call fails.

    This exception is raised when a remote procedure call to another
    actor fails, either due to timeout, an error in the remote actor,
    or other communication issues.

    Args:
        message: Description of the RPC failure.
        code: Optional error code for programmatic error handling.

    Attributes:
        code: An optional error code string for categorizing the error.

    Example:
        >>> raise RpcError("RPC call timed out", code="TIMEOUT")
    """

    def __init__(self, message: str, code: Optional[str] = None):
        super().__init__(message)
        self.code = code
