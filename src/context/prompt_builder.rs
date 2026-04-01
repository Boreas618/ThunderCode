//! System prompt assembly.
//!
//! Combines all context sources, tool prompts, and static instruction
//! sections into the final system prompt array sent to the model.
//!
//! Ported from the TypeScript `getSystemPrompt()` flow which collects
//! a prefix, intro, tool sections, environment details, user context
//! (RULES.md), system context (git status), and date.

use crate::config::SettingsJson;
use crate::constants::prompts;
use crate::types::tool::Tool;

use crate::context::system_context::SystemContext;
use crate::context::user_context::UserContext;

// ---------------------------------------------------------------------------
// SystemPromptBuilder
// ---------------------------------------------------------------------------

/// Stateless builder that assembles the complete system prompt.
pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    /// Build the system prompt string from all available context.
    ///
    /// # Arguments
    ///
    /// * `system_context` -- git status, platform, shell, etc.
    /// * `user_context` -- RULES.md content and current date.
    /// * `tools` -- slice of enabled tools (used for the "Using your tools"
    ///   section and per-tool prompt contributions).
    /// * `model` -- model identifier (used for knowledge-cutoff note).
    /// * `config` -- merged settings (checked for `output_style`,
    ///   `include_git_instructions`, custom system prompts, etc.).
    pub fn build(
        system_context: &SystemContext,
        user_context: &UserContext,
        tools: &[&dyn Tool],
        model: &str,
        config: &SettingsJson,
    ) -> String {
        let mut sections: Vec<String> = Vec::new();

        // 1. Prefix
        sections.push(prompts::DEFAULT_PREFIX.to_string());

        // 2. Intro
        let has_output_style = config
            .extra
            .get("outputStyle")
            .and_then(|v| v.as_str())
            .is_some();
        sections.push(prompts::simple_intro_section(has_output_style));

        // 3. System reminders guidance
        sections.push(prompts::SYSTEM_REMINDERS_SECTION.to_string());

        // 4. Using your tools
        let has_task_tool = tools.iter().any(|t| t.name() == "TodoWrite");
        sections.push(prompts::using_your_tools_section(has_task_tool));

        // 5. Tone and style
        sections.push(prompts::tone_and_style_section());

        // 6. Executing actions with care
        sections.push(prompts::ACTIONS_SECTION.to_string());

        // 7. Output efficiency
        sections.push(prompts::OUTPUT_EFFICIENCY_SECTION.to_string());

        // 8. Summarize tool results
        sections.push(prompts::SUMMARIZE_TOOL_RESULTS_SECTION.to_string());

        // 9. Hooks guidance
        sections.push(prompts::HOOKS_SECTION.to_string());

        // 10. Knowledge cutoff
        if let Some(cutoff) = prompts::get_knowledge_cutoff(model) {
            sections.push(format!(
                "Assistant knowledge cutoff is {cutoff}."
            ));
        }

        // ---- Dynamic boundary (separates cacheable from per-conversation) ---
        sections.push(prompts::SYSTEM_PROMPT_DYNAMIC_BOUNDARY.to_string());

        // 11. Environment details
        sections.push(format_env_section(system_context));

        // 12. Git status (system context)
        let include_git = config
            .extra
            .get("includeGitInstructions")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if include_git {
            if let Some(ref gs) = system_context.git_status {
                sections.push(format!("gitStatus: {gs}"));
            }
        }

        // 13. RULES.md / memory content (user context)
        if let Some(ref md) = user_context.memory_entrypoint {
            sections.push(md.clone());
        }

        // 14. Current date (user context)
        sections.push(format!(
            "# currentDate\nToday's date is {}.",
            user_context.current_date
        ));

        // ---- Custom system prompt overrides ---------------------------------
        // If `custom_system_prompt` is set in config, it replaces everything
        // above.  `append_system_prompt` is appended.
        // We check `extra` since the thundercode-types SettingsJson is minimal.
        if let Some(custom) = config.extra.get("customSystemPrompt").and_then(|v| v.as_str()) {
            if !custom.is_empty() {
                return custom.to_string();
            }
        }

        let mut prompt = sections.join("\n\n");

        if let Some(append) = config.extra.get("appendSystemPrompt").and_then(|v| v.as_str()) {
            if !append.is_empty() {
                prompt.push_str("\n\n");
                prompt.push_str(append);
            }
        }

        prompt
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format the environment details section.
fn format_env_section(ctx: &SystemContext) -> String {
    let mut lines = Vec::new();
    lines.push("Here is useful information about the environment you are running in:".to_string());
    lines.push("<env>".to_string());
    lines.push(format!("Working directory: {}", ctx.cwd.display()));
    lines.push(format!(
        "Is directory a git repo: {}",
        if ctx.git_status.is_some() { "Yes" } else { "No" }
    ));
    lines.push(format!("Platform: {}", ctx.platform));
    lines.push(format!("Shell: {}", ctx.shell));
    lines.push(format!("OS Version: {}", ctx.os_version));
    lines.push("</env>".to_string());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_system_context() -> SystemContext {
        SystemContext {
            git_status: Some("Current branch: main\n\nStatus:\n(clean)".into()),
            branch: Some("main".into()),
            user: Some("Test User".into()),
            recent_commits: vec!["abc1234 initial commit".into()],
            cwd: PathBuf::from("/tmp/test-project"),
            platform: "darwin".into(),
            shell: "/bin/zsh".into(),
            os_version: "Darwin 24.0.0".into(),
        }
    }

    fn sample_user_context() -> UserContext {
        UserContext {
            rules_md_files: vec![],
            memory_entrypoint: Some("# From /tmp/test-project/RULES.md\n\nAlways test.".into()),
            current_date: "2026-03-31".into(),
        }
    }

    #[test]
    fn build_includes_all_sections() {
        let sys = sample_system_context();
        let usr = sample_user_context();
        let tools: Vec<&dyn Tool> = vec![];
        let config = SettingsJson::default();

        let prompt = SystemPromptBuilder::build(&sys, &usr, &tools, "gpt-4o", &config);

        // Check key sections are present.
        assert!(prompt.contains(prompts::DEFAULT_PREFIX));
        assert!(prompt.contains("Working directory: /tmp/test-project"));
        assert!(prompt.contains("Platform: darwin"));
        assert!(prompt.contains("Is directory a git repo: Yes"));
        assert!(prompt.contains("Current branch: main"));
        assert!(prompt.contains("Always test."));
        assert!(prompt.contains("Today's date is 2026-03-31"));
        assert!(prompt.contains("knowledge cutoff"));
    }

    #[test]
    fn build_without_git() {
        let sys = SystemContext {
            git_status: None,
            branch: None,
            user: None,
            recent_commits: vec![],
            cwd: PathBuf::from("/tmp/no-git"),
            platform: "linux".into(),
            shell: "/bin/bash".into(),
            os_version: "Linux 6.5.0".into(),
        };
        let usr = UserContext {
            rules_md_files: vec![],
            memory_entrypoint: None,
            current_date: "2026-01-01".into(),
        };
        let config = SettingsJson::default();

        let prompt = SystemPromptBuilder::build(&sys, &usr, &[], "gpt-4o-mini", &config);

        assert!(prompt.contains("Is directory a git repo: No"));
        assert!(!prompt.contains("gitStatus:"));
    }

    #[test]
    fn custom_system_prompt_replaces_all() {
        let sys = sample_system_context();
        let usr = sample_user_context();
        let mut config = SettingsJson::default();
        config.extra.insert(
            "customSystemPrompt".into(),
            serde_json::Value::String("You are a custom bot.".into()),
        );

        let prompt = SystemPromptBuilder::build(&sys, &usr, &[], "gpt-4o", &config);
        assert_eq!(prompt, "You are a custom bot.");
    }

    #[test]
    fn append_system_prompt() {
        let sys = sample_system_context();
        let usr = sample_user_context();
        let mut config = SettingsJson::default();
        config.extra.insert(
            "appendSystemPrompt".into(),
            serde_json::Value::String("EXTRA INSTRUCTION".into()),
        );

        let prompt = SystemPromptBuilder::build(&sys, &usr, &[], "gpt-4o", &config);
        assert!(prompt.ends_with("EXTRA INSTRUCTION"));
    }

    #[test]
    fn env_section_format() {
        let sys = sample_system_context();
        let section = format_env_section(&sys);
        assert!(section.contains("<env>"));
        assert!(section.contains("</env>"));
        assert!(section.contains("Platform: darwin"));
        assert!(section.contains("Shell: /bin/zsh"));
    }
}
