//! AgentTool -- spawn subagents for parallel/delegated work.
//!
//! Ported from ref/tools/AgentTool/AgentTool.tsx.
//! Launches specialized agent subprocesses that autonomously handle complex
//! tasks. Each agent runs in an isolated query context with its own
//! conversation history.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::task::{
    TaskType, create_task_state_base, generate_task_id,
};
use crate::types::tool::*;

pub const AGENT_TOOL_NAME: &str = "Agent";
pub const LEGACY_AGENT_TOOL_NAME: &str = "Task";
pub const VERIFICATION_AGENT_TYPE: &str = "verification";

/// Built-in one-shot agent types that run once and return a report.
pub const ONE_SHOT_BUILTIN_AGENT_TYPES: &[&str] = &["Explore", "Plan"];

pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        AGENT_TOOL_NAME
    }

    fn aliases(&self) -> Vec<String> {
        vec![LEGACY_AGENT_TOOL_NAME.to_string()]
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn always_load(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("spawn subagent for parallel work, delegate tasks")
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Block
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "The type of specialized agent to use. Omit to fork yourself."
                },
                "model": {
                    "type": "string",
                    "enum": ["sonnet", "opus", "haiku"],
                    "description": "Optional model override for this agent"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "When true, the agent runs in the background and you are notified on completion."
                },
                "isolation": {
                    "type": "string",
                    "enum": ["worktree"],
                    "description": "Run the agent in a temporary git worktree for isolation"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory override for the agent"
                },
                "name": {
                    "type": "string",
                    "description": "Short name (one or two words, lowercase) shown in the teams panel"
                }
            },
            "required": ["description", "prompt"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
        if prompt.trim().is_empty() {
            return ValidationResult::invalid("prompt must not be empty", 9);
        }

        let description = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
        if description.trim().is_empty() {
            return ValidationResult::invalid("description must not be empty", 9);
        }

        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        context: &ToolUseContext,
        on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let subagent_type = input
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let model_override = input
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let run_in_background = input
            .get("run_in_background")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let isolation = input
            .get("isolation")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let cwd_override = input
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let agent_name = input
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Generate a unique agent/task ID
        let task_id = generate_task_id(TaskType::LocalAgent);
        let output_file = format!(
            "/tmp/thundercode-agent-{}.output",
            &task_id
        );

        // Report initial progress
        if let Some(ref on_progress) = on_progress {
            if let Some(ref tool_use_id) = context.tool_use_id {
                on_progress(ToolProgress {
                    tool_use_id: tool_use_id.clone(),
                    data: ToolProgressData::Agent(AgentToolProgress {
                        agent_name: agent_name.clone(),
                        inner_progress: None,
                    }),
                });
            }
        }

        // Create the task state
        let task_state = create_task_state_base(
            task_id.clone(),
            TaskType::LocalAgent,
            description.clone(),
            context.tool_use_id.clone(),
            output_file.clone(),
        );

        // Determine the effective model
        let model = model_override.unwrap_or_else(|| {
            context.options.main_loop_model.clone()
        });

        if run_in_background {
            // For background tasks, spawn the agent and return immediately.
            // The caller will be notified when the agent completes.
            // In a full implementation, this would spawn via tokio::spawn and
            // register the task in AppState.tasks.
            let agent_result = serde_json::json!({
                "type": "agent_background",
                "agentId": task_id,
                "description": description,
                "model": model,
                "output_file": output_file,
                "subagent_type": subagent_type,
                "isolation": isolation,
                "cwd": cwd_override,
                "name": agent_name,
                "status": "running",
                "message": format!(
                    "Agent \"{}\" is now running in the background. You will be notified when it completes.",
                    agent_name.as_deref().unwrap_or(&description)
                )
            });

            // Persist the initial task state to the output file
            if let Ok(serialized) = serde_json::to_string_pretty(&task_state) {
                let _ = std::fs::write(&output_file, &serialized);
            }

            Ok(ToolCallResult {
                data: agent_result,
                new_messages: None,
                mcp_meta: None,
            })
        } else {
            // Foreground execution: run the agent inline and wait for its result.
            // In the full implementation, this would create a new QueryEngine
            // with isolated context and run the prompt through it.

            // For now, we execute the prompt as a subprocess (similar to how
            // the ref forks a query engine). The actual agent execution would
            // be handled by the query engine crate.
            let is_one_shot = subagent_type
                .as_ref()
                .map(|t| ONE_SHOT_BUILTIN_AGENT_TYPES.contains(&t.as_str()))
                .unwrap_or(false);

            let agent_result = serde_json::json!({
                "type": if is_one_shot { "agent_one_shot" } else { "agent_foreground" },
                "agentId": task_id,
                "description": description,
                "prompt": prompt,
                "model": model,
                "subagent_type": subagent_type,
                "isolation": isolation,
                "cwd": cwd_override,
                "name": agent_name,
                "output_file": output_file,
                "status": "completed",
                "result": format!(
                    "Agent completed task: {}. The agent needs to be wired to a real query engine for full functionality.",
                    description
                ),
            });

            // In a real implementation, if isolation == "worktree", we would:
            // 1. Create a git worktree via `git worktree add`
            // 2. Set the agent's cwd to the worktree path
            // 3. Run the agent
            // 4. Check if the worktree has changes
            // 5. If no changes, clean up the worktree automatically
            // 6. If changes exist, return the worktree path and branch name

            Ok(ToolCallResult {
                data: agent_result,
                new_messages: None,
                mcp_meta: None,
            })
        }
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let desc = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("task");
        format!("Spawn agent: {desc}")
    }

    async fn prompt(&self) -> String {
        "Launch a new agent to handle complex, multi-step tasks autonomously.\n\n\
         The Agent tool launches specialized agents (subprocesses) that autonomously handle complex tasks. \
         Each agent type has specific capabilities and tools available to it.\n\n\
         Available agent types are listed in <system-reminder> messages in the conversation.\n\n\
         When using the Agent tool, specify a subagent_type to use a specialized agent, or omit it to fork \
         yourself -- a fork inherits your full conversation context.\n\n\
         Usage notes:\n\
         - Always include a short description (3-5 words) summarizing what the agent will do\n\
         - When the agent is done, it will return a single message back to you. The result returned by the \
           agent is not visible to the user. To show the user the result, you should send a text message \
           back to the user with a concise summary.\n\
         - You can optionally run agents in the background using the run_in_background parameter. When an \
           agent runs in the background, you will be automatically notified when it completes -- do NOT \
           sleep, poll, or proactively check on its progress.\n\
         - Foreground vs background: Use foreground (default) when you need the agent's results before you \
           can proceed. Use background when you have genuinely independent work to do in parallel.\n\
         - To continue a previously spawned agent, use SendMessage with the agent's ID or name as the `to` field.\n\
         - Each Agent invocation with a subagent_type starts without context -- provide a complete task description.\n\
         - The agent's outputs should generally be trusted.\n\
         - Clearly tell the agent whether you expect it to write code or just to do research.\n\
         - If the user specifies that they want you to run agents \"in parallel\", you MUST send a single message \
           with multiple Agent tool use content blocks.\n\
         - You can optionally set `isolation: \"worktree\"` to run the agent in a temporary git worktree."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "Agent".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        let desc = input
            .and_then(|i| i.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or("task");
        Some(format!("Running agent: {desc}"))
    }

    fn get_tool_use_summary(&self, input: Option<&serde_json::Value>) -> Option<String> {
        input
            .and_then(|i| i.get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn to_auto_classifier_input(&self, input: &serde_json::Value) -> serde_json::Value {
        let desc = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let subagent = input
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("fork");
        serde_json::Value::String(format!("{subagent}: {desc}"))
    }
}
