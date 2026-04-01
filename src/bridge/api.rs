//! Bridge API client for environment management.
//!
//! Provides registration, polling, acknowledgment, and session lifecycle
//! operations against `/v1/environments/bridge/*` endpoints.

use std::time::Duration;

use reqwest::Client;
use serde_json::json;

use crate::bridge::types::{
    BridgeConfig, PermissionResponseEvent, WorkResponse, BRIDGE_LOGIN_INSTRUCTION,
};

/// Allowlist pattern for server-provided IDs used in URL path segments.
const SAFE_ID_PATTERN: &str = r"^[a-zA-Z0-9_-]+$";

/// API version header value.
const API_VERSION: &str = "2023-06-01";

/// HTTP request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Fatal bridge errors that should not be retried (auth failures, expiry, etc.).
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct BridgeFatalError {
    pub message: String,
    pub status: u16,
    /// Server-provided error type (e.g. "environment_expired").
    pub error_type: Option<String>,
}

impl BridgeFatalError {
    pub fn new(message: String, status: u16, error_type: Option<String>) -> Self {
        Self {
            message,
            status,
            error_type,
        }
    }

    /// Check if this is an expiry-related error.
    pub fn is_expired(&self) -> bool {
        is_expired_error_type(self.error_type.as_deref())
    }

    /// Check if this is a suppressible 403 permission error.
    pub fn is_suppressible_403(&self) -> bool {
        if self.status != 403 {
            return false;
        }
        self.message.contains("external_poll_sessions")
            || self.message.contains("environments:manage")
    }
}

/// Validate that a server-provided ID is safe to interpolate into a URL path.
pub fn validate_bridge_id<'a>(id: &'a str, label: &str) -> Result<&'a str, anyhow::Error> {
    let re = regex::Regex::new(SAFE_ID_PATTERN).unwrap();
    if id.is_empty() || !re.is_match(id) {
        anyhow::bail!("Invalid {label}: contains unsafe characters");
    }
    Ok(id)
}

/// Check whether an error type string indicates session/environment expiry.
pub fn is_expired_error_type(error_type: Option<&str>) -> bool {
    match error_type {
        Some(t) => t.contains("expired") || t.contains("lifetime"),
        None => false,
    }
}

/// Bridge API client for environment management and session lifecycle.
///
/// Wraps reqwest HTTP client with the bridge API protocol:
/// authentication headers, error status handling, and typed responses.
pub struct BridgeApiClient {
    http: Client,
    base_url: String,
    access_token: String,
    runner_version: String,
}

