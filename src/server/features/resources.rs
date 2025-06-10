//! Resource management for MCP server.
//!
//! This module implements the resource feature of MCP, allowing the server
//! to expose resources (files, data, etc.) to clients.

use base64::Engine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use url::Url;

use crate::error::{McpError, Result};
use crate::protocol::{
    PaginationParams, PaginationResult, Resource, ResourceContents, ResourceTemplate,
};
use crate::server::features::FeatureManager;

/// Resource manager for handling MCP resources
pub struct ResourceManager {
    /// Registered resources
    resources: Arc<RwLock<HashMap<String, Resource>>>,

    /// Resource templates
    templates: Arc<RwLock<HashMap<String, ResourceTemplate>>>,

    /// Resource providers
    providers: Arc<RwLock<HashMap<String, Box<dyn ResourceProvider>>>>,

    /// Resource subscriptions
    subscriptions: Arc<RwLock<HashMap<String, Vec<String>>>>, // URI -> client IDs

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Resource provider trait for different resource types
#[async_trait::async_trait]
pub trait ResourceProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Check if the provider can handle the given URI
    fn can_handle(&self, uri: &str) -> bool;

    /// Read resource contents
    async fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContents>>;

    /// List resources (optional)
    async fn list_resources(&self, pattern: Option<&str>) -> Result<Vec<Resource>> {
        let _ = pattern;
        Ok(Vec::new())
    }

    /// Subscribe to resource updates (optional)
    async fn subscribe(&self, uri: &str) -> Result<()> {
        let _ = uri;
        Ok(())
    }

