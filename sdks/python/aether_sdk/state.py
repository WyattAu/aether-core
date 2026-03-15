"""State management for Aether actors.

This module provides the StateHandle class for managing persistent state
within actors. State is stored as key-value pairs where values are bytes,
with convenience methods for JSON serialization.

Example:
    >>> from aether_sdk.state import StateHandle
    >>> state = StateHandle()
    >>> await state.set_json("counter", 0)
    >>> counter = await state.get_json("counter")
    >>> print(counter)
    0
"""

from typing import Optional, Dict, Any
import json


class StateHandle:
    """Handle to actor state storage.
    
    StateHandle provides a key-value store interface for actors to
    persist data across message handling. All values are stored as
    bytes, with convenience methods for JSON serialization.
    
    The state is local to the actor instance and persists for the
    lifetime of the actor. For distributed state, consider using
    external storage systems.
    
    Attributes:
        _store: Internal dictionary storing state values as bytes.
    
    Example:
        >>> state = StateHandle()
        >>> await state.set("key", b"value")
        >>> value = await state.get("key")
        >>> print(value)
        b'value'
    """
    
    def __init__(self):
        """Initialize an empty StateHandle."""
        self._store: Dict[str, bytes] = {}
    
    async def get(self, key: str) -> Optional[bytes]:
        """Get a value by key.
        
        Args:
            key: The key to look up.
        
        Returns:
            The stored bytes value, or None if the key doesn't exist.
        
        Example:
            >>> await state.set("name", b"Aether")
            >>> await state.get("name")
            b'Aether'
        """
        return self._store.get(key)
    
    async def set(self, key: str, value: bytes) -> None:
        """Set a key to a bytes value.
        
        Args:
            key: The key to set.
            value: The bytes value to store.
        
        Example:
            >>> await state.set("data", b"\\x00\\x01\\x02")
        """
        self._store[key] = value
    
    async def delete(self, key: str) -> None:
        """Delete a key from state.
        
        Args:
            key: The key to delete.
        
        Note:
            Silently does nothing if the key doesn't exist.
        
        Example:
            >>> await state.set("temp", b"value")
            >>> await state.delete("temp")
            >>> await state.get("temp")
            None
        """
        self._store.pop(key, None)
    
    async def get_json(self, key: str) -> Optional[Any]:
        """Get and deserialize a JSON value.
        
        Args:
            key: The key to look up.
        
        Returns:
            The deserialized JSON value, or None if the key doesn't exist.
        
        Raises:
            json.JSONDecodeError: If the stored value is not valid JSON.
        
        Example:
            >>> await state.set_json("config", {"enabled": True})
            >>> await state.get_json("config")
            {'enabled': True}
        """
        data = await self.get(key)
        return json.loads(data) if data else None
    
    async def set_json(self, key: str, value: Any) -> None:
        """Serialize and store a JSON value.
        
        Args:
            key: The key to set.
            value: Any JSON-serializable value.
        
        Example:
            >>> await state.set_json("items", [1, 2, 3])
            >>> await state.get("items")
            b'[1, 2, 3]'
        """
        await self.set(key, json.dumps(value).encode())
