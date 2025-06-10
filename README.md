# MCP Server - Rust Implementation

A Test Model Context Protocol (MCP) server implementation in Rust, following the official MCP specification (2025-03-26).

## Features

### Core MCP Protocol Support

- **Complete MCP 2025-03-26 Implementation**: Full compliance with the latest MCP specification
- **JSON-RPC 2.0**: Proper message format and error handling
- **Server Capabilities Negotiation**: Dynamic capability discovery and configuration
- **Request/Response Validation**: Comprehensive validation according to the specification

### Transport Layers

- **HTTP Transport**: RESTful API with Server-Sent Events (SSE) for streaming
- **STDIO Transport**: Standard input/output for subprocess communication
- **Session Management**: HTTP session tracking with automatic cleanup
- **CORS Support**: Configurable cross-origin resource sharing

### Server Features

- **Resources**: File system and HTTP resource providers with subscription support
- **Tools**: Extensible tool execution framework with validation
- **Prompts**: Template-based prompt generation with Handlebars support
- **Logging**: Structured logging with multiple levels and formats
- **Completion**: Argument completion for prompts and resources

### Client Features

- **Sampling**: LLM sampling integration with multiple providers
- **Roots**: Root directory management for secure file access

### Security & Production Features

- **Authentication**: API key and JWT token support
- **Authorization**: Role-based access control
- **Configuration Management**: TOML-based configuration with validation
- **Error Handling**: Comprehensive error handling with proper MCP error codes
- **Logging**: Structured logging with configurable levels and formats

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/example/mcp-server-rust
cd mcp-server-rust

# Build the project
cargo build --release

# Install the binary
cargo install --path .
```

### Basic Usage

#### Start HTTP Server

```bash
# Start with default settings (HTTP on localhost:8080)
mcp-server start

# Start with custom settings
mcp-server start --bind 0.0.0.0 --port 9090 --name "My MCP Server"
```

#### Start STDIO Server

```bash
# Start STDIO transport for subprocess communication
mcp-server start --stdio
```

#### Generate Configuration

```bash
# Generate default configuration file
mcp-server config --output mcp-server.toml

# Validate configuration
mcp-server validate mcp-server.toml
```

### Configuration

Create a configuration file (`mcp-server.toml`):

```toml
[server]
name = "My MCP Server"
version = "1.0.0"
instructions = "A helpful MCP server"
max_connections = 100
request_timeout = 30

[transport]
transport_type = "http"

[transport.http]
bind_address = "127.0.0.1"
port = 8080
endpoint_path = "/mcp"
enable_cors = true
cors_origins = ["*"]
session_timeout = 3600

[auth]
enabled = false
method = "none"

[logging]
level = "info"
format = "pretty"
enable_request_logging = false

[features]
resources = true
tools = true
prompts = true
sampling = true
logging = true
completion = true
roots = true
```

## API Usage

### HTTP Transport

The server exposes a RESTful API at the configured endpoint:

- `POST /mcp` - Send JSON-RPC messages
- `GET /mcp` - Establish SSE connection for streaming
- `DELETE /mcp` - Terminate session

#### Example Request

```bash
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-03-26",
      "capabilities": {},
      "clientInfo": {
        "name": "example-client",
        "version": "1.0.0"
      }
    }
  }'
```

### STDIO Transport

For subprocess communication, use STDIO transport:

```bash
echo '{"jsonrpc":"2.0","id":"1","method":"ping"}' | mcp-server start --stdio
```

## Development

### Project Structure

```ini
src/
├── main.rs              # CLI application entry point
├── lib.rs               # Library exports
├── config.rs            # Configuration management
├── error.rs             # Error handling
├── protocol/            # MCP protocol implementation
│   ├── mod.rs
│   ├── messages.rs      # Message types
│   ├── validation.rs    # Message validation
│   └── handler.rs       # Protocol handler
├── transport/           # Transport layer
│   ├── mod.rs
│   ├── http.rs          # HTTP transport
│   ├── stdio.rs         # STDIO transport
│   └── session.rs       # Session management
├── server/              # Server features
│   ├── mod.rs
│   └── features/
│       ├── mod.rs
│       ├── resources.rs # Resource management
│       ├── tools.rs     # Tool execution
│       ├── prompts.rs   # Prompt templates
│       ├── logging.rs   # Logging feature
│       └── completion.rs # Completion feature
├── client/              # Client features
│   └── features/
│       ├── mod.rs
│       ├── sampling.rs  # LLM sampling
│       └── roots.rs     # Root directories
└── utils/               # Utilities
    ├── mod.rs
    ├── logging.rs       # Logging setup
    ├── auth.rs          # Authentication
    └── validation.rs    # Additional validation
```

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- start --verbose
```

### Adding Custom Features

#### Custom Tool Handler

```rust
use mcp_server::server::features::{ToolHandler, ToolResult};
use async_trait::async_trait;

pub struct MyToolHandler;

#[async_trait]
impl ToolHandler for MyToolHandler {
    fn name(&self) -> &str {
        "my_tool"
    }

    async fn execute(&self, arguments: Option<serde_json::Value>) -> mcp_server::Result<ToolResult> {
        // Your tool implementation
        Ok(ToolResult::text("Tool executed successfully".to_string()))
    }
}
```

#### Custom Resource Provider

```rust
use mcp_server::server::features::{ResourceProvider, ResourceContents};
use async_trait::async_trait;

pub struct MyResourceProvider;

#[async_trait]
impl ResourceProvider for MyResourceProvider {
    fn name(&self) -> &str {
        "my_provider"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("my://")
    }

    async fn read_resource(&self, uri: &str) -> mcp_server::Result<Vec<ResourceContents>> {
        // Your resource implementation
        Ok(vec![])
    }
}
```

## Testing

The project includes comprehensive tests:

```bash
# Run all tests
cargo test

# Run specific test module
cargo test protocol::validation

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Ensure all tests pass (`cargo test`)
- Add documentation for public APIs
- Follow the existing error handling patterns

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Model Context Protocol Specification](https://modelcontextprotocol.io/specification/2025-03-26)
- [Actix Web](https://actix.rs/) for the HTTP framework
- [Tokio](https://tokio.rs/) for async runtime
- [Serde](https://serde.rs/) for serialization

## Support

- [Documentation](https://docs.rs/mcp-server)
- [Issues](https://github.com/example/mcp-server-rust/issues)
- [Discussions](https://github.com/example/mcp-server-rust/discussions)
