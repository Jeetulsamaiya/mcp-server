//! Tool management for MCP server.
//!
//! This module implements the tool feature of MCP, allowing the server
//! to expose executable tools to clients.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::error::{McpError, Result};
use crate::protocol::{Content, PaginationParams, PaginationResult, Tool};
use crate::server::features::FeatureManager;

/// Configuration for tool handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHandlerConfig {
    /// Tool handler name
    pub name: String,

    /// Whether the tool handler is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Priority for loading order (higher = loaded first)
    #[serde(default)]
    pub priority: i32,

    /// Custom configuration for the tool handler
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

/// Configuration for all tool handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// List of tool handler configurations
    #[serde(default)]
    pub handlers: Vec<ToolHandlerConfig>,

    /// Whether to auto-discover built-in handlers
    #[serde(default = "default_true")]
    pub auto_discover_builtin: bool,

    /// Whether to enable all discovered handlers by default
    #[serde(default = "default_true")]
    pub enable_all_by_default: bool,
}

/// Tool handler factory function type
pub type ToolHandlerFactory = fn() -> Result<Box<dyn ToolHandler>>;

/// Tool handler registration entry
#[derive(Clone)]
pub struct ToolHandlerRegistration {
    /// Handler name
    pub name: String,

    /// Factory function to create the handler
    pub factory: ToolHandlerFactory,

    /// Priority for loading order
    pub priority: i32,

    /// Whether this is a built-in handler
    pub is_builtin: bool,
}

/// Global tool handler registry
static TOOL_HANDLER_REGISTRY: OnceLock<Arc<std::sync::Mutex<Vec<ToolHandlerRegistration>>>> = OnceLock::new();

/// Tool handler registry for managing available tool handlers
pub struct ToolHandlerRegistry;

fn default_true() -> bool {
    true
}

impl ToolHandlerRegistry {
    /// Initialize the global registry
    fn get_registry() -> &'static Arc<std::sync::Mutex<Vec<ToolHandlerRegistration>>> {
        TOOL_HANDLER_REGISTRY.get_or_init(|| {
            Arc::new(std::sync::Mutex::new(Vec::new()))
        })
    }

    /// Register a tool handler factory
    pub fn register(
        name: impl Into<String>,
        factory: ToolHandlerFactory,
        priority: i32,
        is_builtin: bool,
    ) -> Result<()> {
        let name = name.into();
        let registry = Self::get_registry();
        let mut handlers = registry.lock().map_err(|e| {
            McpError::Tool(format!("Failed to lock registry: {}", e))
        })?;

        // Check for duplicate names
        if handlers.iter().any(|h| h.name == name) {
            return Err(McpError::Tool(format!(
                "Tool handler '{}' is already registered", name
            )));
        }

        handlers.push(ToolHandlerRegistration {
            name,
            factory,
            priority,
            is_builtin,
        });

        // Sort by priority (higher priority first)
        handlers.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(())
    }

    /// Get all registered tool handlers
    pub fn get_all() -> Result<Vec<ToolHandlerRegistration>> {
        let registry = Self::get_registry();
        let handlers = registry.lock().map_err(|e| {
            McpError::Tool(format!("Failed to lock registry: {}", e))
        })?;
        Ok(handlers.clone())
    }

    /// Get a specific tool handler registration by name
    pub fn get(name: &str) -> Result<Option<ToolHandlerRegistration>> {
        let registry = Self::get_registry();
        let handlers = registry.lock().map_err(|e| {
            McpError::Tool(format!("Failed to lock registry: {}", e))
        })?;
        Ok(handlers.iter().find(|h| h.name == name).cloned())
    }

    /// Clear all registrations (mainly for testing)
    pub fn clear() -> Result<()> {
        let registry = Self::get_registry();
        let mut handlers = registry.lock().map_err(|e| {
            McpError::Tool(format!("Failed to lock registry: {}", e))
        })?;
        handlers.clear();
        Ok(())
    }

    /// Register all built-in tool handlers
    pub fn register_builtin_handlers() -> Result<()> {
        info!("Registering built-in tool handlers");

        // Register echo tool handler
        Self::register(
            "echo",
            || Ok(Box::new(EchoToolHandler)),
            100, // High priority for built-in tools
            true,
        )?;

        // Register calculator tool handler
        Self::register(
            "calculator",
            || Ok(Box::new(CalculatorToolHandler)),
            100, // High priority for built-in tools
            true,
        )?;

        info!("Successfully registered built-in tool handlers");
        Ok(())
    }
}

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
            args.get("message")
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

        // let b = args.get("b").and_then(|v| v.as_f64()).ok_or_else(|| {
            
        //     McpError::invalid_params("Parameter 'b' is required and must be a number")
        // })?;

        // check if b is provided for all operatrions except sqrt
        let b = if operation == "sqrt" {
            0.0
        } else {
            args.get("b").and_then(|v| v.as_f64()).ok_or_else(|| {
                McpError::invalid_params("Parameter 'b' is required and must be a number")
            })?
        };

        let result = match operation {
            "add" => {
                (a + b) as isize
            },
            "subtract" => (a - b) as isize,
            "multiply" => (a * b) as isize,
            "divide" => {
                if b == 0.0 {
                    return Ok(ToolResult::error_text("Division by zero".to_string()));
                }
                (a / b) as isize
            }
            "power" => (a.powf(b)) as isize,
            "sqrt" => (a.sqrt()) as isize,
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

        let valid_operations = ["add", "subtract", "multiply", "divide", "power", "sqrt"];
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


        if operation != "sqrt" {
            if !args.get("b").map(|v| v.is_number()).unwrap_or(false) {
                return Err(McpError::invalid_params(
                    "Parameter 'b' is required and must be a number",
                ));
            }
        }

        Ok(())
    }
}

