//! API error types and classification.
//!
//! Ported from ref/services/api/errors.ts` and `ref/services/api/withRetry.ts`.

use std::time::Duration;

/// Errors returned by the the API or the HTTP transport layer.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 429 -- rate limited. May include a `Retry-After` header.
    #[error("Rate limited: {message}")]
    RateLimit {
        message: String,
        retry_after: Option<Duration>,
    },

    /// 400 -- prompt is too long for the model context window.
    #[error("Context too long: {message}")]
    ContextTooLong {
        message: String,
        max_tokens: Option<u32>,
    },

    /// 400 -- generic invalid request (bad params, duplicate tool IDs, etc.).
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// 401 / 403 -- authentication or authorization failure.
    #[error("Authentication error: {message}")]
    Authentication { message: String },

    /// 5xx -- server-side error.
    #[error("Server error ({status}): {message}")]
    ServerError { message: String, status: u16 },

    /// Transport-level failure (DNS, TLS, connection reset, etc.).
    #[error("Network error: {message}")]
    Network {
        message: String,
        #[source]
        source: reqwest::Error,
    },

    /// Request timed out before getting a response.
    #[error("Request timed out: {message}")]
    Timeout { message: String },

    /// 529 -- the API is overloaded.
    #[error("API overloaded: {message}")]
    Overloaded { message: String },
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify a raw HTTP error response into a typed `ApiError`.
///
/// `status` is the HTTP status code and `body` is the response body text.
pub fn classify_error(status: u16, body: &str) -> ApiError {
    // Try to extract the error message from the JSON body.
    let message = extract_error_message(body).unwrap_or_else(|| body.to_owned());

    match status {
        // Rate limit
        429 => {
            let retry_after = extract_retry_after_from_body(body);
            ApiError::RateLimit {
                message,
                retry_after,
            }
        }

        // Overloaded
        529 => ApiError::Overloaded { message },

        // Authentication / authorization
        401 | 403 => ApiError::Authentication { message },

        // Bad request -- further subclassify
        400 => classify_bad_request(&message, body),

        // Request entity too large
        413 => ApiError::InvalidRequest {
            message: format!("Request too large: {message}"),
        },

        // Request timeout
        408 => ApiError::Timeout { message },

        // Any other 5xx
        s if s >= 500 => ApiError::ServerError {
            message,
            status: s,
        },

        // Anything else is an invalid request
        _ => ApiError::InvalidRequest { message },
    }
}

/// Decide whether a given `ApiError` is worth retrying.
pub fn should_retry(error: &ApiError) -> bool {
    match error {
        // Rate limits and overload are always retryable.
        ApiError::RateLimit { .. } => true,
        ApiError::Overloaded { .. } => true,

        // Server errors are generally transient.
        ApiError::ServerError { status, .. } => *status >= 500,

        // Network / timeout errors are transient.
        ApiError::Network { .. } => true,
        ApiError::Timeout { .. } => true,

        // Context-too-long can sometimes be retried after compaction.
        ApiError::ContextTooLong { .. } => true,

        // Auth and generic invalid-request errors are not retryable.
        ApiError::Authentication { .. } => false,
        ApiError::InvalidRequest { .. } => false,
    }
}

/// Check whether the error body indicates a 529 overloaded error, even when
/// the SDK might not propagate the status code correctly during streaming.
pub fn is_overloaded_error(body: &str) -> bool {
    body.contains("\"type\":\"overloaded_error\"") || body.contains("overloaded_error")
}

/// Check whether the error body indicates a prompt-too-long condition.
pub fn is_prompt_too_long(body: &str) -> bool {
    let lower = body.to_lowercase();
    lower.contains("prompt is too long")
}

/// Parse "actual > limit" token counts from a prompt-too-long error message.
/// Example: "prompt is too long: 137500 tokens > 135000 maximum"
pub fn parse_prompt_too_long_tokens(raw: &str) -> Option<(u64, u64)> {
    let lower = raw.to_lowercase();
    // Look for pattern: <digits> tokens > <digits>
    let re_like = |s: &str| -> Option<(u64, u64)> {
        let idx = s.find("prompt is too long")?;
        let tail = &s[idx..];
        let mut nums = Vec::new();
        let mut current = String::new();
        for ch in tail.chars() {
            if ch.is_ascii_digit() {
                current.push(ch);
            } else {
                if !current.is_empty() {
                    if let Ok(n) = current.parse::<u64>() {
                        nums.push(n);
                    }
                    current.clear();
                }
            }
            if nums.len() >= 2 {
                break;
            }
        }
        if !current.is_empty() {
            if let Ok(n) = current.parse::<u64>() {
                nums.push(n);
            }
        }
        if nums.len() >= 2 {
            Some((nums[0], nums[1]))
        } else {
            None
        }
    };
    re_like(&lower)
}

