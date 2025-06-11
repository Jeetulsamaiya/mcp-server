//! Example demonstrating the dynamic tool handler system.
//!
//! This example shows how to:
//! 1. Create custom tool handlers
//! 2. Register them using the registry system
//! 3. Configure tool discovery and enablement
//! 4. Use the dynamic tool system in a server

use mcp_server::{
    Config,
    server::features::tools::{
        ToolHandler, ToolResult, ToolHandlerRegistry, ToolHandlerDiscovery,
        ToolsConfig, ToolHandlerConfig, get_tool_handlers_with_config
    },
    protocol::{Content, ToolInputSchema},
    error::{McpError, Result},
};
use serde_json::Value;
use std::collections::HashMap;
use async_trait::async_trait;

/// Example custom tool handler for string manipulation
pub struct StringToolHandler;

#[async_trait]
impl ToolHandler for StringToolHandler {
    fn name(&self) -> &str {
        "string_manipulator"
    }

    fn description(&self) -> Option<String> {
        Some("Perform various string manipulation operations".to_string())
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert(
                    "text".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "description": "The text to manipulate"
                    }),
                );
                props.insert(
                    "operation".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "enum": ["uppercase", "lowercase", "reverse", "length"],
                        "description": "The operation to perform"
                    }),
                );
                props
            }),
            required: Some(vec!["text".to_string(), "operation".to_string()]),
        }
    }

    async fn execute(&self, arguments: Option<Value>) -> Result<ToolResult> {
        let args = arguments.ok_or_else(|| {
            McpError::invalid_params("String manipulator requires arguments")
        })?;

        let text = args.get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Text parameter is required"))?;

        let operation = args.get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Operation parameter is required"))?;

        let result = match operation {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            "reverse" => text.chars().rev().collect(),
            "length" => text.len().to_string(),
            _ => return Err(McpError::invalid_params("Invalid operation")),
        };

        Ok(ToolResult {
            content: vec![Content::Text {text:format!("Result: {}",result), annotations: None }],
            is_error: false,
        })
    }
}

/// Example custom tool handler for math operations
pub struct MathToolHandler;

#[async_trait]
impl ToolHandler for MathToolHandler {
    fn name(&self) -> &str {
        "advanced_math"
    }

    fn description(&self) -> Option<String> {
        Some("Perform advanced mathematical operations".to_string())
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert(
                    "operation".to_string(),
                    serde_json::json!({
                        "type": "string",
                        "enum": ["sqrt", "pow", "log", "sin", "cos"],
                        "description": "The mathematical operation to perform"
                    }),
                );
                props.insert(
                    "value".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "The input value"
                    }),
                );
                props.insert(
                    "exponent".to_string(),
                    serde_json::json!({
                        "type": "number",
                        "description": "Exponent for power operation (optional)"
                    }),
                );
                props
            }),
            required: Some(vec!["operation".to_string(), "value".to_string()]),
        }
    }

    async fn execute(&self, arguments: Option<Value>) -> Result<ToolResult> {
        let args = arguments.ok_or_else(|| {
            McpError::invalid_params("Advanced math requires arguments")
        })?;

        let operation = args.get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Operation parameter is required"))?;

        let value = args.get("value")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| McpError::invalid_params("Value parameter is required"))?;

        let result = match operation {
            "sqrt" => value.sqrt(),
            "pow" => {
                let exponent = args.get("exponent")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(2.0);
                value.powf(exponent)
            },
            "log" => value.ln(),
            "sin" => value.sin(),
            "cos" => value.cos(),
            _ => return Err(McpError::invalid_params("Invalid operation")),
        };

        Ok(ToolResult {
            content: vec![Content::Text {text:format!("Result: {}",result), annotations: None }],
            is_error: false,
        })
    }
}

fn main() -> Result<()> {
    // Initialize logging
    // tracing_subscriber::init();

    println!("Dynamic Tool Handler System Example");
    println!("===================================");

    // Clear registry for clean example
    ToolHandlerRegistry::clear()?;

    // Register built-in handlers
    ToolHandlerRegistry::register_builtin_handlers()?;
    println!("✓ Registered built-in tool handlers");

    // Register custom tool handlers
    ToolHandlerRegistry::register(
        "string_manipulator",
        || Ok(Box::new(StringToolHandler)),
        75, // Medium priority
        false, // Not built-in
    )?;
    println!("✓ Registered string manipulator tool handler");

    ToolHandlerRegistry::register(
        "advanced_math",
        || Ok(Box::new(MathToolHandler)),
        80, // Higher priority
        false, // Not built-in
    )?;
    println!("✓ Registered advanced math tool handler");

    // Example 1: Get all available handlers (default behavior)
    println!("\n1. Default discovery (all enabled):");
    let default_handlers = get_tool_handlers_with_config(None);
    for handler in &default_handlers {
        println!("   - {}: {}", handler.name(), 
                 handler.description().unwrap_or_else(|| "No description".to_string()));
    }

    // Example 2: Custom configuration with selective enablement
    println!("\n2. Custom configuration (selective enablement):");
    let custom_config = ToolsConfig {
        handlers: vec![
            ToolHandlerConfig {
                name: "echo".to_string(),
                enabled: true,
                priority: 100,
                config: HashMap::new(),
            },
            ToolHandlerConfig {
                name: "string_manipulator".to_string(),
                enabled: true,
                priority: 90,
                config: HashMap::new(),
            },
            ToolHandlerConfig {
                name: "calculator".to_string(),
                enabled: false, // Disabled
                priority: 0,
                config: HashMap::new(),
            },
        ],
        auto_discover_builtin: true,
        enable_all_by_default: false, // Only explicitly enabled handlers
    };

    let custom_handlers = get_tool_handlers_with_config(Some(&custom_config));
    for handler in &custom_handlers {
        println!("   - {}: {}", handler.name(), 
                 handler.description().unwrap_or_else(|| "No description".to_string()));
    }

    // Example 3: Discovery information
    println!("\n3. Discovery information:");
    let available_names = ToolHandlerDiscovery::get_available_handler_names()?;
    println!("   Available handlers: {:?}", available_names);

    for name in &available_names {
        let is_available = ToolHandlerDiscovery::is_handler_available(name)?;
        println!("   {} is available: {}", name, is_available);
    }

    println!("\n✓ Dynamic tool handler system example completed successfully!");
    Ok(())
}
