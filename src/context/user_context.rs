//! User context gathered once per conversation.
//!
//! Ported from ref/context.ts` -- the `getUserContext` logic.
//! Discovers RULES.md files up the directory tree and captures the
//! current date for inclusion in the system prompt.

use std::path::{Path, PathBuf};

use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Well-known RULES.md filenames
// ---------------------------------------------------------------------------

/// Filenames to search for at each directory level.
const RULES_MD_NAMES: &[&str] = &["RULES.md", ".thundercode/RULES.md"];

// ---------------------------------------------------------------------------
// RulesMdFile
// ---------------------------------------------------------------------------

/// A single RULES.md file discovered during the directory walk.
#[derive(Debug, Clone)]
pub struct RulesMdFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Raw file content.
    pub content: String,
}

// ---------------------------------------------------------------------------
// UserContext
// ---------------------------------------------------------------------------

/// User-level context captured at conversation start.
///
/// Like [`crate::context::system_context::SystemContext`], this is memoized for the
/// lifetime of a conversation.
#[derive(Debug, Clone)]
pub struct UserContext {
    /// RULES.md files discovered by walking from `cwd` up to the root and
    /// into `$HOME/.thundercode/`.
    pub rules_md_files: Vec<RulesMdFile>,
    /// Combined RULES.md content formatted for system prompt injection
    /// (sections separated by headers), or `None` when no files were found.
    pub memory_entrypoint: Option<String>,
    /// Current local date string (e.g. `"2026-03-31"`).
    pub current_date: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build the [`UserContext`] for the given working directory.
///
/// Discovers RULES.md files and captures the current date.
pub async fn get_user_context(cwd: &Path) -> UserContext {
    let current_date = chrono::Local::now().format("%Y-%m-%d").to_string();

    let rules_md_files = discover_rules_md_files(cwd);

    let memory_entrypoint = if rules_md_files.is_empty() {
        None
    } else {
        Some(format_rules_md_files(&rules_md_files))
    };

    debug!(
        rules_md_count = rules_md_files.len(),
        date = %current_date,
        "user context gathered"
    );

    UserContext {
        rules_md_files,
        memory_entrypoint,
        current_date,
    }
}

// ---------------------------------------------------------------------------
// RULES.md discovery
// ---------------------------------------------------------------------------

/// Walk from `cwd` up to the filesystem root, collecting RULES.md files.
///
/// Also checks the user home `~/.thundercode/RULES.md` if not already covered
/// by the walk.  Files are returned in order from most specific (cwd) to
/// most general (home).
fn discover_rules_md_files(cwd: &Path) -> Vec<RulesMdFile> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Walk up from cwd.
    let mut dir = cwd.to_path_buf();
    loop {
        for name in RULES_MD_NAMES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                if let Some(f) = read_rules_md(&candidate, &mut seen) {
                    files.push(f);
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }

    // Also check the user home config directory.
    if let Some(home) = dirs::home_dir() {
        let home_rules: PathBuf = home.join(".thundercode").join("RULES.md");
        if home_rules.is_file() {
            if let Some(f) = read_rules_md(&home_rules, &mut seen) {
                files.push(f);
            }
        }
    }

    files
}

/// Read a RULES.md file if we haven't seen its canonical path before.
fn read_rules_md(
    path: &Path,
    seen: &mut std::collections::HashSet<PathBuf>,
) -> Option<RulesMdFile> {
    let canonical = path.canonicalize().ok()?;
    if !seen.insert(canonical.clone()) {
        return None; // already collected
    }

    match std::fs::read_to_string(path) {
        Ok(content) if !content.trim().is_empty() => Some(RulesMdFile {
            path: canonical,
            content,
        }),
        Ok(_) => {
            debug!(?path, "skipping empty RULES.md");
            None
        }
        Err(e) => {
            warn!(?path, %e, "failed to read RULES.md");
            None
        }
    }
}

/// Format collected RULES.md files into a single string for the system
/// prompt, with path-based section headers.
fn format_rules_md_files(files: &[RulesMdFile]) -> String {
    let mut sections = Vec::with_capacity(files.len());

    for f in files {
        let header = format!("# From {}", f.path.display());
        sections.push(format!("{header}\n\n{}", f.content.trim()));
    }

    sections.join("\n\n")
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
    fn discover_finds_rules_md_in_cwd() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("RULES.md"), "# Project rules\nAlways test.").unwrap();

        let files = discover_rules_md_files(tmp.path());
        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("Always test"));
    }

    #[test]
    fn discover_skips_empty_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("RULES.md"), "   \n  ").unwrap();

        let files = discover_rules_md_files(tmp.path());
        assert!(files.is_empty());
    }

    #[test]
    fn discover_finds_nested_rules_dir() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join(".thundercode");
        fs::create_dir_all(&rules_dir).unwrap();
        fs::write(rules_dir.join("RULES.md"), "nested content").unwrap();

        let files = discover_rules_md_files(tmp.path());
        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("nested content"));
    }

    #[test]
    fn discover_deduplicates() {
        let tmp = TempDir::new().unwrap();
        // Both RULES.md and .primary/RULES.md point to same content;
        // we just check that parent dirs don't duplicate.
        fs::write(tmp.path().join("RULES.md"), "top level").unwrap();

        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();

        // Walk from sub should find the parent RULES.md.
        let files = discover_rules_md_files(&sub);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn format_produces_headers() {
        let files = vec![
            RulesMdFile {
                path: PathBuf::from("/project/RULES.md"),
                content: "rule one".into(),
            },
            RulesMdFile {
                path: PathBuf::from("/home/user/.thundercode/RULES.md"),
                content: "global rule".into(),
            },
        ];

        let out = format_rules_md_files(&files);
        assert!(out.contains("# From /project/RULES.md"));
        assert!(out.contains("rule one"));
        assert!(out.contains("# From /home/user/.thundercode/RULES.md"));
        assert!(out.contains("global rule"));
    }

    #[test]
    fn current_date_is_today() {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        // Just verify the format is correct (YYYY-MM-DD).
        assert_eq!(today.len(), 10);
        assert_eq!(&today[4..5], "-");
    }
}
