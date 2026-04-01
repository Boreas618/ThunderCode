//! Skill invocation -- expanding skill prompts with arguments.
//!
//! Ported from createSkillCommand.getPromptForCommand in
//! ref/skills/loadSkillsDir.ts.

use crate::types::ContentBlockParam;
use regex::Regex;
use std::sync::LazyLock;

use crate::skills::loader::SkillDefinition;

// ---------------------------------------------------------------------------
// Argument substitution regex
// ---------------------------------------------------------------------------

/// Matches `$ARGUMENTS` or `${ARGUMENTS}` in skill templates.
static ARGUMENTS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{?ARGUMENTS\}?").unwrap());

/// Matches `$1`, `$2`, ... or `${1}`, `${2}`, ... positional placeholders.
static POSITIONAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{?(\d+)\}?").unwrap());

/// Matches `${THUNDERCODE_SKILL_DIR}` placeholder.
static SKILL_DIR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{THUNDERCODE_SKILL_DIR\}").unwrap());

/// Matches `${THUNDERCODE_SESSION_ID}` placeholder.
static SESSION_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{THUNDERCODE_SESSION_ID\}").unwrap());

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Expand a skill's prompt template with the given arguments.
///
/// Performs the following substitutions:
/// - `$ARGUMENTS` / `${ARGUMENTS}` -> the full `args` string
/// - `$1`, `$2`, ... -> positional arguments (split by whitespace)
/// - `${THUNDERCODE_SKILL_DIR}` -> the skill's root directory (if available)
/// - Prepends "Base directory for this skill: <dir>" if a skill root is set
///
/// Returns a list of `ContentBlockParam` blocks ready to send to the model.
pub fn expand_skill_prompt(skill: &SkillDefinition, args: &str) -> Vec<ContentBlockParam> {
    expand_skill_prompt_with_session(skill, args, None)
}

/// Like [`expand_skill_prompt`] but also substitutes `${THUNDERCODE_SESSION_ID}`.
pub fn expand_skill_prompt_with_session(
    skill: &SkillDefinition,
    args: &str,
    session_id: Option<&str>,
) -> Vec<ContentBlockParam> {
    let mut content = skill.prompt_template.clone();

    // Prepend base directory if the skill has a root.
    if let Some(ref root) = skill.skill_root {
        let root_str = root.to_string_lossy();
        content = format!("Base directory for this skill: {root_str}\n\n{content}");
    }

    // Substitute $ARGUMENTS / ${ARGUMENTS} with the full args string.
    content = ARGUMENTS_RE.replace_all(&content, args).to_string();

    // Substitute named arguments ($1, $2, ...) based on arg_names or positional.
    if let Some(ref arg_names) = skill.arg_names {
        let positional = split_args(args);
        for (i, _name) in arg_names.iter().enumerate() {
            let value = positional.get(i).map(|s| s.as_str()).unwrap_or("");
            let pattern = format!(r"\$\{{?{}\}}?", i + 1);
            if let Ok(re) = Regex::new(&pattern) {
                content = re.replace_all(&content, value).to_string();
            }
        }
    } else {
        // Generic positional substitution.
        let positional = split_args(args);
        content = POSITIONAL_RE
            .replace_all(&content, |caps: &regex::Captures| {
                let idx: usize = caps[1].parse().unwrap_or(0);
                if idx > 0 {
                    positional
                        .get(idx - 1)
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    caps[0].to_string()
                }
            })
            .to_string();
    }

    // Substitute ${THUNDERCODE_SKILL_DIR} with the skill's root directory.
    if let Some(ref root) = skill.skill_root {
        let skill_dir = root.to_string_lossy().to_string();
        content = SKILL_DIR_RE.replace_all(&content, &*skill_dir).to_string();
    }

    // Substitute ${THUNDERCODE_SESSION_ID} with the session ID.
    if let Some(sid) = session_id {
        content = SESSION_ID_RE.replace_all(&content, sid).to_string();
    }

    vec![ContentBlockParam::Text { text: content }]
}

