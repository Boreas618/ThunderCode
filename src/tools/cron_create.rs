//! CronCreateTool -- schedule a recurring task.
//!
//! Ported from ref/tools/ScheduleCronTool/CronCreateTool.ts.
//! Creates a cron-like scheduled task that runs on an interval.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use std::sync::LazyLock;
use std::sync::Mutex;

/// A scheduled cron job.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub command: String,
    pub description: String,
    pub created_at: String,
    pub last_run: Option<String>,
    pub run_count: u64,
    pub enabled: bool,
}

/// In-memory cron store.
pub static CRON_STORE: LazyLock<Mutex<Vec<CronJob>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

pub const CRON_CREATE_TOOL_NAME: &str = "CronCreate";

pub struct CronCreateTool;

#[async_trait]
impl Tool for CronCreateTool {
    fn name(&self) -> &str {
        CRON_CREATE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("schedule a recurring cron job")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "schedule": {
                    "type": "string",
                    "description": "Interval specification: e.g., '5m' (every 5 minutes), '1h' (every hour), '30s' (every 30 seconds)"
                },
                "command": {
                    "type": "string",
                    "description": "The prompt or command to execute on each interval"
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of what this cron job does"
                }
            },
            "required": ["schedule", "command"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let schedule = input.get("schedule").and_then(|v| v.as_str()).unwrap_or("");
        if schedule.is_empty() {
            return ValidationResult::invalid("schedule must not be empty", 9);
        }
        // Validate schedule format (e.g., "5m", "1h", "30s")
        if parse_interval(schedule).is_none() {
            return ValidationResult::invalid(
                "schedule must be a valid interval like '5m', '1h', '30s', '2h30m'",
                9,
            );
        }

        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
        if command.is_empty() {
            return ValidationResult::invalid("command must not be empty", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let schedule = input
            .get("schedule")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let cron_id = format!(
            "cron_{}",
            nanoid::nanoid!(8, &nanoid::alphabet::SAFE.iter().cloned().collect::<Vec<_>>())
        );

        let job = CronJob {
            id: cron_id.clone(),
            schedule: schedule.clone(),
            command: command.clone(),
            description: description.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_run: None,
            run_count: 0,
            enabled: true,
        };

        {
            let mut store = CRON_STORE.lock().unwrap();
            store.push(job);
        }

        let interval_secs = parse_interval(&schedule).unwrap_or(600);

        Ok(ToolCallResult {
            data: serde_json::json!({
                "cronId": cron_id,
                "schedule": schedule,
                "intervalSeconds": interval_secs,
                "command": command,
                "description": description,
                "created": true,
                "message": format!("Cron job {} created: runs every {}", cron_id, schedule),
            }),
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let schedule = input.get("schedule").and_then(|v| v.as_str()).unwrap_or("");
        format!("Schedule cron: every {schedule}")
    }

    async fn prompt(&self) -> String {
        "Schedule a recurring task that runs at a specified interval.\n\
         \n\
         The schedule is an interval like '5m' (every 5 minutes), '1h' (every hour), \
         or '30s' (every 30 seconds). You can combine units: '2h30m'.\n\
         \n\
         The command is a prompt that will be executed on each interval."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "CronCreate".to_string()
    }
}

/// Parse an interval string like "5m", "1h", "30s", "2h30m" into seconds.
fn parse_interval(s: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut num_buf = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            let n: u64 = num_buf.parse().ok()?;
            num_buf.clear();
            match ch {
                's' => total += n,
                'm' => total += n * 60,
                'h' => total += n * 3600,
                'd' => total += n * 86400,
                _ => return None,
            }
        }
    }

    if !num_buf.is_empty() {
        // If no unit suffix, treat as minutes
        let n: u64 = num_buf.parse().ok()?;
        total += n * 60;
    }

    if total == 0 {
        return None;
    }
    Some(total)
}
