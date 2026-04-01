//! Skill resolution order and deduplication.
//!
//! Ported from the getSkillDirCommands function in ref/skills/loadSkillsDir.ts.
//! Skills are loaded from multiple directories with a well-defined priority
//! order, and deduplicated by resolved real path (handling symlinks).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::skills::loader::{load_skills_dir, SkillDefinition, SkillSource};

// ---------------------------------------------------------------------------
// Resolution order
// ---------------------------------------------------------------------------

/// Resolve and load all skills for a given working directory.
///
/// Resolution order (first wins on name collision):
///
/// 1. **Policy-managed** -- skills at the managed config path (e.g.
///    `<managed>/.thundercode/skills/`).
/// 2. **User home** -- `~/.thundercode/skills/`.
/// 3. **Project** -- `.primary/skills/` relative to `cwd` (and ancestor dirs
///    up to home).
/// 4. **Bundled** -- built-in skills (registered separately, not loaded from
///    the filesystem -- included here for conceptual completeness).
/// 5. **MCP** -- skills from MCP servers (loaded at runtime, not here).
///
/// Skills are deduplicated by their resolved real path so that symlinks
/// pointing to the same SKILL.md file are only loaded once.
pub fn resolve_skills(cwd: &Path) -> Vec<SkillDefinition> {
    let mut all_skills: Vec<SkillDefinition> = Vec::new();

    // 1. Policy-managed skills
    if let Some(managed_dir) = managed_skills_dir() {
        match load_skills_dir(&managed_dir, SkillSource::Managed) {
            Ok(skills) => {
                debug!("loaded {} managed skills from {}", skills.len(), managed_dir.display());
                all_skills.extend(skills);
            }
            Err(e) => warn!("error loading managed skills: {}", e),
        }
    }

    // 2. User home skills
    if let Some(user_dir) = user_skills_dir() {
        match load_skills_dir(&user_dir, SkillSource::User) {
            Ok(skills) => {
                debug!("loaded {} user skills from {}", skills.len(), user_dir.display());
                all_skills.extend(skills);
            }
            Err(e) => warn!("error loading user skills: {}", e),
        }
    }

    // 3. Project skills (walk up from cwd to home)
    for dir in project_skills_dirs(cwd) {
        match load_skills_dir(&dir, SkillSource::Project) {
            Ok(skills) => {
                debug!("loaded {} project skills from {}", skills.len(), dir.display());
                all_skills.extend(skills);
            }
            Err(e) => warn!("error loading project skills from {}: {}", dir.display(), e),
        }
    }

    // Deduplicate by real path (handles symlinks + overlapping parent dirs).
    deduplicate_skills(all_skills)
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Returns the managed skills directory, if a managed config path is set.
///
/// The managed path is controlled by the `THUNDERCODE_MANAGED_CONFIG_DIR` env var.
fn managed_skills_dir() -> Option<PathBuf> {
    std::env::var("THUNDERCODE_MANAGED_CONFIG_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|p| PathBuf::from(p).join(".thundercode").join("skills"))
}

/// Returns the user-level skills directory (`~/.thundercode/skills/`).
fn user_skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".thundercode").join("skills"))
}

/// Returns project-level skills directories, walking from `cwd` up to home.
///
/// For each directory from `cwd` to the user's home, we check for a
/// `.primary/skills/` subdirectory. The list is ordered from deepest (cwd)
/// to shallowest (home), so closer skills take precedence.
fn project_skills_dirs(cwd: &Path) -> Vec<PathBuf> {
    let home = dirs::home_dir();
    let mut dirs = Vec::new();
    let mut current = cwd.to_path_buf();

    loop {
        let skills_dir = current.join(".thundercode").join("skills");
        dirs.push(skills_dir);

        // Stop at home directory (inclusive).
        if let Some(ref h) = home {
            if current == *h {
                break;
            }
        }

        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => break,
        }
    }

    dirs
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

