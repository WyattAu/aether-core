"""Type stubs for http module."""

from typing import Any, Dict, Optional, Self

import aiohttp

from .capabilities import CapabilitySet

class HttpClient:
    """HTTP client for actors with NETWORK_OUTBOUND capability."""

    _timeout: aiohttp.ClientTimeout
    _session: Optional[aiohttp.ClientSession]

    def __init__(self, capabilities: CapabilitySet, timeout: float = 30.0) -> None: ...
    async def __aenter__(self) -> Self: ...
    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None: ...
    async def _get_session(self) -> aiohttp.ClientSession: ...
    async def get(
        self, url: str, headers: Optional[Dict[str, str]] = None
    ) -> aiohttp.ClientResponse: ...
    async def post(
        self, url: str, json: Any = None, headers: Optional[Dict[str, str]] = None
    ) -> aiohttp.ClientResponse: ...
    async def put(
        self, url: str, json: Any = None, headers: Optional[Dict[str, str]] = None
    ) -> aiohttp.ClientResponse: ...
    async def delete(
        self, url: str, headers: Optional[Dict[str, str]] = None
    ) -> aiohttp.ClientResponse: ...
    async def close(self) -> None: ...
