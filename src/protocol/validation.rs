//! Message validation for MCP protocol compliance.
//!
//! This module provides validation functions to ensure messages conform
//! to the MCP specification requirements.

use crate::error::{McpError, Result};
use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JSONRPC_VERSION};
use base64::Engine;
use serde_json::Value;

/// Validate a JSON-RPC request
pub fn validate_request(request: &JsonRpcRequest) -> Result<()> {
    // Check JSON-RPC version
    if request.jsonrpc != JSONRPC_VERSION {
        return Err(McpError::invalid_request(format!(
            "Invalid JSON-RPC version: expected '{}', got '{}'",
            JSONRPC_VERSION, request.jsonrpc
        )));
    }

    // Check method name
    if request.method.is_empty() {
        return Err(McpError::invalid_request("Method name cannot be empty"));
    }

    // Validate ID (must not be null according to MCP spec)
    match &request.id {
        Value::Null => {
            return Err(McpError::invalid_request("Request ID must not be null"));
        }
        Value::String(s) if s.is_empty() => {
            return Err(McpError::invalid_request(
                "Request ID cannot be empty string",
            ));
        }
        _ => {}
    }

    Ok(())
}

/// Validate a JSON-RPC notification
pub fn validate_notification(notification: &JsonRpcNotification) -> Result<()> {
    // Check JSON-RPC version
    if notification.jsonrpc != JSONRPC_VERSION {
        return Err(McpError::invalid_request(format!(
            "Invalid JSON-RPC version: expected '{}', got '{}'",
            JSONRPC_VERSION, notification.jsonrpc
        )));
    }

    // Check method name
    if notification.method.is_empty() {
        return Err(McpError::invalid_request("Method name cannot be empty"));
    }

    Ok(())
}

/// Validate a JSON-RPC response
pub fn validate_response(response: &JsonRpcResponse) -> Result<()> {
    // Check JSON-RPC version
    if response.jsonrpc != JSONRPC_VERSION {
        return Err(McpError::invalid_request(format!(
            "Invalid JSON-RPC version: expected '{}', got '{}'",
            JSONRPC_VERSION, response.jsonrpc
        )));
    }

    // Check that either result or error is present, but not both
    match (&response.result, &response.error) {
        (Some(_), Some(_)) => {
            return Err(McpError::invalid_request(
                "Response cannot have both result and error",
            ));
        }
        (None, None) => {
            return Err(McpError::invalid_request(
                "Response must have either result or error",
            ));
        }
        _ => {}
    }

    Ok(())
}

/// Validate MCP method name
pub fn validate_method_name(method: &str) -> Result<()> {
    if method.is_empty() {
        return Err(McpError::invalid_request("Method name cannot be empty"));
    }

    // Check for valid MCP method patterns
    let valid_prefixes = [
        "initialize",
        "ping",
        "notifications/",
        "resources/",
        "prompts/",
        "tools/",
        "sampling/",
        "logging/",
        "completion/",
        "roots/",
    ];

    if !valid_prefixes
        .iter()
        .any(|prefix| method.starts_with(prefix))
    {
        return Err(McpError::method_not_found(method));
    }

    Ok(())
}

/// Validate URI format
pub fn validate_uri(uri: &str) -> Result<()> {
    if uri.is_empty() {
        return Err(McpError::invalid_params("URI cannot be empty"));
    }

    // Basic URI validation - could be enhanced with proper URI parsing
    if !uri.contains("://") && !uri.starts_with("file:") {
        return Err(McpError::invalid_params(format!(
            "Invalid URI format: {}",
            uri
        )));
    }

    Ok(())
}

/// Validate MIME type format
pub fn validate_mime_type(mime_type: &str) -> Result<()> {
    if mime_type.is_empty() {
        return Err(McpError::invalid_params("MIME type cannot be empty"));
    }

    // Basic MIME type validation (type/subtype)
    if !mime_type.contains('/') {
        return Err(McpError::invalid_params(format!(
            "Invalid MIME type format: {}",
            mime_type
        )));
    }

    let parts: Vec<&str> = mime_type.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(McpError::invalid_params(format!(
            "Invalid MIME type format: {}",
            mime_type
        )));
    }

    Ok(())
}

/// Validate base64 encoded data
pub fn validate_base64(data: &str) -> Result<()> {
    if data.is_empty() {
        return Err(McpError::invalid_params("Base64 data cannot be empty"));
    }

    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| McpError::invalid_params(format!("Invalid base64 data: {}", e)))?;

    Ok(())
}

