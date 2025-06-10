//! Roots management for MCP client features.
//!
//! This module implements the roots feature, allowing clients to provide
//! root directories that servers can operate on.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::{McpError, Result};

/// Roots manager for handling root directories
pub struct RootsManager {
    /// Registered roots
    roots: Arc<RwLock<HashMap<String, Root>>>,

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Root directory information
#[derive(Debug, Clone)]
pub struct Root {
    /// Root URI (must start with file://)
    pub uri: String,

    /// Optional human-readable name
    pub name: Option<String>,

    /// Root path (derived from URI)
    pub path: PathBuf,

    /// Whether the root is accessible
    pub accessible: bool,

    /// Root metadata
    pub metadata: RootMetadata,
}

/// Root metadata
#[derive(Debug, Clone)]
pub struct RootMetadata {
    /// Root type
    pub root_type: RootType,

    /// Whether the root is read-only
    pub read_only: bool,

    /// File count (if known)
    pub file_count: Option<usize>,

    /// Total size in bytes (if known)
    pub total_size: Option<u64>,

    /// Last modified time
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// Root type enumeration
#[derive(Debug, Clone)]
pub enum RootType {
    Directory,
    Repository,
    Project,
    Workspace,
    Other(String),
}

impl RootsManager {
    /// Create a new roots manager
    pub fn new() -> Self {
        Self {
            roots: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Add a root directory
    pub async fn add_root(&self, uri: String, name: Option<String>) -> Result<()> {
        if !self.is_enabled().await {
            return Err(McpError::Resource("Roots feature is disabled".to_string()));
        }

        // Validate URI format
        if !uri.starts_with("file://") {
            return Err(McpError::invalid_params("Root URI must start with file://"));
        }

        // Parse path from URI
        let path = self.uri_to_path(&uri)?;

        // Check if path exists and is accessible
        let accessible = path.exists() && path.is_dir();

        // Generate metadata
        let metadata = self.generate_metadata(&path).await?;

        let root = Root {
            uri: uri.clone(),
            name,
            path,
            accessible,
            metadata,
        };

        {
            let mut roots = self.roots.write().await;
            roots.insert(uri.clone(), root);
        }

        info!("Added root: {}", uri);
        Ok(())
    }

    /// Remove a root directory
    pub async fn remove_root(&self, uri: &str) -> Result<Option<Root>> {
        let mut roots = self.roots.write().await;
        let root = roots.remove(uri);

        if root.is_some() {
            info!("Removed root: {}", uri);
        }

        Ok(root)
    }

    /// Get a root by URI
    pub async fn get_root(&self, uri: &str) -> Option<Root> {
        let roots = self.roots.read().await;
        roots.get(uri).cloned()
    }

    /// List all roots
    pub async fn list_roots(&self) -> Result<Vec<Root>> {
        if !self.is_enabled().await {
            return Err(McpError::Resource("Roots feature is disabled".to_string()));
        }

        let roots = self.roots.read().await;
        let mut all_roots: Vec<Root> = roots.values().cloned().collect();

        // Sort by URI for consistent ordering
        all_roots.sort_by(|a, b| a.uri.cmp(&b.uri));

        Ok(all_roots)
    }

    /// Add a root from a file path
    pub async fn add_root_from_path(&self, path: PathBuf, name: Option<String>) -> Result<()> {
        let uri = self.path_to_uri(&path)?;
        self.add_root(uri, name).await
    }

    /// Check if a path is within any registered root
    pub async fn is_path_allowed(&self, path: &PathBuf) -> bool {
        let roots = self.roots.read().await;

        for root in roots.values() {
            if root.accessible && path.starts_with(&root.path) {
                return true;
            }
        }

        false
    }

    /// Get the root that contains the given path
    pub async fn find_containing_root(&self, path: &PathBuf) -> Option<Root> {
        let roots = self.roots.read().await;

        for root in roots.values() {
            if root.accessible && path.starts_with(&root.path) {
                return Some(root.clone());
            }
        }

        None
    }

    /// Refresh metadata for all roots
    pub async fn refresh_metadata(&self) -> Result<()> {
        let mut roots = self.roots.write().await;

        for root in roots.values_mut() {
            // Check accessibility
            root.accessible = root.path.exists() && root.path.is_dir();

            // Update metadata
            if root.accessible {
                match self.generate_metadata(&root.path).await {
                    Ok(metadata) => root.metadata = metadata,
                    Err(e) => {
                        info!("Failed to update metadata for {}: {}", root.uri, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if roots feature is enabled
    pub async fn is_enabled(&self) -> bool {
        let enabled = self.enabled.read().await;
        *enabled
    }

    /// Enable or disable roots feature
    pub async fn set_enabled(&self, enabled: bool) {
        let mut current_enabled = self.enabled.write().await;
        *current_enabled = enabled;
    }

    /// Convert URI to file path
    fn uri_to_path(&self, uri: &str) -> Result<PathBuf> {
        let url = url::Url::parse(uri)
            .map_err(|e| McpError::invalid_params(format!("Invalid URI: {}", e)))?;

        if url.scheme() != "file" {
            return Err(McpError::invalid_params("Only file:// URIs are supported"));
        }

        url.to_file_path()
            .map_err(|_| McpError::invalid_params("Invalid file path in URI"))
    }

    /// Convert file path to URI
    fn path_to_uri(&self, path: &PathBuf) -> Result<String> {
        let canonical_path = path
            .canonicalize()
            .map_err(|e| McpError::invalid_params(format!("Cannot canonicalize path: {}", e)))?;

        let url = url::Url::from_file_path(&canonical_path)
            .map_err(|_| McpError::invalid_params("Cannot convert path to URI"))?;

        Ok(url.to_string())
    }

    /// Generate metadata for a root path
    async fn generate_metadata(&self, path: &PathBuf) -> Result<RootMetadata> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|e| McpError::Resource(format!("Failed to read metadata: {}", e)))?;

        let root_type = self.detect_root_type(path).await;
        let read_only = metadata.permissions().readonly();

        // Try to get last modified time
        let last_modified = metadata.modified().ok().and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| {
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .unwrap_or_else(chrono::Utc::now)
                })
        });

        // Count files and calculate size (limited scan for performance)
        let (file_count, total_size) = self.scan_directory(path, 1000).await;

        Ok(RootMetadata {
            root_type,
            read_only,
            file_count,
            total_size,
            last_modified,
        })
    }

    /// Detect the type of root directory
    async fn detect_root_type(&self, path: &PathBuf) -> RootType {
        if path.join(".git").exists() {
            return RootType::Repository;
        }

        if path.join("Cargo.toml").exists()
            || path.join("package.json").exists()
            || path.join("pyproject.toml").exists()
            || path.join("pom.xml").exists()
        {
            return RootType::Project;
        }

        if path.join(".vscode").exists() || path.join(".idea").exists() {
            return RootType::Workspace;
        }

        RootType::Directory
    }

    /// Scan directory for file count and size (with limits for performance)
    async fn scan_directory(
        &self,
        path: &PathBuf,
        max_files: usize,
    ) -> (Option<usize>, Option<u64>) {
        let mut file_count = 0;
        let mut total_size = 0;

        if let Ok(mut entries) = tokio::fs::read_dir(path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if file_count >= max_files {
                    break;
                }

                if let Ok(metadata) = entry.metadata().await {
                    if metadata.is_file() {
                        file_count += 1;
                        total_size += metadata.len();
                    }
                }
            }
        }

        let file_count = if file_count > 0 {
            Some(file_count)
        } else {
            None
        };
        let total_size = if total_size > 0 {
            Some(total_size)
        } else {
            None
        };

        (file_count, total_size)
    }
}

impl Default for RootsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Root {
    /// Create a new root
    pub fn new(uri: String, path: PathBuf) -> Self {
        Self {
            uri,
            name: None,
            path,
            accessible: false,
            metadata: RootMetadata {
                root_type: RootType::Directory,
                read_only: false,
                file_count: None,
                total_size: None,
                last_modified: None,
            },
        }
    }

    /// Create a new root with name
    pub fn with_name(uri: String, path: PathBuf, name: String) -> Self {
        let mut root = Self::new(uri, path);
        root.name = Some(name);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_roots_manager() {
        let manager = RootsManager::new();
        let temp_dir = TempDir::new().unwrap();

        // Add a root
        let uri = format!("file://{}", temp_dir.path().display());
        assert!(manager
            .add_root(uri.clone(), Some("Test Root".to_string()))
            .await
            .is_ok());

        // Test retrieval
        let root = manager.get_root(&uri).await;
        assert!(root.is_some());
        let root = root.unwrap();
        assert_eq!(root.name, Some("Test Root".to_string()));
        assert!(root.accessible);

        // Test listing
        let roots = manager.list_roots().await.unwrap();
        assert_eq!(roots.len(), 1);

        // Test path checking
        let test_file = temp_dir.path().join("test.txt");
        assert!(manager.is_path_allowed(&test_file).await);

        let outside_path = PathBuf::from("/tmp/outside");
        assert!(!manager.is_path_allowed(&outside_path).await);

        // Test removal
        let removed = manager.remove_root(&uri).await.unwrap();
        assert!(removed.is_some());

        let not_found = manager.get_root(&uri).await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_root_type_detection() {
        let manager = RootsManager::new();
        let temp_dir = TempDir::new().unwrap();

        // Create a Cargo.toml to make it look like a Rust project
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        let root_type = manager
            .detect_root_type(&temp_dir.path().to_path_buf())
            .await;
        assert!(matches!(root_type, RootType::Project));
    }

    #[test]
    fn test_uri_path_conversion() {
        let manager = RootsManager::new();

        // Test URI to path
        let uri = "file:///tmp/test";
        let path = manager.uri_to_path(uri).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test"));

        // Test invalid URI
        let invalid_uri = "http://example.com";
        assert!(manager.uri_to_path(invalid_uri).is_err());
    }
}