    /// Unsubscribe from resource updates (optional)
    async fn unsubscribe(&self, uri: &str) -> Result<()> {
        let _ = uri;
        Ok(())
    }
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new() -> Self {
        Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            templates: Arc::new(RwLock::new(HashMap::new())),
            providers: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Register a resource
    pub async fn register_resource(&self, resource: Resource) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Resource feature is disabled".to_string(),
            ));
        }

        let uri = resource.uri.clone();

        {
            let mut resources = self.resources.write().await;
            resources.insert(uri.clone(), resource);
        }

        info!("Registered resource: {}", uri);
        Ok(())
    }

    /// Unregister a resource
    pub async fn unregister_resource(&self, uri: &str) -> Result<Option<Resource>> {
        let mut resources = self.resources.write().await;
        let resource = resources.remove(uri);

        if resource.is_some() {
            info!("Unregistered resource: {}", uri);
        }

        Ok(resource)
    }

    /// Get a resource by URI
    pub async fn get_resource(&self, uri: &str) -> Option<Resource> {
        let resources = self.resources.read().await;
        resources.get(uri).cloned()
    }

    /// List all resources with optional pagination
    pub async fn list_resources(
        &self,
        pagination: Option<PaginationParams>,
    ) -> Result<(Vec<Resource>, PaginationResult)> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Resource feature is disabled".to_string(),
            ));
        }

        let resources = self.resources.read().await;
        let mut all_resources: Vec<Resource> = resources.values().cloned().collect();

        // Add resources from providers
        let providers = self.providers.read().await;
        for provider in providers.values() {
            match provider.list_resources(None).await {
                Ok(provider_resources) => {
                    all_resources.extend(provider_resources);
                }
                Err(e) => {
                    warn!(
                        "Failed to list resources from provider {}: {}",
                        provider.name(),
                        e
                    );
                }
            }
        }

        // Sort by URI for consistent ordering
        all_resources.sort_by(|a, b| a.uri.cmp(&b.uri));

        // Apply pagination if provided
        let (resources, pagination_result) = if let Some(params) = pagination {
            self.apply_pagination(all_resources, params)?
        } else {
            (all_resources, PaginationResult { next_cursor: None })
        };

        Ok((resources, pagination_result))
    }

    /// Register a resource template
    pub async fn register_template(&self, template: ResourceTemplate) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Resource feature is disabled".to_string(),
            ));
        }

        let uri_template = template.uri_template.clone();

        {
            let mut templates = self.templates.write().await;
            templates.insert(uri_template.clone(), template);
        }

        info!("Registered resource template: {}", uri_template);
        Ok(())
    }

    /// List all resource templates with optional pagination
    pub async fn list_templates(
        &self,
        pagination: Option<PaginationParams>,
    ) -> Result<(Vec<ResourceTemplate>, PaginationResult)> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Resource feature is disabled".to_string(),
            ));
        }

        let templates = self.templates.read().await;
        let mut all_templates: Vec<ResourceTemplate> = templates.values().cloned().collect();

        // Sort by URI template for consistent ordering
        all_templates.sort_by(|a, b| a.uri_template.cmp(&b.uri_template));

        // Apply pagination if provided
        let (templates, pagination_result) = if let Some(params) = pagination {
            self.apply_template_pagination(all_templates, params)?
        } else {
            (all_templates, PaginationResult { next_cursor: None })
        };

        Ok((templates, pagination_result))
    }

    /// Read resource contents
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContents>> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Resource feature is disabled".to_string(),
            ));
        }

        // First check if we have a registered resource
        if let Some(_resource) = self.get_resource(uri).await {
            // Try to find a provider that can handle this URI
            let providers = self.providers.read().await;
            for provider in providers.values() {
                if provider.can_handle(uri) {
                    return provider.read_resource(uri).await;
                }
            }
        }

        // If no provider found, try all providers
        let providers = self.providers.read().await;
        for provider in providers.values() {
            if provider.can_handle(uri) {
                return provider.read_resource(uri).await;
            }
        }

        Err(McpError::Resource(format!(
            "No provider found for resource: {}",
            uri
        )))
    }

    /// Subscribe to resource updates
    pub async fn subscribe(&self, uri: &str, client_id: &str) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Resource feature is disabled".to_string(),
            ));
        }

        // Add to subscriptions
        {
            let mut subscriptions = self.subscriptions.write().await;
            let clients = subscriptions
                .entry(uri.to_string())
                .or_insert_with(Vec::new);
            if !clients.contains(&client_id.to_string()) {
                clients.push(client_id.to_string());
            }
        }

        // Notify providers
        let providers = self.providers.read().await;
        for provider in providers.values() {
            if provider.can_handle(uri) {
                if let Err(e) = provider.subscribe(uri).await {
                    warn!(
                        "Provider {} failed to subscribe to {}: {}",
                        provider.name(),
                        uri,
                        e
                    );
                }
            }
        }

        info!("Client {} subscribed to resource: {}", client_id, uri);
        Ok(())
    }

    /// Unsubscribe from resource updates
    pub async fn unsubscribe(&self, uri: &str, client_id: &str) -> Result<()> {
        // Remove from subscriptions
        {
            let mut subscriptions = self.subscriptions.write().await;
            if let Some(clients) = subscriptions.get_mut(uri) {
                clients.retain(|id| id != client_id);
                if clients.is_empty() {
                    subscriptions.remove(uri);
                }
            }
        }

        // Check if any clients are still subscribed
        let has_subscribers = {
            let subscriptions = self.subscriptions.read().await;
            subscriptions.contains_key(uri)
        };

        // If no more subscribers, notify providers
        if !has_subscribers {
            let providers = self.providers.read().await;
            for provider in providers.values() {
                if provider.can_handle(uri) {
                    if let Err(e) = provider.unsubscribe(uri).await {
                        warn!(
                            "Provider {} failed to unsubscribe from {}: {}",
                            provider.name(),
                            uri,
                            e
                        );
                    }
                }
            }
        }

        info!("Client {} unsubscribed from resource: {}", client_id, uri);
        Ok(())
    }

    /// Register a resource provider
    pub async fn register_provider(&self, provider: Box<dyn ResourceProvider>) -> Result<()> {
        let name = provider.name().to_string();

        {
            let mut providers = self.providers.write().await;
            providers.insert(name.clone(), provider);
        }

        info!("Registered resource provider: {}", name);
        Ok(())
    }

    /// Get resource count
    pub async fn get_resource_count(&self) -> usize {
        let resources = self.resources.read().await;
        resources.len()
    }

    /// Get subscription count
    pub async fn get_subscription_count(&self) -> usize {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.len()
    }

    /// Check if the feature is enabled
    pub fn is_enabled(&self) -> bool {
        // Use try_read to avoid blocking in sync context
        self.enabled
            .try_read()
            .map(|enabled| *enabled)
            .unwrap_or(true)
    }

    /// Check if the feature is enabled (async version)
    pub async fn is_enabled_async(&self) -> bool {
        let enabled = self.enabled.read().await;
        *enabled
    }

    /// Set enabled state
    pub async fn set_enabled(&self, enabled: bool) {
        let mut state = self.enabled.write().await;
        *state = enabled;
    }

    /// Apply pagination to resources
    fn apply_pagination(
        &self,
        mut resources: Vec<Resource>,
        params: PaginationParams,
    ) -> Result<(Vec<Resource>, PaginationResult)> {
        let start_index = if let Some(cursor) = params.cursor {
            cursor.parse::<usize>().unwrap_or(0)
        } else {
            0
        };

        let page_size = 50; // Default page size
        let end_index = std::cmp::min(start_index + page_size, resources.len());

        let page_resources = if start_index < resources.len() {
            resources.drain(start_index..end_index).collect()
        } else {
            Vec::new()
        };

        let next_cursor = if end_index < resources.len() {
            Some(end_index.to_string())
        } else {
            None
        };

        Ok((page_resources, PaginationResult { next_cursor }))
    }

    /// Apply pagination to templates
    fn apply_template_pagination(
        &self,
        mut templates: Vec<ResourceTemplate>,
        params: PaginationParams,
    ) -> Result<(Vec<ResourceTemplate>, PaginationResult)> {
        let start_index = if let Some(cursor) = params.cursor {
            cursor.parse::<usize>().unwrap_or(0)
        } else {
            0
        };

        let page_size = 50;
        let end_index = std::cmp::min(start_index + page_size, templates.len());

        let page_templates = if start_index < templates.len() {
            templates.drain(start_index..end_index).collect()
        } else {
            Vec::new()
        };

        let next_cursor = if end_index < templates.len() {
            Some(end_index.to_string())
        } else {
            None
        };

        Ok((page_templates, PaginationResult { next_cursor }))
    }
}

