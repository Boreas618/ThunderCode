//! Skill directory loading.
//!
//! Ported from ref/skills/loadSkillsDir.ts. Loads skill definitions from
//! `skills/` directories in the filesystem.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::skills::frontmatter::{extract_description_from_markdown, parse_skill_file};

// ---------------------------------------------------------------------------
// SkillSource
// ---------------------------------------------------------------------------

/// Where a skill was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillSource {
    /// Policy-managed skills (from managed config path).
    Managed,
    /// User-level skills (~/.thundercode/skills/).
    User,
    /// Project-level skills (.primary/skills/ relative to cwd).
    Project,
    /// Built-in / bundled skills compiled into the binary.
    Bundled,
    /// Skills from installed plugins.
    Plugin,
    /// Skills from MCP servers.
    Mcp,
}

impl std::fmt::Display for SkillSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Managed => write!(f, "managed"),
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
            Self::Bundled => write!(f, "bundled"),
            Self::Plugin => write!(f, "plugin"),
            Self::Mcp => write!(f, "mcp"),
        }
    }
}

// ---------------------------------------------------------------------------
// SkillContext
// ---------------------------------------------------------------------------

/// Execution context for a skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillContext {
    /// Skill content expands into the current conversation.
    Inline,
    /// Skill runs in a sub-agent with separate context and token budget.
    Fork,
}

impl Default for SkillContext {
    fn default() -> Self {
        Self::Inline
    }
}

impl SkillContext {
    /// Parse from a string value (frontmatter `context:` field).
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s {
            Some("fork") => Self::Fork,
            _ => Self::Inline,
        }
    }
}

// ---------------------------------------------------------------------------
// SkillDefinition
// ---------------------------------------------------------------------------

/// A fully resolved skill definition ready for registration.
#[derive(Debug, Clone)]
pub struct SkillDefinition {
    /// Canonical skill name (used for `/skill-name` invocation).
    pub name: String,
    /// Short description of what the skill does.
    pub description: String,
    /// Detailed usage scenarios.
    pub when_to_use: Option<String>,
    /// The raw markdown template (body after frontmatter).
    pub prompt_template: String,
    /// Where this skill was loaded from.
    pub source: SkillSource,
    /// Named arguments the skill accepts.
    pub arg_names: Option<Vec<String>>,
    /// Tool names allowed when this skill is active.
    pub allowed_tools: Option<Vec<String>>,
    /// Model alias or name.
    pub model: Option<String>,
    /// Execution context (inline or fork).
    pub context: SkillContext,
    /// Glob patterns for file paths this skill applies to.
    pub paths: Option<Vec<String>>,
    /// Hooks to register when this skill is invoked.
    pub hooks: Option<serde_json::Value>,
    /// Skill version string.
    pub version: Option<String>,
    /// Length of the prompt template content in characters.
    pub content_length: usize,
    /// Whether users can invoke this skill by typing /skill-name.
    pub user_invocable: bool,
    /// Whether to disable this skill from being invoked by models.
    pub disable_model_invocation: bool,
    /// Hint text for command arguments.
    pub argument_hint: Option<String>,
    /// Agent type when forked.
    pub agent: Option<String>,
    /// Effort level for agents.
    pub effort: Option<String>,
    /// Base directory for the skill (the directory containing SKILL.md).
    pub skill_root: Option<PathBuf>,
    /// Resolved real path of SKILL.md (for deduplication).
    pub real_path: Option<PathBuf>,
}

