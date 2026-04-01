//! Git blame -- annotate each line of a file with its last-changing commit.
//!
//! Uses `git2`'s blame API for the core work.

use std::path::Path;

use anyhow::{Context, Result};
use git2::Repository;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Blame annotation for a single source line.
#[derive(Debug, Clone)]
pub struct BlameLine {
    /// Full SHA-1 commit hash.
    pub commit_hash: String,
    /// Author name.
    pub author: String,
    /// ISO-style date string (YYYY-MM-DD).
    pub date: String,
    /// 1-based line number in the file.
    pub line_number: u32,
    /// Text content of the line.
    pub content: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run `git blame` on `file_path` (relative to `repo_path`) and return
/// per-line annotations.
pub fn blame_file(repo_path: &Path, file_path: &str) -> Result<Vec<BlameLine>> {
    let repo = Repository::open(repo_path).context("failed to open repository")?;
    let blame = repo
        .blame_file(Path::new(file_path), None)
        .with_context(|| format!("failed to blame {}", file_path))?;

    // Read the file content so we can pair blame hunks with actual line text.
    let full_path = repo_path.join(file_path);
    let content =
        std::fs::read_to_string(&full_path).context("failed to read file for blame")?;
    let lines: Vec<&str> = content.lines().collect();

    let mut result = Vec::with_capacity(lines.len());

    for (i, line_text) in lines.iter().enumerate() {
        let line_no = i + 1; // 1-based

        if let Some(hunk) = blame.get_line(line_no) {
            let oid = hunk.final_commit_id();
            let commit_hash = oid.to_string();

            let sig = hunk.final_signature();
            let author = sig.name().unwrap_or("").to_string();

            let date = match sig.when().seconds() {
                secs => {
                    let dt = chrono::DateTime::from_timestamp(secs, 0)
                        .unwrap_or_default()
                        .with_timezone(&chrono::Utc);
                    dt.format("%Y-%m-%d").to_string()
                }
            };

            result.push(BlameLine {
                commit_hash,
                author,
                date,
                line_number: line_no as u32,
                content: line_text.to_string(),
            });
        } else {
            // Uncommitted / not-yet-committed lines get a zeroed hash.
            result.push(BlameLine {
                commit_hash: "0".repeat(40),
                author: String::new(),
                date: String::new(),
                line_number: line_no as u32,
                content: line_text.to_string(),
            });
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: init a repo and make a commit with a single file.
    fn setup_repo(content: &str) -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let repo = Repository::init(tmp.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Blame Author").unwrap();
        config.set_str("user.email", "blame@test.com").unwrap();

        let file_path = "sample.txt";
        fs::write(tmp.path().join(file_path), content).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file_path)).unwrap();
        index.write().unwrap();

        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        (tmp, file_path.to_string())
    }

    #[test]
    fn test_blame_file() {
        let (tmp, file_path) = setup_repo("line one\nline two\nline three\n");

        let annotations = blame_file(tmp.path(), &file_path).unwrap();
        assert_eq!(annotations.len(), 3);

        assert_eq!(annotations[0].line_number, 1);
        assert_eq!(annotations[0].content, "line one");
        assert_eq!(annotations[0].author, "Blame Author");
        assert!(!annotations[0].commit_hash.is_empty());
        assert!(!annotations[0].date.is_empty());

        assert_eq!(annotations[2].line_number, 3);
        assert_eq!(annotations[2].content, "line three");
    }
}
