//! ThunderCode remote session support.
//!
//! Provides WebSocket-based communication with remote CCR (ThunderCode Runner)
//! sessions, including:
//!
//! - **`manager`** -- `RemoteSessionManager` for coordinating WebSocket
//!   subscriptions, HTTP message sending, and permission request/response flow.
//! - **`websocket`** -- `SessionsWebSocket` with automatic reconnection and
//!   exponential backoff state machine.
//! - **`types`** -- Protocol types for SDK messages, control requests/responses.
//!
//! Ported from:
//! - `ref/remote/RemoteSessionManager.ts`
//! - `ref/remote/SessionsWebSocket.ts`

pub mod manager;
pub mod types;
pub mod websocket;

pub use manager::{RemoteSessionConfig, RemoteSessionManager};
pub use types::{
    ControlRequestInner, ControlResponse, RemoteEvent, RemotePermissionResponse, SdkControlRequest,
    SdkMessage, SessionsMessage,
};
pub use websocket::{SessionsWebSocket, SessionsWebSocketCallbacks, WebSocketState};
