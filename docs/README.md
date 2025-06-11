# MCP Server Documentation

This directory contains comprehensive documentation for the MCP (Model Context Protocol) server implementation using HTTP transport with Actix Web framework.

## Architecture Overview

The MCP server is built with a clean, modular architecture that separates concerns between protocol handling, transport layers, and business logic. The implementation follows the MCP specification (2025-03-26) and provides compatibility with MCP Inspector client.

## Documentation Structure

### Core Architecture Diagrams

1. **[Architecture Overview](./architecture-overview.md)** - High-level system architecture showing all major components and their relationships
2. **[HTTP Request Flow](./http-request-flow.md)** - Complete request/response lifecycle from client to server
3. **[SSE Connection Flow](./sse-connection-flow.md)** - Server-Sent Events connection establishment and message handling
4. **[Protocol Message Flow](./protocol-message-flow.md)** - MCP protocol message types and their processing sequence

### Feature-Specific Diagrams

5. **[Tool Registration Flow](./tool-registration-flow.md)** - Dynamic tool registration system and execution
6. **[Error Handling Flow](./error-handling-flow.md)** - Error propagation and handling throughout the system
7. **[Authentication Flow](./authentication-flow.md)** - Security mechanisms and authorization
8. **[State Management](./state-management.md)** - Server state maintenance across requests

### Technical Implementation Diagrams

9. **[Concurrency Model](./concurrency-model.md)** - Actix Web framework concurrent request handling
10. **[Session Management](./session-management.md)** - HTTP session lifecycle and cleanup

## Key Features

- **Protocol Compliance**: Full MCP 2025-03-26 specification implementation
- **HTTP Transport**: RESTful API with Server-Sent Events (SSE) streaming
- **Dynamic Tool Registration**: Extensible tool system with runtime registration
- **Session Management**: Automatic session tracking and cleanup
- **Error Handling**: Comprehensive error propagation and recovery
- **Security**: Authentication and authorization mechanisms
- **Concurrency**: Thread-safe operations with Actix Web

## Integration Points

- **MCP Inspector Compatibility**: Designed to work seamlessly with the MCP Inspector client
- **SSE Endpoint**: Default `/sse` endpoint for Server-Sent Events connections
- **RESTful API**: Standard HTTP methods for protocol operations
- **JSON-RPC**: Complete JSON-RPC 2.0 message handling

## Getting Started

To understand the system architecture, start with the [Architecture Overview](./architecture-overview.md) and then explore the specific flows that interest you.

For implementation details, refer to the source code in the `src/` directory:
- `src/protocol/handler.rs` - Main protocol message handler
- `src/transport/http.rs` - HTTP transport implementation
- `src/server/features/` - Feature implementations (tools, resources, prompts)
- `src/config.rs` - Configuration management
