//! Server feature implementations.
//!
//! This module contains the implementations of MCP server features including
//! resources, tools, prompts, and other capabilities.

pub mod completion;
pub mod logging;
pub mod prompts;
pub mod resources;
pub mod tools;

// Re-export main types
pub use completion::CompletionManager;
pub use logging::LoggingManager;
pub use prompts::PromptManager;
pub use resources::ResourceManager;
pub use tools::ToolManager;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::protocol::{
    PromptsCapability, ResourcesCapability, ServerCapabilities, ToolsCapability,
};

/// Feature manager trait for common functionality
pub trait FeatureManager: Send + Sync {
    /// Get the feature name
    fn name(&self) -> &'static str;

    /// Check if the feature is enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable the feature
    fn set_enabled(&mut self, enabled: bool);
}

/// Combined feature manager for all server capabilities
pub struct ServerFeatureManager {
    /// Resource manager
    pub resources: Arc<ResourceManager>,

    /// Tool manager
    pub tools: Arc<ToolManager>,

    /// Prompt manager
    pub prompts: Arc<PromptManager>,

    /// Logging manager
    pub logging: Arc<LoggingManager>,

    /// Completion manager
    pub completion: Arc<CompletionManager>,

    /// Feature enablement flags
    enabled_features: Arc<RwLock<HashMap<String, bool>>>,
}

impl ServerFeatureManager {
    /// Create a new server feature manager
    pub fn new() -> Self {
        let mut enabled_features = HashMap::new();
        enabled_features.insert("resources".to_string(), true);
        enabled_features.insert("tools".to_string(), true);
        enabled_features.insert("prompts".to_string(), true);
        enabled_features.insert("logging".to_string(), true);
        enabled_features.insert("completion".to_string(), true);

        Self {
            resources: Arc::new(ResourceManager::new()),
            tools: Arc::new(ToolManager::new()),
            prompts: Arc::new(PromptManager::new()),
            logging: Arc::new(LoggingManager::new()),
            completion: Arc::new(CompletionManager::new()),
            enabled_features: Arc::new(RwLock::new(enabled_features)),
        }
    }

    /// Check if a feature is enabled
    pub async fn is_feature_enabled(&self, feature: &str) -> bool {
        let features = self.enabled_features.read().await;
        features.get(feature).copied().unwrap_or(false)
    }

    /// Enable or disable a feature
    pub async fn set_feature_enabled(&self, feature: &str, enabled: bool) {
        let mut features = self.enabled_features.write().await;
        features.insert(feature.to_string(), enabled);
    }

    /// Get server capabilities based on enabled features
    pub async fn get_capabilities(&self) -> ServerCapabilities {
        let features = self.enabled_features.read().await;

        ServerCapabilities {
            experimental: None,
            logging: if *features.get("logging").unwrap_or(&false) {
                Some(serde_json::json!({}))
            } else {
                None
            },
            prompts: if *features.get("prompts").unwrap_or(&false) {
                Some(PromptsCapability {
                    list_changed: Some(true),
                })
            } else {
                None
            },
            resources: if *features.get("resources").unwrap_or(&false) {
                Some(ResourcesCapability {
                    subscribe: Some(true),
                    list_changed: Some(true),
                })
            } else {
                None
            },
            tools: if *features.get("tools").unwrap_or(&false) {
                Some(ToolsCapability {
                    list_changed: Some(true),
                })
            } else {
                None
            },
            completion: if *features.get("completion").unwrap_or(&false) {
                Some(serde_json::json!({}))
            } else {
                None
            },
        }
    }

    /// Initialize all feature managers
    pub async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    /// Shutdown all feature managers
    pub async fn shutdown(&self) -> Result<()> {
        // Cleanup resources, save state, etc.

        Ok(())
    }

    /// Get feature statistics
    pub async fn get_stats(&self) -> FeatureStats {
        let features = self.enabled_features.read().await;

        FeatureStats {
            enabled_features: features.clone(),
            resource_count: self.resources.get_resource_count().await,
            tool_count: self.tools.get_tool_count().await,
            prompt_count: self.prompts.get_prompt_count().await,
        }
    }
}

