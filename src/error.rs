//! Error handling for the MCP server implementation.
//!
//! This module provides comprehensive error handling following the MCP specification
//! error codes and JSON-RPC error format.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type alias for MCP operations
pub type Result<T> = std::result::Result<T, McpError>;

/// Main error type for MCP server operations
#[derive(Error, Debug)]
pub enum McpError {
    /// JSON-RPC parse error (-32700)
    #[error("Parse error: {0}")]
    ParseError(String),

    /// JSON-RPC invalid request (-32600)
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// JSON-RPC method not found (-32601)
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// JSON-RPC invalid params (-32602)
    #[error("Invalid params: {0}")]
    InvalidParams(String),

    /// JSON-RPC internal error (-32603)
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Transport-related errors
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// Protocol-related errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Resource-related errors
    #[error("Resource error: {0}")]
    Resource(String),

    /// Tool-related errors
    #[error("Tool error: {0}")]
    Tool(String),

    /// Prompt-related errors
    #[error("Prompt error: {0}")]
    Prompt(String),

    /// Authentication/Authorization errors
    #[error("Auth error: {0}")]
    Auth(String),

    /// Configuration errors
    #[error("Config error: {0}")]
    Config(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// HTTP errors
    #[error("HTTP error: {0}")]
    Http(#[from] actix_web::Error),

    /// Request errors
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    /// Other errors
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

// Ensure McpError is Send + Sync
unsafe impl Send for McpError {}
unsafe impl Sync for McpError {}

/// Transport-specific errors
#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Connection lost: {0}")]
    ConnectionLost(String),

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Session expired: {0}")]
    SessionExpired(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

// Ensure TransportError is Send + Sync
unsafe impl Send for TransportError {}
unsafe impl Sync for TransportError {}

/// JSON-RPC error response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpError {
    /// Convert to JSON-RPC error code
    pub fn to_json_rpc_code(&self) -> i32 {
        match self {
            McpError::ParseError(_) => -32700,
            McpError::InvalidRequest(_) => -32600,
            McpError::MethodNotFound(_) => -32601,
            McpError::InvalidParams(_) => -32602,
            McpError::InternalError(_) => -32603,
            _ => -32603, // Default to internal error
        }
    }

    /// Convert to JSON-RPC error structure
    pub fn to_json_rpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.to_json_rpc_code(),
            message: self.to_string(),
            data: None,
        }
    }

    /// Create a parse error
    pub fn parse_error(msg: impl Into<String>) -> Self {
        McpError::ParseError(msg.into())
    }

    /// Create an invalid request error
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        McpError::InvalidRequest(msg.into())
    }

    /// Create a method not found error
    pub fn method_not_found(method: impl Into<String>) -> Self {
        McpError::MethodNotFound(format!("Method '{}' not found", method.into()))
    }

    /// Create an invalid params error
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        McpError::InvalidParams(msg.into())
    }

    /// Create an internal error
    pub fn internal_error(msg: impl Into<String>) -> Self {
        McpError::InternalError(msg.into())
    }
}
