//! SkillTool -- invoke named skills (slash commands).
//!
//! Ported from ref/tools/SkillTool/SkillTool.ts.
//! Skills are specialized prompt-based capabilities that can be invoked
//! by name. When a skill is invoked, its prompt is expanded and executed
//! as a message within the conversation.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const SKILL_TOOL_NAME: &str = "Skill";

pub struct SkillTool;

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        SKILL_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn always_load(&self) -> bool {
        true
    }

    fn is_read_only(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("invoke named skill or slash command")
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The skill name. E.g., \"commit\", \"review-pr\", or \"pdf\""
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments for the skill"
                }
            },
            "required": ["skill"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let skill = input.get("skill").and_then(|v| v.as_str()).unwrap_or("");
        if skill.trim().is_empty() {
            return ValidationResult::invalid("skill name must not be empty", 9);
        }
        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let skill_name = input
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let args = input
            .get("args")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Report progress
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = context.tool_use_id {
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::Skill(SkillToolProgress {
                        skill_name: skill_name.clone(),
                        status: "loading".to_string(),
                    }),
                });
            }
        }

        // In the full implementation, this would:
        // 1. Look up the skill by name in the commands registry
        // 2. Expand the skill's prompt template with args
        // 3. Fork the conversation context
        // 4. Execute the expanded prompt via the query engine
        // 5. Return the result
        //
        // The skill lookup supports:
        // - Simple names: "commit", "review-pr"
        // - Fully qualified names: "ms-office-suite:pdf"
        // - Prefix matching for disambiguation

        // Check if this looks like a built-in CLI command
        let builtin_commands = ["help", "clear", "exit", "quit", "logout", "login",
                                "status", "config", "model", "compact", "resume"];
        if builtin_commands.contains(&skill_name.to_lowercase().as_str()) {
            return Err(ToolError::ExecutionFailed {
                message: format!(
                    "\"{}\" is a built-in CLI command, not a skill. Do not use this tool for built-in CLI commands.",
                    skill_name
                ),
            });
        }

        // Report execution progress
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = context.tool_use_id {
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::Skill(SkillToolProgress {
                        skill_name: skill_name.clone(),
                        status: "executing".to_string(),
                    }),
                });
            }
        }

        // The skill result includes the expanded prompt that the caller
        // (query engine) should inject into the conversation as a user message.
        let expanded_prompt = if args.is_empty() {
            format!("Execute the skill: /{skill_name}")
        } else {
            format!("Execute the skill: /{skill_name} {args}")
        };

        Ok(ToolCallResult {
            data: serde_json::json!({
                "skill": skill_name,
                "args": args,
                "status": "invoked",
                "expanded_prompt": expanded_prompt,
            }),
            // The expanded prompt becomes a new user message that drives
            // the next turn of the conversation.
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
        let skill = input
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        format!("Run skill: {skill}")
    }

    async fn prompt(&self) -> String {
        "Execute a skill within the main conversation\n\n\
         When users ask you to perform tasks, check if any of the available skills match. \
         Skills provide specialized capabilities and domain knowledge.\n\n\
         When users reference a \"slash command\" or \"/<something>\" (e.g., \"/commit\", \
         \"/review-pr\"), they are referring to a skill. Use this tool to invoke it.\n\n\
         How to invoke:\n\
         - Use this tool with the skill name and optional arguments\n\
         - Examples:\n\
           - `skill: \"pdf\"` - invoke the pdf skill\n\
           - `skill: \"commit\", args: \"-m 'Fix bug'\"` - invoke with arguments\n\
           - `skill: \"review-pr\", args: \"123\"` - invoke with arguments\n\
           - `skill: \"ms-office-suite:pdf\"` - invoke using fully qualified name\n\n\
         Important:\n\
         - Available skills are listed in system-reminder messages in the conversation\n\
         - When a skill matches the user's request, this is a BLOCKING REQUIREMENT: invoke the \
           relevant Skill tool BEFORE generating any other response about the task\n\
         - NEVER mention a skill without actually calling this tool\n\
         - Do not invoke a skill that is already running\n\
         - Do not use this tool for built-in CLI commands (like /help, /clear, etc.)\n\
         - If you see a <command-name> tag in the current conversation turn, the skill has ALREADY \
           been loaded - follow the instructions directly instead of calling this tool again"
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "Skill".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let skill = input
            .and_then(|i| i.get("skill"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        Some(format!("Running skill: /{skill}"))
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> serde_json::Value {
        let skill = input
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        serde_json::Value::String(format!("/{skill}"))
    }
}
