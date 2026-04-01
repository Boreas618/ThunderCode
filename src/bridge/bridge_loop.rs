//! The main bridge polling loop.
//!
//! Polls the environments API for work, spawns sessions, and manages
//! the session lifecycle. Implements reconnection with exponential backoff.
//!
//! Ported from ref/bridge/bridgeMain.ts` (`runBridgeLoop`).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use tokio::sync::watch;

use crate::bridge::api::BridgeApiClient;
use crate::bridge::poll::PollIntervalConfig;
use crate::bridge::runner::{SessionHandle, SessionSpawner};
use crate::bridge::types::{BridgeConfig, BridgeState, SessionDoneStatus, SpawnMode, WorkSecret};

/// Exponential backoff configuration.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial delay for connection errors.
    pub conn_initial: Duration,
    /// Maximum delay for connection errors.
    pub conn_cap: Duration,
    /// Give-up threshold for connection errors.
    pub conn_give_up: Duration,
    /// Initial delay for general (non-connection) errors.
    pub general_initial: Duration,
    /// Maximum delay for general errors.
    pub general_cap: Duration,
    /// Give-up threshold for general errors.
    pub general_give_up: Duration,
    /// SIGTERM -> SIGKILL grace period on shutdown.
    pub shutdown_grace: Duration,
    /// Base delay for stopWork retries.
    pub stop_work_base_delay: Duration,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            conn_initial: Duration::from_millis(2_000),
            conn_cap: Duration::from_millis(120_000),
            conn_give_up: Duration::from_millis(600_000),
            general_initial: Duration::from_millis(500),
            general_cap: Duration::from_millis(30_000),
            general_give_up: Duration::from_millis(600_000),
            shutdown_grace: Duration::from_secs(30),
            stop_work_base_delay: Duration::from_millis(1_000),
        }
    }
}

/// Status update interval for the live display.
pub const STATUS_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

/// Default max sessions when not configured.
pub const DEFAULT_MAX_SESSIONS: usize = 32;

/// Decode a base64url-encoded work secret.
pub fn decode_work_secret(encoded: &str) -> Result<WorkSecret, anyhow::Error> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded)?;
    let secret: WorkSecret = serde_json::from_slice(&bytes)?;
    Ok(secret)
}

/// Computes the sleep-detection threshold for the poll loop.
///
/// Must exceed the max backoff cap so normal backoff delays don't trigger
/// false sleep detection. Uses 2x the connection backoff cap.
pub fn sleep_detection_threshold(backoff: &BackoffConfig) -> Duration {
    backoff.conn_cap * 2
}

/// Format a duration in human-readable form.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// State tracking for the bridge loop.
struct BridgeLoopState {
    /// Active session handles, keyed by session ID.
    active_sessions: HashMap<String, SessionHandle>,
    /// When each session started (millis since epoch).
    session_start_times: HashMap<String, u64>,
    /// Work IDs associated with sessions.
    session_work_ids: HashMap<String, String>,
    /// Session ingress tokens for heartbeat auth.
    session_ingress_tokens: HashMap<String, String>,
    /// Work IDs that have already been completed/stopped.
    completed_work_ids: HashSet<String>,
    /// Connection error backoff state (current delay in ms).
    conn_backoff_ms: u64,
    /// General error backoff state (current delay in ms).
    general_backoff_ms: u64,
    /// When connection errors started.
    conn_error_start: Option<u64>,
    /// When general errors started.
    general_error_start: Option<u64>,
    /// Whether a fatal error has occurred.
    fatal_exit: bool,
    /// Current bridge state.
    bridge_state: BridgeState,
}

impl BridgeLoopState {
    fn new() -> Self {
        Self {
            active_sessions: HashMap::new(),
            session_start_times: HashMap::new(),
            session_work_ids: HashMap::new(),
            session_ingress_tokens: HashMap::new(),
            completed_work_ids: HashSet::new(),
            conn_backoff_ms: 0,
            general_backoff_ms: 0,
            conn_error_start: None,
            general_error_start: None,
            fatal_exit: false,
            bridge_state: BridgeState::Ready,
        }
    }

