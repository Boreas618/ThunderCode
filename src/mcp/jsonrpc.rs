//! JSON-RPC 2.0 protocol types for MCP.
//!
//! Implements the minimal subset of JSON-RPC 2.0 needed by the Model Context
//! Protocol: requests, responses, notifications, and standard error codes.

use serde::{Deserialize, Serialize};

/// JSON-RPC version string. Always `"2.0"`.
pub const JSONRPC_VERSION: &str = "2.0";

// ============================================================================
// JsonRpcId
// ============================================================================

/// A JSON-RPC request identifier. May be a number or a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(i64),
    String(String),
}

impl JsonRpcId {
    /// Create a new numeric ID.
    pub fn number(n: i64) -> Self {
        JsonRpcId::Number(n)
    }

    /// Create a new string ID.
    pub fn string(s: impl Into<String>) -> Self {
        JsonRpcId::String(s.into())
    }
}

impl std::fmt::Display for JsonRpcId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonRpcId::Number(n) => write!(f, "{}", n),
            JsonRpcId::String(s) => write!(f, "{}", s),
        }
    }
}

// ============================================================================
// JsonRpcRequest
// ============================================================================

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub id: JsonRpcId,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request with the given method and params.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>, id: JsonRpcId) -> Self {
        JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id,
        }
    }
}

// ============================================================================
// JsonRpcNotification
// ============================================================================

/// A JSON-RPC 2.0 notification (no `id` field -- no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    /// Create a new notification.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        JsonRpcNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

// ============================================================================
// JsonRpcResponse
// ============================================================================

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: JsonRpcId,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: JsonRpcId, result: serde_json::Value) -> Self {
        JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response.
    pub fn error(id: JsonRpcId, error: JsonRpcError) -> Self {
        JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }

    /// Returns `true` if this is an error response.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Extract the result, returning an error if this is an error response.
    pub fn into_result(self) -> Result<serde_json::Value, JsonRpcError> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(self.result.unwrap_or(serde_json::Value::Null))
        }
    }
}

// ============================================================================
// JsonRpcError
// ============================================================================

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

// ============================================================================
// Standard JSON-RPC error codes
// ============================================================================

/// Standard JSON-RPC 2.0 error codes.
pub mod error_codes {
    /// Invalid JSON was received by the server.
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid request object.
    pub const INVALID_REQUEST: i32 = -32600;
    /// The method does not exist / is not available.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid method parameter(s).
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal JSON-RPC error.
    pub const INTERNAL_ERROR: i32 = -32603;
    /// MCP session expired / not found.
    pub const SESSION_NOT_FOUND: i32 = -32001;
}

impl JsonRpcError {
    /// Create a "parse error" response.
    pub fn parse_error(data: Option<serde_json::Value>) -> Self {
        JsonRpcError {
            code: error_codes::PARSE_ERROR,
            message: "Parse error".to_string(),
            data,
        }
    }

    /// Create an "invalid request" error.
    pub fn invalid_request(data: Option<serde_json::Value>) -> Self {
        JsonRpcError {
            code: error_codes::INVALID_REQUEST,
            message: "Invalid Request".to_string(),
            data,
        }
    }

    /// Create a "method not found" error.
    pub fn method_not_found(data: Option<serde_json::Value>) -> Self {
        JsonRpcError {
            code: error_codes::METHOD_NOT_FOUND,
            message: "Method not found".to_string(),
            data,
        }
    }

    /// Create an "invalid params" error.
    pub fn invalid_params(data: Option<serde_json::Value>) -> Self {
        JsonRpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Invalid params".to_string(),
            data,
        }
    }

    /// Create an "internal error" response.
    pub fn internal_error(msg: impl Into<String>) -> Self {
        JsonRpcError {
            code: error_codes::INTERNAL_ERROR,
            message: msg.into(),
            data: None,
        }
    }
}

// ============================================================================
// JsonRpcMessage (union type for parsing incoming messages)
// ============================================================================

/// Any JSON-RPC 2.0 message: request, notification, or response.
///
/// Used for parsing incoming data from a transport where the message type
/// is not known ahead of time.
///
/// Discrimination logic:
/// - If `result` or `error` is present, it's a **Response**.
/// - If `method` is present and `id` is present, it's a **Request**.
/// - If `method` is present but `id` is absent, it's a **Notification**.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Response(JsonRpcResponse),
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

impl<'de> Deserialize<'de> for JsonRpcMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: serde_json::Value = serde_json::Value::deserialize(deserializer)?;
        let obj = raw
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("JSON-RPC message must be an object"))?;

        // A response has `result` or `error`.
        if obj.contains_key("result") || obj.contains_key("error") {
            let resp: JsonRpcResponse = serde_json::from_value(raw)
                .map_err(serde::de::Error::custom)?;
            return Ok(JsonRpcMessage::Response(resp));
        }

        // A request or notification must have `method`.
        if obj.contains_key("method") {
            if obj.contains_key("id") {
                let req: JsonRpcRequest = serde_json::from_value(raw)
                    .map_err(serde::de::Error::custom)?;
                return Ok(JsonRpcMessage::Request(req));
            } else {
                let notif: JsonRpcNotification = serde_json::from_value(raw)
                    .map_err(serde::de::Error::custom)?;
                return Ok(JsonRpcMessage::Notification(notif));
            }
        }

        Err(serde::de::Error::custom(
            "JSON-RPC message must contain 'result', 'error', or 'method'",
        ))
    }
}

