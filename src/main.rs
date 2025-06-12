//! MCP Server CLI application.
//!
//! This is the main entry point for the MCP server binary.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{error, info};

use mcp_server::server::McpServerBuilder;
use mcp_server::{Config, McpServer};

/// MCP Server CLI
#[derive(Parser)]
#[command(name = "mcp-server")]
#[command(about = "A Model Context Protocol (MCP) server implementation in Rust")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server
    Start {
        /// Server name
        #[arg(long)]
        name: Option<String>,

        /// Server version
        #[arg(long)]
        version: Option<String>,

        /// Server instructions
        #[arg(long)]
        instructions: Option<String>,

        /// HTTP bind address
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,

        /// HTTP port
        #[arg(long, default_value = "8080")]
        port: u16,
    },

    /// Generate a default configuration file
    Config {
        /// Output file path
        #[arg(short, long, default_value = "mcp-server.toml")]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },

    /// Validate a configuration file
    Validate {
        /// Configuration file to validate
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Show server information
    Info,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level, cli.verbose)?;

    match cli.command {
        Some(Commands::Start {
            name,
            version,
            instructions,
            bind,
            port,
        }) => {
            start_server(cli.config, name, version, instructions, bind, port).await?;
        }
        Some(Commands::Config { output, force }) => {
            generate_config(output, force)?;
        }
        Some(Commands::Validate { file }) => {
            validate_config(file)?;
        }
        Some(Commands::Info) => {
            show_info();
        }
        None => {
            // Default to starting the server
            start_server(
                cli.config,
                None,
                None,
                None,
                "127.0.0.1".to_string(),
                8080,
            )
            .await?;
        }
    }

    Ok(())
}

/// Initialize logging
fn init_logging(level: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let log_level = if verbose { "debug" } else { level };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    Ok(())
}

/// Start the MCP server
async fn start_server(
    config_path: Option<PathBuf>,
    name: Option<String>,
    version: Option<String>,
    instructions: Option<String>,
    bind: String,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting MCP server...");

    // Load configuration
    let mut config = if let Some(config_path) = config_path {
        info!("Loading configuration from: {}", config_path.display());
        Config::from_file(config_path)?
    } else {
        Config::default()
    };

    // Override configuration with CLI arguments
    if let Some(name) = name {
        config.server.name = name;
    }

    if let Some(version) = version {
        config.server.version = version;
    }

    if let Some(instructions) = instructions {
        config.server.instructions = Some(instructions);
    }

    // Configure transport
    config.transport.transport_type = mcp_server::config::TransportType::Http;
    if let Some(ref mut http_config) = config.transport.http {
        http_config.bind_address = bind;
        http_config.port = port;
    }

    // Create and start server
    let mut server = McpServer::new(config)?;

    info!("Server configuration:");
    info!("  Name: {}", server.config().server.name);
    info!("  Version: {}", server.config().server.version);
    if let Some(ref instructions) = server.config().server.instructions {
        info!("  Instructions: {}", instructions);
    }

    for transport_info in server.transport_info() {
        info!(
            "  Transport: {:?} at {}",
            transport_info.transport_type, transport_info.address
        );
    }

    // Run the server
    server.run().await?;

    info!("MCP server stopped");
    Ok(())
}

/// Generate a default configuration file
fn generate_config(output: PathBuf, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() && !force {
        error!("Configuration file already exists: {}", output.display());
        error!("Use --force to overwrite");
        std::process::exit(1);
    }

    let config = Config::default();
    config.to_file(&output)?;

    info!("Generated configuration file: {}", output.display());
    Ok(())
}

/// Validate a configuration file
fn validate_config(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    info!("Validating configuration file: {}", file.display());

    let config = Config::from_file(&file)?;
    config.validate()?;

    info!("Configuration file is valid");
    Ok(())
}

/// Show server information
fn show_info() {
    info!("MCP Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Protocol Version: {}", mcp_server::PROTOCOL_VERSION);
    info!("Description: {}", env!("CARGO_PKG_DESCRIPTION"));
    info!("--------------------------------");
    info!("Features:");
    info!("  - HTTP transport with Server-Sent Events (SSE)");
    info!("  - Resources: File system and HTTP resource providers");
    info!("  - Tools: Extensible tool execution framework");
    info!("  - Prompts: Template-based prompt generation");
    info!("  - Sampling: LLM sampling integration");
    info!("  - Logging: Structured logging with multiple levels");
    info!("  - Completion: Argument completion for prompts and resources");
    info!("  - Authentication: API key and JWT support");
    info!("  - Configuration: TOML-based configuration management");
    info!("--------------------------------");
    info!("Repository: {}", env!("CARGO_PKG_REPOSITORY"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cli_parsing() {
        // Test basic parsing
        let cli = Cli::try_parse_from(&["mcp-server", "--verbose"]).unwrap();
        assert!(cli.verbose);

        // Test start command
        let cli = Cli::try_parse_from(&[
            "mcp-server",
            "start",
            "--name",
            "test-server",
            "--port",
            "9090",
        ])
        .unwrap();

        if let Some(Commands::Start { name, port, .. }) = cli.command {
            assert_eq!(name, Some("test-server".to_string()));
            assert_eq!(port, 9090);
        } else {
            panic!("Expected Start command");
        }
    }

    #[test]
    fn test_config_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.toml");

        // Generate config
        assert!(generate_config(config_path.clone(), false).is_ok());
        assert!(config_path.exists());

        // Validate generated config
        assert!(validate_config(config_path).is_ok());
    }
}
