//! Remote session manager -- coordinates WebSocket subscriptions, HTTP message
//! sending, and permission request/response flow.
//!
//! Ported from ref/remote/RemoteSessionManager.ts`.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::remote::types::{
    ControlRequestInner, ControlResponse, ControlResponseInner, RemoteEvent,
    RemotePermissionResponse,
};
use crate::remote::websocket::{SessionsWebSocket, SessionsWebSocketCallbacks};

/// Configuration for a remote session connection.
#[derive(Debug, Clone)]
pub struct RemoteSessionConfig {
    /// The CCR session ID.
    pub session_id: String,

    /// Closure to get a fresh access token.
    /// Stored as a string for Clone -- in practice, pass a fresh token.
    pub access_token: String,

    /// Organization UUID.
    pub org_uuid: String,

    /// API base URL (e.g. `https://api.openai.com`).
    pub base_url: String,

    /// True if session was created with an initial prompt that's being processed.
    pub has_initial_prompt: bool,

    /// When true, this client is a pure viewer (no interrupt, no title updates).
    pub viewer_only: bool,
}

/// Callbacks for remote session events.
pub struct RemoteSessionCallbacks {
    /// Called when an SDK message is received from the session.
    pub on_message: Box<dyn Fn(serde_json::Value) + Send + Sync>,

    /// Called when a permission request is received from CCR.
    pub on_permission_request:
        Box<dyn Fn(ControlRequestInner, String) + Send + Sync>,

    /// Called when the server cancels a pending permission request.
    pub on_permission_cancelled: Option<Box<dyn Fn(String, Option<String>) + Send + Sync>>,

    /// Called when connection is established.
    pub on_connected: Option<Box<dyn Fn() + Send + Sync>>,

    /// Called when connection is lost and cannot be restored.
    pub on_disconnected: Option<Box<dyn Fn() + Send + Sync>>,

    /// Called on transient WS drop while reconnect backoff is in progress.
    pub on_reconnecting: Option<Box<dyn Fn() + Send + Sync>>,

    /// Called on error.
    pub on_error: Option<Box<dyn Fn(String) + Send + Sync>>,
}

/// Manages a remote CCR session.
///
/// Coordinates:
/// - WebSocket subscription for receiving messages from CCR
/// - Permission request/response flow
/// - Interrupt signals
///
/// The session manager dispatches incoming WebSocket messages to the
/// appropriate callback based on the message type.
pub struct RemoteSessionManager {
    config: RemoteSessionConfig,
    websocket: Option<SessionsWebSocket>,
    pending_permissions: Arc<Mutex<HashMap<String, ControlRequestInner>>>,
    event_tx: tokio::sync::mpsc::UnboundedSender<RemoteEvent>,
    event_rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<RemoteEvent>>>,
}

