//! Protocol message handler for MCP server.
//!
//! This module provides the main protocol handler that processes incoming
//! JSON-RPC messages and routes them to appropriate handlers.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::client::features::SamplingManager;
use crate::error::{McpError, Result};
use crate::protocol::{
    validation, AnyJsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RequestId,
};
use crate::server::features::{PromptManager, ResourceManager, ToolManager};

/// Protocol handler for processing MCP messages
#[derive(Clone)]
pub struct ProtocolHandler {
    /// Resource manager
    resource_manager: Arc<ResourceManager>,

    /// Tool manager
    tool_manager: Arc<ToolManager>,

    /// Prompt manager
    prompt_manager: Arc<PromptManager>,

    /// Sampling manager
    sampling_manager: Arc<SamplingManager>,

    /// Active requests tracking
    active_requests: Arc<RwLock<HashMap<RequestId, tokio::time::Instant>>>,

    /// Server initialized flag
    initialized: Arc<RwLock<bool>>,
}

impl ProtocolHandler {
    /// Create a new protocol handler
    pub fn new(
        resource_manager: Arc<ResourceManager>,
        tool_manager: Arc<ToolManager>,
        prompt_manager: Arc<PromptManager>,
        sampling_manager: Arc<SamplingManager>,
    ) -> Self {
        let handler = Self {
            resource_manager,
            tool_manager,
            prompt_manager,
            sampling_manager,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            initialized: Arc::new(RwLock::new(false)),
        };

        // Initialize with some default resources, tools, and prompts for testing
        tokio::spawn({
            let handler = handler.clone();
            async move {
                if let Err(e) = handler.setup_defaults().await {
                    error!("Failed to setup default resources: {}", e);
                }
            }
        });

        handler
    }

    /// Setup default resources, tools, and prompts for testing
    async fn setup_defaults(&self) -> Result<()> {
        // Add a simple text resource
        let text_resource = crate::protocol::Resource {
            uri: "text://hello".to_string(),
            name: "Hello World".to_string(),
            description: Some("A simple hello world text resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            annotations: None,
            size: Some(13),
        };

        if let Err(e) = self.resource_manager.register_resource(text_resource).await {
            warn!("Failed to register default text resource: {}", e);
        }

        // Register a simple text resource provider
        let text_provider = Box::new(SimpleTextProvider::new());
        if let Err(e) = self.resource_manager.register_provider(text_provider).await {
            warn!("Failed to register text resource provider: {}", e);
        }

        // Add a simple echo tool
        let echo_tool = crate::protocol::Tool {
            name: "echo".to_string(),
            description: Some("Echo back the input text".to_string()),
            input_schema: crate::protocol::ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Some({
                    let mut props = std::collections::HashMap::new();
                    props.insert(
                        "name".to_string(),
                        serde_json::json!({
                            "type": "string",
                            "description": "Text to echo back"
                        }),
                    );
                    props
                }),
                required: Some(vec!["text".to_string()]),
            },
            annotations: None,
        };

        if let Err(e) = self.tool_manager.register_tool(echo_tool).await {
            warn!("Failed to register default echo tool: {}", e);
        }

        // Register the echo tool handler
        let echo_handler = Box::new(crate::server::features::tools::EchoToolHandler);
        if let Err(e) = self.tool_manager.register_handler(echo_handler).await {
            warn!("Failed to register echo tool handler: {}", e);
        }

        // Add a simple greeting prompt
        let greeting_prompt = crate::protocol::Prompt {
            name: "greeting".to_string(),
            description: Some("Generate a greeting message".to_string()),
            arguments: Some(vec![crate::protocol::PromptArgument {
                name: "name".to_string(),
                description: Some("Name to greet".to_string()),
                required: Some(false),
            }]),
        };

        if let Err(e) = self.prompt_manager.register_prompt(greeting_prompt).await {
            warn!("Failed to register default greeting prompt: {}", e);
        }

        // Register the greeting prompt generator
        let greeting_generator =
            Box::new(crate::server::features::prompts::GreetingPromptGenerator);
        if let Err(e) = self
            .prompt_manager
            .register_generator(greeting_generator)
            .await
        {
            warn!("Failed to register greeting prompt generator: {}", e);
        }

        info!("Default resources, tools, and prompts setup completed");
        Ok(())
    }

