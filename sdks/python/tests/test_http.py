from unittest.mock import AsyncMock, patch

import pytest

from aether_sdk.capabilities import Capability, CapabilitySet
from aether_sdk.exceptions import CapabilityDenied
from aether_sdk.http import HttpClient


class TestHttpClient:
    """Tests for HttpClient class."""

    def test_init_requires_network_outbound_capability(self):
        """HttpClient should raise CapabilityDenied without NETWORK_OUTBOUND."""
        caps = CapabilitySet()
        with pytest.raises(CapabilityDenied) as exc_info:
            HttpClient(caps)
        assert "NETWORK_OUTBOUND" in str(exc_info.value)

    def test_init_with_network_outbound_capability(self):
        """HttpClient should initialize with NETWORK_OUTBOUND capability."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)
        assert client is not None

    def test_init_with_custom_timeout(self):
        """HttpClient should accept custom timeout."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps, timeout=60.0)
        assert client._timeout.total == 60.0

    @pytest.mark.asyncio
    async def test_get_request(self):
        """HttpClient.get should make GET request."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response = AsyncMock()
        mock_response.status = 200

        with patch.object(client, "_get_session") as mock_get_session:
            mock_session = AsyncMock()
            mock_session.get = AsyncMock(return_value=mock_response)
            mock_get_session.return_value = mock_session

            response = await client.get("https://example.com/api")
            assert response.status == 200
            mock_session.get.assert_called_once_with(
                "https://example.com/api", headers=None
            )

        await client.close()

    @pytest.mark.asyncio
    async def test_get_request_with_headers(self):
        """HttpClient.get should pass headers."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response = AsyncMock()
        mock_response.status = 200

        headers = {"Authorization": "Bearer token"}

        with patch.object(client, "_get_session") as mock_get_session:
            mock_session = AsyncMock()
            mock_session.get = AsyncMock(return_value=mock_response)
            mock_get_session.return_value = mock_session

            response = await client.get("https://example.com/api", headers=headers)
            assert response.status == 200
            mock_session.get.assert_called_once_with(
                "https://example.com/api", headers=headers
            )

        await client.close()

    @pytest.mark.asyncio
    async def test_post_request(self):
        """HttpClient.post should make POST request with JSON body."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response = AsyncMock()
        mock_response.status = 201

        json_data = {"name": "test", "value": 42}

        with patch.object(client, "_get_session") as mock_get_session:
            mock_session = AsyncMock()
            mock_session.post = AsyncMock(return_value=mock_response)
            mock_get_session.return_value = mock_session

            response = await client.post("https://example.com/api", json=json_data)
            assert response.status == 201
            mock_session.post.assert_called_once_with(
                "https://example.com/api", json=json_data, headers=None
            )

        await client.close()

    @pytest.mark.asyncio
    async def test_put_request(self):
        """HttpClient.put should make PUT request with JSON body."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response = AsyncMock()
        mock_response.status = 200

        json_data = {"id": 1, "name": "updated"}

        with patch.object(client, "_get_session") as mock_get_session:
            mock_session = AsyncMock()
            mock_session.put = AsyncMock(return_value=mock_response)
            mock_get_session.return_value = mock_session

            response = await client.put("https://example.com/api/1", json=json_data)
            assert response.status == 200
            mock_session.put.assert_called_once_with(
                "https://example.com/api/1", json=json_data, headers=None
            )

        await client.close()

    @pytest.mark.asyncio
    async def test_delete_request(self):
        """HttpClient.delete should make DELETE request."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response = AsyncMock()
        mock_response.status = 204

        with patch.object(client, "_get_session") as mock_get_session:
            mock_session = AsyncMock()
            mock_session.delete = AsyncMock(return_value=mock_response)
            mock_get_session.return_value = mock_session

            response = await client.delete("https://example.com/api/1")
            assert response.status == 204
            mock_session.delete.assert_called_once_with(
                "https://example.com/api/1", headers=None
            )

        await client.close()

    @pytest.mark.asyncio
    async def test_close_session(self):
        """HttpClient.close should close the session."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        # Create a session
        mock_session = AsyncMock()
        mock_session.closed = False
        client._session = mock_session

        await client.close()
        mock_session.close.assert_called_once()

    @pytest.mark.asyncio
    async def test_close_without_session(self):
        """HttpClient.close should handle no session gracefully."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        # Should not raise
        await client.close()

    @pytest.mark.asyncio
    async def test_session_reuse(self):
        """HttpClient should reuse session across requests."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response1 = AsyncMock(status=200)
        mock_response2 = AsyncMock(status=201)

        with patch("aether_sdk.http.aiohttp.ClientSession") as MockSession:
            mock_session = AsyncMock()
            mock_session.get = AsyncMock(side_effect=[mock_response1, mock_response2])
            mock_session.closed = False
            MockSession.return_value = mock_session

            # First request creates session
            response1 = await client.get("https://example.com/1")
            assert response1.status == 200

            # Second request reuses session
            response2 = await client.get("https://example.com/2")
            assert response2.status == 201

            # ClientSession should only be created once
            assert MockSession.call_count == 1

        await client.close()

    @pytest.mark.asyncio
    async def test_session_recreation_after_close(self):
        """HttpClient should create new session after close."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        client = HttpClient(caps)

        mock_response = AsyncMock(status=200)

        with patch("aether_sdk.http.aiohttp.ClientSession") as MockSession:
            mock_session = AsyncMock()
            mock_session.get = AsyncMock(return_value=mock_response)
            mock_session.closed = False
            MockSession.return_value = mock_session

            # First request
            await client.get("https://example.com/1")
            first_call_count = MockSession.call_count

            # Close session
            await client.close()

            # Reset mock for closed state
            mock_session.closed = True

            # Create new mock session for recreation
            new_mock_session = AsyncMock()
            new_mock_session.get = AsyncMock(return_value=mock_response)
            new_mock_session.closed = False
            MockSession.return_value = new_mock_session

            # New request should create new session
            await client.get("https://example.com/2")

            assert MockSession.call_count > first_call_count

    @pytest.mark.asyncio
    async def test_async_context_manager(self):
        """HttpClient should work as async context manager."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)

        mock_response = AsyncMock(status=200)

        with patch("aether_sdk.http.aiohttp.ClientSession") as MockSession:
            mock_session = AsyncMock()
            mock_session.get = AsyncMock(return_value=mock_response)
            mock_session.closed = False
            MockSession.return_value = mock_session

            async with HttpClient(caps) as client:
                response = await client.get("https://example.com")
                assert response.status == 200

            # Session should be closed after exiting context
            mock_session.close.assert_called_once()

    @pytest.mark.asyncio
    async def test_async_context_manager_closes_on_exception(self):
        """HttpClient should close session even if exception occurs."""
        caps = CapabilitySet(Capability.NETWORK_OUTBOUND)

        mock_response = AsyncMock(status=200)

        with patch("aether_sdk.http.aiohttp.ClientSession") as MockSession:
            mock_session = AsyncMock()
            mock_session.get = AsyncMock(return_value=mock_response)
            mock_session.closed = False
            MockSession.return_value = mock_session

            try:
                async with HttpClient(caps) as client:
                    await client.get("https://example.com")
                    raise ValueError("Simulated error")
            except ValueError:
                pass

            # Session should still be closed
            mock_session.close.assert_called_once()
