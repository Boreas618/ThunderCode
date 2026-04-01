//! Child process management for spawned bridge sessions.
//!
//! Ported from ref/bridge/sessionRunner.ts`.

use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{watch, Mutex};

use crate::bridge::messaging::{safe_filename_id, tool_summary};
use crate::bridge::types::{SessionActivity, SessionActivityType, SessionDoneStatus, SessionSpawnOpts};

/// Maximum number of recent activities to keep per session.
const MAX_ACTIVITIES: usize = 10;

/// Maximum number of stderr lines to buffer per session.
const MAX_STDERR_LINES: usize = 10;

/// Handle to a running session child process.
///
/// Provides access to the session's activity log, status, and control
/// (kill, force-kill, stdin writing).
pub struct SessionHandle {
    /// The session ID.
    pub session_id: String,

    /// Access token for session ingress (may be refreshed).
    pub access_token: String,

    /// Ring buffer of recent activities.
    activities: Arc<Mutex<VecDeque<SessionActivity>>>,

    /// Most recent activity.
    current_activity: Arc<Mutex<Option<SessionActivity>>>,

    /// Ring buffer of last stderr lines.
    last_stderr: Arc<Mutex<VecDeque<String>>>,

    /// Watch channel that resolves when the child process exits.
    done_rx: watch::Receiver<Option<SessionDoneStatus>>,

    /// Sender for stdin data.
    stdin_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,

    /// Child process handle for kill signals.
    child: Arc<Mutex<Option<Child>>>,
}

impl SessionHandle {
    /// Get a snapshot of the recent activity ring buffer.
    pub async fn activities(&self) -> Vec<SessionActivity> {
        self.activities.lock().await.iter().cloned().collect()
    }

    /// Get the most recent activity, if any.
    pub async fn current_activity(&self) -> Option<SessionActivity> {
        self.current_activity.lock().await.clone()
    }

    /// Get the last stderr lines for error diagnostics.
    pub async fn last_stderr(&self) -> Vec<String> {
        self.last_stderr.lock().await.iter().cloned().collect()
    }

    /// Wait for the session to complete.
    pub async fn wait(&mut self) -> SessionDoneStatus {
        loop {
            self.done_rx.changed().await.ok();
            if let Some(status) = *self.done_rx.borrow() {
                return status;
            }
        }
    }

    /// Check if the session has completed.
    pub fn is_done(&self) -> bool {
        self.done_rx.borrow().is_some()
    }

    /// Send SIGTERM to the child process.
    pub async fn kill(&self) {
        if let Some(ref mut child) = *self.child.lock().await {
            tracing::debug!(
                session_id = %self.session_id,
                "bridge:session sending SIGTERM"
            );
            let _ = child.kill().await;
        }
    }

    /// Write data to the child's stdin.
    pub fn write_stdin(&self, data: String) {
        if let Some(ref tx) = self.stdin_tx {
            let _ = tx.send(data);
        }
    }

    /// Update the access token for a running session (e.g. after token refresh).
    pub fn update_access_token(&mut self, token: String) {
        self.access_token = token.clone();
        // Send the fresh token to the child process via stdin.
        let update = serde_json::json!({
            "type": "update_environment_variables",
            "variables": {
                "THUNDERCODE_SESSION_ACCESS_TOKEN": token,
            }
        });
        self.write_stdin(format!("{}\n", update));
        tracing::debug!(
            session_id = %self.session_id,
            "bridge:session sent token refresh via stdin"
        );
    }
}

/// Factory for spawning session child processes.
pub struct SessionSpawner {
    /// Path to the executable.
    pub exec_path: String,
    /// Additional script args (for non-compiled installs).
    pub script_args: Vec<String>,
    /// Enable verbose logging for child processes.
    pub verbose: bool,
    /// Enable sandbox mode.
    pub sandbox: bool,
    /// Debug file path template.
    pub debug_file: Option<String>,
    /// Permission mode for child sessions.
    pub permission_mode: Option<String>,
}

