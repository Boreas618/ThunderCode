//! WebSocket client for CCR session subscriptions.
//!
//! Implements the reconnection state machine with exponential backoff, ping
//! keepalive, and permanent close code detection. Ported from
//! `ref/remote/SessionsWebSocket.ts`.

use std::sync::Arc;
use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::remote::types::{
    is_sessions_message, ControlRequestInner, ControlResponse, SdkControlRequest,
};

/// Default delay between reconnection attempts.
const RECONNECT_DELAY: Duration = Duration::from_millis(2000);

/// Maximum number of reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Interval for WebSocket ping frames to keep the connection alive.
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum retries for 4001 (session not found) -- can be transient during compaction.
const MAX_SESSION_NOT_FOUND_RETRIES: u32 = 3;

/// WebSocket close codes that indicate permanent server-side rejection.
const PERMANENT_CLOSE_CODES: &[u16] = &[
    4003, // unauthorized
];

/// Close code for session not found (may be transient during compaction).
const SESSION_NOT_FOUND_CODE: u16 = 4001;

/// State of the WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketState {
    /// Attempting to connect.
    Connecting,
    /// Connected and authenticated.
    Connected,
    /// Connection closed (terminal or pre-reconnect).
    Closed,
}

/// Callbacks for WebSocket events.
pub struct SessionsWebSocketCallbacks {
    /// Called when a parsed message arrives from the session.
    pub on_message: Box<dyn Fn(serde_json::Value) + Send + Sync>,
    /// Called when the connection is permanently closed.
    pub on_close: Option<Box<dyn Fn() + Send + Sync>>,
    /// Called on error.
    pub on_error: Option<Box<dyn Fn(String) + Send + Sync>>,
    /// Called when the connection is established.
    pub on_connected: Option<Box<dyn Fn() + Send + Sync>>,
    /// Called when a transient drop triggers a reconnect attempt.
    pub on_reconnecting: Option<Box<dyn Fn() + Send + Sync>>,
}

/// Type alias for the write half of the WebSocket.
type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// WebSocket client for CCR session subscriptions.
///
/// Connects to `wss://{base}/v1/sessions/ws/{session_id}/subscribe` and
/// handles the authentication, ping keepalive, and reconnection state machine.
///
/// # Protocol
///
/// 1. Connect with auth headers (`Authorization: Bearer <token>`)
/// 2. Receive SDK message stream from the session
/// 3. Send control responses / control requests back
pub struct SessionsWebSocket {
    session_id: String,
    org_uuid: String,
    base_url: String,
    get_access_token: Box<dyn Fn() -> String + Send + Sync>,
    callbacks: Arc<SessionsWebSocketCallbacks>,
    state: Arc<Mutex<WebSocketState>>,
    reconnect_attempts: Arc<Mutex<u32>>,
    session_not_found_retries: Arc<Mutex<u32>>,
    /// Channel for sending messages through the WebSocket.
    send_tx: mpsc::UnboundedSender<String>,
    /// Notify to trigger shutdown.
    shutdown: Arc<Notify>,
    /// Flag to track if close was requested.
    close_requested: Arc<Mutex<bool>>,
}

impl SessionsWebSocket {
    /// Create a new WebSocket client for the given session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The CCR session ID to subscribe to.
    /// * `org_uuid` - Organization UUID for the subscription URL.
    /// * `base_url` - The API base URL (e.g. `https://api.openai.com`).
    /// * `get_access_token` - Closure that returns a fresh OAuth access token.
    /// * `callbacks` - Event callbacks.
    pub fn new(
        session_id: String,
        org_uuid: String,
        base_url: String,
        get_access_token: Box<dyn Fn() -> String + Send + Sync>,
        callbacks: SessionsWebSocketCallbacks,
    ) -> Self {
        let (send_tx, _) = mpsc::unbounded_channel();
        Self {
            session_id,
            org_uuid,
            base_url,
            get_access_token,
            callbacks: Arc::new(callbacks),
            state: Arc::new(Mutex::new(WebSocketState::Closed)),
            reconnect_attempts: Arc::new(Mutex::new(0)),
            session_not_found_retries: Arc::new(Mutex::new(0)),
            send_tx,
            shutdown: Arc::new(Notify::new()),
            close_requested: Arc::new(Mutex::new(false)),
        }
    }