    /// Returns whether we are at session capacity.
    fn at_capacity(&self, max_sessions: usize) -> bool {
        self.active_sessions.len() >= max_sessions
    }

    /// Reset error tracking after a successful poll.
    fn reset_errors(&mut self) {
        self.conn_backoff_ms = 0;
        self.general_backoff_ms = 0;
        self.conn_error_start = None;
        self.general_error_start = None;
    }
}

/// Run the main bridge polling loop.
///
/// This is the core of bridge mode. It:
/// 1. Polls the API for work items
/// 2. Spawns sessions for new work
/// 3. Manages session lifecycle (done, failed, interrupted)
/// 4. Handles reconnection with exponential backoff
/// 5. Respects session capacity limits
///
/// The loop runs until `shutdown_rx` signals or a fatal error occurs.
pub async fn run_bridge_loop(
    config: BridgeConfig,
    environment_id: String,
    environment_secret: String,
    api: BridgeApiClient,
    spawner: SessionSpawner,
    mut shutdown_rx: watch::Receiver<bool>,
    backoff_config: BackoffConfig,
    poll_config: PollIntervalConfig,
) -> Result<(), anyhow::Error> {
    let mut state = BridgeLoopState::new();
    state.bridge_state = BridgeState::Connected;

    let max_sessions = config.max_sessions;
    let spawn_mode = config.spawn_mode;

    tracing::info!(
        spawn_mode = ?spawn_mode,
        max_sessions,
        environment_id = %environment_id,
        "bridge: starting poll loop"
    );

    loop {
        // Check for shutdown signal.
        if *shutdown_rx.borrow() {
            tracing::info!("bridge: shutdown signal received");
            break;
        }

        // Clean up completed sessions.
        let completed: Vec<String> = state
            .active_sessions
            .iter()
            .filter(|(_, h)| h.is_done())
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in completed {
            if let Some(mut handle) = state.active_sessions.remove(&session_id) {
                let status = handle.wait().await;
                let work_id = state.session_work_ids.remove(&session_id);
                state.session_start_times.remove(&session_id);
                state.session_ingress_tokens.remove(&session_id);

                tracing::info!(
                    session_id = %session_id,
                    status = ?status,
                    "bridge: session completed"
                );

                // Notify server that work is done (non-interrupted only).
                if status != SessionDoneStatus::Interrupted {
                    if let Some(ref wid) = work_id {
                        if let Err(e) = api.stop_work(&environment_id, wid, false).await {
                            tracing::warn!(
                                work_id = %wid,
                                error = %e,
                                "bridge: failed to stop work"
                            );
                        }
                        state.completed_work_ids.insert(wid.clone());
                    }
                }

                // In single-session mode, exit after the session completes.
                if spawn_mode == SpawnMode::SingleSession
                    && status != SessionDoneStatus::Interrupted
                {
                    tracing::info!("bridge: single-session mode, exiting after session done");
                    return Ok(());
                }
            }
        }

        // Poll for work.
        let poll_result = api
            .poll_for_work(
                &environment_id,
                &environment_secret,
                Some(poll_config.reclaim_older_than_ms()),
            )
            .await;

        match poll_result {
            Ok(Some(work)) => {
                state.reset_errors();

                // Skip already-completed work.
                if state.completed_work_ids.contains(&work.id) {
                    tracing::debug!(
                        work_id = %work.id,
                        "bridge: skipping already-completed work"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                // Decode work secret.
                let secret = match decode_work_secret(&work.secret) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(
                            work_id = %work.id,
                            error = %e,
                            "bridge: failed to decode work secret"
                        );
                        // Stop poisoned work to prevent re-delivery.
                        state.completed_work_ids.insert(work.id.clone());
                        let _ = api.stop_work(&environment_id, &work.id, false).await;
                        continue;
                    }
                };

                let session_id = &work.data.id;

                // Check if this is a token refresh for an existing session.
                if let Some(existing) = state.active_sessions.get_mut(session_id) {
                    tracing::debug!(
                        session_id,
                        "bridge: refreshing token for existing session"
                    );
                    existing.update_access_token(secret.session_ingress_token.clone());
                    state.session_ingress_tokens.insert(
                        session_id.to_string(),
                        secret.session_ingress_token.clone(),
                    );

                    // Acknowledge the work.
                    let _ = api
                        .acknowledge_work(
                            &environment_id,
                            &work.id,
                            &secret.session_ingress_token,
                        )
                        .await;
                    continue;
                }

                // Check capacity.
                if state.at_capacity(max_sessions) {
                    tracing::debug!(
                        active = state.active_sessions.len(),
                        max = max_sessions,
                        "bridge: at capacity, cannot accept new session"
                    );
                    let sleep_dur = poll_config.poll_interval(
                        state.active_sessions.len(),
                        max_sessions,
                    );
                    tokio::time::sleep(sleep_dur).await;
                    continue;
                }

                // Handle based on work type.
                match work.data.work_type.as_str() {
                    "session" => {
                        tracing::info!(
                            session_id,
                            work_id = %work.id,
                            "bridge: spawning session"
                        );

                        let sdk_url = format!(
                            "{}/v1/sessions/{}",
                            secret.api_base_url, session_id
                        );

                        let spawn_opts = crate::bridge::types::SessionSpawnOpts {
                            session_id: session_id.to_string(),
                            sdk_url,
                            access_token: secret.session_ingress_token.clone(),
                            use_ccr_v2: secret.use_code_sessions,
                            worker_epoch: None,
                        };

                        // Acknowledge the work before spawning.
                        let _ = api
                            .acknowledge_work(
                                &environment_id,
                                &work.id,
                                &secret.session_ingress_token,
                            )
                            .await;

                        match spawner.spawn(&spawn_opts, &config.dir) {
                            Ok(handle) => {
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;

                                state
                                    .active_sessions
                                    .insert(session_id.to_string(), handle);
                                state
                                    .session_start_times
                                    .insert(session_id.to_string(), now);
                                state
                                    .session_work_ids
                                    .insert(session_id.to_string(), work.id.clone());
                                state.session_ingress_tokens.insert(
                                    session_id.to_string(),
                                    secret.session_ingress_token,
                                );

                                tracing::info!(
                                    session_id,
                                    "bridge: session spawned successfully"
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    session_id,
                                    error = %e,
                                    "bridge: failed to spawn session"
                                );
                                // Stop the work item to prevent re-delivery.
                                state.completed_work_ids.insert(work.id.clone());
                                let _ = api
                                    .stop_work(&environment_id, &work.id, false)
                                    .await;
                            }
                        }
                    }
                    "healthcheck" => {
                        tracing::debug!(
                            work_id = %work.id,
                            "bridge: healthcheck received, acknowledging"
                        );
                        let _ = api
                            .acknowledge_work(
                                &environment_id,
                                &work.id,
                                &secret.session_ingress_token,
                            )
                            .await;
                    }
                    other => {
                        tracing::warn!(
                            work_type = other,
                            "bridge: unknown work type, skipping"
                        );
                    }
                }
            }
            Ok(None) => {
                // No work available -- sleep based on capacity.
                state.reset_errors();
                let sleep_dur = poll_config.poll_interval(
                    state.active_sessions.len(),
                    max_sessions,
                );
                tokio::select! {
                    _ = tokio::time::sleep(sleep_dur) => {}
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                // Handle poll errors with backoff.
                let is_connection_error = e.to_string().contains("connect")
                    || e.to_string().contains("timeout")
                    || e.to_string().contains("dns");

                if is_connection_error {
                    if state.conn_error_start.is_none() {
                        state.conn_error_start = Some(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        );
                    }
                    state.conn_backoff_ms = if state.conn_backoff_ms == 0 {
                        backoff_config.conn_initial.as_millis() as u64
                    } else {
                        (state.conn_backoff_ms * 2)
                            .min(backoff_config.conn_cap.as_millis() as u64)
                    };

                    state.bridge_state = BridgeState::Reconnecting;
                    tracing::warn!(
                        error = %e,
                        backoff_ms = state.conn_backoff_ms,
                        "bridge: connection error, backing off"
                    );

                    // Check give-up threshold.
                    if let Some(start) = state.conn_error_start {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        if now - start > backoff_config.conn_give_up.as_millis() as u64 {
                            tracing::error!("bridge: connection error budget exhausted");
                            state.fatal_exit = true;
                            state.bridge_state = BridgeState::Failed;
                            break;
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(state.conn_backoff_ms)).await;
                } else {
                    // Check for fatal errors.
                    if let Some(fatal) = e.downcast_ref::<crate::bridge::api::BridgeFatalError>() {
                        tracing::error!(
                            status = fatal.status,
                            error_type = ?fatal.error_type,
                            "bridge: fatal error"
                        );
                        state.fatal_exit = true;
                        state.bridge_state = BridgeState::Failed;
                        break;
                    }

                    state.general_backoff_ms = if state.general_backoff_ms == 0 {
                        backoff_config.general_initial.as_millis() as u64
                    } else {
                        (state.general_backoff_ms * 2)
                            .min(backoff_config.general_cap.as_millis() as u64)
                    };

                    tracing::warn!(
                        error = %e,
                        backoff_ms = state.general_backoff_ms,
                        "bridge: general error, backing off"
                    );

                    tokio::time::sleep(Duration::from_millis(state.general_backoff_ms)).await;
                }
            }
        }
    }

    // Shutdown: kill all active sessions.
    tracing::info!(
        active = state.active_sessions.len(),
        "bridge: shutting down active sessions"
    );

    for (session_id, handle) in &state.active_sessions {
        tracing::debug!(session_id, "bridge: killing session on shutdown");
        handle.kill().await;
    }

    // Wait for sessions to exit (up to grace period).
    let grace = backoff_config.shutdown_grace;
    for (session_id, mut handle) in state.active_sessions {
        tokio::select! {
            status = handle.wait() => {
                tracing::debug!(
                    session_id,
                    status = ?status,
                    "bridge: session exited during shutdown"
                );
            }
            _ = tokio::time::sleep(grace) => {
                tracing::warn!(
                    session_id,
                    "bridge: session did not exit within grace period"
                );
            }
        }
    }

    // Deregister the environment.
    if !state.fatal_exit {
        if let Err(e) = api.deregister_environment(&environment_id).await {
            tracing::warn!(
                error = %e,
                "bridge: failed to deregister environment"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m");
    }

    #[test]
    fn test_decode_work_secret() {
        use base64::Engine;
        let secret = serde_json::json!({
            "version": 1,
            "session_ingress_token": "test-token",
            "api_base_url": "https://api.example.com"
        });
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_string(&secret).unwrap());
        let decoded = decode_work_secret(&encoded).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.session_ingress_token, "test-token");
        assert_eq!(decoded.api_base_url, "https://api.example.com");
        assert!(!decoded.use_code_sessions);
    }

    #[test]
    fn test_backoff_config_defaults() {
        let config = BackoffConfig::default();
        assert_eq!(config.conn_initial, Duration::from_millis(2_000));
        assert_eq!(config.conn_cap, Duration::from_millis(120_000));
        assert_eq!(config.conn_give_up, Duration::from_millis(600_000));
        assert_eq!(config.shutdown_grace, Duration::from_secs(30));
    }

    #[test]
    fn test_sleep_detection_threshold() {
        let config = BackoffConfig::default();
        let threshold = sleep_detection_threshold(&config);
        assert_eq!(threshold, Duration::from_millis(240_000));
    }

    #[test]
    fn test_bridge_loop_state_capacity() {
        let state = BridgeLoopState::new();
        assert!(!state.at_capacity(4));
        // Can't easily test with real SessionHandles, but the logic is clear.
    }
}
