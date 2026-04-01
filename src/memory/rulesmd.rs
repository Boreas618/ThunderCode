//! RULES.md discovery and loading system.
//!
//! Ported from ref/utils/rulesmd.ts`.
//!
//! Files are loaded in the following priority order (lowest to highest):
//!
//! 1. Managed memory (e.g. `/etc/thundercode/RULES.md`) -- global policy
//! 2. User memory (`~/.thundercode/RULES.md`) -- private global instructions
//! 3. Project memory (`RULES.md`, `.primary/RULES.md`, `.primary/rules/*.md`) -- checked in
//! 4. Local memory (`RULES.local.md`) -- private project-specific
//!
//! File discovery traverses from cwd up to the filesystem root.
//! Files closer to cwd have higher priority (loaded later).

use std::fs;
use std::path::{Path, PathBuf};

use crate::memory::frontmatter::parse_generic_frontmatter;
use crate::memory::memdir::{truncate_entrypoint_content, ENTRYPOINT_NAME};
use crate::memory::types::{RulesMdFile, RulesMdSource};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Instruction prompt prepended to assembled instructions.
pub const MEMORY_INSTRUCTION_PROMPT: &str =
    "Codebase and user instructions are shown below. Be sure to adhere to these \
     instructions. IMPORTANT: These instructions OVERRIDE any default behavior \
     and you MUST follow them exactly as written.";

/// Recommended max character count for a single memory file.
pub const MAX_MEMORY_CHARACTER_COUNT: usize = 40_000;

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover all RULES.md files relevant to the given working directory.
///
/// Traverses upward from `cwd` to the filesystem root, collecting files
/// in priority order (managed -> user -> project -> local).
pub fn discover_rules_md_files(cwd: &Path) -> Vec<RulesMdFile> {
    let mut result = Vec::new();
    let mut processed_paths = std::collections::HashSet::new();

    // 1. Managed RULES.md (policy)
    let managed_path = managed_rules_md_path();
    if let Some(file) = load_rules_md_if_exists(&managed_path, RulesMdSource::Managed) {
        processed_paths.insert(normalize_path(&managed_path));
        result.push(file);
    }

    // 1b. Managed rules directory
    let managed_rules_dir = managed_rules_dir();
    collect_rules_dir(&managed_rules_dir, RulesMdSource::Managed, &mut processed_paths, &mut result);

    // 2. User RULES.md (~/.thundercode/RULES.md)
    let user_path = user_rules_md_path();
    if let Some(file) = load_rules_md_if_exists(&user_path, RulesMdSource::User) {
        processed_paths.insert(normalize_path(&user_path));
        result.push(file);
    }

    // 2b. User rules directory (~/.thundercode/rules/*.md)
    let user_rules_dir = user_rules_dir();
    collect_rules_dir(&user_rules_dir, RulesMdSource::User, &mut processed_paths, &mut result);

    // 3 & 4. Walk from root down to cwd for Project and Local files
    let mut dirs = Vec::new();
    let mut current = cwd.to_path_buf();
    loop {
        dirs.push(current.clone());
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => break,
        }
    }
    // Reverse so we process from root down to cwd (lower priority first)
    dirs.reverse();

    for dir in &dirs {
        // Project: RULES.md
        let project_path = dir.join("RULES.md");
        if let Some(file) = load_if_not_processed(&project_path, RulesMdSource::Project, &mut processed_paths) {
            result.push(file);
        }

        // Project: .primary/RULES.md
        let dot_rules_path = dir.join(".thundercode").join("RULES.md");
        if let Some(file) = load_if_not_processed(&dot_rules_path, RulesMdSource::Project, &mut processed_paths) {
            result.push(file);
        }

        // Project: .primary/rules/*.md
        let rules_dir = dir.join(".thundercode").join("rules");
        collect_rules_dir(&rules_dir, RulesMdSource::Project, &mut processed_paths, &mut result);

        // Local: RULES.local.md
        let local_path = dir.join("RULES.local.md");
        if let Some(file) = load_if_not_processed(&local_path, RulesMdSource::Local, &mut processed_paths) {
            result.push(file);
        }
    }

    result
}

/// Load a single RULES.md file from a specific path.
///
/// Returns `Err` if the file does not exist or cannot be read.
pub fn load_rules_md(path: &Path) -> anyhow::Result<RulesMdFile> {
    let content = fs::read_to_string(path)?;
    let source = infer_source(path);
    let parsed = parse_generic_frontmatter(&content);

    Ok(RulesMdFile {
        path: path.to_path_buf(),
        content: parsed.content,
        source,
        globs: parsed.paths,
        parent: None,
    })
}