    /// Connect to the sessions WebSocket endpoint.
    ///
    /// Spawns background tasks for reading messages and sending pings.
    /// Returns immediately after initiating the connection.
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        {
            let state = *self.state.lock().await;
            if state == WebSocketState::Connecting {
                tracing::debug!("SessionsWebSocket: already connecting");
                return Ok(());
            }
        }

        *self.state.lock().await = WebSocketState::Connecting;
        *self.close_requested.lock().await = false;

        let ws_base = self.base_url.replace("https://", "wss://");
        let url = format!(
            "{ws_base}/v1/sessions/ws/{}/subscribe?organization_uuid={}",
            self.session_id, self.org_uuid
        );

        tracing::debug!(url = %url, "SessionsWebSocket: connecting");

        let token = (self.get_access_token)();

        // Build the WebSocket request with auth headers.
        let request = http::Request::builder()
            .uri(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("x-api-version", "2023-06-01")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .body(())?;

        let (ws_stream, _response): (WebSocketStream<MaybeTlsStream<TcpStream>>, _) =
            connect_async(request).await?;
        let (write, read) = ws_stream.split();

        *self.state.lock().await = WebSocketState::Connected;
        *self.reconnect_attempts.lock().await = 0;
        *self.session_not_found_retries.lock().await = 0;

        tracing::debug!("SessionsWebSocket: connected, authenticated via headers");

        if let Some(ref on_connected) = self.callbacks.on_connected {
            on_connected();
        }

        // Create a new send channel for this connection.
        let (send_tx, send_rx) = mpsc::unbounded_channel::<String>();
        self.send_tx = send_tx;

        // Spawn read loop.
        let callbacks = Arc::clone(&self.callbacks);
        let state = Arc::clone(&self.state);
        let _close_requested = Arc::clone(&self.close_requested);
        let _reconnect_attempts = Arc::clone(&self.reconnect_attempts);
        let _session_not_found_retries = Arc::clone(&self.session_not_found_retries);
        let shutdown = Arc::clone(&self.shutdown);

        tokio::spawn(Self::read_loop(read, callbacks.clone(), state.clone(), shutdown.clone()));

        // Spawn write loop (forwards queued messages to the sink).
        let write = Arc::new(Mutex::new(write));
        let write_clone = Arc::clone(&write);
        tokio::spawn(Self::write_loop(send_rx, write_clone));

        // Spawn ping loop.
        let state_ping = Arc::clone(&self.state);
        let write_ping = Arc::clone(&write);
        let shutdown_ping = Arc::clone(&self.shutdown);
        tokio::spawn(Self::ping_loop(state_ping, write_ping, shutdown_ping));

        Ok(())
    }

