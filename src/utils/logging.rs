//! Logging utilities for the MCP server.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging with the specified configuration
pub fn init_logging(config: &crate::config::LoggingConfig) -> crate::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.level));

    let subscriber = tracing_subscriber::registry().with(filter);

    match config.format {
        crate::config::LogFormat::Json => {
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(false)
                        .compact(),
                )
                .init();
        }
        crate::config::LogFormat::Pretty => {
            subscriber
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        crate::config::LogFormat::Compact => {
            subscriber
                .with(tracing_subscriber::fmt::layer().compact())
                .init();
        }
    }

    Ok(())
}