impl SkillDefinition {
    /// Estimate token count for this skill based on frontmatter metadata only
    /// (name + description + when_to_use). Full content is loaded on invocation.
    pub fn estimate_frontmatter_tokens(&self) -> usize {
        let text = [
            Some(self.name.as_str()),
            Some(self.description.as_str()),
            self.when_to_use.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        rough_token_count(&text)
    }

    /// Whether this skill is conditional (has path patterns).
    pub fn is_conditional(&self) -> bool {
        self.paths.as_ref().is_some_and(|p| !p.is_empty())
    }
}

/// Rough token count estimation (approximately 4 characters per token).
fn rough_token_count(text: &str) -> usize {
    // A simple heuristic: count words and multiply, or divide chars by 4.
    // This matches the TypeScript roughTokenCountEstimation().
    (text.len() + 3) / 4
}

// ---------------------------------------------------------------------------
// Loading skills from a directory
// ---------------------------------------------------------------------------

/// Load all skills from a `skills/` directory.
///
/// Expects the directory format: `<basePath>/<skill-name>/SKILL.md`.
/// Each subdirectory that contains a `SKILL.md` file is treated as a skill.
pub fn load_skills_dir(dir: &Path, source: SkillSource) -> Result<Vec<SkillDefinition>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            warn!("permission denied reading skills dir: {}", dir.display());
            return Ok(Vec::new());
        }
        Err(e) => return Err(e.into()),
    };

    let mut skills = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("error reading dir entry in {}: {}", dir.display(), e);
                continue;
            }
        };

        let entry_path = entry.path();

        // Only support directory format: skill-name/SKILL.md
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }

        let skill_dir = &entry_path;
        let skill_file = skill_dir.join("SKILL.md");

        let content = match fs::read_to_string(&skill_file) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                warn!(
                    "failed to read {}: {}",
                    skill_file.display(),
                    e
                );
                continue;
            }
        };

        match load_single_skill(&content, &entry_path, &skill_file, source) {
            Ok(skill) => skills.push(skill),
            Err(e) => {
                warn!(
                    "failed to parse skill {}: {}",
                    skill_file.display(),
                    e
                );
            }
        }
    }

    Ok(skills)
}

/// Parse a single SKILL.md file into a `SkillDefinition`.
fn load_single_skill(
    content: &str,
    skill_dir: &Path,
    skill_file: &Path,
    source: SkillSource,
) -> Result<SkillDefinition> {
    let (fm, body) = parse_skill_file(content)?;

    // Skill name comes from the directory name.
    let skill_name = skill_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Resolve description: prefer frontmatter, fall back to first paragraph.
    let description = fm
        .description
        .clone()
        .unwrap_or_else(|| extract_description_from_markdown(&body, "Skill"));

    // Resolve the real path for deduplication.
    let real_path = fs::canonicalize(skill_file).ok();

    let context = SkillContext::from_str_opt(fm.context.as_deref());

    Ok(SkillDefinition {
        name: skill_name,
        description,
        when_to_use: fm.when_to_use,
        prompt_template: body,
        source,
        arg_names: fm.arg_names,
        allowed_tools: fm.allowed_tools,
        model: fm.model,
        context,
        paths: fm.paths,
        hooks: fm.hooks,
        version: fm.version,
        content_length: content.len(),
        user_invocable: fm.user_invocable.unwrap_or(true),
        disable_model_invocation: fm.disable_model_invocation.unwrap_or(false),
        argument_hint: fm.argument_hint,
        agent: fm.agent,
        effort: fm.effort,
        skill_root: Some(skill_dir.to_path_buf()),
        real_path,
    })
}

// ---------------------------------------------------------------------------
// Load all skills (convenience that drives resolution + dedup)
// ---------------------------------------------------------------------------

