//! Logging management for MCP server.
//!
//! This module implements the logging feature of MCP, allowing the server
//! to send log messages to clients.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

use crate::error::{McpError, Result};
use crate::protocol::LoggingLevel;
use crate::server::features::FeatureManager;

/// Logging manager for handling MCP logging
pub struct LoggingManager {
    /// Current logging level
    level: Arc<RwLock<LoggingLevel>>,

    /// Log message sender
    sender: Arc<RwLock<Option<mpsc::Sender<LogMessage>>>>,

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Log message structure
#[derive(Debug, Clone)]
pub struct LogMessage {
    /// Log level
    pub level: LoggingLevel,

    /// Optional logger name
    pub logger: Option<String>,

    /// Log data
    pub data: serde_json::Value,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl LoggingManager {
    /// Create a new logging manager
    pub fn new() -> Self {
        Self {
            level: Arc::new(RwLock::new(LoggingLevel::Info)),
            sender: Arc::new(RwLock::new(None)),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Set the logging level
    pub async fn set_level(&self, level: LoggingLevel) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Logging feature is disabled".to_string(),
            ));
        }

        {
            let mut current_level = self.level.write().await;
            *current_level = level.clone();
        }

        info!("Set logging level to: {:?}", level);
        Ok(())
    }

    /// Get the current logging level
    pub async fn get_level(&self) -> LoggingLevel {
        let level = self.level.read().await;
        level.clone()
    }

    /// Set the log message sender
    pub async fn set_sender(&self, sender: mpsc::Sender<LogMessage>) {
        let mut current_sender = self.sender.write().await;
        *current_sender = Some(sender);
        info!("Log message sender configured");
    }

    /// Send a log message
    pub async fn log(
        &self,
        level: LoggingLevel,
        logger: Option<String>,
        data: serde_json::Value,
    ) -> Result<()> {
        if !self.is_enabled() {
            return Ok(()); // Silently ignore if disabled
        }

        // Check if message level is high enough
        if !self.should_log(&level).await {
            return Ok(());
        }

        let message = LogMessage {
            level,
            logger,
            data,
            timestamp: chrono::Utc::now(),
        };

        // Send message if sender is configured
        let sender = self.sender.read().await;
        if let Some(sender) = sender.as_ref() {
            if let Err(e) = sender.send(message).await {
                // Don't return error for logging failures to avoid infinite loops
                eprintln!("Failed to send log message: {}", e);
            }
        }

        Ok(())
    }

    /// Log a debug message
    pub async fn debug(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self.log(LoggingLevel::Debug, logger, message.into()).await;
    }

    /// Log an info message
    pub async fn info(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self.log(LoggingLevel::Info, logger, message.into()).await;
    }

    /// Log a notice message
    pub async fn notice(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self.log(LoggingLevel::Notice, logger, message.into()).await;
    }

    /// Log a warning message
    pub async fn warning(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self
            .log(LoggingLevel::Warning, logger, message.into())
            .await;
    }

    /// Log an error message
    pub async fn error(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self.log(LoggingLevel::Error, logger, message.into()).await;
    }

    /// Log a critical message
    pub async fn critical(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self
            .log(LoggingLevel::Critical, logger, message.into())
            .await;
    }

    /// Log an alert message
    pub async fn alert(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self.log(LoggingLevel::Alert, logger, message.into()).await;
    }

    /// Log an emergency message
    pub async fn emergency(&self, logger: Option<String>, message: impl Into<serde_json::Value>) {
        let _ = self
            .log(LoggingLevel::Emergency, logger, message.into())
            .await;
    }

    /// Check if a message should be logged based on current level
    async fn should_log(&self, message_level: &LoggingLevel) -> bool {
        let current_level = self.level.read().await;
        self.level_priority(message_level) >= self.level_priority(&current_level)
    }

    /// Get priority value for logging level (higher = more important)
    fn level_priority(&self, level: &LoggingLevel) -> u8 {
        match level {
            LoggingLevel::Emergency => 8,
            LoggingLevel::Alert => 7,
            LoggingLevel::Critical => 6,
            LoggingLevel::Error => 5,
            LoggingLevel::Warning => 4,
            LoggingLevel::Notice => 3,
            LoggingLevel::Info => 2,
            LoggingLevel::Debug => 1,
        }
    }
}

impl FeatureManager for LoggingManager {
    fn name(&self) -> &'static str {
        "logging"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}
}

