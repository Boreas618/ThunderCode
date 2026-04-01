//! ThunderCode skill loading and management.
//!
//! This crate handles:
//! - Parsing YAML frontmatter from markdown skill files ([`frontmatter`])
//! - Loading skills from `skills/` directories ([`loader`])
//! - Resolving skills across multiple directories with deduplication ([`resolution`])
//! - Dynamically discovering skills based on file paths ([`discovery`])
//! - Expanding skill prompts with arguments ([`invocation`])
//!
//! Ported from ref/skills/loadSkillsDir.ts`, `ref/skills/bundledSkills.ts`,
//! and `ref/utils/frontmatterParser.ts`.

pub mod frontmatter;
pub mod loader;
pub mod resolution;
pub mod discovery;
pub mod invocation;

// Re-export the most commonly used types at the crate root.
pub use frontmatter::{parse_skill_file, SkillFrontmatter};
pub use loader::{load_all_skills, load_skills_dir, SkillContext, SkillDefinition, SkillSource};
pub use resolution::resolve_skills;
pub use discovery::{discover_skills_for_file, discover_skills_for_files};
pub use invocation::{expand_skill_prompt, expand_skill_prompt_with_session};