impl FeatureManager for ResourceManager {
    fn name(&self) -> &'static str {
        "resources"
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }

    fn set_enabled(&mut self, enabled: bool) {
        if let Ok(mut state) = self.enabled.try_write() {
            *state = enabled;
        }
    }
}

/// File system resource provider
pub struct FileSystemProvider {
    /// Root directory for file access
    root_dir: PathBuf,

    /// Whether to allow access outside root directory
    allow_outside_root: bool,
}

impl FileSystemProvider {
    /// Create a new file system provider
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            allow_outside_root: false,
        }
    }

    /// Create a new file system provider with custom settings
    pub fn with_settings(root_dir: PathBuf, allow_outside_root: bool) -> Self {
        Self {
            root_dir,
            allow_outside_root,
        }
    }

    /// Validate and resolve file path
    fn resolve_path(&self, uri: &str) -> Result<PathBuf> {
        let url = Url::parse(uri).map_err(|e| McpError::Resource(format!("Invalid URI: {}", e)))?;

        if url.scheme() != "file" {
            return Err(McpError::Resource(
                "Only file:// URIs are supported".to_string(),
            ));
        }

        let path = url
            .to_file_path()
            .map_err(|_| McpError::Resource("Invalid file path".to_string()))?;

        // Security check: ensure path is within root directory
        if !self.allow_outside_root {
            let canonical_path = path
                .canonicalize()
                .map_err(|e| McpError::Resource(format!("Failed to canonicalize path: {}", e)))?;

            let canonical_root = self
                .root_dir
                .canonicalize()
                .map_err(|e| McpError::Resource(format!("Failed to canonicalize root: {}", e)))?;

            if !canonical_path.starts_with(&canonical_root) {
                return Err(McpError::Resource(
                    "Access denied: path outside root directory".to_string(),
                ));
            }
        }

        Ok(path)
    }
}

