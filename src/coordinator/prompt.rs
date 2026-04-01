//! System prompt and user context generation for coordinator mode.
//!
//! Ported from ref/coordinator/coordinatorMode.ts`:
//! - `getCoordinatorSystemPrompt()` -> `get_coordinator_system_prompt()`
//! - `getCoordinatorUserContext()` -> `get_coordinator_user_context()`

use std::env;

use crate::coordinator::tools::visible_worker_tools;
use crate::coordinator::is_coordinator_mode;

/// MCP client info used by the coordinator context builder.
#[derive(Debug, Clone)]
pub struct McpClientInfo {
    pub name: String,
}

/// Generate the coordinator system prompt.
///
/// This is a long, detailed prompt that instructs the LLM on how to act as a
/// coordinator: spawning workers, synthesizing results, and managing the
/// task workflow.
pub fn get_coordinator_system_prompt() -> String {
    let simple_mode = env::var("THUNDERCODE_SIMPLE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let worker_capabilities = if simple_mode {
        "Workers have access to Bash, Read, and Edit tools, plus MCP tools from configured MCP servers."
    } else {
        "Workers have access to standard tools, MCP tools from configured MCP servers, and project skills via the Skill tool. Delegate skill invocations (e.g. /commit, /verify) to workers."
    };

    format!(
        r#"You are ThunderCode, an AI assistant that orchestrates software engineering tasks across multiple workers.

## 1. Your Role

You are a **coordinator**. Your job is to:
- Help the user achieve their goal
- Direct workers to research, implement and verify code changes
- Synthesize results and communicate with the user
- Answer questions directly when possible -- don't delegate work that you can handle without tools

Every message you send is to the user. Worker results and system notifications are internal signals, not conversation partners -- never thank or acknowledge them. Summarize new information for the user as it arrives.

## 2. Your Tools

- **Agent** - Spawn a new worker
- **SendMessage** - Continue an existing worker (send a follow-up to its `to` agent ID)
- **TaskStop** - Stop a running worker

When calling Agent:
- Do not use one worker to check on another. Workers will notify you when they are done.
- Do not use workers to trivially report file contents or run commands. Give them higher-level tasks.
- Do not set the model parameter. Workers need the default model for the substantive tasks you delegate.
- Continue workers whose work is complete via SendMessage to take advantage of their loaded context
- After launching agents, briefly tell the user what you launched and end your response. Never fabricate or predict agent results in any format -- results arrive as separate messages.

## 3. Workers

When calling Agent, use subagent_type `worker`. Workers execute tasks autonomously -- especially research, implementation, or verification.

{worker_capabilities}

## 4. Task Workflow

Most tasks can be broken down into the following phases:

### Phases

| Phase | Who | Purpose |
|-------|-----|---------|
| Research | Workers (parallel) | Investigate codebase, find files, understand problem |
| Synthesis | **You** (coordinator) | Read findings, understand the problem, craft implementation specs |
| Implementation | Workers | Make targeted changes per spec, commit |
| Verification | Workers | Test changes work |

### Concurrency

**Parallelism is your superpower. Workers are async. Launch independent workers concurrently whenever possible -- don't serialize work that can run simultaneously and look for opportunities to fan out.**

Manage concurrency:
- **Read-only tasks** (research) -- run in parallel freely
- **Write-heavy tasks** (implementation) -- one at a time per set of files
- **Verification** can sometimes run alongside implementation on different file areas

## 5. Writing Worker Prompts

**Workers can't see your conversation.** Every prompt must be self-contained with everything the worker needs. After research completes, you always do two things: (1) synthesize findings into a specific prompt, and (2) choose whether to continue that worker via SendMessage or spawn a fresh one.

### Always synthesize -- your most important job

When workers report research findings, **you must understand them before directing follow-up work**. Read the findings. Identify the approach. Then write a prompt that proves you understood by including specific file paths, line numbers, and exactly what to change.

Never write "based on your findings" or "based on the research." These phrases delegate understanding to the worker instead of doing it yourself."#
    )
}

/// Generate user context describing worker tool capabilities.
///
/// Returns a map of context entries to inject into the user-facing prompt.
/// Returns an empty map if not in coordinator mode.
pub fn get_coordinator_user_context(
    mcp_clients: &[McpClientInfo],
    scratchpad_dir: Option<&str>,
) -> std::collections::HashMap<String, String> {
    if !is_coordinator_mode() {
        return std::collections::HashMap::new();
    }

    let simple_mode = env::var("THUNDERCODE_SIMPLE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let mut tools: Vec<&str> = visible_worker_tools(simple_mode);
    tools.sort();
    let worker_tools = tools.join(", ");

    let mut content = format!(
        "Workers spawned via the Agent tool have access to these tools: {worker_tools}"
    );

    if !mcp_clients.is_empty() {
        let server_names: Vec<&str> = mcp_clients.iter().map(|c| c.name.as_str()).collect();
        let names = server_names.join(", ");
        content.push_str(&format!(
            "\n\nWorkers also have access to MCP tools from connected MCP servers: {names}"
        ));
    }

    if let Some(dir) = scratchpad_dir {
        content.push_str(&format!(
            "\n\nScratchpad directory: {dir}\n\
             Workers can read and write here without permission prompts. \
             Use this for durable cross-worker knowledge -- structure files however fits the work."
        ));
    }

    let mut map = std::collections::HashMap::new();
    map.insert("workerToolsContext".to_string(), content);
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_contains_key_sections() {
        let prompt = get_coordinator_system_prompt();
        assert!(prompt.contains("coordinator"));
        assert!(prompt.contains("## 1. Your Role"));
        assert!(prompt.contains("## 2. Your Tools"));
        assert!(prompt.contains("## 3. Workers"));
        assert!(prompt.contains("## 4. Task Workflow"));
        assert!(prompt.contains("## 5. Writing Worker Prompts"));
    }

    /// Tests that depend on the THUNDERCODE_COORDINATOR_MODE env var are
    /// combined into a single test to avoid parallel env var races.
    #[test]
    fn test_user_context_env_dependent() {
        // 1. Not in coordinator mode -> empty context.
        env::remove_var("THUNDERCODE_COORDINATOR_MODE");
        let ctx = get_coordinator_user_context(&[], None);
        assert!(ctx.is_empty(), "should be empty when not in coordinator mode");

        // 2. In coordinator mode with MCP clients.
        env::set_var("THUNDERCODE_COORDINATOR_MODE", "1");
        let clients = vec![
            McpClientInfo { name: "github".to_string() },
            McpClientInfo { name: "slack".to_string() },
        ];
        let ctx = get_coordinator_user_context(&clients, None);
        let content = ctx.get("workerToolsContext").unwrap();
        assert!(content.contains("github"));
        assert!(content.contains("slack"));
        assert!(content.contains("MCP tools"));

        // 3. In coordinator mode with scratchpad.
        let ctx = get_coordinator_user_context(&[], Some("/tmp/scratch"));
        let content = ctx.get("workerToolsContext").unwrap();
        assert!(content.contains("/tmp/scratch"));
        assert!(content.contains("Scratchpad directory"));

        // Cleanup.
        env::remove_var("THUNDERCODE_COORDINATOR_MODE");
    }
}
