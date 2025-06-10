//! MCP Server implementation.
//!
//! This module contains the main server implementation and feature managers
//! for resources, tools, prompts, and other MCP server capabilities.

pub mod features;

use std::sync::Arc;
use tracing::{error, info, warn};

use crate::client::features::SamplingManager;
use crate::config::Config;
use crate::error::Result;
use crate::protocol::handler::ProtocolHandler;
use crate::server::features::{PromptManager, ResourceManager, ToolManager};
use crate::transport::{Transport, TransportFactory, TransportManager};

/// Main MCP server implementation
pub struct McpServer {
    /// Server configuration
    config: Config,

    /// Transport manager
    transport_manager: TransportManager,

    /// Protocol handler
    protocol_handler: Arc<ProtocolHandler>,

    /// Server running state
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl McpServer {
    /// Create a new MCP server with the given configuration
    pub fn new(config: Config) -> Result<Self> {
        // Validate configuration
        config.validate()?;

        // Create feature managers
        let resource_manager = Arc::new(ResourceManager::new());
        let tool_manager = Arc::new(ToolManager::new());
        let prompt_manager = Arc::new(PromptManager::new());
        let sampling_manager = Arc::new(SamplingManager::new());

        // Create protocol handler
        let protocol_handler = Arc::new(ProtocolHandler::new(
            resource_manager,
            tool_manager,
            prompt_manager,
            sampling_manager,
        ));

        // Create transport manager
        let mut transport_manager = TransportManager::new();

        // Create and add transport based on configuration
        let transport = TransportFactory::create(&config.transport)?;
        transport_manager.add_transport(transport);

        Ok(Self {
            config,
            transport_manager,
            protocol_handler,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }

    /// Create a new MCP server with custom transport
    pub fn with_transport(config: Config, transport: Arc<dyn Transport>) -> Result<Self> {
        // Validate configuration
        config.validate()?;

        // Create feature managers
        let resource_manager = Arc::new(ResourceManager::new());
        let tool_manager = Arc::new(ToolManager::new());
        let prompt_manager = Arc::new(PromptManager::new());
        let sampling_manager = Arc::new(SamplingManager::new());

        // Create protocol handler
        let protocol_handler = Arc::new(ProtocolHandler::new(
            resource_manager,
            tool_manager,
            prompt_manager,
            sampling_manager,
        ));

        // Create transport manager and add the custom transport
        let mut transport_manager = TransportManager::new();
        transport_manager.add_transport(transport);

        Ok(Self {
            config,
            transport_manager,
            protocol_handler,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }

    /// Start the MCP server
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting MCP server");

        // Check if already running
        {
            let running = self.running.read().await;
            if *running {
                warn!("Server is already running");
                return Ok(());
            }
        }

        // Mark as running
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        // Start transport manager
        let mut message_receiver = self.transport_manager.start().await?;

        info!("MCP server started successfully");

        // Main message processing loop
        while let Some(transport_message) = message_receiver.recv().await {
            // Check if we should stop
            {
                let running = self.running.read().await;
                if !*running {
                    break;
                }
            }

            // Handle the message
            match self
                .protocol_handler
                .handle_message(transport_message.message)
                .await
            {
                Ok(Some(response)) => {
                    // Send response back through transport
                    info!("Generated response: {:?}", response);
                }
                Ok(None) => {
                    // No response needed (e.g., for notifications)
                }
                Err(e) => {
                    error!("Error handling message: {}", e);
                }
            }
        }

        info!("MCP server message loop ended");
        Ok(())
    }

    /// Run the server (blocking)
    pub async fn run(&mut self) -> Result<()> {
        // Set up signal handling for graceful shutdown
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to create SIGTERM handler");

            let mut sigint =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                    .expect("Failed to create SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, shutting down gracefully");
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT, shutting down gracefully");
                }
            }

            let mut running_guard = running.write().await;
            *running_guard = false;
        });

        // Start the server
        self.start().await
    }

    /// Stop the MCP server
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping MCP server");

        // Mark as not running
        {
            let mut running = self.running.write().await;
            *running = false;
        }

        // Stop transport manager
        self.transport_manager.stop().await?;

        info!("MCP server stopped");
        Ok(())
    }

    /// Check if the server is running
    pub async fn is_running(&self) -> bool {
        let running = self.running.read().await;
        *running
    }

    /// Get server configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get transport information
    pub fn transport_info(&self) -> Vec<crate::transport::TransportInfo> {
        self.transport_manager.get_transport_info()
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> ServerStats {
        ServerStats {
            running: self.is_running().await,
            transport_count: self.transport_info().len(),
            // Add more statistics as needed
        }
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub running: bool,
    pub transport_count: usize,
}

/// Server builder for easier configuration
pub struct McpServerBuilder {
    config: Config,
    custom_transport: Option<Arc<dyn Transport>>,
}

impl McpServerBuilder {
    /// Create a new server builder with default configuration
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            custom_transport: None,
        }
    }

    /// Set the server configuration
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Set a custom transport
    pub fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.custom_transport = Some(transport);
        self
    }

    /// Set the server name
    pub fn name(mut self, name: String) -> Self {
        self.config.server.name = name;
        self
    }

    /// Set the server version
    pub fn version(mut self, version: String) -> Self {
        self.config.server.version = version;
        self
    }

    /// Set server instructions
    pub fn instructions(mut self, instructions: String) -> Self {
        self.config.server.instructions = Some(instructions);
        self
    }

    /// Build the server
    pub fn build(self) -> Result<McpServer> {
        if let Some(transport) = self.custom_transport {
            McpServer::with_transport(self.config, transport)
        } else {
            McpServer::new(self.config)
        }
    }
}

impl Default for McpServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_builder() {
        let server = McpServerBuilder::new()
            .name("test-server".to_string())
            .version("1.0.0".to_string())
            .instructions("Test server instructions".to_string())
            .build();

        assert!(server.is_ok());
        let server = server.unwrap();
        assert_eq!(server.config().server.name, "test-server");
        assert_eq!(server.config().server.version, "1.0.0");
        assert_eq!(
            server.config().server.instructions,
            Some("Test server instructions".to_string())
        );
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let config = Config::default();
        let server = McpServer::new(config);
        assert!(server.is_ok());

        let server = server.unwrap();
        assert!(!server.is_running().await);
    }
}
