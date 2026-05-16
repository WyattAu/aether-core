"""Type stubs for state module."""

from typing import Any, Dict, Optional

class StateHandle:
    """Handle to actor state."""

    _store: Dict[str, bytes]

    def __init__(self) -> None: ...
    async def get(self, key: str) -> Optional[bytes]:
        """Get value by key."""
        ...

    async def set(self, key: str, value: bytes) -> None:
        """Set key to value."""
        ...

    async def delete(self, key: str) -> None:
        """Delete key."""
        ...

    async def get_json(self, key: str) -> Optional[Any]:
        """Get and deserialize JSON value."""
        ...

    async def set_json(self, key: str, value: Any) -> None:
        """Serialize and set JSON value."""
        ...
