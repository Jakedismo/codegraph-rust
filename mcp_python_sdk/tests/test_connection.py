"""
Tests for the Connection class.
"""

import asyncio
import pytest
import websockets
from unittest.mock import AsyncMock, patch

from mcp_sdk.connection import Connection
from mcp_sdk.exceptions import ConnectionError

@pytest.mark.asyncio
async def test_connection_successful():
    """Tests a successful connection."""
    with patch('websockets.connect', new_callable=AsyncMock) as mock_connect:
        mock_connect.return_value = AsyncMock()
        connection = Connection("ws://localhost:8765")
        await connection.connect()
        assert connection.is_connected
        await connection.disconnect()
        assert not connection.is_connected

@pytest.mark.asyncio
async def test_connection_failed():
    """Tests a failed connection."""
    with patch('websockets.connect', new_callable=AsyncMock) as mock_connect:
        mock_connect.side_effect = websockets.exceptions.WebSocketException("Connection failed")
        connection = Connection("ws://localhost:8765")
        with pytest.raises(ConnectionError):
            await connection.connect()

@pytest.mark.asyncio
async def test_send_receive():
    """Tests sending and receiving messages."""
    with patch('websockets.connect', new_callable=AsyncMock) as mock_connect:
        mock_websocket = AsyncMock()
        mock_connect.return_value = mock_websocket
        connection = Connection("ws://localhost:8765")
        await connection.connect()

        await connection.send("Hello")
        mock_websocket.send.assert_called_with("Hello")

        mock_websocket.recv.return_value = "World"
        message = await connection.recv()
        assert message == "World"

@pytest.mark.asyncio
async def test_reconnect_on_send_error():
    """Tests reconnection on send error."""
    with patch('websockets.connect', new_callable=AsyncMock) as mock_connect:
        mock_websocket = AsyncMock()
        mock_websocket.send.side_effect = [websockets.exceptions.WebSocketException("Send error"), None]
        mock_connect.return_value = mock_websocket

        connection = Connection("ws://localhost:8765")
        await connection.connect()

        # The first send will fail, trigger a reconnect, and the second will succeed
        await connection.send("Hello")
        assert mock_websocket.send.call_count == 2

@pytest.mark.asyncio
async def test_reconnect_on_recv_error():
    """Tests reconnection on receive error."""
    with patch('websockets.connect', new_callable=AsyncMock) as mock_connect:
        mock_websocket = AsyncMock()
        mock_websocket.recv.side_effect = [websockets.exceptions.WebSocketException("Recv error"), "World"]
        mock_connect.return_value = mock_websocket

        connection = Connection("ws://localhost:8765")
        await connection.connect()

        # The first recv will fail, trigger a reconnect, and the second will succeed
        message = await connection.recv()
        assert message == "World"
        assert mock_websocket.recv.call_count == 2
