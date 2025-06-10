//! Completion management for MCP server.
//!
//! This module implements the completion feature of MCP, allowing the server
//! to provide argument completion suggestions to clients.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::{McpError, Result};
use crate::server::features::FeatureManager;

/// Completion manager for handling MCP completions
pub struct CompletionManager {
    /// Completion providers
    providers: Arc<RwLock<HashMap<String, Box<dyn CompletionProvider>>>>,

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Completion provider trait for different completion types
#[async_trait::async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Check if the provider can handle the given reference
    fn can_handle(&self, reference: &CompletionReference) -> bool;

    /// Provide completions for the given context
    async fn complete(&self, context: &CompletionContext) -> Result<CompletionResult>;
}

/// Completion reference (prompt or resource)
#[derive(Debug, Clone)]
pub enum CompletionReference {
    Prompt { name: String },
    Resource { uri: String },
}

/// Completion context
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// Reference to the prompt or resource
    pub reference: CompletionReference,

    /// Argument information
    pub argument: ArgumentInfo,
}

/// Argument information for completion
#[derive(Debug, Clone)]
pub struct ArgumentInfo {
    /// Argument name
    pub name: String,

    /// Current argument value (partial)
    pub value: String,
}

/// Completion result
#[derive(Debug, Clone)]
pub struct CompletionResult {
    /// Completion values
    pub values: Vec<String>,

    /// Total number of available completions
    pub total: Option<usize>,

    /// Whether there are more completions available
    pub has_more: Option<bool>,
}

impl CompletionManager {
    /// Create a new completion manager
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Register a completion provider
    pub async fn register_provider(&self, provider: Box<dyn CompletionProvider>) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Completion feature is disabled".to_string(),
            ));
        }

        let name = provider.name().to_string();

        {
            let mut providers = self.providers.write().await;
            providers.insert(name.clone(), provider);
        }

        info!("Registered completion provider: {}", name);
        Ok(())
    }

    /// Get completions for the given context
    pub async fn complete(&self, context: CompletionContext) -> Result<CompletionResult> {
        if !self.is_enabled() {
            return Err(McpError::Resource(
                "Completion feature is disabled".to_string(),
            ));
        }

        // Find a provider that can handle this reference
        let providers = self.providers.read().await;
        for provider in providers.values() {
            if provider.can_handle(&context.reference) {
                let result = provider.complete(&context).await?;
                info!(
                    "Completion provider {} returned {} values",
                    provider.name(),
                    result.values.len()
                );
                return Ok(result);
            }
        }

        // No provider found, return empty result
        Ok(CompletionResult {
            values: Vec::new(),
            total: Some(0),
            has_more: Some(false),
        })
    }
}

impl FeatureManager for CompletionManager {
    fn name(&self) -> &'static str {
        "completion"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}
}

impl CompletionResult {
    /// Create a new completion result
    pub fn new(values: Vec<String>) -> Self {
        let total = values.len();
        Self {
            values,
            total: Some(total),
            has_more: Some(false),
        }
    }

    /// Create a completion result with pagination info
    pub fn with_pagination(values: Vec<String>, total: Option<usize>, has_more: bool) -> Self {
        Self {
            values,
            total,
            has_more: Some(has_more),
        }
    }

    /// Create an empty completion result
    pub fn empty() -> Self {
        Self {
            values: Vec::new(),
            total: Some(0),
            has_more: Some(false),
        }
    }
}

/// File path completion provider
pub struct FilePathCompletionProvider {
    /// Root directory for file completions
    root_dir: std::path::PathBuf,
}

impl FilePathCompletionProvider {
    /// Create a new file path completion provider
    pub fn new(root_dir: std::path::PathBuf) -> Self {
        Self { root_dir }
    }
}

#[async_trait::async_trait]
impl CompletionProvider for FilePathCompletionProvider {
    fn name(&self) -> &str {
        "filepath"
    }

    fn can_handle(&self, reference: &CompletionReference) -> bool {
        match reference {
            CompletionReference::Resource { uri } => uri.starts_with("file://"),
            _ => false,
        }
    }

    async fn complete(&self, context: &CompletionContext) -> Result<CompletionResult> {
        let partial_path = &context.argument.value;

        // Determine the directory to search in
        let search_dir = if partial_path.is_empty() {
            self.root_dir.clone()
        } else {
            let path = std::path::Path::new(partial_path);
            if path.is_absolute() {
                if let Some(parent) = path.parent() {
                    parent.to_path_buf()
                } else {
                    return Ok(CompletionResult::empty());
                }
            } else {
                self.root_dir
                    .join(path.parent().unwrap_or(std::path::Path::new("")))
            }
        };

        // Read directory entries
        let mut completions = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&search_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Filter based on partial input
                if name.starts_with(&partial_path.split('/').last().unwrap_or("")) {
                    if path.is_dir() {
                        completions.push(format!("{}/", name));
                    } else {
                        completions.push(name);
                    }
                }
            }
        }

        completions.sort();
        completions.truncate(100); // Limit to 100 completions

        Ok(CompletionResult::new(completions))
    }
}

