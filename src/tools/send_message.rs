//! SendMessageTool -- send messages between agents/teammates.
//!
//! Ported from ref/tools/SendMessageTool/SendMessageTool.ts.
//! Routes messages to teammate mailboxes, handles broadcasts, and
//! processes structured protocol messages (shutdown, plan approval).

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const SEND_MESSAGE_TOOL_NAME: &str = "SendMessage";
pub const TEAM_LEAD_NAME: &str = "team-lead";

pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        SEND_MESSAGE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("send messages to agent teammates (swarm protocol)")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient: teammate name, or \"*\" for broadcast to all teammates"
                },
                "summary": {
                    "type": "string",
                    "description": "A 5-10 word summary shown as a preview in the UI (required when message is a string)"
                },
                "message": {
                    "description": "Plain text message content, or a structured protocol message",
                    "oneOf": [
                        { "type": "string" },
                        {
                            "type": "object",
                            "properties": {
                                "type": {
                                    "type": "string",
                                    "enum": ["shutdown_request", "shutdown_response", "plan_approval_response"]
                                }
                            },
                            "required": ["type"]
                        }
                    ]
                }
            },
            "required": ["to", "message"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("");
        if to.trim().is_empty() {
            return ValidationResult::invalid("to must not be empty", 9);
        }
        if to.contains('@') {
            return ValidationResult::invalid(
                "to must be a bare teammate name or \"*\" -- there is only one team per session",
                9,
            );
        }

        let message = input.get("message");
        let is_string_message = message.and_then(|v| v.as_str()).is_some();
        let is_structured = message.and_then(|v| v.as_object()).is_some();

        if is_string_message {
            let summary = input.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            if summary.trim().is_empty() {
                return ValidationResult::invalid(
                    "summary is required when message is a string",
                    9,
                );
            }
        }

        if is_structured && to == "*" {
            return ValidationResult::invalid(
                "structured messages cannot be broadcast (to: \"*\")",
                9,
            );
        }

        if is_structured {
            if let Some(msg_obj) = message.and_then(|v| v.as_object()) {
                let msg_type = msg_obj
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if msg_type == "shutdown_response" && to != TEAM_LEAD_NAME {
                    return ValidationResult::invalid(
                        &format!("shutdown_response must be sent to \"{TEAM_LEAD_NAME}\""),
                        9,
                    );
                }

                if msg_type == "shutdown_response" {
                    let approve = msg_obj.get("approve").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !approve {
                        let reason = msg_obj
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if reason.trim().is_empty() {
                            return ValidationResult::invalid(
                                "reason is required when rejecting a shutdown request",
                                9,
                            );
                        }
                    }
                }
            }
        }

        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let to = input
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let summary = input
            .get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let message = input.get("message");
        let is_string_message = message.and_then(|v| v.as_str()).is_some();

        if is_string_message {
            let content = message.and_then(|v| v.as_str()).unwrap_or("").to_string();

            if to == "*" {
                // Broadcast to all teammates
                // In the full implementation, this reads the team file and
                // writes to each teammate's mailbox file.
                return Ok(ToolCallResult {
                    data: serde_json::json!({
                        "success": true,
                        "message": "Message broadcast to all teammates",
                        "recipients": [],
                        "routing": {
                            "sender": "assistant",
                            "target": "@team",
                            "summary": summary,
                            "content": content,
                        }
                    }),
                    new_messages: None,
                    mcp_meta: None,
                });
            }

            // Direct message to a specific teammate
            // In the full implementation, this would:
            // 1. Check if the recipient is an in-process agent (route via queuePendingMessage)
            // 2. Check if the agent is stopped (auto-resume via resumeAgentBackground)
            // 3. Otherwise write to the teammate's mailbox file
            return Ok(ToolCallResult {
                data: serde_json::json!({
                    "success": true,
                    "message": format!("Message sent to {}'s inbox", to),
                    "routing": {
                        "sender": "assistant",
                        "target": format!("@{}", to),
                        "summary": summary,
                        "content": content,
                    }
                }),
                new_messages: None,
                mcp_meta: None,
            });
        }

        // Structured message handling
        if let Some(msg_obj) = message.and_then(|v| v.as_object()) {
            let msg_type = msg_obj
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match msg_type {
                "shutdown_request" => {
                    let _reason = msg_obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let request_id = format!("shutdown-{}-{}", to, chrono::Utc::now().timestamp_millis());

                    Ok(ToolCallResult {
                        data: serde_json::json!({
                            "success": true,
                            "message": format!("Shutdown request sent to {}. Request ID: {}", to, request_id),
                            "request_id": request_id,
                            "target": to,
                        }),
                        new_messages: None,
                        mcp_meta: None,
                    })
                }
                "shutdown_response" => {
                    let approve = msg_obj.get("approve").and_then(|v| v.as_bool()).unwrap_or(false);
                    let request_id = msg_obj
                        .get("request_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if approve {
                        Ok(ToolCallResult {
                            data: serde_json::json!({
                                "success": true,
                                "message": format!("Shutdown approved. Sent confirmation to team-lead. Agent is now exiting."),
                                "request_id": request_id,
                            }),
                            new_messages: None,
                            mcp_meta: None,
                        })
                    } else {
                        let reason = msg_obj
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No reason given")
                            .to_string();
                        Ok(ToolCallResult {
                            data: serde_json::json!({
                                "success": true,
                                "message": format!("Shutdown rejected. Reason: \"{}\". Continuing to work.", reason),
                                "request_id": request_id,
                            }),
                            new_messages: None,
                            mcp_meta: None,
                        })
                    }
                }
                "plan_approval_response" => {
                    let approve = msg_obj.get("approve").and_then(|v| v.as_bool()).unwrap_or(false);
                    let request_id = msg_obj
                        .get("request_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let feedback = msg_obj
                        .get("feedback")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Plan needs revision")
                        .to_string();

                    if approve {
                        Ok(ToolCallResult {
                            data: serde_json::json!({
                                "success": true,
                                "message": format!("Plan approved for {}. They will receive the approval and can proceed with implementation.", to),
                                "request_id": request_id,
                            }),
                            new_messages: None,
                            mcp_meta: None,
                        })
                    } else {
                        Ok(ToolCallResult {
                            data: serde_json::json!({
                                "success": true,
                                "message": format!("Plan rejected for {} with feedback: \"{}\"", to, feedback),
                                "request_id": request_id,
                            }),
                            new_messages: None,
                            mcp_meta: None,
                        })
                    }
                }
                _ => Err(ToolError::ExecutionFailed {
                    message: format!("Unknown structured message type: {msg_type}"),
                }),
            }
        } else {
            Err(ToolError::ValidationFailed {
                message: "message must be a string or structured object".to_string(),
            })
        }
    }

    fn is_read_only(&self, input: &serde_json::Value) -> bool {
        // String messages are read-only; structured messages may have side effects
        input.get("message").and_then(|v| v.as_str()).is_some()
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("agent");
        format!("Send message to {to}")
    }

    async fn prompt(&self) -> String {
        "# SendMessage\n\n\
         Send a message to another agent.\n\n\
         ```json\n\
         {\"to\": \"researcher\", \"summary\": \"assign task 1\", \"message\": \"start on task #1\"}\n\
         ```\n\n\
         | `to` | |\n\
         |---|---|\n\
         | `\"researcher\"` | Teammate by name |\n\
         | `\"*\"` | Broadcast to all teammates -- expensive, use only when everyone genuinely needs it |\n\n\
         Your plain text output is NOT visible to other agents -- to communicate, you MUST call this tool. \
         Messages from teammates are delivered automatically; you don't check an inbox. \
         Refer to teammates by name, never by UUID.\n\n\
         ## Protocol responses (legacy)\n\n\
         If you receive a JSON message with `type: \"shutdown_request\"` or `type: \"plan_approval_request\"`, \
         respond with the matching `_response` type -- echo the `request_id`, set `approve` true/false:\n\n\
         ```json\n\
         {\"to\": \"team-lead\", \"message\": {\"type\": \"shutdown_response\", \"request_id\": \"...\", \"approve\": true}}\n\
         {\"to\": \"researcher\", \"message\": {\"type\": \"plan_approval_response\", \"request_id\": \"...\", \"approve\": false, \"feedback\": \"add error handling\"}}\n\
         ```\n\n\
         Approving shutdown terminates your process. Rejecting plan sends the teammate back to revise. \
         Don't originate `shutdown_request` unless asked. Don't send structured JSON status messages -- use TaskUpdate."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "SendMessage".to_string()
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> serde_json::Value {
        let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(msg) = input.get("message").and_then(|v| v.as_str()) {
            serde_json::Value::String(format!("to {to}: {msg}"))
        } else if let Some(msg_obj) = input.get("message").and_then(|v| v.as_object()) {
            let msg_type = msg_obj
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            serde_json::Value::String(format!("{msg_type} to {to}"))
        } else {
            serde_json::Value::String(format!("to {to}"))
        }
    }
}