// ---------------------------------------------------------------------------
// Argument splitting
// ---------------------------------------------------------------------------

/// Split arguments by whitespace, respecting quoted strings.
fn split_args(args: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = ' ';

    for ch in args.chars() {
        match ch {
            '"' | '\'' if !in_quote => {
                in_quote = true;
                quote_char = ch;
            }
            c if c == quote_char && in_quote => {
                in_quote = false;
            }
            c if c.is_whitespace() && !in_quote => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    result.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        result.push(trimmed);
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::loader::{SkillContext, SkillSource};
    use std::path::PathBuf;

    fn make_skill(template: &str) -> SkillDefinition {
        SkillDefinition {
            name: "test-skill".to_string(),
            description: "Test".to_string(),
            when_to_use: None,
            prompt_template: template.to_string(),
            source: SkillSource::User,
            arg_names: None,
            allowed_tools: None,
            model: None,
            context: SkillContext::Inline,
            paths: None,
            hooks: None,
            version: None,
            content_length: template.len(),
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
    fn test_expand_no_placeholders() {
        let skill = make_skill("Do the thing.");
        let result = expand_skill_prompt(&skill, "some args");
        assert_eq!(result.len(), 1);
        match &result[0] {
            ContentBlockParam::Text { text } => assert_eq!(text, "Do the thing."),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_expand_arguments_placeholder() {
        let skill = make_skill("Review $ARGUMENTS for issues.");
        let result = expand_skill_prompt(&skill, "src/main.rs");
        match &result[0] {
            ContentBlockParam::Text { text } => {
                assert_eq!(text, "Review src/main.rs for issues.");
            }
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_expand_braced_arguments() {
        let skill = make_skill("Review ${ARGUMENTS} for issues.");
        let result = expand_skill_prompt(&skill, "my-file.rs");
        match &result[0] {
            ContentBlockParam::Text { text } => {
                assert_eq!(text, "Review my-file.rs for issues.");
            }
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_expand_positional_args() {
        let skill = make_skill("File: $1, Message: $2");
        let result = expand_skill_prompt(&skill, "foo.rs bar");
        match &result[0] {
            ContentBlockParam::Text { text } => {
                assert_eq!(text, "File: foo.rs, Message: bar");
            }
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_expand_with_base_dir() {
        let mut skill = make_skill("Template content");
        skill.skill_root = Some(PathBuf::from("/home/user/.thundercode/skills/my-skill"));

        let result = expand_skill_prompt(&skill, "");
        match &result[0] {
            ContentBlockParam::Text { text } => {
                assert!(text.starts_with("Base directory for this skill:"));
                assert!(text.contains("Template content"));
            }
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_expand_skill_dir_placeholder() {
        let mut skill = make_skill("Run ${THUNDERCODE_SKILL_DIR}/script.sh");
        skill.skill_root = Some(PathBuf::from("/skills/my-skill"));

        let result = expand_skill_prompt(&skill, "");
        match &result[0] {
            ContentBlockParam::Text { text } => {
                assert!(text.contains("/skills/my-skill/script.sh"));
            }
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_expand_session_id() {
        let skill = make_skill("Session: ${THUNDERCODE_SESSION_ID}");
        let result = expand_skill_prompt_with_session(&skill, "", Some("sess-123"));
        match &result[0] {
            ContentBlockParam::Text { text } => {
                assert_eq!(text, "Session: sess-123");
            }
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn test_split_args_basic() {
        assert_eq!(split_args("a b c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_split_args_quoted() {
        assert_eq!(
            split_args(r#"foo "hello world" bar"#),
            vec!["foo", "hello world", "bar"]
        );
    }

    #[test]
    fn test_split_args_empty() {
        assert!(split_args("").is_empty());
        assert!(split_args("   ").is_empty());
    }
}