/// Load all skills from all known locations for the given working directory.
///
/// This is a convenience wrapper that calls `resolve_skills` from the
/// resolution module. See [`crate::skills::resolution::resolve_skills`] for details.
pub fn load_all_skills(cwd: &Path) -> Vec<SkillDefinition> {
    crate::skills::resolution::resolve_skills(cwd)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_skill_dir(base: &Path, name: &str, content: &str) {
        let skill_dir = base.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn test_load_skills_dir_basic() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        create_skill_dir(
            &skills_dir,
            "my-skill",
            "---\ndescription: A test skill\n---\nDo the thing.\n",
        );

        let skills = load_skills_dir(&skills_dir, SkillSource::User).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "A test skill");
        assert_eq!(skills[0].source, SkillSource::User);
        assert!(skills[0].prompt_template.contains("Do the thing."));
    }

    #[test]
    fn test_load_skills_dir_no_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        create_skill_dir(&skills_dir, "plain", "Just do this task.\n");

        let skills = load_skills_dir(&skills_dir, SkillSource::Project).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "plain");
        // Falls back to first paragraph for description
        assert_eq!(skills[0].description, "Just do this task.");
    }

    #[test]
    fn test_load_skills_dir_empty() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let skills = load_skills_dir(&skills_dir, SkillSource::User).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_load_skills_dir_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let skills = load_skills_dir(&tmp.path().join("nope"), SkillSource::User).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_load_skills_dir_ignores_files() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        // A plain .md file at the top level should be ignored
        fs::write(skills_dir.join("stray.md"), "Not a skill").unwrap();

        create_skill_dir(&skills_dir, "real-skill", "---\ndescription: Real\n---\nContent\n");

        let skills = load_skills_dir(&skills_dir, SkillSource::User).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "real-skill");
    }

    #[test]
    fn test_load_skills_dir_missing_skill_md() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        // Directory exists but has no SKILL.md
        fs::create_dir_all(skills_dir.join("empty-dir")).unwrap();

        let skills = load_skills_dir(&skills_dir, SkillSource::User).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_skill_context_parsing() {
        assert_eq!(SkillContext::from_str_opt(None), SkillContext::Inline);
        assert_eq!(SkillContext::from_str_opt(Some("inline")), SkillContext::Inline);
        assert_eq!(SkillContext::from_str_opt(Some("fork")), SkillContext::Fork);
        assert_eq!(SkillContext::from_str_opt(Some("other")), SkillContext::Inline);
    }

    #[test]
    fn test_skill_with_all_fields() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        fs::create_dir_all(&skills_dir).unwrap();

        let content = r#"---
description: Full featured skill
when_to_use: When testing
allowed-tools: Read, Write
model: sonnet
context: fork
agent: general-purpose
effort: high
version: "2.0"
user-invocable: false
disable-model-invocation: true
argument-hint: "<path>"
paths:
  - src/**/*.rs
---
Do the full thing with $ARGUMENTS.
"#;
        create_skill_dir(&skills_dir, "full-skill", content);

        let skills = load_skills_dir(&skills_dir, SkillSource::Project).unwrap();
        assert_eq!(skills.len(), 1);
        let s = &skills[0];
        assert_eq!(s.name, "full-skill");
        assert_eq!(s.context, SkillContext::Fork);
        assert!(!s.user_invocable);
        assert!(s.disable_model_invocation);
        assert_eq!(s.version.as_deref(), Some("2.0"));
        assert_eq!(s.agent.as_deref(), Some("general-purpose"));
        assert_eq!(s.effort.as_deref(), Some("high"));
        assert!(s.paths.is_some());
    }

    #[test]
    fn test_rough_token_count() {
        assert_eq!(rough_token_count(""), 0);
        assert_eq!(rough_token_count("abcd"), 1);
        assert_eq!(rough_token_count("12345678"), 2);
        assert!(rough_token_count("hello world this is a test") > 0);
    }

    #[test]
    fn test_estimate_frontmatter_tokens() {
        let skill = SkillDefinition {
            name: "test".to_string(),
            description: "A test skill".to_string(),
            when_to_use: Some("When testing".to_string()),
            prompt_template: String::new(),
            source: SkillSource::User,
            arg_names: None,
            allowed_tools: None,
            model: None,
            context: SkillContext::Inline,
            paths: None,
            hooks: None,
            version: None,
            content_length: 0,
            user_invocable: true,
            disable_model_invocation: false,
            argument_hint: None,
            agent: None,
            effort: None,
            skill_root: None,
            real_path: None,
        };
        let tokens = skill.estimate_frontmatter_tokens();
        assert!(tokens > 0);
    }
}
