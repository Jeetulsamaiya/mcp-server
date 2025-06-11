//! Tool management for MCP server.
//!
//! This module implements the tool feature of MCP, allowing the server
//! to expose executable tools to clients.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{McpError, Result};
use crate::protocol::{Content, PaginationParams, PaginationResult, Tool};
use crate::server::features::FeatureManager;

/// Tool manager for handling MCP tools
pub struct ToolManager {
    /// Registered tools
    tools: Arc<RwLock<HashMap<String, Tool>>>,

    /// Tool handlers
    handlers: Arc<RwLock<HashMap<String, Box<dyn ToolHandler>>>>,

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Tool handler trait for executing tools
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    /// Get the handler name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> Option<String> {
        None
    }

    /// Get the tool input schema
    fn input_schema(&self) -> crate::protocol::ToolInputSchema;

    /// Get the tool annotations (optional)
    fn annotations(&self) -> Option<crate::protocol::ToolAnnotations> {
        None
    }

    /// Get the complete tool definition
    fn tool_definition(&self) -> crate::protocol::Tool {
        crate::protocol::Tool {
            name: self.name().to_string(),
            description: self.description(),
            input_schema: self.input_schema(),
            annotations: self.annotations(),
        }
    }

    /// Execute the tool with given arguments
    async fn execute(&self, arguments: Option<Value>) -> Result<ToolResult>;

    /// Validate tool arguments (optional)
    async fn validate_arguments(&self, arguments: Option<&Value>) -> Result<()> {
        let _ = arguments;
        Ok(())
    }
}

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Result content
    pub content: Vec<Content>,

    /// Whether the execution resulted in an error
    pub is_error: bool,
}

impl ToolManager {
    /// Create a new tool manager
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Register a tool
    pub async fn register_tool(&self, tool: Tool) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Tool("Tool feature is disabled".to_string()));
        }

        let name = tool.name.clone();

        {
            let mut tools = self.tools.write().await;
            tools.insert(name.clone(), tool);
        }

        info!("Registered tool: {}", name);
        Ok(())
    }

    /// Unregister a tool
    pub async fn unregister_tool(&self, name: &str) -> Result<Option<Tool>> {
        let mut tools = self.tools.write().await;
        let tool = tools.remove(name);

        if tool.is_some() {
            info!("Unregistered tool: {}", name);
        }

        Ok(tool)
    }

    /// Get a tool by name
    pub async fn get_tool(&self, name: &str) -> Option<Tool> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    /// List all tools with optional pagination
    pub async fn list_tools(
        &self,
        pagination: Option<PaginationParams>,
    ) -> Result<(Vec<Tool>, PaginationResult)> {
        if !self.is_enabled() {
            return Err(McpError::Tool("Tool feature is disabled".to_string()));
        }

        let tools = self.tools.read().await;
        let mut all_tools: Vec<Tool> = tools.values().cloned().collect();

        // Sort by name for consistent ordering
        all_tools.sort_by(|a, b| a.name.cmp(&b.name));

        // Apply pagination if provided
        let (tools, pagination_result) = if let Some(params) = pagination {
            self.apply_pagination(all_tools, params)?
        } else {
            (all_tools, PaginationResult { next_cursor: None })
        };

        Ok((tools, pagination_result))
    }

    /// Execute a tool
    pub async fn call_tool(&self, name: &str, arguments: Option<Value>) -> Result<ToolResult> {
        if !self.is_enabled() {
            return Err(McpError::Tool("Tool feature is disabled".to_string()));
        }

        // Check if tool exists
        let _tool = self
            .get_tool(name)
            .await
            .ok_or_else(|| McpError::Tool(format!("Tool not found: {}", name)))?;

        // Find handler
        let handlers = self.handlers.read().await;
        let handler = handlers
            .get(name)
            .ok_or_else(|| McpError::Tool(format!("No handler found for tool: {}", name)))?;

        // Validate arguments
        handler.validate_arguments(arguments.as_ref()).await?;

        // Execute tool
        let result = handler.execute(arguments).await?;

        info!(
            "Executed tool: {} -> {} content items",
            name,
            result.content.len()
        );
        Ok(result)
    }

    /// Register a tool handler
    pub async fn register_handler(&self, handler: Box<dyn ToolHandler>) -> Result<()> {
        let name = handler.name().to_string();

        {
            let mut handlers = self.handlers.write().await;
            handlers.insert(name.clone(), handler);
        }

        info!("Registered tool handler: {}", name);
        Ok(())
    }

    /// Register a tool handler and automatically create the tool definition from it
    pub async fn register_handler_with_tool(&self, handler: Box<dyn ToolHandler>) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Tool("Tool feature is disabled".to_string()));
        }

        let tool_definition = handler.tool_definition();
        let name = handler.name().to_string();

        // Register the tool definition first
        self.register_tool(tool_definition).await?;

        // Then register the handler
        self.register_handler(handler).await?;

        info!("Registered tool and handler: {}", name);
        Ok(())
    }

    /// Register multiple tool handlers dynamically
    pub async fn register_handlers(&self, handlers: Vec<Box<dyn ToolHandler>>) -> Result<()> {
        for handler in handlers {
            if let Err(e) = self.register_handler_with_tool(handler).await {
                warn!("Failed to register tool handler: {}", e);
            }
        }
        Ok(())
    }

    /// Get tool count
    pub async fn get_tool_count(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }

    /// Check if the feature is enabled
    pub fn is_enabled(&self) -> bool {
        // Use try_read to avoid blocking in sync context
        self.enabled
            .try_read()
            .map(|enabled| *enabled)
            .unwrap_or(true)
    }

    /// Check if the feature is enabled (async version)
    pub async fn is_enabled_async(&self) -> bool {
        let enabled = self.enabled.read().await;
        *enabled
    }

    /// Set enabled state
    pub async fn set_enabled(&self, enabled: bool) {
        let mut state = self.enabled.write().await;
        *state = enabled;
    }

    /// Apply pagination to tools
    fn apply_pagination(
        &self,
        mut tools: Vec<Tool>,
        params: PaginationParams,
    ) -> Result<(Vec<Tool>, PaginationResult)> {
        let start_index = if let Some(cursor) = params.cursor {
            cursor.parse::<usize>().unwrap_or(0)
        } else {
            0
        };

        let page_size = 50; // Default page size
        let end_index = std::cmp::min(start_index + page_size, tools.len());

        let page_tools = if start_index < tools.len() {
            tools.drain(start_index..end_index).collect()
        } else {
            Vec::new()
        };

        let next_cursor = if end_index < tools.len() {
            Some(end_index.to_string())
        } else {
            None
        };

        Ok((page_tools, PaginationResult { next_cursor }))
    }
}