    /// Handle an incoming message
    pub async fn handle_message(
        &self,
        message: AnyJsonRpcMessage,
    ) -> Result<Option<AnyJsonRpcMessage>> {
        match message {
            AnyJsonRpcMessage::Request(request) => {
                let response = self.handle_request(request).await?;
                Ok(Some(AnyJsonRpcMessage::Response(response)))
            }
            AnyJsonRpcMessage::Notification(notification) => {
                self.handle_notification(notification).await?;
                Ok(None)
            }
            AnyJsonRpcMessage::Response(response) => {
                self.handle_response(response).await?;
                Ok(None)
            }
            AnyJsonRpcMessage::Batch(batch) => self.handle_batch(batch).await,
        }
    }

    /// Handle a JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        info!(
            "Handling request: {} (id: {:?})",
            request.method, request.id
        );

        // Validate the request
        validation::validate_request(&request)?;
        validation::validate_method_name(&request.method)?;

        // Track the request
        {
            let mut active = self.active_requests.write().await;
            active.insert(request.id.clone(), tokio::time::Instant::now());
        }

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request).await,
            "ping" => self.handle_ping(&request).await,

            // Resource methods
            "resources/list" => self.handle_resources_list(&request).await,
            "resources/templates/list" => self.handle_resource_templates_list(&request).await,
            "resources/read" => self.handle_resources_read(&request).await,
            "resources/subscribe" => self.handle_resources_subscribe(&request).await,
            "resources/unsubscribe" => self.handle_resources_unsubscribe(&request).await,

            // Tool methods
            "tools/list" => self.handle_tools_list(&request).await,
            "tools/call" => self.handle_tools_call(&request).await,

            // Prompt methods
            "prompts/list" => self.handle_prompts_list(&request).await,
            "prompts/get" => self.handle_prompts_get(&request).await,

            // Sampling methods
            "sampling/createMessage" => self.handle_sampling_create_message(&request).await,

            // Logging methods
            "logging/setLevel" => self.handle_logging_set_level(&request).await,

            // Completion methods
            "completion/complete" => self.handle_completion_complete(&request).await,

            // Roots methods
            "roots/list" => self.handle_roots_list(&request).await,

            _ => Err(McpError::method_not_found(&request.method)),
        };

        // Remove from active requests
        {
            let mut active = self.active_requests.write().await;
            active.remove(&request.id);
        }

        match result {
            Ok(result) => Ok(JsonRpcResponse::success(request.id, result)),
            Err(error) => {
                error!("Request {} failed: {}", request.method, error);
                Ok(JsonRpcResponse::error(request.id, error.into()))
            }
        }
    }

    /// Handle a JSON-RPC notification
    async fn handle_notification(&self, notification: JsonRpcNotification) -> Result<()> {
        info!("Handling notification: {}", notification.method);

        // Validate the notification
        validation::validate_notification(&notification)?;
        validation::validate_method_name(&notification.method)?;

        match notification.method.as_str() {
            "notifications/initialized" => {
                self.handle_initialized_notification(&notification).await
            }
            "notifications/cancelled" => self.handle_cancelled_notification(&notification).await,
            "notifications/progress" => self.handle_progress_notification(&notification).await,
            "notifications/resources/list_changed" => {
                self.handle_resource_list_changed_notification(&notification)
                    .await
            }
            "notifications/resources/updated" => {
                self.handle_resource_updated_notification(&notification)
                    .await
            }
            "notifications/tools/list_changed" => {
                self.handle_tool_list_changed_notification(&notification)
                    .await
            }
            "notifications/prompts/list_changed" => {
                self.handle_prompt_list_changed_notification(&notification)
                    .await
            }
            "notifications/roots/list_changed" => {
                self.handle_roots_list_changed_notification(&notification)
                    .await
            }
            "notifications/message" => self.handle_message_notification(&notification).await,
            _ => {
                warn!("Unknown notification method: {}", notification.method);
                Ok(())
            }
        }
    }

    /// Handle a JSON-RPC response
    async fn handle_response(&self, response: JsonRpcResponse) -> Result<()> {
        info!("Handling response for request: {:?}", response.id);

        // Validate the response
        validation::validate_response(&response)?;

        // Check if this was an active request
        let was_active = {
            let active = self.active_requests.read().await;
            active.contains_key(&response.id)
        };

        if !was_active {
            warn!("Received response for unknown request: {:?}", response.id);
        }

        if let Some(error) = &response.error {
            error!(
                "Request {:?} failed with error: {}",
                response.id, error.message
            );
        } else {
            info!("Request {:?} completed successfully", response.id);
        }

        Ok(())
    }

    /// Handle a batch of messages
    async fn handle_batch(&self, batch: Vec<Value>) -> Result<Option<AnyJsonRpcMessage>> {
        info!("Handling batch of {} messages", batch.len());

        if batch.is_empty() {
            return Err(McpError::invalid_request("Batch cannot be empty"));
        }

        let mut responses = Vec::new();

        for item in batch {
            let message: AnyJsonRpcMessage =
                serde_json::from_value(item).map_err(|e| McpError::parse_error(e.to_string()))?;

            if let Some(response) = Box::pin(self.handle_message(message)).await? {
                if let AnyJsonRpcMessage::Response(resp) = response {
                    responses.push(serde_json::to_value(resp)?);
                }
            }
        }

        if responses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(AnyJsonRpcMessage::Batch(responses)))
        }
    }

    /// Check if the server is initialized
    async fn check_initialized(&self) -> Result<()> {
        let initialized = *self.initialized.read().await;
        if !initialized {
            return Err(McpError::Protocol("Server not initialized".to_string()));
        }
        Ok(())
    }

    async fn handle_initialize(&self, request: &JsonRpcRequest) -> Result<Value> {
        info!("Handling initialize request");

        // Parse initialize request parameters
        let params = request
            .params
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("Initialize request requires parameters"))?;

        let init_request: crate::protocol::InitializeRequest =
            serde_json::from_value(params.clone()).map_err(|e| {
                McpError::invalid_params(format!("Invalid initialize parameters: {}", e))
            })?;

        // Validate protocol version
        if init_request.protocol_version != crate::protocol::PROTOCOL_VERSION {
            warn!(
                "Client requested protocol version {}, server supports {}",
                init_request.protocol_version,
                crate::protocol::PROTOCOL_VERSION
            );
        }

        // Build server capabilities based on available features
        let mut server_capabilities = crate::protocol::ServerCapabilities {
            experimental: None,
            logging: Some(serde_json::json!({})),
            prompts: None,
            resources: None,
            tools: None,
            completion: None,
        };

        // Check if prompt manager is enabled and add capability
        if self.prompt_manager.is_enabled() {
            server_capabilities.prompts = Some(crate::protocol::PromptsCapability {
                list_changed: Some(true),
            });
        }

        // Check if resource manager is enabled and add capability
        if self.resource_manager.is_enabled() {
            server_capabilities.resources = Some(crate::protocol::ResourcesCapability {
                subscribe: Some(true),
                list_changed: Some(true),
            });
        }

        // Check if tool manager is enabled and add capability
        if self.tool_manager.is_enabled() {
            server_capabilities.tools = Some(crate::protocol::ToolsCapability {
                list_changed: Some(true),
            });
        }

        // Create initialize result
        let init_result = crate::protocol::InitializeResult {
            protocol_version: crate::protocol::PROTOCOL_VERSION.to_string(),
            capabilities: server_capabilities,
            server_info: crate::protocol::Implementation {
                name: "mcp-server-rust".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(
                "A Model Context Protocol server implementation in Rust".to_string(),
            ),
        };

        // Mark as initialized
        {
            let mut initialized = self.initialized.write().await;
            *initialized = true;
        }

        info!("Initialize successful, capabilities negotiated, server marked as initialized");
        Ok(serde_json::to_value(init_result)?)
    }

    async fn handle_ping(&self, _request: &JsonRpcRequest) -> Result<Value> {
        Ok(serde_json::json!({}))
    }

    async fn handle_resources_list(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling resources/list request");

        // Parse pagination parameters if provided
        let pagination = if let Some(params) = &request.params {
            if let Ok(pagination_params) =
                serde_json::from_value::<crate::protocol::PaginationParams>(params.clone())
            {
                Some(pagination_params)
            } else {
                None
            }
        } else {
            None
        };

        // Get resources from resource manager
        let (resources, pagination_result) =
            self.resource_manager.list_resources(pagination).await?;

        // Build response
        let mut response = serde_json::json!({
            "resources": resources
        });

        // Add pagination info if present
        if let Some(next_cursor) = pagination_result.next_cursor {
            response["nextCursor"] = serde_json::Value::String(next_cursor);
        }

        info!("Returning {} resources", resources.len());
        Ok(response)
    }

    async fn handle_resource_templates_list(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling resources/templates/list request");

        // Parse pagination parameters if provided
        let pagination = if let Some(params) = &request.params {
            if let Ok(pagination_params) =
                serde_json::from_value::<crate::protocol::PaginationParams>(params.clone())
            {
                Some(pagination_params)
            } else {
                None
            }
        } else {
            None
        };

        // Get resource templates from resource manager
        let (templates, pagination_result) =
            self.resource_manager.list_templates(pagination).await?;

        // Build response
        let mut response = serde_json::json!({
            "resourceTemplates": templates
        });

        // Add pagination info if present
        if let Some(next_cursor) = pagination_result.next_cursor {
            response["nextCursor"] = serde_json::Value::String(next_cursor);
        }

        info!("Returning {} resource templates", templates.len());
        Ok(response)
    }

    async fn handle_resources_read(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling resources/read request");

        // Parse request parameters
        let params = request.params.as_ref().ok_or_else(|| {
            McpError::invalid_params("resources/read request requires parameters")
        })?;

        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing or invalid 'uri' parameter"))?;

        info!("Reading resource: {}", uri);

        // Read resource contents from resource manager
        let contents = self.resource_manager.read_resource(uri).await?;

        // Build response
        let response = serde_json::json!({
            "contents": contents
        });

        info!("Successfully read resource: {}", uri);
        Ok(response)
    }

    async fn handle_resources_subscribe(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling resources/subscribe request");

        // Parse request parameters
        let params = request.params.as_ref().ok_or_else(|| {
            McpError::invalid_params("resources/subscribe request requires parameters")
        })?;

        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing or invalid 'uri' parameter"))?;

        let client_id = "default-client";

        info!("Subscribing to resource: {}", uri);

        // Subscribe through resource manager
        self.resource_manager.subscribe(uri, client_id).await?;

        // Build response
        let response = serde_json::json!({});

        info!("Successfully subscribed to resource: {}", uri);
        Ok(response)
    }

    async fn handle_resources_unsubscribe(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling resources/unsubscribe request");

        // Parse request parameters
        let params = request.params.as_ref().ok_or_else(|| {
            McpError::invalid_params("resources/unsubscribe request requires parameters")
        })?;

        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing or invalid 'uri' parameter"))?;

        let client_id = "default-client";

        info!("Unsubscribing from resource: {}", uri);

        // Unsubscribe through resource manager
        self.resource_manager.unsubscribe(uri, client_id).await?;

        // Build response
        let response = serde_json::json!({});

        info!("Successfully unsubscribed from resource: {}", uri);
        Ok(response)
    }

    async fn handle_tools_list(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling tools/list request");

        // Parse pagination parameters if provided
        let pagination = if let Some(params) = &request.params {
            if let Ok(pagination_params) =
                serde_json::from_value::<crate::protocol::PaginationParams>(params.clone())
            {
                Some(pagination_params)
            } else {
                None
            }
        } else {
            None
        };

        // Get tools from tool manager
        let (tools, pagination_result) = self.tool_manager.list_tools(pagination).await?;

        // Build response
        let mut response = serde_json::json!({
            "tools": tools
        });

        // Add pagination info if present
        if let Some(next_cursor) = pagination_result.next_cursor {
            response["nextCursor"] = serde_json::Value::String(next_cursor);
        }

        info!("Returning {} tools", tools.len());
        Ok(response)
    }

    async fn handle_tools_call(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling tools/call request");

        // info the request
        info!("Request: {:?}", request);

        // Parse request parameters
        let params = request
            .params
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("tools/call request requires parameters"))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing or invalid 'name' parameter"))?;

        let arguments = params.get("arguments").cloned();

        info!("Calling tool: {} with arguments: {:?}", name, arguments);

        // Call tool through tool manager
        let result = self.tool_manager.call_tool(name, arguments).await?;

        // Build response
        let response = serde_json::json!({
            "content": result.content,
            "isError": result.is_error
        });

        info!("Tool call completed: {}", name);
        Ok(response)
    }

    async fn handle_prompts_list(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling prompts/list request");

        // Parse pagination parameters if provided
        let pagination = if let Some(params) = &request.params {
            if let Ok(pagination_params) =
                serde_json::from_value::<crate::protocol::PaginationParams>(params.clone())
            {
                Some(pagination_params)
            } else {
                None
            }
        } else {
            None
        };

        // Get prompts from prompt manager
        let (prompts, pagination_result) = self.prompt_manager.list_prompts(pagination).await?;

        // Build response
        let mut response = serde_json::json!({
            "prompts": prompts
        });

        // Add pagination info if present
        if let Some(next_cursor) = pagination_result.next_cursor {
            response["nextCursor"] = serde_json::Value::String(next_cursor);
        }

        info!("Returning {} prompts", prompts.len());
        Ok(response)
    }

    async fn handle_prompts_get(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling prompts/get request");

        // Parse request parameters
        let params = request
            .params
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("prompts/get request requires parameters"))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing or invalid 'name' parameter"))?;

        let arguments = params
            .get("arguments")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<std::collections::HashMap<String, String>>()
            });

        info!("Getting prompt: {} with arguments: {:?}", name, arguments);

        // Get prompt result from prompt manager
        let result = self
            .prompt_manager
            .get_prompt_with_args(name, arguments)
            .await?;

        // Build response
        let response = serde_json::json!({
            "description": result.description,
            "messages": result.messages
        });

        info!("Successfully retrieved prompt: {}", name);
        Ok(response)
    }

    async fn handle_sampling_create_message(&self, _request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling sampling/createMessage request");

        let response = serde_json::json!({
            "role": "assistant",
            "content": {
                "type": "text",
                "text": "This is a placeholder response from the MCP server."
            }
        });

        Ok(response)
    }

    async fn handle_logging_set_level(&self, request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling logging/setLevel request");

        // Parse request parameters
        let params = request.params.as_ref().ok_or_else(|| {
            McpError::invalid_params("logging/setLevel request requires parameters")
        })?;

        let level = params
            .get("level")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("Missing or invalid 'level' parameter"))?;

        info!("Setting log level to: {}", level);

        let response = serde_json::json!({});

        Ok(response)
    }

    async fn handle_completion_complete(&self, _request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling completion/complete request");

        let response = serde_json::json!({
            "completion": {
                "values": [],
                "total": 0,
                "hasMore": false
            }
        });

        Ok(response)
    }

    async fn handle_roots_list(&self, _request: &JsonRpcRequest) -> Result<Value> {
        self.check_initialized().await?;
        info!("Handling roots/list request");

        let response = serde_json::json!({
            "roots": []
        });

        info!("Returning empty roots list");
        Ok(response)
    }

    // Notification handlers
    async fn handle_initialized_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        let mut initialized = self.initialized.write().await;
        *initialized = true;
        info!("Server marked as initialized");
        Ok(())
    }

    async fn handle_cancelled_notification(
        &self,
        notification: &JsonRpcNotification,
    ) -> Result<()> {
        // Handle request cancellation
        if let Some(params) = &notification.params {
            if let Some(request_id) = params.get("requestId") {
                let mut active = self.active_requests.write().await;
                active.remove(request_id);
                info!("Request {:?} cancelled", request_id);
            }
        }
        Ok(())
    }

    async fn handle_progress_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        // Handle progress updates
        info!("Progress notification received");
        Ok(())
    }

    async fn handle_resource_list_changed_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        info!("Resource list changed notification received");
        Ok(())
    }

    async fn handle_resource_updated_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        info!("Resource updated notification received");
        Ok(())
    }

    async fn handle_tool_list_changed_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        info!("Tool list changed notification received");
        Ok(())
    }

    async fn handle_prompt_list_changed_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        info!("Prompt list changed notification received");
        Ok(())
    }

    async fn handle_roots_list_changed_notification(
        &self,
        _notification: &JsonRpcNotification,
    ) -> Result<()> {
        info!("Roots list changed notification received");
        Ok(())
    }

    async fn handle_message_notification(&self, _notification: &JsonRpcNotification) -> Result<()> {
        info!("Message notification received");
        Ok(())
    }
}

