//! Configuration management for the MCP server.
//!
//! This module handles server configuration including transport settings,
//! authentication, logging, and feature enablement.

use crate::error::{McpError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration structure for the MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server information
    pub server: ServerConfig,

    /// Transport configuration
    pub transport: TransportConfig,

    /// Authentication configuration
    pub auth: AuthConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Feature configuration
    pub features: FeatureConfig,

    /// Tools configuration
    #[serde(default)]
    pub tools: crate::server::features::tools::ToolsConfig,

    /// Custom server-specific settings
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Server-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server name
    pub name: String,

    /// Server version
    pub version: String,

    /// Server description/instructions
    pub instructions: Option<String>,

    /// Maximum number of concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,
}

/// Transport layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport type (http only)
    #[serde(default = "default_transport_type")]
    pub transport_type: TransportType,

    /// HTTP-specific configuration
    pub http: Option<HttpConfig>,
}

/// Transport type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Http,
}

/// HTTP transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Bind address
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Port number
    #[serde(default = "default_port")]
    pub port: u16,

    /// MCP endpoint path
    #[serde(default = "default_endpoint_path")]
    pub endpoint_path: String,

    /// Enable CORS
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,

    /// CORS allowed origins
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Session timeout in seconds
    #[serde(default = "default_session_timeout")]
    pub session_timeout: u64,

    /// Enable SSL/TLS
    #[serde(default)]
    pub enable_tls: bool,

    /// SSL certificate file path
    pub cert_file: Option<PathBuf>,

    /// SSL private key file path
    pub key_file: Option<PathBuf>,
}



/// Authentication and authorization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    #[serde(default)]
    pub enabled: bool,

    /// Authentication method
    #[serde(default)]
    pub method: AuthMethod,

    /// API keys for authentication
    #[serde(default)]
    pub api_keys: Vec<String>,

    /// JWT secret for token validation
    pub jwt_secret: Option<String>,

    /// Token expiration time in seconds
    #[serde(default = "default_token_expiration")]
    pub token_expiration: u64,
}

/// Authentication method enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    None,
    ApiKey,
    Bearer,
    Jwt,
}

impl Default for AuthMethod {
    fn default() -> Self {
        AuthMethod::None
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format
    #[serde(default = "default_log_format")]
    pub format: LogFormat,

    /// Log file path (if None, logs to stdout)
    pub file: Option<PathBuf>,

    /// Enable request/response logging
    #[serde(default = "default_enable_request_logging")]
    pub enable_request_logging: bool,
}

/// Log format enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

/// Feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Enable resources feature
    #[serde(default = "default_true")]
    pub resources: bool,

    /// Enable tools feature
    #[serde(default = "default_true")]
    pub tools: bool,

    /// Enable prompts feature
    #[serde(default = "default_true")]
    pub prompts: bool,

    /// Enable sampling feature
    #[serde(default = "default_true")]
    pub sampling: bool,

    /// Enable logging feature
    #[serde(default = "default_true")]
    pub logging: bool,

    /// Enable completion feature
    #[serde(default = "default_true")]
    pub completion: bool,

    /// Enable roots feature
    #[serde(default = "default_true")]
    pub roots: bool,
}

// Default value functions
fn default_max_connections() -> usize {
    100
}
fn default_request_timeout() -> u64 {
    30
}
fn default_transport_type() -> TransportType {
    TransportType::Http
}
fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_endpoint_path() -> String {
    "/mcp".to_string()
}
fn default_enable_cors() -> bool {
    true
}
fn default_session_timeout() -> u64 {
    3600
}

fn default_token_expiration() -> u64 {
    3600
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_format() -> LogFormat {
    LogFormat::Pretty
}
fn default_enable_request_logging() -> bool {
    false
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                name: crate::SERVER_NAME.to_string(),
                version: crate::SERVER_VERSION.to_string(),
                instructions: None,
                max_connections: default_max_connections(),
                request_timeout: default_request_timeout(),
            },
            transport: TransportConfig {
                transport_type: default_transport_type(),
                http: Some(HttpConfig::default()),
            },
            auth: AuthConfig::default(),
            logging: LoggingConfig::default(),
            features: FeatureConfig::default(),
            tools: crate::server::features::tools::ToolsConfig::default(),
            custom: HashMap::new(),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: default_port(),
            endpoint_path: default_endpoint_path(),
            enable_cors: default_enable_cors(),
            cors_origins: vec!["*".to_string()],
            session_timeout: default_session_timeout(),
            enable_tls: false,
            cert_file: None,
            key_file: None,
        }
    }
}



impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            method: AuthMethod::None,
            api_keys: Vec::new(),
            jwt_secret: None,
            token_expiration: default_token_expiration(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
            enable_request_logging: default_enable_request_logging(),
        }
    }
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            resources: default_true(),
            tools: default_true(),
            prompts: default_true(),
            sampling: default_true(),
            logging: default_true(),
            completion: default_true(),
            roots: default_true(),
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| McpError::Config(format!("Failed to read config file: {}", e)))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| McpError::Config(format!("Failed to parse config file: {}", e)))?;

        Ok(config)
    }

    /// Save configuration to a file
    pub fn to_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| McpError::Config(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| McpError::Config(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate transport configuration
        match self.transport.transport_type {
            TransportType::Http => {
                if self.transport.http.is_none() {
                    return Err(McpError::Config(
                        "HTTP transport selected but no HTTP config provided".to_string(),
                    ));
                }
            }
        }

        // Validate authentication configuration
        if self.auth.enabled {
            match self.auth.method {
                AuthMethod::ApiKey => {
                    if self.auth.api_keys.is_empty() {
                        return Err(McpError::Config(
                            "API key authentication enabled but no API keys provided".to_string(),
                        ));
                    }
                }
                AuthMethod::Jwt => {
                    if self.auth.jwt_secret.is_none() {
                        return Err(McpError::Config(
                            "JWT authentication enabled but no JWT secret provided".to_string(),
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
