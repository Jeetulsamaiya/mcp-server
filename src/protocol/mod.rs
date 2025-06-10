//! MCP protocol implementation.
//!
//! This module contains the core protocol types and message handling
//! for the Model Context Protocol (MCP) specification 2025-03-26.

pub mod handler;
pub mod messages;
pub mod validation;

// Re-export commonly used types
pub use handler::*;
pub use messages::*;
pub use validation::*;

use crate::error::McpError;
use serde::{Deserialize, Serialize};

/// Current MCP protocol version
pub const PROTOCOL_VERSION: &str = "2025-03-26";

/// JSON-RPC version
pub const JSONRPC_VERSION: &str = "2.0";

/// Request ID type
pub type RequestId = serde_json::Value;

/// Progress token type
pub type ProgressToken = serde_json::Value;

/// Cursor type for pagination
pub type Cursor = String;

/// Base JSON-RPC message trait
pub trait JsonRpcMessage {
    fn jsonrpc(&self) -> &str;
}

/// Base request trait
pub trait Request: JsonRpcMessage {
    fn method(&self) -> &str;
    fn params(&self) -> Option<&serde_json::Value>;
    fn id(&self) -> Option<&RequestId>;
}

/// Base notification trait
pub trait Notification: JsonRpcMessage {
    fn method(&self) -> &str;
    fn params(&self) -> Option<&serde_json::Value>;
}

/// Base result trait
pub trait JsonRpcResult: JsonRpcMessage {
    fn id(&self) -> &RequestId;
    fn result(&self) -> Option<&serde_json::Value>;
    fn error(&self) -> Option<&JsonRpcError>;
}

/// JSON-RPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Generic JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Generic JSON-RPC notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// Generic JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Batch request/notification
pub type JsonRpcBatch = Vec<serde_json::Value>;

/// Any JSON-RPC message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnyJsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
    Batch(JsonRpcBatch),
}

impl JsonRpcMessage for JsonRpcRequest {
    fn jsonrpc(&self) -> &str {
        &self.jsonrpc
    }
}

impl Request for JsonRpcRequest {
    fn method(&self) -> &str {
        &self.method
    }

    fn params(&self) -> Option<&serde_json::Value> {
        self.params.as_ref()
    }

    fn id(&self) -> Option<&RequestId> {
        Some(&self.id)
    }
}

impl JsonRpcMessage for JsonRpcNotification {
    fn jsonrpc(&self) -> &str {
        &self.jsonrpc
    }
}

impl Notification for JsonRpcNotification {
    fn method(&self) -> &str {
        &self.method
    }

    fn params(&self) -> Option<&serde_json::Value> {
        self.params.as_ref()
    }
}

impl JsonRpcMessage for JsonRpcResponse {
    fn jsonrpc(&self) -> &str {
        &self.jsonrpc
    }
}

impl JsonRpcResult for JsonRpcResponse {
    fn id(&self) -> &RequestId {
        &self.id
    }

    fn result(&self) -> Option<&serde_json::Value> {
        self.result.as_ref()
    }

    fn error(&self) -> Option<&JsonRpcError> {
        self.error.as_ref()
    }
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(id: RequestId, method: String, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method,
            params,
        }
    }
}

impl JsonRpcNotification {
    /// Create a new JSON-RPC notification
    pub fn new(method: String, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method,
            params,
        }
    }
}

impl JsonRpcResponse {
    /// Create a successful JSON-RPC response
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error JSON-RPC response
    pub fn error(id: RequestId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl From<McpError> for JsonRpcError {
    fn from(error: McpError) -> Self {
        JsonRpcError {
            code: error.to_json_rpc_code(),
            message: error.to_string(),
            data: None,
        }
    }
}

/// Parse a JSON-RPC message from a string
pub fn parse_message(data: &str) -> crate::Result<AnyJsonRpcMessage> {
    serde_json::from_str(data).map_err(|e| McpError::parse_error(e.to_string()))
}

/// Serialize a JSON-RPC message to a string
pub fn serialize_message(message: &AnyJsonRpcMessage) -> crate::Result<String> {
    serde_json::to_string(message).map_err(|e| McpError::Serialization(e))
}
