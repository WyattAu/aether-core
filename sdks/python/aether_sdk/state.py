from typing import Optional, Dict, Any
import json


class StateHandle:
    """Handle to actor state."""
    
    def __init__(self):
        self._store: Dict[str, bytes] = {}
    
    async def get(self, key: str) -> Optional[bytes]:
        """Get value by key."""
        return self._store.get(key)
    
    async def set(self, key: str, value: bytes) -> None:
        """Set key to value."""
        self._store[key] = value
    
    async def delete(self, key: str) -> None:
        """Delete key."""
        self._store.pop(key, None)
    
    async def get_json(self, key: str) -> Optional[Any]:
        """Get and deserialize JSON value."""
        data = await self.get(key)
        return json.loads(data) if data else None
    
    async def set_json(self, key: str, value: Any) -> None:
        """Serialize and set JSON value."""
        await self.set(key, json.dumps(value).encode())