#[async_trait::async_trait]
impl ResourceProvider for FileSystemProvider {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("file://")
    }

    async fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContents>> {
        let path = self.resolve_path(uri)?;

        if !path.exists() {
            return Err(McpError::Resource(format!(
                "File not found: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(McpError::Resource(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        // Read file contents
        let contents = tokio::fs::read(&path)
            .await
            .map_err(|e| McpError::Resource(format!("Failed to read file: {}", e)))?;

        // Determine MIME type
        let mime_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();

        // Try to read as text first
        if let Ok(text) = String::from_utf8(contents.clone()) {
            Ok(vec![ResourceContents::Text {
                uri: uri.to_string(),
                mime_type: Some(mime_type),
                text,
            }])
        } else {
            // Fall back to binary
            let blob = base64::engine::general_purpose::STANDARD.encode(&contents);
            Ok(vec![ResourceContents::Blob {
                uri: uri.to_string(),
                mime_type: Some(mime_type),
                blob,
            }])
        }
    }

    async fn list_resources(&self, pattern: Option<&str>) -> Result<Vec<Resource>> {
        let mut resources = Vec::new();

        // Walk the directory tree
        let mut entries = tokio::fs::read_dir(&self.root_dir)
            .await
            .map_err(|e| McpError::Resource(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| McpError::Resource(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();

            if path.is_file() {
                let uri = format!("file://{}", path.display());

                // Apply pattern filter if provided
                if let Some(pattern) = pattern {
                    if !path.to_string_lossy().contains(pattern) {
                        continue;
                    }
                }

                let metadata = entry
                    .metadata()
                    .await
                    .map_err(|e| McpError::Resource(format!("Failed to read metadata: {}", e)))?;

                let mime_type = mime_guess::from_path(&path)
                    .first_or_octet_stream()
                    .to_string();

                let resource = Resource {
                    uri,
                    name: path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    description: Some(format!("File: {}", path.display())),
                    mime_type: Some(mime_type),
                    annotations: None,
                    size: Some(metadata.len()),
                };

                resources.push(resource);
            }
        }

        Ok(resources)
    }
}

/// HTTP resource provider
pub struct HttpProvider {
    /// HTTP client
    client: reqwest::Client,

    /// Allowed URL patterns
    allowed_patterns: Vec<String>,
}

impl HttpProvider {
    /// Create a new HTTP provider
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            allowed_patterns: vec!["https://".to_string(), "http://".to_string()],
        }
    }

    /// Create a new HTTP provider with custom patterns
    pub fn with_patterns(patterns: Vec<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            allowed_patterns: patterns,
        }
    }
}

#[async_trait::async_trait]
impl ResourceProvider for HttpProvider {
    fn name(&self) -> &str {
        "http"
    }

    fn can_handle(&self, uri: &str) -> bool {
        self.allowed_patterns
            .iter()
            .any(|pattern| uri.starts_with(pattern))
    }

    async fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContents>> {
        let response = self
            .client
            .get(uri)
            .send()
            .await
            .map_err(|e| McpError::Resource(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(McpError::Resource(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let bytes = response
            .bytes()
            .await
            .map_err(|e| McpError::Resource(format!("Failed to read response body: {}", e)))?;

        // Try to decode as text if content type suggests it
        if let Some(ref ct) = content_type {
            if ct.starts_with("text/") || ct.contains("json") || ct.contains("xml") {
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    return Ok(vec![ResourceContents::Text {
                        uri: uri.to_string(),
                        mime_type: content_type,
                        text,
                    }]);
                }
            }
        }

        // Fall back to binary
        let blob = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(vec![ResourceContents::Blob {
            uri: uri.to_string(),
            mime_type: content_type,
            blob,
        }])
    }
}

impl Default for HttpProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_resource_manager() {
        let manager = ResourceManager::new();

        let resource = Resource {
            uri: "test://example".to_string(),
            name: "Test Resource".to_string(),
            description: Some("A test resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            annotations: None,
            size: Some(100),
        };

        // Test registration
        assert!(manager.register_resource(resource.clone()).await.is_ok());

        // Test retrieval
        let retrieved = manager.get_resource("test://example").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Resource");

        // Test listing
        let (resources, _) = manager.list_resources(None).await.unwrap();
        assert_eq!(resources.len(), 1);

        // Test unregistration
        let removed = manager.unregister_resource("test://example").await.unwrap();
        assert!(removed.is_some());

        let not_found = manager.get_resource("test://example").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_filesystem_provider() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "Hello, world!").await.unwrap();

        let provider = FileSystemProvider::new(temp_dir.path().to_path_buf());
        let uri = format!("file://{}", test_file.display());

        assert!(provider.can_handle(&uri));

        let contents = provider.read_resource(&uri).await.unwrap();
        assert_eq!(contents.len(), 1);

        if let ResourceContents::Text { text, .. } = &contents[0] {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected text content");
        }
    }
}
