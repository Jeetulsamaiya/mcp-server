# Dynamic Tool Handler System

The MCP server now features a dynamic tool handler discovery system that allows for flexible registration and configuration of tool handlers without requiring manual updates to the core `get_tool_handlers()` function.

## Overview

The dynamic tool handler system provides:

1. **Automatic Discovery**: Tool handlers can be automatically discovered and registered
2. **Configuration-Driven**: Tool enablement and configuration through config files
3. **Runtime Registration**: Support for registering new tool handlers at runtime
4. **Thread Safety**: All operations are thread-safe for async environments
5. **Backward Compatibility**: Existing code continues to work unchanged
6. **Error Handling**: Robust error handling for failed registrations or instantiations

## Architecture

### Core Components

- **`ToolHandlerRegistry`**: Global registry for managing tool handler registrations
- **`ToolHandlerDiscovery`**: Discovery engine for finding and instantiating tool handlers
- **`ToolsConfig`**: Configuration structure for tool handler settings
- **`ToolHandlerRegistration`**: Registration entry containing factory function and metadata

### Registry Pattern

The system uses a global registry pattern with factory functions:

```rust
// Register a tool handler
ToolHandlerRegistry::register(
    "my_tool",
    || Ok(Box::new(MyToolHandler)),
    priority,
    is_builtin
)?;

// Discover and instantiate handlers
let handlers = ToolHandlerDiscovery::discover_handlers(config)?;
```

## Usage

### 1. Creating Custom Tool Handlers

Implement the `ToolHandler` trait:

```rust
use async_trait::async_trait;
use mcp_server::server::features::tools::{ToolHandler, ToolResult};

pub struct MyCustomTool;

#[async_trait]
impl ToolHandler for MyCustomTool {
    fn name(&self) -> &str {
        "my_custom_tool"
    }

    fn description(&self) -> Option<String> {
        Some("My custom tool description".to_string())
    }

    fn input_schema(&self) -> ToolInputSchema {
        // Define your input schema
    }

    async fn execute(&self, arguments: Option<Value>) -> Result<ToolResult> {
        // Implement your tool logic
    }
}
```

### 2. Registering Tool Handlers

#### Manual Registration

```rust
use mcp_server::server::features::tools::ToolHandlerRegistry;

// Register with custom priority
ToolHandlerRegistry::register(
    "my_custom_tool",
    || Ok(Box::new(MyCustomTool)),
    75, // Priority (higher = loaded first)
    false // Not a built-in tool
)?;
```

#### Using the Macro

```rust
use mcp_server::register_tool_handler;

// Simple registration
register_tool_handler!(MyCustomTool, "my_custom_tool")?;

// With priority
register_tool_handler!(MyCustomTool, "my_custom_tool", 75)?;

// With priority and built-in flag
register_tool_handler!(MyCustomTool, "my_custom_tool", 75, true)?;
```

### 3. Configuration

#### TOML Configuration

```toml
[tools]
# Auto-discover built-in handlers
auto_discover_builtin = true

# Enable all discovered handlers by default
enable_all_by_default = true

# Specific handler configurations
[[tools.handlers]]
name = "echo"
enabled = true
priority = 100

[tools.handlers.config]
default_message = "Hello from dynamic system!"

[[tools.handlers]]
name = "calculator"
enabled = true
priority = 90

[[tools.handlers]]
name = "my_custom_tool"
enabled = false
priority = 50
```

#### Programmatic Configuration

```rust
use mcp_server::server::features::tools::{ToolsConfig, ToolHandlerConfig};

let config = ToolsConfig {
    handlers: vec![
        ToolHandlerConfig {
            name: "echo".to_string(),
            enabled: true,
            priority: 100,
            config: HashMap::new(),
        },
        ToolHandlerConfig {
            name: "my_custom_tool".to_string(),
            enabled: false,
            priority: 50,
            config: HashMap::new(),
        },
    ],
    auto_discover_builtin: true,
    enable_all_by_default: false,
};

let handlers = get_tool_handlers_with_config(Some(&config));
```

### 4. Discovery and Instantiation

#### Default Discovery

```rust
// Uses default configuration (all built-in handlers enabled)
let handlers = get_tool_handlers();
```

#### Custom Configuration

```rust
// Uses custom configuration
let handlers = get_tool_handlers_with_config(Some(&config));
```

#### Advanced Discovery

```rust
use mcp_server::server::features::tools::ToolHandlerDiscovery;

// Get available handler names
let available = ToolHandlerDiscovery::get_available_handler_names()?;

// Check if specific handler is available
let is_available = ToolHandlerDiscovery::is_handler_available("echo")?;

// Custom discovery with error handling
match ToolHandlerDiscovery::discover_handlers(Some(&config)) {
    Ok(handlers) => {
        println!("Discovered {} handlers", handlers.len());
        for handler in handlers {
            println!("- {}: {}", 
                handler.name(), 
                handler.description().unwrap_or_default()
            );
        }
    }
    Err(e) => {
        eprintln!("Discovery failed: {}", e);
    }
}
```

## Built-in Tool Handlers

The system includes these built-in tool handlers:

- **`echo`**: Echo back provided messages
- **`calculator`**: Perform mathematical calculations

Built-in handlers are automatically registered with high priority (100) when the system initializes.

## Error Handling

The system provides comprehensive error handling:

- **Registration Errors**: Duplicate handler names, invalid factory functions
- **Discovery Errors**: Configuration parsing, handler instantiation failures
- **Runtime Errors**: Tool execution failures, validation errors

```rust
// Handle registration errors
match ToolHandlerRegistry::register("my_tool", factory, 50, false) {
    Ok(()) => println!("Handler registered successfully"),
    Err(e) => eprintln!("Registration failed: {}", e),
}

// Handle discovery errors
let handlers = match ToolHandlerDiscovery::discover_handlers(config) {
    Ok(handlers) => handlers,
    Err(e) => {
        eprintln!("Discovery failed: {}", e);
        Vec::new() // Fallback to empty list
    }
};
```

## Migration Guide

### From Static to Dynamic

**Before:**
```rust
pub fn get_tool_handlers() -> Vec<Box<dyn ToolHandler>> {
    vec![
        Box::new(CalculatorToolHandler),
        Box::new(EchoToolHandler),
        Box::new(MyCustomTool), // Manual addition required
    ]
}
```

**After:**
```rust
// Register once during initialization
ToolHandlerRegistry::register(
    "my_custom_tool",
    || Ok(Box::new(MyCustomTool)),
    75,
    false
)?;

// Use existing function (now dynamic)
let handlers = get_tool_handlers();
```

### Backward Compatibility

Existing code continues to work without changes:

```rust
// This still works exactly as before
let handlers = get_tool_handlers();

// But now you can also use configuration
let handlers = get_tool_handlers_with_config(Some(&config));
```

## Best Practices

1. **Register Early**: Register custom handlers during application initialization
2. **Use Priorities**: Set appropriate priorities for loading order
3. **Handle Errors**: Always handle registration and discovery errors gracefully
4. **Configuration**: Use configuration files for production deployments
5. **Testing**: Test tool handlers in isolation and integration scenarios

## Examples

See the `examples/` directory for complete examples:

- `examples/dynamic_tools.rs`: Comprehensive example with custom handlers
- `examples/tool-config.toml`: Example configuration file
