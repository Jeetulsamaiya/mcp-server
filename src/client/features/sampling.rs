//! Sampling management for MCP client features.
//!
//! This module implements the sampling feature, allowing the server to request
//! LLM sampling from the client.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{McpError, Result};

/// Sampling manager for handling LLM sampling requests
pub struct SamplingManager {
    /// Sampling providers
    providers: Arc<RwLock<HashMap<String, Box<dyn SamplingProvider>>>>,

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Sampling provider trait for different LLM providers
#[async_trait::async_trait]
pub trait SamplingProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Check if the provider can handle the given model preferences
    fn can_handle(&self, preferences: &ModelPreferences) -> bool;

    /// Create a message using the LLM
    async fn create_message(&self, request: &CreateMessageRequest) -> Result<CreateMessageResult>;

    /// Get available models
    async fn get_available_models(&self) -> Result<Vec<ModelInfo>>;
}

/// Create message request
#[derive(Debug, Clone)]
pub struct CreateMessageRequest {
    /// Messages for the conversation
    pub messages: Vec<SamplingMessage>,

    /// Model preferences
    pub model_preferences: Option<ModelPreferences>,

    /// System prompt
    pub system_prompt: Option<String>,

    /// Context inclusion preference
    pub include_context: Option<ContextInclusion>,

    /// Temperature for sampling
    pub temperature: Option<f64>,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Stop sequences
    pub stop_sequences: Option<Vec<String>>,

    /// Provider-specific metadata
    pub metadata: Option<serde_json::Value>,
}

/// Create message result
#[derive(Debug, Clone)]
pub struct CreateMessageResult {
    /// Generated message
    pub message: SamplingMessage,

    /// Model used for generation
    pub model: String,

    /// Reason why sampling stopped
    pub stop_reason: Option<StopReason>,
}

/// Sampling message
#[derive(Debug, Clone)]
pub struct SamplingMessage {
    /// Message role
    pub role: Role,

    /// Message content
    pub content: Content,
}

/// Message role
#[derive(Debug, Clone)]
pub enum Role {
    User,
    Assistant,
}

/// Message content
#[derive(Debug, Clone)]
pub enum Content {
    Text {
        text: String,
        annotations: Option<Annotations>,
    },
    Image {
        data: String, // base64 encoded
        mime_type: String,
        annotations: Option<Annotations>,
    },
    Audio {
        data: String, // base64 encoded
        mime_type: String,
        annotations: Option<Annotations>,
    },
}

/// Content annotations
#[derive(Debug, Clone)]
pub struct Annotations {
    /// Target audience
    pub audience: Option<Vec<Role>>,

    /// Priority (0.0 to 1.0)
    pub priority: Option<f64>,
}

/// Model preferences
#[derive(Debug, Clone)]
pub struct ModelPreferences {
    /// Model hints
    pub hints: Option<Vec<ModelHint>>,

    /// Cost priority (0.0 to 1.0)
    pub cost_priority: Option<f64>,

    /// Speed priority (0.0 to 1.0)
    pub speed_priority: Option<f64>,

    /// Intelligence priority (0.0 to 1.0)
    pub intelligence_priority: Option<f64>,
}

/// Model hint
#[derive(Debug, Clone)]
pub struct ModelHint {
    /// Model name hint
    pub name: Option<String>,
}

/// Context inclusion preference
#[derive(Debug, Clone)]
pub enum ContextInclusion {
    None,
    ThisServer,
    AllServers,
}

/// Stop reason
#[derive(Debug, Clone)]
pub enum StopReason {
    EndTurn,
    StopSequence,
    MaxTokens,
    Other(String),
}

/// Model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model name
    pub name: String,

    /// Model description
    pub description: Option<String>,

    /// Maximum context length
    pub max_context_length: Option<u32>,

    /// Supported features
    pub features: Vec<String>,
}

impl SamplingManager {
    /// Create a new sampling manager
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Register a sampling provider
    pub async fn register_provider(&self, provider: Box<dyn SamplingProvider>) -> Result<()> {
        if !self.is_enabled().await {
            return Err(McpError::Resource(
                "Sampling feature is disabled".to_string(),
            ));
        }

        let name = provider.name().to_string();

        {
            let mut providers = self.providers.write().await;
            providers.insert(name.clone(), provider);
        }

        info!("Registered sampling provider: {}", name);
        Ok(())
    }

