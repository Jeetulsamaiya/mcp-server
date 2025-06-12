//! Transport layer for MCP server.
//!
//! This module provides the HTTP transport implementation for the MCP server,
//! supporting streamable HTTP with Server-Sent Events (SSE) as defined in the specification.

pub mod http;
pub mod session;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::protocol::AnyJsonRpcMessage;

/// Transport trait for different communication methods
#[async_trait]
pub trait Transport: Send + Sync {
    /// Start the transport and return message channels
    async fn start(&self) -> Result<(
        mpsc::Receiver<TransportMessage>,
        mpsc::Sender<TransportMessage>
    )>;

    /// Stop the transport
    async fn stop(&self) -> Result<()>;

    /// Get transport information
    fn info(&self) -> TransportInfo;
}

/// Transport message containing the actual JSON-RPC message and metadata
#[derive(Debug, Clone)]
pub struct TransportMessage {
    /// The JSON-RPC message
    pub message: AnyJsonRpcMessage,
    
    /// Session ID (for HTTP transport)
    pub session_id: Option<String>,
    
    /// Client identifier
    pub client_id: Option<String>,
    
    /// Message metadata
    pub metadata: TransportMetadata,
}

/// Transport metadata
#[derive(Debug, Clone)]
pub struct TransportMetadata {
    /// Timestamp when message was received
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Source address (for HTTP transport)
    pub source_addr: Option<std::net::SocketAddr>,
    
    /// User agent (for HTTP transport)
    pub user_agent: Option<String>,
    
    /// Additional headers (for HTTP transport)
    pub headers: std::collections::HashMap<String, String>,
}

/// Transport information
#[derive(Debug, Clone)]
pub struct TransportInfo {
    /// Transport type
    pub transport_type: TransportType,
    
    /// Transport address/endpoint
    pub address: String,
    
    /// Whether the transport is secure
    pub secure: bool,
    
    /// Maximum message size
    pub max_message_size: Option<usize>,
}

/// Transport type enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    Http,
}

impl Default for TransportMetadata {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            source_addr: None,
            user_agent: None,
            headers: std::collections::HashMap::new(),
        }
    }
}

impl TransportMessage {
    /// Create a new transport message
    pub fn new(message: AnyJsonRpcMessage) -> Self {
        Self {
            message,
            session_id: None,
            client_id: None,
            metadata: TransportMetadata::default(),
        }
    }

    /// Create a new transport message with session ID
    pub fn with_session(message: AnyJsonRpcMessage, session_id: String) -> Self {
        Self {
            message,
            session_id: Some(session_id),
            client_id: None,
            metadata: TransportMetadata::default(),
        }
    }

    /// Create a new transport message with metadata
    pub fn with_metadata(message: AnyJsonRpcMessage, metadata: TransportMetadata) -> Self {
        Self {
            message,
            session_id: None,
            client_id: None,
            metadata,
        }
    }
}

/// Transport factory for creating transport instances
pub struct TransportFactory;

impl TransportFactory {
    /// Create a transport based on configuration
    pub fn create(config: &crate::config::TransportConfig) -> Result<Arc<dyn Transport>> {
        match config.transport_type {
            crate::config::TransportType::Http => {
                let http_config = config.http.as_ref()
                    .ok_or_else(|| crate::error::McpError::Config(
                        "HTTP transport selected but no HTTP config provided".to_string()
                    ))?;

                let transport = http::HttpTransport::new(http_config.clone())?;
                Ok(Arc::new(transport))
            }
        }
    }
}

/// Transport manager for handling multiple transports
pub struct TransportManager {
    transports: Vec<Arc<dyn Transport>>,
    message_sender: mpsc::Sender<TransportMessage>,
    message_receiver: Option<mpsc::Receiver<TransportMessage>>,
}

impl TransportManager {
    /// Create a new transport manager
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(1000);
        
        Self {
            transports: Vec::new(),
            message_sender: sender,
            message_receiver: Some(receiver),
        }
    }

    /// Add a transport to the manager
    pub fn add_transport(&mut self, transport: Arc<dyn Transport>) {
        self.transports.push(transport);
    }

    /// Start all transports
    pub async fn start(&mut self) -> Result<mpsc::Receiver<TransportMessage>> {
        for transport in &self.transports {
            let (mut receiver, _sender) = transport.start().await?;
            let message_sender = self.message_sender.clone();
            
            // Spawn a task to forward messages from this transport
            tokio::spawn(async move {
                while let Some(message) = receiver.recv().await {
                    if let Err(e) = message_sender.send(message).await {
                        tracing::error!("Failed to forward transport message: {}", e);
                        break;
                    }
                }
            });
        }

        self.message_receiver.take()
            .ok_or_else(|| crate::error::McpError::Transport(
                crate::error::TransportError::ConnectionFailed(
                    "Message receiver already taken".to_string()
                )
            ))
    }

    /// Stop all transports
    pub async fn stop(&self) -> Result<()> {
        for transport in &self.transports {
            if let Err(e) = transport.stop().await {
                tracing::error!("Failed to stop transport: {}", e);
            }
        }
        Ok(())
    }

    /// Get information about all transports
    pub fn get_transport_info(&self) -> Vec<TransportInfo> {
        self.transports.iter().map(|t| t.info()).collect()
    }
}

impl Default for TransportManager {
    fn default() -> Self {
        Self::new()
    }
}
