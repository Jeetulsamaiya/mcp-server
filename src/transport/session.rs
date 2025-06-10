//! Session management for HTTP transport.
//!
//! This module handles session lifecycle, tracking, and cleanup for HTTP-based
//! MCP connections as defined in the specification.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier
    pub id: String,

    /// Session creation time
    pub created_at: Instant,

    /// Last activity time
    pub last_activity: Instant,

    /// Client information
    pub client_info: Option<ClientInfo>,

    /// Session state
    pub state: SessionState,

    /// Custom session data
    pub data: HashMap<String, serde_json::Value>,
}

/// Client information
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Client IP address
    pub ip_address: Option<std::net::IpAddr>,

    /// User agent string
    pub user_agent: Option<String>,

    /// Client capabilities
    pub capabilities: Option<crate::protocol::ClientCapabilities>,
}

/// Session state
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    /// Session created but not initialized
    Created,

    /// Session initialized and active
    Active,

    /// Session expired
    Expired,

    /// Session terminated
    Terminated,
}

/// Session manager for handling HTTP sessions
pub struct SessionManager {
    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, Session>>>,

    /// Session timeout duration
    timeout: Duration,

    /// Cleanup task handle
    cleanup_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl Session {
    /// Create a new session
    pub fn new(id: String) -> Self {
        let now = Instant::now();

        Self {
            id,
            created_at: now,
            last_activity: now,
            client_info: None,
            state: SessionState::Created,
            data: HashMap::new(),
        }
    }

    /// Update last activity time
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if session is expired
    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Set client information
    pub fn set_client_info(&mut self, client_info: ClientInfo) {
        self.client_info = Some(client_info);
    }

    /// Set session state
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// Get session data
    pub fn get_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Set session data
    pub fn set_data(&mut self, key: String, value: serde_json::Value) {
        self.data.insert(key, value);
    }

    /// Remove session data
    pub fn remove_data(&mut self, key: &str) -> Option<serde_json::Value> {
        self.data.remove(key)
    }
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(timeout: Duration) -> Self {
        let manager = Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            timeout,
            cleanup_handle: Arc::new(RwLock::new(None)),
        };

        // Start cleanup task
        manager.start_cleanup_task();

        manager
    }

    /// Add a new session
    pub async fn add_session(&self, session: Session) {
        let session_id = session.id.clone();

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session);
        }

        info!("Added session: {}", session_id);
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Update a session
    pub async fn update_session<F>(&self, session_id: &str, update_fn: F) -> bool
    where
        F: FnOnce(&mut Session),
    {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(session_id) {
            update_fn(session);
            true
        } else {
            false
        }
    }

    /// Touch a session (update last activity)
    pub async fn touch_session(&self, session_id: &str) -> bool {
        self.update_session(session_id, |session| {
            session.touch();
        })
        .await
    }

    /// Remove a session
    pub async fn remove_session(&self, session_id: &str) -> Option<Session> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.remove(session_id);

        if session.is_some() {
            info!("Removed session: {}", session_id);
        }

        session
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.state == SessionState::Active)
            .cloned()
            .collect()
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let mut expired_sessions = Vec::new();

        // Find expired sessions
        for (id, session) in sessions.iter() {
            if session.is_expired(self.timeout) {
                expired_sessions.push(id.clone());
            }
        }

        // Remove expired sessions
        let count = expired_sessions.len();
        for session_id in expired_sessions {
            sessions.remove(&session_id);
            info!("Cleaned up expired session: {}", session_id);
        }

        if count > 0 {
            info!("Cleaned up {} expired sessions", count);
        }

        count
    }

    /// Start the cleanup task
    fn start_cleanup_task(&self) {
        let sessions = self.sessions.clone();
        let timeout = self.timeout;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Cleanup every minute

            loop {
                interval.tick().await;

                let mut expired_sessions = Vec::new();

                // Find expired sessions
                {
                    let sessions_guard = sessions.read().await;
                    for (id, session) in sessions_guard.iter() {
                        if session.is_expired(timeout) {
                            expired_sessions.push(id.clone());
                        }
                    }
                }

                // Remove expired sessions
                if !expired_sessions.is_empty() {
                    let mut sessions_guard = sessions.write().await;
                    let count = expired_sessions.len();

                    for session_id in expired_sessions {
                        sessions_guard.remove(&session_id);
                    }

                    info!("Cleaned up {} expired sessions", count);
                }
            }
        });

        tokio::spawn(async move {
            // Placeholder for handle storage
        });
    }

    /// Stop the session manager and cleanup task
    pub async fn stop(&self) {
        let handle = {
            let mut cleanup_handle = self.cleanup_handle.write().await;
            cleanup_handle.take()
        };

        if let Some(handle) = handle {
            handle.abort();
        }

        // Clear all sessions
        {
            let mut sessions = self.sessions.write().await;
            let count = sessions.len();
            sessions.clear();

            if count > 0 {
                info!("Cleared {} sessions during shutdown", count);
            }
        }
    }

    /// Get session statistics
    pub async fn get_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let total = sessions.len();
        let mut active = 0;
        let mut expired = 0;

        for session in sessions.values() {
            match session.state {
                SessionState::Active => active += 1,
                SessionState::Expired => expired += 1,
                _ => {}
            }
        }

        SessionStats {
            total,
            active,
            expired,
            created: total - active - expired,
        }
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total: usize,
    pub active: usize,
    pub expired: usize,
    pub created: usize,
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        // Synchronous drop - cannot await here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_session_creation() {
        let session = Session::new("test-session".to_string());
        assert_eq!(session.id, "test-session");
        assert_eq!(session.state, SessionState::Created);
    }

    #[tokio::test]
    async fn test_session_manager() {
        let manager = SessionManager::new(Duration::from_secs(1));

        // Add a session
        let session = Session::new("test-session".to_string());
        manager.add_session(session).await;

        // Get the session
        let retrieved = manager.get_session("test-session").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test-session");

        // Remove the session
        let removed = manager.remove_session("test-session").await;
        assert!(removed.is_some());

        // Try to get the removed session
        let not_found = manager.get_session("test-session").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let manager = SessionManager::new(Duration::from_millis(100));

        // Add a session
        let session = Session::new("test-session".to_string());
        manager.add_session(session).await;

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Clean up expired sessions
        let cleaned = manager.cleanup_expired_sessions().await;
        assert_eq!(cleaned, 1);

        // Session should be gone
        let not_found = manager.get_session("test-session").await;
        assert!(not_found.is_none());
    }
}
