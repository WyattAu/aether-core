"""HTTP client for Aether actors.

This module provides :class:`HttpClient`, a thin async wrapper around
``aiohttp`` that enforces the :data:`NETWORK_OUTBOUND
<aether_sdk.capabilities.Capability.NETWORK_OUTBOUND>` capability check
before allowing any HTTP requests.

Example:
    >>> from aether_sdk.http import HttpClient
    >>> from aether_sdk.capabilities import Capability, CapabilitySet
    >>> caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
    >>> async with HttpClient(caps) as client:
    ...     resp = await client.get("https://example.com")
    ...     data = await resp.text()
"""

import aiohttp
from typing import Optional, Dict, Any, Self
from .capabilities import Capability, CapabilitySet
from .exceptions import CapabilityDenied


class HttpClient:
    """Async HTTP client that requires the ``NETWORK_OUTBOUND`` capability.

    The client lazily creates an ``aiohttp.ClientSession`` on first use.
    Use it as an async context manager for automatic session cleanup.

    Args:
        capabilities: The actor's capability set.
        timeout: Default request timeout in seconds.

    Raises:
        CapabilityDenied: If ``NETWORK_OUTBOUND`` is not in *capabilities*.

    Example:
        >>> async with HttpClient(capabilities, timeout=10.0) as client:
        ...     resp = await client.post(
        ...         "https://api.example.com/data",
        ...         json={"key": "value"},
        ...     )
    """

    def __init__(self, capabilities: CapabilitySet, timeout: float = 30.0):
        """Initialize the HTTP client.

        Args:
            capabilities: Capability set that must include
                ``NETWORK_OUTBOUND``.
            timeout: Global request timeout in seconds (default ``30.0``).

        Raises:
            CapabilityDenied: If the capability set does not include
                :data:`Capability.NETWORK_OUTBOUND`.
        """
        if not capabilities.has(Capability.NETWORK_OUTBOUND):
            raise CapabilityDenied("HTTP client requires NETWORK_OUTBOUND capability")
        self._timeout = aiohttp.ClientTimeout(total=timeout)
        self._session: Optional[aiohttp.ClientSession] = None

    async def __aenter__(self) -> Self:
        """Enter the async context manager.

        Returns:
            The :class:`HttpClient` instance.
        """
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        """Exit the async context manager and close the underlying session."""
        await self.close()

    async def _get_session(self) -> aiohttp.ClientSession:
        """Return the active session, creating one if necessary.

        Returns:
            A ready-to-use ``aiohttp.ClientSession``.
        """
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(timeout=self._timeout)
        return self._session

    async def get(self, url: str, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        """Perform an HTTP GET request.

        Args:
            url: The request URL.
            headers: Optional request headers.

        Returns:
            An ``aiohttp.ClientResponse`` object.
        """
        session = await self._get_session()
        return await session.get(url, headers=headers)

    async def post(self, url: str, json: Any = None, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        """Perform an HTTP POST request.

        Args:
            url: The request URL.
            json: JSON-serializable body.
            headers: Optional request headers.

        Returns:
            An ``aiohttp.ClientResponse`` object.
        """
        session = await self._get_session()
        return await session.post(url, json=json, headers=headers)

    async def put(self, url: str, json: Any = None, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        """Perform an HTTP PUT request.

        Args:
            url: The request URL.
            json: JSON-serializable body.
            headers: Optional request headers.

        Returns:
            An ``aiohttp.ClientResponse`` object.
        """
        session = await self._get_session()
        return await session.put(url, json=json, headers=headers)

    async def delete(self, url: str, headers: Optional[Dict[str, str]] = None) -> aiohttp.ClientResponse:
        """Perform an HTTP DELETE request.

        Args:
            url: The request URL.
            headers: Optional request headers.

        Returns:
            An ``aiohttp.ClientResponse`` object.
        """
        session = await self._get_session()
        return await session.delete(url, headers=headers)

    async def close(self) -> None:
        """Close the underlying ``aiohttp.ClientSession`` if it is open.

        It is safe to call this method multiple times.
        """
        if self._session:
            await self._session.close()
