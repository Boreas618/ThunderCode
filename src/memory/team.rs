//! Team memory support.
//!
//! Ported from ref/memdir/teamMemPaths.ts` and `ref/memdir/teamMemPrompts.ts`.
//!
//! Team memory is a subdirectory of the auto-memory directory:
//! `~/.thundercode/projects/<slug>/memory/team/`
//!
//! It stores memories shared across all users working in the same project.

use std::path::PathBuf;

use crate::memory::memdir::get_memory_dir_path;
use crate::memory::types::MemoryFile;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Returns the team memory directory path for a given project slug.
///
/// `~/.thundercode/projects/<slug>/memory/team/`
pub fn get_team_memory_dir(project_slug: &str) -> PathBuf {
    get_memory_dir_path(project_slug).join("team")
}

/// Returns the team memory entrypoint (MEMORY.md) path.
pub fn get_team_memory_entrypoint(project_slug: &str) -> PathBuf {
    get_team_memory_dir(project_slug).join("MEMORY.md")
}

/// Check if a file path is within the team memory directory.
pub fn is_team_mem_path(file_path: &std::path::Path, project_slug: &str) -> bool {
    let team_dir = get_team_memory_dir(project_slug);
    file_path.starts_with(&team_dir)
}

// ---------------------------------------------------------------------------
// Prompt generation
// ---------------------------------------------------------------------------

/// Build the team memory prompt section from a list of team memory files.
///
/// Returns a formatted markdown section summarizing team memories, suitable
/// for inclusion in a system prompt.
pub fn get_team_memory_prompt_section(team_memories: &[MemoryFile]) -> String {
    if team_memories.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "## Team memories".to_string(),
        String::new(),
        "The following memories are shared across all team members working in this project:".to_string(),
        String::new(),
    ];

    for memory in team_memories {
        let type_tag = memory
            .memory_type
            .map(|t| format!("[{}] ", t.as_str()))
            .unwrap_or_default();
        lines.push(format!("### {type_tag}{}", memory.name));
        if !memory.description.is_empty() {
            lines.push(format!("*{}*", memory.description));
        }
        lines.push(String::new());
        lines.push(memory.content.clone());
        lines.push(String::new());
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::MemoryType;

    #[test]
    fn team_dir_path() {
        let dir = get_team_memory_dir("github.com-user-repo");
        assert!(dir.to_string_lossy().contains("memory"));
        assert!(dir.to_string_lossy().contains("team"));
    }

    #[test]
    fn team_entrypoint_path() {
        let path = get_team_memory_entrypoint("github.com-user-repo");
        assert!(path.to_string_lossy().ends_with("MEMORY.md"));
        assert!(path.to_string_lossy().contains("team"));
    }

    #[test]
    fn is_team_path() {
        let slug = "github.com-user-repo";
        let team_dir = get_team_memory_dir(slug);
        let team_file = team_dir.join("test.md");
        assert!(is_team_mem_path(&team_file, slug));

        let non_team_file = get_memory_dir_path(slug).join("personal.md");
        assert!(!is_team_mem_path(&non_team_file, slug));
    }

    #[test]
    fn prompt_section_empty() {
        let result = get_team_memory_prompt_section(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn prompt_section_with_memories() {
        let memories = vec![
            MemoryFile {
                path: PathBuf::from("/test/merge_freeze.md"),
                name: "Merge freeze".to_string(),
                description: "Code freeze for release".to_string(),
                memory_type: Some(MemoryType::Project),
                content: "No non-critical merges after 2026-03-05.".to_string(),
            },
            MemoryFile {
                path: PathBuf::from("/test/testing_policy.md"),
                name: "Testing policy".to_string(),
                description: "Always use real DB".to_string(),
                memory_type: Some(MemoryType::Feedback),
                content: "Integration tests must hit a real database.".to_string(),
            },
        ];

        let section = get_team_memory_prompt_section(&memories);
        assert!(section.contains("## Team memories"));
        assert!(section.contains("[project] Merge freeze"));
        assert!(section.contains("[feedback] Testing policy"));
        assert!(section.contains("No non-critical merges"));
        assert!(section.contains("Integration tests"));
    }
}
