# MCP Server - Rust Implementation

A Test Model Context Protocol (MCP) server implementation in Rust, following the official MCP specification (2025-03-26). This server provides a complete implementation of the MCP protocol with support for both HTTP and STDIO transports, comprehensive error handling, and all core MCP features.

## Features

### Core MCP Protocol Support

- **Complete MCP 2025-03-26 Implementation**: Full compliance with the latest MCP specification
- **JSON-RPC 2.0**: Proper message format and error handling
- **Server Capabilities Negotiation**: Dynamic capability discovery and configuration
- **Request/Response Validation**: Comprehensive validation according to the specification

### Transport Layers

- **HTTP Transport**: Streamable HTTP transport with optional SSE streaming
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
git clone <repository-url>
cd test-sse-mcp-server

# Build the project
cargo build --release

# Install the binary (optional)
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

# Show server information
mcp-server info
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

The server exposes a Streamable HTTP transport API at the configured endpoint (default: `/mcp`):

- `POST /mcp` - Send JSON-RPC messages (single requests or batches)
- `GET /mcp` - Establish Server-Sent Events (SSE) connection for streaming
- `DELETE /mcp` - Terminate session and cleanup resources

#### Session Management

The server automatically manages HTTP sessions with:

- Session ID tracking via `Mcp-Session-Id` header
- Automatic session creation for new clients
- Configurable session timeout (default: 1 hour)
- Session cleanup on termination

#### Example Requests

**Initialize Connection:**

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

**List Available Tools:**

```bash
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "2",
    "method": "tools/list"
  }'
```

**Establish SSE Stream:**

```bash
curl -X GET http://localhost:8080/mcp \
  -H "Accept: text/event-stream" \
  -H "Cache-Control: no-cache"
```

### STDIO Transport

For subprocess communication, use STDIO transport:

```bash
# Start STDIO server
mcp-server start --stdio

# Send JSON-RPC messages via stdin
echo '{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"stdio-client","version":"1.0.0"}}}' | mcp-server start --stdio
```

## Development

### Project Structure

```ini
src/
├── main.rs              # CLI application entry point
├── lib.rs               # Library exports and public API
├── config.rs            # Configuration management (TOML/JSON)
├── error.rs             # Error handling and MCP error codes
├── protocol/            # MCP protocol implementation
│   ├── mod.rs           # Protocol exports
│   ├── messages.rs      # MCP message types and serialization
│   ├── validation.rs    # Request/response validation
│   └── handler.rs       # Central protocol message handler
├── transport/           # Transport layer implementations
│   ├── mod.rs           # Transport abstractions
│   ├── http.rs          # HTTP transport with SSE streaming
│   ├── stdio.rs         # STDIO transport for subprocess
│   └── session.rs       # HTTP session lifecycle management
├── server/              # Server-side MCP features
│   ├── mod.rs           # Server implementation
│   └── features/        # Feature managers
│       ├── mod.rs       # Feature exports
│       ├── resources.rs # Resource management (file/HTTP)
│       ├── tools.rs     # Tool execution framework
│       ├── prompts.rs   # Prompt template engine
│       ├── logging.rs   # Logging feature
│       └── completion.rs # Argument completion
├── client/              # Client-side MCP features
│   └── features/        # Client feature managers
│       ├── mod.rs       # Client feature exports
│       ├── sampling.rs  # LLM sampling integration
│       └── roots.rs     # Root directory management
└── utils/               # Shared utilities
    ├── mod.rs           # Utility exports
    ├── logging.rs       # Logging configuration
    ├── auth.rs          # Authentication helpers
    └── validation.rs    # Additional validation utilities

docs/                    # Comprehensive documentation
├── README.md            # Documentation index
├── architecture-overview.md
├── http-request-flow.md
├── http-streaming-flow.md
├── protocol-message-flow.md
├── tool-registration-flow.md
├── error-handling-flow.md
├── authentication-flow.md
├── state-management.md
├── session-management.md
├── concurrency-model.md
├── system-initialization-flow.md
└── sequence-interaction-flow.md

examples/                # Usage examples
├── basic_server.rs      # Basic server setup example
└── mcp-server.toml      # Example configuration file
```

### Building and Testing

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test protocol::validation

# Run with debug logging
RUST_LOG=debug cargo run -- start --verbose

# Check code formatting
cargo fmt --check

# Run clippy lints
cargo clippy -- -D warnings
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

The project includes comprehensive tests covering all major components:

### Unit Tests

```bash
# Run all unit tests
cargo test

# Run specific module tests
cargo test protocol::validation
cargo test transport::http
cargo test server::features

# Run with output for debugging
cargo test -- --nocapture
```

### Integration Tests

```bash
# Run integration tests (if available)
cargo test --test integration

# Test with MCP Inspector client
# 1. Start the server: cargo run -- start
# 2. Connect MCP Inspector to http://localhost:8080/mcp
# 3. Test protocol compliance and feature functionality
```

### Manual Testing

```bash
# Test HTTP transport
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'

# Test STDIO transport
echo '{"jsonrpc":"2.0","id":"1","method":"tools/list"}' | cargo run -- start --stdio
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

## Documentation

For detailed technical documentation, see the [docs/](./docs/) directory:

- **[Architecture Overview](./docs/architecture-overview.md)** - System architecture and component relationships
- **[HTTP Request Flow](./docs/http-request-flow.md)** - HTTP transport request/response lifecycle
- **[Protocol Message Flow](./docs/protocol-message-flow.md)** - MCP protocol message handling
- **[Tool Registration Flow](./docs/tool-registration-flow.md)** - Dynamic tool registration system
- **[Error Handling Flow](./docs/error-handling-flow.md)** - Error propagation and handling
- **[Session Management](./docs/session-management.md)** - HTTP session lifecycle
- **[Authentication Flow](./docs/authentication-flow.md)** - Security and authorization

## Acknowledgments

- [Model Context Protocol Specification](https://modelcontextprotocol.io/specification/2025-03-26) - Official MCP specification
- [Actix Web](https://actix.rs/) - High-performance HTTP framework
- [Tokio](https://tokio.rs/) - Asynchronous runtime for Rust
- [Serde](https://serde.rs/) - Serialization framework
- [Tracing](https://tracing.rs/) - Structured logging and diagnostics
- [Clap](https://clap.rs/) - Command-line argument parsing

## Support

- **Documentation**: See [docs/](./docs/) directory for comprehensive guides
- **Examples**: Check [examples/](./examples/) directory for usage examples
- **Issues**: Report bugs and feature requests via repository issues
- **Configuration**: Use `mcp-server config` to generate example configuration files