/// Load a MEMORY.md entrypoint as a [`RulesMdFile`], applying truncation.
pub fn load_memory_entrypoint(path: &Path, source: RulesMdSource) -> Option<RulesMdFile> {
    let raw_content = fs::read_to_string(path).ok()?;
    if raw_content.trim().is_empty() {
        return None;
    }

    let truncated = truncate_entrypoint_content(&raw_content);

    Some(RulesMdFile {
        path: path.to_path_buf(),
        content: truncated.content,
        source,
        globs: None,
        parent: None,
    })
}

/// Strip block-level HTML comments from markdown content.
///
/// Removes `<!-- ... -->` comments that occupy their own lines.
/// Comments inside code blocks or inline code are preserved.
pub fn strip_html_comments(content: &str) -> String {
    if !content.contains("<!--") {
        return content.to_string();
    }

    // Simple regex-based stripping for block-level HTML comments.
    // Matches comments that start at the beginning of a line (possibly after whitespace).
    let re = regex::Regex::new(r"(?m)^\s*<!--[\s\S]*?-->\s*\n?").unwrap();
    re.replace_all(content, "").to_string()
}

/// Assemble the full instruction prompt from a list of RULES.md files.
///
/// Returns the combined content with source headers.
pub fn assemble_instructions(files: &[RulesMdFile]) -> String {
    if files.is_empty() {
        return String::new();
    }

    let mut parts = vec![MEMORY_INSTRUCTION_PROMPT.to_string()];

    for file in files {
        let source_label = match file.source {
            RulesMdSource::Managed => "Managed instructions",
            RulesMdSource::User => "User instructions",
            RulesMdSource::Project => "Project instructions",
            RulesMdSource::Local => "Local instructions",
            RulesMdSource::AutoMem => "Memory index",
            RulesMdSource::TeamMem => "Team memory index",
        };

        let path_display = file.path.display();
        parts.push(format!("\n--- {source_label} ({path_display}) ---\n"));
        parts.push(file.content.clone());
    }

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Path to managed RULES.md (policy).
fn managed_rules_md_path() -> PathBuf {
    crate::config::managed_settings_dir().join("RULES.md")
}

/// Path to managed rules directory.
fn managed_rules_dir() -> PathBuf {
    crate::config::managed_settings_dir()
        .join(".thundercode")
        .join("rules")
}

/// Path to user RULES.md.
fn user_rules_md_path() -> PathBuf {
    crate::config::config_home_dir().join("RULES.md")
}

/// Path to user rules directory.
fn user_rules_dir() -> PathBuf {
    crate::config::config_home_dir().join("rules")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Load a RULES.md file if it exists. Returns `None` for missing files.
fn load_rules_md_if_exists(path: &Path, source: RulesMdSource) -> Option<RulesMdFile> {
    let content = fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        return None;
    }

    let parsed = parse_generic_frontmatter(&content);

    Some(RulesMdFile {
        path: path.to_path_buf(),
        content: parsed.content,
        source,
        globs: parsed.paths,
        parent: None,
    })
}

/// Load a file only if it hasn't been processed yet.
fn load_if_not_processed(
    path: &Path,
    source: RulesMdSource,
    processed: &mut std::collections::HashSet<String>,
) -> Option<RulesMdFile> {
    let normalized = normalize_path(path);
    if processed.contains(&normalized) {
        return None;
    }

    let file = load_rules_md_if_exists(path, source)?;
    processed.insert(normalized);
    Some(file)
}

/// Collect all .md files from a rules directory.
fn collect_rules_dir(
    rules_dir: &Path,
    source: RulesMdSource,
    processed: &mut std::collections::HashSet<String>,
    result: &mut Vec<RulesMdFile>,
) {
    if !rules_dir.is_dir() {
        return;
    }

    collect_rules_dir_recursive(rules_dir, source, processed, result);
}

/// Recursively collect .md files from a directory tree.
fn collect_rules_dir_recursive(
    dir: &Path,
    source: RulesMdSource,
    processed: &mut std::collections::HashSet<String>,
    result: &mut Vec<RulesMdFile>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_rules_dir_recursive(&path, source, processed, result);
        } else if path.extension().map_or(false, |e| e == "md") {
            if let Some(file) = load_if_not_processed(&path, source, processed) {
                result.push(file);
            }
        }
    }
}

