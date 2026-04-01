//! Git status operations.
//!
//! Inspects the working tree and index to report branch info, file-level
//! status (staged / unstaged / untracked), ahead/behind counts, and
//! recent commit history.  Uses `git2` exclusively -- no CLI fallback
//! needed for these operations.

use std::path::Path;

use anyhow::{Context, Result};
use git2::{Repository, StatusOptions, StatusShow};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The kind of change applied to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

/// A single file together with its change kind.
#[derive(Debug, Clone)]
pub struct FileStatus {
    pub path: String,
    pub status: FileChangeType,
}

/// Aggregated status of a git repository.
#[derive(Debug, Clone)]
pub struct GitStatus {
    /// Current branch name, or `None` for a detached HEAD.
    pub branch: Option<String>,
    /// Upstream tracking branch (e.g. `origin/main`).
    pub upstream: Option<String>,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
    /// Files staged in the index.
    pub staged: Vec<FileStatus>,
    /// Files modified in the working tree but not staged.
    pub unstaged: Vec<FileStatus>,
    /// Untracked paths.
    pub untracked: Vec<String>,
}

/// Minimal metadata for a single commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub date: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Inspect the repository at `repo_path` and return a [`GitStatus`].
pub fn get_status(repo_path: &Path) -> Result<GitStatus> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;

    // -- branch / upstream / ahead-behind --------------------------------
    let (branch, upstream, ahead, behind) = branch_info(&repo);

    // -- staged files (index vs HEAD) ------------------------------------
    let staged = collect_statuses(&repo, StatusShow::Index)?;

    // -- unstaged files (workdir vs index) -------------------------------
    let unstaged = collect_statuses(&repo, StatusShow::Workdir)?;

    // -- untracked -------------------------------------------------------
    let untracked = collect_untracked(&repo)?;

    Ok(GitStatus {
        branch,
        upstream,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
    })
}

/// Return the current branch name, or `None` for detached HEAD.
pub fn get_branch_name(repo_path: &Path) -> Result<Option<String>> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    Ok(current_branch_name(&repo))
}

/// Return the `count` most recent commits starting from HEAD.
pub fn get_recent_commits(repo_path: &Path, count: usize) -> Result<Vec<CommitInfo>> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk.push_head().context("failed to push HEAD")?;

    let mut commits = Vec::with_capacity(count);
    for oid in revwalk.take(count) {
        let oid = oid.context("revwalk error")?;
        let commit = repo.find_commit(oid).context("failed to find commit")?;

        let hash = oid.to_string();
        let short_hash = hash[..7.min(hash.len())].to_string();
        let sig = commit.author();
        let author = sig.name().unwrap_or("").to_string();
        let email = sig.email().unwrap_or("").to_string();

        let time = commit.time();
        let dt = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .unwrap_or_default()
            .with_timezone(&chrono::Utc);
        let date = dt.format("%Y-%m-%d %H:%M:%S").to_string();

        let message = commit.message().unwrap_or("").to_string();

        commits.push(CommitInfo {
            hash,
            short_hash,
            author,
            email,
            date,
            message,
        });
    }

    Ok(commits)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_branch_name(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if head.is_branch() {
        head.shorthand().map(|s| s.to_string())
    } else {
        None // detached HEAD
    }
}

/// Gather branch name, upstream name, and ahead/behind counts.
fn branch_info(repo: &Repository) -> (Option<String>, Option<String>, u32, u32) {
    let branch_name = current_branch_name(repo);

    let result: Option<(String, u32, u32)> = (|| -> Option<(String, u32, u32)> {
        let name = branch_name.as_deref()?;
        let branch = repo.find_branch(name, git2::BranchType::Local).ok()?;
        let upstream_branch = branch.upstream().ok()?;
        let upstream_name = upstream_branch.name().ok()??.to_string();

        let local_oid = repo.head().ok()?.target()?;
        let upstream_oid = upstream_branch.get().target()?;
        let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid).ok()?;

        Some((upstream_name, ahead as u32, behind as u32))
    })();

    let (upstream_str, ahead, behind) = result.unwrap_or((String::new(), 0, 0));
    let upstream = if upstream_str.is_empty() {
        None
    } else {
        Some(upstream_str)
    };

    (branch_name, upstream, ahead, behind)
}