/// Parse `max_tokens` context overflow errors.
/// Example: "input length and `max_tokens` exceed context limit: 188059 + 20000 > 200000"
pub fn parse_context_overflow(message: &str) -> Option<(u64, u64, u64)> {
    if !message.contains("input length and `max_tokens` exceed context limit") {
        return None;
    }
    // Extract: <input> + <max_tokens> > <context_limit>
    let colon_idx = message.find(':')?;
    let tail = &message[colon_idx + 1..];
    let nums: Vec<u64> = tail
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() >= 3 {
        Some((nums[0], nums[1], nums[2]))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Try to pull the `"message"` field from a JSON error body.
fn extract_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_owned())
}

/// Try to pull a `retry_after` duration from the body (some error responses
/// embed it as a JSON field rather than an HTTP header).
fn extract_retry_after_from_body(body: &str) -> Option<Duration> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("error")
        .and_then(|e| e.get("retry_after"))
        .and_then(|r| r.as_f64())
        .map(|secs| Duration::from_secs_f64(secs))
}

/// Sub-classify a 400 Bad Request into `ContextTooLong` or `InvalidRequest`.
fn classify_bad_request(message: &str, body: &str) -> ApiError {
    let lower = message.to_lowercase();

    // Prompt too long
    if lower.contains("prompt is too long") {
        let max_tokens = parse_prompt_too_long_tokens(body).map(|(_, limit)| limit as u32);
        return ApiError::ContextTooLong {
            message: message.to_owned(),
            max_tokens,
        };
    }

    // Context overflow (input + max_tokens > limit)
    if lower.contains("exceed context limit") {
        let max_tokens = parse_context_overflow(body).map(|(_, _, limit)| limit as u32);
        return ApiError::ContextTooLong {
            message: message.to_owned(),
            max_tokens,
        };
    }

    ApiError::InvalidRequest {
        message: message.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_429_rate_limit() {
        let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"Rate limit reached"}}"#;
        let err = classify_error(429, body);
        assert!(matches!(err, ApiError::RateLimit { .. }));
        assert!(should_retry(&err));
    }

    #[test]
    fn classify_529_overloaded() {
        let body = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        let err = classify_error(529, body);
        assert!(matches!(err, ApiError::Overloaded { .. }));
        assert!(should_retry(&err));
    }

    #[test]
    fn classify_401_auth() {
        let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let err = classify_error(401, body);
        assert!(matches!(err, ApiError::Authentication { .. }));
        assert!(!should_retry(&err));
    }

    #[test]
    fn classify_400_prompt_too_long() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 137500 tokens > 135000 maximum"}}"#;
        let err = classify_error(400, body);
        match &err {
            ApiError::ContextTooLong { max_tokens, .. } => {
                assert_eq!(*max_tokens, Some(135000));
            }
            other => panic!("expected ContextTooLong, got: {other:?}"),
        }
    }

    #[test]
    fn classify_400_context_overflow() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"input length and `max_tokens` exceed context limit: 188059 + 20000 > 200000"}}"#;
        let err = classify_error(400, body);
        match &err {
            ApiError::ContextTooLong { max_tokens, .. } => {
                assert_eq!(*max_tokens, Some(200000));
            }
            other => panic!("expected ContextTooLong, got: {other:?}"),
        }
    }

    #[test]
    fn classify_400_generic() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"messages: some unknown field"}}"#;
        let err = classify_error(400, body);
        assert!(matches!(err, ApiError::InvalidRequest { .. }));
        assert!(!should_retry(&err));
    }

    #[test]
    fn classify_500_server() {
        let body = r#"{"type":"error","error":{"type":"api_error","message":"Internal server error"}}"#;
        let err = classify_error(500, body);
        match &err {
            ApiError::ServerError { status, .. } => assert_eq!(*status, 500),
            other => panic!("expected ServerError, got: {other:?}"),
        }
        assert!(should_retry(&err));
    }

    #[test]
    fn classify_408_timeout() {
        let err = classify_error(408, "timeout");
        assert!(matches!(err, ApiError::Timeout { .. }));
        assert!(should_retry(&err));
    }

    #[test]
    fn is_overloaded_from_body() {
        assert!(is_overloaded_error(
            r#"{"type":"overloaded_error","message":"overloaded"}"#
        ));
        assert!(!is_overloaded_error(r#"{"type":"error"}"#));
    }

    #[test]
    fn parse_prompt_tokens() {
        let msg = "prompt is too long: 137500 tokens > 135000 maximum";
        let (actual, limit) = parse_prompt_too_long_tokens(msg).unwrap();
        assert_eq!(actual, 137500);
        assert_eq!(limit, 135000);
    }

    #[test]
    fn parse_context_overflow_values() {
        let msg =
            "input length and `max_tokens` exceed context limit: 188059 + 20000 > 200000";
        let (input, max, ctx) = parse_context_overflow(msg).unwrap();
        assert_eq!(input, 188059);
        assert_eq!(max, 20000);
        assert_eq!(ctx, 200000);
    }

    #[test]
    fn classify_non_json_body() {
        let err = classify_error(500, "plain text error");
        match &err {
            ApiError::ServerError { message, status } => {
                assert_eq!(*status, 500);
                assert_eq!(message, "plain text error");
            }
            other => panic!("expected ServerError, got: {other:?}"),
        }
    }
}