/// Static completion provider for predefined values
pub struct StaticCompletionProvider {
    /// Provider name
    name: String,

    /// Static completion values
    values: Vec<String>,

    /// Reference patterns this provider handles
    patterns: Vec<String>,
}

impl StaticCompletionProvider {
    /// Create a new static completion provider
    pub fn new(name: String, values: Vec<String>, patterns: Vec<String>) -> Self {
        Self {
            name,
            values,
            patterns,
        }
    }

    /// Create a provider for common programming languages
    pub fn programming_languages() -> Self {
        let languages = vec![
            "rust".to_string(),
            "python".to_string(),
            "javascript".to_string(),
            "typescript".to_string(),
            "java".to_string(),
            "c".to_string(),
            "cpp".to_string(),
            "go".to_string(),
            "ruby".to_string(),
            "php".to_string(),
            "swift".to_string(),
            "kotlin".to_string(),
            "scala".to_string(),
            "haskell".to_string(),
            "clojure".to_string(),
        ];

        Self::new(
            "programming_languages".to_string(),
            languages,
            vec!["language".to_string(), "lang".to_string()],
        )
    }

    /// Create a provider for common file extensions
    pub fn file_extensions() -> Self {
        let extensions = vec![
            ".rs".to_string(),
            ".py".to_string(),
            ".js".to_string(),
            ".ts".to_string(),
            ".java".to_string(),
            ".c".to_string(),
            ".cpp".to_string(),
            ".go".to_string(),
            ".rb".to_string(),
            ".php".to_string(),
            ".swift".to_string(),
            ".kt".to_string(),
            ".scala".to_string(),
            ".hs".to_string(),
            ".clj".to_string(),
            ".txt".to_string(),
            ".md".to_string(),
            ".json".to_string(),
            ".xml".to_string(),
            ".yaml".to_string(),
            ".toml".to_string(),
        ];

        Self::new(
            "file_extensions".to_string(),
            extensions,
            vec![
                "extension".to_string(),
                "ext".to_string(),
                "file_type".to_string(),
            ],
        )
    }
}

#[async_trait::async_trait]
impl CompletionProvider for StaticCompletionProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn can_handle(&self, reference: &CompletionReference) -> bool {
        match reference {
            CompletionReference::Prompt { name } => {
                self.patterns.iter().any(|pattern| name.contains(pattern))
            }
            CompletionReference::Resource { uri } => {
                self.patterns.iter().any(|pattern| uri.contains(pattern))
            }
        }
    }

    async fn complete(&self, context: &CompletionContext) -> Result<CompletionResult> {
        let partial = &context.argument.value.to_lowercase();

        let mut matching_values: Vec<String> = self
            .values
            .iter()
            .filter(|value| value.to_lowercase().starts_with(partial))
            .cloned()
            .collect();

        matching_values.sort();
        matching_values.truncate(100); // Limit to 100 completions

        Ok(CompletionResult::new(matching_values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_completion_manager() {
        let manager = CompletionManager::new();

        // Register a static provider
        let provider = Box::new(StaticCompletionProvider::programming_languages());
        assert!(manager.register_provider(provider).await.is_ok());

        // Test completion
        let context = CompletionContext {
            reference: CompletionReference::Prompt {
                name: "code_language".to_string(),
            },
            argument: ArgumentInfo {
                name: "language".to_string(),
                value: "ru".to_string(),
            },
        };

        let result = manager.complete(context).await.unwrap();
        assert!(!result.values.is_empty());
        assert!(result.values.contains(&"rust".to_string()));
        assert!(result.values.contains(&"ruby".to_string()));
    }

    #[tokio::test]
    async fn test_static_completion_provider() {
        let provider = StaticCompletionProvider::programming_languages();

        let context = CompletionContext {
            reference: CompletionReference::Prompt {
                name: "test_language".to_string(),
            },
            argument: ArgumentInfo {
                name: "language".to_string(),
                value: "py".to_string(),
            },
        };

        assert!(provider.can_handle(&context.reference));

        let result = provider.complete(&context).await.unwrap();
        assert_eq!(result.values, vec!["python".to_string()]);
    }

    #[tokio::test]
    async fn test_file_path_completion_provider() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "test content").await.unwrap();

        let provider = FilePathCompletionProvider::new(temp_dir.path().to_path_buf());

        let context = CompletionContext {
            reference: CompletionReference::Resource {
                uri: "file:///test".to_string(),
            },
            argument: ArgumentInfo {
                name: "path".to_string(),
                value: "te".to_string(),
            },
        };

        assert!(provider.can_handle(&context.reference));

        let result = provider.complete(&context).await.unwrap();
        assert!(!result.values.is_empty());
        assert!(result.values.iter().any(|v| v.starts_with("test")));
    }
}