impl SessionSpawner {
    /// Spawn a new session child process.
    ///
    /// Returns a `SessionHandle` that can be used to monitor and control
    /// the session, or an error if the spawn fails.
    pub fn spawn(
        &self,
        opts: &SessionSpawnOpts,
        dir: &str,
    ) -> Result<SessionHandle, anyhow::Error> {
        let safe_id = safe_filename_id(&opts.session_id);

        // Build debug file path with session ID suffix.
        let debug_file = self.debug_file.as_ref().map(|f| {
            if let Some(pos) = f.rfind('.') {
                format!("{}-{}{}", &f[..pos], safe_id, &f[pos..])
            } else {
                format!("{}-{}", f, safe_id)
            }
        });

        // Build child process arguments.
        let mut args = Vec::new();
        args.extend(self.script_args.iter().cloned());
        args.extend([
            "--print".to_string(),
            "--sdk-url".to_string(),
            opts.sdk_url.clone(),
            "--session-id".to_string(),
            opts.session_id.clone(),
            "--input-format".to_string(),
            "stream-json".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--replay-user-messages".to_string(),
        ]);
        if self.verbose {
            args.push("--verbose".to_string());
        }
        if let Some(ref df) = debug_file {
            args.extend(["--debug-file".to_string(), df.clone()]);
        }
        if let Some(ref mode) = self.permission_mode {
            args.extend(["--permission-mode".to_string(), mode.clone()]);
        }

        tracing::debug!(
            session_id = %opts.session_id,
            sdk_url = %opts.sdk_url,
            "bridge:session spawning child"
        );

        let mut cmd = Command::new(&self.exec_path);
        cmd.args(&args)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("THUNDERCODE_ENVIRONMENT_KIND", "bridge")
            .env(
                "THUNDERCODE_SESSION_ACCESS_TOKEN",
                &opts.access_token,
            );

        if self.sandbox {
            cmd.env("THUNDERCODE_FORCE_SANDBOX", "1");
        }
        if opts.use_ccr_v2 {
            cmd.env("THUNDERCODE_USE_CCR_V2", "1");
            if let Some(epoch) = opts.worker_epoch {
                cmd.env("THUNDERCODE_WORKER_EPOCH", epoch.to_string());
            }
        }

        let mut child = cmd.spawn()?;
        let pid = child.id();
        tracing::debug!(
            session_id = %opts.session_id,
            pid = ?pid,
            "bridge:session child spawned"
        );

        let activities: Arc<Mutex<VecDeque<SessionActivity>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(MAX_ACTIVITIES)));
        let current_activity: Arc<Mutex<Option<SessionActivity>>> =
            Arc::new(Mutex::new(None));
        let last_stderr: Arc<Mutex<VecDeque<String>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(MAX_STDERR_LINES)));

        let (done_tx, done_rx) = watch::channel(None);

        // Set up stdin forwarding.
        let stdin = child.stdin.take();
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        if let Some(mut stdin_writer) = stdin {
            tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                while let Some(data) = stdin_rx.recv().await {
                    if stdin_writer.write_all(data.as_bytes()).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Parse NDJSON from child stdout.
        let stdout = child.stdout.take();
        let activities_clone = Arc::clone(&activities);
        let current_activity_clone = Arc::clone(&current_activity);
        let session_id = opts.session_id.clone();

        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            tokio::spawn(async move {
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Try to extract activity from the NDJSON line.
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(activity) =
                            extract_activity(&parsed, &session_id)
                        {
                            let mut acts = activities_clone.lock().await;
                            if acts.len() >= MAX_ACTIVITIES {
                                acts.pop_front();
                            }
                            acts.push_back(activity.clone());
                            *current_activity_clone.lock().await = Some(activity);
                        }
                    }
                }
            });
        }

        // Buffer stderr lines.
        let stderr = child.stderr.take();
        let last_stderr_clone = Arc::clone(&last_stderr);
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            tokio::spawn(async move {
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut buf = last_stderr_clone.lock().await;
                    if buf.len() >= MAX_STDERR_LINES {
                        buf.pop_front();
                    }
                    buf.push_back(line);
                }
            });
        }

        // Monitor child exit.
        let child_arc = Arc::new(Mutex::new(Some(child)));
        let child_monitor = Arc::clone(&child_arc);
        let session_id_monitor = opts.session_id.clone();
        tokio::spawn(async move {
            let status = {
                let mut guard = child_monitor.lock().await;
                if let Some(ref mut child) = *guard {
                    match child.wait().await {
                        Ok(exit) => {
                            if exit.success() {
                                SessionDoneStatus::Completed
                            } else {
                                // Check if it was a signal.
                                #[cfg(unix)]
                                {
                                    use std::os::unix::process::ExitStatusExt;
                                    if exit.signal().is_some() {
                                        SessionDoneStatus::Interrupted
                                    } else {
                                        SessionDoneStatus::Failed
                                    }
                                }
                                #[cfg(not(unix))]
                                {
                                    SessionDoneStatus::Failed
                                }
                            }
                        }
                        Err(_) => SessionDoneStatus::Failed,
                    }
                } else {
                    SessionDoneStatus::Failed
                }
            };

            tracing::debug!(
                session_id = %session_id_monitor,
                status = ?status,
                "bridge:session child exited"
            );
            let _ = done_tx.send(Some(status));
        });

        Ok(SessionHandle {
            session_id: opts.session_id.clone(),
            access_token: opts.access_token.clone(),
            activities,
            current_activity,
            last_stderr,
            done_rx,
            stdin_tx: Some(stdin_tx),
            child: child_arc,
        })
    }
}

