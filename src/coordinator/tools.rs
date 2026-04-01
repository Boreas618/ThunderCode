//! Tool set definitions for coordinator and worker modes.
//!
//! Mirrors the tool allowlists from the TypeScript codebase that control which
//! tools are available to async agents, in-process teammates, and the
//! coordinator itself.

/// Tools that internal worker agents should not see in the coordinator's
/// user-facing context description (they are infrastructure, not user tools).
pub const INTERNAL_WORKER_TOOLS: &[&str] = &[
    "TeamCreate",
    "TeamDelete",
    "SendMessage",
    "SyntheticOutput",
];

/// Tools available to async agents (workers spawned via the Agent tool).
///
/// This is the full set of tools a worker can use. The coordinator's user
/// context lists these (minus internal tools) so the LLM knows what
/// capabilities workers have.
pub const ASYNC_AGENT_ALLOWED_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Write",
    "Edit",
    "MultiEdit",
    "Glob",
    "Grep",
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "Skill",
    "Task",
    "SendMessage",
    "SyntheticOutput",
    "TeamCreate",
    "TeamDelete",
];

/// Tools available to in-process teammate agents.
///
/// A more restricted set than async agents since in-process teammates share
/// the coordinator's process and need tighter isolation.
pub const IN_PROCESS_TEAMMATE_ALLOWED_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Write",
    "Edit",
    "MultiEdit",
    "Glob",
    "Grep",
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "Skill",
];

/// Tools available to the coordinator itself.
///
/// The coordinator only has orchestration tools -- it delegates all file
/// system and execution work to workers.
pub const COORDINATOR_MODE_ALLOWED_TOOLS: &[&str] = &[
    "Agent",
    "SendMessage",
    "TaskStop",
    "TeamCreate",
    "TeamDelete",
];

/// Simplified tool set for `THUNDERCODE_SIMPLE` mode workers.
pub const SIMPLE_MODE_WORKER_TOOLS: &[&str] = &["Bash", "Read", "Edit"];

/// Returns whether a tool name is an internal worker tool (not shown in
/// coordinator user context).
pub fn is_internal_worker_tool(name: &str) -> bool {
    INTERNAL_WORKER_TOOLS.contains(&name)
}

/// Get the list of worker tools visible to the coordinator, filtering out
/// internal infrastructure tools.
pub fn visible_worker_tools(simple_mode: bool) -> Vec<&'static str> {
    if simple_mode {
        SIMPLE_MODE_WORKER_TOOLS.to_vec()
    } else {
        ASYNC_AGENT_ALLOWED_TOOLS
            .iter()
            .copied()
            .filter(|name| !is_internal_worker_tool(name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_tools_excluded_from_visible() {
        let visible = visible_worker_tools(false);
        for internal in INTERNAL_WORKER_TOOLS {
            assert!(
                !visible.contains(internal),
                "{internal} should not be in visible tools"
            );
        }
    }

    #[test]
    fn test_simple_mode_tools() {
        let tools = visible_worker_tools(true);
        assert_eq!(tools, vec!["Bash", "Read", "Edit"]);
    }

    #[test]
    fn test_coordinator_tools_are_orchestration_only() {
        // Coordinator should not have any file system tools
        for tool in COORDINATOR_MODE_ALLOWED_TOOLS {
            assert!(
                !["Bash", "Read", "Write", "Edit", "Grep", "Glob"].contains(tool),
                "Coordinator should not have file system tool: {tool}"
            );
        }
    }
}
