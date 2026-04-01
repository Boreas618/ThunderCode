//! ThunderCode memory system.
//!
//! This crate implements the full memory and RULES.md system, ported from
//! the TypeScript reference (`ref/memdir/` and `ref/utils/rulesmd.ts`).
//!
//! ## Modules
//!
//! - **`types`** -- Memory types (taxonomy, file metadata, frontmatter structs).
//! - **`frontmatter`** -- YAML frontmatter parsing and serialization.
//! - **`memdir`** -- Memory directory management, CRUD, truncation, age helpers.
//! - **`rulesmd`** -- RULES.md discovery, loading, and assembly.
//! - **`relevance`** -- Memory relevance scoring for query-time recall.
//! - **`team`** -- Team memory paths and prompt generation.

pub mod types;
pub mod frontmatter;
pub mod memdir;
pub mod rulesmd;
pub mod relevance;
pub mod team;

// Re-export the most commonly used items at the crate root.
pub use types::{
    RulesMdFile, RulesMdSource, EntrypointTruncation, MemoryFile, MemoryFrontmatter,
    MemoryHeader, MemoryType,
};
pub use frontmatter::{parse_frontmatter, serialize_frontmatter};
pub use memdir::{
    get_memory_dir_path, get_project_slug, get_project_slug_from_path,
    truncate_entrypoint_content, MemoryDir, ENTRYPOINT_NAME,
    MAX_ENTRYPOINT_BYTES, MAX_ENTRYPOINT_LINES,
};
pub use rulesmd::{
    assemble_instructions, discover_rules_md_files, load_rules_md,
    load_memory_entrypoint, strip_html_comments,
    MEMORY_INSTRUCTION_PROMPT, MAX_MEMORY_CHARACTER_COUNT,
};
pub use relevance::find_relevant_memories;
pub use team::{
    get_team_memory_dir, get_team_memory_entrypoint, get_team_memory_prompt_section,
    is_team_mem_path,
};
