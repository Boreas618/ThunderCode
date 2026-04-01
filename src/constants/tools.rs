//! Tool name constants and per-mode tool availability sets.

// ---------------------------------------------------------------------------
// Tool names (canonical string identifiers)
// ---------------------------------------------------------------------------

pub const BASH_TOOL_NAME: &str = "Bash";
pub const FILE_READ_TOOL_NAME: &str = "Read";
pub const FILE_WRITE_TOOL_NAME: &str = "Write";
pub const FILE_EDIT_TOOL_NAME: &str = "Edit";
pub const GLOB_TOOL_NAME: &str = "Glob";
pub const GREP_TOOL_NAME: &str = "Grep";
pub const NOTEBOOK_EDIT_TOOL_NAME: &str = "NotebookEdit";
pub const WEB_SEARCH_TOOL_NAME: &str = "WebSearch";
pub const WEB_FETCH_TOOL_NAME: &str = "WebFetch";
pub const TODO_WRITE_TOOL_NAME: &str = "TodoWrite";
pub const AGENT_TOOL_NAME: &str = "Agent";
pub const SKILL_TOOL_NAME: &str = "Skill";
pub const TOOL_SEARCH_TOOL_NAME: &str = "ToolSearch";
pub const ASK_USER_QUESTION_TOOL_NAME: &str = "AskUserQuestion";
pub const TASK_CREATE_TOOL_NAME: &str = "TaskCreate";
pub const TASK_GET_TOOL_NAME: &str = "TaskGet";
pub const TASK_LIST_TOOL_NAME: &str = "TaskList";
pub const TASK_UPDATE_TOOL_NAME: &str = "TaskUpdate";
pub const TASK_OUTPUT_TOOL_NAME: &str = "TaskOutput";
pub const TASK_STOP_TOOL_NAME: &str = "TaskStop";
pub const SEND_MESSAGE_TOOL_NAME: &str = "SendMessage";
pub const ENTER_PLAN_MODE_TOOL_NAME: &str = "EnterPlanMode";
pub const EXIT_PLAN_MODE_V2_TOOL_NAME: &str = "ExitPlanModeV2";
pub const SYNTHETIC_OUTPUT_TOOL_NAME: &str = "SyntheticOutput";
pub const ENTER_WORKTREE_TOOL_NAME: &str = "EnterWorktree";
pub const EXIT_WORKTREE_TOOL_NAME: &str = "ExitWorktree";
pub const WORKFLOW_TOOL_NAME: &str = "Workflow";
pub const CRON_CREATE_TOOL_NAME: &str = "CronCreate";
pub const CRON_DELETE_TOOL_NAME: &str = "CronDelete";
pub const CRON_LIST_TOOL_NAME: &str = "CronList";
pub const SLEEP_TOOL_NAME: &str = "Sleep";

/// Shell tool names (e.g. Bash on unix, PowerShell on Windows).
pub const SHELL_TOOL_NAMES: &[&str] = &[BASH_TOOL_NAME];

// ---------------------------------------------------------------------------
// Tool-availability sets
// ---------------------------------------------------------------------------

/// Tools disallowed for ALL agent types.
pub const ALL_AGENT_DISALLOWED_TOOLS: &[&str] = &[
    TASK_OUTPUT_TOOL_NAME,
    EXIT_PLAN_MODE_V2_TOOL_NAME,
    ENTER_PLAN_MODE_TOOL_NAME,
    AGENT_TOOL_NAME,
    ASK_USER_QUESTION_TOOL_NAME,
    TASK_STOP_TOOL_NAME,
];

/// Tools allowed for async agent execution.
pub const ASYNC_AGENT_ALLOWED_TOOLS: &[&str] = &[
    FILE_READ_TOOL_NAME,
    WEB_SEARCH_TOOL_NAME,
    TODO_WRITE_TOOL_NAME,
    GREP_TOOL_NAME,
    WEB_FETCH_TOOL_NAME,
    GLOB_TOOL_NAME,
    BASH_TOOL_NAME,
    FILE_EDIT_TOOL_NAME,
    FILE_WRITE_TOOL_NAME,
    NOTEBOOK_EDIT_TOOL_NAME,
    SKILL_TOOL_NAME,
    SYNTHETIC_OUTPUT_TOOL_NAME,
    TOOL_SEARCH_TOOL_NAME,
    ENTER_WORKTREE_TOOL_NAME,
    EXIT_WORKTREE_TOOL_NAME,
];

/// Additional tools allowed for in-process teammates (not general async agents).
pub const IN_PROCESS_TEAMMATE_ALLOWED_TOOLS: &[&str] = &[
    TASK_CREATE_TOOL_NAME,
    TASK_GET_TOOL_NAME,
    TASK_LIST_TOOL_NAME,
    TASK_UPDATE_TOOL_NAME,
    SEND_MESSAGE_TOOL_NAME,
];

/// Tools allowed in coordinator mode.
pub const COORDINATOR_MODE_ALLOWED_TOOLS: &[&str] = &[
    AGENT_TOOL_NAME,
    TASK_STOP_TOOL_NAME,
    SEND_MESSAGE_TOOL_NAME,
    SYNTHETIC_OUTPUT_TOOL_NAME,
];
