//! Common git operations.
//!
//! Wraps repository initialisation, staging, committing, config reading,
//! and repository discovery.  Uses `git2` for most operations, falling
//! back to the git CLI only where necessary (e.g. `get_ignore_patterns`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use git2::Repository;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise a new git repository at `path`.
pub fn init_repo(path: &Path) -> Result<()> {
    Repository::init(path).context("failed to init repository")?;
    Ok(())
}

/// Stage files by their paths (relative to the repo root).
pub fn add_files(repo_path: &Path, paths: &[&str]) -> Result<()> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let mut index = repo.index().context("failed to get index")?;

    for p in paths {
        index
            .add_path(Path::new(p))
            .with_context(|| format!("failed to add path: {}", p))?;
    }

    index.write().context("failed to write index")?;
    Ok(())
}

/// Create a commit on HEAD with the given `message`.
///
/// Returns the full SHA-1 hex string of the new commit.
pub fn commit(repo_path: &Path, message: &str) -> Result<String> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let sig = repo.signature().context("failed to build signature")?;

    let mut index = repo.index().context("failed to get index")?;
    let tree_oid = index.write_tree().context("failed to write tree")?;
    let tree = repo.find_tree(tree_oid).context("failed to find tree")?;

    // Find parent commit(s).  On a fresh repo with no commits, there are
    // zero parents.
    let parents = match repo.head() {
        Ok(head) => {
            let oid = head
                .target()
                .context("HEAD has no target")?;
            let parent = repo.find_commit(oid).context("failed to find HEAD commit")?;
            vec![parent]
        }
        Err(_) => vec![], // initial commit
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .context("failed to create commit")?;

    Ok(oid.to_string())
}

/// Read `user.name` from the repository (or global) git config.
pub fn get_user_name(repo_path: &Path) -> Result<Option<String>> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let config = repo.config().context("failed to read config")?;
    match config.get_string("user.name") {
        Ok(name) => Ok(Some(name)),
        Err(_) => Ok(None),
    }
}

/// Read `user.email` from the repository (or global) git config.
pub fn get_user_email(repo_path: &Path) -> Result<Option<String>> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let config = repo.config().context("failed to read config")?;
    match config.get_string("user.email") {
        Ok(email) => Ok(Some(email)),
        Err(_) => Ok(None),
    }
}

/// Read the URL of a named remote (e.g. `"origin"`).
pub fn get_remote_url(repo_path: &Path, remote: &str) -> Result<Option<String>> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let url = match repo.find_remote(remote) {
        Ok(r) => r.url().map(|s| s.to_string()),
        Err(_) => None,
    };
    Ok(url)
}

/// Return `true` if `path` is inside a git working tree.
///
/// This mirrors `git rev-parse --is-inside-work-tree`.
pub fn is_inside_work_tree(path: &Path) -> bool {
    find_repo_root(path).is_some()
}

/// Walk up from `path` looking for a `.git` directory or file (worktrees
/// / submodules use a `.git` *file* containing a `gitdir:` pointer).
///
/// Returns the directory that contains `.git`, i.e. the repo root.
pub fn find_repo_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };

    loop {
        let git_path = current.join(".git");
        if git_path.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// List patterns from `.gitignore` files that are active in the repo.
///
/// This shells out to `git ls-files` because libgit2's ignore API is
/// limited.  Returns a list of pattern strings.
pub fn get_ignore_patterns(repo_path: &Path) -> Result<Vec<String>> {
    // Read the top-level .gitignore if it exists.
    let gitignore_path = repo_path.join(".gitignore");
    if !gitignore_path.is_file() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&gitignore_path)
        .context("failed to read .gitignore")?;

    let patterns: Vec<String> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();

    Ok(patterns)
}

/// Check if a path appears to be the root of a git repository (has `.git`).
pub fn is_git_root(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Determine the default branch name by inspecting `refs/remotes/origin/HEAD`
/// or falling back to common names.
pub fn get_default_branch(repo_path: &Path) -> Result<String> {
    // Try git symbolic-ref refs/remotes/origin/HEAD
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_path)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let refname = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Some(branch) = refname.strip_prefix("refs/remotes/origin/") {
                return Ok(branch.to_string());
            }
        }
    }

    // Fallback: check which common branch exists.
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    for candidate in &["main", "master", "develop"] {
        let refname = format!("refs/heads/{}", candidate);
        if repo.find_reference(&refname).is_ok() {
            return Ok(candidate.to_string());
        }
    }

    Ok("main".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_init_repo() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();
        assert!(tmp.path().join(".git").is_dir());
    }

    #[test]
    fn test_find_repo_root() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();

        // From the root itself.
        assert_eq!(
            find_repo_root(tmp.path()).unwrap(),
            tmp.path().to_path_buf()
        );

        // From a subdirectory.
        let sub = tmp.path().join("a").join("b");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(find_repo_root(&sub).unwrap(), tmp.path().to_path_buf());
    }

    #[test]
    fn test_is_inside_work_tree() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();
        assert!(is_inside_work_tree(tmp.path()));

        let outside = TempDir::new().unwrap();
        assert!(!is_inside_work_tree(outside.path()));
    }

    #[test]
    fn test_add_and_commit() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();

        // Configure user.
        let repo = Repository::open(tmp.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "t@t.com").unwrap();
        drop(config);
        drop(repo);

        fs::write(tmp.path().join("file.txt"), "content").unwrap();
        add_files(tmp.path(), &["file.txt"]).unwrap();
        let sha = commit(tmp.path(), "test commit").unwrap();
        assert_eq!(sha.len(), 40);
    }

    #[test]
    fn test_get_user_name_email() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();

        let repo = Repository::open(tmp.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Alice").unwrap();
        config.set_str("user.email", "alice@example.com").unwrap();
        drop(config);
        drop(repo);

        assert_eq!(
            get_user_name(tmp.path()).unwrap(),
            Some("Alice".to_string())
        );
        assert_eq!(
            get_user_email(tmp.path()).unwrap(),
            Some("alice@example.com".to_string())
        );
    }

    #[test]
    fn test_get_ignore_patterns() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();

        // No .gitignore yet.
        assert!(get_ignore_patterns(tmp.path()).unwrap().is_empty());

        fs::write(
            tmp.path().join(".gitignore"),
            "# comment\ntarget/\n*.log\n\n",
        )
        .unwrap();

        let patterns = get_ignore_patterns(tmp.path()).unwrap();
        assert_eq!(patterns, vec!["target/", "*.log"]);
    }

    #[test]
    fn test_get_remote_url_none() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).unwrap();
        assert_eq!(get_remote_url(tmp.path(), "origin").unwrap(), None);
    }
}