/// Collect file statuses for either the index or the working directory.
fn collect_statuses(repo: &Repository, show: StatusShow) -> Result<Vec<FileStatus>> {
    let mut opts = StatusOptions::new();
    opts.show(show);
    opts.include_untracked(false);

    let statuses = repo
        .statuses(Some(&mut opts))
        .context("failed to get statuses")?;

    let mut result = Vec::new();
    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let s = entry.status();

        let change = match show {
            StatusShow::Index => index_change_type(s),
            StatusShow::Workdir => workdir_change_type(s),
            _ => None,
        };

        if let Some(status) = change {
            result.push(FileStatus { path, status });
        }
    }
    Ok(result)
}

fn index_change_type(s: git2::Status) -> Option<FileChangeType> {
    if s.contains(git2::Status::INDEX_NEW) {
        Some(FileChangeType::Added)
    } else if s.contains(git2::Status::INDEX_MODIFIED) {
        Some(FileChangeType::Modified)
    } else if s.contains(git2::Status::INDEX_DELETED) {
        Some(FileChangeType::Deleted)
    } else if s.contains(git2::Status::INDEX_RENAMED) {
        Some(FileChangeType::Renamed)
    } else if s.contains(git2::Status::INDEX_TYPECHANGE) {
        Some(FileChangeType::Modified)
    } else {
        None
    }
}

fn workdir_change_type(s: git2::Status) -> Option<FileChangeType> {
    if s.contains(git2::Status::WT_NEW) {
        Some(FileChangeType::Added)
    } else if s.contains(git2::Status::WT_MODIFIED) {
        Some(FileChangeType::Modified)
    } else if s.contains(git2::Status::WT_DELETED) {
        Some(FileChangeType::Deleted)
    } else if s.contains(git2::Status::WT_RENAMED) {
        Some(FileChangeType::Renamed)
    } else if s.contains(git2::Status::WT_TYPECHANGE) {
        Some(FileChangeType::Modified)
    } else {
        None
    }
}

/// Collect untracked file paths.
fn collect_untracked(repo: &Repository) -> Result<Vec<String>> {
    let mut opts = StatusOptions::new();
    opts.show(StatusShow::Workdir);
    opts.include_untracked(true);
    opts.exclude_submodules(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .context("failed to get statuses")?;

    let mut paths = Vec::new();
    for entry in statuses.iter() {
        if entry.status().contains(git2::Status::WT_NEW) {
            if let Some(p) = entry.path() {
                paths.push(p.to_string());
            }
        }
    }
    Ok(paths)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a repo, add a file, and make an initial commit.
    fn init_repo_with_commit(dir: &Path) {
        let repo = Repository::init(dir).unwrap();

        // Configure user for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        drop(config);

        // Create and add a file
        let file = dir.join("hello.txt");
        fs::write(&file, "hello world\n").unwrap();

        let mut index = repo.index().unwrap();
        index
            .add_path(Path::new("hello.txt"))
            .unwrap();
        index.write().unwrap();

        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap();
    }

    #[test]
    fn test_get_branch_name() {
        let tmp = TempDir::new().unwrap();
        init_repo_with_commit(tmp.path());

        // Default branch after `git init` + commit is typically "master" or "main"
        let branch = get_branch_name(tmp.path()).unwrap();
        assert!(branch.is_some(), "expected a branch name");
    }

    #[test]
    fn test_get_status_clean() {
        let tmp = TempDir::new().unwrap();
        init_repo_with_commit(tmp.path());

        let status = get_status(tmp.path()).unwrap();
        assert!(status.staged.is_empty());
        assert!(status.unstaged.is_empty());
        assert!(status.untracked.is_empty());
    }

    #[test]
    fn test_get_status_modified() {
        let tmp = TempDir::new().unwrap();
        init_repo_with_commit(tmp.path());

        // Modify the tracked file
        fs::write(tmp.path().join("hello.txt"), "changed\n").unwrap();

        let status = get_status(tmp.path()).unwrap();
        assert!(status.staged.is_empty());
        assert_eq!(status.unstaged.len(), 1);
        assert_eq!(status.unstaged[0].status, FileChangeType::Modified);
    }

    #[test]
    fn test_get_status_untracked() {
        let tmp = TempDir::new().unwrap();
        init_repo_with_commit(tmp.path());

        // Create an untracked file
        fs::write(tmp.path().join("new_file.txt"), "new\n").unwrap();

        let status = get_status(tmp.path()).unwrap();
        assert_eq!(status.untracked.len(), 1);
        assert_eq!(status.untracked[0], "new_file.txt");
    }

    #[test]
    fn test_get_recent_commits() {
        let tmp = TempDir::new().unwrap();
        init_repo_with_commit(tmp.path());

        let commits = get_recent_commits(tmp.path(), 5).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message.trim(), "initial commit");
        assert_eq!(commits[0].author, "Test User");
    }
}