/// Deduplicate skills by resolved real path (first wins).
///
/// Two skills that resolve to the same canonical filesystem path (via symlinks
/// or overlapping parent directory walks) are treated as duplicates. When a
/// duplicate is found, only the first occurrence (higher priority) is kept.
fn deduplicate_skills(skills: Vec<SkillDefinition>) -> Vec<SkillDefinition> {
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for skill in skills {
        // Deduplicate by real path (if available).
        if let Some(ref rp) = skill.real_path {
            if seen_paths.contains(rp) {
                debug!(
                    "skipping duplicate skill '{}' from {} (same file already loaded)",
                    skill.name, skill.source
                );
                continue;
            }
            seen_paths.insert(rp.clone());
        }

        // Also deduplicate by name (first wins in resolution order).
        if seen_names.contains(&skill.name) {
            debug!(
                "skipping duplicate skill name '{}' from {} (name already registered)",
                skill.name, skill.source
            );
            continue;
        }
        seen_names.insert(skill.name.clone());

        result.push(skill);
    }

    let dedup_count = seen_paths.len().saturating_sub(result.len());
    if dedup_count > 0 {
        debug!("deduplicated {} skills (same file)", dedup_count);
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_skill(base: &Path, name: &str, desc: &str) {
        let skill_dir = base.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\ndescription: {desc}\n---\nContent for {name}\n"),
        )
        .unwrap();
    }

    #[test]
    fn test_deduplicate_by_name() {
        let tmp = TempDir::new().unwrap();

        // Create two skill dirs with the same skill name
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        create_skill(&dir_a, "my-skill", "First");
        create_skill(&dir_b, "my-skill", "Second");

        let skills_a = load_skills_dir(&dir_a, SkillSource::Managed).unwrap();
        let skills_b = load_skills_dir(&dir_b, SkillSource::User).unwrap();

        let all: Vec<SkillDefinition> = skills_a.into_iter().chain(skills_b).collect();
        let deduped = deduplicate_skills(all);

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].description, "First"); // First wins
        assert_eq!(deduped[0].source, SkillSource::Managed);
    }

    #[test]
    fn test_deduplicate_by_real_path() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");
        create_skill(&skills_dir, "real-skill", "The real one");

        // Create a symlink to the same directory
        let link_dir = tmp.path().join("link-skills");
        fs::create_dir_all(&link_dir).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                skills_dir.join("real-skill"),
                link_dir.join("real-skill"),
            )
            .unwrap();

            let orig = load_skills_dir(&skills_dir, SkillSource::User).unwrap();
            let linked = load_skills_dir(&link_dir, SkillSource::Project).unwrap();

            let all: Vec<SkillDefinition> = orig.into_iter().chain(linked).collect();
            let deduped = deduplicate_skills(all);

            assert_eq!(deduped.len(), 1);
            assert_eq!(deduped[0].source, SkillSource::User); // First wins
        }
    }

    #[test]
    fn test_resolution_order_first_wins() {
        let tmp = TempDir::new().unwrap();

        let managed = tmp.path().join("managed");
        let user = tmp.path().join("user");
        let project = tmp.path().join("project");

        create_skill(&managed, "shared", "Managed version");
        create_skill(&user, "shared", "User version");
        create_skill(&project, "shared", "Project version");

        let all: Vec<SkillDefinition> = [
            (managed, SkillSource::Managed),
            (user, SkillSource::User),
            (project, SkillSource::Project),
        ]
        .into_iter()
        .flat_map(|(dir, src)| load_skills_dir(&dir, src).unwrap())
        .collect();

        let deduped = deduplicate_skills(all);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].source, SkillSource::Managed);
        assert_eq!(deduped[0].description, "Managed version");
    }

    #[test]
    fn test_different_skills_kept() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join("skills");

        create_skill(&skills_dir, "alpha", "Alpha skill");
        create_skill(&skills_dir, "beta", "Beta skill");

        let skills = load_skills_dir(&skills_dir, SkillSource::User).unwrap();
        let deduped = deduplicate_skills(skills);

        assert_eq!(deduped.len(), 2);
        let names: HashSet<&str> = deduped.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains("alpha"));
        assert!(names.contains("beta"));
    }

    #[test]
    fn test_project_skills_dirs_returns_dirs() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("a").join("b").join("c");
        fs::create_dir_all(&cwd).unwrap();

        let dirs = project_skills_dirs(&cwd);
        // Should contain at least the cwd-level directory
        assert!(!dirs.is_empty());
        assert!(dirs[0].ends_with(".primary/skills"));
    }
}
