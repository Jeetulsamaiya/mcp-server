//! MCP Client features implementation.
//!
//! This module contains client-side features that the MCP server can use,
//! such as sampling and root directory management.

pub mod features;

// Re-export main types
pub use features::*;
