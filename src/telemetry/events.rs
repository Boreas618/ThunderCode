//! Telemetry event logging.
//!
//! Ported from ref/utils/telemetry/events.ts.
//! Events are emitted via `tracing` so they can be picked up by any
//! subscriber (console, file, OpenTelemetry, etc.).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TelemetryEvent
// ---------------------------------------------------------------------------

/// Enumeration of all telemetry events emitted by ThunderCode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TelemetryEvent {
    /// A new session has started.
    #[serde(rename = "session_start")]
    SessionStart {
        session_id: String,
        model: String,
    },

    /// A session has ended.
    #[serde(rename = "session_end")]
    SessionEnd {
        session_id: String,
        duration_ms: u64,
        cost_usd: f64,
    },

    /// A tool was invoked.
    #[serde(rename = "tool_use")]
    ToolUse {
        tool_name: String,
        duration_ms: u64,
        success: bool,
    },

    /// An API call was made to the model provider.
    #[serde(rename = "api_call")]
    ApiCall {
        model: String,
        input_tokens: u64,
        output_tokens: u64,
        duration_ms: u64,
    },

    /// An error occurred.
    #[serde(rename = "error")]
    Error {
        error_type: String,
        message: String,
    },

    /// A feature was used (e.g. "compact_mode", "voice_input").
    #[serde(rename = "feature_used")]
    FeatureUsed {
        feature: String,
    },
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

/// Log a telemetry event.
///
/// Events are emitted as structured `tracing` events at the INFO level so
/// any configured subscriber can capture them.  The event body is serialised
/// to JSON for easy downstream consumption.
pub fn log_event(event: TelemetryEvent) {
    match &event {
        TelemetryEvent::SessionStart { session_id, model } => {
            tracing::info!(
                telemetry_type = "session_start",
                session_id = %session_id,
                model = %model,
                "telemetry event"
            );
        }
        TelemetryEvent::SessionEnd {
            session_id,
            duration_ms,
            cost_usd,
        } => {
            tracing::info!(
                telemetry_type = "session_end",
                session_id = %session_id,
                duration_ms = duration_ms,
                cost_usd = cost_usd,
                "telemetry event"
            );
        }
        TelemetryEvent::ToolUse {
            tool_name,
            duration_ms,
            success,
        } => {
            tracing::info!(
                telemetry_type = "tool_use",
                tool_name = %tool_name,
                duration_ms = duration_ms,
                success = success,
                "telemetry event"
            );
        }
        TelemetryEvent::ApiCall {
            model,
            input_tokens,
            output_tokens,
            duration_ms,
        } => {
            tracing::info!(
                telemetry_type = "api_call",
                model = %model,
                input_tokens = input_tokens,
                output_tokens = output_tokens,
                duration_ms = duration_ms,
                "telemetry event"
            );
        }
        TelemetryEvent::Error {
            error_type,
            message,
        } => {
            tracing::warn!(
                telemetry_type = "error",
                error_type = %error_type,
                message = %message,
                "telemetry event"
            );
        }
        TelemetryEvent::FeatureUsed { feature } => {
            tracing::info!(
                telemetry_type = "feature_used",
                feature = %feature,
                "telemetry event"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization_roundtrip() {
        let events = vec![
            TelemetryEvent::SessionStart {
                session_id: "s123".to_string(),
                model: "primary-sonnet-4".to_string(),
            },
            TelemetryEvent::SessionEnd {
                session_id: "s123".to_string(),
                duration_ms: 60_000,
                cost_usd: 0.05,
            },
            TelemetryEvent::ToolUse {
                tool_name: "bash".to_string(),
                duration_ms: 500,
                success: true,
            },
            TelemetryEvent::ApiCall {
                model: "primary-sonnet-4".to_string(),
                input_tokens: 1000,
                output_tokens: 500,
                duration_ms: 2000,
            },
            TelemetryEvent::Error {
                error_type: "api_error".to_string(),
                message: "rate limited".to_string(),
            },
            TelemetryEvent::FeatureUsed {
                feature: "compact_mode".to_string(),
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).expect("serialize");
            let back: TelemetryEvent = serde_json::from_str(&json).expect("deserialize");
            // Verify round-trip produces identical JSON
            let json2 = serde_json::to_string(&back).expect("re-serialize");
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn test_log_event_does_not_panic() {
        // Smoke test -- just make sure log_event does not panic for each variant.
        log_event(TelemetryEvent::SessionStart {
            session_id: "test".to_string(),
            model: "m".to_string(),
        });
        log_event(TelemetryEvent::SessionEnd {
            session_id: "test".to_string(),
            duration_ms: 0,
            cost_usd: 0.0,
        });
        log_event(TelemetryEvent::ToolUse {
            tool_name: "t".to_string(),
            duration_ms: 0,
            success: false,
        });
        log_event(TelemetryEvent::ApiCall {
            model: "m".to_string(),
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: 0,
        });
        log_event(TelemetryEvent::Error {
            error_type: "e".to_string(),
            message: "msg".to_string(),
        });
        log_event(TelemetryEvent::FeatureUsed {
            feature: "f".to_string(),
        });
    }
}
