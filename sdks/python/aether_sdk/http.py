import aiohttp
from typing import Optional, Dict, Any
from .capabilities import Capability, CapabilitySet
from .exceptions import CapabilityDenied


class HttpClient:
    """HTTP client for actors with NETWORK_OUTBOUND capability."""
    
    def __init__(self, capabilities: CapabilitySet, timeout: float = 30.0):
        if not capabilities.has(Capability.NETWORK_OUTBOUND):
            raise CapabilityDenied("HTTP client requires NETWORK_OUTBOUND capability")
        self._timeout = aiohttp.ClientTimeout(total=timeout)
        self._session: Optional[aiohttp.ClientSession] = None
    
    async def _get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(timeout=self._timeout)
        return self._session
    
    async def get(self, url: str, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        session = await self._get_session()
        return await session.get(url, headers=headers)
    
    async def post(self, url: str, json: Any = None, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        session = await self._get_session()
        return await session.post(url, json=json, headers=headers)
    
    async def put(self, url: str, json: Any = None, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        session = await self._get_session()
        return await session.put(url, json=json, headers=headers)
    
    async def delete(self, url: str, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        session = await self._get_session()
        return await session.delete(url, headers=headers)
    
    async def close(self) -> None:
        if self._session:
            await self._session.close()