impl Default for ServerFeatureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Feature statistics
#[derive(Debug, Clone)]
pub struct FeatureStats {
    pub enabled_features: HashMap<String, bool>,
    pub resource_count: usize,
    pub tool_count: usize,
    pub prompt_count: usize,
}

/// Feature configuration
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// Feature name
    pub name: String,

    /// Whether the feature is enabled
    pub enabled: bool,

    /// Feature-specific configuration
    pub config: serde_json::Value,
}

impl FeatureConfig {
    /// Create a new feature configuration
    pub fn new(name: String, enabled: bool) -> Self {
        Self {
            name,
            enabled,
            config: serde_json::Value::Null,
        }
    }

    /// Create a new feature configuration with custom config
    pub fn with_config(name: String, enabled: bool, config: serde_json::Value) -> Self {
        Self {
            name,
            enabled,
            config,
        }
    }
}

/// Feature registry for managing available features
pub struct FeatureRegistry {
    features: HashMap<String, FeatureConfig>,
}

impl FeatureRegistry {
    /// Create a new feature registry
    pub fn new() -> Self {
        let mut features = HashMap::new();

        // Register default features
        features.insert(
            "resources".to_string(),
            FeatureConfig::new("resources".to_string(), true),
        );
        features.insert(
            "tools".to_string(),
            FeatureConfig::new("tools".to_string(), true),
        );
        features.insert(
            "prompts".to_string(),
            FeatureConfig::new("prompts".to_string(), true),
        );
        features.insert(
            "logging".to_string(),
            FeatureConfig::new("logging".to_string(), true),
        );
        features.insert(
            "completion".to_string(),
            FeatureConfig::new("completion".to_string(), true),
        );

        Self { features }
    }

    /// Register a feature
    pub fn register_feature(&mut self, config: FeatureConfig) {
        self.features.insert(config.name.clone(), config);
    }

    /// Get a feature configuration
    pub fn get_feature(&self, name: &str) -> Option<&FeatureConfig> {
        self.features.get(name)
    }

    /// Get all feature configurations
    pub fn get_all_features(&self) -> &HashMap<String, FeatureConfig> {
        &self.features
    }

    /// Check if a feature is registered
    pub fn has_feature(&self, name: &str) -> bool {
        self.features.contains_key(name)
    }

    /// Remove a feature
    pub fn remove_feature(&mut self, name: &str) -> Option<FeatureConfig> {
        self.features.remove(name)
    }
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_feature_manager() {
        let manager = ServerFeatureManager::new();

        // Test initial state
        assert!(manager.is_feature_enabled("resources").await);
        assert!(manager.is_feature_enabled("tools").await);
        assert!(manager.is_feature_enabled("prompts").await);

        // Test disabling a feature
        manager.set_feature_enabled("resources", false).await;
        assert!(!manager.is_feature_enabled("resources").await);

        // Test capabilities
        let capabilities = manager.get_capabilities().await;
        assert!(capabilities.resources.is_none());
        assert!(capabilities.tools.is_some());
        assert!(capabilities.prompts.is_some());
    }

    #[test]
    fn test_feature_registry() {
        let mut registry = FeatureRegistry::new();

        // Test default features
        assert!(registry.has_feature("resources"));
        assert!(registry.has_feature("tools"));
        assert!(registry.has_feature("prompts"));

        // Test custom feature
        let custom_config = FeatureConfig::new("custom".to_string(), true);
        registry.register_feature(custom_config);
        assert!(registry.has_feature("custom"));

        // Test removal
        let removed = registry.remove_feature("custom");
        assert!(removed.is_some());
        assert!(!registry.has_feature("custom"));
    }

    #[test]
    fn test_feature_config() {
        let config = FeatureConfig::new("test".to_string(), true);
        assert_eq!(config.name, "test");
        assert!(config.enabled);
        assert_eq!(config.config, serde_json::Value::Null);

        let config_with_data = FeatureConfig::with_config(
            "test2".to_string(),
            false,
            serde_json::json!({"key": "value"}),
        );
        assert_eq!(config_with_data.name, "test2");
        assert!(!config_with_data.enabled);
        assert_eq!(config_with_data.config["key"], "value");
    }
}