impl RemoteSessionManager {
    /// Create a new remote session manager.
    pub fn new(config: RemoteSessionConfig) -> Self {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            config,
            websocket: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
        }
    }

    /// Connect to the remote session via WebSocket.
    ///
    /// Sets up the WebSocket with appropriate callbacks and initiates
    /// the connection.
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        tracing::debug!(
            session_id = %self.config.session_id,
            "RemoteSessionManager: connecting"
        );

        let pending = Arc::clone(&self.pending_permissions);
        let event_tx = self.event_tx.clone();
        let event_tx_connected = self.event_tx.clone();
        let event_tx_close = self.event_tx.clone();
        let event_tx_reconnecting = self.event_tx.clone();
        let event_tx_error = self.event_tx.clone();

        let callbacks = SessionsWebSocketCallbacks {
            on_message: Box::new(move |value: serde_json::Value| {
                let msg_type = value
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");

                match msg_type {
                    "control_request" => {
                        if let (Some(request_id), Some(request)) =
                            (value.get("request_id"), value.get("request"))
                        {
                            let request_id =
                                request_id.as_str().unwrap_or_default().to_string();
                            if let Ok(inner) =
                                serde_json::from_value::<ControlRequestInner>(request.clone())
                            {
                                if inner.subtype == "can_use_tool" {
                                    tracing::debug!(
                                        tool = ?inner.tool_name,
                                        "RemoteSessionManager: permission request"
                                    );
                                    {
                                        // Store in pending map -- blocking lock is fine
                                        // since this is a callback that completes quickly.
                                        let pending = pending.clone();
                                        let inner_clone = inner.clone();
                                        let rid = request_id.clone();
                                        tokio::spawn(async move {
                                            pending.lock().await.insert(rid, inner_clone);
                                        });
                                    }
                                    let _ = event_tx.send(RemoteEvent::PermissionRequest {
                                        request: inner,
                                        request_id,
                                    });
                                } else {
                                    tracing::debug!(
                                        subtype = %inner.subtype,
                                        "RemoteSessionManager: unsupported control request"
                                    );
                                }
                            }
                        }
                    }
                    "control_cancel_request" => {
                        if let Some(request_id) = value.get("request_id").and_then(|r| r.as_str())
                        {
                            tracing::debug!(
                                request_id,
                                "RemoteSessionManager: permission cancelled"
                            );
                            let pending = pending.clone();
                            let rid = request_id.to_string();
                            let tx = event_tx.clone();
                            tokio::spawn(async move {
                                let removed = pending.lock().await.remove(&rid);
                                let tool_use_id =
                                    removed.and_then(|r| r.tool_use_id);
                                let _ = tx.send(RemoteEvent::PermissionCancelled {
                                    request_id: rid,
                                    tool_use_id,
                                });
                            });
                        }
                    }
                    "control_response" => {
                        tracing::debug!("RemoteSessionManager: received control response");
                    }
                    _ => {
                        // Forward as SDK message.
                        let _ = event_tx.send(RemoteEvent::Message(
                            crate::remote::types::SdkMessage {
                                msg_type: msg_type.to_string(),
                                uuid: value
                                    .get("uuid")
                                    .and_then(|u| u.as_str())
                                    .map(String::from),
                                extra: value
                                    .as_object()
                                    .map(|o| {
                                        o.iter()
                                            .filter(|(k, _)| *k != "type" && *k != "uuid")
                                            .map(|(k, v)| (k.clone(), v.clone()))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            },
                        ));
                    }
                }
            }),
            on_connected: Some(Box::new(move || {
                tracing::debug!("RemoteSessionManager: connected");
                let _ = event_tx_connected.send(RemoteEvent::Connected);
            })),
            on_close: Some(Box::new(move || {
                tracing::debug!("RemoteSessionManager: disconnected");
                let _ = event_tx_close.send(RemoteEvent::Disconnected);
            })),
            on_reconnecting: Some(Box::new(move || {
                tracing::debug!("RemoteSessionManager: reconnecting");
                let _ = event_tx_reconnecting.send(RemoteEvent::Reconnecting);
            })),
            on_error: Some(Box::new(move |error: String| {
                tracing::error!(error = %error, "RemoteSessionManager: error");
                let _ = event_tx_error.send(RemoteEvent::Error(error));
            })),
        };

        let token = self.config.access_token.clone();
        let mut ws = SessionsWebSocket::new(
            self.config.session_id.clone(),
            self.config.org_uuid.clone(),
            self.config.base_url.clone(),
            Box::new(move || token.clone()),
            callbacks,
        );

        ws.connect().await?;
        self.websocket = Some(ws);

        Ok(())
    }

    /// Respond to a permission request from CCR.
    pub async fn respond_to_permission(
        &self,
        request_id: &str,
        result: RemotePermissionResponse,
    ) {
        let pending_request = {
            let mut pending = self.pending_permissions.lock().await;
            pending.remove(request_id)
        };

        if pending_request.is_none() {
            tracing::error!(
                request_id,
                "RemoteSessionManager: no pending permission request"
            );
            return;
        }

        let response_value = match result {
            RemotePermissionResponse::Allow { ref updated_input } => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "behavior".to_string(),
                    serde_json::Value::String("allow".to_string()),
                );
                map.insert(
                    "updatedInput".to_string(),
                    serde_json::to_value(updated_input).unwrap_or_default(),
                );
                serde_json::Value::Object(map)
            }
            RemotePermissionResponse::Deny { ref message } => {
                let mut map = serde_json::Map::new();
                map.insert(
                    "behavior".to_string(),
                    serde_json::Value::String("deny".to_string()),
                );
                map.insert(
                    "message".to_string(),
                    serde_json::Value::String(message.clone()),
                );
                serde_json::Value::Object(map)
            }
        };

        let response = ControlResponse {
            msg_type: "control_response".to_string(),
            response: ControlResponseInner {
                subtype: "success".to_string(),
                request_id: request_id.to_string(),
                error: None,
                response: Some(response_value),
            },
        };

        tracing::debug!(
            "RemoteSessionManager: sending permission response"
        );

        if let Some(ref ws) = self.websocket {
            ws.send_control_response(response);
        }
    }

    /// Check if connected to the remote session.
    pub async fn is_connected(&self) -> bool {
        match &self.websocket {
            Some(ws) => ws.is_connected().await,
            None => false,
        }
    }

    /// Send an interrupt signal to cancel the current request on the remote session.
    pub fn cancel_session(&self) {
        tracing::debug!("RemoteSessionManager: sending interrupt signal");
        if let Some(ref ws) = self.websocket {
            ws.send_control_request(ControlRequestInner {
                subtype: "interrupt".to_string(),
                tool_name: None,
                input: None,
                tool_use_id: None,
                model: None,
                mode: None,
                max_thinking_tokens: None,
            });
        }
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.config.session_id
    }

    /// Receive the next event from the session.
    ///
    /// Returns `None` if the event channel is closed.
    pub async fn recv_event(&self) -> Option<RemoteEvent> {
        self.event_rx.lock().await.recv().await
    }

    /// Disconnect from the remote session.
    pub async fn disconnect(&mut self) {
        tracing::debug!("RemoteSessionManager: disconnecting");
        if let Some(ref mut ws) = self.websocket {
            ws.close().await;
        }
        self.websocket = None;
        self.pending_permissions.lock().await.clear();
    }

    /// Force reconnect the WebSocket.
    pub async fn reconnect(&mut self) {
        tracing::debug!("RemoteSessionManager: reconnecting WebSocket");
        if let Some(ref mut ws) = self.websocket {
            ws.reconnect().await;
        }
    }
}

/// Create a remote session config.
pub fn create_remote_session_config(
    session_id: String,
    access_token: String,
    org_uuid: String,
    base_url: String,
    has_initial_prompt: bool,
    viewer_only: bool,
) -> RemoteSessionConfig {
    RemoteSessionConfig {
        session_id,
        access_token,
        org_uuid,
        base_url,
        has_initial_prompt,
        viewer_only,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_config() {
        let config = create_remote_session_config(
            "sess-123".to_string(),
            "token-abc".to_string(),
            "org-xyz".to_string(),
            "https://api.example.com".to_string(),
            false,
            false,
        );
        assert_eq!(config.session_id, "sess-123");
        assert_eq!(config.org_uuid, "org-xyz");
        assert!(!config.has_initial_prompt);
        assert!(!config.viewer_only);
    }

    #[tokio::test]
    async fn test_manager_initial_state() {
        let config = create_remote_session_config(
            "sess-123".to_string(),
            "token-abc".to_string(),
            "org-xyz".to_string(),
            "https://api.example.com".to_string(),
            false,
            false,
        );
        let manager = RemoteSessionManager::new(config);
        assert!(!manager.is_connected().await);
        assert_eq!(manager.session_id(), "sess-123");
    }
}
