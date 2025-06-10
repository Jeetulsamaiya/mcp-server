//! Client feature implementations.
//!
//! This module contains the implementations of MCP client features including
//! sampling and root directory management.

pub mod sampling;
pub mod roots;

// Re-export main types
pub use sampling::SamplingManager;
pub use roots::RootsManager;
