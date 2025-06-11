//! HTTP transport implementation for MCP server.
//!
//! This module implements the Streamable HTTP transport as defined in the MCP specification,
//! supporting HTTP POST requests with optional SSE streaming for responses and GET requests
//! for server-initiated SSE streams.

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Result as ActixResult};
use async_trait::async_trait;
use futures_util;
use serde_json;

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::HttpConfig;
use crate::error::Result;
use crate::protocol::parse_message;
use crate::transport::session::{Session, SessionManager};
use crate::transport::{Transport, TransportInfo, TransportMessage, TransportType};

use std::sync::OnceLock;

// Global protocol handler instance
static GLOBAL_PROTOCOL_HANDLER: OnceLock<Arc<crate::protocol::handler::ProtocolHandler>> =
    OnceLock::new();

/// Initialize the global protocol handler
pub fn init_global_protocol_handler() -> Arc<crate::protocol::handler::ProtocolHandler> {
    GLOBAL_PROTOCOL_HANDLER
        .get_or_init(|| {
            let resource_manager =
                Arc::new(crate::server::features::resources::ResourceManager::new());
            let tool_manager = Arc::new(crate::server::features::tools::ToolManager::new());
            let prompt_manager = Arc::new(crate::server::features::prompts::PromptManager::new());
            let sampling_manager =
                Arc::new(crate::client::features::sampling::SamplingManager::new());

            Arc::new(crate::protocol::handler::ProtocolHandler::new(
                resource_manager,
                tool_manager,
                prompt_manager,
                sampling_manager,
            ))
        })
        .clone()
}

/// Get the global protocol handler
pub fn get_global_protocol_handler() -> Option<Arc<crate::protocol::handler::ProtocolHandler>> {
    GLOBAL_PROTOCOL_HANDLER.get().cloned()
}

/// HTTP transport implementation
pub struct HttpTransport {
    config: HttpConfig,
    session_manager: Arc<SessionManager>,
    message_sender: Arc<RwLock<Option<mpsc::Sender<TransportMessage>>>>,
    shutdown_sender: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    session_manager: Arc<SessionManager>,
    message_sender: Arc<RwLock<Option<mpsc::Sender<TransportMessage>>>>,
    config: HttpConfig,
    protocol_handler: Arc<crate::protocol::handler::ProtocolHandler>,
}

impl HttpTransport {
    /// Create a new HTTP transport
    pub fn new(config: HttpConfig) -> Result<Self> {
        let session_manager = Arc::new(SessionManager::new(std::time::Duration::from_secs(
            config.session_timeout,
        )));

        Ok(Self {
            config,
            session_manager,
            message_sender: Arc::new(RwLock::new(None)),
            shutdown_sender: Arc::new(RwLock::new(None)),
        })
    }