impl BridgeApiClient {
    /// Create a new bridge API client.
    pub fn new(base_url: String, access_token: String, runner_version: String) -> Self {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            base_url,
            access_token,
            runner_version,
        }
    }

    /// Build standard headers for bridge API requests.
    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.access_token).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers.insert("x-api-version", API_VERSION.parse().unwrap());
        headers.insert(
            "x-environment-runner-version",
            self.runner_version.parse().unwrap(),
        );
        headers
    }

    /// Register a bridge environment with the server.
    ///
    /// Returns the server-assigned environment ID and secret.
    pub async fn register_environment(
        &self,
        config: &BridgeConfig,
    ) -> Result<(String, String), anyhow::Error> {
        tracing::debug!(
            bridge_id = %config.bridge_id,
            "bridge:api POST /v1/environments/bridge"
        );

        let mut body = json!({
            "machine_name": config.machine_name,
            "directory": config.dir,
            "branch": config.branch,
            "git_repo_url": config.git_repo_url,
            "max_sessions": config.max_sessions,
            "metadata": { "worker_type": config.worker_type },
        });

        if let Some(ref reuse_id) = config.reuse_environment_id {
            body.as_object_mut()
                .unwrap()
                .insert("environment_id".to_string(), json!(reuse_id));
        }

        let response = self
            .http
            .post(format!("{}/v1/environments/bridge", self.base_url))
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await?;
        handle_error_status(status, &data, "Registration")?;

        let env_id = data["environment_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing environment_id in response"))?
            .to_string();
        let env_secret = data["environment_secret"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing environment_secret in response"))?
            .to_string();

        tracing::debug!(
            environment_id = %env_id,
            "bridge:api registered environment"
        );

        Ok((env_id, env_secret))
    }

    /// Poll for available work items.
    ///
    /// Returns `None` if no work is available (empty poll).
    pub async fn poll_for_work(
        &self,
        environment_id: &str,
        environment_secret: &str,
        reclaim_older_than_ms: Option<u64>,
    ) -> Result<Option<WorkResponse>, anyhow::Error> {
        validate_bridge_id(environment_id, "environmentId")?;

        let mut url = format!(
            "{}/v1/environments/{}/work/poll",
            self.base_url, environment_id
        );
        if let Some(reclaim_ms) = reclaim_older_than_ms {
            url.push_str(&format!("?reclaim_older_than_ms={reclaim_ms}"));
        }

        let mut headers = self.headers();
        // Poll uses environment secret, not OAuth token.
        headers.insert(
            "Authorization",
            format!("Bearer {environment_secret}").parse().unwrap(),
        );

        let response = self
            .http
            .get(&url)
            .headers(headers)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let text = response.text().await?;

        if text.is_empty() || text == "null" {
            return Ok(None);
        }

        let data: serde_json::Value = serde_json::from_str(&text)?;
        handle_error_status(status, &data, "Poll")?;

        let work: WorkResponse = serde_json::from_value(data)?;
        tracing::debug!(
            work_id = %work.id,
            work_type = ?work.data.work_type,
            "bridge:api poll returned work"
        );
        Ok(Some(work))
    }

    /// Acknowledge receipt of a work item.
    pub async fn acknowledge_work(
        &self,
        environment_id: &str,
        work_id: &str,
        session_token: &str,
    ) -> Result<(), anyhow::Error> {
        validate_bridge_id(environment_id, "environmentId")?;
        validate_bridge_id(work_id, "workId")?;

        tracing::debug!(work_id, "bridge:api acknowledging work");

        let mut headers = self.headers();
        headers.insert(
            "Authorization",
            format!("Bearer {session_token}").parse().unwrap(),
        );

        let response = self
            .http
            .post(format!(
                "{}/v1/environments/{}/work/{}/ack",
                self.base_url, environment_id, work_id
            ))
            .headers(headers)
            .json(&json!({}))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await.unwrap_or_default();
        handle_error_status(status, &data, "Acknowledge")?;

        Ok(())
    }

    /// Stop a work item.
    pub async fn stop_work(
        &self,
        environment_id: &str,
        work_id: &str,
        force: bool,
    ) -> Result<(), anyhow::Error> {
        validate_bridge_id(environment_id, "environmentId")?;
        validate_bridge_id(work_id, "workId")?;

        tracing::debug!(work_id, force, "bridge:api stopping work");

        let response = self
            .http
            .post(format!(
                "{}/v1/environments/{}/work/{}/stop",
                self.base_url, environment_id, work_id
            ))
            .headers(self.headers())
            .json(&json!({ "force": force }))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await.unwrap_or_default();
        handle_error_status(status, &data, "StopWork")?;

        Ok(())
    }

    /// Deregister/delete the bridge environment on graceful shutdown.
    pub async fn deregister_environment(
        &self,
        environment_id: &str,
    ) -> Result<(), anyhow::Error> {
        validate_bridge_id(environment_id, "environmentId")?;

        tracing::debug!(environment_id, "bridge:api deregistering environment");

        let response = self
            .http
            .delete(format!(
                "{}/v1/environments/bridge/{}",
                self.base_url, environment_id
            ))
            .headers(self.headers())
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await.unwrap_or_default();
        handle_error_status(status, &data, "Deregister")?;

        Ok(())
    }

    /// Send a heartbeat for an active work item, extending its lease.
    pub async fn heartbeat_work(
        &self,
        environment_id: &str,
        work_id: &str,
        session_token: &str,
    ) -> Result<HeartbeatResponse, anyhow::Error> {
        validate_bridge_id(environment_id, "environmentId")?;
        validate_bridge_id(work_id, "workId")?;

        let mut headers = self.headers();
        headers.insert(
            "Authorization",
            format!("Bearer {session_token}").parse().unwrap(),
        );

        let response = self
            .http
            .post(format!(
                "{}/v1/environments/{}/work/{}/heartbeat",
                self.base_url, environment_id, work_id
            ))
            .headers(headers)
            .json(&json!({}))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await?;
        handle_error_status(status, &data, "Heartbeat")?;

        Ok(HeartbeatResponse {
            lease_extended: data["lease_extended"].as_bool().unwrap_or(false),
            state: data["state"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
        })
    }

    /// Archive a session so it no longer appears as active on the server.
    pub async fn archive_session(&self, session_id: &str) -> Result<(), anyhow::Error> {
        validate_bridge_id(session_id, "sessionId")?;

        tracing::debug!(session_id, "bridge:api archiving session");

        let response = self
            .http
            .post(format!(
                "{}/v1/sessions/{}/archive",
                self.base_url, session_id
            ))
            .headers(self.headers())
            .json(&json!({}))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();

        // 409 = already archived (idempotent, not an error).
        if status == 409 {
            tracing::debug!(session_id, "bridge:api session already archived");
            return Ok(());
        }

        let data: serde_json::Value = response.json().await.unwrap_or_default();
        handle_error_status(status, &data, "ArchiveSession")?;

        Ok(())
    }

    /// Reconnect a session (force-stop stale workers and re-queue).
    pub async fn reconnect_session(
        &self,
        environment_id: &str,
        session_id: &str,
    ) -> Result<(), anyhow::Error> {
        validate_bridge_id(environment_id, "environmentId")?;
        validate_bridge_id(session_id, "sessionId")?;

        tracing::debug!(
            environment_id,
            session_id,
            "bridge:api reconnecting session"
        );

        let response = self
            .http
            .post(format!(
                "{}/v1/environments/{}/bridge/reconnect",
                self.base_url, environment_id
            ))
            .headers(self.headers())
            .json(&json!({ "session_id": session_id }))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await.unwrap_or_default();
        handle_error_status(status, &data, "ReconnectSession")?;

        Ok(())
    }

    /// Send a permission response event to a session.
    pub async fn send_permission_response(
        &self,
        session_id: &str,
        event: PermissionResponseEvent,
        session_token: &str,
    ) -> Result<(), anyhow::Error> {
        validate_bridge_id(session_id, "sessionId")?;

        let mut headers = self.headers();
        headers.insert(
            "Authorization",
            format!("Bearer {session_token}").parse().unwrap(),
        );

        let response = self
            .http
            .post(format!(
                "{}/v1/sessions/{}/events",
                self.base_url, session_id
            ))
            .headers(headers)
            .json(&json!({ "events": [event] }))
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let status = response.status().as_u16();
        let data: serde_json::Value = response.json().await.unwrap_or_default();
        handle_error_status(status, &data, "SendPermissionResponseEvent")?;

        Ok(())
    }
}

