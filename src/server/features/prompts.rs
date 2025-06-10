//! Prompt management for MCP server.
//!
//! This module implements the prompt feature of MCP, allowing the server
//! to expose prompt templates to clients.

use handlebars::Handlebars;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::{McpError, Result};
use crate::protocol::{PaginationParams, PaginationResult, Prompt, PromptMessage};
use crate::server::features::FeatureManager;

/// Prompt manager for handling MCP prompts
pub struct PromptManager {
    /// Registered prompts
    prompts: Arc<RwLock<HashMap<String, Prompt>>>,

    /// Prompt generators
    generators: Arc<RwLock<HashMap<String, Box<dyn PromptGenerator>>>>,

    /// Template engine
    handlebars: Arc<Handlebars<'static>>,

    /// Whether the feature is enabled
    enabled: Arc<RwLock<bool>>,
}

/// Prompt generator trait for dynamic prompt creation
#[async_trait::async_trait]
pub trait PromptGenerator: Send + Sync {
    /// Get the generator name
    fn name(&self) -> &str;

    /// Generate prompt messages with given arguments
    async fn generate(&self, arguments: Option<HashMap<String, String>>) -> Result<PromptResult>;

    /// Validate prompt arguments (optional)
    async fn validate_arguments(&self, arguments: Option<&HashMap<String, String>>) -> Result<()> {
        let _ = arguments;
        Ok(())
    }
}

/// Prompt generation result
#[derive(Debug, Clone)]
pub struct PromptResult {
    /// Generated messages
    pub messages: Vec<PromptMessage>,

    /// Optional description
    pub description: Option<String>,
}

