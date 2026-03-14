from typing import Optional


class AetherError(Exception):
    """Base exception for Aether SDK."""
    pass


class CapabilityDenied(AetherError):
    """Capability not granted."""
    def __init__(self, message: str):
        super().__init__(f"Capability denied: {message}")


class ActorNotFound(AetherError):
    """Actor not found."""
    def __init__(self, actor: str):
        super().__init__(f"Actor not found: {actor}")


class StateError(AetherError):
    """State operation failed."""
    pass


class RpcError(AetherError):
    """RPC call failed."""
    def __init__(self, message: str, code: Optional[str] = None):
        super().__init__(message)
        self.code = code
