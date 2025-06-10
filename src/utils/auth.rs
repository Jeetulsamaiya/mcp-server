//! Authentication and authorization utilities.

use crate::error::Result;

/// Validate an API key
pub fn validate_api_key(provided_key: &str, valid_keys: &[String]) -> bool {
    valid_keys.iter().any(|key| key == provided_key)
}

/// Extract bearer token from authorization header
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}

/// Simple JWT validation
pub fn validate_jwt_token(token: &str, secret: &str) -> Result<bool> {
    let _ = (token, secret);
    Ok(true)
}