/// Validate tool input schema
pub fn validate_tool_schema(schema: &Value) -> Result<()> {
    // Basic JSON Schema validation
    if !schema.is_object() {
        return Err(McpError::invalid_params("Tool schema must be an object"));
    }

    let obj = schema.as_object().unwrap();

    // Check required 'type' field
    match obj.get("type") {
        Some(Value::String(t)) if t == "object" => {}
        Some(_) => {
            return Err(McpError::invalid_params(
                "Tool schema type must be 'object'",
            ));
        }
        None => {
            return Err(McpError::invalid_params(
                "Tool schema must have 'type' field",
            ));
        }
    }

    // Validate properties if present
    if let Some(properties) = obj.get("properties") {
        if !properties.is_object() {
            return Err(McpError::invalid_params(
                "Tool schema properties must be an object",
            ));
        }
    }

    // Validate required array if present
    if let Some(required) = obj.get("required") {
        if !required.is_array() {
            return Err(McpError::invalid_params(
                "Tool schema required must be an array",
            ));
        }
    }

    Ok(())
}

/// Validate pagination cursor
pub fn validate_cursor(cursor: &str) -> Result<()> {
    if cursor.is_empty() {
        return Err(McpError::invalid_params("Cursor cannot be empty"));
    }

    // Cursors should be opaque strings, so we just check they're not empty
    // and contain valid characters
    if !cursor
        .chars()
        .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
    {
        return Err(McpError::invalid_params(
            "Cursor contains invalid characters",
        ));
    }

    Ok(())
}

/// Validate progress token
pub fn validate_progress_token(token: &Value) -> Result<()> {
    match token {
        Value::String(s) if s.is_empty() => {
            return Err(McpError::invalid_params(
                "Progress token string cannot be empty",
            ));
        }
        Value::Number(n) if n.as_f64().map_or(false, |f| !f.is_finite()) => {
            return Err(McpError::invalid_params(
                "Progress token number must be finite",
            ));
        }
        Value::String(_) | Value::Number(_) => {}
        _ => {
            return Err(McpError::invalid_params(
                "Progress token must be string or number",
            ));
        }
    }

    Ok(())
}

/// Validate logging level
pub fn validate_logging_level(level: &str) -> Result<()> {
    let valid_levels = [
        "debug",
        "info",
        "notice",
        "warning",
        "error",
        "critical",
        "alert",
        "emergency",
    ];

    if !valid_levels.contains(&level) {
        return Err(McpError::invalid_params(format!(
            "Invalid logging level: {}. Valid levels are: {}",
            level,
            valid_levels.join(", ")
        )));
    }

    Ok(())
}

/// Validate role
pub fn validate_role(role: &str) -> Result<()> {
    match role {
        "user" | "assistant" => Ok(()),
        _ => Err(McpError::invalid_params(format!(
            "Invalid role: {}. Valid roles are: user, assistant",
            role
        ))),
    }
}

/// Validate priority value (0.0 to 1.0)
pub fn validate_priority(priority: f64) -> Result<()> {
    if !(0.0..=1.0).contains(&priority) {
        return Err(McpError::invalid_params(format!(
            "Priority must be between 0.0 and 1.0, got: {}",
            priority
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_request() {
        let valid_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!("test-id"),
            method: "initialize".to_string(),
            params: None,
        };
        assert!(validate_request(&valid_request).is_ok());

        let invalid_version = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            id: json!("test-id"),
            method: "initialize".to_string(),
            params: None,
        };
        assert!(validate_request(&invalid_version).is_err());

        let null_id = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(null),
            method: "initialize".to_string(),
            params: None,
        };
        assert!(validate_request(&null_id).is_err());
    }

    #[test]
    fn test_validate_uri() {
        assert!(validate_uri("https://example.com/resource").is_ok());
        assert!(validate_uri("file:///path/to/file").is_ok());
        assert!(validate_uri("").is_err());
        assert!(validate_uri("invalid-uri").is_err());
    }

    #[test]
    fn test_validate_mime_type() {
        assert!(validate_mime_type("text/plain").is_ok());
        assert!(validate_mime_type("application/json").is_ok());
        assert!(validate_mime_type("").is_err());
        assert!(validate_mime_type("invalid").is_err());
        assert!(validate_mime_type("text/").is_err());
        assert!(validate_mime_type("/plain").is_err());
    }

    #[test]
    fn test_validate_priority() {
        assert!(validate_priority(0.0).is_ok());
        assert!(validate_priority(0.5).is_ok());
        assert!(validate_priority(1.0).is_ok());
        assert!(validate_priority(-0.1).is_err());
        assert!(validate_priority(1.1).is_err());
    }
}
