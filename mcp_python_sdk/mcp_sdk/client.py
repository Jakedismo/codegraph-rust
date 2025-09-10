"""
The main MCP client for interacting with the MCP server.
"""

from .connection import Connection
from .exceptions import MCPError

class MCPClient:
    """The main MCP client."""

    def __init__(self, uri: str):
        self.connection = Connection(uri)

    async def connect(self) -> None:
        """Connects to the MCP server."""
        await self.connection.connect()

    async def disconnect(self) -> None:
        """Disconnects from the MCP server."""
        await self.connection.disconnect()

    def _serialize_message(self, message: str) -> str:
        """Serializes a message to be sent to the MCP server.

        This is a placeholder for the actual message serialization logic.

        Args:
            message: The message to serialize.

        Returns:
            The serialized message.
        """
        return message

    def _process_message(self, message: str) -> str:
        """Processes a message received from the MCP server.

        This is a placeholder for the actual message deserialization logic.

        Args:
            message: The message to process.

        Returns:
            The processed message.
        """
        return message

    async def send_message(self, message: str) -> None:
        """Sends a message to the MCP server."""
        serialized_message = self._serialize_message(message)
        await self.connection.send(serialized_message)

    async def receive_message(self) -> str:
        """Receives a message from the MCP server."""
        message = await self.connection.recv()
        return self._process_message(message)

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.disconnect()