/// Response from the heartbeat endpoint.
#[derive(Debug, Clone)]
pub struct HeartbeatResponse {
    pub lease_extended: bool,
    pub state: String,
}

/// Map HTTP error status codes to appropriate error types.
fn handle_error_status(
    status: u16,
    data: &serde_json::Value,
    context: &str,
) -> Result<(), anyhow::Error> {
    if status == 200 || status == 204 {
        return Ok(());
    }

    let detail = extract_error_detail(data);
    let error_type = extract_error_type(data);

    match status {
        401 => Err(BridgeFatalError::new(
            format!(
                "{context}: Authentication failed (401){}. {BRIDGE_LOGIN_INSTRUCTION}",
                detail.map(|d| format!(": {d}")).unwrap_or_default()
            ),
            401,
            error_type,
        )
        .into()),
        403 => {
            let msg = if is_expired_error_type(error_type.as_deref()) {
                "Remote Control session has expired. Please restart.".to_string()
            } else {
                format!(
                    "{context}: Access denied (403){}. Check your organization permissions.",
                    detail.map(|d| format!(": {d}")).unwrap_or_default()
                )
            };
            Err(BridgeFatalError::new(msg, 403, error_type).into())
        }
        404 => Err(BridgeFatalError::new(
            detail.unwrap_or_else(|| {
                format!("{context}: Not found (404). Remote Control may not be available.")
            }),
            404,
            error_type,
        )
        .into()),
        410 => Err(BridgeFatalError::new(
            detail.unwrap_or_else(|| {
                "Remote Control session has expired. Please restart.".to_string()
            }),
            410,
            error_type.or_else(|| Some("environment_expired".to_string())),
        )
        .into()),
        429 => Err(anyhow::anyhow!(
            "{context}: Rate limited (429). Polling too frequently."
        )),
        _ => Err(anyhow::anyhow!(
            "{context}: Failed with status {status}{}",
            detail.map(|d| format!(": {d}")).unwrap_or_default()
        )),
    }
}