// ============================================================================
// ID generator
// ============================================================================

/// Atomic counter for generating sequential JSON-RPC request IDs.
static NEXT_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

/// Generate a new unique numeric JSON-RPC request ID.
pub fn next_request_id() -> JsonRpcId {
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    JsonRpcId::Number(id)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::json!({"capabilities": {}})),
            JsonRpcId::Number(1),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
        assert!(json.contains("\"id\":1"));

        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.method, "initialize");
        assert_eq!(parsed.id, JsonRpcId::Number(1));
    }

    #[test]
    fn test_request_without_params() {
        let req = JsonRpcRequest::new("ping", None, JsonRpcId::Number(42));
        let json = serde_json::to_string(&req).unwrap();
        // params should be omitted entirely
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn test_response_success_serialization() {
        let resp = JsonRpcResponse::success(
            JsonRpcId::Number(1),
            serde_json::json!({"tools": []}),
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));

        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert!(!parsed.is_error());
        assert!(parsed.into_result().is_ok());
    }

    #[test]
    fn test_response_error_serialization() {
        let resp = JsonRpcResponse::error(
            JsonRpcId::Number(1),
            JsonRpcError::method_not_found(None),
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(!json.contains("\"result\""));

        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_error());
        let err = parsed.into_result().unwrap_err();
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
    }

    #[test]
    fn test_notification_serialization() {
        let notif = JsonRpcNotification::new(
            "notifications/initialized",
            None,
        );
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("\"method\":\"notifications/initialized\""));
        // Notifications must NOT have an id field
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn test_id_types() {
        let num_id = JsonRpcId::Number(42);
        let str_id = JsonRpcId::String("abc-123".to_string());

        let num_json = serde_json::to_string(&num_id).unwrap();
        assert_eq!(num_json, "42");

        let str_json = serde_json::to_string(&str_id).unwrap();
        assert_eq!(str_json, "\"abc-123\"");

        // Round-trip
        let parsed_num: JsonRpcId = serde_json::from_str(&num_json).unwrap();
        assert_eq!(parsed_num, num_id);

        let parsed_str: JsonRpcId = serde_json::from_str(&str_json).unwrap();
        assert_eq!(parsed_str, str_id);
    }

    #[test]
    fn test_error_codes() {
        let err = JsonRpcError::parse_error(None);
        assert_eq!(err.code, -32700);

        let err = JsonRpcError::invalid_request(None);
        assert_eq!(err.code, -32600);

        let err = JsonRpcError::method_not_found(None);
        assert_eq!(err.code, -32601);

        let err = JsonRpcError::invalid_params(None);
        assert_eq!(err.code, -32602);

        let err = JsonRpcError::internal_error("boom");
        assert_eq!(err.code, -32603);
        assert_eq!(err.message, "boom");
    }

    #[test]
    fn test_message_union_parsing() {
        // Response
        let resp_json = r#"{"jsonrpc":"2.0","result":{"ok":true},"id":1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(resp_json).unwrap();
        assert!(matches!(msg, JsonRpcMessage::Response(_)));

        // Request
        let req_json = r#"{"jsonrpc":"2.0","method":"tools/list","id":2}"#;
        let msg: JsonRpcMessage = serde_json::from_str(req_json).unwrap();
        assert!(matches!(msg, JsonRpcMessage::Request(_)));

        // Notification (no id)
        let notif_json = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let msg: JsonRpcMessage = serde_json::from_str(notif_json).unwrap();
        assert!(matches!(msg, JsonRpcMessage::Notification(_)));
    }

    #[test]
    fn test_next_request_id() {
        let id1 = next_request_id();
        let id2 = next_request_id();
        // IDs should be monotonically increasing
        match (&id1, &id2) {
            (JsonRpcId::Number(a), JsonRpcId::Number(b)) => assert!(b > a),
            _ => panic!("expected numeric IDs"),
        }
    }

    #[test]
    fn test_error_with_data() {
        let err = JsonRpcError {
            code: error_codes::INVALID_PARAMS,
            message: "Missing field".to_string(),
            data: Some(serde_json::json!({"field": "name"})),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"data\""));

        let parsed: JsonRpcError = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.data.unwrap()["field"], "name");
    }

    #[test]
    fn test_full_roundtrip() {
        // Simulate a complete request-response cycle
        let req = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::json!({
                "name": "read_file",
                "arguments": {"path": "/tmp/test.txt"}
            })),
            JsonRpcId::Number(7),
        );
        let req_json = serde_json::to_string(&req).unwrap();

        // Parse it back
        let parsed_req: JsonRpcRequest = serde_json::from_str(&req_json).unwrap();
        assert_eq!(parsed_req.method, "tools/call");
        assert_eq!(parsed_req.id, JsonRpcId::Number(7));

        // Build a response
        let resp = JsonRpcResponse::success(
            parsed_req.id.clone(),
            serde_json::json!({
                "content": [{"type": "text", "text": "file contents"}]
            }),
        );
        let resp_json = serde_json::to_string(&resp).unwrap();

        let parsed_resp: JsonRpcResponse = serde_json::from_str(&resp_json).unwrap();
        assert_eq!(parsed_resp.id, parsed_req.id);
        let result = parsed_resp.into_result().unwrap();
        assert!(result["content"].is_array());
    }
}
