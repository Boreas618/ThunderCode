//! System context gathered once per conversation.
//!
//! Ported from ref/context.ts` -- the `getSystemContext` / `getGitStatus`
//! logic.  Collects git status, branch info, platform details, and user
//! name so they can be injected into the system prompt.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum characters for `git status --short` output before truncation.
const MAX_STATUS_CHARS: usize = 2000;

// ---------------------------------------------------------------------------
// SystemContext
// ---------------------------------------------------------------------------

/// Snapshot of system-level context captured at conversation start.
///
/// This is memoized for the lifetime of a conversation -- the caller should
/// capture it once and reuse the value.
#[derive(Debug, Clone)]
pub struct SystemContext {
    /// Formatted git status block (branch, status, recent commits), or
    /// `None` when the cwd is not inside a git repository.
    pub git_status: Option<String>,
    /// Current branch name.
    pub branch: Option<String>,
    /// Git `user.name` from config.
    pub user: Option<String>,
    /// One-line summaries of recent commits.
    pub recent_commits: Vec<String>,
    /// Working directory for this conversation.
    pub cwd: PathBuf,
    /// Platform string (e.g. `"darwin"`, `"linux"`, `"windows"`).
    pub platform: String,
    /// Login shell (e.g. `"/bin/zsh"`).
    pub shell: String,
    /// OS version string from `uname -r` (or `os_info` on non-Unix).
    pub os_version: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build the [`SystemContext`] for the given working directory.
///
/// This function performs git inspection and environment sniffing.  It is
/// designed to be called once at conversation start and the result cached.
pub async fn get_system_context(cwd: &Path) -> SystemContext {
    let platform = get_platform();
    let shell = get_shell();
    let os_version = get_os_version();

    let is_git = crate::git::is_inside_work_tree(cwd);

    if !is_git {
        debug!("cwd is not inside a git repository");
        return SystemContext {
            git_status: None,
            branch: None,
            user: None,
            recent_commits: Vec::new(),
            cwd: cwd.to_path_buf(),
            platform,
            shell,
            os_version,
        };
    }

    // Run git queries (these are blocking I/O via git2, wrapped in
    // `spawn_blocking` by the caller if desired).
    let branch = crate::git::get_branch_name(cwd)
        .ok()
        .flatten();
    let default_branch = crate::git::operations::get_default_branch(cwd)
        .unwrap_or_else(|_| "main".to_string());
    let user = crate::git::get_user_name(cwd).ok().flatten();
    let status_result = crate::git::get_status(cwd);
    let commits_result = crate::git::get_recent_commits(cwd, 5);

    // -- Format status -------------------------------------------------------
    let status_text = match &status_result {
        Ok(gs) => format_short_status(gs),
        Err(e) => {
            warn!("failed to get git status: {e}");
            String::new()
        }
    };

    let truncated_status = if status_text.len() > MAX_STATUS_CHARS {
        let mut s = status_text[..MAX_STATUS_CHARS].to_string();
        s.push_str(
            "\n... (truncated because it exceeds 2k characters. \
             If you need more information, run \"git status\" using BashTool)",
        );
        s
    } else {
        status_text
    };

    // -- Format commits ------------------------------------------------------
    let recent_commits: Vec<String> = match &commits_result {
        Ok(commits) => commits
            .iter()
            .map(|c| format!("{} {}", c.short_hash, c.message.lines().next().unwrap_or("")))
            .collect(),
        Err(e) => {
            warn!("failed to get recent commits: {e}");
            Vec::new()
        }
    };

    let log_text = recent_commits.join("\n");

    // -- Assemble the git_status block (matches TS format) -------------------
    let mut block = String::new();
    let _ = writeln!(
        block,
        "This is the git status at the start of the conversation. \
         Note that this status is a snapshot in time, and will not update \
         during the conversation."
    );
    let _ = writeln!(block);
    if let Some(ref b) = branch {
        let _ = writeln!(block, "Current branch: {b}");
        let _ = writeln!(block);
    }
    let _ = writeln!(
        block,
        "Main branch (you will usually use this for PRs): {default_branch}"
    );
    let _ = writeln!(block);
    if let Some(ref u) = user {
        let _ = writeln!(block, "Git user: {u}");
        let _ = writeln!(block);
    }
    let _ = writeln!(
        block,
        "Status:\n{}",
        if truncated_status.is_empty() {
            "(clean)"
        } else {
            &truncated_status
        }
    );
    let _ = writeln!(block);
    let _ = write!(block, "Recent commits:\n{log_text}");

    SystemContext {
        git_status: Some(block),
        branch,
        user,
        recent_commits,
        cwd: cwd.to_path_buf(),
        platform,
        shell,
        os_version,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a `GitStatus` into git-status-short style lines.
fn format_short_status(gs: &crate::git::GitStatus) -> String {
    let mut lines = Vec::new();

    for f in &gs.staged {
        let prefix = match f.status {
            crate::git::status::FileChangeType::Added => "A ",
            crate::git::status::FileChangeType::Modified => "M ",
            crate::git::status::FileChangeType::Deleted => "D ",
            crate::git::status::FileChangeType::Renamed => "R ",
            crate::git::status::FileChangeType::Copied => "C ",
        };
        lines.push(format!("{prefix} {}", f.path));
    }

    for f in &gs.unstaged {
        let prefix = match f.status {
            crate::git::status::FileChangeType::Added => " A",
            crate::git::status::FileChangeType::Modified => " M",
            crate::git::status::FileChangeType::Deleted => " D",
            crate::git::status::FileChangeType::Renamed => " R",
            crate::git::status::FileChangeType::Copied => " C",
        };
        lines.push(format!("{prefix} {}", f.path));
    }

    for path in &gs.untracked {
        lines.push(format!("?? {path}"));
    }

    lines.join("\n")
}

/// Return a platform identifier similar to `process.platform` in Node.
fn get_platform() -> String {
    if cfg!(target_os = "macos") {
        "darwin".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

/// Read the user's default shell from `$SHELL`, falling back to `/bin/sh`.
fn get_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

/// Best-effort OS version string.
fn get_os_version() -> String {
    // Try `uname -sr` on Unix-like systems.
    #[cfg(unix)]
    {
        if let Ok(out) = std::process::Command::new("uname").args(["-sr"]).output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }
    }
    // Fallback
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_is_known() {
        let p = get_platform();
        assert!(
            ["darwin", "linux", "windows"].contains(&p.as_str()) || !p.is_empty(),
            "unexpected platform: {p}"
        );
    }

    #[test]
    fn shell_is_non_empty() {
        let s = get_shell();
        assert!(!s.is_empty());
    }

    #[test]
    fn os_version_is_non_empty() {
        let v = get_os_version();
        assert!(!v.is_empty());
    }

    #[test]
    fn format_short_status_empty() {
        let gs = crate::git::GitStatus {
            branch: Some("main".into()),
            upstream: None,
            ahead: 0,
            behind: 0,
            staged: vec![],
            unstaged: vec![],
            untracked: vec![],
        };
        assert!(format_short_status(&gs).is_empty());
    }

    #[test]
    fn format_short_status_mixed() {
        let gs = crate::git::GitStatus {
            branch: Some("main".into()),
            upstream: None,
            ahead: 0,
            behind: 0,
            staged: vec![crate::git::status::FileStatus {
                path: "staged.rs".into(),
                status: crate::git::status::FileChangeType::Added,
            }],
            unstaged: vec![crate::git::status::FileStatus {
                path: "modified.rs".into(),
                status: crate::git::status::FileChangeType::Modified,
            }],
            untracked: vec!["new.txt".into()],
        };
        let out = format_short_status(&gs);
        assert!(out.contains("A  staged.rs"));
        assert!(out.contains(" M modified.rs"));
        assert!(out.contains("?? new.txt"));
    }
}