impl LogMessage {
    /// Create a new log message
    pub fn new(level: LoggingLevel, data: serde_json::Value) -> Self {
        Self {
            level,
            logger: None,
            data,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a new log message with logger
    pub fn with_logger(level: LoggingLevel, logger: String, data: serde_json::Value) -> Self {
        Self {
            level,
            logger: Some(logger),
            data,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a simple text log message
    pub fn text(level: LoggingLevel, message: String) -> Self {
        Self::new(level, serde_json::Value::String(message))
    }

    /// Create a structured log message
    pub fn structured(level: LoggingLevel, data: serde_json::Value) -> Self {
        Self::new(level, data)
    }
}

/// Log message builder for easier construction
pub struct LogMessageBuilder {
    level: LoggingLevel,
    logger: Option<String>,
    data: serde_json::Value,
}

impl LogMessageBuilder {
    /// Create a new log message builder
    pub fn new(level: LoggingLevel) -> Self {
        Self {
            level,
            logger: None,
            data: serde_json::Value::Null,
        }
    }

    /// Set the logger name
    pub fn logger(mut self, logger: String) -> Self {
        self.logger = Some(logger);
        self
    }

    /// Set the log data
    pub fn data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Set a text message
    pub fn message(mut self, message: String) -> Self {
        self.data = serde_json::Value::String(message);
        self
    }

    /// Build the log message
    pub fn build(self) -> LogMessage {
        LogMessage {
            level: self.level,
            logger: self.logger,
            data: self.data,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Convenience functions for creating log messages
impl LogMessage {
    pub fn debug(message: String) -> Self {
        Self::text(LoggingLevel::Debug, message)
    }

    pub fn info(message: String) -> Self {
        Self::text(LoggingLevel::Info, message)
    }

    pub fn notice(message: String) -> Self {
        Self::text(LoggingLevel::Notice, message)
    }

    pub fn warning(message: String) -> Self {
        Self::text(LoggingLevel::Warning, message)
    }

    pub fn error(message: String) -> Self {
        Self::text(LoggingLevel::Error, message)
    }

    pub fn critical(message: String) -> Self {
        Self::text(LoggingLevel::Critical, message)
    }

    pub fn alert(message: String) -> Self {
        Self::text(LoggingLevel::Alert, message)
    }

    pub fn emergency(message: String) -> Self {
        Self::text(LoggingLevel::Emergency, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_logging_manager() {
        let manager = LoggingManager::new();

        // Test initial level
        let level = manager.get_level().await;
        assert!(matches!(level, LoggingLevel::Info));

        // Test setting level
        assert!(manager.set_level(LoggingLevel::Debug).await.is_ok());
        let level = manager.get_level().await;
        assert!(matches!(level, LoggingLevel::Debug));
    }

    #[tokio::test]
    async fn test_log_message_priority() {
        let manager = LoggingManager::new();

        // Set level to warning
        manager.set_level(LoggingLevel::Warning).await.unwrap();

        // Test priority checking
        assert!(manager.should_log(&LoggingLevel::Error).await);
        assert!(manager.should_log(&LoggingLevel::Warning).await);
        assert!(!manager.should_log(&LoggingLevel::Info).await);
        assert!(!manager.should_log(&LoggingLevel::Debug).await);
    }

    #[tokio::test]
    async fn test_log_message_sending() {
        let manager = LoggingManager::new();
        let (sender, mut receiver) = mpsc::channel(10);

        manager.set_sender(sender).await;

        // Send a log message
        manager
            .info(Some("test".to_string()), serde_json::json!("test message"))
            .await;

        // Check if message was received
        let message = receiver.recv().await;
        assert!(message.is_some());

        let message = message.unwrap();
        assert!(matches!(message.level, LoggingLevel::Info));
        assert_eq!(message.logger, Some("test".to_string()));
    }

    #[test]
    fn test_log_message_builder() {
        let message = LogMessageBuilder::new(LoggingLevel::Info)
            .logger("test-logger".to_string())
            .message("Test message".to_string())
            .build();

        assert!(matches!(message.level, LoggingLevel::Info));
        assert_eq!(message.logger, Some("test-logger".to_string()));
        assert_eq!(
            message.data,
            serde_json::Value::String("Test message".to_string())
        );
    }

    #[test]
    fn test_convenience_functions() {
        let message = LogMessage::error("Error occurred".to_string());
        assert!(matches!(message.level, LoggingLevel::Error));
        assert_eq!(
            message.data,
            serde_json::Value::String("Error occurred".to_string())
        );

        let message = LogMessage::debug("Debug info".to_string());
        assert!(matches!(message.level, LoggingLevel::Debug));
    }
}