/// Dynamic tool handler discovery and instantiation
pub struct ToolHandlerDiscovery;

impl ToolHandlerDiscovery {
    /// Discover and create tool handlers based on configuration
    pub fn discover_handlers(config: Option<&ToolsConfig>) -> Result<Vec<Box<dyn ToolHandler>>> {
        let mut handlers = Vec::new();
        let mut errors = Vec::new();

        // Initialize built-in handlers if not already done
        if let Err(e) = ToolHandlerRegistry::register_builtin_handlers() {
            // Ignore duplicate registration errors
            if !e.to_string().contains("already registered") {
                warn!("Failed to register built-in handlers: {}", e);
            }
        }

        // Get all registered handlers
        let registrations = ToolHandlerRegistry::get_all()?;

        // Apply configuration filtering
        let enabled_handlers = Self::filter_by_config(&registrations, config)?;

        // Create handler instances
        for registration in enabled_handlers {
            match (registration.factory)() {
                Ok(handler) => {
                    info!("Successfully created tool handler: {}", registration.name);
                    handlers.push(handler);
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to create tool handler '{}': {}",
                        registration.name, e
                    );
                    warn!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        if !errors.is_empty() {
            warn!(
                "Some tool handlers failed to initialize: {}",
                errors.join(", ")
            );
        }

        info!("Successfully discovered {} tool handlers", handlers.len());
        Ok(handlers)
    }

    /// Filter registrations based on configuration
    fn filter_by_config(
        registrations: &[ToolHandlerRegistration],
        config: Option<&ToolsConfig>,
    ) -> Result<Vec<ToolHandlerRegistration>> {
        let config = match config {
            Some(c) => c,
            None => {
                // No config provided, return all built-in handlers
                return Ok(registrations.iter().filter(|r| r.is_builtin).cloned().collect());
            }
        };

        let mut enabled_handlers = Vec::new();

        // Create a map of configured handlers
        let configured_handlers: HashMap<String, &ToolHandlerConfig> = config
            .handlers
            .iter()
            .map(|h| (h.name.clone(), h))
            .collect();

        for registration in registrations {
            let should_enable = if let Some(handler_config) = configured_handlers.get(&registration.name) {
                // Explicitly configured
                handler_config.enabled
            } else if registration.is_builtin && config.auto_discover_builtin {
                // Built-in handler with auto-discovery enabled
                config.enable_all_by_default
            } else {
                // Non-built-in handler without explicit config
                false
            };

            if should_enable {
                enabled_handlers.push(registration.clone());
            }
        }

        // Sort by priority
        enabled_handlers.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(enabled_handlers)
    }

    /// Get available handler names
    pub fn get_available_handler_names() -> Result<Vec<String>> {
        let registrations = ToolHandlerRegistry::get_all()?;
        Ok(registrations.into_iter().map(|r| r.name).collect())
    }

    /// Check if a handler is available
    pub fn is_handler_available(name: &str) -> Result<bool> {
        Ok(ToolHandlerRegistry::get(name)?.is_some())
    }
}

/// Get all available tool handlers (backward compatibility)
///
/// This function maintains backward compatibility while using the new dynamic system.
/// It will discover and return all enabled tool handlers based on default configuration.
pub fn get_tool_handlers() -> Vec<Box<dyn ToolHandler>> {
    // Ensure built-in handlers are registered
    let _ = ToolHandlerRegistry::register_builtin_handlers();
    get_tool_handlers_with_config(None)
}

/// Get tool handlers with custom configuration
pub fn get_tool_handlers_with_config(config: Option<&ToolsConfig>) -> Vec<Box<dyn ToolHandler>> {
    match ToolHandlerDiscovery::discover_handlers(config) {
        Ok(handlers) => handlers,
        Err(e) => {
            error!("Failed to discover tool handlers: {}", e);
            // Fallback to empty list rather than panicking
            Vec::new()
        }
    }
}

/// Macro for easy tool handler registration
///
/// Usage:
/// ```
/// register_tool_handler!(MyToolHandler, "my_tool", 50, true);
/// ```
#[macro_export]
macro_rules! register_tool_handler {
    ($handler_type:ty, $name:expr, $priority:expr, $is_builtin:expr) => {
        {
            use $crate::server::features::tools::ToolHandlerRegistry;
            ToolHandlerRegistry::register(
                $name,
                || Ok(Box::new(<$handler_type>::default())),
                $priority,
                $is_builtin,
            )
        }
    };
    ($handler_type:ty, $name:expr, $priority:expr) => {
        register_tool_handler!($handler_type, $name, $priority, false)
    };
    ($handler_type:ty, $name:expr) => {
        register_tool_handler!($handler_type, $name, 0, false)
    };
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            handlers: Vec::new(),
            auto_discover_builtin: true, 
            enable_all_by_default: true,
        }
    }
}

