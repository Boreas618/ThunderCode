//! MCP transport implementations.
//!
//! Defines the `McpTransport` trait and concrete implementations for
//! stdio (child process), SSE (HTTP server-sent events), and WebSocket
//! transports.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};

use crate::mcp::jsonrpc::{JsonRpcRequest, JsonRpcResponse};

// ============================================================================
// McpTransport trait
// ============================================================================

/// Trait for MCP transport layers.
///
/// A transport sends JSON-RPC requests and returns JSON-RPC responses.
/// Implementations handle the specifics of communication (stdio pipes,
/// HTTP SSE, WebSocket, etc.).
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC request and wait for the matching response.
    async fn send(&self, message: &JsonRpcRequest) -> Result<JsonRpcResponse>;

    /// Gracefully close the transport.
    async fn close(&self) -> Result<()>;
}

// ============================================================================
// StdioTransport
// ============================================================================

/// Transport that communicates with an MCP server via a spawned child process.
///
/// Writes JSON-RPC messages to the child's stdin (one per line) and reads
/// responses from stdout (one JSON object per line).
pub struct StdioTransport {
    inner: Arc<StdioInner>,
}

struct StdioInner {
    /// Handle to the child process.
    child: Mutex<Option<Child>>,
    /// Stdin writer, guarded by a mutex for concurrent access.
    stdin: Mutex<Option<tokio::process::ChildStdin>>,
    /// Pending response receivers keyed by request ID string.
    pending: Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>,
    /// Handle to the reader task so we can abort on close.
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl StdioTransport {
    /// Spawn the child process and create a transport.
    ///
    /// `command` is the binary to run; `args` are its arguments.
    /// `env` contains additional environment variables to set.
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server: {}", command))?;

        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");

        let inner = Arc::new(StdioInner {
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(Some(stdin)),
            pending: Mutex::new(HashMap::new()),
            reader_handle: Mutex::new(None),
        });

        // Spawn a background task that reads lines from stdout and dispatches
        // them to the appropriate pending request.
        let reader_inner = Arc::clone(&inner);
        let reader_handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        let id_key = response.id.to_string();
                        let mut pending = reader_inner.pending.lock().await;
                        if let Some(sender) = pending.remove(&id_key) {
                            let _ = sender.send(response);
                        } else {
                            tracing::debug!(
                                "MCP stdio: received response for unknown id: {}",
                                id_key
                            );
                        }
                    }
                    Err(e) => {
                        tracing::debug!("MCP stdio: failed to parse line as JSON-RPC: {}", e);
                    }
                }
            }
        });

        *inner.reader_handle.lock().await = Some(reader_handle);

        Ok(StdioTransport { inner })
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, message: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let id_key = message.id.to_string();

        // Register a pending receiver before sending.
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.inner.pending.lock().await;
            pending.insert(id_key.clone(), tx);
        }

        // Serialize and write to stdin.
        let mut line = serde_json::to_string(message)
            .context("failed to serialize JSON-RPC request")?;
        line.push('\n');

        {
            let mut stdin_guard = self.inner.stdin.lock().await;
            if let Some(ref mut stdin) = *stdin_guard {
                stdin
                    .write_all(line.as_bytes())
                    .await
                    .context("failed to write to MCP server stdin")?;
                stdin
                    .flush()
                    .await
                    .context("failed to flush MCP server stdin")?;
            } else {
                anyhow::bail!("MCP server stdin is closed");
            }
        }

        // Wait for the response.
        let response = rx
            .await
            .context("MCP server response channel closed (server may have exited)")?;
        Ok(response)
    }

    async fn close(&self) -> Result<()> {
        // Close stdin to signal the child process.
        {
            let mut stdin_guard = self.inner.stdin.lock().await;
            *stdin_guard = None;
        }

        // Abort the reader task.
        {
            let mut handle = self.inner.reader_handle.lock().await;
            if let Some(h) = handle.take() {
                h.abort();
            }
        }

        // Kill the child process if still running.
        {
            let mut child_guard = self.inner.child.lock().await;
            if let Some(ref mut child) = *child_guard {
                let _ = child.kill().await;
            }
            *child_guard = None;
        }

        Ok(())
    }
}

// ============================================================================
// HttpTransport (shared by SSE and Streamable HTTP)
// ============================================================================

/// Transport that communicates with an MCP server over HTTP.
///
/// Sends JSON-RPC requests as HTTP POST and reads JSON-RPC responses
/// from the response body. This handles both plain streamable-HTTP and
/// SSE-based MCP servers.
pub struct HttpTransport {
    url: String,
    client: reqwest::Client,
    headers: HashMap<String, String>,
}

