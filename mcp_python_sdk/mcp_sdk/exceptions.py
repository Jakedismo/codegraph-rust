"""
Custom exceptions for the MCP SDK.
"""

class MCPError(Exception):
    """Base exception for all MCP SDK errors."""
    pass

class ConnectionError(MCPError):
    """Raised when there is a connection error."""
    pass

class AuthenticationError(MCPError):
    """Raised when there is an authentication error."""
    pass

class ProtocolError(MCPError):
    """Raised when there is a protocol error."""
    pass