impl FeatureManager for ToolManager {
    fn name(&self) -> &'static str {
        "tools"
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }

    fn set_enabled(&mut self, enabled: bool) {
        if let Ok(mut state) = self.enabled.try_write() {
            *state = enabled;
        }
    }
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: false,
        }
    }

    /// Create an error tool result
    pub fn error(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: true,
        }
    }

    /// Create a simple text result
    pub fn text(text: String) -> Self {
        Self::success(vec![Content::Text {
            text,
            annotations: None,
        }])
    }

    /// Create a simple error text result
    pub fn error_text(text: String) -> Self {
        Self::error(vec![Content::Text {
            text,
            annotations: None,
        }])
    }
}

/// Example echo tool handler
pub struct EchoToolHandler;

#[async_trait::async_trait]
impl ToolHandler for EchoToolHandler {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> Option<String> {
        Some("Echo back the provided message".to_string())
    }

    fn input_schema(&self) -> crate::protocol::ToolInputSchema {
        use std::collections::HashMap;

        crate::protocol::ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert(
                    "message".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "The message to echo back"
                    }),
                );
                props
            }),
            required: Some(vec!["message".to_string()]),
        }
    }

    async fn execute(&self, arguments: Option<Value>) -> Result<ToolResult> {
        info!("Executing echo tool with arguments: {:?}", arguments);
        let message = if let Some(args) = arguments {
            args.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Hello, world!")
                .to_string()
        } else {
            "Hello, world!".to_string()
        };

        Ok(ToolResult::text(format!("Echo: {}", message)))
    }

    async fn validate_arguments(&self, arguments: Option<&Value>) -> Result<()> {
        if let Some(args) = arguments {
            if !args.is_object() {
                return Err(McpError::invalid_params("Arguments must be an object"));
            }

            if let Some(message) = args.get("message") {
                if !message.is_string() {
                    return Err(McpError::invalid_params("Message must be a string"));
                }
            }
        }

        Ok(())
    }
}

/// Example calculator tool handler
pub struct CalculatorToolHandler;

#[async_trait::async_trait]
impl ToolHandler for CalculatorToolHandler {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> Option<String> {
        Some("Perform mathematical calculations".to_string())
    }