/// Extract a session activity from a parsed NDJSON line.
fn extract_activity(
    parsed: &serde_json::Value,
    session_id: &str,
) -> Option<SessionActivity> {
    let msg_type = parsed.get("type")?.as_str()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    match msg_type {
        "assistant" => {
            let content = parsed.get("message")?.get("content")?.as_array()?;
            for block in content {
                let block_type = block.get("type")?.as_str()?;
                match block_type {
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("Tool");
                        let input = block
                            .get("input")
                            .cloned()
                            .unwrap_or(serde_json::json!({}));
                        let summary = tool_summary(name, &input);
                        tracing::debug!(
                            session_id,
                            tool = name,
                            "bridge:activity tool_use"
                        );
                        return Some(SessionActivity {
                            activity_type: SessionActivityType::ToolStart,
                            summary,
                            timestamp: now,
                        });
                    }
                    "text" => {
                        let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        if !text.is_empty() {
                            return Some(SessionActivity {
                                activity_type: SessionActivityType::Text,
                                summary: text.chars().take(80).collect(),
                                timestamp: now,
                            });
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        "result" => {
            let subtype = parsed.get("subtype").and_then(|s| s.as_str())?;
            if subtype == "success" {
                Some(SessionActivity {
                    activity_type: SessionActivityType::Result,
                    summary: "Session completed".to_string(),
                    timestamp: now,
                })
            } else {
                let error_summary = parsed
                    .get("errors")
                    .and_then(|e| e.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|e| e.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| format!("Error: {subtype}"));
                Some(SessionActivity {
                    activity_type: SessionActivityType::Error,
                    summary: error_summary,
                    timestamp: now,
                })
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_activity_tool_use() {
        let json = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "name": "Read",
                    "input": {"file_path": "src/main.rs"}
                }]
            }
        });
        let activity = extract_activity(&json, "test-session");
        assert!(activity.is_some());
        let a = activity.unwrap();
        assert_eq!(a.activity_type, SessionActivityType::ToolStart);
        assert!(a.summary.contains("Reading"));
        assert!(a.summary.contains("src/main.rs"));
    }

    #[test]
    fn test_extract_activity_text() {
        let json = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [{
                    "type": "text",
                    "text": "Hello, world!"
                }]
            }
        });
        let activity = extract_activity(&json, "test-session");
        assert!(activity.is_some());
        let a = activity.unwrap();
        assert_eq!(a.activity_type, SessionActivityType::Text);
        assert_eq!(a.summary, "Hello, world!");
    }

    #[test]
    fn test_extract_activity_result() {
        let json = serde_json::json!({
            "type": "result",
            "subtype": "success"
        });
        let activity = extract_activity(&json, "test-session");
        assert!(activity.is_some());
        let a = activity.unwrap();
        assert_eq!(a.activity_type, SessionActivityType::Result);
    }

    #[test]
    fn test_extract_activity_error_result() {
        let json = serde_json::json!({
            "type": "result",
            "subtype": "error",
            "errors": ["Something went wrong"]
        });
        let activity = extract_activity(&json, "test-session");
        assert!(activity.is_some());
        let a = activity.unwrap();
        assert_eq!(a.activity_type, SessionActivityType::Error);
        assert!(a.summary.contains("Something went wrong"));
    }

    #[test]
    fn test_extract_activity_unknown_type() {
        let json = serde_json::json!({"type": "user", "content": "hi"});
        assert!(extract_activity(&json, "test-session").is_none());
    }
}
