//! Memory types: taxonomy, file metadata, and frontmatter.
//!
//! Ported from ref/memdir/memoryTypes.ts` and `ref/utils/memory/types.ts`.
//!
//! Memories are constrained to four types capturing context NOT derivable
//! from the current project state. Code patterns, architecture, git history,
//! and file structure are derivable (via grep/git/RULES.md) and should NOT
//! be saved as memories.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MemoryType -- the four-type taxonomy
// ---------------------------------------------------------------------------

/// The four memory types. Each captures information not derivable from
/// the current project state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    /// Information about the user's role, goals, responsibilities, and knowledge.
    User,
    /// Guidance the user has given about how to approach work.
    Feedback,
    /// Information about ongoing work, goals, initiatives, bugs, or incidents.
    Project,
    /// Pointers to where information can be found in external systems.
    Reference,
}

impl MemoryType {
    /// All valid memory type values.
    pub const ALL: &'static [MemoryType] = &[
        MemoryType::User,
        MemoryType::Feedback,
        MemoryType::Project,
        MemoryType::Reference,
    ];

    /// The string label used in frontmatter.
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::User => "user",
            MemoryType::Feedback => "feedback",
            MemoryType::Project => "project",
            MemoryType::Reference => "reference",
        }
    }
}

impl fmt::Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MemoryType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(MemoryType::User),
            "feedback" => Ok(MemoryType::Feedback),
            "project" => Ok(MemoryType::Project),
            "reference" => Ok(MemoryType::Reference),
            _ => Err(()),
        }
    }
}

/// Parse a raw frontmatter value into a [`MemoryType`].
///
/// Invalid or missing values return `None` -- legacy files without a
/// `type:` field keep working, files with unknown types degrade gracefully.
pub fn parse_memory_type(raw: &str) -> Option<MemoryType> {
    MemoryType::from_str(raw).ok()
}

// ---------------------------------------------------------------------------
// MemoryFrontmatter
// ---------------------------------------------------------------------------

/// YAML frontmatter parsed from a memory file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryFrontmatter {
    /// Human-readable name for the memory.
    pub name: String,

    /// One-line description used for relevance decisions.
    pub description: String,

    /// Memory type from the four-type taxonomy.
    #[serde(rename = "type")]
    pub memory_type: Option<MemoryType>,
}

// ---------------------------------------------------------------------------
// MemoryFile
// ---------------------------------------------------------------------------

/// A fully loaded memory file: metadata + body content.
#[derive(Debug, Clone)]
pub struct MemoryFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Relative filename within the memory directory (e.g. `user_role.md`).
    pub name: String,
    /// One-line description from frontmatter.
    pub description: String,
    /// Memory type (may be `None` for legacy files without a type field).
    pub memory_type: Option<MemoryType>,
    /// Body content after the frontmatter.
    pub content: String,
}

// ---------------------------------------------------------------------------
// MemoryHeader -- lightweight scan result (no body)
// ---------------------------------------------------------------------------

/// Lightweight metadata extracted from a memory file scan.
/// Used for relevance selection without loading the full body.
#[derive(Debug, Clone)]
pub struct MemoryHeader {
    /// Relative filename within the memory directory.
    pub filename: String,
    /// Absolute file path.
    pub file_path: PathBuf,
    /// File modification time in milliseconds since epoch.
    pub mtime_ms: i64,
    /// Description from frontmatter, if present.
    pub description: Option<String>,
    /// Memory type from frontmatter, if present.
    pub memory_type: Option<MemoryType>,
}

// ---------------------------------------------------------------------------
// RulesMd types
// ---------------------------------------------------------------------------

/// Source location of a RULES.md file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RulesMdSource {
    /// Policy-managed (e.g. `/etc/thundercode/RULES.md`).
    Managed,
    /// User home (e.g. `~/.thundercode/RULES.md`).
    User,
    /// Project root or parent directory (`RULES.md`, `.primary/RULES.md`).
    Project,
    /// Project-local gitignored (`RULES.local.md`).
    Local,
    /// Auto-memory entrypoint (`MEMORY.md` in memory dir).
    AutoMem,
    /// Team memory entrypoint.
    TeamMem,
}

/// A loaded RULES.md / instruction file.
#[derive(Debug, Clone)]
pub struct RulesMdFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// File content (after frontmatter strip, comment strip, truncation).
    pub content: String,
    /// Where this file was discovered.
    pub source: RulesMdSource,
    /// Glob patterns from `paths:` frontmatter (if any).
    pub globs: Option<Vec<String>>,
    /// Path of the file that @included this one (if any).
    pub parent: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// EntrypointTruncation
// ---------------------------------------------------------------------------

/// Result of truncating a MEMORY.md entrypoint to the line/byte caps.
#[derive(Debug, Clone)]
pub struct EntrypointTruncation {
    /// The (possibly truncated) content.
    pub content: String,
    /// Original line count before truncation.
    pub line_count: usize,
    /// Original byte count before truncation.
    pub byte_count: usize,
    /// Whether the content was truncated by line count.
    pub was_line_truncated: bool,
    /// Whether the original content exceeded the byte cap.
    pub was_byte_truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_type_round_trip() {
        for ty in MemoryType::ALL {
            let s = ty.as_str();
            let parsed = parse_memory_type(s);
            assert_eq!(parsed, Some(*ty), "round-trip failed for {s}");
        }
    }

    #[test]
    fn parse_memory_type_invalid() {
        assert_eq!(parse_memory_type("unknown"), None);
        assert_eq!(parse_memory_type(""), None);
        assert_eq!(parse_memory_type("User"), None); // case-sensitive
    }

    #[test]
    fn memory_type_display() {
        assert_eq!(MemoryType::User.to_string(), "user");
        assert_eq!(MemoryType::Feedback.to_string(), "feedback");
        assert_eq!(MemoryType::Project.to_string(), "project");
        assert_eq!(MemoryType::Reference.to_string(), "reference");
    }

    #[test]
    fn frontmatter_serde() {
        let fm = MemoryFrontmatter {
            name: "test".to_string(),
            description: "a test memory".to_string(),
            memory_type: Some(MemoryType::User),
        };
        let yaml = serde_yaml::to_string(&fm).unwrap();
        assert!(yaml.contains("name: test"));
        assert!(yaml.contains("type: user"));

        let round_tripped: MemoryFrontmatter = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(round_tripped, fm);
    }
}