    fn input_schema(&self) -> crate::protocol::ToolInputSchema {
        use std::collections::HashMap;

        crate::protocol::ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert(
                    "operation".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Mathematical operation to perform",
                        "enum": ["add", "subtract", "multiply", "divide", "power", "sqrt"]
                    }),
                );
                props.insert(
                    "a".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "First operand"
                    }),
                );
                props.insert(
                    "b".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Second operand (not required for sqrt)"
                    }),
                );
                props
            }),
            required: Some(vec!["operation".to_string(), "a".to_string()]),
        }
    }

    async fn execute(&self, arguments: Option<Value>) -> Result<ToolResult> {
        let args =
            arguments.ok_or_else(|| McpError::invalid_params("Calculator requires arguments"))?;

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Operation is required"))?;

        let a = args.get("a").and_then(|v| v.as_f64()).ok_or_else(|| {
            McpError::invalid_params("Parameter 'a' is required and must be a number")
        })?;

        let b = args.get("b").and_then(|v| v.as_f64()).ok_or_else(|| {
            McpError::invalid_params("Parameter 'b' is required and must be a number")
        })?;

        let result = match operation {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Ok(ToolResult::error_text("Division by zero".to_string()));
                }
                a / b
            }
            _ => {
                return Ok(ToolResult::error_text(format!(
                    "Unknown operation: {}",
                    operation
                )));
            }
        };

        Ok(ToolResult::text(format!(
            "{} {} {} = {}",
            a, operation, b, result
        )))
    }

    async fn validate_arguments(&self, arguments: Option<&Value>) -> Result<()> {
        let args =
            arguments.ok_or_else(|| McpError::invalid_params("Calculator requires arguments"))?;

        if !args.is_object() {
            return Err(McpError::invalid_params("Arguments must be an object"));
        }

        // Validate operation
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpError::invalid_params("Operation is required and must be a string")
            })?;

        let valid_operations = ["add", "subtract", "multiply", "divide"];
        if !valid_operations.contains(&operation) {
            return Err(McpError::invalid_params(format!(
                "Invalid operation: {}. Valid operations are: {}",
                operation,
                valid_operations.join(", ")
            )));
        }

        // Validate parameters
        if !args.get("a").map(|v| v.is_number()).unwrap_or(false) {
            return Err(McpError::invalid_params(
                "Parameter 'a' is required and must be a number",
            ));
        }

        if !args.get("b").map(|v| v.is_number()).unwrap_or(false) {
            return Err(McpError::invalid_params(
                "Parameter 'b' is required and must be a number",
            ));
        }

        Ok(())
    }
}

/// Get all available production tool handlers
pub fn get_production_tool_handlers() -> Vec<Box<dyn ToolHandler>> {
    vec![
        Box::new(CalculatorToolHandler),
        // Uncomment to enable echo tool for testing
        // Box::new(EchoToolHandler),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ToolInputSchema;

    #[tokio::test]
    async fn test_tool_manager() {
        let manager = ToolManager::new();

        let tool = Tool {
            name: "test-tool".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: None,
                required: None,
            },
            annotations: None,
        };

        // Test registration
        assert!(manager.register_tool(tool.clone()).await.is_ok());

        // Test retrieval
        let retrieved = manager.get_tool("test-tool").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-tool");

        // Test listing
        let (tools, _) = manager.list_tools(None).await.unwrap();
        assert_eq!(tools.len(), 1);

        // Test unregistration
        let removed = manager.unregister_tool("test-tool").await.unwrap();
        assert!(removed.is_some());

        let not_found = manager.get_tool("test-tool").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_echo_tool() {
        let handler = EchoToolHandler;

        // Test with arguments
        let args = serde_json::json!({"message": "Hello, MCP!"});
        let result = handler.execute(Some(args)).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);

        // Test without arguments
        let result = handler.execute(None).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[tokio::test]
    async fn test_calculator_tool() {
        let handler = CalculatorToolHandler;

        // Test addition
        let args = serde_json::json!({
            "operation": "add",
            "a": 5.0,
            "b": 3.0
        });
        let result = handler.execute(Some(args)).await.unwrap();
        assert!(!result.is_error);

        // Test division by zero
        let args = serde_json::json!({
            "operation": "divide",
            "a": 5.0,
            "b": 0.0
        });
        let result = handler.execute(Some(args)).await.unwrap();
        assert!(result.is_error);

        // Test invalid operation
        let args = serde_json::json!({
            "operation": "invalid",
            "a": 5.0,
            "b": 3.0
        });
        let result = handler.execute(Some(args)).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_dynamic_tool_registration() {
        let manager = ToolManager::new();

        // Test registering a tool handler with automatic tool definition
        let handler = Box::new(CalculatorToolHandler);
        assert!(manager.register_handler_with_tool(handler).await.is_ok());

        // Verify the tool was registered
        let tool = manager.get_tool("calculator").await;
        assert!(tool.is_some());
        let tool = tool.unwrap();
        assert_eq!(tool.name, "calculator");
        assert!(tool.description.is_some());
        assert_eq!(tool.description.unwrap(), "Perform mathematical calculations");

        // Verify the tool appears in the list
        let (tools, _) = manager.list_tools(None).await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "calculator");

        // Test tool execution
        let args = serde_json::json!({
            "operation": "add",
            "a": 2.0,
            "b": 3.0
        });
        let result = manager.call_tool("calculator", Some(args)).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_production_tool_handlers() {
        let handlers = get_production_tool_handlers();
        assert!(!handlers.is_empty());

        // Verify all handlers have valid names and schemas
        for handler in handlers {
            assert!(!handler.name().is_empty());
            let schema = handler.input_schema();
            assert_eq!(schema.schema_type, "object");

            // Verify tool definition can be created
            let tool_def = handler.tool_definition();
            assert_eq!(tool_def.name, handler.name());
        }
    }
}
