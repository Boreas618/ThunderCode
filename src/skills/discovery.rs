//! Dynamic skill discovery based on file paths.
//!
//! Ported from the dynamic skill discovery and conditional skill activation
//! logic in ref/skills/loadSkillsDir.ts.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;
use tracing::debug;

use crate::skills::loader::SkillDefinition;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Discover skills whose path patterns match the given file path.
///
/// Returns references to skills whose `paths` frontmatter patterns match
/// `file_path` relative to `cwd`. Skills without `paths` (unconditional)
/// are not returned here; they are always active.
pub fn discover_skills_for_file<'a>(
    file_path: &str,
    skills: &'a [SkillDefinition],
) -> Vec<&'a SkillDefinition> {
    skills
        .iter()
        .filter(|skill| skill_matches_file(skill, file_path))
        .collect()
}

/// Discover skills whose path patterns match any of the given file paths.
///
/// This is the batch version of [`discover_skills_for_file`], useful when
/// multiple files have been touched in a single operation.
pub fn discover_skills_for_files<'a>(
    file_paths: &[&str],
    cwd: &Path,
    skills: &'a [SkillDefinition],
) -> Vec<&'a SkillDefinition> {
    skills
        .iter()
        .filter(|skill| {
            if !skill.is_conditional() {
                return false;
            }
            file_paths
                .iter()
                .any(|fp| skill_matches_file(skill, &make_relative(fp, cwd)))
        })
        .collect()
}

/// Walk up from file paths to discover `.primary/skills/` directories.
///
/// Only discovers directories _below_ `cwd` (cwd-level skills are loaded at
/// startup). Returns directories sorted deepest first so that skills closer
/// to the file take precedence.
pub fn discover_skill_dirs_for_paths(file_paths: &[&str], cwd: &Path) -> Vec<std::path::PathBuf> {
    let cwd_str = cwd.to_string_lossy();
    let cwd_prefix = if cwd_str.ends_with(std::path::MAIN_SEPARATOR) {
        cwd_str.to_string()
    } else {
        format!("{}{}", cwd_str, std::path::MAIN_SEPARATOR)
    };

    let mut new_dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for file_path in file_paths {
        let fp = Path::new(file_path);
        let mut current = match fp.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };

        // Walk up to cwd but NOT including cwd itself.
        while current
            .to_string_lossy()
            .starts_with(cwd_prefix.as_str())
        {
            let skills_dir = current.join(".thundercode").join("skills");
            let skills_dir_str = skills_dir.to_string_lossy().to_string();

            if !seen.contains(&skills_dir_str) {
                seen.insert(skills_dir_str);
                if skills_dir.is_dir() {
                    debug!("discovered skills dir: {}", skills_dir.display());
                    new_dirs.push(skills_dir);
                }
            }

            match current.parent() {
                Some(parent) if parent != current => {
                    current = parent.to_path_buf();
                }
                _ => break,
            }
        }
    }

    // Sort by depth, deepest first.
    new_dirs.sort_by(|a, b| {
        let a_depth = a.components().count();
        let b_depth = b.components().count();
        b_depth.cmp(&a_depth)
    });

    new_dirs
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Check whether a single skill's path patterns match a file path.
fn skill_matches_file(skill: &SkillDefinition, file_path: &str) -> bool {
    let patterns = match &skill.paths {
        Some(p) if !p.is_empty() => p,
        _ => return false,
    };

    // Build a GlobSet from the skill's patterns.
    match build_glob_set(patterns) {
        Some(gs) => gs.is_match(file_path),
        None => {
            // If glob compilation fails, fall back to simple prefix matching.
            patterns.iter().any(|p| file_path.starts_with(p))
        }
    }
}

/// Compile a list of glob patterns into a `GlobSet`.
fn build_glob_set(patterns: &[String]) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(g) => {
                builder.add(g);
            }
            Err(e) => {
                debug!("invalid glob pattern '{}': {}", pattern, e);
                // Add a pattern that matches the literal prefix as fallback.
                if let Ok(g) = Glob::new(&format!("{}/**", pattern)) {
                    builder.add(g);
                }
            }
        }
    }
    builder.build().ok()
}

/// Make a file path relative to cwd.
fn make_relative(file_path: &str, cwd: &Path) -> String {
    let fp = Path::new(file_path);
    match fp.strip_prefix(cwd) {
        Ok(rel) => rel.to_string_lossy().to_string(),
        Err(_) => file_path.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::loader::{SkillContext, SkillSource};

    fn make_skill(name: &str, paths: Option<Vec<&str>>) -> SkillDefinition {
        SkillDefinition {
            name: name.to_string(),
            description: format!("Skill {name}"),
            when_to_use: None,
            prompt_template: String::new(),
            source: SkillSource::Project,
            arg_names: None,
            allowed_tools: None,
            model: None,
            context: SkillContext::Inline,
            paths: paths.map(|v| v.into_iter().map(String::from).collect()),
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
        }
    }

    #[test]
    fn test_discover_matching_skills() {
        let skills = vec![
            make_skill("rust-skill", Some(vec!["src/**/*.rs"])),
            make_skill("ts-skill", Some(vec!["src/**/*.ts"])),
            make_skill("all-skill", None), // unconditional
        ];

        let matches = discover_skills_for_file("src/main.rs", &skills);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "rust-skill");
    }

    #[test]
    fn test_discover_no_match() {
        let skills = vec![make_skill("rust-skill", Some(vec!["src/**/*.rs"]))];
        let matches = discover_skills_for_file("docs/readme.md", &skills);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_discover_multiple_matches() {
        let skills = vec![
            make_skill("src-skill", Some(vec!["src/**"])),
            make_skill("rs-skill", Some(vec!["**/*.rs"])),
        ];

        let matches = discover_skills_for_file("src/lib.rs", &skills);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_unconditional_not_returned() {
        let skills = vec![
            make_skill("always", None),
            make_skill("conditional", Some(vec!["src/**"])),
        ];

        let matches = discover_skills_for_file("src/lib.rs", &skills);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "conditional");
    }

    #[test]
    fn test_batch_discovery() {
        let skills = vec![
            make_skill("rs-skill", Some(vec!["**/*.rs"])),
            make_skill("ts-skill", Some(vec!["**/*.ts"])),
        ];

        let cwd = Path::new("/project");
        let matches = discover_skills_for_files(
            &["/project/src/main.rs", "/project/src/app.ts"],
            cwd,
            &skills,
        );
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_make_relative() {
        let cwd = Path::new("/home/user/project");
        assert_eq!(
            make_relative("/home/user/project/src/main.rs", cwd),
            "src/main.rs"
        );
        assert_eq!(
            make_relative("../other/file.rs", cwd),
            "../other/file.rs"
        );
    }
}
