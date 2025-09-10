"""
Tests for the MCPClient class.
"""

import pytest
from unittest.mock import AsyncMock, patch

from mcp_sdk.client import MCPClient

@pytest.mark.asyncio
async def test_client_connect_disconnect():
    """Tests client connection and disconnection."""
    with patch('mcp_sdk.connection.Connection', new_callable=AsyncMock) as mock_connection:
        client = MCPClient("ws://localhost:8765")
        client.connection = mock_connection

        await client.connect()
        mock_connection.connect.assert_called_once()

        await client.disconnect()
        mock_connection.disconnect.assert_called_once()

@pytest.mark.asyncio
async def test_client_send_receive():
    """Tests client send and receive."""
    with patch('mcp_sdk.connection.Connection', new_callable=AsyncMock) as mock_connection:
        client = MCPClient("ws://localhost:8765")
        client.connection = mock_connection

        await client.send_message("Hello")
        mock_connection.send.assert_called_with("Hello")

        mock_connection.recv.return_value = "World"
        message = await client.receive_message()
        assert message == "World"

@pytest.mark.asyncio
async def test_client_context_manager():
    """Tests the client as an async context manager."""
    with patch('mcp_sdk.connection.Connection', new_callable=AsyncMock) as mock_connection:
        client = MCPClient("ws://localhost:8765")
        client.connection = mock_connection

        async with client as c:
            assert c == client
            mock_connection.connect.assert_called_once()

        mock_connection.disconnect.assert_called_once()