/// Normalize a path for deduplication.
fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Infer the [`RulesMdSource`] from a file path.
fn infer_source(path: &Path) -> RulesMdSource {
    let path_str = path.to_string_lossy();

    if path_str.contains("RULES.local.md") {
        return RulesMdSource::Local;
    }

    // Check managed dir
    let managed = crate::config::managed_settings_dir();
    if path.starts_with(&managed) {
        return RulesMdSource::Managed;
    }

    // Check user dir (~/.primary)
    let config_home = crate::config::config_home_dir();
    if path.starts_with(&config_home) {
        // Could be AutoMem or User
        if path.file_name().map_or(false, |n| n == ENTRYPOINT_NAME) {
            return RulesMdSource::AutoMem;
        }
        return RulesMdSource::User;
    }

    RulesMdSource::Project
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn discover_project_rules_md() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();

        // Create a RULES.md in the project root
        fs::write(
            project_dir.join("RULES.md"),
            "Always use Rust for new code.\n",
        )
        .unwrap();

        let files = discover_rules_md_files(project_dir);

        // Should find the project RULES.md (managed/user may not exist)
        let project_files: Vec<_> = files
            .iter()
            .filter(|f| f.source == RulesMdSource::Project)
            .collect();
        assert_eq!(project_files.len(), 1);
        assert!(project_files[0].content.contains("Always use Rust"));
    }

    #[test]
    fn discover_local_rules_md() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();

        fs::write(
            project_dir.join("RULES.local.md"),
            "Local dev note.\n",
        )
        .unwrap();

        let files = discover_rules_md_files(project_dir);

        let local_files: Vec<_> = files
            .iter()
            .filter(|f| f.source == RulesMdSource::Local)
            .collect();
        assert_eq!(local_files.len(), 1);
        assert!(local_files[0].content.contains("Local dev note"));
    }

    #[test]
    fn discover_rules_dir() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();

        let rules_dir = project_dir.join(".thundercode").join("rules");
        fs::create_dir_all(&rules_dir).unwrap();
        fs::write(rules_dir.join("naming.md"), "Use snake_case.\n").unwrap();
        fs::write(rules_dir.join("testing.md"), "Always write tests.\n").unwrap();

        let files = discover_rules_md_files(project_dir);

        let rule_files: Vec<_> = files
            .iter()
            .filter(|f| {
                f.source == RulesMdSource::Project
                    && f.path.to_string_lossy().contains("rules")
            })
            .collect();
        assert_eq!(rule_files.len(), 2);
    }

    #[test]
    fn discover_dot_rules_md() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();

        let dot_rules = project_dir.join(".thundercode");
        fs::create_dir_all(&dot_rules).unwrap();
        fs::write(dot_rules.join("RULES.md"), "Dot rules instructions.\n").unwrap();

        let files = discover_rules_md_files(project_dir);

        let project_files: Vec<_> = files
            .iter()
            .filter(|f| f.source == RulesMdSource::Project)
            .collect();
        assert_eq!(project_files.len(), 1);
        assert!(project_files[0].content.contains("Dot rules instructions"));
    }

    #[test]
    fn strip_comments() {
        let input = "Line 1\n<!-- This is a comment -->\nLine 2\n";
        let result = strip_html_comments(input);
        assert!(!result.contains("comment"));
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
    }

    #[test]
    fn strip_no_comments() {
        let input = "Just plain markdown.\n";
        let result = strip_html_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn assemble_empty() {
        assert_eq!(assemble_instructions(&[]), "");
    }

    #[test]
    fn assemble_with_files() {
        let files = vec![
            RulesMdFile {
                path: PathBuf::from("/project/RULES.md"),
                content: "Use Rust.".to_string(),
                source: RulesMdSource::Project,
                globs: None,
                parent: None,
            },
        ];
        let assembled = assemble_instructions(&files);
        assert!(assembled.contains(MEMORY_INSTRUCTION_PROMPT));
        assert!(assembled.contains("Project instructions"));
        assert!(assembled.contains("Use Rust."));
    }

    #[test]
    fn load_rules_md_with_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("RULES.md");
        fs::write(
            &path,
            "---\ndescription: test rule\npaths: \"src/**/*.rs\"\n---\n\nAlways test.\n",
        )
        .unwrap();

        let file = load_rules_md(&path).unwrap();
        assert!(file.content.contains("Always test."));
        assert!(file.globs.is_some());
        let globs = file.globs.unwrap();
        assert!(globs.contains(&"src/**/*.rs".to_string()));
    }

    #[test]
    fn memory_entrypoint_truncation() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("MEMORY.md");
        let content = (0..10).map(|i| format!("- [{i}](file{i}.md)")).collect::<Vec<_>>().join("\n");
        fs::write(&path, &content).unwrap();

        let loaded = load_memory_entrypoint(&path, RulesMdSource::AutoMem);
        assert!(loaded.is_some());
        let file = loaded.unwrap();
        assert!(file.content.contains("[0]"));
    }
}
