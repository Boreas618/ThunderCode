//! ThunderCode git operations.
//!
//! Provides git repository inspection, diff parsing, blame, worktree
//! management, and common git operations.  Uses `git2` (libgit2 bindings)
//! where possible and falls back to the git CLI for operations that
//! libgit2 does not support.

pub mod blame;
pub mod diff;
pub mod operations;
pub mod status;
pub mod worktree;

// Re-export the most commonly used items at the crate root.
pub use blame::{blame_file, BlameLine};
pub use diff::{parse_diff, DiffFile, DiffHunk, DiffLine};
pub use operations::{
    add_files, commit, find_repo_root, get_ignore_patterns, get_remote_url, get_user_email,
    get_user_name, init_repo, is_inside_work_tree,
};
pub use status::{get_branch_name, get_recent_commits, get_status, CommitInfo, GitStatus};
pub use worktree::{create_worktree, list_worktrees, remove_worktree, Worktree};
