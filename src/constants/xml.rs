//! XML tag names used in messages, terminal output wrapping, and task
//! notifications.

// Skill / command metadata
pub const COMMAND_NAME_TAG: &str = "command-name";
pub const COMMAND_MESSAGE_TAG: &str = "command-message";
pub const COMMAND_ARGS_TAG: &str = "command-args";

// Terminal / bash command input and output in user messages
pub const BASH_INPUT_TAG: &str = "bash-input";
pub const BASH_STDOUT_TAG: &str = "bash-stdout";
pub const BASH_STDERR_TAG: &str = "bash-stderr";
pub const LOCAL_COMMAND_STDOUT_TAG: &str = "local-command-stdout";
pub const LOCAL_COMMAND_STDERR_TAG: &str = "local-command-stderr";
pub const LOCAL_COMMAND_CAVEAT_TAG: &str = "local-command-caveat";

/// All terminal-related tags that indicate a message is terminal output, not a
/// user prompt.
pub const TERMINAL_OUTPUT_TAGS: &[&str] = &[
    BASH_INPUT_TAG,
    BASH_STDOUT_TAG,
    BASH_STDERR_TAG,
    LOCAL_COMMAND_STDOUT_TAG,
    LOCAL_COMMAND_STDERR_TAG,
    LOCAL_COMMAND_CAVEAT_TAG,
];

pub const TICK_TAG: &str = "tick";

// Task notifications (background task completions)
pub const TASK_NOTIFICATION_TAG: &str = "task-notification";
pub const TASK_ID_TAG: &str = "task-id";
pub const TOOL_USE_ID_TAG: &str = "tool-use-id";
pub const TASK_TYPE_TAG: &str = "task-type";
pub const OUTPUT_FILE_TAG: &str = "output-file";
pub const STATUS_TAG: &str = "status";
pub const SUMMARY_TAG: &str = "summary";
pub const REASON_TAG: &str = "reason";
pub const WORKTREE_TAG: &str = "worktree";
pub const WORKTREE_PATH_TAG: &str = "worktreePath";
pub const WORKTREE_BRANCH_TAG: &str = "worktreeBranch";

// Ultraplan mode (remote parallel planning sessions)
pub const ULTRAPLAN_TAG: &str = "ultraplan";

// Remote /review results
pub const REMOTE_REVIEW_TAG: &str = "remote-review";
pub const REMOTE_REVIEW_PROGRESS_TAG: &str = "remote-review-progress";

// Teammate / swarm inter-agent communication
pub const TEAMMATE_MESSAGE_TAG: &str = "teammate-message";

// External channel messages
pub const CHANNEL_MESSAGE_TAG: &str = "channel-message";
pub const CHANNEL_TAG: &str = "channel";

// Cross-session UDS messages
pub const CROSS_SESSION_MESSAGE_TAG: &str = "cross-session-message";

// Fork boilerplate
pub const FORK_BOILERPLATE_TAG: &str = "fork-boilerplate";
pub const FORK_DIRECTIVE_PREFIX: &str = "Your directive: ";

// Common argument patterns for slash commands that request help
pub const COMMON_HELP_ARGS: &[&str] = &["help", "-h", "--help"];

// Common argument patterns for slash commands that request current state/info
pub const COMMON_INFO_ARGS: &[&str] = &[
    "list", "show", "display", "current", "view", "get", "check", "describe",
    "print", "version", "about", "status", "?",
];
