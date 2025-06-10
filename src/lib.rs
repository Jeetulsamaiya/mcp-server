//! # MCP Server
//!
//! A production-ready Model Context Protocol (MCP) server implementation in Rust.
//!
//! This library provides a complete implementation of the MCP specification (2025-03-26)
//! with support for both HTTP and STDIO transports, comprehensive error handling,
//! and all core MCP features including resources, tools, prompts, and sampling.
//!
//! ## Features
//!
//! - **Complete MCP Protocol Support**: Full implementation of MCP 2025-03-26 specification
//! - **Multiple Transports**: HTTP (with SSE) and STDIO transport support
//! - **Server Features**: Resources, tools, prompts with template support
//! - **Client Features**: LLM sampling and root directory management
//! - **Production Ready**: Comprehensive error handling, logging, and configuration
//! - **Security**: Built-in authentication and authorization mechanisms
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use mcp_server::{McpServer, Config, transport::HttpTransport};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::default();
//!     let transport = HttpTransport::new("127.0.0.1:8080").await?;
//!     let server = McpServer::new(config, transport);
//!     
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod error;
pub mod protocol;
pub mod transport;
pub mod server;
pub mod client;
pub mod utils;

// Re-export main types for convenience
pub use config::Config;
pub use error::{McpError, Result};
pub use protocol::{
    JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, JsonRpcNotification,
    InitializeRequest, InitializeResult, ServerCapabilities, ClientCapabilities,
};
pub use server::McpServer;

/// Current MCP protocol version supported by this implementation
pub const PROTOCOL_VERSION: &str = "2025-03-26";

/// JSON-RPC version used by MCP
pub const JSONRPC_VERSION: &str = "2.0";

/// Default server information
pub const SERVER_NAME: &str = "mcp-server-rust";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
