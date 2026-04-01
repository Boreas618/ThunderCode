//! Git worktree management.
//!
//! Provides creation, removal, and listing of git worktrees.  Because
//! libgit2 has limited worktree support, we shell out to the git CLI for
//! create/remove and use a combination of git2 + CLI for listing.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single git worktree entry.
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Filesystem path of the worktree.
    pub path: PathBuf,
    /// Branch checked out in the worktree.
    pub branch: String,
    /// HEAD commit hash.
    pub head: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Create a new worktree for `branch` at `target_path`.
///
/// If the branch does not exist locally it is created from HEAD.  The git
/// CLI is used because libgit2's worktree API does not handle branch
/// creation well.
pub fn create_worktree(
    repo_path: &Path,
    branch: &str,
    target_path: &Path,
) -> Result<Worktree> {
    // Try to create a worktree for an existing branch first.
    let output = Command::new("git")
        .args(["worktree", "add", "--force"])
        .arg(target_path)
        .arg(branch)
        .current_dir(repo_path)
        .output()
        .context("failed to execute git worktree add")?;

    if !output.status.success() {
        // The branch might not exist yet -- try creating it.
        let output2 = Command::new("git")
            .args(["worktree", "add", "-b", branch])
            .arg(target_path)
            .current_dir(repo_path)
            .output()
            .context("failed to execute git worktree add -b")?;

        if !output2.status.success() {
            let stderr = String::from_utf8_lossy(&output2.stderr);
            bail!("git worktree add failed: {}", stderr.trim());
        }
    }

    // Read the HEAD of the newly created worktree.
    let head = read_head_at(target_path)?;
    Ok(Worktree {
        path: target_path.to_path_buf(),
        branch: branch.to_string(),
        head,
    })
}

/// Remove the worktree at `worktree_path`.
pub fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .current_dir(repo_path)
        .output()
        .context("failed to execute git worktree remove")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree remove failed: {}", stderr.trim());
    }

    Ok(())
}

/// List all worktrees attached to the repository.
pub fn list_worktrees(repo_path: &Path) -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .context("failed to execute git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git worktree list failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_worktree_list(&stdout)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read the HEAD commit hash at a given path by running `git rev-parse HEAD`.
fn read_head_at(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .context("failed to read HEAD")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Parse the porcelain output of `git worktree list --porcelain`.
///
/// Each worktree block looks like:
/// ```text
/// worktree /path/to/worktree
/// HEAD <sha>
/// branch refs/heads/main
///
/// ```
/// Bare / detached entries may lack a `branch` line.
fn parse_worktree_list(text: &str) -> Result<Vec<Worktree>> {
    let mut worktrees = Vec::new();

    let mut path: Option<PathBuf> = None;
    let mut head = String::new();
    let mut branch = String::new();

    for line in text.lines() {
        if line.is_empty() {
            // End of a block -- flush.
            if let Some(p) = path.take() {
                worktrees.push(Worktree {
                    path: p,
                    branch: branch.clone(),
                    head: head.clone(),
                });
            }
            head.clear();
            branch.clear();
            continue;
        }

        if let Some(rest) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            head = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            // Strip "refs/heads/" prefix.
            branch = rest
                .strip_prefix("refs/heads/")
                .unwrap_or(rest)
                .to_string();
        }
        // "bare", "detached", "prunable" lines are silently skipped.
    }

    // Flush the last block if there was no trailing blank line.
    if let Some(p) = path.take() {
        worktrees.push(Worktree {
            path: p,
            branch,
            head,
        });
    }

    Ok(worktrees)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let porcelain = "\
worktree /home/user/repo
HEAD abc1234def5678abc1234def5678abc1234def567
branch refs/heads/main

worktree /home/user/repo-wt
HEAD 1234567890abcdef1234567890abcdef12345678
branch refs/heads/feature

";
        let wts = parse_worktree_list(porcelain).unwrap();
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].path, PathBuf::from("/home/user/repo"));
        assert_eq!(wts[0].branch, "main");
        assert!(wts[0].head.starts_with("abc1234"));
        assert_eq!(wts[1].branch, "feature");
    }

    #[test]
    fn test_parse_worktree_bare() {
        let porcelain = "\
worktree /home/user/repo.git
HEAD abc1234def5678abc1234def5678abc1234def567
bare

";
        let wts = parse_worktree_list(porcelain).unwrap();
        assert_eq!(wts.len(), 1);
        // bare worktrees have no branch line, so branch is empty.
        assert_eq!(wts[0].branch, "");
    }

    #[test]
    fn test_parse_worktree_detached() {
        let porcelain = "\
worktree /home/user/repo
HEAD abc1234def5678abc1234def5678abc1234def567
detached

";
        let wts = parse_worktree_list(porcelain).unwrap();
        assert_eq!(wts.len(), 1);
        assert_eq!(wts[0].branch, "");
    }

    #[test]
    fn test_parse_no_trailing_newline() {
        let porcelain = "\
worktree /repo
HEAD aaa
branch refs/heads/dev";
        let wts = parse_worktree_list(porcelain).unwrap();
        assert_eq!(wts.len(), 1);
        assert_eq!(wts[0].branch, "dev");
    }
}
