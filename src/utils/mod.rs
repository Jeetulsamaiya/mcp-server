//! Utility functions and helpers for the MCP server.
//!
//! This module contains various utility functions, logging setup,
//! and other helper functionality.

pub mod auth;
pub mod logging;
pub mod validation;

use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique request ID
pub fn generate_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Generate a timestamp in ISO 8601 format
pub fn generate_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Get current Unix timestamp
pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Validate a JSON-RPC ID
pub fn validate_jsonrpc_id(id: &serde_json::Value) -> bool {
    match id {
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Number(_) => true,
        serde_json::Value::Null => false, // Null IDs are not allowed in MCP
        _ => false,
    }
}

/// Sanitize a string for logging (remove sensitive information)
pub fn sanitize_for_logging(input: &str) -> String {
    let sensitive_patterns = ["password", "token", "key", "secret"];

    let mut result = input.to_string();
    for pattern in &sensitive_patterns {
        if result.to_lowercase().contains(pattern) {
            result = format!("[REDACTED:{}]", pattern);
            break;
        }
    }

    result
}

/// Parse a version string into components
pub fn parse_version(version: &str) -> Result<(u32, u32, u32), String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err("Version must have format major.minor.patch".to_string());
    }

    let major = parts[0]
        .parse::<u32>()
        .map_err(|_| "Invalid major version number")?;
    let minor = parts[1]
        .parse::<u32>()
        .map_err(|_| "Invalid minor version number")?;
    let patch = parts[2]
        .parse::<u32>()
        .map_err(|_| "Invalid patch version number")?;

    Ok((major, minor, patch))
}

/// Compare two version strings
pub fn compare_versions(v1: &str, v2: &str) -> Result<std::cmp::Ordering, String> {
    let (major1, minor1, patch1) = parse_version(v1)?;
    let (major2, minor2, patch2) = parse_version(v2)?;

    match major1.cmp(&major2) {
        std::cmp::Ordering::Equal => match minor1.cmp(&minor2) {
            std::cmp::Ordering::Equal => Ok(patch1.cmp(&patch2)),
            other => Ok(other),
        },
        other => Ok(other),
    }
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes < THRESHOLD {
        return format!("{} B", bytes);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

/// Format duration as human-readable string
pub fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();

    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}m {}s", minutes, seconds)
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        format!("{}h {}m {}s", hours, minutes, seconds)
    }
}

/// Truncate a string to a maximum length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Check if a string is a valid URI
pub fn is_valid_uri(uri: &str) -> bool {
    url::Url::parse(uri).is_ok()
}

/// Extract file extension from a path or URI
pub fn extract_file_extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
}

/// Generate a secure random string
pub fn generate_random_string(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Retry a future with exponential backoff
pub async fn retry_with_backoff<F, Fut, T, E>(
    mut operation: F,
    max_retries: usize,
    initial_delay: std::time::Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_retries {
                    return Err(e);
                }

                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, std::time::Duration::from_secs(60));
            }
        }
    }

    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();

        assert_ne!(id1, id2);
        assert!(uuid::Uuid::parse_str(&id1).is_ok());
        assert!(uuid::Uuid::parse_str(&id2).is_ok());
    }

    #[test]
    fn test_validate_jsonrpc_id() {
        assert!(validate_jsonrpc_id(&serde_json::Value::String(
            "test".to_string()
        )));
        assert!(validate_jsonrpc_id(&serde_json::Value::Number(
            serde_json::Number::from(123)
        )));
        assert!(!validate_jsonrpc_id(&serde_json::Value::Null));
        assert!(!validate_jsonrpc_id(&serde_json::Value::String(
            "".to_string()
        )));
        assert!(!validate_jsonrpc_id(&serde_json::Value::Bool(true)));
    }

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("1.2.3"), Ok((1, 2, 3)));
        assert_eq!(parse_version("0.1.0"), Ok((0, 1, 0)));
        assert!(parse_version("1.2").is_err());
        assert!(parse_version("1.2.3.4").is_err());
        assert!(parse_version("a.b.c").is_err());
    }

    #[test]
    fn test_compare_versions() {
        assert_eq!(
            compare_versions("1.2.3", "1.2.3"),
            Ok(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            compare_versions("1.2.4", "1.2.3"),
            Ok(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            compare_versions("1.2.3", "1.2.4"),
            Ok(std::cmp::Ordering::Less)
        );
        assert_eq!(
            compare_versions("2.0.0", "1.9.9"),
            Ok(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(std::time::Duration::from_secs(30)), "30s");
        assert_eq!(
            format_duration(std::time::Duration::from_secs(90)),
            "1m 30s"
        );
        assert_eq!(
            format_duration(std::time::Duration::from_secs(3661)),
            "1h 1m 1s"
        );
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
        assert_eq!(truncate_string("hello", 3), "...");
    }

    #[test]
    fn test_is_valid_uri() {
        assert!(is_valid_uri("https://example.com"));
        assert!(is_valid_uri("file:///path/to/file"));
        assert!(is_valid_uri("mcp://server"));
        assert!(!is_valid_uri("not a uri"));
        assert!(!is_valid_uri(""));
    }

    #[test]
    fn test_extract_file_extension() {
        assert_eq!(extract_file_extension("file.txt"), Some("txt".to_string()));
        assert_eq!(
            extract_file_extension("path/to/file.JSON"),
            Some("json".to_string())
        );
        assert_eq!(extract_file_extension("file"), None);
        assert_eq!(extract_file_extension("file."), Some("".to_string()));
    }

    #[test]
    fn test_generate_random_string() {
        let s1 = generate_random_string(10);
        let s2 = generate_random_string(10);

        assert_eq!(s1.len(), 10);
        assert_eq!(s2.len(), 10);
        assert_ne!(s1, s2);
        assert!(s1.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_sanitize_for_logging() {
        assert_eq!(sanitize_for_logging("hello world"), "hello world");
        assert_eq!(
            sanitize_for_logging("my password is secret"),
            "[REDACTED:password]"
        );
        assert_eq!(
            sanitize_for_logging("API token: abc123"),
            "[REDACTED:token]"
        );
    }
}