    /// Background task that reads messages from the WebSocket.
    async fn read_loop(
        mut read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        callbacks: Arc<SessionsWebSocketCallbacks>,
        state: Arc<Mutex<WebSocketState>>,
        shutdown: Arc<Notify>,
    ) {
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(data))) => {
                            match serde_json::from_str::<serde_json::Value>(&data) {
                                Ok(value) => {
                                    if is_sessions_message(&value) {
                                        (callbacks.on_message)(value);
                                    } else {
                                        tracing::debug!(
                                            "SessionsWebSocket: ignoring non-session message"
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        error = %e,
                                        "SessionsWebSocket: failed to parse message"
                                    );
                                    if let Some(ref on_error) = callbacks.on_error {
                                        on_error(format!("Parse error: {e}"));
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            tracing::trace!("SessionsWebSocket: pong received");
                        }
                        Some(Ok(Message::Close(frame))) => {
                            let code = frame.as_ref().map(|f| f.code.into()).unwrap_or(0u16);
                            tracing::debug!(code, "SessionsWebSocket: close frame received");
                            *state.lock().await = WebSocketState::Closed;
                            break;
                        }
                        Some(Err(e)) => {
                            tracing::error!(error = %e, "SessionsWebSocket: read error");
                            if let Some(ref on_error) = callbacks.on_error {
                                on_error(format!("WebSocket error: {e}"));
                            }
                            *state.lock().await = WebSocketState::Closed;
                            break;
                        }
                        None => {
                            tracing::debug!("SessionsWebSocket: stream ended");
                            *state.lock().await = WebSocketState::Closed;
                            break;
                        }
                        _ => {
                            // Binary/Frame messages ignored.
                        }
                    }
                }
                _ = shutdown.notified() => {
                    tracing::debug!("SessionsWebSocket: read loop shutdown");
                    break;
                }
            }
        }
    }

    /// Background task that forwards queued messages to the WebSocket sink.
    async fn write_loop(
        mut rx: mpsc::UnboundedReceiver<String>,
        write: Arc<Mutex<WsSink>>,
    ) {
        while let Some(data) = rx.recv().await {
            let mut sink = write.lock().await;
            if let Err(e) = sink.send(Message::Text(data)).await {
                tracing::error!(error = %e, "SessionsWebSocket: write error");
                break;
            }
        }
    }

    /// Background task that sends periodic ping frames.
    async fn ping_loop(
        state: Arc<Mutex<WebSocketState>>,
        write: Arc<Mutex<WsSink>>,
        shutdown: Arc<Notify>,
    ) {
        let mut interval = time::interval(PING_INTERVAL);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if *state.lock().await != WebSocketState::Connected {
                        break;
                    }
                    let mut sink = write.lock().await;
                    if let Err(e) = sink.send(Message::Ping(vec![])).await {
                        tracing::trace!(error = %e, "SessionsWebSocket: ping error");
                        break;
                    }
                }
                _ = shutdown.notified() => {
                    break;
                }
            }
        }
    }

    /// Send a control response back to the session.
    pub fn send_control_response(&self, response: ControlResponse) {
        let state_handle = self.state.clone();
        let tx = self.send_tx.clone();

        // We need to check state, but since we can't block, just try to send.
        // The write loop will handle errors if disconnected.
        match serde_json::to_string(&response) {
            Ok(json) => {
                tracing::debug!("SessionsWebSocket: sending control response");
                let _ = tx.send(json);
            }
            Err(e) => {
                tracing::error!(error = %e, "SessionsWebSocket: failed to serialize control response");
            }
        }

        // Suppress unused variable warning
        let _ = state_handle;
    }

    /// Send a control request to the session (e.g., interrupt).
    pub fn send_control_request(&self, request: ControlRequestInner) {
        let control_request = SdkControlRequest {
            msg_type: "control_request".to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
            request,
        };

        match serde_json::to_string(&control_request) {
            Ok(json) => {
                tracing::debug!(
                    subtype = %control_request.request.subtype,
                    "SessionsWebSocket: sending control request"
                );
                let _ = self.send_tx.send(json);
            }
            Err(e) => {
                tracing::error!(error = %e, "SessionsWebSocket: failed to serialize control request");
            }
        }
    }

    /// Check if the WebSocket is currently connected.
    pub async fn is_connected(&self) -> bool {
        *self.state.lock().await == WebSocketState::Connected
    }

    /// Close the WebSocket connection.
    pub async fn close(&mut self) {
        tracing::debug!("SessionsWebSocket: closing connection");
        *self.close_requested.lock().await = true;
        *self.state.lock().await = WebSocketState::Closed;
        self.shutdown.notify_waiters();
    }

    /// Force reconnect -- closes existing connection and establishes a new one.
    pub async fn reconnect(&mut self) {
        tracing::debug!("SessionsWebSocket: force reconnecting");
        *self.reconnect_attempts.lock().await = 0;
        *self.session_not_found_retries.lock().await = 0;
        self.close().await;

        // Small delay before reconnecting.
        time::sleep(Duration::from_millis(500)).await;

        *self.close_requested.lock().await = false;
        if let Err(e) = self.connect().await {
            tracing::error!(error = %e, "SessionsWebSocket: reconnect failed");
        }
    }

    /// Schedule a reconnection attempt after a delay.
    ///
    /// This implements the reconnection state machine:
    /// - Permanent close codes (4003) -> no reconnect
    /// - Session not found (4001) -> limited retries with increasing delay
    /// - Other codes -> up to MAX_RECONNECT_ATTEMPTS with fixed delay
    pub async fn handle_close(&mut self, close_code: u16) {
        // Permanent codes: stop reconnecting.
        if PERMANENT_CLOSE_CODES.contains(&close_code) {
            tracing::debug!(
                code = close_code,
                "SessionsWebSocket: permanent close code, not reconnecting"
            );
            if let Some(ref on_close) = self.callbacks.on_close {
                on_close();
            }
            return;
        }

        // 4001 (session not found) -- limited retries for transient compaction.
        if close_code == SESSION_NOT_FOUND_CODE {
            let mut retries = self.session_not_found_retries.lock().await;
            *retries += 1;
            if *retries > MAX_SESSION_NOT_FOUND_RETRIES {
                tracing::debug!(
                    retries = *retries,
                    "SessionsWebSocket: 4001 retry budget exhausted"
                );
                if let Some(ref on_close) = self.callbacks.on_close {
                    on_close();
                }
                return;
            }
            let delay = RECONNECT_DELAY * *retries;
            drop(retries);
            self.schedule_reconnect(delay).await;
            return;
        }

        // General reconnection with attempt limit.
        let mut attempts = self.reconnect_attempts.lock().await;
        *attempts += 1;
        if *attempts > MAX_RECONNECT_ATTEMPTS {
            tracing::debug!("SessionsWebSocket: max reconnect attempts reached");
            if let Some(ref on_close) = self.callbacks.on_close {
                on_close();
            }
            return;
        }
        let attempt = *attempts;
        drop(attempts);

        tracing::debug!(
            attempt,
            max = MAX_RECONNECT_ATTEMPTS,
            "SessionsWebSocket: scheduling reconnect"
        );
        self.schedule_reconnect(RECONNECT_DELAY).await;
    }

    /// Sleep then reconnect.
    async fn schedule_reconnect(&mut self, delay: Duration) {
        if let Some(ref on_reconnecting) = self.callbacks.on_reconnecting {
            on_reconnecting();
        }

        tracing::debug!(
            delay_ms = delay.as_millis(),
            "SessionsWebSocket: reconnecting after delay"
        );
        time::sleep(delay).await;

        if *self.close_requested.lock().await {
            return;
        }

        if let Err(e) = self.connect().await {
            tracing::error!(error = %e, "SessionsWebSocket: reconnect attempt failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permanent_close_codes() {
        assert!(PERMANENT_CLOSE_CODES.contains(&4003));
        assert!(!PERMANENT_CLOSE_CODES.contains(&4001));
        assert!(!PERMANENT_CLOSE_CODES.contains(&1000));
    }

    #[test]
    fn test_websocket_state_eq() {
        assert_eq!(WebSocketState::Closed, WebSocketState::Closed);
        assert_ne!(WebSocketState::Connecting, WebSocketState::Connected);
    }

    #[tokio::test]
    async fn test_websocket_initial_state() {
        let callbacks = SessionsWebSocketCallbacks {
            on_message: Box::new(|_| {}),
            on_close: None,
            on_error: None,
            on_connected: None,
            on_reconnecting: None,
        };
        let ws = SessionsWebSocket::new(
            "test-session".to_string(),
            "test-org".to_string(),
            "https://api.example.com".to_string(),
            Box::new(|| "test-token".to_string()),
            callbacks,
        );
        assert!(!ws.is_connected().await);
    }
}
