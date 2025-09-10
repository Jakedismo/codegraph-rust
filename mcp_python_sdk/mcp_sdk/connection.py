"""
Manages the WebSocket connection to the MCP server.
"""

import asyncio
import websockets
from typing import Optional

from .exceptions import ConnectionError

class Connection:
    """Manages the WebSocket connection and reconnection logic."""

    def __init__(self, uri: str, reconnect_delay: int = 5):
        self.uri = uri
        self.reconnect_delay = reconnect_delay
        self.websocket: Optional[websockets.WebSocketClientProtocol] = None
        self._is_connected = False

    async def connect(self) -> None:
        """Connects to the WebSocket server."""
        if self._is_connected:
            return

        try:
            self.websocket = await websockets.connect(self.uri)
            self._is_connected = True
        except websockets.exceptions.WebSocketException as e:
            raise ConnectionError(f"Failed to connect to {self.uri}: {e}") from e

    async def disconnect(self) -> None:
        """Disconnects from the WebSocket server."""
        if not self._is_connected or not self.websocket:
            return

        await self.websocket.close()
        self._is_connected = False

    async def send(self, message: str) -> None:
        """Sends a message over the WebSocket."""
        if not self._is_connected or not self.websocket:
            raise ConnectionError("Not connected.")

        try:
            await self.websocket.send(message)
        except websockets.exceptions.WebSocketException as e:
            self._is_connected = False
            await self._reconnect()
            await self.websocket.send(message)

    async def recv(self) -> str:
        """Receives a message from the WebSocket."""
        if not self._is_connected or not self.websocket:
            raise ConnectionError("Not connected.")

        try:
            return await self.websocket.recv()
        except websockets.exceptions.WebSocketException as e:
            self._is_connected = False
            await self._reconnect()
            return await self.websocket.recv()

    async def _reconnect(self) -> None:
        """Handles the reconnection logic."""
        while not self._is_connected:
            try:
                await self.disconnect()
                await asyncio.sleep(self.reconnect_delay)
                await self.connect()
            except ConnectionError:
                pass

    @property
    def is_connected(self) -> bool:
        """Returns True if the WebSocket is connected."""
        return self._is_connected
