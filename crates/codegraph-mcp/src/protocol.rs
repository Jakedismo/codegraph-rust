use crate::{message::*, version::*, McpError, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;
use uuid::Uuid;

/// MCP protocol helper to build requests and validate responses
#[derive(Debug, Clone)]
pub struct McpProtocol {
    version: ProtocolVersion,
}

impl McpProtocol {
    pub fn new(version: ProtocolVersion) -> Self {
        Self { version }
    }

    pub fn default() -> Self {
        Self { version: ProtocolVersion::latest() }
    }

    pub fn version(&self) -> &ProtocolVersion {
        &self.version
    }

    pub fn build_request<T: Serialize>(&self, method: &str, params: &T) -> Result<JsonRpcRequest> {
        let id = json!(Uuid::new_v4().to_string());
        let params_value = serde_json::to_value(params)?;
        Ok(JsonRpcRequest::new(id, method.to_string(), Some(params_value)))
    }

    pub fn build_notification<T: Serialize>(&self, method: &str, params: &T) -> Result<JsonRpcNotification> {
        let params_value = serde_json::to_value(params)?;
        Ok(JsonRpcNotification::new(method.to_string(), Some(params_value)))
    }
}

/// Convenience functions for core MCP handshake
pub mod handshake {
    use super::*;

    pub const METHOD_INITIALIZE: &str = "initialize";

    pub async fn build_initialize_request(
        negotiator: &VersionNegotiator,
        requested_version: Option<&str>,
        client_name: &str,
        client_version: &str,
        capabilities: Option<McpCapabilities>,
    ) -> Result<JsonRpcRequest> {
        let requested = requested_version.unwrap_or(DEFAULT_VERSION);
        let version = negotiator.negotiate(requested)?;
        let protocol = McpProtocol::new(version);
        let params = McpInitializeParams {
            protocol_version: protocol.version().to_string(),
            capabilities: capabilities.unwrap_or(McpCapabilities { experimental: None, sampling: None }),
            client_info: McpClientInfo { name: client_name.to_string(), version: client_version.to_string() },
        };
        protocol.build_request(METHOD_INITIALIZE, &params)
    }

    pub fn parse_initialize_result(msg: &JsonRpcMessage) -> Result<McpInitializeResult> {
        match msg {
            JsonRpcMessage::V2(JsonRpcV2Message::Response(res)) => match &res.result {
                JsonRpcResult::Success { result } => {
                    let v: McpInitializeResult = serde_json::from_value(result.clone())?;
                    Ok(v)
                }
                JsonRpcResult::Error { error } => Err(McpError::Protocol(error.message.clone())),
            },
            _ => Err(McpError::InvalidMessage("Expected initialize response".into())),
        }
    }
}

/// Typed request/response helpers
pub fn parse_response_typed<R: DeserializeOwned>(msg: &JsonRpcMessage) -> Result<R> {
    match msg {
        JsonRpcMessage::V2(JsonRpcV2Message::Response(res)) => match &res.result {
            JsonRpcResult::Success { result } => {
                let v: R = serde_json::from_value(result.clone())?;
                Ok(v)
            }
            JsonRpcResult::Error { error } => Err(McpError::Protocol(error.message.clone())),
        },
        _ => Err(McpError::InvalidMessage("Expected JSON-RPC response".into())),
    }
}