impl HttpTransport {
    /// Create a new HTTP transport.
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        HttpTransport {
            url: url.into(),
            client: reqwest::Client::new(),
            headers,
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for HttpTransport {
    async fn send(&self, message: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let mut builder = self.client.post(&self.url);
        builder = builder.header("Content-Type", "application/json");
        for (k, v) in &self.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }

        let body = serde_json::to_string(message)
            .context("failed to serialize JSON-RPC request")?;

        let resp = builder
            .body(body)
            .send()
            .await
            .context("HTTP request to MCP server failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "MCP HTTP server returned status {}: {}",
                status,
                body_text
            );
        }

        let text = resp.text().await.context("failed to read HTTP response body")?;
        let response: JsonRpcResponse =
            serde_json::from_str(&text).context("failed to parse JSON-RPC response from HTTP body")?;
        Ok(response)
    }

    async fn close(&self) -> Result<()> {
        // HTTP is stateless; nothing to close.
        Ok(())
    }
}

// ============================================================================
// WebSocketTransport
// ============================================================================

/// Transport that communicates with an MCP server over WebSocket.
///
/// Sends JSON-RPC requests as text frames and reads responses from incoming
/// text frames.
pub struct WebSocketTransport {
    inner: Arc<WsInner>,
}

struct WsInner {
    writer: Mutex<
        Option<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
    pending: Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>,
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl WebSocketTransport {
    /// Connect to a WebSocket MCP server at the given URL.
    pub async fn connect(url: &str) -> Result<Self> {
        use futures::StreamExt;

        let (ws_stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .with_context(|| format!("failed to connect to WebSocket MCP server: {}", url))?;

        let (writer, mut reader) = futures::StreamExt::split(ws_stream);

        let inner = Arc::new(WsInner {
            writer: Mutex::new(Some(writer)),
            pending: Mutex::new(HashMap::new()),
            reader_handle: Mutex::new(None),
        });

        let reader_inner = Arc::clone(&inner);
        let reader_handle = tokio::spawn(async move {
            use tokio_tungstenite::tungstenite::Message;
            while let Some(msg_result) = reader.next().await {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<JsonRpcResponse>(&text) {
                            Ok(response) => {
                                let id_key = response.id.to_string();
                                let mut pending = reader_inner.pending.lock().await;
                                if let Some(sender) = pending.remove(&id_key) {
                                    let _ = sender.send(response);
                                }
                            }
                            Err(e) => {
                                tracing::debug!(
                                    "MCP WebSocket: failed to parse message: {}",
                                    e
                                );
                            }
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(e) => {
                        tracing::debug!("MCP WebSocket read error: {}", e);
                        break;
                    }
                    _ => {} // Ignore ping/pong/binary
                }
            }
        });

        *inner.reader_handle.lock().await = Some(reader_handle);

        Ok(WebSocketTransport { inner })
    }
}

#[async_trait::async_trait]
impl McpTransport for WebSocketTransport {
    async fn send(&self, message: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let id_key = message.id.to_string();

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.inner.pending.lock().await;
            pending.insert(id_key.clone(), tx);
        }

        let json = serde_json::to_string(message)
            .context("failed to serialize JSON-RPC request")?;

        {
            let mut writer_guard = self.inner.writer.lock().await;
            if let Some(ref mut writer) = *writer_guard {
                writer
                    .send(Message::Text(json))
                    .await
                    .context("failed to send WebSocket message")?;
            } else {
                anyhow::bail!("WebSocket connection is closed");
            }
        }

        let response = rx
            .await
            .context("WebSocket response channel closed")?;
        Ok(response)
    }

    async fn close(&self) -> Result<()> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        // Send a close frame.
        {
            let mut writer_guard = self.inner.writer.lock().await;
            if let Some(ref mut writer) = *writer_guard {
                let _ = writer.send(Message::Close(None)).await;
            }
            *writer_guard = None;
        }

        // Abort the reader task.
        {
            let mut handle = self.inner.reader_handle.lock().await;
            if let Some(h) = handle.take() {
                h.abort();
            }
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::jsonrpc::JsonRpcId;

    #[test]
    fn test_http_transport_creation() {
        let transport = HttpTransport::new(
            "http://localhost:8080/mcp",
            HashMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
        );
        assert_eq!(transport.url, "http://localhost:8080/mcp");
        assert_eq!(transport.headers.len(), 1);
    }

    #[tokio::test]
    async fn test_stdio_spawn_nonexistent_command() {
        let result = StdioTransport::spawn(
            "/nonexistent/binary/mcp-server-12345",
            &[],
            &HashMap::new(),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_websocket_connect_invalid_url() {
        let result = WebSocketTransport::connect("ws://127.0.0.1:1").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_request_serialization_for_transport() {
        let req = JsonRpcRequest::new(
            "tools/list",
            None,
            JsonRpcId::Number(1),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"tools/list\""));
    }
}