/// Extract a human-readable error detail from a server response.
fn extract_error_detail(data: &serde_json::Value) -> Option<String> {
    data.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(String::from)
        .or_else(|| {
            data.get("message")
                .and_then(|m| m.as_str())
                .map(String::from)
        })
}

/// Extract the error type field from a server error response.
fn extract_error_type(data: &serde_json::Value) -> Option<String> {
    data.get("error")
        .and_then(|e| e.get("type"))
        .and_then(|t| t.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bridge_id() {
        assert!(validate_bridge_id("abc-123_XYZ", "test").is_ok());
        assert!(validate_bridge_id("", "test").is_err());
        assert!(validate_bridge_id("../admin", "test").is_err());
        assert!(validate_bridge_id("foo/bar", "test").is_err());
        assert!(validate_bridge_id("foo bar", "test").is_err());
    }

    #[test]
    fn test_is_expired_error_type() {
        assert!(is_expired_error_type(Some("environment_expired")));
        assert!(is_expired_error_type(Some("session_lifetime_exceeded")));
        assert!(!is_expired_error_type(Some("unauthorized")));
        assert!(!is_expired_error_type(None));
    }

    #[test]
    fn test_handle_error_status_ok() {
        let data = serde_json::json!({});
        assert!(handle_error_status(200, &data, "Test").is_ok());
        assert!(handle_error_status(204, &data, "Test").is_ok());
    }

    #[test]
    fn test_handle_error_status_401() {
        let data = serde_json::json!({"error": {"message": "bad token"}});
        let err = handle_error_status(401, &data, "Test").unwrap_err();
        assert!(err.to_string().contains("Authentication failed"));
    }

    #[test]
    fn test_handle_error_status_429() {
        let data = serde_json::json!({});
        let err = handle_error_status(429, &data, "Test").unwrap_err();
        assert!(err.to_string().contains("Rate limited"));
    }

    #[test]
    fn test_extract_error_detail() {
        let data = serde_json::json!({"error": {"message": "not found"}});
        assert_eq!(extract_error_detail(&data), Some("not found".to_string()));

        let data = serde_json::json!({"message": "top level"});
        assert_eq!(extract_error_detail(&data), Some("top level".to_string()));

        let data = serde_json::json!({});
        assert_eq!(extract_error_detail(&data), None);
    }

    #[test]
    fn test_bridge_fatal_error_suppressible() {
        let err = BridgeFatalError::new(
            "external_poll_sessions forbidden".to_string(),
            403,
            None,
        );
        assert!(err.is_suppressible_403());

        let err = BridgeFatalError::new("real error".to_string(), 403, None);
        assert!(!err.is_suppressible_403());

        let err = BridgeFatalError::new("test".to_string(), 401, None);
        assert!(!err.is_suppressible_403());
    }
}