/// Simple text resource provider for testing
pub struct SimpleTextProvider;

impl SimpleTextProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl crate::server::features::resources::ResourceProvider for SimpleTextProvider {
    fn name(&self) -> &str {
        "simple-text"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("text://")
    }

    async fn read_resource(
        &self,
        uri: &str,
    ) -> crate::error::Result<Vec<crate::protocol::ResourceContents>> {
        match uri {
            "text://hello" => Ok(vec![crate::protocol::ResourceContents::Text {
                uri: uri.to_string(),
                mime_type: Some("text/plain".to_string()),
                text: "Hello, World! This is a simple text resource from the MCP server."
                    .to_string(),
            }]),
            _ => Err(crate::error::McpError::Resource(format!(
                "Unknown text resource: {}",
                uri
            ))),
        }
    }

    async fn list_resources(
        &self,
        _pattern: Option<&str>,
    ) -> crate::error::Result<Vec<crate::protocol::Resource>> {
        Ok(vec![crate::protocol::Resource {
            uri: "text://hello".to_string(),
            name: "Hello World".to_string(),
            description: Some("A simple hello world text resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            annotations: None,
            size: Some(65),
        }])
    }

    async fn subscribe(&self, _uri: &str) -> crate::error::Result<()> {
        // No-op for simple text provider
        Ok(())
    }

    async fn unsubscribe(&self, _uri: &str) -> crate::error::Result<()> {
        // No-op for simple text provider
        Ok(())
    }
}