    /// Create the Actix Web application
    fn create_app(
        state: AppState,
    ) -> App<
        impl actix_web::dev::ServiceFactory<
            actix_web::dev::ServiceRequest,
            Config = (),
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
            InitError = (),
        >,
    > {
        let app = App::new().app_data(web::Data::new(state.clone())).service(
            web::resource(&state.config.endpoint_path)
                .route(web::post().to(handle_streamable_http_post))
                .route(web::get().to(handle_streamable_http_get))
                .route(web::delete().to(handle_delete_request)),
        );

        app
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn start(
        &self,
    ) -> Result<(
        mpsc::Receiver<TransportMessage>,
        mpsc::Sender<TransportMessage>,
    )> {
        let (message_tx, message_rx) = mpsc::channel(1000);
        let (response_tx, response_rx) = mpsc::channel(1000);

        // Store the message sender
        {
            let mut sender = self.message_sender.write().await;
            *sender = Some(message_tx.clone());
        }

        let state = AppState {
            session_manager: self.session_manager.clone(),
            message_sender: self.message_sender.clone(),
            config: self.config.clone(),
            protocol_handler: init_global_protocol_handler(),
        };

        let bind_addr = format!("{}:{}", self.config.bind_address, self.config.port);

        info!(
            "Starting HTTP transport on {}{}",
            bind_addr, self.config.endpoint_path
        );

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Store the shutdown sender
        {
            let mut sender = self.shutdown_sender.write().await;
            *sender = Some(shutdown_tx);
        }

        // Clone the bind address for the spawned task
        let bind_addr_clone = bind_addr.clone();

        // Start the server in a separate task to avoid Send issues
        tokio::spawn(async move {
            let server = match HttpServer::new(move || Self::create_app(state.clone()))
                .bind(&bind_addr_clone)
            {
                Ok(server) => server,
                Err(e) => {
                    error!("Failed to bind to {}: {}", bind_addr_clone, e);
                    return;
                }
            };

            let server_handle = server.run();
            tokio::select! {
                result = server_handle => {
                    if let Err(e) = result {
                        error!("HTTP server error: {}", e);
                    }
                }
                _ = shutdown_rx => {
                    info!("HTTP server shutdown signal received");
                }
            }
        });

        Ok((message_rx, response_tx))
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping HTTP transport");

        // Send shutdown signal
        let mut shutdown_sender = self.shutdown_sender.write().await;
        if let Some(sender) = shutdown_sender.take() {
            if let Err(_) = sender.send(()) {
                warn!("Failed to send shutdown signal to HTTP server (receiver may have been dropped)");
            }
        }

        // Clear message sender
        {
            let mut message_sender = self.message_sender.write().await;
            *message_sender = None;
        }

        info!("HTTP transport stopped");
        Ok(())
    }

    fn info(&self) -> TransportInfo {
        TransportInfo {
            transport_type: TransportType::Http,
            address: format!(
                "{}:{}{}",
                self.config.bind_address, self.config.port, self.config.endpoint_path
            ),
            secure: self.config.enable_tls,
            max_message_size: Some(1024 * 1024), // 1MB default
        }
    }
}

/// Handle Streamable HTTP POST requests
/// Supports both single JSON responses and SSE streaming based on request content
async fn handle_streamable_http_post(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    info!("Handling Streamable HTTP POST request");

    // Validate Origin header for security
    if let Some(origin) = req.headers().get("Origin") {
        if let Ok(origin_str) = origin.to_str() {
            if !is_origin_allowed(origin_str, &state.config.cors_origins) {
                warn!("Rejected request from unauthorized origin: {}", origin_str);
                return Ok(HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "Origin not allowed"
                })));
            }
        }
    }

    // Validate Accept header - must support both application/json and text/event-stream
    let accept_header = req
        .headers()
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if !accept_header.contains("application/json") || !accept_header.contains("text/event-stream") {
        warn!("Invalid Accept header: {}", accept_header);
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Accept header must include both application/json and text/event-stream"
        })));
    }

    // Get or create session
    let session_id = get_or_create_session(&req, &state.session_manager).await?;

    // Parse the request body
    let body_str = String::from_utf8_lossy(&body);

    // Try to parse as single message or batch
    let messages = match parse_message_or_batch(&body_str) {
        Ok(msgs) => msgs,
        Err(e) => {
            error!("Failed to parse JSON-RPC message(s): {}", e);
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32700,
                    "message": "Parse error"
                },
                "id": null
            })));
        }
    };

    // Check if all messages are responses or notifications (no requests)
    let has_requests = messages
        .iter()
        .any(|msg| matches!(msg, crate::protocol::AnyJsonRpcMessage::Request(_)));

    if !has_requests {
        // Only responses/notifications - return 202 Accepted
        info!("Received only responses/notifications, returning 202 Accepted");
        return Ok(HttpResponse::Accepted().finish());
    }

    // Has requests - process them and decide response format
    let protocol_handler = &state.protocol_handler;

    // For now, return JSON response for single requests
    // TODO: Implement SSE streaming for complex scenarios
    if messages.len() == 1 {
        if let crate::protocol::AnyJsonRpcMessage::Request(request) = &messages[0] {
            info!("Processing single JSON-RPC request: {}", request.method);

            match protocol_handler.handle_request(request.clone()).await {
                Ok(response) => {
                    info!("Request processed successfully");
                    let mut http_response = HttpResponse::Ok().json(response);

                    // Include session ID in response header if present
                    if let Some(session_id) = get_session_id(&req) {
                        http_response.headers_mut().insert(
                            actix_web::http::header::HeaderName::from_static("mcp-session-id"),
                            actix_web::http::header::HeaderValue::from_str(&session_id).unwrap(),
                        );
                    }

                    Ok(http_response)
                }
                Err(e) => {
                    error!("Failed to process request: {}", e);
                    Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32603,
                            "message": format!("Internal error: {}", e)
                        },
                        "id": request.id
                    })))
                }
            }
        } else {
            Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Expected request message"
            })))
        }
    } else {
        // Multiple messages - would need SSE streaming
        // For now, return error
        Ok(HttpResponse::NotImplemented().json(serde_json::json!({
            "error": "Batch requests not yet implemented"
        })))
    }
}

