//! Tool registry -- central place to discover, filter, and assemble tools.
//!
//! Ported from ref/tools.ts: getAllBaseTools, getTools, assembleToolPool.

use crate::types::permissions::ToolPermissionContext;
use crate::types::tool::{find_tool_by_name, Tool};

// ============================================================================
// ToolRegistry
// ============================================================================

/// Central registry that holds all tool instances and provides query methods.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register a single tool.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Register multiple tools at once.
    pub fn register_all(&mut self, tools: Vec<Box<dyn Tool>>) {
        self.tools.extend(tools);
    }

    /// Get a reference to all registered tools.
    pub fn all(&self) -> &[Box<dyn Tool>] {
        &self.tools
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Find a tool by name or alias.
    pub fn find_by_name(&self, name: &str) -> Option<&dyn Tool> {
        find_tool_by_name(&self.tools, name)
    }

    // ========================================================================
    // Factory methods (mirror ref/tools.ts)
    // ========================================================================

    /// Get the complete list of all built-in tools (the equivalent of
    /// `getAllBaseTools()` in TypeScript).
    ///
    /// This is the source of truth for ALL tools that could be available.
    pub fn get_all_base_tools() -> Vec<Box<dyn Tool>> {
        vec![
            // Agent / collaboration
            Box::new(crate::tools::agent::AgentTool),
            Box::new(crate::tools::task_output::TaskOutputTool),
            // Shell
            Box::new(crate::tools::bash::BashTool),
            // Search / browse
            Box::new(crate::tools::glob_tool::GlobTool),
            Box::new(crate::tools::grep::GrepTool),
            // Plan mode
            Box::new(crate::tools::exit_plan_mode::ExitPlanModeTool),
            // File operations
            Box::new(crate::tools::file_read::FileReadTool),
            Box::new(crate::tools::file_edit::FileEditTool),
            Box::new(crate::tools::file_write::FileWriteTool),
            // Notebooks
            Box::new(crate::tools::notebook_edit::NotebookEditTool),
            // Web
            Box::new(crate::tools::web_fetch::WebFetchTool),
            // Todo
            Box::new(crate::tools::todo_write::TodoWriteTool),
            // Web search
            Box::new(crate::tools::web_search::WebSearchTool),
            // Task management
            Box::new(crate::tools::task_stop::TaskStopTool),
            // User interaction
            Box::new(crate::tools::ask_user::AskUserQuestionTool),
            // Skills
            Box::new(crate::tools::skill::SkillTool),
            // Plan mode entry
            Box::new(crate::tools::enter_plan_mode::EnterPlanModeTool),
            // Task CRUD (v2 tasks)
            Box::new(crate::tools::task_create::TaskCreateTool),
            Box::new(crate::tools::task_get::TaskGetTool),
            Box::new(crate::tools::task_update::TaskUpdateTool),
            Box::new(crate::tools::task_list::TaskListTool),
            // LSP
            Box::new(crate::tools::lsp::LSPTool),
            // Worktree
            Box::new(crate::tools::enter_worktree::EnterWorktreeTool),
            Box::new(crate::tools::exit_worktree::ExitWorktreeTool),
            // Multi-agent messaging
            Box::new(crate::tools::send_message::SendMessageTool),
            // Teams
            Box::new(crate::tools::team_create::TeamCreateTool),
            Box::new(crate::tools::team_delete::TeamDeleteTool),
            // Sleep
            Box::new(crate::tools::sleep::SleepTool),
            // Cron
            Box::new(crate::tools::cron_create::CronCreateTool),
            Box::new(crate::tools::cron_delete::CronDeleteTool),
            Box::new(crate::tools::cron_list::CronListTool),
            // Brief
            Box::new(crate::tools::brief::BriefTool),
            // MCP
            Box::new(crate::tools::list_mcp_resources::ListMcpResourcesTool),
            Box::new(crate::tools::read_mcp_resource::ReadMcpResourceTool),
            // Tool search
            Box::new(crate::tools::tool_search::ToolSearchTool),
        ]
    }

    /// Get tools filtered by permission context (equivalent of `getTools()`).
    ///
    /// Filters out blanket-denied tools, disabled tools, and special tools
    /// that are added conditionally.
    pub fn get_tools(permission_context: &ToolPermissionContext) -> Vec<Box<dyn Tool>> {
        let special_tools = ["ListMcpResources", "ReadMcpResource", "SyntheticOutput"];
        let all = Self::get_all_base_tools();

        let filtered: Vec<Box<dyn Tool>> = all
            .into_iter()
            .filter(|t| !special_tools.contains(&t.name()))
            .filter(|t| !is_blanket_denied(t.as_ref(), permission_context))
            .filter(|t| t.is_enabled())
            .collect();

        filtered
    }

    /// Assemble the full tool pool from built-in + MCP tools.
    ///
    /// 1. Gets built-in tools via `get_tools()`.
    /// 2. Filters MCP tools by deny rules.
    /// 3. Deduplicates by name (built-in tools take precedence).
    pub fn assemble_tool_pool(
        permission_context: &ToolPermissionContext,
        mcp_tools: Vec<Box<dyn Tool>>,
    ) -> Vec<Box<dyn Tool>> {
        let mut builtin = Self::get_tools(permission_context);

        // Sort built-in tools by name for cache stability.
        builtin.sort_by(|a, b| a.name().cmp(b.name()));

        // Filter MCP tools by deny rules and sort.
        let mut allowed_mcp: Vec<Box<dyn Tool>> = mcp_tools
            .into_iter()
            .filter(|t| !is_blanket_denied(t.as_ref(), permission_context))
            .collect();
        allowed_mcp.sort_by(|a, b| a.name().cmp(b.name()));

        // Deduplicate: built-ins first, MCP tools added only if name is new.
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for tool in builtin.into_iter().chain(allowed_mcp) {
            if seen.insert(tool.name().to_string()) {
                result.push(tool);
            }
        }

        result
    }

    /// Build a registry pre-loaded with all base tools.
    pub fn with_all_base_tools() -> Self {
        let mut reg = Self::new();
        reg.register_all(Self::get_all_base_tools());
        reg
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Check if a tool is blanket-denied by the permission context.
///
/// A tool is blanket-denied if there is a deny rule matching its name with
/// no rule_content (i.e. all uses of that tool are denied).
fn is_blanket_denied(tool: &dyn Tool, ctx: &ToolPermissionContext) -> bool {
    for rules in ctx.always_deny_rules.values() {
        for rule_str in rules {
            // A blanket deny is just the tool name with no content qualifier.
            // The TS code uses `getDenyRuleForTool` which checks if there's a
            // deny rule whose tool_name matches and has no ruleContent.
            if rule_str == tool.name() {
                return true;
            }
        }
    }
    false
}

// ============================================================================
// Constants (exported, mirrors ref/constants/tools.ts)
// ============================================================================

/// Tools that agents (subagents) are never allowed to use.
pub const ALL_AGENT_DISALLOWED_TOOLS: &[&str] = &[
    "EnterPlanMode",
    "ExitPlanMode",
    "AskUserQuestion",
];

/// Tools disallowed for custom (user-defined) agents.
pub const CUSTOM_AGENT_DISALLOWED_TOOLS: &[&str] = &[
    "EnterPlanMode",
    "ExitPlanMode",
    "AskUserQuestion",
    "Agent",
];

/// Tools allowed for async/background agents.
pub const ASYNC_AGENT_ALLOWED_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Edit",
    "Write",
    "Glob",
    "Grep",
    "Agent",
    "TaskStop",
    "SendMessage",
];

/// Tools allowed in coordinator mode.
pub const COORDINATOR_MODE_ALLOWED_TOOLS: &[&str] = &[
    "Agent",
    "TaskStop",
    "TaskOutput",
    "SendMessage",
    "EnterPlanMode",
    "ExitPlanMode",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_is_empty() {
        let reg = ToolRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_get_all_base_tools_returns_tools() {
        let tools = ToolRegistry::get_all_base_tools();
        assert!(tools.len() >= 30, "Expected at least 30 base tools, got {}", tools.len());
    }

    #[test]
    fn test_all_tools_have_unique_names() {
        let tools = ToolRegistry::get_all_base_tools();
        let mut names = std::collections::HashSet::new();
        for tool in &tools {
            assert!(
                names.insert(tool.name().to_string()),
                "Duplicate tool name: {}",
                tool.name()
            );
        }
    }

    #[test]
    fn test_find_by_name() {
        let reg = ToolRegistry::with_all_base_tools();
        assert!(reg.find_by_name("Bash").is_some());
        assert!(reg.find_by_name("Read").is_some());
        assert!(reg.find_by_name("Edit").is_some());
        assert!(reg.find_by_name("Write").is_some());
        assert!(reg.find_by_name("Glob").is_some());
        assert!(reg.find_by_name("Grep").is_some());
        assert!(reg.find_by_name("NonExistent").is_none());
    }

    #[test]
    fn test_get_tools_filters_disabled() {
        let ctx = ToolPermissionContext::default();
        let tools = ToolRegistry::get_tools(&ctx);
        // Should have tools but fewer than all base tools (specials removed).
        let all = ToolRegistry::get_all_base_tools();
        assert!(tools.len() <= all.len());
    }

    #[test]
    fn test_assemble_tool_pool_deduplicates() {
        let ctx = ToolPermissionContext::default();
        let tools = ToolRegistry::assemble_tool_pool(&ctx, vec![]);
        let mut names: Vec<_> = tools.iter().map(|t| t.name().to_string()).collect();
        let before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(before, names.len(), "Tool pool should have unique names");
    }

    #[test]
    fn test_input_schemas_are_objects() {
        let tools = ToolRegistry::get_all_base_tools();
        for tool in &tools {
            let schema = tool.input_schema();
            assert!(
                schema.is_object(),
                "Tool {} input_schema should be a JSON object, got: {}",
                tool.name(),
                schema
            );
        }
    }

    #[test]
    fn test_all_tools_have_prompts() {
        let tools = ToolRegistry::get_all_base_tools();
        let rt = tokio::runtime::Runtime::new().unwrap();
        for tool in &tools {
            let prompt = rt.block_on(tool.prompt());
            assert!(
                !prompt.is_empty(),
                "Tool {} should have a non-empty prompt",
                tool.name()
            );
        }
    }
}
