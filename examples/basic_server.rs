//! Basic MCP server example.
//!
//! This example demonstrates how to create a simple MCP server with
//! basic resource and tool providers.

use mcp_server::{
    Config, McpServerBuilder,
    server::features::{
        ResourceManager, ToolManager, PromptManager,
        resources::{FileSystemProvider, HttpProvider},
        tools::{EchoToolHandler, CalculatorToolHandler},
        prompts::{GreetingPromptGenerator, CodeReviewPromptGenerator},
    },
    protocol::{Resource, Tool, Prompt, ToolInputSchema, PromptArgument},
    transport::http::HttpTransport,
};
use std::path::PathBuf;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();

    // Create configuration
    let mut config = Config::default();
    config.server.name = "Basic MCP Server".to_string();
    config.server.version = "1.0.0".to_string();
    config.server.instructions = Some("A basic MCP server example with file resources and simple tools".to_string());

    // Create server
    let server = McpServerBuilder::new()
        .config(config)
        .build()?;

    // Set up resources
    setup_resources(&server).await?;
    
    // Set up tools
    setup_tools(&server).await?;
    
    // Set up prompts
    setup_prompts(&server).await?;

    println!("Starting basic MCP server on http://127.0.0.1:8080/mcp");
    println!("Press Ctrl+C to stop");

    // Run the server
    server.run().await?;

    Ok(())
}

async fn setup_resources(server: &mcp_server::McpServer) -> Result<(), Box<dyn std::error::Error>> {
    // This is a simplified example - in the actual implementation,
    // you would need to access the resource manager through the server
    println!("Setting up resources...");
    
    // Example resources that would be registered
    let example_resource = Resource {
        uri: "file:///tmp/example.txt".to_string(),
        name: "Example Text File".to_string(),
        description: Some("An example text file resource".to_string()),
        mime_type: Some("text/plain".to_string()),
        annotations: None,
        size: Some(1024),
    };

    println!("Registered resource: {}", example_resource.name);
    Ok(())
}

async fn setup_tools(server: &mcp_server::McpServer) -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up tools...");
    
    // Example tools that would be registered
    let echo_tool = Tool {
        name: "echo".to_string(),
        description: Some("Echo back the provided message".to_string()),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert("message".to_string(), serde_json::json!({
                    "type": "string",
                    "description": "The message to echo back"
                }));
                props
            }),
            required: Some(vec!["message".to_string()]),
        },
        annotations: None,
    };

    let calculator_tool = Tool {
        name: "calculator".to_string(),
        description: Some("Perform basic arithmetic operations".to_string()),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = HashMap::new();
                props.insert("operation".to_string(), serde_json::json!({
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                }));
                props.insert("a".to_string(), serde_json::json!({
                    "type": "number",
                    "description": "The first operand"
                }));
                props.insert("b".to_string(), serde_json::json!({
                    "type": "number",
                    "description": "The second operand"
                }));
                props
            }),
            required: Some(vec!["operation".to_string(), "a".to_string(), "b".to_string()]),
        },
        annotations: None,
    };

    println!("Registered tools: {}, {}", echo_tool.name, calculator_tool.name);
    Ok(())
}

async fn setup_prompts(server: &mcp_server::McpServer) -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up prompts...");
    
    // Example prompts that would be registered
    let greeting_prompt = Prompt {
        name: "greeting".to_string(),
        description: Some("Generate a personalized greeting".to_string()),
        arguments: Some(vec![
            PromptArgument {
                name: "name".to_string(),
                description: Some("The name of the person to greet".to_string()),
                required: Some(false),
            },
            PromptArgument {
                name: "time_of_day".to_string(),
                description: Some("The time of day (morning, afternoon, evening, night)".to_string()),
                required: Some(false),
            },
        ]),
    };

    let code_review_prompt = Prompt {
        name: "code_review".to_string(),
        description: Some("Generate a code review prompt".to_string()),
        arguments: Some(vec![
            PromptArgument {
                name: "code".to_string(),
                description: Some("The code to review".to_string()),
                required: Some(true),
            },
            PromptArgument {
                name: "language".to_string(),
                description: Some("The programming language".to_string()),
                required: Some(false),
            },
            PromptArgument {
                name: "focus".to_string(),
                description: Some("The focus area for the review".to_string()),
                required: Some(false),
            },
        ]),
    };

    println!("Registered prompts: {}, {}", greeting_prompt.name, code_review_prompt.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = Config::default();
        let server = McpServerBuilder::new()
            .config(config)
            .build();
        
        assert!(server.is_ok());
    }
}