/// Handle Streamable HTTP GET requests
/// Opens an optional SSE stream for server-initiated messages
async fn handle_streamable_http_get(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    info!("Handling Streamable HTTP GET request");

    // Validate Origin header for security
    if let Some(origin) = req.headers().get("Origin") {
        if let Ok(origin_str) = origin.to_str() {
            if !is_origin_allowed(origin_str, &state.config.cors_origins) {
                warn!("Rejected request from unauthorized origin: {}", origin_str);
                return Ok(HttpResponse::Forbidden().finish());
            }
        }
    }

    // Check Accept header - must support text/event-stream
    let accepts_sse = req
        .headers()
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .map(|accept| accept.contains("text/event-stream"))
        .unwrap_or(false);

    if !accepts_sse {
        return Ok(HttpResponse::MethodNotAllowed().finish());
    }

    // Get or create session
    let session_id = get_or_create_session(&req, &state.session_manager).await?;

    // Check for Last-Event-ID header for resumability
    let last_event_id = req
        .headers()
        .get("Last-Event-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    if let Some(event_id) = &last_event_id {
        info!("Resuming stream from event ID: {}", event_id);
        // TODO: Implement stream resumption logic
    }

    // Create SSE stream for server-initiated messages
    // For now, just send a connection confirmation
    let stream = futures_util::stream::iter(vec![Ok::<_, actix_web::Error>(web::Bytes::from(
        "data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n\n",
    ))]);

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Mcp-Session-Id", session_id))
        .streaming(stream))
}

/// Handle DELETE requests (session termination)
async fn handle_delete_request(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    info!("Handling DELETE request for session termination");

    if let Some(session_id) = get_session_id(&req) {
        state.session_manager.remove_session(&session_id).await;
        info!("Session {} terminated", session_id);
        Ok(HttpResponse::Ok().finish())
    } else {
        Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No session ID provided"
        })))
    }
}

/// Get or create a session for the request
async fn get_or_create_session(
    req: &HttpRequest,
    session_manager: &SessionManager,
) -> ActixResult<String> {
    if let Some(session_id) = get_session_id(req) {
        // Validate existing session
        if session_manager.get_session(&session_id).await.is_some() {
            return Ok(session_id);
        }
    }

    // Create new session
    let session_id = Uuid::new_v4().to_string();
    let session = Session::new(session_id.clone());
    session_manager.add_session(session).await;

    Ok(session_id)
}

/// Extract session ID from request headers
fn get_session_id(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Mcp-Session-Id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
}

/// Parse a single JSON-RPC message or batch of messages
fn parse_message_or_batch(body: &str) -> Result<Vec<crate::protocol::AnyJsonRpcMessage>> {
    // Try to parse as array first (batch)
    if let Ok(batch) = serde_json::from_str::<Vec<serde_json::Value>>(body) {
        let mut messages = Vec::new();
        for value in batch {
            let message = parse_message(&serde_json::to_string(&value)?)?;
            messages.push(message);
        }
        Ok(messages)
    } else {
        // Parse as single message
        let message = parse_message(body)?;
        Ok(vec![message])
    }
}

/// Check if origin is allowed
fn is_origin_allowed(origin: &str, allowed_origins: &[String]) -> bool {
    if allowed_origins.contains(&"*".to_string()) {
        return true;
    }

    allowed_origins.iter().any(|allowed| {
        allowed == origin || (allowed.starts_with("*.") && origin.ends_with(&allowed[1..]))
    })
}
