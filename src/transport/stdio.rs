//! STDIO transport implementation for MCP server.
//!
//! This module implements the STDIO transport as defined in the MCP specification,
//! allowing communication through standard input and output streams.

use async_trait::async_trait;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::StdioConfig;
use crate::error::{McpError, Result};
use crate::protocol::{parse_message, serialize_message};
use crate::transport::{
    Transport, TransportInfo, TransportMessage, TransportMetadata, TransportType,
};

/// STDIO transport implementation
pub struct StdioTransport {
    config: StdioConfig,
    shutdown_sender: Arc<RwLock<Option<mpsc::Sender<()>>>>,
}

impl StdioTransport {
    /// Create a new STDIO transport
    pub fn new(config: StdioConfig) -> Result<Self> {
        Ok(Self {
            config,
            shutdown_sender: Arc::new(RwLock::new(None)),
        })
    }

    /// Handle incoming messages from stdin
    async fn handle_stdin_messages(
        message_sender: mpsc::Sender<TransportMessage>,
        mut shutdown_receiver: mpsc::Receiver<()>,
        buffer_size: usize,
        enable_stderr_logging: bool,
    ) {
        let stdin = tokio::io::stdin();
        let reader = BufReader::with_capacity(buffer_size, stdin);
        let mut lines = reader.lines();

        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = shutdown_receiver.recv() => {
                    info!("STDIO transport received shutdown signal");
                    break;
                }

                // Read line from stdin
                line_result = lines.next_line() => {
                    match line_result {
                        Ok(Some(line)) => {
                            if line.trim().is_empty() {
                                continue;
                            }

                            info!("Received message from stdin: {}", line);

                            // Parse the message
                            match parse_message(&line) {
                                Ok(message) => {
                                    let transport_message = TransportMessage {
                                        message,
                                        session_id: None,
                                        client_id: Some("stdio".to_string()),
                                        metadata: TransportMetadata::default(),
                                    };

                                    if let Err(e) = message_sender.send(transport_message).await {
                                        error!("Failed to send message to protocol handler: {}", e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse message from stdin: {}", e);

                                    if enable_stderr_logging {
                                        if let Err(write_err) = Self::write_stderr(&format!(
                                            "Parse error: {}\n", e
                                        )).await {
                                            error!("Failed to write to stderr: {}", write_err);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            // EOF reached
                            info!("EOF reached on stdin");
                            break;
                        }
                        Err(e) => {
                            error!("Error reading from stdin: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        info!("STDIO input handler stopped");
    }

    /// Handle outgoing messages to stdout
    async fn handle_stdout_messages(
        mut response_receiver: mpsc::Receiver<TransportMessage>,
        mut shutdown_receiver: mpsc::Receiver<()>,
        enable_stderr_logging: bool,
    ) {
        let mut stdout = tokio::io::stdout();

        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = shutdown_receiver.recv() => {
                    info!("STDIO output handler received shutdown signal");
                    break;
                }

                // Handle outgoing message
                message = response_receiver.recv() => {
                    match message {
                        Some(transport_message) => {
                            match serialize_message(&transport_message.message) {
                                Ok(serialized) => {
                                    let output = format!("{}\n", serialized);

                                    if let Err(e) = stdout.write_all(output.as_bytes()).await {
                                        error!("Failed to write to stdout: {}", e);
                                        break;
                                    }

                                    if let Err(e) = stdout.flush().await {
                                        error!("Failed to flush stdout: {}", e);
                                        break;
                                    }

                                    info!("Sent message to stdout: {}", serialized);
                                }
                                Err(e) => {
                                    error!("Failed to serialize message: {}", e);

                                    if enable_stderr_logging {
                                        if let Err(write_err) = Self::write_stderr(&format!(
                                            "Serialization error: {}\n", e
                                        )).await {
                                            error!("Failed to write to stderr: {}", write_err);
                                        }
                                    }
                                }
                            }
                        }
                        None => {
                            info!("Response channel closed");
                            break;
                        }
                    }
                }
            }
        }

        info!("STDIO output handler stopped");
    }

    /// Write a message to stderr
    async fn write_stderr(message: &str) -> Result<()> {
        let mut stderr = tokio::io::stderr();
        stderr
            .write_all(message.as_bytes())
            .await
            .map_err(|e| McpError::Io(e))?;
        stderr.flush().await.map_err(|e| McpError::Io(e))?;
        Ok(())
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn start(
        &self,
    ) -> Result<(
        mpsc::Receiver<TransportMessage>,
        mpsc::Sender<TransportMessage>,
    )> {
        info!("Starting STDIO transport");

        let (message_tx, message_rx) = mpsc::channel(1000);
        let (response_tx, response_rx) = mpsc::channel(1000);
        let (shutdown_tx, shutdown_rx1) = mpsc::channel(1);
        let (shutdown_tx2, shutdown_rx2) = mpsc::channel(1);

        // Store shutdown sender
        {
            let mut sender = self.shutdown_sender.write().await;
            *sender = Some(shutdown_tx.clone());
        }

        // Start stdin handler
        let message_sender = message_tx.clone();
        let buffer_size = self.config.buffer_size;
        let enable_stderr_logging = self.config.enable_stderr_logging;

        tokio::spawn(async move {
            Self::handle_stdin_messages(
                message_sender,
                shutdown_rx1,
                buffer_size,
                enable_stderr_logging,
            )
            .await;
        });

        // Start stdout handler
        let enable_stderr_logging = self.config.enable_stderr_logging;
        tokio::spawn(async move {
            Self::handle_stdout_messages(response_rx, shutdown_rx2, enable_stderr_logging).await;
        });

        Ok((message_rx, response_tx))
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping STDIO transport");

        let sender = {
            let mut shutdown_sender = self.shutdown_sender.write().await;
            shutdown_sender.take()
        };

        if let Some(sender) = sender {
            // Send shutdown signal
            if let Err(e) = sender.send(()).await {
                warn!("Failed to send shutdown signal: {}", e);
            }
        }

        Ok(())
    }

    fn info(&self) -> TransportInfo {
        TransportInfo {
            transport_type: TransportType::Stdio,
            address: "stdio".to_string(),
            secure: false,
            max_message_size: None, // No inherent limit for STDIO
        }
    }
}

/// STDIO transport builder for easier configuration
pub struct StdioTransportBuilder {
    config: StdioConfig,
}

impl StdioTransportBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: StdioConfig::default(),
        }
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Enable or disable stderr logging
    pub fn enable_stderr_logging(mut self, enable: bool) -> Self {
        self.config.enable_stderr_logging = enable;
        self
    }

    /// Build the transport
    pub fn build(self) -> Result<StdioTransport> {
        StdioTransport::new(self.config)
    }
}

impl Default for StdioTransportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_stdio_transport_creation() {
        let config = StdioConfig::default();
        let transport = StdioTransport::new(config);
        assert!(transport.is_ok());
    }

    #[tokio::test]
    async fn test_stdio_transport_info() {
        let config = StdioConfig::default();
        let transport = StdioTransport::new(config).unwrap();
        let info = transport.info();

        assert_eq!(info.transport_type, TransportType::Stdio);
        assert_eq!(info.address, "stdio");
        assert!(!info.secure);
        assert!(info.max_message_size.is_none());
    }

    #[tokio::test]
    async fn test_stdio_transport_builder() {
        let transport = StdioTransportBuilder::new()
            .buffer_size(4096)
            .enable_stderr_logging(false)
            .build();

        assert!(transport.is_ok());
        let transport = transport.unwrap();
        assert_eq!(transport.config.buffer_size, 4096);
        assert!(!transport.config.enable_stderr_logging);
    }

    #[test]
    fn test_write_stderr() {
        // Test would require mocking stderr
    }
}
