//! Additional validation utilities.

use crate::error::{McpError, Result};

/// Validate that a string is not empty
pub fn validate_non_empty(value: &str, field_name: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(McpError::invalid_params(format!(
            "{} cannot be empty",
            field_name
        )));
    }
    Ok(())
}

/// Validate that a number is within a range
pub fn validate_range<T: PartialOrd + std::fmt::Display>(
    value: T,
    min: T,
    max: T,
    field_name: &str,
) -> Result<()> {
    if value < min || value > max {
        return Err(McpError::invalid_params(format!(
            "{} must be between {} and {}, got {}",
            field_name, min, max, value
        )));
    }
    Ok(())
}

/// Validate email format
pub fn validate_email(email: &str) -> Result<()> {
    if !email.contains('@') || !email.contains('.') {
        return Err(McpError::invalid_params("Invalid email format"));
    }
    Ok(())
}