impl PromptManager {
    /// Create a new prompt manager
    pub fn new() -> Self {
        Self {
            prompts: Arc::new(RwLock::new(HashMap::new())),
            generators: Arc::new(RwLock::new(HashMap::new())),
            handlebars: Arc::new(Handlebars::new()),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Register a prompt
    pub async fn register_prompt(&self, prompt: Prompt) -> Result<()> {
        if !self.is_enabled() {
            return Err(McpError::Prompt("Prompt feature is disabled".to_string()));
        }

        let name = prompt.name.clone();

        {
            let mut prompts = self.prompts.write().await;
            prompts.insert(name.clone(), prompt);
        }

        info!("Registered prompt: {}", name);
        Ok(())
    }

    /// Unregister a prompt
    pub async fn unregister_prompt(&self, name: &str) -> Result<Option<Prompt>> {
        let mut prompts = self.prompts.write().await;
        let prompt = prompts.remove(name);

        if prompt.is_some() {
            info!("Unregistered prompt: {}", name);
        }

        Ok(prompt)
    }

    /// Get a prompt by name
    pub async fn get_prompt(&self, name: &str) -> Option<Prompt> {
        let prompts = self.prompts.read().await;
        prompts.get(name).cloned()
    }

    /// List all prompts with optional pagination
    pub async fn list_prompts(
        &self,
        pagination: Option<PaginationParams>,
    ) -> Result<(Vec<Prompt>, PaginationResult)> {
        if !self.is_enabled() {
            return Err(McpError::Prompt("Prompt feature is disabled".to_string()));
        }

        let prompts = self.prompts.read().await;
        let mut all_prompts: Vec<Prompt> = prompts.values().cloned().collect();

        // Sort by name for consistent ordering
        all_prompts.sort_by(|a, b| a.name.cmp(&b.name));

        // Apply pagination if provided
        let (prompts, pagination_result) = if let Some(params) = pagination {
            self.apply_pagination(all_prompts, params)?
        } else {
            (all_prompts, PaginationResult { next_cursor: None })
        };

        Ok((prompts, pagination_result))
    }

    /// Get a prompt with arguments applied
    pub async fn get_prompt_with_args(
        &self,
        name: &str,
        arguments: Option<HashMap<String, String>>,
    ) -> Result<PromptResult> {
        if !self.is_enabled() {
            return Err(McpError::Prompt("Prompt feature is disabled".to_string()));
        }

        // Check if we have a registered prompt
        let prompt = self
            .get_prompt(name)
            .await
            .ok_or_else(|| McpError::Prompt(format!("Prompt not found: {}", name)))?;

        // Find generator
        let generators = self.generators.read().await;
        if let Some(generator) = generators.get(name) {
            // Validate arguments
            generator.validate_arguments(arguments.as_ref()).await?;

            // Generate prompt
            let result = generator.generate(arguments).await?;
            info!(
                "Generated prompt: {} -> {} messages",
                name,
                result.messages.len()
            );
            return Ok(result);
        }

        Ok(PromptResult {
            messages: Vec::new(),
            description: prompt.description,
        })
    }

    /// Register a prompt generator
    pub async fn register_generator(&self, generator: Box<dyn PromptGenerator>) -> Result<()> {
        let name = generator.name().to_string();

        {
            let mut generators = self.generators.write().await;
            generators.insert(name.clone(), generator);
        }

        info!("Registered prompt generator: {}", name);
        Ok(())
    }

    /// Register a template with the template engine
    pub async fn register_template(&self, name: &str, template: &str) -> Result<()> {
        let mut handlebars = Handlebars::new();
        handlebars
            .register_template_string(name, template)
            .map_err(|e| McpError::Prompt(format!("Failed to register template: {}", e)))?;

        info!("Registered template: {}", name);
        Ok(())
    }

    /// Render a template with given data
    pub async fn render_template(&self, name: &str, data: &serde_json::Value) -> Result<String> {
        self.handlebars
            .render(name, data)
            .map_err(|e| McpError::Prompt(format!("Failed to render template: {}", e)))
    }

    /// Get prompt count
    pub async fn get_prompt_count(&self) -> usize {
        let prompts = self.prompts.read().await;
        prompts.len()
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

    /// Apply pagination to prompts
    fn apply_pagination(
        &self,
        mut prompts: Vec<Prompt>,
        params: PaginationParams,
    ) -> Result<(Vec<Prompt>, PaginationResult)> {
        let start_index = if let Some(cursor) = params.cursor {
            cursor.parse::<usize>().unwrap_or(0)
        } else {
            0
        };

        let page_size = 50; // Default page size
        let end_index = std::cmp::min(start_index + page_size, prompts.len());

        let page_prompts = if start_index < prompts.len() {
            prompts.drain(start_index..end_index).collect()
        } else {
            Vec::new()
        };

        let next_cursor = if end_index < prompts.len() {
            Some(end_index.to_string())
        } else {
            None
        };

        Ok((page_prompts, PaginationResult { next_cursor }))
    }
}

impl FeatureManager for PromptManager {
    fn name(&self) -> &'static str {
        "prompts"
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

impl PromptResult {
    /// Create a new prompt result
    pub fn new(messages: Vec<PromptMessage>) -> Self {
        Self {
            messages,
            description: None,
        }
    }

    /// Create a new prompt result with description
    pub fn with_description(messages: Vec<PromptMessage>, description: String) -> Self {
        Self {
            messages,
            description: Some(description),
        }
    }
}

/// Example greeting prompt generator
pub struct GreetingPromptGenerator;

#[async_trait::async_trait]
impl PromptGenerator for GreetingPromptGenerator {
    fn name(&self) -> &str {
        "greeting"
    }

    async fn generate(&self, arguments: Option<HashMap<String, String>>) -> Result<PromptResult> {
        let name = arguments
            .as_ref()
            .and_then(|args| args.get("name"))
            .map(|s| s.as_str())
            .unwrap_or("World");

        let time_of_day = arguments
            .as_ref()
            .and_then(|args| args.get("time_of_day"))
            .map(|s| s.as_str())
            .unwrap_or("day");

        let greeting = match time_of_day {
            "morning" => "Good morning",
            "afternoon" => "Good afternoon",
            "evening" => "Good evening",
            "night" => "Good night",
            _ => "Hello",
        };

        let message = PromptMessage {
            role: crate::protocol::Role::User,
            content: crate::protocol::Content::Text {
                text: format!("{}, {}! How can I help you today?", greeting, name),
                annotations: None,
            },
        };

        Ok(PromptResult::with_description(
            vec![message],
            format!("A {} greeting for {}", time_of_day, name),
        ))
    }

    async fn validate_arguments(&self, arguments: Option<&HashMap<String, String>>) -> Result<()> {
        if let Some(args) = arguments {
            if let Some(time_of_day) = args.get("time_of_day") {
                let valid_times = ["morning", "afternoon", "evening", "night", "day"];
                if !valid_times.contains(&time_of_day.as_str()) {
                    return Err(McpError::invalid_params(format!(
                        "Invalid time_of_day: {}. Valid values are: {}",
                        time_of_day,
                        valid_times.join(", ")
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Example code review prompt generator
pub struct CodeReviewPromptGenerator;

#[async_trait::async_trait]
impl PromptGenerator for CodeReviewPromptGenerator {
    fn name(&self) -> &str {
        "code_review"
    }

    async fn generate(&self, arguments: Option<HashMap<String, String>>) -> Result<PromptResult> {
        let code = arguments
            .as_ref()
            .and_then(|args| args.get("code"))
            .ok_or_else(|| McpError::invalid_params("Code parameter is required"))?;

        let language = arguments
            .as_ref()
            .and_then(|args| args.get("language"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let focus = arguments
            .as_ref()
            .and_then(|args| args.get("focus"))
            .map(|s| s.as_str())
            .unwrap_or("general");

        let system_message = PromptMessage {
            role: crate::protocol::Role::Assistant,
            content: crate::protocol::Content::Text {
                text: format!(
                    "You are an expert code reviewer. Please review the following {} code with a focus on {}. \
                     Provide constructive feedback on code quality, potential issues, and suggestions for improvement.",
                    language, focus
                ),
                annotations: None,
            },
        };

        let user_message = PromptMessage {
            role: crate::protocol::Role::User,
            content: crate::protocol::Content::Text {
                text: format!(
                    "Please review this {} code:\n\n```{}\n{}\n```",
                    language, language, code
                ),
                annotations: None,
            },
        };

        Ok(PromptResult::with_description(
            vec![system_message, user_message],
            format!(
                "Code review prompt for {} code focusing on {}",
                language, focus
            ),
        ))
    }

    async fn validate_arguments(&self, arguments: Option<&HashMap<String, String>>) -> Result<()> {
        let args =
            arguments.ok_or_else(|| McpError::invalid_params("Code review requires arguments"))?;

        if !args.contains_key("code") {
            return Err(McpError::invalid_params("Code parameter is required"));
        }

        if let Some(focus) = args.get("focus") {
            let valid_focus = ["general", "security", "performance", "style", "bugs"];
            if !valid_focus.contains(&focus.as_str()) {
                return Err(McpError::invalid_params(format!(
                    "Invalid focus: {}. Valid values are: {}",
                    focus,
                    valid_focus.join(", ")
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PromptArgument;

    #[tokio::test]
    async fn test_prompt_manager() {
        let manager = PromptManager::new();

        let prompt = Prompt {
            name: "test-prompt".to_string(),
            description: Some("A test prompt".to_string()),
            arguments: Some(vec![PromptArgument {
                name: "name".to_string(),
                description: Some("The name to greet".to_string()),
                required: Some(false),
            }]),
        };

        // Test registration
        assert!(manager.register_prompt(prompt.clone()).await.is_ok());

        // Test retrieval
        let retrieved = manager.get_prompt("test-prompt").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-prompt");

        // Test listing
        let (prompts, _) = manager.list_prompts(None).await.unwrap();
        assert_eq!(prompts.len(), 1);

        // Test unregistration
        let removed = manager.unregister_prompt("test-prompt").await.unwrap();
        assert!(removed.is_some());

        let not_found = manager.get_prompt("test-prompt").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_greeting_generator() {
        let generator = GreetingPromptGenerator;

        // Test with arguments
        let mut args = HashMap::new();
        args.insert("name".to_string(), "Alice".to_string());
        args.insert("time_of_day".to_string(), "morning".to_string());

        let result = generator.generate(Some(args)).await.unwrap();
        assert_eq!(result.messages.len(), 1);
        assert!(result.description.is_some());

        // Test without arguments
        let result = generator.generate(None).await.unwrap();
        assert_eq!(result.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_code_review_generator() {
        let generator = CodeReviewPromptGenerator;

        let mut args = HashMap::new();
        args.insert(
            "code".to_string(),
            "function hello() { console.log('Hello'); }".to_string(),
        );
        args.insert("language".to_string(), "javascript".to_string());
        args.insert("focus".to_string(), "style".to_string());

        let result = generator.generate(Some(args)).await.unwrap();
        assert_eq!(result.messages.len(), 2);
        assert!(result.description.is_some());

        // Test validation
        let mut invalid_args = HashMap::new();
        invalid_args.insert("language".to_string(), "javascript".to_string());

        let validation_result = generator.validate_arguments(Some(&invalid_args)).await;
        assert!(validation_result.is_err());
    }
}