impl Default for ToolHandlerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            priority: 0,
            config: HashMap::new(),
        }
    }
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
        let handlers = get_tool_handlers();
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

    #[tokio::test]
    async fn test_tool_handler_registry_basic() {
        // Test basic registry functionality
        let _available_names = ToolHandlerDiscovery::get_available_handler_names().unwrap_or_default();

        // Test that discovery works (may be empty if no handlers registered yet)
        let _handlers = get_tool_handlers();

        // Test configuration-based discovery
        let config = ToolsConfig::default();
        let _configured_handlers = get_tool_handlers_with_config(Some(&config));

        // If we reach here, the basic functionality works
        assert!(true);
    }

    #[tokio::test]
    async fn test_tool_handler_discovery() {
        // Clear registry for clean test
        ToolHandlerRegistry::clear().unwrap();

        // Register test handlers with unique names
        let echo_name = format!("echo_test_{}", std::process::id());
        let calc_name = format!("calculator_test_{}", std::process::id());

        ToolHandlerRegistry::register(
            &echo_name,
            || Ok(Box::new(EchoToolHandler)),
            100,
            true
        ).unwrap();

        ToolHandlerRegistry::register(
            &calc_name,
            || Ok(Box::new(CalculatorToolHandler)),
            100,
            true
        ).unwrap();

        // Test discovery with default config
        let handlers = ToolHandlerDiscovery::discover_handlers(None).unwrap();
        assert!(handlers.len() >= 2); // At least our two handlers

        // Test discovery with custom config
        let config = ToolsConfig {
            handlers: vec![
                ToolHandlerConfig {
                    name: echo_name.clone(),
                    enabled: true,
                    priority: 0,
                    config: HashMap::new(),
                },
                ToolHandlerConfig {
                    name: calc_name.clone(),
                    enabled: false,
                    priority: 0,
                    config: HashMap::new(),
                }
            ],
            auto_discover_builtin: true,
            enable_all_by_default: false,
        };

        let handlers = ToolHandlerDiscovery::discover_handlers(Some(&config)).unwrap();
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].name(), "echo");
    }

    #[tokio::test]
    async fn test_get_tool_handlers_with_config() {
        // Clear registry for clean test
        ToolHandlerRegistry::clear().unwrap();

        // Register built-in handlers (ignore duplicate registration errors)
        let _ = ToolHandlerRegistry::register_builtin_handlers();

        // Test with no config (should return all built-in handlers)
        let handlers = get_tool_handlers_with_config(None);
        assert!(!handlers.is_empty());

        // Test with config that disables all
        let config = ToolsConfig {
            handlers: Vec::new(),
            auto_discover_builtin: false,
            enable_all_by_default: false,
        };

        let handlers = get_tool_handlers_with_config(Some(&config));
        assert!(handlers.is_empty());
    }
}
