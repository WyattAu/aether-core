"""Type stubs for exceptions module."""

from typing import Optional

class AetherError(Exception):
    """Base exception for Aether SDK."""

class CapabilityDenied(AetherError):
    """Capability not granted."""

    def __init__(self, message: str) -> None: ...

class ActorNotFound(AetherError):
    """Actor not found."""

    def __init__(self, actor: str) -> None: ...

class StateError(AetherError):
    """State operation failed."""

class RpcError(AetherError):
    """RPC call failed."""

    code: Optional[str]

    def __init__(self, message: str, code: Optional[str] = None) -> None: ...