    /// Create a message using the best available provider
    pub async fn create_message(
        &self,
        request: CreateMessageRequest,
    ) -> Result<CreateMessageResult> {
        if !self.is_enabled().await {
            return Err(McpError::Resource(
                "Sampling feature is disabled".to_string(),
            ));
        }

        // Find the best provider based on preferences
        let providers = self.providers.read().await;

        if providers.is_empty() {
            return Err(McpError::Resource(
                "No sampling providers available".to_string(),
            ));
        }

        let provider = if let Some(preferences) = &request.model_preferences {
            providers
                .values()
                .find(|p| p.can_handle(preferences))
                .or_else(|| providers.values().next())
        } else {
            providers.values().next()
        };

        let provider = provider
            .ok_or_else(|| McpError::Resource("No suitable sampling provider found".to_string()))?;

        let result = provider.create_message(&request).await?;
        info!("Generated message using provider: {}", provider.name());

        Ok(result)
    }

    /// Get all available models from all providers
    pub async fn get_available_models(&self) -> Result<Vec<ModelInfo>> {
        let providers = self.providers.read().await;
        let mut all_models = Vec::new();

        for provider in providers.values() {
            match provider.get_available_models().await {
                Ok(models) => all_models.extend(models),
                Err(e) => warn!(
                    "Failed to get models from provider {}: {}",
                    provider.name(),
                    e
                ),
            }
        }

        Ok(all_models)
    }

    /// Check if sampling is enabled
    pub async fn is_enabled(&self) -> bool {
        let enabled = self.enabled.read().await;
        *enabled
    }

    /// Enable or disable sampling
    pub async fn set_enabled(&self, enabled: bool) {
        let mut current_enabled = self.enabled.write().await;
        *current_enabled = enabled;
    }
}

/// Mock sampling provider for testing
pub struct MockSamplingProvider {
    name: String,
    models: Vec<ModelInfo>,
}

impl MockSamplingProvider {
    /// Create a new mock sampling provider
    pub fn new(name: String) -> Self {
        let models = vec![
            ModelInfo {
                name: "mock-gpt-4".to_string(),
                description: Some("Mock GPT-4 model".to_string()),
                max_context_length: Some(8192),
                features: vec!["text".to_string(), "chat".to_string()],
            },
            ModelInfo {
                name: "mock-claude-3".to_string(),
                description: Some("Mock Claude 3 model".to_string()),
                max_context_length: Some(200000),
                features: vec!["text".to_string(), "chat".to_string(), "vision".to_string()],
            },
        ];

        Self { name, models }
    }
}

#[async_trait::async_trait]
impl SamplingProvider for MockSamplingProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn can_handle(&self, preferences: &ModelPreferences) -> bool {
        if let Some(hints) = &preferences.hints {
            for hint in hints {
                if let Some(name) = &hint.name {
                    if self.models.iter().any(|m| m.name.contains(name)) {
                        return true;
                    }
                }
            }
        }
        true // Can handle any request as a fallback
    }

    async fn create_message(&self, request: &CreateMessageRequest) -> Result<CreateMessageResult> {
        let last_user_message = request
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
            .ok_or_else(|| McpError::Resource("No user message found".to_string()))?;

        let response_text = match &last_user_message.content {
            Content::Text { text, .. } => {
                format!("Mock response to: {}", text)
            }
            _ => "Mock response to non-text content".to_string(),
        };

        let response_message = SamplingMessage {
            role: Role::Assistant,
            content: Content::Text {
                text: response_text,
                annotations: None,
            },
        };

        Ok(CreateMessageResult {
            message: response_message,
            model: "mock-gpt-4".to_string(),
            stop_reason: Some(StopReason::EndTurn),
        })
    }

    async fn get_available_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(self.models.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sampling_manager() {
        let manager = SamplingManager::new();

        // Register a mock provider
        let provider = Box::new(MockSamplingProvider::new("mock".to_string()));
        assert!(manager.register_provider(provider).await.is_ok());

        // Test message creation
        let request = CreateMessageRequest {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::Text {
                    text: "Hello, world!".to_string(),
                    annotations: None,
                },
            }],
            model_preferences: None,
            system_prompt: None,
            include_context: None,
            temperature: None,
            max_tokens: 100,
            stop_sequences: None,
            metadata: None,
        };

        let result = manager.create_message(request).await.unwrap();
        assert!(matches!(result.message.role, Role::Assistant));
        assert_eq!(result.model, "mock-gpt-4");
    }

    #[tokio::test]
    async fn test_mock_sampling_provider() {
        let provider = MockSamplingProvider::new("test".to_string());

        // Test model preferences
        let preferences = ModelPreferences {
            hints: Some(vec![ModelHint {
                name: Some("gpt".to_string()),
            }]),
            cost_priority: None,
            speed_priority: None,
            intelligence_priority: None,
        };

        assert!(provider.can_handle(&preferences));

        // Test available models
        let models = provider.get_available_models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.name == "mock-gpt-4"));
        assert!(models.iter().any(|m| m.name == "mock-claude-3"));
    }
}
