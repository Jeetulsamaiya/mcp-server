# MCP Server Documentation

This directory contains comprehensive documentation for the MCP (Model Context Protocol) server implementation in Rust. The server provides a production-ready implementation of the MCP specification (2025-03-26) with streamable HTTP transport support, built using the Actix Web framework.

## Architecture Overview

The MCP server is built with a clean, modular architecture that separates concerns between protocol handling, transport layers, and business logic. The implementation follows the MCP specification (2025-03-26) and provides full compatibility with MCP Inspector client and other MCP-compliant clients.

## Documentation Structure

### Core Architecture Diagrams

1. **[Architecture Overview](./architecture-overview.md)** - High-level system architecture showing all major components and their relationships
2. **[HTTP Request Flow](./http-request-flow.md)** - Complete request/response lifecycle from client to server
3. **[HTTP Streaming Flow](./http-streaming-flow.md)** - Streamable HTTP transport connection establishment and message handling
4. **[Protocol Message Flow](./protocol-message-flow.md)** - MCP protocol message types and their processing sequence

### Feature-Specific Diagrams

5. **[Tool Registration Flow](./tool-registration-flow.md)** - Dynamic tool registration system and execution
6. **[Error Handling Flow](./error-handling-flow.md)** - Error propagation and handling throughout the system
7. **[Authentication Flow](./authentication-flow.md)** - Security mechanisms and authorization
8. **[State Management](./state-management.md)** - Server state maintenance across requests

### Technical Implementation Diagrams

9. **[Concurrency Model](./concurrency-model.md)** - Actix Web framework concurrent request handling
10. **[Session Management](./session-management.md)** - HTTP session lifecycle and cleanup
11. **[System Initialization Flow](./system-initialization-flow.md)** - Server startup and component initialization
12. **[Sequence Interaction Flow](./sequence-interaction-flow.md)** - Client-server interaction sequences

## Key Features

- **Protocol Compliance**: Full MCP 2025-03-26 specification implementation
- **HTTP Transport Support**: HTTP with SSE streaming for real-time communication
- **Dynamic Tool Registration**: Extensible tool system with runtime registration capabilities
- **Session Management**: Automatic HTTP session tracking, cleanup, and timeout handling
- **Error Handling**: Comprehensive error propagation with proper MCP error codes
- **Security**: Authentication and authorization with API key and JWT support
- **Concurrency**: Thread-safe operations with Actix Web's actor-based architecture
- **Configuration**: Flexible TOML-based configuration with validation
- **Logging**: Structured logging with tracing and multiple output formats

## Integration Points

- **MCP Inspector Compatibility**: Designed to work seamlessly with the MCP Inspector client from modelcontextprotocol/inspector
- **Streamable HTTP Transport**: Default `/mcp` endpoint supporting both single requests and SSE streaming
- **JSON-RPC 2.0**: Complete message handling with proper error responses and batch support
- **CORS Support**: Configurable cross-origin resource sharing for web clients
- **Session Persistence**: HTTP session management with automatic cleanup and timeout handling

## Getting Started

To understand the system architecture, start with the [Architecture Overview](./architecture-overview.md) and then explore the specific flows that interest you.

### Recommended Reading Order

1. **[Architecture Overview](./architecture-overview.md)** - Understand the overall system design
2. **[HTTP Request Flow](./http-request-flow.md)** - Learn how HTTP requests are processed
3. **[Protocol Message Flow](./protocol-message-flow.md)** - Understand MCP message handling
4. **[Tool Registration Flow](./tool-registration-flow.md)** - See how dynamic tools work
5. **[Session Management](./session-management.md)** - Learn about session lifecycle

### Implementation Reference

For implementation details, refer to the source code in the `src/` directory:
- `src/protocol/handler.rs` - Central protocol message handler and routing
- `src/transport/http.rs` - HTTP transport with SSE streaming implementation
- `src/server/features/` - Feature implementations (tools, resources, prompts, etc.)
- `src/config.rs` - Configuration management and validation
- `src/error.rs` - Error handling and MCP error code mapping
